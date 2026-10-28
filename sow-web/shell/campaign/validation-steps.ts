import * as rules from "./rules.ts";
import type { CampaignRecord } from "./types.ts";
import type { CampaignValidationContext } from "./validation-context.ts";

export function validateSteps(context: CampaignValidationContext) {
    const {
        definition,
        options,
        roster,
        allowMissingFactionReferences,
        requireSpeakerPortraits,
        steps,
        byId,
        factions,
        rosterFactions,
        speakers,
        errors,
        warnings,
        issue,
        knownFields,
        text,
        checkEmote,
        checkExchange,
        checkDialogueSpeaker,
        notEmoteLine,
        destination,
        condition,
    } = context;
    const {
        TYPES,
        TEAMS,
        CAMERA_SHOTS,
        METRICS,
        WORLD_TARGETS,
        STRUCTURE_LEVEL_LIMITS,
        CAMPAIGN_ACTIONS,
        UI_TARGETS,
        MENU_GUIDE_OUTCOMES,
        own,
        object,
        id,
    } = rules;

    const stepFields = {
        scene: [
            "id",
            "type",
            "title_key",
            "show_title",
            "body_key",
            "speaker",
            "portrait_expression",
            "continue_key",
            "emote",
            "emote_text_key",
            "emote_pinned",
            "presentation",
            "video_src",
            "lines",
            "marker",
            "next",
            "routes",
            "attack_ratio_on_enter",
            "campaign_attack_on_enter",
            "campaign_halt_on_enter",
            "campaign_assault_on_enter",
            "pause_game",
            "wait_before",
            "unlocks",
            "camera",
            "exchange",
            "unavailable_next",
            "campaign_accept_alliance_on_enter",
        ],
        choice: [
            "id",
            "type",
            "title_key",
            "show_title",
            "body_key",
            "speaker",
            "emote",
            "emote_text_key",
            "emote_pinned",
            "marker",
            "choices",
            "attack_ratio_on_enter",
            "campaign_attack_on_enter",
            "campaign_assault_on_enter",
            "pause_game",
            "wait_before",
            "unlocks",
        ],
        objective: [
            "id",
            "type",
            "title_key",
            "body_key",
            "hint_key",
            "speaker",
            "emote",
            "emote_text_key",
            "emote_pinned",
            "trigger",
            "guide",
            "marker",
            "pause_game",
            "paused_action",
            "camera_only",
            "camera",
            "next",
            "routes",
            "attack_ratio_on_enter",
            "campaign_attack_on_enter",
            "campaign_assault_on_enter",
            "unlocks",
            "exchange",
            "unavailable_next",
            "campaign_accept_alliance_on_enter",
        ],
        guide: [
            "id",
            "type",
            "title_key",
            "body_key",
            "hint_key",
            "speaker",
            "emote",
            "emote_text_key",
            "emote_pinned",
            "trigger",
            "guide",
            "marker",
            "pause_game",
            "paused_action",
            "camera_only",
            "camera",
            "next",
            "routes",
            "attack_ratio_on_enter",
            "campaign_attack_on_enter",
            "campaign_assault_on_enter",
            "unlocks",
            "exchange",
            "unavailable_next",
            "campaign_accept_alliance_on_enter",
        ],
        end: [
            "id",
            "type",
            "title_key",
            "show_title",
            "body_key",
            "speaker",
            "presentation",
            "attack_ratio_on_enter",
            "pause_game",
            "unlocks",
        ],
    };

    steps.filter(object).forEach((step) => {
        knownFields(step, stepFields[step.type] || ["id", "type"], step, "fields");
        if (!TYPES.includes(step.type)) issue(step, "type", "Unknown step type.");
        if (
            own(step, "attack_ratio_on_enter") &&
            (!Number.isFinite(step.attack_ratio_on_enter) ||
                step.attack_ratio_on_enter < 0.05 ||
                step.attack_ratio_on_enter > 1)
        )
            issue(step, "attack_ratio_on_enter", "Attack ratio must be between 0.05 and 1.");
        if (own(step, "unlocks")) {
            const unlocks = step.unlocks;
            knownFields(unlocks, ["buildings", "actions"], step, "unlocks");
            if (!object(unlocks))
                issue(step, "unlocks", "Unlocks must list building levels and available actions.");
            else {
                if (unlocks.buildings != null) {
                    if (!object(unlocks.buildings) || Array.isArray(unlocks.buildings))
                        issue(
                            step,
                            "unlocks.buildings",
                            "Building unlocks must map building names to levels.",
                        );
                    else
                        Object.entries(unlocks.buildings).forEach(([kind, level]) => {
                            if (
                                !own(STRUCTURE_LEVEL_LIMITS, kind) ||
                                !Number.isInteger(level) ||
                                level < 1 ||
                                level > STRUCTURE_LEVEL_LIMITS[kind]
                            )
                                issue(
                                    step,
                                    "unlocks.buildings",
                                    "Unknown building or invalid unlock level: " + kind + " " + level + ".",
                                );
                        });
                }
                if (
                    unlocks.actions != null &&
                    (!Array.isArray(unlocks.actions) ||
                        new Set(unlocks.actions).size !== unlocks.actions.length ||
                        unlocks.actions.some((action) => !CAMPAIGN_ACTIONS.includes(action)))
                )
                    issue(step, "unlocks.actions", "Choose distinct supported campaign actions.");
            }
        }
        if (
            own(step, "wait_before") &&
            (typeof step.wait_before !== "boolean" || !["scene", "choice"].includes(step.type))
        )
            issue(step, "wait_before", "The 1.1-second reveal wait only applies to dialogue and decisions.");
        if (own(step, "campaign_assault_on_enter")) {
            const assault = step.campaign_assault_on_enter;
            knownFields(
                assault,
                ["attacker_team", "target", "preserve_relation", "reinforcement", "hold_last_tile"],
                step,
                "campaign_assault_on_enter",
            );
            if (!object(assault) || !TEAMS.includes(assault.attacker_team))
                issue(
                    step,
                    "campaign_assault_on_enter.attacker_team",
                    "Choose the Red or Blue campaign team.",
                );
            if (
                !object(assault) ||
                typeof assault.target !== "string" ||
                !assault.target ||
                (assault.target !== "player" &&
                    assault.target !== "player_and_allies" &&
                    roster &&
                    !rosterFactions.has(assault.target) &&
                    !allowMissingFactionReferences)
            )
                issue(
                    step,
                    "campaign_assault_on_enter.target",
                    "Choose the player, the player and current allies, or an existing campaign faction.",
                );
            if (
                object(assault) &&
                own(assault, "preserve_relation") &&
                typeof assault.preserve_relation !== "boolean"
            )
                issue(
                    step,
                    "campaign_assault_on_enter.preserve_relation",
                    "Choose whether attackers keep their current campaign relationship.",
                );
            if (
                object(assault) &&
                own(assault, "hold_last_tile") &&
                typeof assault.hold_last_tile !== "boolean"
            )
                issue(
                    step,
                    "campaign_assault_on_enter.hold_last_tile",
                    "Choose whether the assault pauses before taking the target's final territory.",
                );
            if (object(assault) && own(assault, "reinforcement")) {
                const reinforcement = assault.reinforcement;
                knownFields(
                    reinforcement,
                    ["capacity_ratio", "interval_seconds"],
                    step,
                    "campaign_assault_on_enter.reinforcement",
                );
                if (
                    !object(reinforcement) ||
                    !Number.isFinite(reinforcement.capacity_ratio) ||
                    reinforcement.capacity_ratio < 1 ||
                    reinforcement.capacity_ratio > 10
                )
                    issue(
                        step,
                        "campaign_assault_on_enter.reinforcement.capacity_ratio",
                        "Strength ratio must be between 1 and 10.",
                    );
                if (
                    !object(reinforcement) ||
                    !Number.isInteger(reinforcement.interval_seconds) ||
                    reinforcement.interval_seconds < 1 ||
                    reinforcement.interval_seconds > 600
                )
                    issue(
                        step,
                        "campaign_assault_on_enter.reinforcement.interval_seconds",
                        "Reinforcement interval must be 1–600 seconds.",
                    );
            }
            if (object(assault) && assault.target === "player_and_allies" && !own(assault, "reinforcement"))
                issue(
                    step,
                    "campaign_assault_on_enter.reinforcement",
                    "An assault on current allies needs a strength ratio and reinforcement interval.",
                );
            if (
                object(assault) &&
                assault.hold_last_tile === true &&
                (!own(assault, "reinforcement") || !["player", "player_and_allies"].includes(assault.target))
            )
                issue(
                    step,
                    "campaign_assault_on_enter.hold_last_tile",
                    "Holding the final territory requires a reinforced assault against the player.",
                );
            const targetFaction =
                object(assault) &&
                roster &&
                Array.isArray(roster.factions) &&
                roster.factions.find((faction) => faction.id === assault.target);
            if (targetFaction && targetFaction.team === assault.attacker_team)
                issue(
                    step,
                    "campaign_assault_on_enter.target",
                    "The target cannot belong to the attacking team.",
                );
        }
        if (own(step, "campaign_attack_on_enter")) {
            const action = step.campaign_attack_on_enter;
            knownFields(action, ["faction_id", "can_attack_player"], step, "campaign_attack_on_enter");
            if (!object(action) || !id(action.faction_id) || typeof action.can_attack_player !== "boolean")
                issue(
                    step,
                    "campaign_attack_on_enter",
                    "Choose an enemy faction and whether it may attack the player.",
                );
            const faction =
                object(action) &&
                roster &&
                Array.isArray(roster.factions) &&
                roster.factions.find((entry) => entry.id === action.faction_id);
            if (faction && faction.relation !== "enemy")
                issue(
                    step,
                    "campaign_attack_on_enter.faction_id",
                    "Attack behavior can only change for an enemy faction.",
                );
            if (!allowMissingFactionReferences && roster && !rosterFactions.has(action && action.faction_id))
                issue(step, "campaign_attack_on_enter.faction_id", "Choose an existing campaign faction.");
        }
        if (own(step, "campaign_accept_alliance_on_enter")) {
            const action = step.campaign_accept_alliance_on_enter;
            knownFields(action, ["faction_id"], step, "campaign_accept_alliance_on_enter");
            if (!object(action) || !rosterFactions.has(action.faction_id))
                issue(
                    step,
                    "campaign_accept_alliance_on_enter",
                    "Choose the faction answering a real alliance request.",
                );
            if (
                !step.trigger ||
                step.trigger.type !== "alliance" ||
                step.trigger.target !== (action && action.faction_id)
            )
                issue(
                    step,
                    "campaign_accept_alliance_on_enter",
                    "Wait for this faction's confirmed alliance.",
                );
        }
        if (own(step, "campaign_halt_on_enter")) {
            const action = step.campaign_halt_on_enter;
            knownFields(action, ["faction_id"], step, "campaign_halt_on_enter");
            if (!object(action) || !id(action.faction_id))
                issue(step, "campaign_halt_on_enter", "Choose the campaign faction to halt.");
            if (!allowMissingFactionReferences && roster && !rosterFactions.has(action && action.faction_id))
                issue(step, "campaign_halt_on_enter.faction_id", "Choose an existing campaign faction.");
        }
        if (own(step, "camera")) {
            const camera = step.camera;
            knownFields(camera, ["shot", "from", "to", "seconds", "machinima"], step, "camera");
            if (!object(camera) || !CAMERA_SHOTS.includes(camera.shot))
                issue(step, "camera.shot", "Choose a reveal, threat or link shot.");
            if (object(camera) && own(camera, "machinima") && typeof camera.machinima !== "boolean")
                issue(step, "camera.machinima", "Machinima must be true or false.");
            ["from", "to"].forEach((field) => {
                if (
                    object(camera) &&
                    (field === "to" || own(camera, field)) &&
                    (typeof camera[field] !== "string" ||
                        (roster && !factions.has(camera[field]) && !allowMissingFactionReferences))
                )
                    issue(
                        step,
                        "camera." + field,
                        "Choose the player or an existing campaign faction to film.",
                    );
            });
            if (
                object(camera) &&
                own(camera, "seconds") &&
                (!Number.isFinite(camera.seconds) || camera.seconds < 1 || camera.seconds > 5)
            )
                issue(step, "camera.seconds", "Shot length must be 1–5 seconds.");
            // Camera lessons are the player's to perform: a scripted move would do the gesture for them.
            if (step.camera_only === true)
                issue(step, "camera", "Camera lessons cannot run a scripted shot.");
        }
        if (own(step, "pause_game") && typeof step.pause_game !== "boolean")
            issue(step, "pause_game", "Pause game must be true or false.");
        if (
            own(step, "camera_only") &&
            (typeof step.camera_only !== "boolean" || (step.camera_only && step.pause_game !== true))
        )
            issue(step, "camera_only", "Camera-only input requires a paused objective.");
        if (own(step, "paused_action")) {
            const triggerType = step.trigger && step.trigger.type;
            let compatible = false;
            if (step.paused_action === "expand_once_then_resume") {
                compatible =
                    ["territory", "contact"].includes(triggerType) &&
                    step.guide &&
                    step.guide.kind === "world" &&
                    step.guide.target === "expand";
            } else if (step.paused_action === "send_resources_stay_paused") {
                compatible = triggerType === "resource_transfer";
            } else if (step.paused_action === "build_until_started") {
                compatible = ["city", "factory", "tower"].includes(triggerType);
            } else if (step.paused_action === "upgrade_until_started") {
                compatible = triggerType === "structure_upgrade";
            }
            if (step.paused_action === "request_alliance_stay_paused")
                compatible = triggerType === "alliance_request";
            if (
                !compatible ||
                step.pause_game !== true ||
                step.camera_only === true ||
                !["objective", "guide"].includes(step.type)
            ) {
                issue(
                    step,
                    "paused_action",
                    "Paused actions require a paused mechanics step and a compatible objective.",
                );
            }
        }
        ["title_key", "body_key", "hint_key"].forEach((field) => text(step, field, step[field], false));
        const textNamespace =
            typeof definition.text_namespace === "string" ? definition.text_namespace : "tutorial.";
        const dynamicText = (suffix: string) => {
            const key = textNamespace + step.id + suffix;
            if (options.hasText && !options.hasText(key)) return;
            text(step, "guide.text." + suffix, key, true);
        };
        if (step.guide && ["zoom_in", "zoom_out"].includes(step.guide.gesture)) {
            ["pinch", "trackpad", "wheel"].forEach((mode) =>
                ["_hint", "_label"].forEach((suffix) => dynamicText("_" + mode + suffix)),
            );
        }
        if (step.guide && ["drag", "hover"].includes(step.guide.gesture)) {
            ["_mobile_hint", "_desktop_hint"].forEach(dynamicText);
        }
        if (step.trigger && step.trigger.type === "camera_target") dynamicText("_progress");
        if (step.guide && step.guide.gesture === "tap" && step.guide.show_label !== false) {
            ["_mobile_action", "_desktop_action"].forEach(dynamicText);
            if (step.paused_action === "expand_once_then_resume") dynamicText("_continue_action");
        }
        if (step.speaker && !own(speakers, step.speaker)) issue(step, "speaker", "Unknown character.");
        if (
            step.presentation &&
            (!["dialogue", "chapter", "celebration", "cinematic"].includes(step.presentation) ||
                (step.presentation === "cinematic" && step.type !== "scene"))
        )
            issue(step, "presentation", "Choose a presentation supported by this step.");
        if (
            own(step, "portrait_expression") &&
            !["happy", "calm", "fear", "angry"].includes(step.portrait_expression)
        )
            issue(step, "portrait_expression", "Choose a portrait expression: happy, calm, fear or angry.");
        text(step, "continue_key", step.continue_key, false);
        checkEmote(step, step, "");
        if (own(step, "exchange")) {
            if (!["scene", "objective", "guide"].includes(step.type))
                issue(step, "exchange", "Only scenes, objectives and guides can play an exchange.");
            if (
                step.type === "scene" &&
                (step.title_key ||
                    step.body_key ||
                    step.lines ||
                    step.presentation ||
                    own(step, "show_title") ||
                    own(step, "wait_before"))
            )
                issue(step, "exchange", "A scene that plays an exchange has no dialogue of its own.");
            checkExchange(step, step.exchange, "exchange");
            if (step.emote)
                issue(step, "exchange", "Use one emote owner: an exchange or a completion emote.");
        }
        notEmoteLine(step, "title_key", step.title_key);
        notEmoteLine(step, "body_key", step.body_key);
        if (own(step, "show_title") && (typeof step.show_title !== "boolean" || !step.title_key))
            issue(step, "show_title", "Set show_title to true only beside a title_key.");
        if (
            own(step, "emote_pinned") &&
            (typeof step.emote_pinned !== "boolean" || !own(step, "emote") || step.emote === "clear")
        )
            issue(step, "emote_pinned", "Pin only a named emote, with true or false.");
        if (step.video_src != null) {
            const prefix = "/assets/campaign/" + definition.episode_id + "/";
            const relative =
                typeof step.video_src === "string" && step.video_src.startsWith(prefix)
                    ? step.video_src.slice(prefix.length)
                    : "";
            const validPath =
                step.type === "scene" &&
                step.presentation === "cinematic" &&
                relative &&
                /\.(?:mp4|webm)$/i.test(relative) &&
                relative
                    .split("/")
                    .every(
                        (part) =>
                            part &&
                            part !== "." &&
                            part !== ".." &&
                            /^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(part),
                    );
            if (!validPath)
                issue(
                    step,
                    "video_src",
                    "Use an MP4 or WebM inside this episode's assets/campaign folder and select Cinematic presentation.",
                );
        }
        if (step.lines != null) {
            if (
                step.type !== "scene" ||
                !Array.isArray(step.lines) ||
                !step.lines.length ||
                step.lines.length > 64
            )
                issue(step, "lines", "A conversation needs 1–64 lines.");
            else
                step.lines.forEach((line) => {
                    if (!object(line)) return issue(step, "lines", "Invalid conversation line.");
                    knownFields(line, ["speaker", "title_key", "body_key"], step, "lines");
                    text(step, "lines", line.body_key, true);
                    text(step, "lines", line.title_key, false);
                    notEmoteLine(step, "lines", line.body_key);
                    notEmoteLine(step, "lines", line.title_key);
                    if (line.speaker && !own(speakers, line.speaker))
                        issue(step, "lines", "Unknown speaking character.");
                });
        }
        const narratedEnding = step.type === "end" && step.presentation === "chapter" && !step.speaker;
        if (
            requireSpeakerPortraits &&
            !narratedEnding &&
            !step.exchange &&
            (step.type === "scene" ||
                step.type === "choice" ||
                (step.type === "end" && (step.body_key || step.lines)))
        ) {
            if (step.type === "scene" && Array.isArray(step.lines) && step.lines.length) {
                step.lines.forEach((line) =>
                    checkDialogueSpeaker(step, line && (line.speaker || step.speaker)),
                );
            } else checkDialogueSpeaker(step, step.speaker);
        }
        if (!step.title_key && !step.body_key && !step.lines && !step.exchange)
            issue(step, "title_key", "Add a title, text, conversation or exchange.");
        if (step.type === "end") {
            if (step.next || step.routes || step.choices)
                issue(step, "next", "An ending has no outgoing connections.");
        } else if (step.type === "choice") {
            if (!Array.isArray(step.choices) || step.choices.length < 2 || step.choices.length > 4)
                issue(step, "choices", "A decision needs 2–4 answers.");
            const seen = new Set();
            (Array.isArray(step.choices) ? step.choices : []).forEach((option) => {
                if (!object(option)) return issue(step, "choices", "Invalid answer.");
                knownFields(option, ["id", "label_key", "body_key", "next"], step, "choices");
                if (!id(option.id) || seen.has(option.id))
                    issue(step, "choices", "Answer IDs must be unique.");
                seen.add(option.id);
                text(step, "choices", option.label_key, true);
                text(step, "choices", option.body_key, false);
                destination(step, "choices", option.next);
            });
            const choiceTargets = (Array.isArray(step.choices) ? step.choices : [])
                .filter((option) => object(option) && byId.has(option.next))
                .map((option) => option.next);
            const choiceUsedLater = steps.some(
                (item) =>
                    object(item) &&
                    item !== step &&
                    Array.isArray(item.routes) &&
                    item.routes.some(
                        (route) => object(route) && object(route.when) && route.when.choice === step.id,
                    ),
            );
            if (
                step.choices &&
                step.choices.length > 1 &&
                choiceTargets.length === step.choices.length &&
                new Set(choiceTargets).size === 1 &&
                !choiceUsedLater
            ) {
                warnings.push({
                    step: step.id,
                    field: "choices",
                    message: "Both answers lead to the same step and no later scene uses this decision.",
                });
            }
            if (step.next || step.routes) issue(step, "next", "Connect each answer to its own next step.");
        } else destination(step, "next", step.next);
        if (step.unavailable_next) destination(step, "unavailable_next", step.unavailable_next);
        if (step.routes != null) {
            if (!Array.isArray(step.routes)) issue(step, "routes", "Routes must be a list.");
            else
                step.routes.forEach((route, index) => {
                    if (!object(route)) return issue(step, "routes", "Invalid conditional route.");
                    knownFields(route, ["when", "next"], step, "routes");
                    knownFields(route.when, ["choice", "equals", "fact", "gte"], step, "routes.when");
                    destination(step, "routes", route.next);
                    condition(step, route.when);
                    const when = route.when;
                    if (!object(when)) return;
                    const shadowed = step.routes.slice(0, index).findIndex((previous: CampaignRecord) => {
                        const prior = previous && previous.when;
                        if (!object(prior)) return false;
                        if (typeof when.choice === "string")
                            return prior.choice === when.choice && prior.equals === when.equals;
                        return (
                            typeof when.fact === "string" &&
                            prior.fact === when.fact &&
                            Number.isFinite(prior.gte) &&
                            Number.isFinite(when.gte) &&
                            prior.gte <= when.gte
                        );
                    });
                    if (shadowed >= 0)
                        warnings.push({
                            step: step.id,
                            field: "routes",
                            message:
                                "Route " +
                                (index + 1) +
                                " is unreachable because earlier route " +
                                (shadowed + 1) +
                                " matches first.",
                        });
                });
        }
        if (["objective", "guide"].includes(step.type)) {
            const trigger = step.trigger;
            if (
                !object(trigger) ||
                !Object.keys(METRICS)
                    .concat(["contact", "defeated", "eliminated", "ui", "building_selected"])
                    .includes(trigger.type)
            )
                issue(step, "trigger", "Choose a supported objective.");
            else {
                knownFields(
                    trigger,
                    [
                        "type",
                        "scope",
                        "value",
                        "comparison",
                        "target",
                        "targets",
                        "action",
                        "outcome",
                        "unit",
                        "recipient",
                        "resources",
                        "kind",
                        "distance",
                        "faction",
                    ],
                    step,
                    "trigger",
                );
                if (own(trigger, "faction") && trigger.type !== "structure_level")
                    issue(step, "trigger.faction", "Only building triggers can watch a campaign faction.");
                if (!["step", "episode", "total"].includes(trigger.scope))
                    issue(step, "trigger.scope", "Choose when the objective starts counting.");
                if (
                    own(trigger, "comparison") &&
                    (trigger.type !== "territory" ||
                        trigger.scope !== "total" ||
                        trigger.comparison !== "lte")
                )
                    issue(
                        step,
                        "trigger.comparison",
                        "Only total territory objectives can use an at-most comparison.",
                    );
                if (trigger.targets != null && !["contact", "defeated"].includes(trigger.type))
                    issue(
                        step,
                        "trigger.targets",
                        "Only contact and defeat objectives can list multiple factions.",
                    );
                if (trigger.type === "contact") {
                    if (trigger.targets != null) {
                        if (
                            !Array.isArray(trigger.targets) ||
                            !trigger.targets.length ||
                            new Set(trigger.targets).size !== trigger.targets.length ||
                            trigger.targets.some(
                                (target) =>
                                    typeof target !== "string" ||
                                    (roster && !rosterFactions.has(target) && !allowMissingFactionReferences),
                            )
                        )
                            issue(step, "trigger.targets", "Choose one or more distinct existing factions.");
                        if (
                            trigger.value != null &&
                            (!Number.isInteger(trigger.value) ||
                                trigger.value < 1 ||
                                trigger.value > trigger.targets.length)
                        )
                            issue(
                                step,
                                "trigger.value",
                                "Choose how many of the selected factions must be contacted.",
                            );
                    } else if (
                        !trigger.target ||
                        trigger.target === "player" ||
                        (roster && !factions.has(trigger.target) && !allowMissingFactionReferences)
                    )
                        issue(step, "trigger.target", "Choose an existing faction.");
                } else if (trigger.type === "defeated") {
                    if (trigger.targets != null) {
                        if (
                            !Array.isArray(trigger.targets) ||
                            !trigger.targets.length ||
                            new Set(trigger.targets).size !== trigger.targets.length ||
                            trigger.targets.some(
                                (target) =>
                                    typeof target !== "string" ||
                                    (!rosterFactions.has(target) && !allowMissingFactionReferences),
                            )
                        )
                            issue(step, "trigger.targets", "Choose one or more distinct existing factions.");
                        if (!Number.isFinite(trigger.value) || trigger.value !== trigger.targets.length)
                            issue(
                                step,
                                "trigger.value",
                                "The required amount must match the selected factions.",
                            );
                    } else if (
                        !trigger.target ||
                        trigger.target === "player" ||
                        (roster && !factions.has(trigger.target) && !allowMissingFactionReferences)
                    )
                        issue(step, "trigger.target", "Choose an existing faction.");
                } else if (trigger.type === "eliminated") {
                    if (trigger.targets != null)
                        issue(step, "trigger.targets", "Choose one faction to track for elimination.");
                    if (
                        !trigger.target ||
                        (trigger.target !== "player" &&
                            roster &&
                            !rosterFactions.has(trigger.target) &&
                            !allowMissingFactionReferences)
                    )
                        issue(step, "trigger.target", "Choose the player or an existing faction.");
                } else if (trigger.type === "ui") {
                    if (!own(UI_TARGETS, trigger.action))
                        issue(step, "trigger.action", "Choose an existing control.");
                    if (
                        trigger.outcome != null &&
                        (!own(MENU_GUIDE_OUTCOMES, trigger.outcome) ||
                            MENU_GUIDE_OUTCOMES[trigger.outcome].control !== trigger.action)
                    )
                        issue(
                            step,
                            "trigger.outcome",
                            "Choose a confirmed outcome registered for this control.",
                        );
                    if (trigger.action === "map_menu_open" && !(step.marker && step.marker.target))
                        issue(step, "marker", "Choose whose map menu must open.");
                } else if (trigger.type === "building_selected") {
                    if (!["City", "Farm", "Factory", "Tower", "Port"].includes(trigger.kind))
                        issue(step, "trigger.kind", "Choose a supported building type.");
                } else if (trigger.type === "structure_upgrade") {
                    if (!own(STRUCTURE_LEVEL_LIMITS, trigger.kind))
                        issue(step, "trigger.kind", "Choose a supported building type for this upgrade.");
                    if (!Number.isInteger(trigger.value) || trigger.value < 1)
                        issue(step, "trigger.value", "Choose how many upgrades must complete.");
                } else if (trigger.type === "camera_target") {
                    if (!trigger.target)
                        issue(step, "trigger.target", "Choose a faction or the player as the camera target.");
                    if (!Number.isFinite(trigger.distance) || trigger.distance <= 0)
                        issue(step, "trigger.distance", "Set the maximum distance from the camera target.");
                } else if (trigger.type === "zoom_out_complete") {
                    if (!Number.isFinite(trigger.value) || trigger.value <= 0 || trigger.value > 1)
                        issue(step, "trigger.value", "Set the zoom-out target between 0 and 1.");
                } else if (trigger.type === "zoom_in_complete" && trigger.target) {
                    if (!Number.isFinite(trigger.distance) || trigger.distance <= 0)
                        issue(
                            step,
                            "trigger.distance",
                            "Set how close the camera must be to the zoom target.",
                        );
                } else if (trigger.type === "structure_level") {
                    if (!["City", "Farm", "Factory", "Tower", "Port"].includes(trigger.kind))
                        issue(step, "trigger.kind", "Choose a supported building type.");
                    else if (
                        !Number.isInteger(trigger.value) ||
                        trigger.value < 1 ||
                        trigger.value > STRUCTURE_LEVEL_LIMITS[trigger.kind]
                    )
                        issue(step, "trigger.value", "Choose a level that this building can reach.");
                    if (
                        own(trigger, "faction") &&
                        (typeof trigger.faction !== "string" ||
                            (roster &&
                                !rosterFactions.has(trigger.faction) &&
                                !allowMissingFactionReferences))
                    )
                        issue(step, "trigger.faction", "Choose an existing campaign faction.");
                } else {
                    if (!Number.isFinite(trigger.value) || trigger.value <= 0)
                        issue(step, "trigger.value", "Objective value must be greater than zero.");
                    if (
                        trigger.type === "fleet" &&
                        trigger.unit != null &&
                        !["TransportShip", "TradeShip", "Warship"].includes(trigger.unit)
                    )
                        issue(step, "trigger.unit", "Choose a ship type supported by the game.");
                    if (
                        trigger.type === "fleet" &&
                        trigger.target &&
                        trigger.unit &&
                        trigger.unit !== "TransportShip"
                    )
                        issue(step, "trigger.target", "Only a transport can be tied to a landing target.");
                    if (
                        trigger.type === "resource_transfer" &&
                        trigger.resources != null &&
                        (!Array.isArray(trigger.resources) ||
                            !trigger.resources.length ||
                            new Set(trigger.resources).size !== trigger.resources.length ||
                            trigger.resources.some((resource) => !["gold", "troops"].includes(resource)))
                    )
                        issue(step, "trigger.resources", "Choose gold, troops, or both.");
                }
                if (["contact", "defeated", "alliance", "resource_transfer"].includes(trigger.type)) {
                    const factionTarget = trigger.target || trigger.recipient;
                    if (trigger.recipient != null && trigger.type !== "resource_transfer")
                        issue(step, "trigger.recipient", "Only resource transfers have a recipient.");
                    if (
                        factionTarget &&
                        roster &&
                        !factions.has(factionTarget) &&
                        !allowMissingFactionReferences
                    )
                        issue(step, "trigger.target", "Unknown faction.");
                }
                if (trigger.unit != null && trigger.type !== "fleet")
                    issue(step, "trigger.unit", "Only fleet objectives can select a ship type.");
                if (trigger.resources != null && trigger.type !== "resource_transfer")
                    issue(
                        step,
                        "trigger.resources",
                        "Only transfer objectives can require specific resources.",
                    );
                if (
                    trigger.recipient != null &&
                    roster &&
                    !factions.has(trigger.recipient) &&
                    !allowMissingFactionReferences
                )
                    issue(step, "trigger.recipient", "Unknown transfer recipient.");
                if (
                    trigger.target &&
                    roster &&
                    !factions.has(trigger.target) &&
                    !allowMissingFactionReferences
                )
                    issue(step, "trigger.target", "Unknown faction.");
                const targetedFactions = Array.isArray(trigger.targets) ? trigger.targets : [trigger.target];
                const targetableFactions = targetedFactions.filter(
                    (target) => typeof target === "string" && target !== "player",
                );
                if (
                    [
                        "attack",
                        "contact",
                        "alliance",
                        "defeated",
                        "fleet",
                        "camera_target",
                        "hover",
                        "zoom_in_complete",
                    ].includes(trigger.type) &&
                    targetableFactions.length
                ) {
                    if (!step.marker || !targetableFactions.includes(step.marker.target))
                        issue(step, "marker", "Faction objectives need a marker on one of their targets.");
                    const expandsToContact =
                        trigger.type === "contact" &&
                        step.guide &&
                        step.guide.kind === "world" &&
                        step.guide.target === "expand";
                    if (
                        ["attack", "contact", "alliance", "defeated", "fleet"].includes(trigger.type) &&
                        !expandsToContact &&
                        !step.campaign_accept_alliance_on_enter &&
                        (!step.guide ||
                            step.guide.kind !== "world" ||
                            !["target_action", "nameplate", "player"].includes(step.guide.target))
                    ) {
                        issue(
                            step,
                            "guide",
                            "Faction objectives need a hand guide that points to their target.",
                        );
                    }
                }
            }
            if (trigger && trigger.type === "elapsed" && step.guide)
                issue(step, "guide", "Timed waits do not need a hand guide.");
        } else if (step.trigger)
            issue(step, "trigger", "Only objectives and guides have completion conditions.");
        knownFields(step.marker, ["target"], step, "marker");
        if (
            step.marker &&
            (!object(step.marker) ||
                !step.marker.target ||
                (roster && !factions.has(step.marker.target) && !allowMissingFactionReferences))
        )
            issue(step, "marker", "Unknown marked faction.");
        if (step.guide) {
            const guide = step.guide;
            knownFields(guide, ["kind", "target", "gesture", "to", "show_label"], step, "guide");
            if (own(guide, "show_label") && typeof guide.show_label !== "boolean")
                issue(step, "guide.show_label", "Choose whether the hand shows text.");
            if (!["objective", "guide"].includes(step.type))
                issue(step, "guide", "Only mechanics use the hand; decisions never do.");
            if (
                !object(guide) ||
                !["world", "ui"].includes(guide.kind) ||
                !["tap", "hold", "drag", "hover", "pan_keys", "zoom_in", "zoom_out"].includes(guide.gesture)
            )
                issue(step, "guide", "Choose a world/control target and a gesture.");
            else if (
                guide.kind === "world"
                    ? !WORLD_TARGETS.includes(guide.target)
                    : !own(UI_TARGETS, guide.target)
            )
                issue(step, "guide.target", "Unknown guide target.");
            if (
                guide.gesture === "drag" &&
                guide.to != null &&
                (guide.kind === "world" ? !WORLD_TARGETS.includes(guide.to) : !own(UI_TARGETS, guide.to))
            )
                issue(step, "guide.to", "Choose a valid drag destination.");
            if (
                ["zoom_in", "zoom_out"].includes(guide.gesture) &&
                (guide.kind !== "world" ||
                    guide.target !== "player" ||
                    !step.trigger ||
                    ![guide.gesture, guide.gesture + "_complete"].includes(step.trigger.type))
            )
                issue(step, "guide", "Zoom guides must match a zoom objective and point at the player.");
        } else if (step.type === "guide") issue(step, "guide", "A guide step needs a hand target.");
    });
}
