export type CampaignStepType = "scene" | "choice" | "objective" | "guide" | "end";
// Campaign definitions are open data: the validator owns field-specific checks,
// while this record preserves unknown authored fields through edit and playback.
// eslint-disable-next-line @typescript-eslint/no-explicit-any -- campaign JSON is intentionally open-ended.
export type CampaignRecord = Record<string, any>;

export interface CampaignStep extends CampaignRecord {
    id: string;
    type: CampaignStepType;
    next?: string;
}

export interface CampaignDefinition extends CampaignRecord {
    entry: string;
    steps: CampaignStep[];
}

export interface CampaignFaction extends CampaignRecord {
    id: string;
    name: string;
}

export interface CampaignRoster extends CampaignRecord {
    factions: CampaignFaction[];
}

export interface CampaignUnlocks {
    buildings: Record<string, number>;
    actions: string[];
}

export interface CampaignValidationIssue extends CampaignRecord {
    message: string;
    step?: string;
    field?: string;
}

export interface CampaignValidationReport extends CampaignRecord {
    errors: CampaignValidationIssue[];
    warnings: CampaignValidationIssue[];
}

export interface CampaignMachineView extends CampaignRecord {
    step: CampaignStep;
    waiting: boolean;
    paused: boolean;
}

export interface CampaignMachine {
    readonly definition: CampaignDefinition;
    state: CampaignRecord;
    update(facts?: CampaignRecord, ui?: CampaignRecord): CampaignMachineView;
    advance(choice?: string | null, expectedStepId?: string, allowUnavailable?: boolean): boolean;
    jump(stepId: string, resetProgress?: boolean): boolean;
    replaceDefinition(definition: CampaignDefinition, resetProgress?: boolean): boolean;
    setPaused(paused: boolean): void;
    view(): CampaignMachineView;
    dispose(): void;
}

export interface CampaignAnchor extends CampaignRecord {
    x: number;
    y: number;
    width?: number;
    height?: number;
}

export interface CampaignViewOptions extends CampaignRecord {
    translate?: (key: string) => string;
    asset?: (path: string) => string;
    onDismiss?: () => void;
}

export interface CampaignView {
    render(view: CampaignMachineView | null, context?: CampaignRecord): void;
    updateCameraAnchor(x: number, y: number): void;
    destroy(): void;
}

export interface CampaignViewApi {
    mount(root: HTMLElement, options?: CampaignViewOptions): CampaignView;
    isCompactViewport(root?: HTMLElement | null, width?: number, height?: number): boolean;
    compactGuideMetric(step: CampaignStep, progress: CampaignRecord): { text: string } | null;
    resolveUiTarget(
        key: string,
        root: ParentNode,
        episodeId?: string,
        facts?: CampaignRecord,
    ): HTMLElement | null;
    resolveUiAnchor(element: HTMLElement, spotlightElement?: HTMLElement | null): CampaignAnchor;
    resolveUiRangeAnchor(element: HTMLInputElement, value: number): CampaignAnchor | null;
    resolveUiRangeGuideAnchor(
        element: HTMLInputElement,
        value: number,
        targetValue: number,
    ): CampaignAnchor | null;
}

export interface CampaignApi {
    TYPES: readonly CampaignStepType[];
    TEAMS: readonly string[];
    METRICS: Record<string, string>;
    ROUTE_FACTS: readonly string[];
    FACTION_FIELDS: readonly string[];
    WAIT_MS: number;
    EXCHANGE_DEFAULT_REPLY_DELAY_MS: number;
    EXCHANGE_MAX_DELAY_MS: number;
    UI_TARGETS: Record<string, string>;
    MENU_GUIDE_OUTCOMES: CampaignRecord;
    STRUCTURE_LEVEL_LIMITS: Record<string, number>;
    CAMPAIGN_ACTIONS: readonly string[];
    CAMPAIGN_ACTION_BUILDING_REQUIREMENTS: CampaignRecord;
    emptyCampaignUnlocks(): CampaignUnlocks;
    applyCampaignUnlocks(
        previous: CampaignUnlocks | null,
        grant: Partial<CampaignUnlocks> | null,
    ): CampaignUnlocks;
    campaignAllowsBuilding(unlocks: CampaignUnlocks | null, kind: string, level: number): boolean;
    campaignAllowsAction(unlocks: CampaignUnlocks | null, action: string): boolean;
    campaignUnlockStates(
        byId: Map<string, CampaignStep>,
        entry: string,
        choices?: CampaignRecord,
        facts?: CampaignRecord,
    ): Map<string, CampaignUnlocks>;
    campaignUnlocksAtStep(
        byId: Map<string, CampaignStep>,
        entry: string,
        key: string,
        choices?: CampaignRecord,
        facts?: CampaignRecord,
    ): CampaignUnlocks;
    zoomInputMode(info?: CampaignRecord): "pinch" | "trackpad" | "wheel";
    guideAction(step: CampaignStep): string | null;
    zoomOutProgress(facts: CampaignRecord): number;
    zoomOutTarget(facts: CampaignRecord, amount: number): number;
    renameFactionText(
        definition: CampaignDefinition,
        renames: CampaignRecord[],
        factions: CampaignFaction[],
    ): CampaignDefinition;
    removeFactionReferences(
        definition: CampaignDefinition,
        removedFactions: CampaignFaction[],
    ): CampaignRecord;
    replaceFactionStoryNames(text: string, definition: CampaignDefinition, roster: CampaignRoster): string;
    factionReferenceIds(definition: CampaignDefinition): Set<string>;
    validate(
        definition: CampaignDefinition,
        roster: CampaignRoster,
        options?: CampaignRecord,
    ): CampaignValidationReport;
    validateMenuGuides(definition: CampaignDefinition): CampaignValidationReport;
    create(
        definition: CampaignDefinition,
        entry?: string,
        roster?: CampaignRoster | null,
        clock?: CampaignRecord,
        hooks?: CampaignRecord,
    ): CampaignMachine;
}

declare global {
    interface Window {
        SOWCampaign: CampaignApi;
        SOWCampaignView: CampaignViewApi;
    }
}
