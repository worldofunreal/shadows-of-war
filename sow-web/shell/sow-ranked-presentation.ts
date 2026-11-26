import type { RankedLadder, RankedPointsBreakdown, RankedStep } from "./main_menu.state.ts";
import { assetUrl, esc, publicLeaderboardsUrl, tSafe } from "./sow-profile-helpers.ts";

export interface RankedLeagueSpec {
    key: string;
    tier: string;
    culture: string;
    badge: string;
    minScore: number;
    maxScore: number;
    safe: boolean;
    cultureKey: string;
    loreKey: string;
}

export interface RankedModeRatingView {
    tier: string;
    division?: string | null;
    score: number;
    peak_score?: number;
    games_played?: number;
    wins?: number;
    placements_complete?: boolean;
}

export const RANKED_LEAGUES: RankedLeagueSpec[] = [
    { key: "nomad", tier: "Nomad", culture: "unranked", badge: "nomad", minScore: 0, maxScore: 999, safe: true, cultureKey: "site.ranked_culture_unranked", loreKey: "site.ranked_lore_unranked" },
    { key: "chieftain", tier: "Chieftain", culture: "bronze", badge: "chieftain", minScore: 1000, maxScore: 1999, safe: true, cultureKey: "site.ranked_culture_bronze", loreKey: "site.ranked_lore_bronze" },
    { key: "commander", tier: "Commander", culture: "silver", badge: "commander", minScore: 2000, maxScore: 2999, safe: true, cultureKey: "site.ranked_culture_silver", loreKey: "site.ranked_lore_silver" },
    { key: "governor", tier: "Governor", culture: "silver", badge: "governor", minScore: 3000, maxScore: 3999, safe: true, cultureKey: "site.ranked_culture_silver", loreKey: "site.ranked_lore_silver" },
    { key: "king", tier: "King", culture: "gold", badge: "king", minScore: 4000, maxScore: 4999, safe: true, cultureKey: "site.ranked_culture_gold", loreKey: "site.ranked_lore_gold" },
    { key: "emperor", tier: "Emperor", culture: "gold", badge: "emperor", minScore: 5000, maxScore: 5999, safe: false, cultureKey: "site.ranked_culture_gold", loreKey: "site.ranked_lore_gold" },
    { key: "sovereign", tier: "Sovereign", culture: "platinum", badge: "sovereign", minScore: 6000, maxScore: 6999, safe: false, cultureKey: "site.ranked_culture_platinum", loreKey: "site.ranked_lore_platinum" },
    { key: "overlord", tier: "Overlord", culture: "diamond", badge: "overlord", minScore: 7000, maxScore: 7999, safe: false, cultureKey: "site.ranked_culture_diamond", loreKey: "site.ranked_lore_diamond" },
    { key: "immortal", tier: "Immortal", culture: "master", badge: "immortal", minScore: 8000, maxScore: 8999, safe: false, cultureKey: "site.ranked_culture_master", loreKey: "site.ranked_lore_master" },
    { key: "deity", tier: "Deity", culture: "grandmaster", badge: "deity", minScore: 9000, maxScore: 10000, safe: false, cultureKey: "site.ranked_culture_grandmaster", loreKey: "site.ranked_lore_grandmaster" },
];

export const DEFAULT_RANKED_LADDER: RankedLadder = {
    steps: RANKED_LEAGUES.map((league) => ({
        score: league.minScore,
        tier: league.tier,
        division: null,
    })),
    max_score: 10000,
};

const TIER_KEYS: Record<string, string> = {
    nomad: "nomad",
    chieftain: "chieftain",
    commander: "commander",
    governor: "governor",
    king: "king",
    emperor: "emperor",
    sovereign: "sovereign",
    overlord: "overlord",
    immortal: "immortal",
    deity: "deity",
    unranked: "nomad",
    bronze: "bronze",
    silver: "silver",
    gold: "gold",
    platinum: "platinum",
    diamond: "diamond",
    master: "master",
    grandmaster: "grandmaster",
};

const TIER_CULTURE_SLOT: Record<string, string> = {
    nomad: "unranked",
    chieftain: "bronze",
    commander: "silver",
    governor: "silver",
    king: "gold",
    emperor: "gold",
    sovereign: "platinum",
    overlord: "diamond",
    immortal: "master",
    deity: "grandmaster",
    bronze: "bronze",
    silver: "silver",
    gold: "gold",
    platinum: "platinum",
    diamond: "diamond",
    master: "master",
    grandmaster: "grandmaster",
    unranked: "unranked",
};

const TIER_BADGE_SLOT: Record<string, string> = {
    nomad: "nomad",
    chieftain: "chieftain",
    commander: "commander",
    governor: "governor",
    king: "king",
    emperor: "emperor",
    sovereign: "sovereign",
    overlord: "overlord",
    immortal: "immortal",
    deity: "deity",
    bronze: "chieftain",
    silver: "commander",
    gold: "king",
    platinum: "sovereign",
    diamond: "overlord",
    master: "immortal",
    grandmaster: "deity",
    unranked: "nomad",
};

const BADGE_GRID_POS: Record<string, [number, number]> = {
    nomad: [0, 0],
    chieftain: [1, 0],
    commander: [2, 0],
    governor: [3, 0],
    king: [4, 0],
    emperor: [0, 1],
    sovereign: [1, 1],
    overlord: [2, 1],
    immortal: [3, 1],
    deity: [4, 1],
};

const FALLBACK_TIER_LABELS: Record<string, string> = {
    nomad: "Nomad",
    chieftain: "Chieftain",
    commander: "Commander",
    governor: "Governor",
    king: "King",
    emperor: "Emperor",
    sovereign: "Sovereign",
    overlord: "Overlord",
    immortal: "Immortal",
    deity: "Deity",
    unranked: "Unranked",
    bronze: "Bronze",
    silver: "Silver",
    gold: "Gold",
    platinum: "Platinum",
    diamond: "Diamond",
    master: "Master",
    grandmaster: "Grandmaster",
};

export function rankTier(tier?: string | null): string {
    if (!tier) return "nomad";
    return TIER_KEYS[tier.toLowerCase()] || "nomad";
}

export function rankCulture(tier?: string | null): string {
    const key = rankTier(tier);
    return TIER_CULTURE_SLOT[key] || "unranked";
}

export function rankBadgeSlot(tier?: string | null): string {
    const key = rankTier(tier);
    return TIER_BADGE_SLOT[key] || "nomad";
}

export function rankName(tier?: string | null, division?: string | null): string {
    const key = rankTier(tier);
    const base = tSafe("ranked.tier_" + key, FALLBACK_TIER_LABELS[key] || "Nomad");
    return base + (division ? " " + division : "");
}

export function rankLabel(
    rating:
        | (Pick<RankedModeRatingView, "tier" | "division"> &
              Partial<Pick<RankedModeRatingView, "score" | "placements_complete" | "games_played">>)
        | null
        | undefined,
): string {
    const name = rankName(rating?.tier || "Nomad", rating?.division);
    const score =
        rating && typeof rating.score === "number" && Number.isFinite(rating.score)
            ? rating.score
            : 0;
    return name + " · " + score;
}

export function rankEmblem(tier?: string | null, division?: string | null): string {
    const key = rankTier(tier);
    const slot = rankBadgeSlot(key);
    const [col, row] = BADGE_GRID_POS[slot] || [0, 0];
    const xPct = ((col / 4) * 100).toFixed(4);
    const yPct = row * 100;
    return (
        "<span class='sow-ranked__emblem' data-tier='" +
        key +
        "' data-badge='" +
        slot +
        "' aria-hidden='true'><i class='sow-ranked__badge-sprite' style=\"background-image:url('" +
        esc(assetUrl("shell/ranked/badges.webp")) +
        "');background-position:" +
        xPct +
        "% " +
        yPct +
        "%\"></i>" +
        (division ? "<b>" + esc(division) + "</b>" : "") +
        "</span>"
    );
}

export function rankProgress(score: number, ladder?: RankedLadder | null) {
    const activeLadder = ladder?.steps.length ? ladder : DEFAULT_RANKED_LADDER;
    if (!Number.isFinite(score)) return null;
    const steps = activeLadder.steps;
    let index = 0;
    while (index + 1 < steps.length && score >= steps[index + 1].score) index++;
    const step = steps[index];
    const next: RankedStep | null = steps[index + 1] || null;
    const ceiling = next ? next.score : activeLadder.max_score;
    return {
        step,
        next,
        ceiling,
        remaining: Math.max(0, ceiling - score),
        fraction: Math.max(0, Math.min(1, (score - step.score) / Math.max(1, ceiling - step.score))),
    };
}

export function rankProgressMarkup(score: number, ladder?: RankedLadder | null): string {
    const progress = rankProgress(score, ladder);
    if (!progress) return "";
    const target = progress.next
        ? rankName(progress.next.tier, progress.next.division)
        : tSafe("ranked.maximum", "Peak League");
    return (
        "<div class='sow-ranked__progress'><div class='sow-ranked__progress-label'><span data-rank-target>" +
        esc(target) +
        "</span><strong data-rank-remaining>" +
        esc(Math.round(score) + " / " + progress.ceiling) +
        "</strong></div><div class='sow-ranked__track' role='progressbar' aria-label='" +
        esc(target) +
        "' aria-valuemin='0' aria-valuemax='100' aria-valuenow='" +
        Math.round(progress.fraction * 100) +
        "'><i style='transform:scaleX(" +
        progress.fraction +
        ")'></i></div></div>"
    );
}

export function rankPlacements(played: number): string {
    const count = Math.max(0, Math.min(5, played));
    return (
        "<div class='sow-ranked__placements' aria-label='" +
        esc(tSafe("ranked.placements", "Placements ({played}/5)", { played: count })) +
        "'>" +
        Array.from({ length: 5 }, (_, index) =>
            "<span class='" +
            (index < count ? "is-complete" : "") +
            "' aria-hidden='true'>" +
            (index < count ? "✓" : index + 1) +
            "</span>",
        ).join("") +
        "</div><p class='sow-ranked__note'>" +
        esc(tSafe("ranked.placements_help", "Complete 5 matches to lock in your starting league.")) +
        "</p>"
    );
}

export function rankedPointsBreakdownMarkup(points: RankedPointsBreakdown): string {
    if (points.protected_loss) {
        return "";
    }
    const components: Array<[number, string]> = [
        [points.base, "base"],
        [points.opponent, "opponent"],
        [points.league, "league"],
        [points.streak, "streak"],
        [points.protection, "protection"],
        [points.limit, "limits"],
    ];
    const visible = components.filter(([value]) => value !== 0);
    if (!visible.length) {
        return "";
    }
    return (
        "<p class='sow-ranked__breakdown'>" +
        visible
            .map(function ([value, key], idx) {
                const tone =
                    key === "streak" && value > 0
                        ? "is-streak"
                        : value > 0
                          ? "is-positive"
                          : "is-negative";
                return (
                    "<span class='sow-ranked__chip " +
                    tone +
                    "' style='--chip-idx:" +
                    idx +
                    "'>" +
                    esc((value > 0 ? "+" : "") + value + " " + tSafe("lobbies.rank_breakdown_" + key, key)) +
                    "</span>"
                );
            })
            .join(" ") +
        "</p>"
    );
}

export interface RankedOverviewOptions {
    rating1v1: RankedModeRatingView | null | undefined;
    ratingFfa: RankedModeRatingView | null | undefined;
    ladder?: RankedLadder | null;
    own?: boolean;
    mode?: "game" | "site";
    activeQueue?: "Ranked1v1" | "RankedFFA";
}

/** Renders the dual-mode Ranked Card keeping 1v1 & FFA ladders distinct while sharing the 10-league progression. */
export function renderRankedOverview(options: RankedOverviewOptions): string {
    const activeQueue: "Ranked1v1" | "RankedFFA" =
        options.activeQueue === "RankedFFA" ? "RankedFFA" : "Ranked1v1";
    const otherQueue: "Ranked1v1" | "RankedFFA" =
        activeQueue === "Ranked1v1" ? "RankedFFA" : "Ranked1v1";
    const r1v1 = options.rating1v1 || null;
    const rFfa = options.ratingFfa || null;
    const primary = activeQueue === "RankedFFA" ? rFfa : r1v1;
    const secondary = activeQueue === "RankedFFA" ? r1v1 : rFfa;

    const activeTier = primary?.tier || "Nomad";
    const activeDivision = primary?.division || null;
    const activeScore = primary && Number.isFinite(primary.score) ? primary.score : 0;
    const activeHeroTier = rankTier(activeTier);

    const secTier = secondary?.tier || "Nomad";
    const secDivision = secondary?.division || null;
    const secScore = secondary && Number.isFinite(secondary.score) ? secondary.score : 0;

    const effectiveLadder = options.ladder?.steps.length ? options.ladder : DEFAULT_RANKED_LADDER;
    const tiers = effectiveLadder.steps.filter(
        (step, index, steps) => !index || step.tier !== steps[index - 1].tier,
    );

    const label1v1 = tSafe("lobbies.ranked_1v1", "Ranked 1v1");
    const labelFfa = tSafe("lobbies.ranked_ffa", "Ranked FFA");
    const gamesPlayed = Number(primary?.games_played || 0);
    const wins = Number(primary?.wins || 0);
    const winRate = gamesPlayed > 0 ? Math.round((wins / gamesPlayed) * 100) : 0;
    const peakScore = Math.max(activeScore, Number(primary?.peak_score || 0));

    const statsPills =
        "<div class='sow-ranked__stats-row'>" +
        "<span class='sow-ranked__stat-pill'><span>" +
        esc(tSafe("profile.peak_score", "Peak")) +
        "</span><b>" +
        esc(peakScore) +
        "</b></span>" +
        "<span class='sow-ranked__stat-pill'><span>" +
        esc(tSafe("profile.wins", "Wins")) +
        "</span><b>" +
        esc(wins + " / " + gamesPlayed + " (" + winRate + "%)") +
        "</b></span>" +
        "</div>";

    const placementOrProgress =
        primary && primary.placements_complete === false
            ? rankPlacements(gamesPlayed) + rankProgressMarkup(activeScore, effectiveLadder)
            : rankProgressMarkup(activeScore, effectiveLadder);

    const actionsMarkup =
        options.mode === "site"
            ? "<div class='sow-ranked__actions'><a class='sow-menu__secondary' href='" +
              esc(publicLeaderboardsUrl() + "?board=" + activeQueue) +
              "'>" +
              esc(tSafe("ranked.view_board", "View Leaderboard")) +
              "</a></div>"
            : "<div class='sow-ranked__actions'>" +
              (options.own
                  ? "<button type='button' class='sow-menu__primary' data-command='quick_match'>" +
                    esc(label1v1) +
                    "</button><button type='button' class='sow-menu__primary' data-command='quick_match_ffa'>" +
                    esc(labelFfa) +
                    "</button>"
                  : "") +
              "<button type='button' class='sow-menu__secondary' data-command='profile_tab' data-profile-tab='victories'>" +
              esc(tSafe("ranked.view_board", "View Leaderboard")) +
              "</button></div>";

    return (
        "<section class='sow-ranked__overview' data-ranked-overview data-tier='" +
        activeHeroTier +
        "' aria-label='" +
        esc(tSafe("ranked.my_rank", "Ranked")) +
        "'>" +
        "<div class='sow-ranked__mode-tabs' role='group' aria-label='" +
        esc(tSafe("profile.ranked_modes", "Ranked Modes")) +
        "'>" +
        "<button type='button' class='sow-ranked__mode-tab' data-command='ranked_overview_mode' data-ranked-overview-mode='Ranked1v1' aria-pressed='" +
        (activeQueue === "Ranked1v1" ? "true" : "false") +
        "'><strong>" +
        esc(label1v1) +
        "</strong><small>" +
        esc(rankLabel(r1v1)) +
        "</small></button>" +
        "<button type='button' class='sow-ranked__mode-tab' data-command='ranked_overview_mode' data-ranked-overview-mode='RankedFFA' aria-pressed='" +
        (activeQueue === "RankedFFA" ? "true" : "false") +
        "'><strong>" +
        esc(labelFfa) +
        "</strong><small>" +
        esc(rankLabel(rFfa)) +
        "</small></button>" +
        "</div>" +
        "<div class='sow-ranked__hero'>" +
        "<div class='sow-ranked__hero-emblem-wrap'>" +
        rankEmblem(activeTier, activeDivision) +
        "</div>" +
        "<div class='sow-ranked__summary'><span class='sow-ranked__eyebrow'>" +
        esc(activeQueue === "RankedFFA" ? labelFfa : label1v1) +
        "</span><h2>" +
        esc(rankName(activeTier, activeDivision)) +
        "</h2><p class='sow-ranked__points'>" +
        esc(tSafe("lobbies.rank_score", "{score} pts", { score: activeScore })) +
        "</p>" +
        statsPills +
        placementOrProgress +
        "</div></div>" +
        "<button type='button' class='sow-ranked__secondary-mode' data-command='ranked_overview_mode' data-ranked-overview-mode='" +
        otherQueue +
        "' data-tier='" +
        rankTier(secTier) +
        "'>" +
        "<div class='sow-ranked__secondary-mode-main'>" +
        rankEmblem(secTier, secDivision) +
        "<div class='sow-ranked__secondary-mode-copy'><span class='sow-ranked__eyebrow'>" +
        esc(otherQueue === "RankedFFA" ? labelFfa : label1v1) +
        "</span><strong>" +
        esc(rankName(secTier, secDivision)) +
        "</strong><small>" +
        esc(tSafe("lobbies.rank_score", "{score} pts", { score: secScore })) +
        "</small></div></div>" +
        "<span class='sow-profile__section-note'>" +
        esc(tSafe("profile.switch_mode", "Inspect")) +
        " ›</span></button>" +
        (tiers.length
            ? "<p class='sow-ranked__shared-note'>" +
              esc(
                  tSafe(
                      "profile.shared_leagues_hint",
                      "Separate ladders, same ten leagues (0 – 10,000 pts).",
                  ),
              ) +
              "</p><ol class='sow-ranked__leagues'>" +
              tiers
                  .map(
                      (step) =>
                          "<li" +
                          (rankTier(step.tier) === activeHeroTier
                              ? " class='is-current' aria-current='step'"
                              : "") +
                          ">" +
                          rankEmblem(step.tier) +
                          "<strong>" +
                          esc(rankName(step.tier)) +
                          "</strong><small>" +
                          esc(step.score) +
                          "</small></li>",
                  )
                  .join("") +
              "</ol>"
            : "") +
        actionsMarkup +
        "</section>"
    );
}
