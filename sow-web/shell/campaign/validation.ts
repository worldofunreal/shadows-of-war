import * as rules from "./rules.ts";
import { validateSteps } from "./validation-steps.ts";
import { validateReactions } from "./validation-reactions.ts";
import { validateFlow } from "./validation-flow.ts";
import type { CampaignValidationContext } from "./validation-context.ts";
import type {
    CampaignDefinition,
    CampaignRoster,
    CampaignValidationIssue,
    CampaignValidationReport,
    CampaignRecord,
    CampaignStep,
} from "./types.ts";
const {
    TEAMS,
    EMOTE_KINDS,
    EXCHANGE_MAX_DELAY_MS,
    EMOTE_TEXT_MAX_CHARS,
    ROUTE_FACTS,
    FACTION_FIELDS,
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
    const errors: CampaignValidationIssue[] = [],
        warnings: CampaignValidationIssue[] = [];
    const issue = (step: CampaignStep | null | undefined, field: string, message: string) =>
        errors.push({ step: step && step.id, field, message });
    const knownFields = (
        value: CampaignRecord,
        allowed: readonly string[],
        step: CampaignStep | null,
        field: string,
    ) => {
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
    const factions = new Set<string>(["player"]),
        factionNames = new Set<string>(),
        rosterFactions = new Set<string>();
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
            knownFields(faction, FACTION_FIELDS, null, "roster.factions");
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
    function text(step: CampaignStep | null, field: string, key: string, required: boolean) {
        if (key == null && !required) return;
        if (typeof key !== "string" || !/^tutorial\.[a-zA-Z0-9_.-]+$/.test(key))
            issue(step, field, "Choose a tutorial text key.");
        else if (options.hasText && !options.hasText(key))
            issue(step, field, "Missing shared English text: " + key);
    }
    function checkEmoteText(step: CampaignStep | null, field: string, key: string) {
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
            if (code! <= 0x1f || (code! >= 0x7f && code! <= 0x9f)) {
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
            knownFields(
                speaker,
                ["name", "name_key", "avatar", "faction", "base_expression"],
                null,
                "speakers." + key,
            );
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
                if (speaker.base_expression != null) {
                    const rule = speaker.base_expression;
                    knownFields(
                        rule,
                        ["expression", "after_expression", "until"],
                        null,
                        "speakers." + key + ".base_expression",
                    );
                    knownFields(
                        rule && rule.until,
                        ["type", "target"],
                        null,
                        "speakers." + key + ".base_expression.until",
                    );
                    if (
                        !object(rule) ||
                        !["happy", "calm", "fear", "angry"].includes(rule.expression) ||
                        (rule.after_expression != null &&
                            !["happy", "calm", "fear", "angry"].includes(rule.after_expression)) ||
                        !object(rule.until) ||
                        rule.until.type !== "alliance" ||
                        typeof rule.until.target !== "string" ||
                        (roster && !rosterFactions.has(rule.until.target) && !allowMissingFactionReferences)
                    ) {
                        issue(
                            null,
                            "speakers." + key + ".base_expression",
                            "Choose valid expressions and an existing alliance target.",
                        );
                    }
                }
                text(null, "speakers", speaker.name_key, false);
            }
        });
    function availableAvatar(avatar: string) {
        return (
            typeof avatar === "string" &&
            avatar !== "null" &&
            /^[a-z][a-z0-9_]*$/.test(avatar) &&
            (!options.hasAvatar || options.hasAvatar(avatar))
        );
    }
    function speakerHasPortrait(speakerKey: string) {
        const character = speakers[speakerKey];
        if (!object(character)) return false;
        if (character.faction) {
            const faction = ((roster && roster.factions) || []).find((item) => item.id === character.faction);
            return Boolean(faction && availableAvatar(faction.avatar || character.avatar));
        }
        return availableAvatar(character.avatar);
    }
    function checkDialogueSpeaker(step: CampaignStep | null, speakerKey: string) {
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
    function destination(step: CampaignStep, field: string, target: string | undefined) {
        if (typeof target !== "string" || !byId.has(target))
            issue(step, field, "Unknown destination: " + String(target || "(empty)"));
    }
    function condition(step: CampaignStep, when: CampaignRecord) {
        if (!object(when) || ["choice", "fact"].filter((key) => own(when, key)).length !== 1)
            return issue(step, "routes", "Choose one condition: player decision or game fact.");
        knownFields(when, own(when, "choice") ? ["choice", "equals"] : ["fact", "gte"], step, "routes.when");
        if (own(when, "choice")) {
            // A condition may name a player decision or a contact response (its id) and the answer picked there.
            const choice = byId.get(when.choice);
            const response = (definition.reactions || []).find(
                (reaction: CampaignRecord) => reaction && reaction.id === when.choice,
            );
            const answers =
                choice && choice.type === "choice" ? choice.choices : response && response.choices;
            if (!Array.isArray(answers) || !answers.some((option) => option && option.id === when.equals))
                issue(step, "routes", "Condition refers to an unknown decision or answer.");
        } else if (!ROUTE_FACTS.includes(when.fact) || !Number.isFinite(when.gte))
            issue(step, "routes", "Invalid game fact condition.");
    }
    const responses = Array.isArray(definition.reactions) ? definition.reactions : [];
    const emoteLines = new Set(
        steps
            .concat(responses)
            .filter(object)
            .flatMap((value) => [value].concat(Array.isArray(value.exchange) ? value.exchange : []))
            .map((value) => value.emote_text_key)
            .filter(Boolean),
    );
    function notEmoteLine(step: CampaignStep | null, field: string, key: string) {
        if (emoteLines.has(key))
            issue(step, field, "This line is an emote bubble; dialogue cannot reuse it.");
    }
    function checkEmote(step: CampaignStep | null, value: CampaignRecord, prefix: string) {
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
    function checkExchange(step: CampaignStep | null, lines: CampaignRecord[], field: string) {
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
    const context: CampaignValidationContext = {
        definition,
        roster,
        options,
        allowMissingFactionReferences,
        requireSpeakerPortraits,
        errors,
        warnings,
        steps,
        byId,
        factions,
        rosterFactions,
        speakers,
        responses,
        issue,
        knownFields,
        text,
        checkEmoteText,
        checkDialogueSpeaker,
        availableAvatar,
        destination,
        condition,
        notEmoteLine,
        checkEmote,
        checkExchange,
    };
    validateSteps(context);
    validateReactions(context);
    validateFlow(context);
    return { errors, warnings };
}
