import * as rules from "./rules.ts";
import type { CampaignStep } from "./types.ts";
import type { CampaignValidationContext } from "./validation-context.ts";

export function validateFlow(context: CampaignValidationContext) {
    const { definition, steps, byId, issue, warnings } = context;
    const {
        applyCampaignUnlocks,
        campaignAllowsAction,
        campaignEdges,
        campaignUnlockStates,
        stepCampaignRequirements,
        object,
        id,
    } = rules;

    const edges = (step: CampaignStep) => campaignEdges(step, byId);
    const menuFlowReachable = new Set<string>();
    if (object(definition.menu_guide) && byId.has(definition.menu_guide.entry)) {
        const visitMenu = (key: string) => {
            if (menuFlowReachable.has(key) || !byId.has(key)) return;
            menuFlowReachable.add(key);
            edges(byId.get(key)!).forEach(visitMenu);
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
                ["objective", "guide"].includes(step!.type) &&
                (!step!.trigger || step!.trigger.type !== "ui" || (step!.guide && step!.guide.kind !== "ui"))
            )
                issue(step, "trigger", "The menu guide can teach menu controls, not battlefield actions.");
            if (
                step!.trigger &&
                step!.trigger.type === "ui" &&
                (typeof step!.trigger.action !== "string" ||
                    (!step!.trigger.action.startsWith("menu_") && step!.trigger.action !== "campaign_replay"))
            )
                issue(step, "trigger.action", "Choose a campaign-menu control for the return guide.");
        });
    }
    const reached = new Set<string>();
    const visit = (key: string) => {
        if (reached.has(key) || !byId.has(key)) return;
        reached.add(key);
        edges(byId.get(key)!).forEach(visit);
    };
    visit(definition.entry);
    const unlockStateByStep = campaignUnlockStates(byId, definition.entry);
    reached.forEach((key) => {
        const step = byId.get(key)!;
        const policy = applyCampaignUnlocks(unlockStateByStep.get(key) ?? null, step!.unlocks);
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
    const canAdvanceWithoutInput = (step: CampaignStep) => {
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
    function cycle(key: string) {
        const step = byId.get(key)!;
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
}
