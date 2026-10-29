import * as rules from "./rules.ts";
import type { CampaignValidationContext } from "./validation-context.ts";

export function validateReactions(context: CampaignValidationContext) {
    const {
        definition,
        roster,
        allowMissingFactionReferences,
        requireSpeakerPortraits,
        byId,
        rosterFactions,
        speakers,
        responses,
        issue,
        knownFields,
        text,
        checkEmote,
        checkExchange,
        notEmoteLine,
        checkDialogueSpeaker,
        availableAvatar,
    } = context;
    const { REACTION_DIALOGUE_FIELDS, own, object, id } = rules;

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
        if (!["contact", "first_contact", "support", "pre_defeat", "alliance"].includes(type))
            issue(
                null,
                "reactions.when",
                "Choose a supported contact, alliance, support, or pre-defeat event.",
            );
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
            ["support", "alliance"].includes(type) &&
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
}
