import type { CampaignApi, CampaignViewApi } from "./campaign/types.ts";

type DropdownOption = { value: string | number; label: string };
type DropdownConfig = {
    key: string;
    name?: string;
    value?: string | number;
    options?: DropdownOption[];
    className?: string;
    label?: string;
    setting?: string;
};

declare global {
    const SOW_t: (key: string, values?: Record<string, unknown>) => string;
    const SOW_renderDropdown: (config: DropdownConfig) => string;

    interface SowAuthState {
        linked?: boolean;
        pending?: boolean;
        linkedProviders?: string[];
        platform?: string;
        provider?: string;
        canSignOut?: boolean;
    }

    interface SowPlatformIdentity {
        provider?: string;
        displayName?: string;
        externalId?: string;
        avatarUrl?: string | null;
        token?: string | null;
    }

    interface SowBuildingArt {
        atlas: string;
        cell_size: number;
        atlas_width: number;
        atlas_height: number;
        sprites: Record<string, Record<string, { x: number; y: number }>>;
    }

    interface Window {
        [key: `SOW_${string}`]: unknown;
        [key: `SOW${string}`]: unknown;
        SOWCampaign: CampaignApi;
        SOWCampaignView: CampaignViewApi;
        SOW_getAuthState?: () => SowAuthState;
        SOW_hudIcon: (name: string) => string;
        SOW_leaderArtUrl: (slug: string, variant?: string) => string;
        SOW_prepareLeaderArt: (slug: string, priority?: string) => Promise<boolean>;
        SOW_menu_flow: { canGuide(): boolean };
        SOW_menu_guide: {
            update(state: unknown, facts?: unknown): void;
            pause(): void;
        };
        SOW_PLATFORM_IDENTITY: SowPlatformIdentity | null;
        SOW_BUILDING_ART: SowBuildingArt;
        SOW_BLOCKED_IDS?: string[];
        SOW_onStateUpdate: (state: unknown) => void;
        SOW_hover_card_update: (raw: string) => void;
        SOW_tutorial_pointer_update: (raw: string) => void;
        SOW_t: (key: string, values?: Record<string, unknown>) => string;
        SOW_assetUrl: (url: string) => string;
        SOW_menu_command: (message: string) => void;
        SOW_trackExperienceEvent: (payload: unknown) => void;
        SOW_isAndroidTwa: () => boolean;
        SOW_hasText: (key: string) => boolean;
        SOW_renderHeroBriefing: (leaderOrSlug: unknown, surface?: string, headingId?: string) => string;
        SOW_getLocale: () => string;
        SOW_getLocaleLabel: (locale: string) => string;
        SOW_getSupportedLocales: () => string[];
        SOW_setLocale: (locale: string) => Promise<unknown>;
        SOW_tutorial_exit_context: (episodeId: string) => unknown;
        SOW_pokiMeasure: (category: string, what?: string, action?: string) => void;
        SOW_portalCaptureExperienceEvent: (event: unknown) => void;
        SOW_analyticsTrack: (name: string, props?: unknown) => void;
        SOW_menu_music_cinematic: (active: boolean) => void;
        SOW_LOCALE: string;
        SOW_LOCALE_DEFAULT: string;
        SOW_LOCALE_CATALOG_VERSION: number;
        SOW_LOCALE_CODES: string[];
        SOW_LOCALE_BASE: string;
        SOW_LOCALE_BEFORE_PAINT: boolean;
        SOW_LOCALE_VERSION: string;
        SOW_ASSET_TS: Record<string, string>;
        SOW_BUILD_TS: string;
        SOW_ASSETS_URL: string;
        SOW_BOOT_UI_BASE: string;
        SOW_PORTAL: string;
        SOW_SOURCE_URL: string;
        SOW_MENU_STATE: unknown;
        SOW_I18N_READY: Promise<unknown>;
    }

    interface HTMLElement {
        _shell?: HTMLElement | null;
        _shellWasInert?: boolean;
        _restoreFocus?: HTMLElement | null;
    }

    interface Event {
        detail: unknown;
    }
}

export {};
