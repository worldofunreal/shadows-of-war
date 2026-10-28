import { asset, avatarImage, esc, formatMapName, lobbyThumb, menuUi } from "./main_menu.core.ts";
import type { MenuLobby, RankedLadder, RankedPointsBreakdown, RankedStep } from "./main_menu.state.ts";
import type { ProfileRating } from "./main_menu.profile.ts";

// Shared presentation for queue, loader, profile and the confirmed match result.
export function rankTier(tier: string): string {
    const known = /^(Bronze|Silver|Gold|Platinum|Diamond|Master|Grandmaster)$/;
    return known.test(tier) ? tier.toLowerCase() : "unranked";
}

export function rankName(tier: string, division?: string | null): string {
    return SOW_t("ranked.tier_" + rankTier(tier)) + (division ? " " + division : "");
}

export function rankLabel(rating: Pick<ProfileRating, "tier" | "division" | "placements_complete" | "games_played"> | null): string {
    return rating && rating.placements_complete
        ? rankName(rating.tier, rating.division)
        : SOW_t("ranked.placements", { played: Math.min(5, rating?.games_played || 0) });
}

export function rankEmblem(tier: string, division?: string | null): string {
    const key = rankTier(tier);
    return "<span class='sow-ranked__emblem' data-tier='" + key + "' aria-hidden='true'><img src='" +
        esc(asset("shell/ranked/" + key + ".svg")) + "' alt='' width='160' height='160'>" +
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
    return { step, next, remaining: Math.max(0, ceiling - score),
        fraction: Math.max(0, Math.min(1, (score - step.score) / Math.max(1, ceiling - step.score))) };
}

export function rankProgressMarkup(score: number, ladder?: RankedLadder | null): string {
    const progress = rankProgress(score, ladder);
    if (!progress) return "";
    const target = progress.next ? rankName(progress.next.tier, progress.next.division) : SOW_t("ranked.maximum");
    return "<div class='sow-ranked__progress'><div class='sow-ranked__progress-label'><span data-rank-target>" +
        esc(target) + "</span><strong data-rank-remaining>" + esc(SOW_t("ranked.remaining", { points: progress.remaining })) +
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
        return "<p class='sow-ranked__breakdown'>" + esc(SOW_t("lobbies.rank_protected_loss")) + "</p>";
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
    if (!visible.length) return "<p class='sow-ranked__breakdown'>" + esc(SOW_t("lobbies.rank_draw")) + "</p>";
    return "<p class='sow-ranked__breakdown'>" + visible.map(function ([value, key]) {
        return esc((value > 0 ? "+" : "") + value + " " + SOW_t("lobbies.rank_breakdown_" + key));
    }).join(" · ") + "</p>";
}

export function rankOverview(rating: ProfileRating | null, ladder: RankedLadder | null | undefined, own: boolean): string {
    const placed = !!rating?.placements_complete;
    const tiers = (ladder?.steps || []).filter((step, index, steps) => !index || step.tier !== steps[index - 1].tier);
    return "<section class='sow-ranked__overview' aria-label='" + esc(SOW_t("ranked.my_rank")) + "'><div class='sow-ranked__hero'>" +
        rankEmblem(placed ? rating.tier : "unranked", placed ? rating.division : null) +
        "<div class='sow-ranked__summary'><span class='sow-ranked__eyebrow'>" + esc(SOW_t("lobbies.ranked_1v1")) +
        "</span><h2>" + esc(rankLabel(rating)) + "</h2>" +
        (placed ? "<p class='sow-ranked__points'>" + esc(SOW_t("lobbies.rank_score", { score: rating.score })) + "</p>" +
            rankProgressMarkup(rating.score, ladder) : rankPlacements(rating?.games_played || 0)) +
        "</div></div>" + (tiers.length ? "<ol class='sow-ranked__leagues'>" + tiers.map(step =>
            "<li" + (placed && step.tier === rating.tier ? " class='is-current' aria-current='step'" : "") + ">" +
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
    const playerCard = (name: string, leader: string | null | undefined, opponent: boolean) =>
        "<article class='sow-ranked__player" + (opponent ? " sow-ranked__player--rival" : "") +
        "'><img class='sow-ranked__portrait' src='" + esc(leader === undefined ? avatarImage() : avatarImage(leader)) +
        "' alt='' width='128' height='128'><strong>" + esc(name) + "</strong>" +
        (!opponent ? rankEmblem(rating?.placements_complete ? rating.tier : "unranked", rating?.placements_complete ? rating.division : null) +
            "<span>" + esc(rankLabel(rating)) + "</span>" : "<span>" + esc(SOW_t(loading ? "menu.loading" : rival?.is_ready ? "lobbies.ready" : "menu.loading")) + "</span>") + "</article>";
    return "<div class='sow-ranked__duel" + (lobby ? " is-found" : "") + "' data-ranked-match='" + esc(lobby?.id || "search") +
        "'><div class='sow-ranked__duel-heading'><span class='sow-ranked__eyebrow'>" + esc(SOW_t("lobbies.ranked_1v1")) +
        "</span><h2>" + esc(SOW_t(lobby ? "ranked.found" : "ranked.searching")) + "</h2></div><div class='sow-ranked__players'>" +
        playerCard(me?.name || state?.player_name || SOW_t("menu.anonymous"), me?.leader, false) +
        "<span class='sow-ranked__vs' aria-hidden='true'>VS</span>" +
        (rival ? playerCard(rival.name, rival.leader, true) : "<article class='sow-ranked__player sow-ranked__player--empty'><div class='sow-ranked__search-halo' aria-hidden='true'>?</div><strong>" +
            esc(SOW_t("ranked.searching")) + "</strong></article>") + "</div>" +
        (lobby ? `<div class='sow-ranked__map' style="background-image:url('${esc(lobbyThumb(lobby))}')"><span>` +
            esc(formatMapName(lobby)) + "</span></div>" : "") + "</div>";
}

export function animateRankResult(card: HTMLElement, result: import("./main_menu.state.ts").RankedResult,
    reduced: () => boolean): { finish: () => void; cancel: () => void } {
    if (!result.placements_complete) return { finish() {}, cancel() {} };
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
    const firstLeague = result.placements_complete && result.games_played === 5;
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
            if (remaining) remaining.textContent = SOW_t("ranked.remaining", { points: progress.remaining });
        }
        if (final || (!firstLeague && tier === result.tier_after && tier !== result.tier_before))
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
        if (firstLeague && t >= 0.45) card.classList.add("is-rank-revealed");
        if (t < 1) frameId = requestAnimationFrame(frame);
        else stopped = true;
    }
    if (reduced()) finish();
    else { write(result.score_before, false); frameId = requestAnimationFrame(frame); }
    return { finish, cancel() { stopped = true; cancelAnimationFrame(frameId); } };
}
