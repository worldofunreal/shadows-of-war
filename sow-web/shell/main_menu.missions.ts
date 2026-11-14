import { currencyAsset, esc, menuUi, send } from "./main_menu.core.ts";
import { render } from "./main_menu.shell.ts";
import type { MenuMission, MenuState } from "./main_menu.state.ts";

// Missions and Retention Funnel runtime logic and presentation.

export function resolveActiveMissions(state: MenuState | null): MenuMission[] {
    return state && Array.isArray(state.missions) ? state.missions : [];
}

export function getActiveDirective(missions: MenuMission[]): MenuMission | null {
    // 1. Any ready-to-claim unlocked mission across all categories (onboarding, daily, weekly)
    const unclaimedOnboarding = missions.find((m) => !m.locked && m.category === "onboarding" && m.completed && !m.claimed);
    if (unclaimedOnboarding) return unclaimedOnboarding;
    const unclaimedDaily = missions.find((m) => !m.locked && m.category === "daily" && m.completed && !m.claimed);
    if (unclaimedDaily) return unclaimedDaily;
    const unclaimedWeekly = missions.find((m) => !m.locked && m.category === "weekly" && m.completed && !m.claimed);
    if (unclaimedWeekly) return unclaimedWeekly;

    // 2. Unlocked in-progress onboarding
    const inProgressOnboarding = missions.find((m) => !m.locked && m.category === "onboarding" && !m.completed);
    if (inProgressOnboarding) return inProgressOnboarding;

    // 3. Unlocked in-progress daily
    const inProgressDaily = missions.find((m) => !m.locked && m.category === "daily" && !m.completed);
    if (inProgressDaily) return inProgressDaily;

    // 4. Unlocked in-progress weekly
    const inProgressWeekly = missions.find((m) => !m.locked && m.category === "weekly" && !m.completed);
    if (inProgressWeekly) return inProgressWeekly;

    return null;
}

export function missionIconSvg(icon: string): string {
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
        case "swords":
            return (
                "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
                "<path d='M14.5 17.5 3 6 3 3 6 3 17.5 14.5M13 19l6-6M16 16l4 4M19 21l2-2'/><path d='M9.5 17.5 21 6 21 3 18 3 6.5 14.5M11 19l-6-6M8 16l-4 4M5 21l-2-2'/>" +
                "</svg>"
            );
        case "rotation":
            return (
                "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
                "<path d='M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8'/><path d='M3 3v5h5'/><path d='M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16'/><path d='M16 16h5v5'/>" +
                "</svg>"
            );
        case "crosshair":
            return (
                "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
                "<circle cx='12' cy='12' r='8'/><line x1='12' y1='2' x2='12' y2='6'/><line x1='12' y1='18' x2='12' y2='22'/><line x1='2' y1='12' x2='6' y2='12'/><line x1='18' y1='12' x2='22' y2='12'/><circle cx='12' cy='12' r='2'/>" +
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

export function videoPlayIconSvg(): string {
    return (
        "<svg class='sow-missions__video-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linejoin='round' aria-hidden='true'>" +
        "<rect x='2' y='5' width='14' height='14' rx='2'/><path d='m16 10 6-4v12l-6-4z'/>" +
        "</svg>"
    );
}

export function swapIconSvg(): string {
    return (
        "<svg class='sow-missions__swap-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
        "<path d='m16 3 4 4-4 4'/><path d='M20 7H4'/><path d='m8 21-4-4 4-4'/><path d='M4 17h16'/>" +
        "</svg>"
    );
}

export function arrowRightIconSvg(): string {
    return (
        "<svg class='sow-missions__go-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2.25' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
        "<path d='M5 12h14'/><path d='m13 6 6 6-6 6'/>" +
        "</svg>"
    );
}

export function lockIconSvg(size = 15): string {
    return (
        "<svg width='" +
        size +
        "' height='" +
        size +
        "' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2.1' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
        "<rect x='5' y='11' width='14' height='10' rx='2'/>" +
        "<path d='M8 11V7a4 4 0 0 1 8 0v4'/>" +
        "</svg>"
    );
}

function missionActionTooltipAttrs(actionTarget: string | undefined, missionId?: string): string {
    if (!actionTarget) return "";
    if (missionId === "onboarding_1_tutorial") {
        return (
            " data-sow-tooltip='" +
            esc(SOW_t("menu.campaign")) +
            "' data-sow-tooltip-icon='play' data-sow-tooltip-tone='gold'"
        );
    }
    const labels: Record<string, { text: string; icon: string; tone: string }> = {
        battle: { text: SOW_t("lobbies.matchmaking"), icon: "play", tone: "gold" },
        ranked: { text: SOW_t("lobbies.ranked_1v1"), icon: "ranked", tone: "gold" },
        teams: { text: SOW_t("lobbies.teams_two"), icon: "play", tone: "gold" },
        hnv: { text: SOW_t("lobbies.humans_vs_nations"), icon: "play", tone: "gold" },
        ffa: { text: SOW_t("lobbies.free_for_all"), icon: "play", tone: "gold" },
    };
    const meta = labels[actionTarget] || {
        text: SOW_t("menu.heroes"),
        icon: "info",
        tone: "gold",
    };
    return (
        " data-sow-tooltip='" +
        esc(meta.text) +
        "' data-sow-tooltip-icon='" +
        meta.icon +
        "' data-sow-tooltip-tone='" +
        meta.tone +
        "'"
    );
}

function renderRewardPills(
    crowns: number,
    gems: number,
    baseClass: string,
    gemClass: string,
    iconPx: number,
): string {
    let html = "";
    if (crowns > 0) {
        html +=
            "<span class='" +
            baseClass +
            "'><img src='" +
            esc(currencyAsset("crown")) +
            "' alt='' width='" +
            iconPx +
            "' height='" +
            iconPx +
            "'>+" +
            crowns +
            "</span>";
    }
    if (gems > 0) {
        html +=
            "<span class='" +
            baseClass +
            " " +
            gemClass +
            "'><img src='" +
            esc(currencyAsset("gem")) +
            "' alt='' width='" +
            iconPx +
            "' height='" +
            iconPx +
            "'>+" +
            gems +
            "</span>";
    }
    return html;
}

// Ad grants left today, as the server counts them. Zero hides every rewarded
// offer, so no video plays that the server would then refuse.
export function adGrantsLeft(): number {
    return Number(menuUi.state?.ad_grants_left) || 0;
}

export function isRewardedAdAvailable(): boolean {
    return (
        typeof window.SOW_portalRewardedAvailable === "function" &&
        window.SOW_portalRewardedAvailable() === true &&
        typeof window.SOW_portalRewardedBreak === "function" &&
        adGrantsLeft() > 0
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
            arrowRightIconSvg() +
            "</button>" +
            "</div>";
    }

    const rewardsHtml = renderRewardPills(
        directive.crowns_reward,
        directive.gems_reward,
        "sow-directive__reward-pill",
        "sow-directive__reward-pill--gem",
        20,
    );

    return (
        "<section class='sow-directive-card" +
        (readyToClaim ? " is-ready" : "") +
        "' data-command='open_missions' role='button' tabindex='0' aria-label='" +
        esc(SOW_t("missions.directive_title")) +
        "'>" +
        "<div class='sow-directive__head'>" +
        "<div class='sow-directive__title-row'>" +
        "<span class='sow-directive__icon' aria-hidden='true'>" +
        missionIconSvg(directive.icon) +
        "</span>" +
        "<h3 class='sow-directive__title'>" +
        esc(title) +
        "</h3>" +
        "</div>" +
        (readyToClaim
            ? "<span class='sow-directive__ready-badge'>" +
              esc(SOW_t("missions.directive_ready")) +
              "</span>"
            : "") +
        "</div>" +
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
        rewardsHtml +
        "</div>" +
        actionMarkup +
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

function formatWeeklyResetCountdown(): string {
    const nowSecs = Math.floor(Date.now() / 1000);
    const offset = 144000; // server ROTATION_RESET_OFFSET_SECS: rotations reset Tuesday 08:00 UTC
    const period = 604800;
    const currentPeriod = Math.floor((nowSecs + offset) / period);
    const nextResetSecs = (currentPeriod + 1) * period - offset;
    const diffSecs = Math.max(0, nextResetSecs - nowSecs);
    const days = Math.floor(diffSecs / 86400);
    const hours = Math.floor((diffSecs % 86400) / 3600);
    const mins = Math.floor((diffSecs % 3600) / 60);
    if (days > 0) {
        return `${days}d ${hours}h`;
    }
    return `${hours}h ${mins}m`;
}

export function renderMissionsModal(): string {
    const activeTab = menuUi.activeMissionsTab || "onboarding";
    const missions = resolveActiveMissions(menuUi.state);
    const currentMissions = missions.filter((m) => m.category === activeTab);
    const onboardingTotal = missions.filter((m) => m.category === "onboarding");
    const onboardingCompleted = onboardingTotal.filter((m) => m.completed || m.claimed).length;
    const dailyTotal = missions.filter((m) => m.category === "daily");
    const dailyCompleted = dailyTotal.filter((m) => m.completed || m.claimed).length;
    const dailyLocked = dailyTotal.length > 0 && dailyTotal.every((m) => Boolean(m.locked));
    const dailyUnlockLevel = dailyTotal[0]?.unlock_level || 3;
    const weeklyTotal = missions.filter((m) => m.category === "weekly");
    const weeklyCompleted = weeklyTotal.filter((m) => m.completed || m.claimed).length;
    const weeklyLocked = weeklyTotal.length > 0 && weeklyTotal.every((m) => Boolean(m.locked));
    const weeklyUnlockLevel = weeklyTotal[0]?.unlock_level || 5;

    let subtitle = SOW_t("missions.onboarding_subtitle");
    if (activeTab === "daily") {
        subtitle = SOW_t("missions.daily_subtitle");
    } else if (activeTab === "weekly") {
        subtitle = SOW_t("missions.weekly_subtitle");
    }

    const adAvailable = isRewardedAdAvailable();
    const state = menuUi.state;
    const isPeriodTab = activeTab === "daily" || activeTab === "weekly";
    const freeSwapsLeft =
        activeTab === "daily"
            ? Math.max(0, Number(state?.daily_free_swaps_left ?? 1 - Number(state?.daily_swaps_used || 0)))
            : activeTab === "weekly"
              ? Math.max(
                    0,
                    Number(state?.weekly_free_swaps_left ?? 1 - Number(state?.weekly_swaps_used || 0)),
                )
              : 0;
    const totalSwapsLeft =
        activeTab === "daily"
            ? Math.max(0, Number(state?.daily_total_swaps_left ?? 2 - Number(state?.daily_swaps_used || 0)))
            : activeTab === "weekly"
              ? Math.max(
                    0,
                    Number(state?.weekly_total_swaps_left ?? 2 - Number(state?.weekly_swaps_used || 0)),
                )
              : 0;
    const periodLocked =
        (activeTab === "daily" && dailyLocked) || (activeTab === "weekly" && weeklyLocked);

    const lockBanner =
        activeTab === "daily" && dailyLocked
            ? "<div class='sow-missions__locked-banner'>" +
              esc(SOW_t("missions.unlock_at_level").replace("{level}", String(dailyUnlockLevel))) +
              "</div>"
            : activeTab === "weekly" && weeklyLocked
              ? "<div class='sow-missions__locked-banner'>" +
                esc(SOW_t("missions.unlock_at_level").replace("{level}", String(weeklyUnlockLevel))) +
                "</div>"
              : "";

    const items = currentMissions.slice().sort(function (a, b) {
        return a.order - b.order;
    });

    const itemsMarkup = !items.length
        ? "<p class='sow-missions__empty'>" + esc(SOW_t("missions.empty_tab")) + "</p>"
        : items
              .map(function (m) {
                  const isLocked = Boolean(m.locked);
                  const pct = isLocked
                      ? 0
                      : Math.min(100, Math.round((Math.min(m.progress, m.target) / m.target) * 100));
                  const readyToClaim = !isLocked && m.completed && !m.claimed;
                  const isConfirmingSwap = menuUi.swapConfirmMissionId === m.id;
                  let actionHtml = "";

                  if (isLocked) {
                      const lockLabel =
                          m.category === "onboarding"
                              ? SOW_t("missions.locked_prev_step")
                              : SOW_t("missions.unlock_at_level").replace(
                                    "{level}",
                                    String(m.unlock_level || 3),
                                );
                      actionHtml =
                          "<span class='sow-missions__locked-badge' tabindex='0' role='img' aria-label='" +
                          esc(lockLabel) +
                          "' data-sow-tooltip='" +
                          esc(lockLabel) +
                          "' data-sow-tooltip-icon='lock' data-sow-tooltip-tone='muted' data-sow-tooltip-placement='start' data-sow-tooltip-tap='true'>" +
                          lockIconSvg(15) +
                          "</span>";
                  } else if (m.claimed) {
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
                      const goBtn = m.action_target
                          ? "<button class='sow-missions__btn-go' type='button' data-command='mission_action' data-action-target='" +
                            esc(m.action_target) +
                            "' data-mission-id='" +
                            esc(m.id) +
                            "'" +
                            missionActionTooltipAttrs(m.action_target, m.id) +
                            ">" +
                            esc(SOW_t("missions.action_go")) +
                            arrowRightIconSvg() +
                            "</button>"
                          : "";
                      let swapBtn = "";
                      if (m.swappable) {
                          const quotaLabel =
                              totalSwapsLeft > 0
                                  ? SOW_t("missions.swaps_left", { count: totalSwapsLeft })
                                  : SOW_t("missions.swap_limit_reached");
                          if (freeSwapsLeft > 0) {
                              swapBtn =
                                  "<button class='sow-missions__btn-swap' type='button' data-command='request_swap_mission' data-mission-id='" +
                                  esc(m.id) +
                                  "' data-ad='false' data-sow-tooltip='" +
                                  esc(quotaLabel) +
                                  "' data-sow-tooltip-title='" +
                                  esc(SOW_t("missions.swap_free")) +
                                  "' data-sow-tooltip-icon='swap' data-sow-tooltip-tone='gold'>" +
                                  swapIconSvg() +
                                  "<span>" +
                                  esc(SOW_t("missions.swap_free")) +
                                  "</span></button>";
                          } else if (totalSwapsLeft > 0 && adAvailable) {
                              swapBtn =
                                  "<button class='sow-missions__btn-swap is-ad' type='button' data-command='request_swap_mission' data-mission-id='" +
                                  esc(m.id) +
                                  "' data-ad='true' data-sow-tooltip='" +
                                  esc(quotaLabel) +
                                  "' data-sow-tooltip-title='" +
                                  esc(SOW_t("missions.swap_ad")) +
                                  "' data-sow-tooltip-icon='swap' data-sow-tooltip-tone='gem'>" +
                                  videoPlayIconSvg() +
                                  "<span>" +
                                  esc(SOW_t("missions.swap_ad")) +
                                  "</span></button>";
                          }
                      }
                      actionHtml =
                          "<div class='sow-missions__item-actions'>" + swapBtn + goBtn + "</div>";
                  }

                  const itemRewardsHtml = renderRewardPills(
                      m.crowns_reward,
                      m.gems_reward,
                      "sow-missions__reward",
                      "sow-missions__reward--gem",
                      22,
                  );

                  const swapConfirmHtml = isConfirmingSwap
                      ? "<div class='sow-missions__swap-confirm' role='alertdialog' aria-label='" +
                        esc(SOW_t("missions.swap_confirm_title")) +
                        "'>" +
                        "<div class='sow-missions__swap-confirm-copy'><strong>" +
                        esc(SOW_t("missions.swap_confirm_title")) +
                        "</strong><span>" +
                        esc(
                            SOW_t("missions.swap_confirm_body", {
                                progress: Math.min(m.progress, m.target),
                                target: m.target,
                            }),
                        ) +
                        "</span></div>" +
                        "<div class='sow-missions__swap-confirm-actions'>" +
                        "<button type='button' class='sow-missions__swap-keep' data-command='cancel_swap_mission'>" +
                        esc(SOW_t("missions.swap_keep")) +
                        "</button>" +
                        "<button type='button' class='sow-missions__swap-confirm-btn' data-command='confirm_swap_mission' data-mission-id='" +
                        esc(m.id) +
                        "' data-ad='" +
                        Boolean(menuUi.swapConfirmAd) +
                        "'>" +
                        esc(SOW_t("missions.swap_confirm_btn")) +
                        "</button>" +
                        "</div></div>"
                      : "";

                  return (
                      "<div class='sow-missions__item" +
                      (isLocked ? " is-locked" : "") +
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
                      itemRewardsHtml +
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
                      swapConfirmHtml +
                      "</div>" +
                      "<div class='sow-missions__item-action-col'>" +
                      actionHtml +
                      "</div>" +
                      "</div>"
                  );
              })
              .join("");

    const swapQuotaHtml =
        isPeriodTab && !periodLocked
            ? "<span class='sow-missions__swap-quota'>" +
              swapIconSvg() +
              "<span>" +
              esc(
                  totalSwapsLeft > 0
                      ? SOW_t("missions.swaps_left", { count: totalSwapsLeft })
                      : SOW_t("missions.swap_limit_reached"),
              ) +
              "</span></span>"
            : "";

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
        "<span>" +
        esc(SOW_t("missions.tab_onboarding")) +
        "</span>" +
        "<span class='sow-missions__tab-badge'>" +
        onboardingCompleted +
        "/" +
        onboardingTotal.length +
        "</span>" +
        "</button>" +
        "<button type='button' role='tab' class='sow-missions__tab" +
        (activeTab === "daily" ? " is-active" : "") +
        (dailyLocked ? " is-locked" : "") +
        "' data-command='switch_missions_tab' data-missions-tab='daily' aria-selected='" +
        (activeTab === "daily" ? "true" : "false") +
        "'" +
        (dailyLocked
            ? " data-sow-tooltip='" +
              esc(SOW_t("missions.unlock_at_level").replace("{level}", String(dailyUnlockLevel))) +
              "' data-sow-tooltip-icon='lock' data-sow-tooltip-tone='gold' data-sow-tooltip-placement='bottom'"
            : "") +
        ">" +
        "<span>" +
        esc(SOW_t("missions.tab_daily")) +
        "</span>" +
        (dailyLocked
            ? "<span class='sow-missions__tab-badge is-locked'>" +
              lockIconSvg(11) +
              "<span>Lv " +
              dailyUnlockLevel +
              "</span></span>"
            : "<span class='sow-missions__tab-badge'>" +
              dailyCompleted +
              "/" +
              dailyTotal.length +
              "</span>") +
        "</button>" +
        "<button type='button' role='tab' class='sow-missions__tab" +
        (activeTab === "weekly" ? " is-active" : "") +
        (weeklyLocked ? " is-locked" : "") +
        "' data-command='switch_missions_tab' data-missions-tab='weekly' aria-selected='" +
        (activeTab === "weekly" ? "true" : "false") +
        "'" +
        (weeklyLocked
            ? " data-sow-tooltip='" +
              esc(SOW_t("missions.unlock_at_level").replace("{level}", String(weeklyUnlockLevel))) +
              "' data-sow-tooltip-icon='lock' data-sow-tooltip-tone='gold' data-sow-tooltip-placement='bottom'"
            : "") +
        ">" +
        "<span>" +
        esc(SOW_t("missions.tab_weekly")) +
        "</span>" +
        (weeklyLocked
            ? "<span class='sow-missions__tab-badge is-locked'>" +
              lockIconSvg(11) +
              "<span>Lv " +
              weeklyUnlockLevel +
              "</span></span>"
            : "<span class='sow-missions__tab-badge'>" +
              weeklyCompleted +
              "/" +
              weeklyTotal.length +
              "</span>") +
        "</button>" +
        "</div>" +
        "<div class='sow-missions__body'>" +
        "<div class='sow-missions__header-info'>" +
        "<p class='sow-missions__subtitle'>" +
        esc(subtitle) +
        "</p>" +
        (activeTab === "daily"
            ? "<div class='sow-missions__reset-group'>" +
              swapQuotaHtml +
              "<div class='sow-missions__reset-notice'>" +
              "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><circle cx='12' cy='12' r='10'/><polyline points='12 6 12 12 16 14'/></svg>" +
              "<span>" +
              esc(SOW_t("missions.daily_resets_in")) +
              ": <strong>" +
              formatDailyResetCountdown() +
              "</strong></span>" +
              "</div></div>"
            : "") +
        (activeTab === "weekly"
            ? "<div class='sow-missions__reset-group'>" +
              swapQuotaHtml +
              "<div class='sow-missions__reset-notice'>" +
              "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><circle cx='12' cy='12' r='10'/><polyline points='12 6 12 12 16 14'/></svg>" +
              "<span>" +
              esc(SOW_t("missions.weekly_resets_in")) +
              ": <strong>" +
              formatWeeklyResetCountdown() +
              "</strong></span>" +
              "</div></div>"
            : "") +
        "</div>" +
        lockBanner +
        "<div class='sow-missions__list'>" +
        itemsMarkup +
        "</div>" +
        "</div>" +
        "</section>" +
        "</div>"
    );
}

export type MissionClaimHandler = (mission: MenuMission, doubled: boolean) => void;

let missionClaimHandler: MissionClaimHandler | null = null;

export function setMissionClaimHandler(handler: MissionClaimHandler | null): void {
    missionClaimHandler = handler;
}

export function claimMission(missionId: string, doubleWithAd: boolean): void {
    if (!missionId) return;

    const missions = resolveActiveMissions(menuUi.state);
    const mission = missions.find((m) => m.id === missionId);

    const markClaimed = () => {
        const state = menuUi.state;
        if (state) {
            if (Array.isArray(state.missions)) {
                const m = state.missions.find((x) => x.id === missionId);
                if (m) m.claimed = true;
            }
            if (!state.missions_progress) {
                state.missions_progress = {};
            }
            if (!state.missions_progress[missionId]) {
                state.missions_progress[missionId] = {
                    id: missionId,
                    progress: mission ? mission.target : 1,
                    completed: true,
                    claimed: true,
                };
            } else {
                state.missions_progress[missionId].claimed = true;
                state.missions_progress[missionId].completed = true;
            }
        }
        if (mission) {
            mission.claimed = true;
        }
    };

    if (doubleWithAd && isRewardedAdAvailable()) {
        window.SOW_portalRewardedBreak!().then(function (watched: boolean) {
            if (watched) {
                markClaimed();
                send("claim_mission_reward", { mission_id: missionId, doubled_by_ad: true });
                if (mission && missionClaimHandler) {
                    missionClaimHandler(mission, true);
                }
            }
        });
        return;
    }

    markClaimed();
    send("claim_mission_reward", { mission_id: missionId, doubled_by_ad: false });
    if (mission && missionClaimHandler) {
        missionClaimHandler(mission, false);
    }
}

function trackMissionSwap(mission: MenuMission | undefined, adFunded: boolean): void {
    if (typeof window.SOW_trackExperienceEvent !== "function") return;
    window.SOW_trackExperienceEvent({
        name: "mission_swap",
        props: {
            mission_id: mission?.id || "",
            tier: mission?.category || "",
            used_ad: adFunded,
        },
    });
}

function performSwapMission(missionId: string, adFunded: boolean): void {
    if (!missionId) return;
    const missions = resolveActiveMissions(menuUi.state);
    const mission = missions.find((m) => m.id === missionId);
    if (adFunded && isRewardedAdAvailable()) {
        window.SOW_portalRewardedBreak!().then(function (watched: boolean) {
            if (watched) {
                trackMissionSwap(mission, true);
                send("swap_mission", { mission_id: missionId, ad_funded: true });
            }
        });
        return;
    }
    trackMissionSwap(mission, false);
    send("swap_mission", { mission_id: missionId, ad_funded: false });
}

export function requestSwapMission(missionId: string, adFunded: boolean): void {
    if (!missionId) return;
    const missions = resolveActiveMissions(menuUi.state);
    const mission = missions.find((m) => m.id === missionId);
    if (!mission || !mission.swappable || mission.completed || mission.claimed || mission.locked) return;
    if (mission.progress > 0) {
        menuUi.swapConfirmMissionId = missionId;
        menuUi.swapConfirmAd = adFunded;
        render();
        return;
    }
    menuUi.swapConfirmMissionId = null;
    performSwapMission(missionId, adFunded);
}

export function confirmSwapMission(missionId: string, adFunded: boolean): void {
    menuUi.swapConfirmMissionId = null;
    menuUi.swapConfirmAd = false;
    performSwapMission(missionId, adFunded);
    render();
}

export function cancelSwapMission(): void {
    if (!menuUi.swapConfirmMissionId) return;
    menuUi.swapConfirmMissionId = null;
    menuUi.swapConfirmAd = false;
    render();
}
