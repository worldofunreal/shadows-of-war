import * as rules from "./rules.ts";
import type {
    CampaignDefinition,
    CampaignRoster,
    CampaignValidationReport,
    CampaignRecord,
    CampaignStep,
} from "./types.ts";
const {
    TYPES,
    TEAMS,
    CAMERA_SHOTS,
    EMOTE_KINDS,
    REACTION_DIALOGUE_FIELDS,
    EXCHANGE_MAX_DELAY_MS,
    EMOTE_TEXT_MAX_CHARS,
    METRICS,
    WORLD_TARGETS,
    STRUCTURE_LEVEL_LIMITS,
    CAMPAIGN_ACTIONS,
    applyCampaignUnlocks,
    campaignAllowsAction,
    campaignEdges,
    campaignUnlockStates,
    stepCampaignRequirements,
    UI_TARGETS,
    MENU_GUIDE_OUTCOMES,
    own,
    object,
    id,
} = rules;

export function validate(
    definition: CampaignDefinition,
    roster: CampaignRoster,
    options: CampaignRecord = {},
): CampaignValidationReport {
    options = options || {};
    const allowMissingFactionReferences = options.allowMissingFactionReferences === true;
    const errors = [],
        warnings = [];
    const issue = (step, field, message) => errors.push({ step: step && step.id, field, message });
    const knownFields = (value, allowed, step, field) => {
        if (object(value))
            Object.keys(value)
                .filter((key) => !allowed.includes(key))
                .forEach((key) => issue(step, field, "Unsupported field (no runtime behavior): " + key));
    };
    if (!object(definition) || definition.version !== 2) {
        issue(null, "version", "Campaign logic must use version 2.");
        return { errors, warnings };
    }
    knownFields(
        definition,
        [
            "version",
            "episode_id",
            "default_locale",
            "text_namespace",
            "faction_story_names",
            "require_speaker_portraits",
            "settings",
            "entry",
            "menu_guide",
            "speakers",
            "strings",
            "layout",
            "steps",
            "reactions",
        ],
        null,
        "campaign",
    );
    if (!id(definition.episode_id)) issue(null, "episode_id", "Invalid episode ID.");
    if (
        own(definition, "require_speaker_portraits") &&
        typeof definition.require_speaker_portraits !== "boolean"
    )
        issue(
            null,
            "require_speaker_portraits",
            "Choose whether dialogue needs a named speaker and portrait.",
        );
    const requireSpeakerPortraits = definition.require_speaker_portraits === true;
    const settings = definition.settings;
    if (
        !object(settings) ||
        !Number.isInteger(settings.starting_troops) ||
        settings.starting_troops < 1 ||
        settings.starting_troops > 100000
    ) {
        issue(null, "settings", "Choose 1–100000 starting troops.");
    }
    if (object(settings)) {
        if (
            Object.keys(settings).some(
                (key) => !["starting_troops", "starting_gold", "campaign_support"].includes(key),
            )
        )
            issue(null, "settings", "Unknown match setting.");
        if (
            settings.starting_gold != null &&
            (!Number.isInteger(settings.starting_gold) ||
                settings.starting_gold < 0 ||
                settings.starting_gold > 1000000)
        )
            issue(null, "settings.starting_gold", "Choose 0–1000000 starting gold.");
        if (settings.campaign_support != null) {
            const support = settings.campaign_support;
            if (
                !object(support) ||
                Object.keys(support).some((key) => !["after_defeated", "share_percent"].includes(key)) ||
                typeof support.after_defeated !== "string" ||
                !Number.isInteger(support.share_percent) ||
                support.share_percent < 1 ||
                support.share_percent > 100
            )
                issue(null, "settings.campaign_support", "Choose a valid milestone and 1–100 percent share.");
        }
    }
    const factions = new Set(["player"]),
        factionNames = new Set(),
        rosterFactions = new Set();
    if (roster) {
        knownFields(roster, ["_comment", "map", "player_spawn", "player_color", "factions"], null, "roster");
        const expectedMap =
            definition.episode_id === "boudica"
                ? ["eastanglia", 896, 504]
                : /^six_sky_ep[123]$/.test(definition.episode_id)
                  ? ["northamerica", 1000, 516]
                  : null;
        if (
            !object(roster) ||
            typeof roster.map !== "string" ||
            !/^[a-z0-9_-]+$/.test(roster.map) ||
            !Array.isArray(roster.player_spawn) ||
            roster.player_spawn.length !== 2 ||
            !roster.player_spawn.every((n) => Number.isInteger(n) && n >= 0) ||
            !Array.isArray(roster.factions)
        ) {
            issue(null, "roster", "Invalid map, player spawn or factions.");
        }
        if (expectedMap && roster.map !== expectedMap[0])
            issue(null, "roster.map", "This episode uses the " + expectedMap[0] + " map.");
        if (
            roster.player_color != null &&
            (typeof roster.player_color !== "string" || !/^#[0-9a-fA-F]{6}$/.test(roster.player_color))
        )
            issue(null, "roster.player_color", "Use a hex color such as #f0902a.");
        if (
            expectedMap &&
            Array.isArray(roster.player_spawn) &&
            roster.player_spawn.length === 2 &&
            roster.player_spawn.every(Number.isInteger) &&
            (roster.player_spawn[0] >= expectedMap[1] || roster.player_spawn[1] >= expectedMap[2])
        )
            issue(null, "roster.player_spawn", "Player spawn is outside the campaign map.");
        (Array.isArray(roster.factions) ? roster.factions : []).forEach((faction) => {
            if (
                !object(faction) ||
                !id(faction.id) ||
                typeof faction.name !== "string" ||
                !faction.name.trim() ||
                factions.has(faction.id) ||
                factionNames.has(faction.name)
            ) {
                issue(null, "roster", "Faction IDs and names must be valid, unique and nonempty.");
                return;
            }
            knownFields(
                faction,
                [
                    "id",
                    "name",
                    "x",
                    "y",
                    "starting_troops",
                    "relation",
                    "team",
                    "can_request_alliance",
                    "can_attack_player",
                    "color",
                    "iq",
                    "civ",
                    "leader",
                    "avatar",
                    "support_interval_seconds",
                    "gold_loot_bonus",
                    "gold_loot_override",
                    "alliance_group",
                    "assault_force",
                    "hold_structures_until_contact",
                    "city_requires_gold",
                    "accepts_alliance_requests",
                ],
                null,
                "roster.factions",
            );
            factions.add(faction.id);
            factionNames.add(faction.name);
            rosterFactions.add(faction.id);
            if (
                !Number.isInteger(faction.starting_troops) ||
                faction.starting_troops < 0 ||
                faction.starting_troops > 1000000 ||
                typeof faction.civ !== "string" ||
                !faction.civ ||
                typeof faction.leader !== "string" ||
                !faction.leader ||
                ![faction.x, faction.y].every((n) => Number.isInteger(n) && n >= 0)
            )
                issue(
                    null,
                    "roster",
                    "Invalid starting troops, civilization, leader or spawn: " + faction.name,
                );
            else if (expectedMap && (faction.x >= expectedMap[1] || faction.y >= expectedMap[2]))
                issue(null, "roster", "Faction spawn is outside the campaign map: " + faction.name);
            if (!["neutral", "allied", "enemy"].includes(faction.relation))
                issue(
                    null,
                    "roster.factions.relation",
                    "Choose neutral, allied or enemy for " + faction.name + ".",
                );
            if (faction.team != null && !TEAMS.includes(faction.team))
                issue(null, "roster.factions.team", "Choose Red, Blue or no team for " + faction.name + ".");
            if (faction.can_request_alliance != null && typeof faction.can_request_alliance !== "boolean")
                issue(
                    null,
                    "roster.factions.can_request_alliance",
                    "Choose whether this faction can send alliance offers.",
                );
            if (faction.can_attack_player != null && typeof faction.can_attack_player !== "boolean")
                issue(
                    null,
                    "roster.factions.can_attack_player",
                    "Choose whether this enemy can initiate attacks against the player.",
                );
            if (
                String(faction.civ)
                    .replace(/[^a-z0-9]/gi, "")
                    .toLowerCase() === "romanempire" &&
                faction.relation === "neutral"
            )
                issue(
                    null,
                    "roster.factions.relation",
                    "Roman Empire factions must be enemies or allies, never neutral.",
                );
            if (faction.assault_force != null && typeof faction.assault_force !== "boolean")
                issue(
                    null,
                    "roster.factions.assault_force",
                    "Choose whether this faction can join a configured campaign assault.",
                );
            if (
                faction.hold_structures_until_contact != null &&
                typeof faction.hold_structures_until_contact !== "boolean"
            )
                issue(
                    null,
                    "roster.factions.hold_structures_until_contact",
                    "Choose whether this faction waits for your decision before building.",
                );
            if (faction.city_requires_gold != null && typeof faction.city_requires_gold !== "boolean")
                issue(
                    null,
                    "roster.factions.city_requires_gold",
                    "Choose whether this faction pays for its first city.",
                );
            if (
                faction.accepts_alliance_requests != null &&
                typeof faction.accepts_alliance_requests !== "boolean"
            )
                issue(
                    null,
                    "roster.factions.accepts_alliance_requests",
                    "Choose whether this faction always accepts alliance requests.",
                );
            if (
                faction.color != null &&
                (typeof faction.color !== "string" || !/^#[0-9a-fA-F]{6}$/.test(faction.color))
            )
                issue(null, "roster.factions.color", "Use a six-digit hex color for " + faction.name + ".");
            if (
                faction.support_interval_seconds != null &&
                (!Number.isInteger(faction.support_interval_seconds) ||
                    faction.support_interval_seconds < 5 ||
                    faction.support_interval_seconds > 600)
            )
                issue(
                    null,
                    "roster.factions.support_interval_seconds",
                    "Support intervals must be 5–600 seconds.",
                );
            if (
                faction.gold_loot_bonus != null &&
                (!Number.isInteger(faction.gold_loot_bonus) ||
                    faction.gold_loot_bonus < 0 ||
                    faction.gold_loot_bonus > 1_000_000)
            )
                issue(
                    null,
                    "roster.factions.gold_loot_bonus",
                    "Bonus gold loot must be an integer from 0 to 1,000,000.",
                );
            if (
                faction.gold_loot_override != null &&
                (!Number.isInteger(faction.gold_loot_override) ||
                    faction.gold_loot_override < 0 ||
                    faction.gold_loot_override > 1_000_000)
            )
                issue(
                    null,
                    "roster.factions.gold_loot_override",
                    "Fixed gold loot must be an integer from 0 to 1,000,000.",
                );
            if (
                faction.alliance_group != null &&
                (typeof faction.alliance_group !== "string" ||
                    !/^[a-z][a-z0-9_]{0,63}$/.test(faction.alliance_group))
            )
                issue(null, "roster.factions.alliance_group", "Use a lowercase alliance group ID.");
            if (
                faction.avatar != null &&
                (!/^[a-z][a-z0-9_]*$/.test(faction.avatar) ||
                    (faction.avatar !== "null" && options.hasAvatar && !options.hasAvatar(faction.avatar)))
            )
                issue(
                    null,
                    "roster.factions.avatar",
                    "Choose an existing portrait for " + faction.name + ".",
                );
        });
        if (
            !allowMissingFactionReferences &&
            object(settings) &&
            object(settings.campaign_support) &&
            !rosterFactions.has(settings.campaign_support.after_defeated)
        )
            issue(
                null,
                "settings.campaign_support.after_defeated",
                "Support milestone refers to an unknown faction.",
            );
    }
    const strings = definition.strings;
    if (strings != null) {
        if (!object(strings))
            issue(
                null,
                "strings",
                "Campaign files store text keys only; use the shared language catalogs for text.",
            );
        else
            Object.entries(strings).forEach(([locale, catalog]) => {
                if (!/^[a-z]{2,3}(?:-[a-z]{2})?$/.test(locale) || !object(catalog))
                    issue(null, "strings", "Invalid language dictionary: " + locale);
                else if (Object.keys(catalog).length)
                    issue(
                        null,
                        "strings",
                        "Campaign text belongs in the shared language catalogs, not the episode file.",
                    );
            });
    }
    if (definition.faction_story_names != null) {
        if (!object(definition.faction_story_names))
            issue(null, "faction_story_names", "Invalid faction story-name map.");
        else
            Object.entries(definition.faction_story_names).forEach(([factionId, name]) => {
                if (
                    !id(factionId) ||
                    typeof name !== "string" ||
                    !name.trim() ||
                    !rosterFactions.has(factionId)
                )
                    issue(
                        null,
                        "faction_story_names",
                        "Faction story names must refer to a roster faction and its original name.",
                    );
            });
    }
    if (
        definition.text_namespace != null &&
        (typeof definition.text_namespace !== "string" ||
            !/^tutorial\.[a-zA-Z0-9_-]+_$/.test(definition.text_namespace))
    )
        issue(null, "text_namespace", "Use a tutorial text-key namespace ending in an underscore.");
    function text(step, field, key, required) {
        if (key == null && !required) return;
        if (typeof key !== "string" || !/^tutorial\.[a-zA-Z0-9_.-]+$/.test(key))
            issue(step, field, "Choose a tutorial text key.");
        else if (options.hasText && !options.hasText(key))
            issue(step, field, "Missing shared English text: " + key);
    }
    function checkEmoteText(step, field, key) {
        if (typeof options.getText !== "function") return;
        let value;
        try {
            value = options.getText(key);
        } catch {
            value = null;
        }
        if (typeof value !== "string") {
            issue(step, field, "Could not resolve the emote text for display.");
            return;
        }
        const normalized = value.trim();
        let length = 0;
        for (const character of normalized) {
            const code = character.codePointAt(0);
            if (code <= 0x1f || (code >= 0x7f && code <= 0x9f)) {
                issue(step, field, "Emote text cannot contain control characters.");
                return;
            }
            length++;
        }
        if (!length) issue(step, field, "Emote text cannot be blank.");
        else if (length > EMOTE_TEXT_MAX_CHARS)
            issue(
                step,
                field,
                "Emote text is limited to 160 characters; use the dialogue modal for longer lines.",
            );
    }
    const speakers = definition.speakers || {};
    if (!object(speakers)) issue(null, "speakers", "Invalid character dictionary.");
    else
        Object.entries(speakers).forEach(([key, speaker]) => {
            knownFields(speaker, ["name", "name_key", "avatar", "faction"], null, "speakers." + key);
            if (
                !id(key) ||
                !object(speaker) ||
                !(
                    (typeof speaker.name === "string" && speaker.name.trim()) ||
                    (typeof speaker.name_key === "string" && speaker.name_key.trim()) ||
                    (typeof speaker.faction === "string" && speaker.faction.trim())
                )
            )
                issue(null, "speakers", "Invalid character: " + key);
            else {
                if (
                    speaker.avatar != null &&
                    (!/^[a-z][a-z0-9_]*$/.test(speaker.avatar) ||
                        (speaker.avatar !== "null" &&
                            options.hasAvatar &&
                            !options.hasAvatar(speaker.avatar)))
                )
                    issue(null, "speakers", "Choose an existing avatar: " + key);
                if (
                    !allowMissingFactionReferences &&
                    speaker.faction != null &&
                    !rosterFactions.has(speaker.faction)
                )
                    issue(null, "speakers." + key + ".faction", "Choose an existing roster faction.");
                text(null, "speakers", speaker.name_key, false);
            }
        });
    function availableAvatar(avatar) {
        return (
            typeof avatar === "string" &&
            avatar !== "null" &&
            /^[a-z][a-z0-9_]*$/.test(avatar) &&
            (!options.hasAvatar || options.hasAvatar(avatar))
        );
    }
    function speakerHasPortrait(speakerKey) {
        const character = speakers[speakerKey];
        if (!object(character)) return false;
        if (character.faction) {
            const faction = ((roster && roster.factions) || []).find((item) => item.id === character.faction);
            return Boolean(faction && availableAvatar(faction.avatar || character.avatar));
        }
        return availableAvatar(character.avatar);
    }
    function checkDialogueSpeaker(step, speakerKey) {
        if (!speakerKey || !own(speakers, speakerKey))
            issue(step, "speaker", "Spoken dialogue needs a known character.");
        else if (!speakerHasPortrait(speakerKey))
            issue(step, "speaker", "Spoken dialogue needs an existing character portrait.");
    }
    const steps = Array.isArray(definition.steps) ? definition.steps : [];
    if (!steps.length || steps.length > 512) issue(null, "steps", "Use between 1 and 512 steps.");
    const byId = new Map<string, CampaignStep>();
    steps.forEach((step) => {
        if (!object(step) || !id(step.id) || byId.has(step.id))
            issue(step, "id", "Step IDs must be valid and unique.");
        else byId.set(step.id, step);
    });
    if (definition.layout != null) {
        if (!object(definition.layout)) issue(null, "layout", "Step layout must be an object.");
        else
            Object.entries(definition.layout).forEach(([stepId, position]) => {
                if (!byId.has(stepId)) issue(null, "layout", "Layout refers to an unknown step: " + stepId);
                knownFields(position, ["x", "y"], null, "layout." + stepId);
                if (!object(position) || !Number.isFinite(position.x) || !Number.isFinite(position.y))
                    issue(null, "layout." + stepId, "Step positions need numeric x and y values.");
            });
    }
    if (!byId.has(definition.entry)) issue(null, "entry", "Select an existing opening step.");
    knownFields(definition.menu_guide, ["entry", "dismissible"], null, "menu_guide");
    if (
        definition.menu_guide != null &&
        (!object(definition.menu_guide) ||
            !id(definition.menu_guide.entry) ||
            !byId.has(definition.menu_guide.entry) ||
            (definition.menu_guide.dismissible != null &&
                typeof definition.menu_guide.dismissible !== "boolean"))
    ) {
        issue(
            null,
            "menu_guide",
            "The optional menu guide needs an existing opening step and a dismissible setting.",
        );
    }
    function destination(step, field, target) {
        if (!byId.has(target)) issue(step, field, "Unknown destination: " + String(target || "(empty)"));
    }
    function condition(step, when) {
        if (!object(when) || ["choice", "fact"].filter((key) => own(when, key)).length !== 1)
            return issue(step, "routes", "Choose one condition: player decision or game fact.");
        knownFields(when, own(when, "choice") ? ["choice", "equals"] : ["fact", "gte"], step, "routes.when");
        if (own(when, "choice")) {
            // A condition may name a player decision or a contact response (its id) and the answer picked there.
            const choice = byId.get(when.choice);
            const response = (definition.reactions || []).find(
                (reaction) => reaction && reaction.id === when.choice,
            );
            const answers =
                choice && choice.type === "choice" ? choice.choices : response && response.choices;
            if (!Array.isArray(answers) || !answers.some((option) => option && option.id === when.equals))
                issue(step, "routes", "Condition refers to an unknown decision or answer.");
        } else if (
            !Object.values(METRICS)
                .concat(["tiles", "contacts", "defeated", "touch_controls"])
                .includes(when.fact) ||
            !Number.isFinite(when.gte)
        )
            issue(step, "routes", "Invalid game fact condition.");
    }
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
    const responses = Array.isArray(definition.reactions) ? definition.reactions : [];
    const emoteLines = new Set(
        steps
            .concat(responses)
            .filter(object)
            .flatMap((value) => [value].concat(Array.isArray(value.exchange) ? value.exchange : []))
            .map((value) => value.emote_text_key)
            .filter(Boolean),
    );
    function notEmoteLine(step, field, key) {
        if (emoteLines.has(key))
            issue(step, field, "This line is an emote bubble; dialogue cannot reuse it.");
    }
    function checkEmote(step, value, prefix) {
        const named = own(value, "emote") && value.emote !== "clear";
        if (named && !own(speakers, value.speaker))
            issue(step, prefix + "speaker", "Name the character who emotes.");
        if (named && !EMOTE_KINDS.includes(value.emote))
            issue(step, prefix + "emote", "Choose a known emote or clear.");
        if (named) {
            text(step, prefix + "emote_text_key", value.emote_text_key, true);
            checkEmoteText(step, prefix + "emote_text_key", value.emote_text_key);
        } else if (own(value, "emote_text_key"))
            issue(step, prefix + "emote_text_key", "Only a named emote carries a bubble line.");
        const faction =
            named && value.speaker && object(speakers[value.speaker]) && speakers[value.speaker].faction;
        if (faction && roster && !rosterFactions.has(faction) && !allowMissingFactionReferences)
            issue(step, prefix + "emote", "The emoting speaker must play an existing faction.");
    }
    function checkExchange(step, lines, field) {
        if (!Array.isArray(lines) || !lines.length || lines.length > 16)
            return issue(step, field, "An exchange needs 1–16 lines.");
        lines.forEach((line) => {
            if (!object(line)) return issue(step, field, "Invalid exchange line.");
            knownFields(
                line,
                [
                    "speaker",
                    "presentation",
                    "emote",
                    "emote_text_key",
                    "title_key",
                    "show_title",
                    "body_key",
                    "delay_ms",
                ],
                step,
                field,
            );
            if (!own(speakers, line.speaker)) issue(step, field, "Each exchange line names its character.");
            const presentation = line.presentation || "emote";
            if (!["emote", "dialogue"].includes(presentation))
                issue(step, field, "Choose an emote bubble or the dialogue modal for each line.");
            if (presentation === "emote") {
                if (!EMOTE_KINDS.includes(line.emote))
                    issue(step, field, "Each emote line needs a known expression.");
                checkEmote(step, line, field + ".");
                if (own(line, "title_key") || own(line, "body_key") || own(line, "show_title"))
                    issue(
                        step,
                        field,
                        "Emote lines use one bubble text; modal fields belong to dialogue lines.",
                    );
            } else {
                if (own(line, "emote") || own(line, "emote_text_key"))
                    issue(
                        step,
                        field,
                        "Dialogue lines use body_key and the speaker portrait, not an emote bubble.",
                    );
                text(step, field + ".body_key", line.body_key, true);
                text(step, field + ".title_key", line.title_key, false);
                if (own(line, "show_title") && (typeof line.show_title !== "boolean" || !line.title_key))
                    issue(step, field, "A visible dialogue title needs title_key.");
                notEmoteLine(step, field + ".body_key", line.body_key);
                if (requireSpeakerPortraits) checkDialogueSpeaker(step, line.speaker);
            }
            if (
                own(line, "delay_ms") &&
                (!Number.isInteger(line.delay_ms) ||
                    line.delay_ms < 0 ||
                    line.delay_ms > EXCHANGE_MAX_DELAY_MS)
            )
                issue(step, field + ".delay_ms", "Set a whole-number pause from 0 to 10000 ms.");
        });
    }
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
        const dynamicText = (suffix) => {
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
                    const shadowed = step.routes.slice(0, index).findIndex((previous) => {
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
    const reactions = responses;
    if (definition.reactions != null && !Array.isArray(definition.reactions))
        issue(null, "reactions", "Responses must be a list.");
    const reactionIds = new Set(),
        reactionEvents = new Set();
    reactions.forEach((reaction) => {
        if (!object(reaction)) return issue(null, "reactions", "Invalid story response.");
        knownFields(
            reaction,
            [
                "id",
                "after",
                "during",
                "when",
                "presentation",
                "speaker",
                "outcome",
                "gold_cost",
                "choices",
                "emote",
                "emote_text_key",
                "exchange",
                ...REACTION_DIALOGUE_FIELDS,
            ],
            null,
            "reactions",
        );
        checkEmote(null, reaction, "reactions.");
        if (own(reaction, "exchange")) checkExchange(null, reaction.exchange, "reactions.exchange");
        if (reaction.presentation != null && !["dialogue", "exchange"].includes(reaction.presentation))
            issue(null, "reactions.presentation", "Choose a dialogue or automatic emote exchange.");
        if (own(reaction, "emote") && !reaction.speaker)
            issue(null, "reactions.speaker", "Name the character who emotes.");
        notEmoteLine(null, "reactions.title_key", reaction.title_key);
        notEmoteLine(null, "reactions.body_key", reaction.body_key);
        if (own(reaction, "show_title") && (typeof reaction.show_title !== "boolean" || !reaction.title_key))
            issue(null, "reactions.show_title", "Set show_title to true only beside a title_key.");
        if (!id(reaction.id) || byId.has(reaction.id) || reactionIds.has(reaction.id))
            issue(null, "reactions.id", "Response IDs must be unique and distinct from story steps.");
        reactionIds.add(reaction.id);
        knownFields(reaction.when, ["type", "target", "targets", "relation"], null, "reactions.when");
        const when = reaction.when,
            type = object(when) && when.type,
            target = object(when) && when.target;
        if (!["contact", "first_contact", "support", "pre_defeat"].includes(type))
            issue(null, "reactions.when", "Choose a supported contact, support, or pre-defeat event.");
        if (type === "contact") {
            const hasTarget = typeof target === "string";
            const hasRelation = typeof when.relation === "string";
            if (
                hasTarget === hasRelation ||
                (hasRelation && when.relation !== "neutral") ||
                (hasTarget && roster && !rosterFactions.has(target) && !allowMissingFactionReferences)
            )
                issue(null, "reactions.when", "Choose one faction or all initially neutral factions.");
        }
        if (
            type === "support" &&
            (typeof target !== "string" ||
                (roster && !rosterFactions.has(target) && !allowMissingFactionReferences))
        )
            issue(null, "reactions.when.target", "Choose an existing faction.");
        if (
            type === "pre_defeat" &&
            (typeof target !== "string" ||
                (roster && !rosterFactions.has(target) && !allowMissingFactionReferences))
        )
            issue(null, "reactions.when.target", "Choose an existing faction.");
        if (
            (type !== "contact" && when && when.relation != null) ||
            (type !== "first_contact" && when && when.targets != null)
        )
            issue(null, "reactions.when", "That response condition is not valid for this event.");
        if (type === "first_contact") {
            const targets = when && when.targets;
            if (
                target != null ||
                !Array.isArray(targets) ||
                !targets.length ||
                new Set(targets).size !== targets.length ||
                targets.some(
                    (factionId) =>
                        typeof factionId !== "string" ||
                        (roster && !rosterFactions.has(factionId) && !allowMissingFactionReferences),
                )
            )
                issue(
                    null,
                    "reactions.when",
                    "Choose distinct existing factions for the first-contact scene.",
                );
            if (reaction.after != null)
                issue(
                    null,
                    "reactions.after",
                    "First contact happens immediately and cannot wait for another step.",
                );
        }
        if (type === "contact" && reaction.after != null) {
            const gate = byId.get(reaction.after);
            if (!gate || !["objective", "guide"].includes(gate.type))
                issue(null, "reactions.after", "Choose the objective that unlocks this contact response.");
        }
        if (reaction.during != null) {
            const gate = byId.get(reaction.during);
            const gateTargets =
                gate &&
                gate.trigger &&
                (Array.isArray(gate.trigger.targets) ? gate.trigger.targets : [gate.trigger.target]);
            if (
                !["contact", "pre_defeat"].includes(type) ||
                !gate ||
                !["objective", "guide"].includes(gate.type) ||
                !gate.trigger ||
                gate.trigger.type !== "defeated" ||
                !gateTargets.includes(target)
            ) {
                issue(null, "reactions.during", "Choose the active defeat objective for this faction.");
            }
            if (reaction.after != null)
                issue(
                    null,
                    "reactions.during",
                    "Choose either an active objective or a completed objective, not both.",
                );
        }
        if (type === "pre_defeat" && reaction.during == null)
            issue(
                null,
                "reactions.during",
                "A pre-defeat exchange must belong to its active defeat objective.",
            );
        if (type === "pre_defeat" && reaction.after != null)
            issue(null, "reactions.after", "A pre-defeat exchange belongs to an active objective.");
        if (type === "support") {
            const gate = byId.get(reaction.after);
            if (!gate || !["objective", "guide"].includes(gate.type))
                issue(null, "reactions.after", "Choose the objective that unlocks this support response.");
            const supportingFaction = ((roster && roster.factions) || []).find(
                (faction) => faction.id === target,
            );
            if (
                roster &&
                (supportingFaction
                    ? !Number.isInteger(supportingFaction.support_interval_seconds)
                    : !allowMissingFactionReferences)
            )
                issue(
                    null,
                    "reactions.when.target",
                    "This faction is not configured to send campaign support.",
                );
        }
        // The gate changes when one response may fire, not which world event it owns.
        const eventKey = type === "first_contact" ? type : type + ":" + (target || (when && when.relation));
        if (reactionEvents.has(eventKey))
            issue(null, "reactions.when", "Each event can have only one response.");
        if (type === "first_contact" || typeof target === "string" || (when && when.relation))
            reactionEvents.add(eventKey);
        if (reaction.speaker && !own(speakers, reaction.speaker))
            issue(null, "reactions.speaker", "Choose an existing character.");
        text(null, "reactions.speaker_role_key", reaction.speaker_role_key, false);
        if (requireSpeakerPortraits && reaction.presentation !== "exchange") {
            if (reaction.speaker) checkDialogueSpeaker(null, reaction.speaker);
            else {
                if (!reaction.speaker_role_key)
                    issue(null, "reactions.speaker", "A contact response needs a speaker role.");
                const possibleFactions =
                    type === "contact" && when && when.target
                        ? ((roster && roster.factions) || []).filter((faction) => faction.id === when.target)
                        : type === "contact" && when && when.relation
                          ? ((roster && roster.factions) || []).filter(
                                (faction) => faction.relation === when.relation,
                            )
                          : type === "first_contact" && when && Array.isArray(when.targets)
                            ? ((roster && roster.factions) || []).filter((faction) =>
                                  when.targets.includes(faction.id),
                              )
                            : type === "support" && target
                              ? ((roster && roster.factions) || []).filter((faction) => faction.id === target)
                              : [];
                if (!possibleFactions.length)
                    issue(null, "reactions.speaker", "A dynamic speaker needs a faction with a portrait.");
                else
                    possibleFactions.forEach((faction) => {
                        if (!availableAvatar(faction.avatar))
                            issue(
                                null,
                                "reactions.speaker",
                                "The contacted faction needs an existing portrait: " + faction.name + ".",
                            );
                    });
            }
        }
        if (reaction.presentation === "exchange") {
            if (!Array.isArray(reaction.exchange) || !reaction.exchange.length)
                issue(null, "reactions.exchange", "An automatic response needs at least one exchange line.");
            if (
                reaction.title_key ||
                reaction.body_key ||
                reaction.choices ||
                reaction.outcome != null ||
                reaction.gold_cost != null ||
                reaction.emote
            )
                issue(
                    null,
                    "reactions.presentation",
                    "An automatic exchange cannot also open a negotiation or reaction modal.",
                );
        } else {
            text(null, "reactions.title_key", reaction.title_key, true);
            text(null, "reactions.body_key", reaction.body_key, true);
        }
        if (reaction.outcome != null && !["neutral", "allied", "enemy"].includes(reaction.outcome))
            issue(null, "reactions.outcome", "Choose a neutral, allied or enemy result.");
        if (
            reaction.gold_cost != null &&
            (!Number.isFinite(reaction.gold_cost) ||
                reaction.gold_cost < 0 ||
                reaction.gold_cost > 1_000_000 ||
                (reaction.gold_cost > 0 && reaction.outcome !== "allied"))
        )
            issue(null, "reactions.gold_cost", "Gold can only be paid for an alliance (0–1,000,000).");
        if (reaction.choices != null) {
            if (
                !Array.isArray(reaction.choices) ||
                reaction.choices.length < 2 ||
                reaction.choices.length > 4 ||
                reaction.outcome != null ||
                !["contact", "first_contact"].includes(type)
            )
                issue(
                    null,
                    "reactions.choices",
                    "Contact negotiations need 2–4 choices and no automatic result.",
                );
            const choiceIds = new Set();
            (Array.isArray(reaction.choices) ? reaction.choices : []).forEach((choice) => {
                if (!object(choice)) return issue(null, "reactions.choices", "Invalid negotiation choice.");
                knownFields(
                    choice,
                    ["id", "label_key", "body_key", "relation", "gold_cost"],
                    null,
                    "reactions.choices",
                );
                if (!id(choice.id) || choiceIds.has(choice.id))
                    issue(null, "reactions.choices.id", "Negotiation choice IDs must be unique.");
                choiceIds.add(choice.id);
                text(null, "reactions.choices.label_key", choice.label_key, true);
                text(null, "reactions.choices.body_key", choice.body_key, false);
                if (!["neutral", "allied", "enemy"].includes(choice.relation))
                    issue(
                        null,
                        "reactions.choices.relation",
                        "Every choice needs a neutral, allied or enemy result.",
                    );
                if (
                    choice.gold_cost != null &&
                    (!Number.isFinite(choice.gold_cost) ||
                        choice.gold_cost < 0 ||
                        choice.gold_cost > 1_000_000 ||
                        (choice.gold_cost > 0 && choice.relation !== "allied"))
                )
                    issue(
                        null,
                        "reactions.choices.gold_cost",
                        "Gold can only be paid for an alliance (0–1,000,000).",
                    );
            });
            if (
                Array.isArray(reaction.choices) &&
                reaction.choices.some(
                    (choice) => choice && choice.relation === "allied" && Number(choice.gold_cost || 0) > 0,
                ) &&
                reaction.choices.filter(
                    (choice) => choice && choice.relation !== "allied" && Number(choice.gold_cost || 0) === 0,
                ).length !== 1
            ) {
                issue(
                    null,
                    "reactions.choices",
                    "Paid alliances need exactly one free non-allied fallback choice.",
                );
            }
        }
        if (reaction.outcome != null && reaction.choices != null)
            issue(null, "reactions", "Choose an automatic result or negotiation choices, not both.");
        if (
            type === "support" &&
            (reaction.outcome != null || reaction.gold_cost != null || reaction.choices != null)
        )
            issue(null, "reactions", "Support notices cannot change diplomacy.");
    });
    const edges = (step) => campaignEdges(step, byId);
    const menuFlowReachable = new Set<string>();
    if (object(definition.menu_guide) && byId.has(definition.menu_guide.entry)) {
        const visitMenu = (key) => {
            if (menuFlowReachable.has(key) || !byId.has(key)) return;
            menuFlowReachable.add(key);
            edges(byId.get(key)).forEach(visitMenu);
        };
        visitMenu(definition.menu_guide.entry);
        const menuEnds = new Set<string>(
            steps.filter((step) => step && step.type === "end").map((step) => step.id),
        );
        let menuChanged = true;
        while (menuChanged) {
            menuChanged = false;
            byId.forEach((step) => {
                const targets = edges(step);
                if (
                    !menuEnds.has(step.id) &&
                    targets.length &&
                    targets.every((target) => menuEnds.has(target))
                ) {
                    menuEnds.add(step.id);
                    menuChanged = true;
                }
            });
        }
        menuFlowReachable.forEach((key) => {
            const step = byId.get(key);
            if (!menuEnds.has(key)) issue(step, "next", "Every menu-guide path must lead to an ending.");
            if (
                ["objective", "guide"].includes(step.type) &&
                (!step.trigger || step.trigger.type !== "ui" || (step.guide && step.guide.kind !== "ui"))
            )
                issue(step, "trigger", "The menu guide can teach menu controls, not battlefield actions.");
            if (
                step.trigger &&
                step.trigger.type === "ui" &&
                (typeof step.trigger.action !== "string" ||
                    (!step.trigger.action.startsWith("menu_") && step.trigger.action !== "campaign_replay"))
            )
                issue(step, "trigger.action", "Choose a campaign-menu control for the return guide.");
        });
    }
    const reached = new Set<string>();
    const visit = (key) => {
        if (reached.has(key) || !byId.has(key)) return;
        reached.add(key);
        edges(byId.get(key)).forEach(visit);
    };
    visit(definition.entry);
    const unlockStateByStep = campaignUnlockStates(byId, definition.entry);
    reached.forEach((key) => {
        const step = byId.get(key);
        const policy = applyCampaignUnlocks(unlockStateByStep.get(key), step.unlocks);
        const seenRequirements = new Set();
        stepCampaignRequirements(step).forEach((requirement) => {
            const requirementKey = JSON.stringify(requirement);
            if (seenRequirements.has(requirementKey)) return;
            seenRequirements.add(requirementKey);
            if (requirement.type === "building") {
                const available = Number(policy.buildings[requirement.kind]) || 0;
                if (available < requirement.level)
                    issue(
                        step,
                        "unlocks",
                        "This route reaches the step without " +
                            requirement.kind +
                            " level " +
                            requirement.level +
                            " unlocked (available: " +
                            available +
                            ").",
                    );
            } else if (!campaignAllowsAction(policy, requirement.action)) {
                issue(
                    step,
                    "unlocks",
                    "This route reaches the step without the " + requirement.action + " action unlocked.",
                );
            }
        });
    });
    byId.forEach((step) => {
        if (!reached.has(step.id) && !menuFlowReachable.has(step.id))
            warnings.push({
                step: step.id,
                field: "id",
                message: "This step is not connected to either flow.",
            });
    });
    const ends = new Set<string>(steps.filter((s) => s && s.type === "end").map((s) => s.id));
    let changed = true;
    while (changed) {
        changed = false;
        byId.forEach((step) => {
            const targets = edges(step);
            const canReachEnding =
                step.type === "choice"
                    ? targets.some((target) => ends.has(target))
                    : targets.length && targets.every((target) => ends.has(target));
            if (!ends.has(step.id) && canReachEnding) {
                ends.add(step.id);
                changed = true;
            }
        });
    }
    reached.forEach((key) => {
        if (!ends.has(key)) issue(byId.get(key), "next", "This path cannot reach an ending.");
    });
    const checked = new Set<string>(),
        visiting = new Set<string>();
    const canAdvanceWithoutInput = (step) => {
        if (!step || !["objective", "guide"].includes(step.type) || !step.trigger) return false;
        const trigger = step.trigger;
        const canLoseTarget =
            [
                "attack",
                "contact",
                "alliance",
                "defeated",
                "fleet",
                "resource_transfer",
                "camera_target",
                "hover",
                "zoom_in_complete",
            ].includes(trigger.type) &&
            (trigger.target || trigger.recipient || Array.isArray(trigger.targets));
        return (
            trigger.type === "elapsed" ||
            trigger.type === "troops" ||
            trigger.scope !== "step" ||
            canLoseTarget
        );
    };
    function cycle(key) {
        const step = byId.get(key);
        if (!canAdvanceWithoutInput(step) || checked.has(key)) return;
        if (visiting.has(key)) {
            issue(step, "next", "Automatic steps form a loop. Add a conversation or decision.");
            return;
        }
        visiting.add(key);
        edges(step).forEach(cycle);
        visiting.delete(key);
        checked.add(key);
    }
    byId.forEach((step) => cycle(step.id));
    return { errors, warnings };
}
