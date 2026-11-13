import * as rules from "./rules.ts";
import type {
    CampaignDefinition,
    CampaignMachine,
    CampaignRoster,
    CampaignRecord,
    CampaignStep,
    CampaignUnlocks,
} from "./types.ts";

type Exchange = {
    lines: CampaignRecord[];
    index: number;
    reactionInstance: string | null;
    ownerStepId: string;
    phase: "dialogue" | "gap";
    token: string | null;
    until: number | null;
};
type ActiveReaction = { reaction: CampaignRecord; target: string; instanceId: string };
type BackgroundEmoteSequence = {
    faction: string;
    lines: CampaignRecord[];
    index: number;
    ownerStepId: string;
    token: string | null;
    phase: "prepare" | "speaking" | "gap";
    until: number | null;
    ackStartedAt: number | null;
    loop: boolean;
    reaction: ActiveReaction | null;
    loopHadVisibleLine: boolean;
};
type RuntimeState = {
    id: string;
    line: number;
    choices: CampaignRecord;
    completed: string[];
    reactionsShown: string[];
    reactionChoices: CampaignRecord;
    contactsResolved: string[];
    firstContactTarget: string | null;
    done: boolean;
    unlocks: CampaignUnlocks;
};
type CampaignCameraCue = {
    id: string;
    kind: "step" | "contact";
    stepId: string;
    target: string | null;
    camera: CampaignRecord | null;
};

const {
    REACTION_DIALOGUE_FIELDS,
    WAIT_MS,
    EXCHANGE_LINE_MS,
    EXCHANGE_GAP_MS,
    EXCHANGE_SEQUENCE_GAP_MS,
    EXCHANGE_EMOTE_ACK_TIMEOUT_MS,
    METRICS,
    emptyCampaignUnlocks,
    applyCampaignUnlocks,
    campaignUnlocksAtStep,
    zoomOutProgress,
    MENU_GUIDE_OUTCOMES,
    object,
    copy,
} = rules;

export function create(
    definition: CampaignDefinition,
    entry?: string,
    roster: CampaignRoster | null = null,
    clock: CampaignRecord = {},
    hooks: CampaignRecord = {},
): CampaignMachine {
    // Each run owns its definition; editor changes cannot mutate an active game.
    definition = copy(definition);
    const byId = new Map<string, CampaignStep>(definition.steps.map((step) => [step.id, step]));
    const allianceOfferFactionIds = new Set(
        ((roster && roster.factions) || [])
            .filter((faction) => faction.can_request_alliance === true)
            .map((faction) => faction.id),
    );
    const campaignEntry = entry || definition.entry;
    function emitCameraCue(cue: CampaignCameraCue) {
        if (typeof hooks?.onCameraCue === "function") hooks.onCameraCue(cue);
    }
    const state: RuntimeState = {
        id: "",
        line: 0,
        choices: Object.create(null),
        completed: [],
        reactionsShown: [],
        reactionChoices: Object.create(null),
        contactsResolved: [],
        firstContactTarget: null,
        done: false,
        unlocks: emptyCampaignUnlocks(),
    };
    const readClock = (): number => {
        const value =
            typeof clock === "function"
                ? clock()
                : typeof performance !== "undefined" && performance.now
                  ? performance.now()
                  : Date.now();
        return Number.isFinite(value) ? value : Date.now();
    };
    let facts: CampaignRecord = {},
        ui: CampaignRecord = {},
        initial: CampaignRecord | null = null;
    let initialUi: CampaignRecord = {},
        baseline: CampaignRecord = {},
        uiBaseline: CampaignRecord = {},
        externallyPaused = false,
        externalPauseStartedAtMs: number | null = null;
    let currentTimeMs = readClock(),
        waitUntilMs: number | null = null,
        reactionTimerStartedAtMs: number | null = null,
        exchange: Exchange | null = null,
        backgroundEmotes = new Map<string, BackgroundEmoteSequence>(),
        backgroundReactions = new Map<string, ActiveReaction>(),
        exchangeSerial = 0;
    function exchangeOnlyScene(step: CampaignStep) {
        return (
            step.type === "scene" &&
            Array.isArray(step.exchange) &&
            !step.title_key &&
            !step.body_key &&
            !step.lines &&
            !["dialogue", "chapter", "celebration"].includes(step.presentation || "")
        );
    }
    function enter(key: string) {
        if (!byId.has(key)) throw new Error("Unknown campaign destination: " + key);
        if (exchange) clearExchange();
        state.id = key;
        state.line = 0;
        state.done = false;
        state.unlocks = applyCampaignUnlocks(state.unlocks, byId.get(key)!.unlocks);
        baseline = copy(facts);
        uiBaseline = { ...ui };
        currentTimeMs = readClock();
        waitUntilMs = byId.get(key)!.wait_before === true ? currentTimeMs + WAIT_MS : null;
        reactionTimerStartedAtMs = currentTimeMs;
        const step = byId.get(key)!;
        if (step.camera)
            emitCameraCue({
                id: "step:" + key,
                kind: "step",
                stepId: key,
                target: step.camera.to || null,
                camera: copy(step.camera),
            });
        if (step.exchange) beginExchange(step.exchange, null);
    }
    function matches(when: CampaignRecord) {
        if (when.choice) return state.choices[when.choice] === when.equals;
        return Number(facts[when.fact] || 0) >= when.gte;
    }
    function progress() {
        const step = byId.get(state.id),
            trigger = step!.trigger;
        if (!trigger) return { current: 0, target: 0 };
        const reference =
            trigger.scope === "step" ? baseline : trigger.scope === "episode" ? initial || {} : {};
        let current: number,
            target = Number(trigger.value || 1);
        if (trigger.type === "contact" || trigger.type === "defeated" || trigger.type === "eliminated") {
            const field =
                trigger.type === "contact"
                    ? "contact_faction_ids"
                    : trigger.type === "defeated"
                      ? "defeated_faction_ids"
                      : "eliminated_faction_ids";
            if (trigger.type === "contact" && Array.isArray(trigger.targets)) {
                current = trigger.targets.filter(
                    (factionId: string) =>
                        (facts[field] || []).includes(factionId) &&
                        !(reference[field] || []).includes(factionId),
                ).length;
                target = Number(trigger.value || 1);
            } else if (trigger.type === "defeated" && Array.isArray(trigger.targets)) {
                current = trigger.targets.filter(
                    (factionId: string) =>
                        (facts[field] || []).includes(factionId) &&
                        !(reference[field] || []).includes(factionId),
                ).length;
                target = Number(trigger.value || trigger.targets.length);
            } else {
                current =
                    (facts[field] || []).includes(trigger.target) &&
                    !(reference[field] || []).includes(trigger.target)
                        ? 1
                        : 0;
                target = 1;
            }
        } else if (trigger.type === "ui") {
            const outcomes = Array.isArray(trigger.outcomes)
                ? trigger.outcomes
                : trigger.outcome
                  ? [trigger.outcome]
                  : [];
            if (outcomes.length) {
                const outcome = MENU_GUIDE_OUTCOMES[outcomes[0]];
                if (outcome && outcome.kind === "screen") {
                    current = facts.screen === outcome.screen ? 1 : 0;
                    target = 1;
                } else {
                    const currentOutcomes = facts.menu_outcomes || {};
                    const referenceOutcomes = reference.menu_outcomes || {};
                    current = outcomes.reduce(
                        (sum: number, outcomeId: string) =>
                            sum +
                            Number(currentOutcomes[outcomeId] || 0) -
                            Number(referenceOutcomes[outcomeId] || 0),
                        0,
                    );
                    target = 1;
                }
            } else if (trigger.action === "attack_ratio") {
                current = Number(facts.attack_ratio || 0);
                target = Number(trigger.value || 1);
            } else {
                const uiReference =
                    trigger.scope === "step" ? uiBaseline : trigger.scope === "episode" ? initialUi : {};
                current = Number(ui[trigger.action] || 0) - Number(uiReference[trigger.action] || 0);
                target = 1;
            }
        } else if (trigger.type === "building_selected") {
            current = facts.selected_building_kind === trigger.kind ? 1 : 0;
            target = 1;
        } else if (trigger.type === "zoom_out_complete") {
            current = zoomOutProgress(facts);
            target = Number(trigger.value || 1);
            if (Number(facts.zoom_out_events || 0) - Number(reference.zoom_out_events || 0) < 1) current = 0;
        } else if (trigger.type === "zoom_in_complete") {
            const zoomEvents = Number(facts.zoom_in_events || 0) - Number(reference.zoom_in_events || 0);
            const reachedZoom =
                Number.isFinite(Number(facts.camera_zoom)) &&
                Number(facts.camera_zoom) >= Number(facts.camera_zoom_target);
            const triggerDistance = Number(trigger.distance || 12);
            const cameraTargetRadius =
                Number.isFinite(facts.camera_target_radius) && facts.camera_target_radius > 0
                    ? facts.camera_target_radius
                    : triggerDistance;
            const reachedFaction =
                !trigger.target ||
                (Number.isFinite(Number(facts.camera_target_distance)) &&
                    Number(facts.camera_target_distance) <= Math.min(cameraTargetRadius, triggerDistance));
            current = zoomEvents > 0 && reachedZoom && reachedFaction ? 1 : 0;
            target = 1;
        } else if (trigger.type === "camera_target" && Number.isFinite(trigger.distance)) {
            const distance = facts.camera_target_distance;
            const cameraTargetRadius =
                Number.isFinite(facts.camera_target_radius) && facts.camera_target_radius > 0
                    ? facts.camera_target_radius
                    : trigger.distance;
            current =
                typeof distance === "number" && Number.isFinite(distance) && distance <= cameraTargetRadius
                    ? 1
                    : 0;
            target = 1;
        } else if (trigger.type === "alliance_request" && trigger.target) {
            current =
                (facts.alliance_request_faction_ids || []).includes(trigger.target) ||
                (facts.alliance_faction_ids || []).includes(trigger.target)
                    ? 1
                    : 0;
            target = 1;
        } else if (trigger.type === "alliance" && trigger.target) {
            current =
                (facts.alliance_faction_ids || []).includes(trigger.target) &&
                !(reference.alliance_faction_ids || []).includes(trigger.target)
                    ? 1
                    : 0;
            target = 1;
        } else if (trigger.type === "troops") {
            current = Number(facts.troops || 0);
        } else if (trigger.type === "attack" && trigger.target) {
            current =
                Number((facts.attacks_by_faction_id || {})[trigger.target] || 0) -
                Number((reference.attacks_by_faction_id || {})[trigger.target] || 0);
        } else if (trigger.type === "fleet" && trigger.target) {
            const byTarget =
                Number((facts.transport_fleets_by_faction_id || {})[trigger.target] || 0) -
                Number((reference.transport_fleets_by_faction_id || {})[trigger.target] || 0);
            const byType = trigger.unit
                ? Number((facts.fleets_by_type || {})[trigger.unit] || 0) -
                  Number((reference.fleets_by_type || {})[trigger.unit] || 0)
                : byTarget;
            current = Math.min(byTarget, byType);
        } else if (trigger.type === "fleet" && trigger.unit) {
            current =
                Number((facts.fleets_by_type || {})[trigger.unit] || 0) -
                Number((reference.fleets_by_type || {})[trigger.unit] || 0);
        } else if (trigger.type === "resource_transfer" && trigger.recipient) {
            const resources = (trigger.resources && trigger.resources.slice().sort().join("_")) || "total";
            const currentCounts =
                (facts.resource_transfers_by_recipient_faction_id || {})[trigger.recipient] || {};
            const baselineCounts =
                (reference.resource_transfers_by_recipient_faction_id || {})[trigger.recipient] || {};
            current = Number(currentCounts[resources] || 0) - Number(baselineCounts[resources] || 0);
        } else if (trigger.type === "structure_level") {
            const kind = String(trigger.kind || "").toLowerCase();
            // A faction-scoped trigger counts that campaign faction's buildings instead of the player's.
            const currentLevels = trigger.faction
                ? (facts.faction_structure_levels || {})[trigger.faction] || {}
                : facts.structure_levels || {};
            const baselineLevels = trigger.faction
                ? (reference.faction_structure_levels || {})[trigger.faction] || {}
                : reference.structure_levels || {};
            current = Number(currentLevels[kind] || 0) - Number(baselineLevels[kind] || 0);
            target = Number(trigger.value || 1);
        } else {
            let field = METRICS[trigger.type];
            if (trigger.type === "territory" && trigger.scope === "total") field = "tiles";
            current = Number(facts[field] || 0) - Number(reference[field] || 0);
        }
        if (trigger.type === "territory" && trigger.scope === "total" && trigger.comparison === "lte") {
            return { current: current <= target ? target : 0, target };
        }
        return { current: Math.min(target, Math.max(0, current)), target };
    }
    function targetUnavailable(step: CampaignStep, result: { current: number; target: number }) {
        const trigger = step.trigger;
        if (!trigger || !["objective", "guide"].includes(step.type)) return false;
        const eliminated = facts.eliminated_faction_ids || [];
        const targets = Array.isArray(trigger.targets)
            ? trigger.targets
            : [trigger.target || trigger.recipient];
        const factionTargets: string[] = targets.filter(
            (target: unknown): target is string => typeof target === "string" && target !== "player",
        );
        if (!factionTargets.length) return false;
        const field =
            trigger.type === "contact"
                ? "contact_faction_ids"
                : trigger.type === "defeated"
                  ? "defeated_faction_ids"
                  : trigger.type === "eliminated"
                    ? "eliminated_faction_ids"
                    : null;
        const observed = field ? facts[field] || [] : [];
        const reference =
            trigger.scope === "step" ? baseline : trigger.scope === "episode" ? initial || {} : {};
        const priorObserved = field ? reference[field] || [] : [];
        const remaining = factionTargets.filter(
            (target: string) =>
                !eliminated.includes(target) &&
                !(observed.includes(target) && !priorObserved.includes(target)) &&
                !(trigger.type === "contact" && observed.includes(target)),
        );
        if (factionTargets.length > 1) return result.current + remaining.length < result.target;
        if (
            trigger.type === "contact" &&
            observed.includes(factionTargets[0]) &&
            result.current < result.target
        )
            return true;
        if (
            trigger.type === "alliance" &&
            (facts.alliance_faction_ids || []).includes(factionTargets[0]) &&
            result.current < result.target
        )
            return true;
        return eliminated.includes(factionTargets[0]) && result.current < result.target;
    }
    let activeReaction: ActiveReaction | null = null;
    function activePreDefeatTargets() {
        if (state.done) return [];
        const step = byId.get(state.id);
        if (
            !step ||
            !["objective", "guide"].includes(step.type) ||
            !step.trigger ||
            step.trigger.type !== "defeated"
        )
            return [];
        const objectiveTargets = Array.isArray(step.trigger.targets)
            ? step.trigger.targets
            : [step.trigger.target];
        return (definition.reactions || [])
            .filter(
                (reaction: CampaignRecord) =>
                    reaction.when.type === "pre_defeat" &&
                    reaction.during === step.id &&
                    objectiveTargets.includes(reaction.when.target) &&
                    !state.reactionsShown.includes(reaction.id + "@" + reaction.when.target),
            )
            .map((reaction: CampaignRecord) => reaction.when.target);
    }
    function loopShouldStop(sequence: BackgroundEmoteSequence) {
        if (!sequence.loop) return false;
        if (sequence.reaction?.reaction.during && sequence.reaction.reaction.during !== state.id) return true;
        const step = byId.get(state.id)!;
        const result = ["objective", "guide"].includes(step.type) && step.trigger ? progress() : null;
        if (!result || result.current >= result.target || targetUnavailable(step, result)) return true;
        if (
            step.guide?.kind === "world" &&
            step.guide.target === "target_action" &&
            step.trigger?.type === "defeated" &&
            step.trigger.target
        ) {
            const target = step.trigger.target;
            return (
                Number((facts.attacks_by_faction_id || {})[target] || 0) >
                Number((baseline.attacks_by_faction_id || {})[target] || 0)
            );
        }
        return false;
    }
    function timerOccurrence(reaction: CampaignRecord) {
        if (reactionTimerStartedAtMs === null) return null;
        const afterMs = reaction.when.after_ms;
        const repeatMs = reaction.when.repeat_ms;
        if (!Number.isSafeInteger(afterMs) || afterMs <= 0) return null;
        if (repeatMs != null && (!Number.isSafeInteger(repeatMs) || repeatMs <= 0)) return null;
        const firstDueAt = reactionTimerStartedAtMs + afterMs;
        if (repeatMs == null)
            return {
                dueAt: firstDueAt,
                instanceId: reaction.id,
                ready: currentTimeMs >= firstDueAt,
                nextDueAt: firstDueAt,
                repeats: false,
            };
        if (currentTimeMs < firstDueAt)
            return {
                dueAt: firstDueAt,
                instanceId: reaction.id + "@timer:0",
                ready: false,
                nextDueAt: firstDueAt,
                repeats: true,
            };
        const occurrence = Math.floor((currentTimeMs - firstDueAt) / repeatMs);
        const dueAt = firstDueAt + occurrence * repeatMs;
        const ready = currentTimeMs - dueAt <= EXCHANGE_GAP_MS;
        return {
            dueAt,
            instanceId: reaction.id + "@timer:" + occurrence,
            ready,
            nextDueAt: dueAt + repeatMs,
            repeats: true,
        };
    }
    function nextReaction() {
        const contactFactionIds = facts.contact_faction_ids || [];
        const gameplayActive = !["scene", "choice", "end"].includes(byId.get(state.id)!.type);
        return (
            (definition.reactions || [])
                .flatMap((reaction: CampaignRecord, index: number) => {
                    const type = reaction.when.type;
                    const isFirstContact = type === "first_contact";
                    const isContact = type === "contact" || isFirstContact;
                    const duringActive = !reaction.during || reaction.during === state.id;
                    let targets: string[] = [];
                    if (isFirstContact && !state.firstContactTarget) {
                        const eligible = reaction.when.targets || [];
                        targets = eligible
                            .filter(
                                (factionId: string) =>
                                    contactFactionIds.includes(factionId) &&
                                    !state.contactsResolved.includes(factionId),
                            )
                            .slice(0, 1);
                    } else if (type === "contact" && reaction.when.target) {
                        const isNewContact =
                            !reaction.during ||
                            !(baseline.contact_faction_ids || []).includes(reaction.when.target);
                        if (
                            duringActive &&
                            isNewContact &&
                            contactFactionIds.includes(reaction.when.target) &&
                            !state.contactsResolved.includes(reaction.when.target)
                        )
                            targets = [reaction.when.target];
                    } else if (type === "contact" && reaction.when.relation) {
                        const factions = new Map<string, CampaignRecord>(
                            ((roster && roster.factions) || []).map((faction) => [faction.id, faction]),
                        );
                        targets = contactFactionIds.filter((factionId: string) => {
                            const faction = factions.get(factionId);
                            return (
                                !state.contactsResolved.includes(factionId) &&
                                (!faction || faction.can_request_alliance !== false) &&
                                ((faction && faction.relation) || "neutral") === reaction.when.relation
                            );
                        });
                    }
                    if (isContact) {
                        if (!gameplayActive) return [];
                        if (reaction.after && !state.completed.includes(reaction.after)) return [];
                        return targets.map((target) => ({
                            reaction,
                            index,
                            target,
                            isContact,
                            isFirstContact,
                            priority: isFirstContact ? 0 : reaction.when.target ? 1 : 2,
                            instanceId: reaction.id + "@" + target,
                        }));
                    }
                    if (type === "alliance") {
                        return gameplayActive &&
                            (facts.alliance_faction_ids || []).includes(reaction.when.target) &&
                            (!reaction.after || state.completed.includes(reaction.after))
                            ? [
                                  {
                                      reaction,
                                      index,
                                      target: reaction.when.target,
                                      isContact: false,
                                      isFirstContact: false,
                                      priority: 1,
                                      instanceId: reaction.id,
                                  },
                              ]
                            : [];
                    }
                    if (type === "pre_defeat") {
                        const ready =
                            gameplayActive &&
                            duringActive &&
                            (facts.pre_defeat_faction_ids || []).includes(reaction.when.target);
                        return ready
                            ? [
                                  {
                                      reaction,
                                      index,
                                      target: reaction.when.target,
                                      isContact: false,
                                      isFirstContact: false,
                                      priority: 0,
                                      instanceId: reaction.id + "@" + reaction.when.target,
                                  },
                              ]
                            : [];
                    }
                    if (type === "timer") {
                        const step = byId.get(state.id)!;
                        const result =
                            ["objective", "guide"].includes(step.type) && step.trigger ? progress() : null;
                        const timer = timerOccurrence(reaction);
                        const ready =
                            gameplayActive &&
                            duringActive &&
                            result &&
                            result.current < result.target &&
                            timer &&
                            timer.ready;
                        return ready
                            ? [
                                  {
                                      reaction,
                                      index,
                                      target: "",
                                      isContact: false,
                                      isFirstContact: false,
                                      priority: 2,
                                      instanceId: timer.instanceId,
                                      dueAt: timer.dueAt,
                                  },
                              ]
                            : [];
                    }
                    const receipt =
                        type === "support" &&
                        facts.support_deliveries_by_faction_id &&
                        facts.support_deliveries_by_faction_id[reaction.when.target];
                    const ready =
                        gameplayActive &&
                        state.completed.includes(reaction.after) &&
                        receipt &&
                        receipt.deliveries > 0;
                    return ready
                        ? [
                              {
                                  reaction,
                                  index,
                                  receipt,
                                  target: reaction.when.target,
                                  isContact: false,
                                  isFirstContact: false,
                                  priority: 3,
                                  instanceId: reaction.id,
                              },
                          ]
                        : [];
                })
                .filter(
                    (item: CampaignRecord) =>
                        !state.reactionsShown.includes(item.instanceId) &&
                        !backgroundReactions.has(item.instanceId),
                )
                .sort(
                    (a: CampaignRecord, b: CampaignRecord) =>
                        a.priority - b.priority ||
                        (Number.isFinite(a.dueAt) && Number.isFinite(b.dueAt) ? b.dueAt - a.dueAt : 0) ||
                        Number((a.receipt && a.receipt.first_tick) || 0) -
                            Number((b.receipt && b.receipt.first_tick) || 0) ||
                        a.index - b.index,
                )[0] || null
        );
    }
    function nextTimerDelayMs() {
        const remaining = [...backgroundEmotes.values()].map((sequence) =>
            sequence.phase === "prepare"
                ? 100
                : Math.max(1, (sequence.until || currentTimeMs) - currentTimeMs),
        );
        const step = byId.get(state.id)!;
        const dialogueActive = Boolean(activeReaction || exchange?.phase === "dialogue");
        if (
            !state.done &&
            !dialogueActive &&
            step.trigger &&
            ["objective", "guide"].includes(step.type) &&
            reactionTimerStartedAtMs !== null
        ) {
            const result = progress();
            if (result.current < result.target) {
                remaining.push(
                    ...(definition.reactions || [])
                        .filter(
                            (reaction: CampaignRecord) =>
                                reaction.when.type === "timer" &&
                                reaction.during === state.id &&
                                !(reaction.when.repeat_ms == null && state.reactionsShown.includes(reaction.id)),
                        )
                        .map((reaction: CampaignRecord) => {
                            const timer = timerOccurrence(reaction);
                            if (!timer) return null;
                            if (backgroundReactions.has(timer.instanceId)) return null;
                            if (state.reactionsShown.includes(timer.instanceId))
                                return timer.repeats ? timer.nextDueAt - currentTimeMs : null;
                            if (!timer.ready) return timer.nextDueAt - currentTimeMs;
                            return 1;
                        })
                        .filter((delay: number | null): delay is number => Number.isFinite(delay)),
                );
            }
        }
        return remaining.length ? Math.max(1, Math.min(...remaining)) : null;
    }
    function pausesForStep(step: CampaignStep) {
        if (step.pause_game === true || step.type === "choice" || step.type === "end") return true;
        if (step.type !== "scene") return false;
        if (
            ["dialogue", "chapter", "celebration"].includes(step.presentation || "")
        )
            return true;
        return Boolean(step.title_key || step.body_key || step.lines);
    }
    function targetedFactionActionStep(step: CampaignStep) {
        return Boolean(
            step.guide?.kind === "world" &&
                ["target_action", "nameplate", "player", "expand"].includes(step.guide.target) &&
                step.trigger &&
                ["attack", "contact", "defeated", "eliminated"].includes(step.trigger.type),
        );
    }
    function targetAttackPauseForStep(step: CampaignStep) {
        if (
            !targetedFactionActionStep(step) ||
            step.trigger?.type === "contact" ||
            facts.target_action_mode !== "attack"
        )
            return false;
        const targets = Array.isArray(step.trigger.targets)
            ? step.trigger.targets
            : [step.trigger.target];
        const target = targets.find(
            (id: string) =>
                id &&
                !(facts.defeated_faction_ids || []).includes(id) &&
                !(facts.eliminated_faction_ids || []).includes(id),
        );
        if (!target) return false;
        return (
            Number((facts.attacks_by_faction_id || {})[target] || 0) <=
            Number((baseline.attacks_by_faction_id || {})[target] || 0)
        );
    }
    function view() {
        currentTimeMs = readClock();
        const step = byId.get(state.id)!;
        const expansionStarted =
            step!.paused_action === "expand_once_then_resume" &&
            Number(facts.wilderness_orders_accepted || 0) > Number(baseline.wilderness_orders_accepted || 0);
        const targetAttackPause = targetAttackPauseForStep(step!);
        const nonBlockingContactReaction =
            activeReaction &&
            ["contact", "first_contact"].includes(activeReaction.reaction.when.type) &&
            activeReaction.reaction.pause_game !== true;
        const dynamicAttackPause =
            targetAttackPause && !nonBlockingContactReaction && activeReaction?.reaction.pause_game !== false;
        const targetedFactionStep = targetedFactionActionStep(step!);
        const stepPaused = targetedFactionStep
            ? step!.trigger?.type !== "contact" && dynamicAttackPause
            : pausesForStep(step!) && !expansionStarted;
        const stepPausedAction = stepPaused
            ? targetAttackPause
                ? "attack_once_then_resume"
                : step!.paused_action || null
            : null;
        if (exchange) {
            const exchangeLine = exchange.lines[exchange.index];
            const modalLine: CampaignStep | null =
                exchange.phase === "dialogue"
                    ? {
                          id: "exchange-" + exchange.token,
                          type: "scene",
                          speaker: exchangeLine.speaker,
                          title_key: exchangeLine.title_key,
                          show_title: exchangeLine.show_title,
                          body_key: exchangeLine.body_key || exchangeLine.emote_text_key,
                          presentation: "dialogue",
                          ...(exchange.reactionInstance === null && step!.camera
                              ? { camera: step!.camera }
                              : {}),
                      }
                    : null;
            return {
                definition,
                step: modalLine || step,
                line: modalLine || step,
                progress: progress(),
                paused: exchange.phase === "dialogue" || Boolean(activeReaction) || stepPaused,
                paused_action:
                    exchange.phase === "dialogue" || activeReaction ? null : stepPausedAction,
                done: false,
                choices: [],
                state,
                unlocks: copy(state.unlocks),
                waiting: exchange.phase !== "dialogue",
                wait_remaining_ms: Math.max(1, (exchange.until || currentTimeMs) - currentTimeMs),
                exchange: exchangeLine,
                exchange_phase: exchange.phase,
                exchange_token: exchange.token,
                pre_defeat_targets: activePreDefeatTargets(),
                ...(activeReaction
                    ? {
                          reaction: activeReaction.reaction.id,
                          reactionInstance: activeReaction.instanceId,
                          reactionTarget: activeReaction.target,
                          reactionData: activeReaction.reaction,
                      }
                    : {}),
            };
        }
        if (activeReaction) {
            const reaction = activeReaction.reaction;
            const hasChoices = Array.isArray(reaction.choices) && reaction.choices.length > 0;
            const factionSpeaker =
                reaction.when.type === "first_contact" || reaction.when.relation
                    ? Object.keys(definition.speakers || {}).find(
                          (key) => definition.speakers[key].faction === activeReaction!.target,
                      )
                    : null;
            const response: CampaignStep = {
                ...Object.fromEntries(REACTION_DIALOGUE_FIELDS.map((field) => [field, reaction[field]])),
                id: "reaction-" + activeReaction.instanceId,
                type: hasChoices ? "choice" : "scene",
                speaker: reaction.speaker || factionSpeaker,
                presentation: "dialogue",
            };
            return {
                definition,
                step: response,
                line: response,
                progress: progress(),
                paused: reaction.pause_game === false ? stepPaused : true,
                paused_action: reaction.pause_game === false ? stepPausedAction : null,
                done: false,
                choices: reaction.choices || [],
                state,
                unlocks: copy(state.unlocks),
                waiting: false,
                reaction: reaction.id,
                reactionInstance: activeReaction.instanceId,
                reactionTarget: activeReaction.target,
                reactionData: reaction,
                pre_defeat_targets: activePreDefeatTargets(),
            };
        }
        const line = step!.lines ? step!.lines[state.line] : step;
        const waitRemainingMs = waitUntilMs === null ? 0 : Math.max(0, waitUntilMs - currentTimeMs);
        const paused = stepPaused;
        return {
            definition,
            step,
            line,
            progress: progress(),
            paused,
            paused_action: stepPausedAction,
            done: state.done,
            choices: step!.choices || [],
            state,
            unlocks: copy(state.unlocks),
            waiting: waitRemainingMs > 0,
            wait_remaining_ms: waitRemainingMs,
            timer_remaining_ms: nextTimerDelayMs(),
            pre_defeat_targets: activePreDefeatTargets(),
        };
    }
    function setPaused(paused: boolean) {
        paused = Boolean(paused);
        const now = readClock();
        if (paused && !externallyPaused) {
            externalPauseStartedAtMs = now;
        } else if (!paused && externallyPaused && externalPauseStartedAtMs !== null) {
            const elapsed = now - externalPauseStartedAtMs;
            if (exchange?.phase === "gap" && exchange.until !== null) exchange.until += elapsed;
            externalPauseStartedAtMs = null;
        }
        externallyPaused = paused;
        currentTimeMs = now;
    }
    function clearExchange() {
        exchange = null;
    }
    function cancelBackgroundEmoteFor(step: CampaignRecord) {
        if (!step.emote) return;
        const faction = definition.speakers?.[step.speaker]?.faction || "player";
        const sequence = backgroundEmotes.get(faction);
        if (sequence) finishBackgroundSequence(sequence);
    }
    function clearBackgroundCue(sequence: BackgroundEmoteSequence) {
        if (!sequence.token || !["prepare", "speaking"].includes(sequence.phase)) return;
        const line = sequence.lines[sequence.index];
        const token = sequence.token;
        sequence.token = null;
        if (line && hooks && typeof hooks.onExchangeClear === "function")
            hooks.onExchangeClear(line, token);
    }
    function finishBackgroundSequence(sequence: BackgroundEmoteSequence, clearCue = true) {
        if (backgroundEmotes.get(sequence.faction) !== sequence) return;
        if (clearCue) clearBackgroundCue(sequence);
        backgroundEmotes.delete(sequence.faction);
        const reaction = sequence.reaction;
        if (
            reaction &&
            ![...backgroundEmotes.values()].some((item) => item.reaction?.instanceId === reaction.instanceId)
        ) {
            backgroundReactions.delete(reaction.instanceId);
            completeReaction(reaction);
        }
    }
    function startBackgroundEmotes(
        lines: CampaignRecord[],
        ownerStepId: string,
        reaction: ActiveReaction | null = null,
    ) {
        const byFaction = new Map<string, CampaignRecord[]>();
        lines.forEach((line) => {
            if (line.presentation === "dialogue") return;
            const faction = definition.speakers?.[line.speaker]?.faction || "player";
            const sequence = byFaction.get(faction) || [];
            sequence.push(line);
            byFaction.set(faction, sequence);
        });
        if (reaction?.reaction.loop === true) backgroundReactions.set(reaction.instanceId, reaction);
        byFaction.forEach((speakerLines, faction) => {
            const previous = backgroundEmotes.get(faction);
            if (previous) finishBackgroundSequence(previous);
            backgroundEmotes.set(faction, {
                faction,
                lines: speakerLines,
                index: 0,
                ownerStepId,
                token: null,
                phase: "prepare",
                until: null,
                ackStartedAt: null,
                loop: reaction?.reaction.loop === true,
                reaction: reaction?.reaction.loop === true ? reaction : null,
                loopHadVisibleLine: false,
            });
        });
        if (reaction && reaction.reaction.loop !== true) completeReaction(reaction);
        else if (reaction?.reaction.loop === true && !byFaction.size) completeReaction(reaction);
    }
    function advanceBackgroundSequence(sequence: BackgroundEmoteSequence) {
        sequence.index++;
        if (sequence.index < sequence.lines.length) {
            sequence.phase = "prepare";
            sequence.token = null;
            sequence.ackStartedAt = null;
            sequence.until = null;
        } else if (sequence.loop && sequence.loopHadVisibleLine && !loopShouldStop(sequence)) {
            sequence.index = 0;
            sequence.loopHadVisibleLine = false;
            sequence.phase = "prepare";
            sequence.token = null;
            sequence.ackStartedAt = null;
            sequence.until = null;
        } else finishBackgroundSequence(sequence);
    }
    function updateBackgroundEmotes() {
        [...backgroundEmotes.values()].forEach((sequence) => {
            if (backgroundEmotes.get(sequence.faction) !== sequence) return;
            if (sequence.loop && loopShouldStop(sequence)) {
                finishBackgroundSequence(sequence);
                return;
            }
            const line = sequence.lines[sequence.index];
            if (sequence.phase === "prepare") {
                if ((facts.eliminated_faction_ids || []).includes(sequence.faction)) {
                    advanceBackgroundSequence(sequence);
                    return;
                }
                if (sequence.token === null)
                    sequence.token = sequence.ownerStepId + ":emote:" + ++exchangeSerial + ":" + sequence.index;
                const result =
                    hooks && typeof hooks.onExchangeLine === "function"
                        ? hooks.onExchangeLine(line, sequence.token)
                        : true;
                if (result === "unavailable") {
                    clearBackgroundCue(sequence);
                    advanceBackgroundSequence(sequence);
                } else if (result === "pending") {
                    if (sequence.ackStartedAt === null) sequence.ackStartedAt = currentTimeMs;
                    if (currentTimeMs - sequence.ackStartedAt >= EXCHANGE_EMOTE_ACK_TIMEOUT_MS) {
                        clearBackgroundCue(sequence);
                        advanceBackgroundSequence(sequence);
                    }
                } else if (result !== false) {
                    sequence.loopHadVisibleLine = true;
                    sequence.phase = "speaking";
                    sequence.until = currentTimeMs + EXCHANGE_LINE_MS;
                }
                return;
            }
            if (sequence.until !== null && currentTimeMs < sequence.until) return;
            if (sequence.phase === "speaking") {
                clearBackgroundCue(sequence);
                if (sequence.loop && loopShouldStop(sequence)) {
                    finishBackgroundSequence(sequence, false);
                    return;
                }
                sequence.phase = "gap";
                sequence.until = currentTimeMs +
                    (sequence.loop
                        ? EXCHANGE_SEQUENCE_GAP_MS
                        : Number.isInteger(line.delay_ms)
                          ? line.delay_ms
                          : EXCHANGE_GAP_MS);
                return;
            }
            advanceBackgroundSequence(sequence);
        });
    }
    function beginExchange(
        lines: CampaignRecord[],
        reaction: ActiveReaction | null = null,
        ownerStepId = state.id,
    ) {
        if (!lines.some((line) => line.presentation === "dialogue")) {
            if (reaction && activeReaction?.instanceId === reaction.instanceId) activeReaction = null;
            startBackgroundEmotes(lines, ownerStepId, reaction);
            return;
        }
        if (reaction) activeReaction = reaction;
        exchange = {
            lines,
            index: 0,
            reactionInstance: reaction?.instanceId || null,
            ownerStepId,
            phase: "gap",
            token: null,
            until: null,
        };
        processExchangeUntilDialogue();
    }
    function processExchangeUntilDialogue() {
        if (!exchange) return false;
        while (exchange.index < exchange.lines.length) {
            if (exchange.lines[exchange.index].presentation === "dialogue") {
                exchange.phase = "dialogue";
                exchange.token = exchange.ownerStepId + ":dialogue:" + ++exchangeSerial;
                exchange.until = null;
                return true;
            }
            const start = exchange.index;
            while (
                exchange.index < exchange.lines.length &&
                exchange.lines[exchange.index].presentation !== "dialogue"
            )
                exchange.index++;
            startBackgroundEmotes(exchange.lines.slice(start, exchange.index), exchange.ownerStepId);
        }
        endExchange();
        return false;
    }
    function endExchange() {
        const finished = exchange!;
        exchange = null;
        if (finished.ownerStepId === state.id) waitUntilMs = null;
        if (
            finished.reactionInstance !== null &&
            activeReaction?.instanceId === finished.reactionInstance
        )
            completeReaction(activeReaction);
    }
    function dispose() {
        clearExchange();
        backgroundEmotes.forEach((sequence) => clearBackgroundCue(sequence));
        backgroundEmotes.clear();
        backgroundReactions.clear();
        activeReaction = null;
        externallyPaused = true;
    }
    function completeReaction(active: ActiveReaction) {
        const reaction = active.reaction;
        const target = active.target;
        if (reaction.when.type === "timer" && reaction.when.repeat_ms != null) {
            const prefix = reaction.id + "@timer:";
            for (let index = state.reactionsShown.length - 1; index >= 0; index--)
                if (state.reactionsShown[index].startsWith(prefix)) state.reactionsShown.splice(index, 1);
        }
        if (!state.reactionsShown.includes(active.instanceId)) state.reactionsShown.push(active.instanceId);
        if (
            ["contact", "first_contact"].includes(reaction.when.type) &&
            !state.contactsResolved.includes(active.target)
        )
            state.contactsResolved.push(active.target);
        if (activeReaction?.instanceId === active.instanceId) activeReaction = null;
        if (reaction.when.type === "timer") reactionTimerStartedAtMs = currentTimeMs = readClock();
        // A response's emote, like a step's, fires when it completes.
        cancelBackgroundEmoteFor(reaction);
        if (hooks && typeof hooks.onStepComplete === "function") hooks.onStepComplete(reaction);
        if (hooks && typeof hooks.onReactionComplete === "function")
            hooks.onReactionComplete(reaction, target);
    }
    function advance(
        choiceId?: string | null,
        expectedStepId?: string,
        allowUnavailable = false,
    ) {
        if (state.done || externallyPaused) return false;
        if (exchange && exchange.phase === "dialogue") {
            if (expectedStepId !== "exchange-" + exchange.token) return false;
            const delay = Number.isInteger(exchange.lines[exchange.index].delay_ms)
                ? exchange.lines[exchange.index].delay_ms
                : 0;
            exchange.index++;
            if (delay > 0) {
                exchange.phase = "gap";
                exchange.until = readClock() + delay;
                return true;
            }
            processExchangeUntilDialogue();
            return true;
        }
        if (activeReaction && (!exchange || exchange.phase === "dialogue")) {
            const reaction = activeReaction.reaction;
            if (expectedStepId !== "reaction-" + activeReaction.instanceId) return false;
            let answer: CampaignRecord | null = null;
            if (reaction.choices && reaction.choices.length) {
                answer = reaction.choices.find((option: CampaignRecord) => option.id === choiceId) || null;
                if (!answer) return false;
                state.reactionChoices[activeReaction.instanceId] = answer.id;
                state.choices[reaction.id] = answer.id;
            }
            const exchangeLines = Array.isArray(answer?.exchange) ? answer.exchange : reaction.exchange;
            if (Array.isArray(exchangeLines) && exchangeLines.length)
                beginExchange(exchangeLines, activeReaction);
            else completeReaction(activeReaction);
            return true;
        }
        if (expectedStepId && expectedStepId !== state.id) return false;
        currentTimeMs = readClock();
        const step = byId.get(state.id)!;
        if (waitUntilMs !== null && currentTimeMs < waitUntilMs) return false;
        if (step!.type === "scene" && step!.lines && state.line + 1 < step!.lines.length) {
            state.line++;
            return true;
        }
        if (["objective", "guide"].includes(step!.type)) {
            const result = progress();
            const unavailable = allowUnavailable && targetUnavailable(step, result);
            if (result.current < result.target && !unavailable) return false;
        }
        if (step!.type === "end") {
            if (!state.completed.includes(step!.id)) state.completed.push(step!.id);
            state.done = true;
            return true;
        }
        let destination = step!.next;
        if (step!.type === "choice") {
            const answer = step!.choices.find((option: CampaignRecord) => option.id === choiceId);
            if (!answer) return false;
            state.choices[step!.id] = answer.id;
            destination = answer.next;
        } else {
            const route = (step!.routes || []).find((route: CampaignRecord) => matches(route.when));
            if (route) destination = route.next;
        }
        if (typeof destination !== "string" || !byId.has(destination)) return false;
        if (!state.completed.includes(step!.id)) state.completed.push(step!.id);
        // A step's emote directive takes effect exactly when the step completes.
        cancelBackgroundEmoteFor(step!);
        if (hooks && typeof hooks.onStepComplete === "function") hooks.onStepComplete(step);
        enter(destination);
        return true;
    }
    function update(nextFacts?: CampaignRecord, nextUi?: CampaignRecord) {
        currentTimeMs = readClock();
        facts = copy(nextFacts || {});
        ui = { ...(nextUi || {}) };
        if (initial == null) {
            initial = copy(facts);
            initialUi = { ...ui };
            baseline = copy(facts);
            uiBaseline = { ...ui };
            if (!exchange)
                waitUntilMs = byId.get(state.id)!.wait_before === true ? currentTimeMs + WAIT_MS : null;
        }
        if (state.done || externallyPaused) return view();
        updateBackgroundEmotes();
        while (!state.done) {
            const availableStep = byId.get(state.id);
            const partner =
                (availableStep!.marker && availableStep!.marker.target) ||
                (availableStep!.trigger &&
                    (availableStep!.trigger.target || availableStep!.trigger.recipient));
            const allianceAlreadyHandled =
                availableStep!.trigger &&
                availableStep!.trigger.type === "ui" &&
                availableStep!.trigger.action === "map_menu_open" &&
                typeof partner === "string" &&
                allianceOfferFactionIds.has(partner) &&
                ((facts.alliance_faction_ids || []).includes(partner) ||
                    (facts.alliance_request_faction_ids || []).includes(partner));
            if (
                availableStep!.unavailable_next &&
                ((facts.eliminated_faction_ids || []).includes(partner) ||
                    (facts.hostile_faction_ids || []).includes(partner) ||
                    allianceAlreadyHandled)
            ) {
                activeReaction = null;
                enter(availableStep!.unavailable_next);
                continue;
            }
            if (exchange) {
                if (exchange.phase === "dialogue") return view();
                if (exchange.until !== null && currentTimeMs < exchange.until) break;
                if (processExchangeUntilDialogue()) return view();
                if (exchange) break;
                continue;
            }
            const step = byId.get(state.id)!;
            const objectiveStep = ["objective", "guide"].includes(step!.type);
            const result = objectiveStep ? progress() : null;
            const unavailable = objectiveStep && targetUnavailable(step, result!);
            if (!activeReaction && !exchange) {
                const next = nextReaction();
                if (next) {
                    if (next.isFirstContact) state.firstContactTarget = next.target;
                    if (["contact", "first_contact"].includes(next.reaction.when.type))
                        emitCameraCue({
                            id: "contact:" + next.instanceId,
                            kind: "contact",
                            stepId: state.id,
                            target: next.target,
                            camera: null,
                        });
                    const active: ActiveReaction = {
                        reaction: next.reaction,
                        target: next.target,
                        instanceId: next.instanceId,
                    };
                    if (next.reaction.presentation === "exchange" && next.reaction.exchange) {
                        beginExchange(next.reaction.exchange, active);
                        continue;
                    }
                    activeReaction = active;
                }
            }
            if (activeReaction) return view();
            // Emote-only scenes hand off immediately; exchanges stop only at a dialogue line.
            if (exchangeOnlyScene(step!) && !exchange) {
                if (!advance(null, step!.id)) break;
                continue;
            }
            if (!objectiveStep) break;
            if (result!.current < result!.target && !unavailable) break;
            if (!advance(null, step!.id, true)) break;
        }
        return view();
    }
    function jump(key: string, nextFacts?: CampaignRecord, priorChoices?: CampaignRecord) {
        activeReaction = null;
        if (nextFacts) facts = copy(nextFacts);
        Object.entries(object(priorChoices) ? priorChoices : {}).forEach(([decisionId, answerId]) => {
            const decision = byId.get(decisionId);
            if (
                decision &&
                decision.type === "choice" &&
                Array.isArray(decision.choices) &&
                decision.choices.some((answer) => answer.id === answerId)
            )
                state.choices[decisionId] = answerId;
        });
        state.unlocks = campaignUnlocksAtStep(byId, campaignEntry, key, state.choices, facts);
        enter(key);
        return view();
    }
    function replaceDefinition(nextDefinition: CampaignDefinition) {
        if (!object(nextDefinition) || !Array.isArray(nextDefinition.steps)) return false;
        const replacement = copy(nextDefinition);
        const nextById = new Map<string, CampaignStep>(
            replacement.steps.filter(object).map((step) => [step.id, step]),
        );
        const previousStep = byId.get(state.id),
            nextStep = nextById.get(state.id);
        if (!nextStep) return false;
        if ((previousStep && previousStep.wait_before === true) !== (nextStep.wait_before === true)) {
            currentTimeMs = readClock();
            waitUntilMs = nextStep.wait_before === true ? currentTimeMs + WAIT_MS : null;
        }
        const resetProgress =
            !previousStep ||
            previousStep.type !== nextStep.type ||
            JSON.stringify(previousStep.trigger) !== JSON.stringify(nextStep.trigger);
        const previousExchangeOwner = activeReaction ? activeReaction.reaction : previousStep;
        const nextExchangeOwner = activeReaction
            ? (replacement.reactions || []).find(
                  (reaction: CampaignRecord) => reaction.id === activeReaction!.reaction.id,
              )
            : nextStep;
        const exchangeChanged =
            exchange &&
            (JSON.stringify(previousExchangeOwner && previousExchangeOwner.exchange) !==
                JSON.stringify(nextExchangeOwner && nextExchangeOwner.exchange) ||
                JSON.stringify(definition.speakers) !== JSON.stringify(replacement.speakers));
        const reactionForExchange =
            activeReaction && nextExchangeOwner
                ? { ...activeReaction, reaction: nextExchangeOwner }
                : null;
        if (exchangeChanged) clearExchange();
        definition = replacement;
        byId.clear();
        nextById.forEach((step, key) => byId.set(key, step));
        state.line = Math.min(state.line, Math.max(0, (nextStep.lines || []).length - 1));
        state.done = state.done && nextStep.type === "end";
        state.completed = state.completed.filter((key) => byId.has(key));
        Object.keys(state.choices).forEach((key) => {
            const decision = byId.get(key);
            if (
                !decision ||
                !Array.isArray(decision.choices) ||
                !decision.choices.some((answer) => answer && answer.id === state.choices[key])
            )
                delete state.choices[key];
        });
        if (resetProgress) {
            baseline = copy(facts);
            uiBaseline = { ...ui };
        }
        if (activeReaction) {
            if (nextExchangeOwner) activeReaction.reaction = nextExchangeOwner;
            else activeReaction = null;
        }
        if (exchangeChanged && nextExchangeOwner && nextExchangeOwner.exchange)
            beginExchange(nextExchangeOwner.exchange, reactionForExchange);
        return true;
    }
    enter(entry || definition.entry);
    return {
        get definition() {
            return definition;
        },
        state,
        update,
        advance,
        jump,
        replaceDefinition,
        setPaused,
        view,
        dispose,
    };
}
