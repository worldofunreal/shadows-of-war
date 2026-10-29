import * as rules from "./campaign/rules.ts";
import { validate } from "./campaign/validation.ts";
import { validateMenuGuides } from "./campaign/menu-guide-validation.ts";
import { create } from "./campaign/runtime.ts";
import type { CampaignApi } from "./campaign/types.ts";

const api: CampaignApi = {
    TYPES: rules.TYPES,
    TEAMS: rules.TEAMS,
    METRICS: rules.METRICS,
    ROUTE_FACTS: rules.ROUTE_FACTS,
    FACTION_FIELDS: rules.FACTION_FIELDS,
    WAIT_MS: rules.WAIT_MS,
    EXCHANGE_DEFAULT_REPLY_DELAY_MS: rules.EXCHANGE_DEFAULT_REPLY_DELAY_MS,
    EXCHANGE_MAX_DELAY_MS: rules.EXCHANGE_MAX_DELAY_MS,
    UI_TARGETS: rules.UI_TARGETS,
    MENU_GUIDE_OUTCOMES: rules.MENU_GUIDE_OUTCOMES,
    STRUCTURE_LEVEL_LIMITS: rules.STRUCTURE_LEVEL_LIMITS,
    CAMPAIGN_ACTIONS: rules.CAMPAIGN_ACTIONS,
    CAMPAIGN_ACTION_BUILDING_REQUIREMENTS: rules.CAMPAIGN_ACTION_BUILDING_REQUIREMENTS,
    emptyCampaignUnlocks: rules.emptyCampaignUnlocks,
    applyCampaignUnlocks: rules.applyCampaignUnlocks,
    campaignAllowsBuilding: rules.campaignAllowsBuilding,
    campaignAllowsAction: rules.campaignAllowsAction,
    campaignUnlockStates: rules.campaignUnlockStates,
    campaignUnlocksAtStep: rules.campaignUnlocksAtStep,
    zoomInputMode: rules.zoomInputMode,
    guideAction: rules.guideAction,
    zoomOutProgress: rules.zoomOutProgress,
    zoomOutTarget: rules.zoomOutTarget,
    renameFactionText: rules.renameFactionText,
    removeFactionReferences: rules.removeFactionReferences,
    replaceFactionStoryNames: rules.replaceFactionStoryNames,
    factionReferenceIds: rules.factionReferenceIds,
    validate,
    validateMenuGuides,
    create,
    campaignBaseExpressions: rules.campaignBaseExpressions,
};

if (typeof window !== "undefined") window.SOWCampaign = api;

export default api;
