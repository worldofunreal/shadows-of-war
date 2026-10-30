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
    phase: "prepare" | "speaking" | "gap" | "dialogue";
    token: string | null;
    until: number | null;
    ackStartedAt: number | null;
    remainingGap?: number;
};
type ActiveReaction = { reaction: CampaignRecord; target: string; instanceId: string };
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

const {
    REACTION_DIALOGUE_FIELDS,
    WAIT_MS,
    EXCHANGE_LINE_MS,
    EXCHANGE_GAP_MS,
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
        externallyPaused = false;
    let currentTimeMs = readClock(),
        waitUntilMs: number | null = null,
        reactionTimerStartedAtMs: number | null = null,
        exchange: Exchange | null = null,
        exchangeSerial = 0;
    function enter(key: string) {
        if (!byId.has(key)) throw new Error("Unknown campaign destination: " + key);
        clearExchange();
        state.id = key;
        state.line = 0;
        state.done = false;
        state.unlocks = applyCampaignUnlocks(state.unlocks, byId.get(key)!.unlocks);
        baseline = copy(facts);
        uiBaseline = { ...ui };
        currentTimeMs = readClock();
        waitUntilMs = byId.get(key)!.wait_before === true ? currentTimeMs + WAIT_MS : null;
        reactionTimerStartedAtMs = byId.get(key)!.exchange ? null : currentTimeMs;
        if (byId.get(key)!.exchange) beginExchange(byId.get(key)!.exchange, null);
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
            const cameraTargetRadius =
                Number.isFinite(facts.camera_target_radius) && facts.camera_target_radius > 0
                    ? facts.camera_target_radius
                    : Number(trigger.distance || 12);
            const reachedFaction =
                !trigger.target ||
                (Number.isFinite(Number(facts.camera_target_distance)) &&
                    Number(facts.camera_target_distance) <= cameraTargetRadius);
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
                            ["objective", "guide"].includes(step.type) && step.trigger
                                ? progress()
                                : null;
                        const afterMs = reaction.when.after_ms;
                        const ready =
                            gameplayActive &&
                            duringActive &&
                            result &&
                            result.current < result.target &&
                            reactionTimerStartedAtMs !== null &&
                            Number.isSafeInteger(afterMs) &&
                            afterMs > 0 &&
                            currentTimeMs - reactionTimerStartedAtMs >= afterMs;
                        return ready
                            ? [
                                  {
                                      reaction,
                                      index,
                                      target: "",
                                      isContact: false,
                                      isFirstContact: false,
                                      priority: 2,
                                      instanceId: reaction.id,
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
                .filter((item: CampaignRecord) => !state.reactionsShown.includes(item.instanceId))
                .sort(
                    (a: CampaignRecord, b: CampaignRecord) =>
                        a.priority - b.priority ||
                        Number((a.receipt && a.receipt.first_tick) || 0) -
                            Number((b.receipt && b.receipt.first_tick) || 0) ||
                        a.index - b.index,
                )[0] || null
        );
    }
    function nextTimerDelayMs() {
        if (state.done || exchange || activeReaction || reactionTimerStartedAtMs === null) return null;
        const step = byId.get(state.id)!;
        if (!step.trigger || !["objective", "guide"].includes(step.type)) return null;
        const result = progress();
        if (result.current >= result.target) return null;
        const remaining = (definition.reactions || [])
            .filter(
                (reaction: CampaignRecord) =>
                    reaction.when.type === "timer" &&
                    reaction.during === state.id &&
                    !state.reactionsShown.includes(reaction.id) &&
                    Number.isSafeInteger(reaction.when.after_ms) &&
                    reaction.when.after_ms > 0,
            )
            .map(
                (reaction: CampaignRecord) =>
                    reactionTimerStartedAtMs! + reaction.when.after_ms - currentTimeMs,
            );
        return remaining.length ? Math.max(1, Math.min(...remaining)) : null;
    }
    function view() {
        currentTimeMs = readClock();
        const step = byId.get(state.id)!;
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
                      }
                    : null;
            const stepPauses =
                step!.pause_game !== false &&
                (["scene", "choice", "end"].includes(step!.type) || step!.pause_game === true);
            const reactionPause =
                exchange.reactionInstance !== null ? activeReaction?.reaction.pause_game : undefined;
            return {
                definition,
                step: modalLine || step,
                line: modalLine || step,
                progress: progress(),
                paused:
                    reactionPause === false
                        ? false
                        : reactionPause === true ||
                          (exchange.phase === "dialogue" && step!.pause_game !== false) ||
                          stepPauses,
                paused_action: null,
                done: false,
                choices: [],
                state,
                unlocks: copy(state.unlocks),
                waiting: exchange.phase !== "dialogue",
                wait_remaining_ms:
                    exchange.phase === "prepare"
                        ? 100
                        : Math.max(1, (exchange.until || currentTimeMs) - currentTimeMs),
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
                paused: reaction.pause_game !== false,
                paused_action: null,
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
        const expansionStarted =
            step!.paused_action === "expand_once_then_resume" &&
            Number(facts.wilderness_orders_accepted || 0) > Number(baseline.wilderness_orders_accepted || 0);
        const paused =
            (step!.pause_game !== false && ["scene", "choice", "end"].includes(step!.type)) ||
            (step!.pause_game === true && !expansionStarted);
        return {
            definition,
            step,
            line,
            progress: progress(),
            paused,
            paused_action: paused ? step!.paused_action || null : null,
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
        if (paused && !externallyPaused && exchange) {
            if (exchange.phase === "gap") exchange.remainingGap = Math.max(0, exchange.until! - readClock());
            else if (exchange.phase !== "dialogue") {
                clearExchangeLine();
                // Replay only the interrupted line on return; never consume reading time off screen.
                prepareExchangeLine();
            }
        } else if (!paused && externallyPaused && exchange && exchange.phase === "gap") {
            exchange.until = readClock() + (exchange.remainingGap || 0);
            delete exchange.remainingGap;
        }
        externallyPaused = paused;
    }
    function clearExchangeLine() {
        if (
            exchange &&
            ["prepare", "speaking"].includes(exchange.phase) &&
            hooks &&
            typeof hooks.onExchangeClear === "function"
        )
            hooks.onExchangeClear(exchange.lines[exchange.index], exchange.token);
    }
    function clearExchange() {
        clearExchangeLine();
        exchange = null;
    }
    function beginExchange(lines: CampaignRecord[], reactionInstance: string | null) {
        clearExchange();
        exchange = {
            lines,
            index: 0,
            reactionInstance,
            phase: "prepare",
            token: null,
            until: null,
            ackStartedAt: null,
        };
        prepareExchangeLine();
    }
    function prepareExchangeLine() {
        const current = exchange!;
        current.phase = "prepare";
        current.token = state.id + ":" + ++exchangeSerial + ":" + current.index;
        current.until = null;
        current.ackStartedAt = null;
    }
    function updateExchange() {
        const current = exchange!;
        if (current.phase === "prepare") {
            const speaker = definition.speakers && definition.speakers[current.lines[current.index].speaker];
            if (speaker && (facts.eliminated_faction_ids || []).includes(speaker.faction || "player")) {
                current.phase = "gap";
                current.until = readClock() + EXCHANGE_GAP_MS;
                return false;
            }
            const line = current.lines[current.index];
            if ((line.presentation || "emote") === "dialogue") {
                current.phase = "dialogue";
                return false;
            }
            const result =
                hooks && typeof hooks.onExchangeLine === "function"
                    ? hooks.onExchangeLine(current.lines[current.index], current.token)
                    : true;
            // An emote is background noise: a line that cannot show is skipped, never turned into a pause.
            if (result === "unavailable") {
                clearExchangeLine();
                return nextExchangeLine();
            }
            if (result === "pending") {
                if (current.ackStartedAt === null) current.ackStartedAt = currentTimeMs;
                if (currentTimeMs - current.ackStartedAt >= EXCHANGE_EMOTE_ACK_TIMEOUT_MS) {
                    clearExchangeLine();
                    return nextExchangeLine();
                }
                return false;
            }
            const shown = result !== false;
            if (!shown) return false;
            current.phase = "speaking";
            current.until = readClock() + EXCHANGE_LINE_MS;
            return false;
        }
        if (current.phase === "dialogue") return false;
        if (currentTimeMs < current.until!) return false;
        if (current.phase === "speaking") {
            clearExchangeLine();
            current.phase = "gap";
            current.until =
                currentTimeMs +
                (Number.isInteger(current.lines[current.index].delay_ms)
                    ? current.lines[current.index].delay_ms
                    : EXCHANGE_GAP_MS);
            return false;
        }
        return nextExchangeLine();
    }
    function nextExchangeLine() {
        const current = exchange!;
        current.index++;
        if (current.index < current.lines.length) prepareExchangeLine();
        else endExchange();
        return true;
    }
    function endExchange() {
        const finished = exchange!;
        exchange = null;
        waitUntilMs = null;
        if (finished.reactionInstance !== null) completeReaction();
        else reactionTimerStartedAtMs = (currentTimeMs = readClock());
    }
    function dispose() {
        clearExchange();
        externallyPaused = true;
    }
    function completeReaction() {
        const active = activeReaction!;
        const reaction = active.reaction;
        const target = active.target;
        if (!state.reactionsShown.includes(active.instanceId)) state.reactionsShown.push(active.instanceId);
        if (
            ["contact", "first_contact"].includes(reaction.when.type) &&
            !state.contactsResolved.includes(active.target)
        )
            state.contactsResolved.push(active.target);
        activeReaction = null;
        // A response's emote, like a step's, fires when it completes.
        if (hooks && typeof hooks.onStepComplete === "function") hooks.onStepComplete(reaction);
        if (hooks && typeof hooks.onReactionComplete === "function")
            hooks.onReactionComplete(reaction, target);
    }
    function advance(choiceId?: string | null, expectedStepId?: string, allowUnavailable = false) {
        if (state.done || externallyPaused) return false;
        if (exchange) {
            if (exchange.phase !== "dialogue" || expectedStepId !== "exchange-" + exchange.token)
                return false;
            const delay = Number.isInteger(exchange.lines[exchange.index].delay_ms)
                ? exchange.lines[exchange.index].delay_ms
                : EXCHANGE_GAP_MS;
            if (delay > 0) {
                exchange.phase = "gap";
                exchange.until = readClock() + delay;
                return true;
            }
            exchange.index++;
            if (exchange.index < exchange.lines.length) prepareExchangeLine();
            else endExchange();
            return true;
        }
        if (activeReaction) {
            const reaction = activeReaction.reaction;
            if (expectedStepId !== "reaction-" + activeReaction.instanceId) return false;
            if (reaction.choices && reaction.choices.length) {
                const answer = reaction.choices.find((option: CampaignRecord) => option.id === choiceId);
                if (!answer) return false;
                state.reactionChoices[activeReaction.instanceId] = answer.id;
                state.choices[reaction.id] = answer.id;
            }
            if (reaction.exchange) beginExchange(reaction.exchange, activeReaction.instanceId);
            else completeReaction();
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
                if (!updateExchange()) return view();
                continue;
            }
            const step = byId.get(state.id)!;
            const objectiveStep = ["objective", "guide"].includes(step!.type);
            const result = objectiveStep ? progress() : null;
            const unavailable = objectiveStep && targetUnavailable(step, result!);
            if (!activeReaction) {
                const next = nextReaction();
                if (next) {
                    if (next.isFirstContact) state.firstContactTarget = next.target;
                    activeReaction = {
                        reaction: next.reaction,
                        target: next.target,
                        instanceId: next.instanceId,
                    };
                    if (next.reaction.presentation === "exchange" && next.reaction.exchange) {
                        beginExchange(next.reaction.exchange, next.instanceId);
                        continue;
                    }
                }
            }
            if (activeReaction) return view();
            // A scene that only plays an exchange has nothing to read, so it continues once the exchange ends.
            if (step!.type === "scene" && step!.exchange) {
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
        const reactionInstance = activeReaction && activeReaction.instanceId;
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
            beginExchange(nextExchangeOwner.exchange, reactionInstance || null);
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
