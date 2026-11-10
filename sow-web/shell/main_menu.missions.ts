import { currencyAsset, esc, menuUi, send } from "./main_menu.core.ts";
import type { MenuMission, MenuMissionRecord, MenuState } from "./main_menu.state.ts";

// Missions and Retention Funnel runtime logic and presentation.

interface FallbackMissionDef {
    id: string;
    category: "onboarding" | "daily";
    title_key: string;
    description_key: string;
    target: number;
    crowns_reward: number;
    gems_reward: number;
    icon: string;
    order: number;
}

const FALLBACK_ONBOARDING: FallbackMissionDef[] = [
    {
        id: "onboarding_1_tutorial",
        category: "onboarding",
        title_key: "missions.onboarding_tutorial_title",
        description_key: "missions.onboarding_tutorial_desc",
        crowns_reward: 150,
        gems_reward: 25,
        target: 1,
        icon: "shield",
        order: 1,
    },
    {
        id: "onboarding_2_structures",
        category: "onboarding",
        title_key: "missions.onboarding_structures_title",
        description_key: "missions.onboarding_structures_desc",
        crowns_reward: 300,
        gems_reward: 40,
        target: 5,
        icon: "tower",
        order: 2,
    },
    {
        id: "onboarding_3_rotation",
        category: "onboarding",
        title_key: "missions.onboarding_rotation_title",
        description_key: "missions.onboarding_rotation_desc",
        crowns_reward: 200,
        gems_reward: 30,
        target: 1,
        icon: "sword",
        order: 3,
    },
    {
        id: "onboarding_4_team",
        category: "onboarding",
        title_key: "missions.onboarding_team_title",
        description_key: "missions.onboarding_team_desc",
        crowns_reward: 250,
        gems_reward: 35,
        target: 1,
        icon: "team",
        order: 4,
    },
    {
        id: "onboarding_5_regional",
        category: "onboarding",
        title_key: "missions.onboarding_regional_title",
        description_key: "missions.onboarding_regional_desc",
        crowns_reward: 350,
        gems_reward: 50,
        target: 2,
        icon: "globe",
        order: 5,
    },
    {
        id: "onboarding_6_team_win",
        category: "onboarding",
        title_key: "missions.onboarding_team_win_title",
        description_key: "missions.onboarding_team_win_desc",
        crowns_reward: 500,
        gems_reward: 75,
        target: 1,
        icon: "crown",
        order: 6,
    },
];

const FALLBACK_DAILY_POOL: FallbackMissionDef[] = [
    {
        id: "daily_play_matches",
        category: "daily",
        title_key: "missions.daily_play_matches_title",
        description_key: "missions.daily_play_matches_desc",
        crowns_reward: 150,
        gems_reward: 20,
        target: 3,
        icon: "sword",
        order: 1,
    },
    {
        id: "daily_build_structures",
        category: "daily",
        title_key: "missions.daily_build_structures_title",
        description_key: "missions.daily_build_structures_desc",
        crowns_reward: 150,
        gems_reward: 20,
        target: 8,
        icon: "tower",
        order: 2,
    },
    {
        id: "daily_team_game",
        category: "daily",
        title_key: "missions.daily_team_game_title",
        description_key: "missions.daily_team_game_desc",
        crowns_reward: 200,
        gems_reward: 25,
        target: 2,
        icon: "team",
        order: 3,
    },
    {
        id: "daily_team_win",
        category: "daily",
        title_key: "missions.daily_team_win_title",
        description_key: "missions.daily_team_win_desc",
        crowns_reward: 250,
        gems_reward: 30,
        target: 1,
        icon: "crown",
        order: 4,
    },
    {
        id: "daily_west_heroes",
        category: "daily",
        title_key: "missions.daily_west_heroes_title",
        description_key: "missions.daily_west_heroes_desc",
        crowns_reward: 180,
        gems_reward: 25,
        target: 2,
        icon: "globe",
        order: 5,
    },
    {
        id: "daily_east_heroes",
        category: "daily",
        title_key: "missions.daily_east_heroes_title",
        description_key: "missions.daily_east_heroes_desc",
        crowns_reward: 180,
        gems_reward: 25,
        target: 2,
        icon: "globe",
        order: 6,
    },
    {
        id: "daily_free_rotation",
        category: "daily",
        title_key: "missions.daily_free_rotation_title",
        description_key: "missions.daily_free_rotation_desc",
        crowns_reward: 200,
        gems_reward: 25,
        target: 2,
        icon: "sword",
        order: 7,
    },
    {
        id: "daily_eliminate_units",
        category: "daily",
        title_key: "missions.daily_eliminate_units_title",
        description_key: "missions.daily_eliminate_units_desc",
        crowns_reward: 150,
        gems_reward: 20,
        target: 50,
        icon: "sword",
        order: 8,
    },
];

function fallbackDailyMissions(epochDay: number): FallbackMissionDef[] {
    const pool = FALLBACK_DAILY_POOL;
    const count = pool.length;
    const idx0 = (epochDay * 7) % count;
    const idx1 = (idx0 + 1 + ((epochDay * 3) % (count - 1))) % count;
    let idx2 = (idx1 + 1 + ((epochDay * 5) % (count - 2))) % count;
    if (idx2 === idx0 || idx2 === idx1) {
        idx2 = (idx2 + 1) % count;
        if (idx2 === idx0 || idx2 === idx1) {
            idx2 = (idx2 + 1) % count;
        }
    }
    return [
        { ...pool[idx0], order: 1 },
        { ...pool[idx1], order: 2 },
        { ...pool[idx2], order: 3 },
    ];
}

export function resolveActiveMissions(state: MenuState | null): MenuMission[] {
    if (state && Array.isArray(state.missions) && state.missions.length > 0) {
        return state.missions;
    }
    if (!state) return [];

    const records: Record<string, MenuMissionRecord> = state.missions_progress || {};
    const tutorialDone = state.campaign?.tutorial_completed === true;
    const epochDay = Math.floor(Date.now() / 86400000);
    const result: MenuMission[] = [];

    for (const def of FALLBACK_ONBOARDING) {
        const rec = records[def.id];
        let progress = rec ? rec.progress : 0;
        if (def.id === "onboarding_1_tutorial" && tutorialDone) {
            progress = Math.max(progress, 1);
        }
        const completed = rec ? rec.completed : progress >= def.target;
        const claimed = rec ? rec.claimed : false;
        result.push({
            id: def.id,
            category: def.category,
            title_key: def.title_key,
            description_key: def.description_key,
            target: def.target,
            progress: progress,
            completed: completed,
            claimed: claimed,
            crowns_reward: def.crowns_reward,
            gems_reward: def.gems_reward,
            icon: def.icon,
            order: def.order,
        });
    }

    const dailies = fallbackDailyMissions(epochDay);
    for (const def of dailies) {
        const rec = records[def.id];
        const progress = rec ? rec.progress : 0;
        const completed = rec ? rec.completed : progress >= def.target;
        const claimed = rec ? rec.claimed : false;
        result.push({
            id: def.id,
            category: def.category,
            title_key: def.title_key,
            description_key: def.description_key,
            target: def.target,
            progress: progress,
            completed: completed,
            claimed: claimed,
            crowns_reward: def.crowns_reward,
            gems_reward: def.gems_reward,
            icon: def.icon,
            order: def.order,
        });
    }

    return result;
}

export function getActiveDirective(missions: MenuMission[]): MenuMission | null {
    const onboarding = missions.filter((m) => m.category === "onboarding");
    for (const m of onboarding) {
        if (!m.completed || !m.claimed) {
            return m;
        }
    }
    const daily = missions.filter((m) => m.category === "daily");
    for (const m of daily) {
        if (!m.completed || !m.claimed) {
            return m;
        }
    }
    return onboarding[onboarding.length - 1] || null;
}

function missionIconSvg(icon: string): string {
    switch (icon) {
        case "shield":
            return (
                "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
                "<path d='M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z'/>" +
                "</svg>"
            );
        case "tower":
            return (
                "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
                "<path d='M4 21V5a1 1 0 0 1 1-1h3v3h4V4h3a1 1 0 0 1 1 1v16'/><path d='M2 21h20'/><path d='M9 10h6'/><path d='M9 14h6'/><path d='M10 21v-3a2 2 0 0 1 4 0v3'/>" +
                "</svg>"
            );
        case "sword":
            return (
                "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
                "<polyline points='14.5 17.5 3 6 3 3 6 3 17.5 14.5'/><line x1='13' y1='19' x2='19' y2='13'/><line x1='16' y1='16' x2='20' y2='20'/><line x1='19' y1='21' x2='21' y2='19'/>" +
                "</svg>"
            );
        case "team":
            return (
                "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
                "<path d='M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2'/><circle cx='9' cy='7' r='4'/><path d='M23 21v-2a4 4 0 0 0-3-3.87'/><path d='M16 3.13a4 4 0 0 1 0 7.75'/>" +
                "</svg>"
            );
        case "globe":
            return (
                "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
                "<circle cx='12' cy='12' r='10'/><line x1='2' y1='12' x2='22' y2='12'/><path d='M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z'/>" +
                "</svg>"
            );
        case "crown":
            return (
                "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
                "<path d='m2 4 3 12h14l3-12-6 7-4-7-4 7-6-7zm3 16h14v2H5v-2z'/>" +
                "</svg>"
            );
        default:
            return (
                "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
                "<circle cx='12' cy='12' r='10'/><polyline points='12 6 12 12 14 14'/>" +
                "</svg>"
            );
    }
}

function videoPlayIconSvg(): string {
    return (
        "<svg class='sow-missions__video-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linejoin='round' aria-hidden='true'>" +
        "<rect x='2' y='5' width='14' height='14' rx='2'/><path d='m16 10 6-4v12l-6-4z'/>" +
        "</svg>"
    );
}

export function isRewardedAdAvailable(): boolean {
    return (
        typeof window.SOW_portalRewardedAvailable === "function" &&
        window.SOW_portalRewardedAvailable() === true &&
        typeof window.SOW_portalRewardedBreak === "function"
    );
}

export function renderDirectiveCard(): string {
    const missions = resolveActiveMissions(menuUi.state);
    const directive = getActiveDirective(missions);
    if (!directive) return "";

    const readyToClaim = directive.completed && !directive.claimed;
    const pct = Math.min(100, Math.round((Math.min(directive.progress, directive.target) / directive.target) * 100));
    const title = SOW_t(directive.title_key);
    const desc = SOW_t(directive.description_key);
    const categoryLabel =
        directive.category === "onboarding"
            ? SOW_t("missions.tab_onboarding")
            : SOW_t("missions.tab_daily");

    let actionMarkup = "";
    if (readyToClaim) {
        const canDouble = isRewardedAdAvailable();
        actionMarkup =
            "<div class='sow-directive__actions'>" +
            (canDouble
                ? "<button class='sow-directive__btn-double' type='button' data-command='claim_mission_double' data-mission-id='" +
                  esc(directive.id) +
                  "'>" +
                  videoPlayIconSvg() +
                  " " +
                  esc(SOW_t("missions.claim_doubled")) +
                  "</button>"
                : "") +
            "<button class='sow-directive__btn-claim' type='button' data-command='claim_mission' data-mission-id='" +
            esc(directive.id) +
            "'>" +
            esc(SOW_t("missions.claim")) +
            "</button>" +
            "</div>";
    } else {
        actionMarkup =
            "<div class='sow-directive__actions'>" +
            "<button class='sow-directive__btn-view' type='button' data-command='open_missions'>" +
            esc(SOW_t("missions.directive_view_all")) +
            " <span>→</span>" +
            "</button>" +
            "</div>";
    }

    return (
        "<section class='sow-directive-card" +
        (readyToClaim ? " is-ready" : "") +
        "' aria-label='" +
        esc(SOW_t("missions.directive_title")) +
        "'>" +
        "<div class='sow-directive__head'>" +
        "<div class='sow-directive__badge-group'>" +
        "<span class='sow-directive__icon' aria-hidden='true'>" +
        missionIconSvg(directive.icon) +
        "</span>" +
        "<span class='sow-directive__kicker'>" +
        esc(categoryLabel) +
        "</span>" +
        "</div>" +
        (readyToClaim
            ? "<span class='sow-directive__ready-badge'>" +
              esc(SOW_t("missions.directive_ready")) +
              "</span>"
            : "") +
        "</div>" +
        "<div class='sow-directive__body'>" +
        "<h3 class='sow-directive__title'>" +
        esc(title) +
        "</h3>" +
        "<p class='sow-directive__desc'>" +
        esc(desc) +
        "</p>" +
        "<div class='sow-directive__progress-wrap'>" +
        "<div class='sow-directive__progress-bar'>" +
        "<div class='sow-directive__progress-fill' style='width:" +
        pct +
        "%;'></div>" +
        "</div>" +
        "<span class='sow-directive__progress-count'>" +
        Math.min(directive.progress, directive.target) +
        " / " +
        directive.target +
        "</span>" +
        "</div>" +
        "<div class='sow-directive__footer'>" +
        "<div class='sow-directive__rewards'>" +
        "<span class='sow-directive__reward-pill' title='Coronas'>" +
        "<img src='" +
        esc(currencyAsset("crown")) +
        "' alt='' width='16' height='16'>+" +
        directive.crowns_reward +
        "</span>" +
        "<span class='sow-directive__reward-pill' title='Gemas'>" +
        "<img src='" +
        esc(currencyAsset("gem")) +
        "' alt='' width='16' height='16'>+" +
        directive.gems_reward +
        "</span>" +
        "</div>" +
        actionMarkup +
        "</div>" +
        "</div>" +
        "</section>"
    );
}

function formatDailyResetCountdown(): string {
    const now = new Date();
    const utcHours = now.getUTCHours();
    const utcMinutes = now.getUTCMinutes();
    const hoursRemaining = 23 - utcHours;
    const minsRemaining = 59 - utcMinutes;
    return `${hoursRemaining}h ${minsRemaining}m`;
}

export function renderMissionsModal(): string {
    const activeTab = menuUi.activeMissionsTab || "onboarding";
    const missions = resolveActiveMissions(menuUi.state);
    const currentMissions = missions.filter((m) => m.category === activeTab);
    const onboardingTotal = missions.filter((m) => m.category === "onboarding");
    const onboardingCompleted = onboardingTotal.filter((m) => m.completed || m.claimed).length;
    const dailyTotal = missions.filter((m) => m.category === "daily");
    const dailyCompleted = dailyTotal.filter((m) => m.completed || m.claimed).length;

    const subtitle =
        activeTab === "onboarding"
            ? SOW_t("missions.onboarding_subtitle")
            : SOW_t("missions.daily_subtitle");

    const adAvailable = isRewardedAdAvailable();

    const itemsMarkup = currentMissions
        .map(function (m) {
            const pct = Math.min(100, Math.round((Math.min(m.progress, m.target) / m.target) * 100));
            const readyToClaim = m.completed && !m.claimed;
            let actionHtml = "";

            if (m.claimed) {
                actionHtml =
                    "<span class='sow-missions__claimed-badge'>" +
                    "✓ " +
                    esc(SOW_t("missions.claimed")) +
                    "</span>";
            } else if (readyToClaim) {
                actionHtml =
                    "<div class='sow-missions__item-actions'>" +
                    (adAvailable
                        ? "<button class='sow-missions__btn-double' type='button' data-command='claim_mission_double' data-mission-id='" +
                          esc(m.id) +
                          "'>" +
                          videoPlayIconSvg() +
                          " " +
                          esc(SOW_t("missions.claim_doubled")) +
                          "</button>"
                        : "") +
                    "<button class='sow-missions__btn-claim' type='button' data-command='claim_mission' data-mission-id='" +
                    esc(m.id) +
                    "'>" +
                    esc(SOW_t("missions.claim")) +
                    "</button>" +
                    "</div>";
            } else {
                actionHtml =
                    "<span class='sow-missions__in-progress-badge'>" +
                    Math.min(m.progress, m.target) +
                    " / " +
                    m.target +
                    "</span>";
            }

            return (
                "<div class='sow-missions__item" +
                (m.claimed ? " is-claimed" : "") +
                (readyToClaim ? " is-ready" : "") +
                "'>" +
                "<div class='sow-missions__item-icon' aria-hidden='true'>" +
                missionIconSvg(m.icon) +
                "</div>" +
                "<div class='sow-missions__item-details'>" +
                "<div class='sow-missions__item-title-row'>" +
                "<h4 class='sow-missions__item-title'>" +
                esc(SOW_t(m.title_key)) +
                "</h4>" +
                "<div class='sow-missions__item-rewards'>" +
                "<span class='sow-missions__reward' title='Coronas'>" +
                "<img src='" +
                esc(currencyAsset("crown")) +
                "' alt='' width='16' height='16'>+" +
                m.crowns_reward +
                "</span>" +
                "<span class='sow-missions__reward' title='Gemas'>" +
                "<img src='" +
                esc(currencyAsset("gem")) +
                "' alt='' width='16' height='16'>+" +
                m.gems_reward +
                "</span>" +
                "</div>" +
                "</div>" +
                "<p class='sow-missions__item-desc'>" +
                esc(SOW_t(m.description_key)) +
                "</p>" +
                "<div class='sow-missions__item-progress-row'>" +
                "<div class='sow-missions__item-bar'>" +
                "<div class='sow-missions__item-bar-fill' style='width:" +
                pct +
                "%;'></div>" +
                "</div>" +
                "<span class='sow-missions__item-bar-text'>" +
                Math.min(m.progress, m.target) +
                " / " +
                m.target +
                "</span>" +
                "</div>" +
                "</div>" +
                "<div class='sow-missions__item-action-col'>" +
                actionHtml +
                "</div>" +
                "</div>"
            );
        })
        .join("");

    return (
        "<div class='sow-menu__overlay sow-missions-backdrop' data-menu-overlay='missions'>" +
        "<section class='sow-menu__modal sow-missions-modal' role='dialog' aria-modal='true' aria-labelledby='sow-missions-title'>" +
        "<div class='sow-menu__modal-head'>" +
        "<div class='sow-missions__modal-title-group'>" +
        "<span class='sow-missions__modal-icon' aria-hidden='true'>" +
        missionIconSvg("tower") +
        "</span>" +
        "<h2 id='sow-missions-title'>" +
        esc(SOW_t("missions.missions_modal_title")) +
        "</h2>" +
        "</div>" +
        "<button class='sow-menu__icon-button sow-missions__close' type='button' data-command='close_missions' aria-label='" +
        esc(SOW_t("menu.close")) +
        "'>×</button>" +
        "</div>" +
        "<div class='sow-missions__tabs' role='tablist'>" +
        "<button type='button' role='tab' class='sow-missions__tab" +
        (activeTab === "onboarding" ? " is-active" : "") +
        "' data-command='switch_missions_tab' data-missions-tab='onboarding' aria-selected='" +
        (activeTab === "onboarding" ? "true" : "false") +
        "'>" +
        esc(SOW_t("missions.tab_onboarding")) +
        " <small>(" +
        onboardingCompleted +
        "/" +
        onboardingTotal.length +
        ")</small>" +
        "</button>" +
        "<button type='button' role='tab' class='sow-missions__tab" +
        (activeTab === "daily" ? " is-active" : "") +
        "' data-command='switch_missions_tab' data-missions-tab='daily' aria-selected='" +
        (activeTab === "daily" ? "true" : "false") +
        "'>" +
        esc(SOW_t("missions.tab_daily")) +
        " <small>(" +
        dailyCompleted +
        "/" +
        dailyTotal.length +
        ")</small>" +
        "</button>" +
        "</div>" +
        "<div class='sow-missions__body'>" +
        "<div class='sow-missions__header-info'>" +
        "<p class='sow-missions__subtitle'>" +
        esc(subtitle) +
        "</p>" +
        (activeTab === "daily"
            ? "<div class='sow-missions__reset-notice'>" +
              "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><circle cx='12' cy='12' r='10'/><polyline points='12 6 12 12 16 14'/></svg>" +
              "<span>" +
              esc(SOW_t("missions.daily_resets_in")) +
              ": <strong>" +
              formatDailyResetCountdown() +
              "</strong></span>" +
              "</div>"
            : "") +
        "</div>" +
        "<div class='sow-missions__list'>" +
        itemsMarkup +
        "</div>" +
        "</div>" +
        "</section>" +
        "</div>"
    );
}

export function claimMission(missionId: string, doubleWithAd: boolean): void {
    if (!missionId) return;

    if (doubleWithAd && isRewardedAdAvailable()) {
        window.SOW_portalRewardedBreak!().then(function (watched: boolean) {
            if (watched) {
                send("claim_mission_reward", { mission_id: missionId, doubled_by_ad: true });
            }
        });
        return;
    }

    send("claim_mission_reward", { mission_id: missionId, doubled_by_ad: false });
}
