import type {
    CampaignDefinition,
    CampaignRecord,
    CampaignRoster,
    CampaignStep,
    CampaignValidationIssue,
} from "./types.ts";

export interface CampaignValidationContext {
    definition: CampaignDefinition;
    roster: CampaignRoster;
    options: CampaignRecord;
    allowMissingFactionReferences: boolean;
    requireSpeakerPortraits: boolean;
    errors: CampaignValidationIssue[];
    warnings: CampaignValidationIssue[];
    steps: CampaignStep[];
    byId: Map<string, CampaignStep>;
    factions: Set<string>;
    rosterFactions: Set<string>;
    speakers: CampaignRecord;
    responses: CampaignRecord[];
    issue(step: CampaignStep | null | undefined, field: string, message: string): number;
    knownFields(
        value: CampaignRecord,
        allowed: readonly string[],
        step: CampaignStep | null,
        field: string,
    ): void;
    text(step: CampaignStep | null, field: string, key: string, required: boolean): void;
    checkEmoteText(step: CampaignStep | null, field: string, key: string): void;
    checkDialogueSpeaker(step: CampaignStep | null, speakerKey: string): void;
    availableAvatar(avatar: string): boolean;
    destination(step: CampaignStep, field: string, target: string | undefined): void;
    condition(step: CampaignStep, when: CampaignRecord): void;
    notEmoteLine(step: CampaignStep | null, field: string, key: string): void;
    checkEmote(step: CampaignStep | null, value: CampaignRecord, prefix: string): void;
    checkExchange(step: CampaignStep | null, lines: CampaignRecord[], field: string): void;
}
