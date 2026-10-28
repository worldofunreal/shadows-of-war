import * as rules from "./rules.ts";
import type {
    CampaignDefinition,
    CampaignStep,
    CampaignValidationIssue,
    CampaignValidationReport,
} from "./types.ts";
const { UI_TARGETS, MENU_GUIDE_OUTCOMES, own, object, id } = rules;

export function validateMenuGuides(definition: CampaignDefinition): CampaignValidationReport {
    const errors: CampaignValidationIssue[] = [];
    const issue = (step: CampaignStep | null | undefined, field: string, message: string) =>
        errors.push({ step: step && step.id, field, message });
    const allowedGuides = ["welcome"];
    if (!object(definition) || definition.version !== 1 || definition.episode_id !== "menu") {
        issue(null, "version", "Menu guides must use version 1 and episode_id menu.");
        return { errors, warnings: [] };
    }
    const allowedFields = [
        "version",
        "episode_id",
        "text_namespace",
        "menu_guides",
        "steps",
        "layout",
        "strings",
    ];
    Object.keys(definition)
        .filter((key) => !allowedFields.includes(key))
        .forEach((key) => issue(null, key, "Unsupported menu-guide field."));
    if (definition.text_namespace !== "menu.guide.")
        issue(null, "text_namespace", "Use the menu.guide. text namespace.");
    const guides = object(definition.menu_guides) ? definition.menu_guides : {};
    Object.keys(guides)
        .filter((key) => !allowedGuides.includes(key))
        .forEach((key) => issue(null, "menu_guides", "Unknown menu journey: " + key));
    allowedGuides.forEach((key) => {
        if (!object(guides[key])) issue(null, "menu_guides", "Missing menu journey: " + key);
    });
    const strings = object(definition.strings) ? definition.strings : {};
    if (!object(definition.strings)) issue(null, "strings", "Add built-in menu copy by language.");
    if (!object(strings.en)) issue(null, "strings.en", "English menu copy is required.");
    Object.entries(strings).forEach(([locale, catalog]) => {
        if (!/^[a-z]{2,3}(?:-[a-z]{2})?$/.test(locale) || !object(catalog))
            issue(null, "strings", "Invalid menu-copy language: " + locale);
        else
            Object.entries(catalog).forEach(([key, value]) => {
                if (
                    !/^menu\.guide\.[a-z][a-z0-9_.-]{0,127}$/.test(key) ||
                    typeof value !== "string" ||
                    value.length > 1000
                )
                    issue(null, "strings", "Invalid built-in menu copy: " + key);
            });
    });
    const steps = Array.isArray(definition.steps) ? definition.steps : [];
    if (!steps.length || steps.length > 512) issue(null, "steps", "Use between 1 and 512 menu-guide steps.");
    const byId = new Map<string, CampaignStep>();
    steps.forEach((step) => {
        if (!object(step) || !id(step.id) || byId.has(step.id))
            return issue(step, "id", "Step IDs must be valid and unique.");
        byId.set(step.id, step);
        if (!["scene", "choice", "guide", "end"].includes(step.type))
            issue(step, "type", "Choose a scene, decision, hand guide, or ending.");
        ["title_key", "body_key", "hint_key"].forEach((field) => {
            const key = step[field];
            if (key != null && (typeof key !== "string" || !own(strings.en, key)))
                issue(step, field, "Choose built-in English menu copy.");
        });
        if (step.type === "guide") {
            const trigger = step.trigger,
                guide = step.guide;
            if (!object(trigger) || trigger.type !== "ui" || !own(UI_TARGETS, trigger.action))
                issue(step, "trigger", "A menu hand guide needs a registered menu control.");
            if (
                !object(guide) ||
                guide.kind !== "ui" ||
                guide.gesture !== "tap" ||
                !own(UI_TARGETS, guide.target)
            )
                issue(step, "guide", "Point the hand at a registered menu control.");
            const outcomes = object(trigger)
                ? Array.isArray(trigger.outcomes)
                    ? trigger.outcomes
                    : trigger.outcome
                      ? [trigger.outcome]
                      : []
                : [];
            if (!outcomes.length || outcomes.some((outcome) => !own(MENU_GUIDE_OUTCOMES, outcome))) {
                issue(step, "trigger.outcome", "A menu guide advances only on registered outcomes.");
            } else {
                if (MENU_GUIDE_OUTCOMES[outcomes[0]].control !== trigger.action)
                    issue(step, "trigger.outcome", "The primary outcome must match the guided control.");
                if (
                    outcomes.length > 1 &&
                    outcomes.some((outcome) => MENU_GUIDE_OUTCOMES[outcome].kind !== "counter")
                )
                    issue(
                        step,
                        "trigger.outcomes",
                        "Alternative outcomes must be confirmed action counters.",
                    );
                if (trigger.outcome != null && trigger.outcomes != null)
                    issue(step, "trigger.outcomes", "Use either one outcome or an outcome list.");
                if (
                    trigger.outcomes != null &&
                    (!Array.isArray(trigger.outcomes) ||
                        new Set(trigger.outcomes).size !== trigger.outcomes.length)
                )
                    issue(
                        step,
                        "trigger.outcomes",
                        "Outcome lists must contain distinct registered outcomes.",
                    );
            }
            if (object(trigger) && object(guide) && trigger.action !== guide.target)
                issue(step, "guide.target", "The hand and confirmed outcome must use the same control.");
        } else if (step.trigger || step.guide)
            issue(step, "trigger", "Only hand-guide steps use a menu control outcome.");
        if (step.type === "scene" && !step.title_key && !step.body_key)
            issue(step, "title_key", "Add title or body copy.");
        if (step.type === "choice") {
            if (!Array.isArray(step.choices) || step.choices.length < 2 || step.choices.length > 4)
                issue(step, "choices", "A decision needs 2–4 answers.");
            (Array.isArray(step.choices) ? step.choices : []).forEach((choice) => {
                if (
                    !object(choice) ||
                    !id(choice.id) ||
                    typeof choice.label_key !== "string" ||
                    !own(strings.en, choice.label_key) ||
                    !id(choice.next)
                )
                    issue(step, "choices", "Each answer needs built-in copy and a destination.");
            });
        }
        if (step.wait_before != null && typeof step.wait_before !== "boolean")
            issue(step, "wait_before", "Choose whether to pause before showing this step.");
    });
    function destinations(step: CampaignStep) {
        return [step.next]
            .concat((Array.isArray(step.routes) ? step.routes : []).map((route) => route && route.next))
            .concat((Array.isArray(step.choices) ? step.choices : []).map((choice) => choice && choice.next))
            .filter((target) => typeof target === "string");
    }
    steps.forEach((step) => {
        if (!object(step)) return;
        destinations(step).forEach((target) => {
            if (!byId.has(target)) issue(step, "next", "Unknown destination: " + target);
        });
        if (step.routes != null) issue(step, "routes", "The welcome guide does not use conditional routes.");
    });
    allowedGuides.forEach((guideId) => {
        const guide = guides[guideId];
        if (!object(guide)) return;
        Object.keys(guide)
            .filter((key) => !["entry", "step_ids"].includes(key))
            .forEach((key) => issue(null, "menu_guides." + guideId, "Unsupported journey field: " + key));
        if (
            !id(guide.entry) ||
            !byId.has(guide.entry) ||
            !Array.isArray(guide.step_ids) ||
            !guide.step_ids.length ||
            guide.step_ids.some((stepId) => !byId.has(stepId)) ||
            new Set(guide.step_ids).size !== guide.step_ids.length ||
            !guide.step_ids.includes(guide.entry)
        ) {
            issue(
                null,
                "menu_guides." + guideId,
                "Each journey needs an opening and its distinct known step IDs.",
            );
            return;
        }
        const reached = new Set<string>();
        const visit = (stepId: string) => {
            if (reached.has(stepId) || !byId.has(stepId)) return;
            reached.add(stepId);
            destinations(byId.get(stepId)!).forEach(visit);
        };
        visit(guide.entry);
        const ends = new Set<string>(
            steps.filter((step) => step && step.type === "end").map((step) => step.id),
        );
        let changed = true;
        while (changed) {
            changed = false;
            byId.forEach((step) => {
                const next = destinations(step);
                if (
                    step.type !== "end" &&
                    next.length &&
                    next.every((target) => ends.has(target)) &&
                    !ends.has(step.id)
                ) {
                    ends.add(step.id);
                    changed = true;
                }
            });
        }
        reached.forEach((stepId) => {
            if (!ends.has(stepId))
                issue(byId.get(stepId), "next", "Every menu-guide path must reach an ending.");
            if (!guide.step_ids.includes(stepId))
                issue(byId.get(stepId), "step_ids", "Add every reachable step to the journey.");
        });
        guide.step_ids.forEach((stepId) => {
            if (!reached.has(stepId))
                issue(byId.get(stepId), "step_ids", "This step is not reachable from the journey opening.");
        });
    });
    return { errors, warnings: [] };
}
