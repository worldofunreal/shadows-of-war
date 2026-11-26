import { avatarImage, esc, formatMapName, lobbyThumb, menuUi } from "./main_menu.core.ts";
import { avatarImageAttribute } from "./avatar-presentation.ts";
import type { MenuLobby, RankedLadder } from "./main_menu.state.ts";
import type { ProfileRating } from "./main_menu.profile.ts";
import {
    rankBadgeSlot,
    rankCulture,
    rankEmblem,
    rankLabel,
    rankName,
    rankPlacements,
    rankProgress,
    rankProgressMarkup,
    rankTier,
    rankedPointsBreakdownMarkup,
    renderRankedOverview,
} from "./sow-ranked-presentation.ts";

export {
    rankBadgeSlot,
    rankCulture,
    rankEmblem,
    rankLabel,
    rankName,
    rankPlacements,
    rankProgress,
    rankProgressMarkup,
    rankTier,
    rankedPointsBreakdownMarkup,
    renderRankedOverview,
};

// Shared presentation for queue, loader, profile and the confirmed match result.
export function rankOverview(
    rating: ProfileRating | null,
    ffaRating: ProfileRating | null,
    ladder: RankedLadder | null | undefined,
    own: boolean,
    activeQueue: "Ranked1v1" | "RankedFFA" = "Ranked1v1",
): string {
    return renderRankedOverview({
        rating1v1: rating,
        ratingFfa: ffaRating,
        ladder,
        own,
        mode: "game",
        activeQueue,
    });
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
    reduced: () => boolean): { finish: () => void; accelerate: () => void; cancel: () => void } {
    const ladder = result.ladder || menuUi.rankedLadder;
    const score = card.querySelector<HTMLElement>("[data-rank-score]");
    const rank = card.querySelector<HTMLElement>("[data-rank-name]");
    const emblem = card.querySelector<HTMLElement>("[data-rank-current-emblem]");
    const track = card.querySelector<HTMLElement>("[role=progressbar]");
    const fill = track?.querySelector<HTMLElement>("i");
    const target = card.querySelector<HTMLElement>("[data-rank-target]");
    const remaining = card.querySelector<HTMLElement>("[data-rank-remaining]");
    let frameId = 0;
    let lastNow: number | null = null;
    let virtualElapsed = 0;
    let speed = 1;
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
                if (typeof emblem?.animate === "function") {
                    emblem.animate(
                        [
                            { transform: "scale(0.88)" },
                            { transform: "scale(1.12)" },
                            { transform: "scale(1)" },
                        ],
                        { duration: 340, easing: "cubic-bezier(0.22, 1, 0.36, 1)" },
                    );
                }
                if (typeof track?.animate === "function") {
                    track.animate(
                        [
                            { transform: "scaleY(1)" },
                            { transform: "scaleY(1.24)" },
                            { transform: "scaleY(1)" },
                        ],
                        { duration: 260, easing: "ease-out" },
                    );
                }
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
    function accelerate() {
        if (stopped || speed > 1) return;
        speed = 5;
        if (typeof card.getAnimations === "function") {
            card.getAnimations({ subtree: true }).forEach(function (anim) {
                if (anim.playState !== "finished") {
                    try {
                        anim.playbackRate = 5;
                    } catch {}
                }
            });
        }
    }
    function frame(now: number) {
        if (stopped || !card.isConnected) return;
        if (reduced()) { finish(); return; }
        if (lastNow === null) lastNow = now;
        const dt = Math.min(100, Math.max(0, now - lastNow));
        lastNow = now;
        virtualElapsed += dt * speed;
        const t = Math.max(0, Math.min(1, (virtualElapsed - 360) / 800));
        const value = Math.round(result.score_before + (result.score_after - result.score_before) * (1 - Math.pow(1 - t, 3)));
        write(value, t === 1);
        if (t < 1) frameId = requestAnimationFrame(frame);
        else stopped = true;
    }
    if (reduced()) finish();
    else { write(result.score_before, false); frameId = requestAnimationFrame(frame); }
    return { finish, accelerate, cancel() { stopped = true; cancelAnimationFrame(frameId); } };
}
