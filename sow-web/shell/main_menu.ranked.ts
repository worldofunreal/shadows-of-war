import { asset, avatarImage, esc, formatMapName, lobbyThumb, menuUi } from "./main_menu.core.ts";
import { avatarImageAttribute } from "./avatar-presentation.ts";
import type { MenuLobby, RankedLadder, RankedPointsBreakdown, RankedStep } from "./main_menu.state.ts";
import type { ProfileRating } from "./main_menu.profile.ts";

// Shared presentation for queue, loader, profile and the confirmed match result.
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
    bronze: "bronze",
    silver: "silver",
    gold: "gold",
    platinum: "platinum",
    diamond: "diamond",
    master: "master",
    grandmaster: "grandmaster",
};

const TIER_EMBLEM_SVG: Record<string, string> = {
    nomad: "bronze",
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
    unranked: "bronze",
};

export function rankTier(tier?: string | null): string {
    if (!tier) return "nomad";
    return TIER_KEYS[tier.toLowerCase()] || "nomad";
}

export function rankName(tier?: string | null, division?: string | null): string {
    return SOW_t("ranked.tier_" + rankTier(tier)) + (division ? " " + division : "");
}

export function rankLabel(
    rating:
        | (Pick<ProfileRating, "tier" | "division"> &
              Partial<Pick<ProfileRating, "score" | "placements_complete" | "games_played">>)
        | null,
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
    const svg = TIER_EMBLEM_SVG[key] || "bronze";
    return "<span class='sow-ranked__emblem' data-tier='" + key + "' aria-hidden='true'><img src='" +
        esc(asset("shell/ranked/" + svg + ".svg")) + "' alt='' width='160' height='160'>" +
        (division ? "<b>" + esc(division) + "</b>" : "") + "</span>";
}

export function rankProgress(score: number, ladder?: RankedLadder | null) {
    if (!ladder?.steps.length || !Number.isFinite(score)) return null;
    const steps = ladder.steps;
    let index = 0;
    while (index + 1 < steps.length && score >= steps[index + 1].score) index++;
    const step = steps[index];
    const next: RankedStep | null = steps[index + 1] || null;
    const ceiling = next ? next.score : ladder.max_score;
    return { step, next, ceiling, remaining: Math.max(0, ceiling - score),
        fraction: Math.max(0, Math.min(1, (score - step.score) / Math.max(1, ceiling - step.score))) };
}

export function rankProgressMarkup(score: number, ladder?: RankedLadder | null): string {
    const progress = rankProgress(score, ladder);
    if (!progress) return "";
    const target = progress.next ? rankName(progress.next.tier, progress.next.division) : SOW_t("ranked.maximum");
    return "<div class='sow-ranked__progress'><div class='sow-ranked__progress-label'><span data-rank-target>" +
        esc(target) + "</span><strong data-rank-remaining>" + esc(Math.round(score) + " / " + progress.ceiling) +
        "</strong></div><div class='sow-ranked__track' role='progressbar' aria-label='" + esc(target) +
        "' aria-valuemin='0' aria-valuemax='100' aria-valuenow='" + Math.round(progress.fraction * 100) +
        "'><i style='transform:scaleX(" + progress.fraction + ")'></i></div></div>";
}

export function rankPlacements(played: number): string {
    const count = Math.max(0, Math.min(5, played));
    return "<div class='sow-ranked__placements' aria-label='" + esc(SOW_t("ranked.placements", { played: count })) +
        "'>" + Array.from({ length: 5 }, (_, index) => "<span class='" + (index < count ? "is-complete" : "") +
            "' aria-hidden='true'>" + (index < count ? "✓" : index + 1) + "</span>").join("") +
        "</div><p class='sow-ranked__note'>" + esc(SOW_t("ranked.placements_help")) + "</p>";
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
                    esc((value > 0 ? "+" : "") + value + " " + SOW_t("lobbies.rank_breakdown_" + key)) +
                    "</span>"
                );
            })
            .join(" ") +
        "</p>"
    );
}

export function rankOverview(rating: ProfileRating | null, ladder: RankedLadder | null | undefined, own: boolean): string {
    const currentTier = rating?.tier || "Nomad";
    const currentDivision = rating?.division || null;
    const currentScore = rating && Number.isFinite(rating.score) ? rating.score : 0;
    const heroTier = rankTier(currentTier);
    const tiers = (ladder?.steps || []).filter((step, index, steps) => !index || step.tier !== steps[index - 1].tier);
    return "<section class='sow-ranked__overview' data-tier='" + heroTier + "' aria-label='" + esc(SOW_t("ranked.my_rank")) + "'><div class='sow-ranked__hero'>" +
        rankEmblem(currentTier, currentDivision) +
        "<div class='sow-ranked__summary'><span class='sow-ranked__eyebrow'>" + esc(SOW_t("lobbies.ranked_1v1")) +
        "</span><h2>" + esc(rankName(currentTier, currentDivision)) + "</h2>" +
        "<p class='sow-ranked__points'>" + esc(SOW_t("lobbies.rank_score", { score: currentScore })) + "</p>" +
        rankProgressMarkup(currentScore, ladder) +
        "</div></div>" + (tiers.length ? "<ol class='sow-ranked__leagues'>" + tiers.map(step =>
            "<li" + (rankTier(step.tier) === heroTier ? " class='is-current' aria-current='step'" : "") + ">" +
            rankEmblem(step.tier) + "<strong>" + esc(rankName(step.tier)) + "</strong><small>" + esc(step.score) +
            "</small></li>").join("") + "</ol>" : "") +
        "<div class='sow-ranked__actions'>" + (own ? "<button type='button' class='sow-menu__primary' data-command='quick_match'>" +
            esc(SOW_t("ranked.play")) + "</button>" : "") +
        "<button type='button' class='sow-menu__secondary' data-command='profile_tab' data-profile-tab='victories'>" +
            esc(SOW_t("ranked.view_board")) + "</button></div></section>";
}

export function renderRankedDuel(lobby: MenuLobby | null, loading = false): string {
    const state = menuUi.state;
    const me = lobby?.players.find(player => player.player_id === state?.my_player_id);
    const rival = lobby?.players.find(player => player.player_id !== state?.my_player_id);
    const rating = menuUi.rankedRating;
    const myTier = rankTier(rating?.tier || "Nomad");
    const playerCard = (name: string, leader: string | null | undefined, opponent: boolean) =>
        "<article class='sow-ranked__player" + (opponent ? " sow-ranked__player--rival" : "") +
        "'" + (!opponent ? " data-tier='" + myTier + "'" : "") +
        "><img class='sow-ranked__portrait' " + avatarImageAttribute(leader === undefined ? avatarImage() : avatarImage(leader)) +
        " alt='' width='128' height='128'><strong>" + esc(name) + "</strong>" +
        (!opponent ? rankEmblem(rating?.tier || "Nomad", rating?.division || null) +
            "<span class='sow-ranked__player-rank' data-ranked-player-rank>" + esc(rankLabel(rating)) + "</span>" :
            "<span class='sow-ranked__player-status'>" + esc(SOW_t(loading ? "menu.loading" : rival?.is_ready ? "lobbies.ready" : "menu.loading")) + "</span>") + "</article>";
    return "<div class='sow-ranked__duel" + (lobby ? " is-found" : "") + "' data-ranked-match='" + esc(lobby?.id || "search") +
        "'><div class='sow-ranked__duel-heading'><span class='sow-ranked__eyebrow'>" + esc(SOW_t("lobbies.ranked_1v1")) +
        "</span><h2>" + esc(SOW_t(lobby ? "ranked.found" : "ranked.searching")) + "</h2></div><div class='sow-ranked__players'>" +
        playerCard(me?.name || state?.player_name || SOW_t("menu.anonymous"), me?.leader, false) +
        "<span class='sow-ranked__vs' aria-hidden='true'><svg class='sow-ranked__vs-swords' viewBox='0 0 36 36' fill='none'>" +
        "<path d='M6 30L13 23M7 25L11 29M29 7L25 7L11 21L15 25L29 11L29 7Z' stroke='currentColor' stroke-width='1.8' stroke-linecap='round' stroke-linejoin='round'/>" +
        "<path d='M30 30L23 23M29 25L25 29M7 7L11 7L25 21L21 25L7 11L7 7Z' stroke='currentColor' stroke-width='1.8' stroke-linecap='round' stroke-linejoin='round'/>" +
        "</svg><b>VS</b></span>" +
        (rival ? playerCard(rival.name, rival.leader, true) : "<article class='sow-ranked__player sow-ranked__player--empty'>" +
            "<div class='sow-ranked__search-halo' aria-hidden='true'>" +
            "<svg class='sow-ranked__radar-svg' viewBox='0 0 100 100' fill='none'>" +
            "<circle cx='50' cy='50' r='44' stroke='currentColor' stroke-opacity='0.24' stroke-width='1.5' stroke-dasharray='4 4'/>" +
            "<circle cx='50' cy='50' r='28' stroke='currentColor' stroke-opacity='0.34' stroke-width='1.2'/>" +
            "<path d='M50 6V20M50 80V94M6 50H20M80 50H94' stroke='currentColor' stroke-opacity='0.4' stroke-width='1.5' stroke-linecap='round'/>" +
            "</svg><span class='sow-ranked__search-glyph'>?</span></div><strong>" +
            esc(SOW_t("ranked.searching")) + "</strong></article>") + "</div>" +
        (lobby
            ? "<div class='sow-ranked__map'><img class='sow-ranked__map-thumb' src='" +
              esc(lobbyThumb(lobby)) +
              "' alt='' width='320' height='180'><div class='sow-ranked__map-meta'><small class='sow-ranked__map-kicker'>" +
              esc(SOW_t("lobbies.map")) +
              "</small><span>" +
              esc(formatMapName(lobby)) +
              "</span></div></div>"
            : "") +
        "</div>";
}

export function animateRankResult(card: HTMLElement, result: import("./main_menu.state.ts").RankedResult,
    reduced: () => boolean): { finish: () => void; cancel: () => void } {
    const ladder = result.ladder || menuUi.rankedLadder;
    const score = card.querySelector<HTMLElement>("[data-rank-score]");
    const rank = card.querySelector<HTMLElement>("[data-rank-name]");
    const emblem = card.querySelector<HTMLElement>("[data-rank-current-emblem]");
    const track = card.querySelector<HTMLElement>("[role=progressbar]");
    const fill = track?.querySelector<HTMLElement>("i");
    const target = card.querySelector<HTMLElement>("[data-rank-target]");
    const remaining = card.querySelector<HTMLElement>("[data-rank-remaining]");
    let frameId = 0;
    let started: number | null = null;
    let stopped = false;
    let lastStep = result.tier_before + (result.division_before || "");
    function write(value: number, final: boolean) {
        const progress = rankProgress(value, ladder);
        const tier = final ? result.tier_after : progress?.step.tier || result.tier_before;
        const division = final ? result.division_after : progress?.step.division || null;
        const key = tier + (division || "");
        if (score) score.textContent = SOW_t("lobbies.rank_score", { score: value });
        if (rank) rank.textContent = rankName(tier, division);
        if (key !== lastStep || final) {
            if (emblem) emblem.innerHTML = rankEmblem(tier, division);
            card.dataset.tier = rankTier(tier);
            if (key !== lastStep) {
                card.classList.remove("is-division-hit");
                void card.offsetWidth;
                card.classList.add("is-division-hit");
            }
            lastStep = key;
        }
        if (progress) {
            const label = progress.next ? rankName(progress.next.tier, progress.next.division) : SOW_t("ranked.maximum");
            if (fill) fill.style.transform = "scaleX(" + progress.fraction + ")";
            track?.setAttribute("aria-valuenow", String(Math.round(progress.fraction * 100)));
            track?.setAttribute("aria-label", label);
            if (target) target.textContent = label;
            if (remaining) remaining.textContent = Math.round(value) + " / " + progress.ceiling;
        }
        if (final || (tier === result.tier_after && tier !== result.tier_before))
            card.classList.add("is-rank-revealed");
    }
    function finish() {
        if (stopped) return;
        stopped = true;
        cancelAnimationFrame(frameId);
        write(result.score_after, true);
    }
    function frame(now: number) {
        if (stopped || !card.isConnected) return;
        if (reduced()) { finish(); return; }
        if (started === null) started = now;
        const elapsed = now - started;
        const t = Math.max(0, Math.min(1, (elapsed - 360) / 800));
        const value = Math.round(result.score_before + (result.score_after - result.score_before) * (1 - Math.pow(1 - t, 3)));
        write(value, t === 1);
        if (t < 1) frameId = requestAnimationFrame(frame);
        else stopped = true;
    }
    if (reduced()) finish();
    else { write(result.score_before, false); frameId = requestAnimationFrame(frame); }
    return { finish, cancel() { stopped = true; cancelAnimationFrame(frameId); } };
}
