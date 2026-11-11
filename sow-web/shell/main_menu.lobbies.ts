import { rankEmblem, rankLabel, rankTier, renderRankedDuel } from "./main_menu.ranked.ts";
import { avatarBackgroundCss, avatarImageAttribute, setAvatarImage } from "./avatar-presentation.ts";
import type { CampaignDefinition } from "./campaign/types.ts";
import {
    asset,
    avatarImage,
    esc,
    findLobby,
    formatMapName,
    leaderArtSlug,
    leaderArtUrl,
    leaderById,
    leaderDisplayName,
    lobbyThumb,
    localizedText,
    mapInfo,
    menuIcon,
    menuUi,
    renderHeroBriefing,
    root,
} from "./main_menu.core.ts";
import { activeScreenPanel, render } from "./main_menu.shell.ts";
import type {
    MenuCampaignEpisode,
    MenuCustomGameConfig,
    MenuLobby,
    MenuLobbyPlayer,
    MenuState,
} from "./main_menu.state.ts";
import { renderCommandPanel, renderFeedback } from "./main_menu.surfaces.ts";

// Lobby-facing screens: Home, Browser, Create, and Queue.

function publicLobbies(includeMatchmaking: boolean) {
    return (menuUi.state!.lobbies || []).filter(function (lobby) {
        if (lobby.kind === "Matchmaking") {
            if (!includeMatchmaking) return false;
        } else if (lobby.kind !== "Custom" || lobby.is_private) {
            return false;
        }
        return true;
    });
}

export function lobbyPlayerCount(lobby: MenuLobby) {
    return Math.max(0, Number(lobby.num_players) || 0) + "/" + Math.max(0, Number(lobby.max_players) || 0);
}

export function lobbyStatusText(lobby: MenuLobby) {
    return lobby.is_counting_down
        ? SOW_t("lobbies.starting_in", { seconds: Math.ceil(lobby.timer_secs || 0) })
        : SOW_t("lobbies.waiting_for_players");
}

function gameModeLabel(mode: string) {
    const keys: Record<string, string> = {
        FFA: "lobbies.free_for_all",
        Teams: "lobbies.teams_two",
        HumansVsNations: "lobbies.humans_vs_nations",
        Matchmaking: "lobbies.matchmaking",
    };
    const key = keys[mode];
    return key ? SOW_t(key) : String(mode || "");
}

const lobbyThumbnailCache: Record<string, Promise<string> | undefined> = Object.create(null);

function preloadLobbyThumbnail(lobby: MenuLobby) {
    const url = lobbyThumb(lobby);
    const cached = lobbyThumbnailCache[url];
    if (cached) return cached;
    const preload = new Promise<string>(function (resolve, reject) {
        const image = new Image();
        image.onload = function () {
            if (typeof image.decode !== "function") return resolve(url);
            try {
                image.decode().then(
                    function () {
                        resolve(url);
                    },
                    function () {
                        resolve(url);
                    },
                );
            } catch {
                resolve(url);
            }
        };
        image.onerror = function (error) {
            delete lobbyThumbnailCache[url];
            reject(error);
        };
        image.decoding = "async";
        image.src = url;
    });
    lobbyThumbnailCache[url] = preload;
    return preload;
}

export function lobbyLabel(lobby: MenuLobby) {
    return (
        gameModeLabel(lobby.game_mode || "FFA") + " " + formatMapName(lobby) + " " + lobbyPlayerCount(lobby)
    );
}

function renderLobbyCard(lobby: MenuLobby) {
    const lock = lobby.has_password
        ? "<span class='sow-menu__lobby-lock' aria-label='" +
          esc(SOW_t("lobbies.password_protected")) +
          "'>" +
          menuIcon("lock") +
          "</span>"
        : "";
    const label = lobbyLabel(lobby);
    preloadLobbyThumbnail(lobby).catch(function () {});
    return (
        "" +
        "<article class='sow-menu__lobby' role='button' tabindex='0' aria-label='" +
        esc(label) +
        "' data-lobby-card data-command='join_lobby' data-lobby-id='" +
        lobby.id +
        "'>" +
        "<img class='sow-menu__lobby-art' src='" +
        esc(lobbyThumb(lobby)) +
        "' alt='' loading='eager' decoding='async'>" +
        "<div class='sow-menu__lobby-top'><span class='sow-menu__lobby-chip' data-lobby-mode>" +
        esc(gameModeLabel(lobby.game_mode)) +
        "</span><span class='sow-menu__lobby-chip sow-menu__lobby-chip--status' data-lobby-status-for='" +
        lobby.id +
        "'>" +
        esc(lobbyStatusText(lobby)) +
        "</span>" +
        lock +
        "</div>" +
        "<div class='sow-menu__lobby-bottom'><h3 data-lobby-map>" +
        esc(formatMapName(lobby)) +
        "</h3><span class='sow-menu__lobby-count' data-lobby-count>" +
        esc(lobbyPlayerCount(lobby)) +
        "</span></div>" +
        "</article>"
    );
}

export function renderPublicPanel(listName: string) {
    return (
        "" +
        "<section class='sow-menu__public'>" +
        "<div class='sow-menu__lobbies' data-lobby-list='" +
        esc(listName || "home") +
        "'></div>" +
        "</section>"
    );
}

function filteredBrowserLobbies() {
    const lobbies = publicLobbies(false);
    if (!menuUi.browserSearchQuery) return lobbies;
    const q = menuUi.browserSearchQuery.toLowerCase().trim();
    return lobbies.filter(function (lobby) {
        return (
            (lobby.map_name && lobby.map_name.toLowerCase().indexOf(q) !== -1) ||
            (lobby.host_name && lobby.host_name.toLowerCase().indexOf(q) !== -1) ||
            (lobby.game_mode && lobby.game_mode.toLowerCase().indexOf(q) !== -1)
        );
    });
}

function updateLobbyCard(card: HTMLElement, lobby: MenuLobby) {
    const previousMap = card.dataset.mapName || "";
    const nextMap = String(lobby.map_name || "world");
    const art = card.querySelector<HTMLImageElement>(".sow-menu__lobby-art");
    const mode = card.querySelector("[data-lobby-mode]");
    const map = card.querySelector("[data-lobby-map]");
    const status = card.querySelector("[data-lobby-status-for]");
    const count = card.querySelector("[data-lobby-count]");
    const lock = card.querySelector(".sow-menu__lobby-lock");
    card.dataset.mapName = nextMap;
    card.setAttribute("aria-label", lobbyLabel(lobby));
    if (mode) mode.textContent = gameModeLabel(lobby.game_mode);
    if (map) map.textContent = formatMapName(lobby);
    if (status) status.textContent = lobbyStatusText(lobby);
    if (count) count.textContent = lobbyPlayerCount(lobby);
    if (lobby.has_password && !lock) {
        card.querySelector(".sow-menu__lobby-top")!.insertAdjacentHTML(
            "beforeend",
            "<span class='sow-menu__lobby-lock' aria-label='" +
                esc(SOW_t("lobbies.password_protected")) +
                "'>" +
                menuIcon("lock") +
                "</span>",
        );
    } else if (!lobby.has_password && lock) {
        lock.remove();
    }
    if (!art || previousMap === nextMap) return;
    const nextUrl = lobbyThumb(lobby);
    preloadLobbyThumbnail(lobby)
        .then(function () {
            if (card.dataset.mapName !== nextMap || !card.isConnected) return;
            card.classList.add("is-map-changing");
            art.src = nextUrl;
            requestAnimationFrame(function () {
                card.classList.remove("is-map-changing");
            });
        })
        .catch(function () {});
}

function makeLobbyCard(lobby: MenuLobby) {
    const template = document.createElement("template");
    template.innerHTML = renderLobbyCard(lobby).trim();
    const card = template.content.querySelector<HTMLElement>("[data-lobby-card]")!;
    card.dataset.mapName = String(lobby.map_name || "world");
    const art = card.querySelector(".sow-menu__lobby-art");
    if (art)
        art.addEventListener(
            "error",
            function () {
                art.remove();
            },
            { once: true },
        );
    return card;
}

function syncLobbyList(container: HTMLElement, lobbies: MenuLobby[], emptyMessage: string) {
    const cards: Record<string, HTMLElement> = Object.create(null);
    container.querySelectorAll<HTMLElement>("[data-lobby-card]").forEach(function (card) {
        const id = String(card.dataset.lobbyId);
        if (cards[id]) {
            card.remove();
            return;
        }
        cards[id] = card;
    });
    if (!lobbies.length) {
        Object.keys(cards).forEach(function (id) {
            cards[id].remove();
        });
        let empty = container.querySelector(".sow-menu__empty");
        if (!empty) {
            empty = document.createElement("div");
            empty.className = "sow-menu__empty";
            container.appendChild(empty);
        }
        empty.textContent = emptyMessage;
        return;
    }
    const empty = container.querySelector(".sow-menu__empty");
    if (empty) empty.remove();
    const seen: Record<string, boolean> = Object.create(null);
    lobbies.forEach(function (lobby) {
        const id = String(lobby.id);
        if (seen[id]) return;
        seen[id] = true;
        let card = cards[id];
        if (!card) card = makeLobbyCard(lobby);
        updateLobbyCard(card, lobby);
        container.appendChild(card);
        delete cards[id];
    });
    Object.keys(cards).forEach(function (id) {
        cards[id].remove();
    });
}

export function updateLobbyViews() {
    const panel = activeScreenPanel() || root;
    panel.querySelectorAll<HTMLElement>("[data-lobby-list]").forEach(function (container) {
        const browser = container.dataset.lobbyList === "browser";
        syncLobbyList(
            container,
            browser ? filteredBrowserLobbies() : publicLobbies(true),
            browser ? SOW_t("lobbies.no_search_matches") : SOW_t("lobbies.no_active_lobbies"),
        );
    });
}

export function renderHome() {
    const leader = menuUi.state!.prefer_random_leader ? "random" : leaderById(menuUi.state!.selected_leader);
    return (
        "<main class='sow-menu__main' data-screen-panel='home'>" +
        renderCommandPanel() +
        "<section class='sow-menu__battlefield'>" +
        "<div class='sow-menu__leader-copy'>" +
        renderHeroBriefing(leader, "home", "sow-home-selected") +
        "</div>" +
        "</section>" +
        "</main>"
    );
}

const campaignCinematicAvailability: Record<string, boolean> = Object.create(null);
const campaignCinematicDiscovery: Record<string, Promise<void>> = Object.create(null);

function discoverCampaignCinematics(episodes: MenuCampaignEpisode[]) {
    const pending = (episodes || []).filter(function (episode) {
        const id = String((episode && episode.id) || "");
        return (
            /^[a-z][a-z0-9_]{0,63}$/.test(id) &&
            !Object.prototype.hasOwnProperty.call(campaignCinematicAvailability, id) &&
            !campaignCinematicDiscovery[id]
        );
    });
    pending.forEach(function (episode) {
        const id = String(episode.id);
        campaignCinematicDiscovery[id] = fetch(asset("campaign/" + id + ".triggers.json"))
            .then(function (response) {
                if (!response.ok) throw new Error("Campaign scene unavailable.");
                return response.json();
            })
            .then(function (definition: CampaignDefinition) {
                const entry = (definition.steps || []).find(function (step) {
                    return step.id === definition.entry;
                });
                const source = entry && entry.video_src;
                campaignCinematicAvailability[id] = Boolean(
                    entry &&
                    entry.type === "scene" &&
                    entry.presentation === "cinematic" &&
                    typeof source === "string" &&
                    source.indexOf("/assets/campaign/" + id + "/") === 0 &&
                    /\.(?:mp4|webm)$/i.test(source),
                );
            })
            .catch(function () {
                campaignCinematicAvailability[id] = false;
            })
            .then(function () {
                delete campaignCinematicDiscovery[id];
            });
    });
    if (!pending.length) return;
    Promise.all(
        pending.map(function (episode) {
            return campaignCinematicDiscovery[String(episode.id)];
        }),
    ).then(function () {
        if (menuUi.campaignOpen) render();
    });
}

export function campaignEpisodeHeroSlug(episodeId: string): string {
    const id = String(episodeId || "").trim();
    if (id === "boudica") return "boudica";
    if (id.startsWith("six_sky")) return "lady_six_sky";
    const prefixMatch = id.match(/^([a-z0-9_]+)_(?:ep\d+|\d+)$/);
    const candidate = prefixMatch ? prefixMatch[1] : id;
    if (candidate === "six_sky") return "lady_six_sky";
    if (/^[a-z0-9_]+$/.test(candidate)) return candidate;
    const leader = leaderById(candidate);
    if (leader) return leaderArtSlug(leader);
    return "boudica";
}

export function campaignEpisodeMapName(episodeId: string): string {
    const id = String(episodeId || "").trim();
    if (id === "boudica") return "eastanglia";
    if (id.startsWith("six_sky")) return "northamerica";
    return "world";
}

export function campaignEpisodeSagaId(episodeId: string): string {
    const id = String(episodeId || "").trim();
    if (id === "boudica") return "boudica";
    if (id.startsWith("six_sky")) return "six_sky";
    const prefixMatch = id.match(/^([a-z0-9_]+)_(?:ep\d+|\d+)$/);
    return prefixMatch ? prefixMatch[1] : id;
}

export function campaignSagaTitle(sagaId: string): string {
    if (sagaId === "all") return SOW_t("lobbies.the_saga") || "The Saga";
    if (sagaId === "boudica") return SOW_t("lobbies.campaign_boudica_title") || "Rise of the Iceni";
    if (sagaId === "six_sky") {
        return leaderDisplayName(leaderById("lady_six_sky")) || "Lady Six Sky";
    }
    const leader = leaderById(sagaId);
    if (leader) return leaderDisplayName(leader);
    return sagaId.charAt(0).toUpperCase() + sagaId.slice(1).replace(/_/g, " ");
}

export function campaignEpisodeHeroArt(heroSlug: string) {
    return {
        band: leaderArtUrl(leaderArtSlug(heroSlug), "desktop"),
        column: leaderArtUrl(leaderArtSlug(heroSlug), "mobile"),
    };
}

export function campaignHeroImage(episodeId?: string | null): string {
    const id = episodeId || menuUi.selectedCampaignEpisode || "boudica";
    const heroSlug = campaignEpisodeHeroSlug(id);
    const compact =
        typeof window.matchMedia === "function"
            ? window.matchMedia("(max-width: 900px), (orientation: portrait)").matches
            : window.innerWidth <= 900 || window.innerHeight >= window.innerWidth;
    return leaderArtUrl(leaderArtSlug(heroSlug), compact ? "desktop" : "mobile");
}

export function updateCampaignSelection(episodeId: string): boolean {
    const campaign = menuUi.state && menuUi.state.campaign;
    const episodes = campaign && Array.isArray(campaign.episodes) ? campaign.episodes : [];
    const episode = episodes.find((e) => e.id === episodeId);
    if (!episode) return false;

    const panel = activeScreenPanel() || root;
    const campaignPanel = panel.querySelector<HTMLElement>(".sow-campaign");
    if (!campaignPanel) return false;

    // Update active selection on episode cards
    const cards = campaignPanel.querySelectorAll<HTMLElement>(".sow-campaign__episode");
    cards.forEach((card) => {
        const isSelected = card.dataset.episodeId === episodeId;
        card.classList.toggle("is-selected", isSelected);
        card.setAttribute("aria-pressed", isSelected ? "true" : "false");
    });

    // Update the featured hero showcase
    const featured = campaignPanel.querySelector<HTMLElement>(".sow-campaign__featured");
    if (!featured) return false;

    const heroSlug = campaignEpisodeHeroSlug(episodeId);
    const heroArt = campaignEpisodeHeroArt(heroSlug);
    const heroLeader = leaderById(heroSlug);
    const heroName = heroLeader ? leaderDisplayName(heroLeader) : heroSlug;
    const picture = featured.querySelector<HTMLPictureElement>(".sow-campaign__hero-picture");
    const source = picture && picture.querySelector<HTMLSourceElement>("source");
    const img = picture && picture.querySelector<HTMLImageElement>("img");

    if (img && img.dataset.heroSlug !== heroSlug) {
        img.dataset.heroSlug = heroSlug;
        img.alt = heroName;
        featured.classList.add("is-transitioning");
        if (source) source.srcset = heroArt.band;
        img.src = heroArt.column;
        featured.classList.remove("is-transitioning");
        featured.classList.add("is-hero-entering");
        window.setTimeout(() => {
            featured.classList.remove("is-hero-entering");
        }, 420);
    }

    const titleNode = featured.querySelector<HTMLElement>(".sow-campaign__featured-title");
    if (titleNode) titleNode.textContent = localizedText(episode.title);

    const subtitleNode = featured.querySelector<HTMLElement>(".sow-campaign__featured-subtitle");
    if (subtitleNode) subtitleNode.textContent = localizedText(episode.subtitle);

    const chapterTag = featured.querySelector<HTMLElement>(".sow-campaign__chapter-tag");
    if (chapterTag) {
        chapterTag.textContent = campaignSagaTitle(campaignEpisodeSagaId(episode.id));
    }

    const continueId = (campaign && campaign.continue_episode) || "";
    const completed = !!episode.completed;
    const unlocked = !!episode.unlocked;
    const isNext = unlocked && !completed && episode.id === continueId;
    const statusLabel = completed
        ? SOW_t("lobbies.completed")
        : unlocked
          ? isNext
              ? SOW_t("lobbies.next")
              : SOW_t("lobbies.available")
          : SOW_t("lobbies.locked");
    const statusClass = completed
        ? "is-complete"
        : unlocked
          ? isNext
              ? "is-next"
              : "is-unlocked"
          : "is-locked";

    const statusNode = featured.querySelector<HTMLElement>(".sow-campaign__featured-status");
    if (statusNode) {
        statusNode.textContent = statusLabel;
        statusNode.className = "sow-campaign__episode-status sow-campaign__featured-status " + statusClass;
    }

    const mapName = campaignEpisodeMapName(episodeId);
    const mapThumb = featured.querySelector<HTMLImageElement>(".sow-campaign__map-thumb");
    if (mapThumb) mapThumb.src = lobbyThumb({ map_name: mapName });
    const mapLabel = featured.querySelector<HTMLElement>(".sow-campaign__meta-map strong");
    if (mapLabel) mapLabel.textContent = formatMapName(mapName);

    const advisorAvatar = featured.querySelector<HTMLImageElement>(".sow-campaign__advisor-avatar");
    if (advisorAvatar) setAvatarImage(advisorAvatar, asset("gameplay/avatars/" + leaderArtSlug(heroSlug) + ".webp"));
    const advisorName = featured.querySelector<HTMLElement>(".sow-campaign__meta-advisor strong");
    if (advisorName) advisorName.textContent = heroName;

    const playBtn = featured.querySelector<HTMLButtonElement>(".sow-campaign__action-play");
    if (playBtn) {
        playBtn.dataset.episodeId = episode.id;
        playBtn.disabled = !unlocked;
        const labelText = completed
            ? SOW_t("lobbies.replay")
            : isNext
              ? SOW_t("lobbies.continue")
              : SOW_t("lobbies.play");
        const actionIcon = completed ? "↺" : "▶";
        playBtn.innerHTML = `<span>${esc(labelText)}</span> <i>${actionIcon}</i>`;
    }

    const watchBtn = featured.querySelector<HTMLButtonElement>(".sow-campaign__action-watch");
    if (watchBtn) {
        watchBtn.dataset.episodeId = episode.id;
        watchBtn.hidden = !campaignCinematicAvailability[episode.id];
    }

    return true;
}

function renderCampaignEpisode(
    episode: MenuCampaignEpisode,
    continueId: string,
    isSelected: boolean = false,
) {
    const completed = !!episode.completed;
    const unlocked = !!episode.unlocked;
    const isNext = unlocked && !completed && episode.id === continueId;
    const title = localizedText(episode.title);
    const subtitle = localizedText(episode.subtitle);
    const mapName = campaignEpisodeMapName(episode.id);
    const mapArt = lobbyThumb({ map_name: mapName });
    const heroSlug = campaignEpisodeHeroSlug(episode.id);
    const avatar = asset("gameplay/avatars/" + leaderArtSlug(heroSlug) + ".webp");
    const status = completed
        ? SOW_t("lobbies.completed")
        : unlocked
          ? isNext
              ? SOW_t("lobbies.next")
              : SOW_t("lobbies.available")
          : SOW_t("lobbies.locked");
    const label = completed
        ? SOW_t("lobbies.replay")
        : isNext
          ? SOW_t("lobbies.continue")
          : SOW_t("lobbies.play");
    const action = unlocked
        ? "<button class='sow-menu__secondary sow-campaign__play' type='button' data-command='start_campaign_episode' data-episode-id='" +
          esc(episode.id) +
          "'>" +
          esc(label) +
          " <span>" +
          (completed ? "↺" : "↗") +
          "</span></button>"
        : "<button class='sow-menu__secondary sow-campaign__play' type='button' disabled>" +
          esc(SOW_t("lobbies.locked")) +
          "</button>";
    const watch = campaignCinematicAvailability[episode.id]
        ? "<button class='sow-menu__secondary sow-campaign__watch' type='button' data-command='watch_campaign_cinematic' data-episode-id='" +
          esc(episode.id) +
          "'>" +
          esc(SOW_t("tutorial.watch_cinematic")) +
          "</button>"
        : "";
    return (
        "<article class='sow-campaign__episode" +
        (completed ? " is-complete" : unlocked ? " is-unlocked" : " is-locked") +
        (isNext ? " is-next" : "") +
        (isSelected ? " is-selected" : "") +
        "' data-command='select_campaign_episode' data-episode-id='" +
        esc(episode.id) +
        "' role='button' tabindex='0' aria-pressed='" +
        isSelected +
        "'>" +
        "<div class='sow-campaign__episode-art' aria-hidden='true'><img class='sow-campaign__map-art' src='" +
        esc(mapArt) +
        "' alt='' loading='lazy' decoding='async'><img class='sow-campaign__leader-art' " +
        avatarImageAttribute(avatar) +
        " alt='' loading='lazy' decoding='async'></div>" +
        "<div class='sow-campaign__episode-copy'><div class='sow-campaign__episode-status'>" +
        esc(status) +
        "</div>" +
        "<h2>" +
        esc(title) +
        "</h2>" +
        "<p>" +
        esc(subtitle) +
        "</p>" +
        "<div class='sow-campaign__episode-actions'>" +
        action +
        watch +
        "</div>" +
        "</div>" +
        "</article>"
    );
}

export function renderCampaign() {
    const campaign = menuUi.state && menuUi.state.campaign;
    const episodes = campaign && Array.isArray(campaign.episodes) ? campaign.episodes : [];
    discoverCampaignCinematics(episodes);
    const continueId = (campaign && campaign.continue_episode) || "";

    // Determine selected episode
    let selectedId = menuUi.selectedCampaignEpisode || "";
    if (!selectedId || !episodes.some((e) => e.id === selectedId)) {
        if (continueId && episodes.some((e) => e.id === continueId)) {
            selectedId = continueId;
        } else {
            const firstAvailable = episodes.find((e) => e.unlocked && !e.completed);
            selectedId = firstAvailable ? firstAvailable.id : episodes[0] ? episodes[0].id : "";
        }
    }
    menuUi.selectedCampaignEpisode = selectedId;
    const selectedEpisode = episodes.find((e) => e.id === selectedId) || episodes[0] || null;

    // Group into Sagas
    const sagasMap = new Map<
        string,
        { id: string; title: string; count: number; completed: number; heroSlug: string }
    >();
    episodes.forEach((ep) => {
        const sagaId = campaignEpisodeSagaId(ep.id);
        let s = sagasMap.get(sagaId);
        if (!s) {
            s = {
                id: sagaId,
                title: campaignSagaTitle(sagaId),
                count: 0,
                completed: 0,
                heroSlug: campaignEpisodeHeroSlug(ep.id),
            };
            sagasMap.set(sagaId, s);
        }
        s.count++;
        if (ep.completed) s.completed++;
    });

    const totalCompleted = episodes.filter((e) => e.completed).length;
    const activeSaga = menuUi.selectedCampaignSaga || "all";

    const filteredEpisodes =
        activeSaga === "all"
            ? episodes
            : episodes.filter((ep) => campaignEpisodeSagaId(ep.id) === activeSaga);

    let featuredHtml = "";
    if (selectedEpisode) {
        const heroSlug = campaignEpisodeHeroSlug(selectedEpisode.id);
        const heroArt = campaignEpisodeHeroArt(heroSlug);
        const heroLeader = leaderById(heroSlug);
        const heroName = heroLeader ? leaderDisplayName(heroLeader) : heroSlug;
        const mapName = campaignEpisodeMapName(selectedEpisode.id);
        const mapThumb = lobbyThumb({ map_name: mapName });
        const mapDisplayName = formatMapName(mapName);
        const avatar = asset("gameplay/avatars/" + leaderArtSlug(heroSlug) + ".webp");
        const completed = !!selectedEpisode.completed;
        const unlocked = !!selectedEpisode.unlocked;
        const isNext = unlocked && !completed && selectedEpisode.id === continueId;
        const statusLabel = completed
            ? SOW_t("lobbies.completed")
            : unlocked
              ? isNext
                  ? SOW_t("lobbies.next")
                  : SOW_t("lobbies.available")
              : SOW_t("lobbies.locked");
        const statusClass = completed
            ? "is-complete"
            : unlocked
              ? isNext
                  ? "is-next"
                  : "is-unlocked"
              : "is-locked";
        const chapterTitle = campaignSagaTitle(campaignEpisodeSagaId(selectedEpisode.id));
        const playLabel = completed
            ? SOW_t("lobbies.replay")
            : isNext
              ? SOW_t("lobbies.continue")
              : SOW_t("lobbies.play");
        const playIcon = completed ? "↺" : "▶";

        const playAction = unlocked
            ? "<button class='sow-menu__primary sow-campaign__action-play' type='button' data-command='start_campaign_episode' data-episode-id='" +
              esc(selectedEpisode.id) +
              "'><span>" +
              esc(playLabel) +
              "</span> <i>" +
              playIcon +
              "</i></button>"
            : "<button class='sow-menu__primary sow-campaign__action-play' type='button' disabled><span>" +
              esc(SOW_t("lobbies.locked")) +
              "</span> <i>" + menuIcon("lock") + "</i></button>";

        const watchAction = campaignCinematicAvailability[selectedEpisode.id]
            ? "<button class='sow-menu__secondary sow-campaign__action-watch' type='button' data-command='watch_campaign_cinematic' data-episode-id='" +
              esc(selectedEpisode.id) +
              "'><span>" +
              esc(SOW_t("tutorial.watch_cinematic")) +
              "</span> <i>▶</i></button>"
            : "";

        featuredHtml =
            "<section class='sow-campaign__featured' aria-labelledby='sow-campaign-selected-title'>" +
            "<div class='sow-campaign__hero-backdrop' aria-hidden='true'>" +
            "<picture class='sow-campaign__hero-picture'>" +
            "<source media='(max-width: 900px), (orientation: portrait)' srcset='" +
            esc(heroArt.band) +
            "'>" +
            "<img class='sow-campaign__hero-img' src='" +
            esc(heroArt.column) +
            "' alt='" +
            esc(heroName) +
            "' data-campaign-hero-art data-hero-slug='" +
            esc(heroSlug) +
            "' fetchpriority='high'>" +
            "</picture>" +
            "<div class='sow-campaign__hero-vignette'></div>" +
            "<div class='sow-campaign__hero-glow'></div>" +
            "</div>" +
            "<div class='sow-campaign__featured-content'>" +
            "<div class='sow-campaign__featured-badges'>" +
            "<span class='sow-campaign__chapter-tag'>" +
            esc(chapterTitle) +
            "</span>" +
            "<div class='sow-campaign__episode-status sow-campaign__featured-status " +
            statusClass +
            "'>" +
            esc(statusLabel) +
            "</div>" +
            "</div>" +
            "<div class='sow-campaign__featured-titles'>" +
            "<h2 class='sow-campaign__featured-title' id='sow-campaign-selected-title'>" +
            esc(localizedText(selectedEpisode.title)) +
            "</h2>" +
            "<p class='sow-campaign__featured-subtitle'>" +
            esc(localizedText(selectedEpisode.subtitle)) +
            "</p>" +
            "</div>" +
            "<div class='sow-campaign__featured-meta'>" +
            "<div class='sow-campaign__meta-item sow-campaign__meta-advisor'>" +
            "<img class='sow-campaign__advisor-avatar' " +
            avatarImageAttribute(avatar) +
            " alt='' loading='lazy'>" +
            "<div><small class='sow-campaign__meta-label'>" +
            esc(SOW_t("menu.heroes")) +
            "</small><strong>" +
            esc(heroName) +
            "</strong></div>" +
            "</div>" +
            "<div class='sow-campaign__meta-item sow-campaign__meta-map'>" +
            "<img class='sow-campaign__map-thumb' src='" +
            esc(mapThumb) +
            "' alt='' loading='lazy'>" +
            "<div><small class='sow-campaign__meta-label'>" +
            esc(SOW_t("lobbies.map")) +
            "</small><strong>" +
            esc(mapDisplayName) +
            "</strong></div>" +
            "</div>" +
            "</div>" +
            "<div class='sow-campaign__featured-actions'>" +
            playAction +
            watchAction +
            "</div>" +
            "</div>" +
            "</section>";
    }

    const sagaTabsHtml =
        "<button class='sow-campaign__saga-tab" +
        (activeSaga === "all" ? " is-active" : "") +
        "' type='button' data-command='select_campaign_saga' data-saga-id='all'>" +
        "<span class='sow-campaign__saga-name'>" +
        esc(SOW_t("lobbies.the_saga")) +
        "</span>" +
        "<span class='sow-campaign__saga-count'>" +
        totalCompleted +
        "/" +
        episodes.length +
        "</span></button>" +
        Array.from(sagasMap.values())
            .map(
                (saga) =>
                    "<button class='sow-campaign__saga-tab" +
                    (activeSaga === saga.id ? " is-active" : "") +
                    "' type='button' data-command='select_campaign_saga' data-saga-id='" +
                    esc(saga.id) +
                    "'>" +
                    "<img class='sow-campaign__saga-avatar' " +
                    avatarImageAttribute(asset("gameplay/avatars/" + leaderArtSlug(saga.heroSlug) + ".webp")) +
                    " alt='' loading='lazy'>" +
                    "<span class='sow-campaign__saga-name'>" +
                    esc(saga.title) +
                    "</span>" +
                    "<span class='sow-campaign__saga-count'>" +
                    saga.completed +
                    "/" +
                    saga.count +
                    "</span></button>",
            )
            .join("");

    const isSagaComplete = episodes.length > 0 && totalCompleted === episodes.length;

    const continueButton = continueId
        ? "<button class='sow-menu__primary' type='button' data-command='start_campaign_episode' data-episode-id='" +
          esc(continueId) +
          "'>" +
          esc(SOW_t("lobbies.continue_campaign")) +
          " <span>↗</span></button>"
        : isSagaComplete
          ? "<div class='sow-campaign__complete'>" + esc(SOW_t("lobbies.saga_complete")) + "</div>"
          : "";

    return (
        "<main class='sow-menu__main sow-campaign' data-screen-panel='campaign'>" +
        "<section class='sow-menu__command sow-campaign__intro'>" +
        "<div class='sow-campaign__header-group'>" +
        "<p class='sow-menu__eyebrow'>" +
        esc(SOW_t("lobbies.single_player")) +
        "</p>" +
        "<h1>" +
        esc(SOW_t("lobbies.campaign_chronicles")).replace("\n", "<br>") +
        "</h1>" +
        "</div>" +
        "<button class='sow-menu__secondary' type='button' data-command='close_campaign'>← " +
        esc(SOW_t("lobbies.back")) +
        "</button>" +
        "</section>" +
        "<nav class='sow-campaign__sagas-nav' aria-label='" +
        esc(SOW_t("lobbies.the_saga")) +
        "'><div class='sow-campaign__sagas-rail'>" +
        sagaTabsHtml +
        "</div></nav>" +
        "<div class='sow-campaign__workspace'>" +
        featuredHtml +
        "<section class='sow-menu__battlefield sow-campaign__battlefield sow-campaign__roster'>" +
        "<div class='sow-campaign__heading'><div><p class='sow-menu__eyebrow'>" +
        esc(campaignSagaTitle(activeSaga)) +
        "</p><h2>" +
        esc(SOW_t("lobbies.episodes")) +
        "</h2></div>" +
        continueButton +
        "</div>" +
        "<div class='sow-campaign__episodes'>" +
        (filteredEpisodes.length
            ? filteredEpisodes
                  .map((episode) => renderCampaignEpisode(episode, continueId, episode.id === selectedId))
                  .join("")
            : "<p class='sow-menu__empty'>" + esc(SOW_t("lobbies.campaign_unavailable")) + "</p>") +
        "</div>" +
        "</section>" +
        "</div>" +
        "</main>"
    );
}

export function renderBrowser() {
    return (
        "<main class='sow-menu__main sow-browser' data-screen-panel='browser'>" +
        "<section class='sow-menu__command sow-browser__header'>" +
        "<div class='sow-browser__title-bar'>" +
        "<button class='sow-menu__secondary sow-browser__back-btn' type='button' data-command='close_overlay'>← " +
        esc(SOW_t("lobbies.back")) +
        "</button>" +
        "<div class='sow-browser__titles'>" +
        "<p class='sow-menu__eyebrow'>" +
        esc(SOW_t("menu.lobby_browser")) +
        "</p>" +
        "<h1>" +
        esc(SOW_t("lobbies.active_matches")).replace("\n", " ") +
        "</h1>" +
        "</div>" +
        "</div>" +
        "<div class='sow-browser__toolbar'>" +
        "<div class='sow-menu__browser-search'>" +
        "<span class='sow-browser__search-icon' aria-hidden='true'>" +
        createTacticalIcon("search") +
        "</span>" +
        "<input data-role='browser-search' type='search' placeholder='" +
        esc(SOW_t("lobbies.search_map_host")) +
        "' value=\"" +
        esc(menuUi.browserSearchQuery) +
        "\" aria-label='" +
        esc(SOW_t("lobbies.search_map_host")) +
        "'>" +
        "</div>" +
        "<form class='sow-menu__join sow-browser__join' data-form='join'>" +
        "<input name='code' inputmode='numeric' autocomplete='off' placeholder='" +
        esc(SOW_t("menu.lobby_code")) +
        "' aria-label='" +
        esc(SOW_t("menu.lobby_code")) +
        "'>" +
        "<button type='submit'>" +
        esc(SOW_t("menu.join")) +
        "</button>" +
        "</form>" +
        "<button class='sow-menu__primary sow-browser__create-btn' type='button' data-command='open_create'>" +
        createTacticalIcon("plus") +
        " <span>" +
        esc(SOW_t("menu.create_custom_game")) +
        "</span></button>" +
        "</div>" +
        renderFeedback() +
        "</section>" +
        "<section class='sow-menu__battlefield sow-browser__workspace'>" +
        "<section class='sow-menu__public sow-browser__public'>" +
        "<div class='sow-menu__lobbies' data-lobby-list='browser'></div>" +
        "</section>" +
        "</section>" +
        "</main>"
    );
}

const CREATE_TACTICAL_ICONS: Record<string, string> = {
    ffa: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='m14.5 17.5-11-11V3h3.5l11 11'/><path d='m13 19 6-6'/><path d='m16 16 4 4'/><path d='m19 21 2-2'/><path d='m9.5 17.5 11-11V3H17L6 14'/><path d='m11 19-6-6'/><path d='m8 16-4 4'/><path d='m5 21-2-2'/></svg>",
    teams: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z'/><path d='M12 4v17'/><path d='M5 10h14'/></svg>",
    hvn: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='m2 4 3 12h14l3-12-6 7-4-7-4 7-6-7zm3 16h14'/></svg>",
    vanilla:
        "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M12 22s7-3.5 7-9.5V6l-7-3-7 3v6.5C5 18.5 12 22 12 22Z'/><path d='m9 12 2 2 4-4'/></svg>",
    terminator:
        "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M13 2 4 14h7l-1 8 9-12h-7l1-8Z'/></svg>",
    spawn_random:
        "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><rect x='3' y='3' width='18' height='18' rx='3'/><circle cx='8.5' cy='8.5' r='1' fill='currentColor'/><circle cx='15.5' cy='8.5' r='1' fill='currentColor'/><circle cx='12' cy='12' r='1' fill='currentColor'/><circle cx='8.5' cy='15.5' r='1' fill='currentColor'/><circle cx='15.5' cy='15.5' r='1' fill='currentColor'/></svg>",
    spawn_preset:
        "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M12 21s-6-5.3-6-10a6 6 0 1 1 12 0c0 4.7-6 10-6 10Z'/><circle cx='12' cy='11' r='2.5'/></svg>",
    public: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><circle cx='12' cy='12' r='9'/><path d='M3 12h18'/><path d='M12 3a14 14 0 0 1 0 18'/><path d='M12 3a14 14 0 0 0 0 18'/></svg>",
    private:
        "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><rect x='5' y='11' width='14' height='10' rx='2'/><path d='M8 11V7a4 4 0 0 1 8 0v4'/></svg>",
    solo: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><circle cx='12' cy='8' r='4'/><path d='M5 20a7 7 0 0 1 14 0'/></svg>",
    players:
        "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2'/><circle cx='9' cy='7' r='4'/><path d='M22 21v-2a4 4 0 0 0-3-3.87'/><path d='M16 3.13a4 4 0 0 1 0 7.75'/></svg>",
    tribes: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M3 20h18L12 4 3 20Z'/><path d='m9.5 20 2.5-6 2.5 6'/></svg>",
    nations:
        "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M4 21h16'/><path d='M6 21V8l3 2 3-4 3 4 3-2v13'/><path d='M10 21v-5h4v5'/></svg>",
    seed: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M10 3 8 21'/><path d='M16 3l-2 18'/><path d='M4 9h17'/><path d='M3 15h17'/></svg>",
    map: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><polygon points='3 6 9 3 15 6 21 3 21 18 15 21 9 18 3 21'/><line x1='9' y1='3' x2='9' y2='18'/><line x1='15' y1='6' x2='15' y2='21'/></svg>",
    roster: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M8 6h13'/><path d='M8 12h13'/><path d='M8 18h13'/><path d='M3 6h.01'/><path d='M3 12h.01'/><path d='M3 18h.01'/></svg>",
    prev: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='m15 18-6-6 6-6'/></svg>",
    next: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='m9 18 6-6-6-6'/></svg>",
    plus: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M12 5v14M5 12h14'/></svg>",
    minus: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M5 12h14'/></svg>",
    search: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><circle cx='11' cy='11' r='7'/><path d='m20 20-3.5-3.5'/></svg>",
    campaign:
        "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20'/></svg>",
    play: "<svg class='sow-create__svg-icon' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'><path d='M8 5v14l11-7z'/></svg>",
};

export function createTacticalIcon(name: string): string {
    return CREATE_TACTICAL_ICONS[name] || menuIcon(name) || "";
}

let createMapAnimDirection: -1 | 0 | 1 = 0;

function availableMapCatalog() {
    const catalog = (menuUi.state && menuUi.state.map_catalog) || [];
    if (catalog.length) return catalog;
    return [mapInfo((menuUi.createDraft && menuUi.createDraft.map_name) || "world")];
}

function applyCreateMapSelection(mapKey: string, direction: -1 | 1) {
    if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
    const draft = menuUi.createDraft;
    const map = mapInfo(mapKey);
    if (!map) return;
    draft.map_name = map.key;
    draft.map_width = map.width;
    draft.map_height = map.height;
    const presets = Array.isArray(map.roster_presets) ? map.roster_presets : [];
    if (!presets.some((preset) => preset.id === draft.map_roster_preset)) {
        draft.map_roster_preset = map.default_roster || (presets[0] && presets[0].id) || null;
    }
    createMapAnimDirection = direction;
}

export function cycleCreateMap(direction: number) {
    const catalog = availableMapCatalog();
    if (!catalog.length) return;
    if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
    const currentKey = menuUi.createDraft.map_name || "world";
    const currentIndex = Math.max(
        0,
        catalog.findIndex((m) => m.key === currentKey),
    );
    const step = direction < 0 ? -1 : 1;
    const nextIndex = (currentIndex + step + catalog.length) % catalog.length;
    applyCreateMapSelection(catalog[nextIndex].key, step);
}

export function selectCreateMap(mapKey: string) {
    if (!mapKey) return;
    const catalog = availableMapCatalog();
    if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
    const currentKey = menuUi.createDraft.map_name || "world";
    if (currentKey === mapKey) return;
    const prevIdx = catalog.findIndex((m) => m.key === currentKey);
    const nextIdx = catalog.findIndex((m) => m.key === mapKey);
    const dir: -1 | 1 = nextIdx >= prevIdx ? 1 : -1;
    applyCreateMapSelection(mapKey, dir);
}

export function stepCreateSlider(field: string, delta: number) {
    if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
    const draft = menuUi.createDraft;
    if (field === "max_players") {
        const cur = Number(draft.max_players) || 8;
        draft.max_players = Math.max(2, Math.min(16, cur + delta));
    } else if (field === "bot_count") {
        const maxTribes = botLimit("tribes", 400);
        const cur = draft.bot_count != null ? Number(draft.bot_count) : 128;
        draft.bot_count = Math.max(0, Math.min(maxTribes, cur + delta));
    } else if (field === "nation_count") {
        const maxNations = botLimit("nations", 200);
        const cur = draft.nation_count != null ? Number(draft.nation_count) : 32;
        draft.nation_count = Math.max(0, Math.min(maxNations, cur + delta));
    } else if (field === "seed") {
        const cur = Number(draft.seed) || 42;
        draft.seed = Math.max(1, Math.min(9999, cur + delta));
    }
}

function sliderFillPercent(val: number, min: number, max: number): number {
    if (max <= min) return 0;
    return Math.max(0, Math.min(100, Math.round(((val - min) / (max - min)) * 100)));
}

// One bot per catalog character: the sliders stop at the catalog size.
function botLimit(role: keyof MenuState["bot_limits"], fallback: number) {
    const limit = menuUi.state!.bot_limits && Number(menuUi.state!.bot_limits[role]);
    return limit > 0 ? limit : fallback;
}

export function cloneConfig(): MenuCustomGameConfig {
    return JSON.parse(JSON.stringify(menuUi.state!.custom_game_config || {}));
}

export function syncCreateDraft(form: HTMLFormElement) {
    if (!menuUi.createDraft) menuUi.createDraft = cloneConfig();
    const draft = menuUi.createDraft;
    const prevMap = draft.map_name || "world";
    Array.prototype.forEach.call(form.elements, function (field: Element) {
        if (!(field instanceof HTMLInputElement) || !field.name) return;
        if (field.name === "session_mode") {
            menuUi.createOffline = field.value === "offline";
        } else if (field.name === "visibility") {
            menuUi.createPrivate = field.value === "private";
        } else if (field.name === "password") {
            menuUi.createPassword = field.value;
        } else if (
            field.name === "bot_count" ||
            field.name === "nation_count" ||
            field.name === "max_players" ||
            field.name === "seed"
        ) {
            draft[field.name] = Number(field.value);
        } else if (field.name === "map_name") {
            if (field.value && field.value !== prevMap) {
                const catalog = availableMapCatalog();
                const prevIdx = catalog.findIndex((m) => m.key === prevMap);
                const nextIdx = catalog.findIndex((m) => m.key === field.value);
                createMapAnimDirection = nextIdx >= prevIdx ? 1 : -1;
            }
            draft.map_name = field.value;
            const map = mapInfo(field.value);
            if (map) {
                draft.map_width = map.width;
                draft.map_height = map.height;
            }
        } else if (field.name !== "visibility") {
            draft[field.name] = field.value;
        }
    });
    const selected = mapInfo(draft.map_name || "world");
    const presets = Array.isArray(selected.roster_presets) ? selected.roster_presets : [];
    if (
        !presets.some(function (preset) {
            return preset.id === draft.map_roster_preset;
        })
    ) {
        draft.map_roster_preset = selected.default_roster || null;
    }
}

export function renderPasswordModal() {
    if (menuUi.passwordLobbyId == null) return "";
    const lobby = findLobby(menuUi.passwordLobbyId);
    const title = lobby ? formatMapName(lobby) : SOW_t("lobbies.private_lobby");
    const error = menuUi.state!.error
        ? "<div class='sow-menu__status sow-menu__status--error'>" +
          esc(localizedText(menuUi.state!.error)) +
          "</div>"
        : "";
    return (
        "<div class='sow-menu__overlay' data-menu-overlay='password'><form class='sow-menu__modal sow-menu__password-modal' data-form='password' novalidate>" +
        "<div class='sow-menu__modal-head'><div><p class='sow-menu__panel-label'>" +
        esc(SOW_t("lobbies.password_required")) +
        "</p><h2>" +
        esc(title) +
        "</h2></div>" +
        "<button class='sow-menu__icon-button' type='button' data-command='close_password' aria-label='" +
        esc(SOW_t("menu.close")) +
        "'>×</button></div>" +
        "<p class='sow-menu__tagline'>" +
        esc(SOW_t("lobbies.private_lobby_hint")) +
        "</p>" +
        "<label class='sow-menu__form-field'>" +
        esc(SOW_t("lobbies.password")) +
        "<input class='sow-menu__field' name='password' type='password' autocomplete='current-password' value='" +
        esc(menuUi.passwordDraft) +
        "' autofocus></label>" +
        error +
        "<div class='sow-menu__modal-actions'><button class='sow-menu__ghost-button' type='button' data-command='close_password'>" +
        esc(SOW_t("lobbies.cancel")) +
        "</button><button class='sow-menu__primary' type='submit'>" +
        esc(SOW_t("lobbies.join_lobby")) +
        " <span>↗</span></button></div></form></div>"
    );
}

export function renderCreate() {
    const config = menuUi.createDraft || cloneConfig();
    menuUi.createDraft = config;
    const isSp = menuUi.createOffline;
    const selectedMap = mapInfo(config.map_name || "world");
    const mapThumbUrl = lobbyThumb({ map_name: selectedMap.key });
    const catalog = availableMapCatalog();
    const mapIndex = Math.max(
        0,
        catalog.findIndex((m) => m.key === selectedMap.key),
    );
    const animClass =
        createMapAnimDirection === 1
            ? " is-animating-enter-next"
            : createMapAnimDirection === -1
              ? " is-animating-enter-prev"
              : "";
    createMapAnimDirection = 0;

    const modeMeta: Record<string, { label: string; icon: string }> = {
        FFA: { label: SOW_t("lobbies.free_for_all"), icon: createTacticalIcon("ffa") },
        Teams: { label: SOW_t("lobbies.teams_two"), icon: createTacticalIcon("teams") },
        HumansVsNations: { label: SOW_t("lobbies.hvn"), icon: createTacticalIcon("hvn") },
    };

    const currentMode = config.game_mode || "FFA";
    const modeOptionsHtml = ["FFA", "Teams", "HumansVsNations"]
        .map(function (mode) {
            const meta = modeMeta[mode] || { label: mode, icon: "" };
            const isSelected = currentMode === mode;
            return (
                "<button type='button' class='sow-menu__pill sow-create__mode-card" +
                (isSelected ? " active" : "") +
                "' data-command='set_create_mode' data-mode='" +
                mode +
                "' aria-pressed='" +
                String(isSelected) +
                "'>" +
                "<span class='sow-create__mode-icon'>" +
                meta.icon +
                "</span>" +
                "<span class='sow-create__mode-label'>" +
                esc(meta.label) +
                "</span>" +
                "</button>"
            );
        })
        .join("");

    const currentDiff = config.bot_difficulty || "Vanilla";
    const diffOptionsHtml = ["Vanilla", "Terminator"]
        .map(function (diff) {
            const isSelected = currentDiff === diff;
            const icon = createTacticalIcon(diff === "Vanilla" ? "vanilla" : "terminator");
            const label = diff === "Vanilla" ? SOW_t("lobbies.vanilla") : SOW_t("lobbies.terminator");
            return (
                "<button type='button' class='sow-menu__pill sow-create__toggle-pill" +
                (isSelected ? " active" : "") +
                "' data-command='set_create_diff' data-diff='" +
                diff +
                "' aria-pressed='" +
                String(isSelected) +
                "'>" +
                icon +
                "<span>" +
                esc(label) +
                "</span>" +
                "</button>"
            );
        })
        .join("");

    const currentSpawn = config.random_spawn !== false;
    const spawnOptionsHtml = [true, false]
        .map(function (val) {
            const isSelected = currentSpawn === val;
            const icon = createTacticalIcon(val ? "spawn_random" : "spawn_preset");
            const label = val ? SOW_t("lobbies.random_on") : SOW_t("lobbies.preset_off");
            return (
                "<button type='button' class='sow-menu__pill sow-create__toggle-pill" +
                (isSelected ? " active" : "") +
                "' data-command='set_create_spawn' data-spawn='" +
                String(val) +
                "' aria-pressed='" +
                String(isSelected) +
                "'>" +
                icon +
                "<span>" +
                esc(label) +
                "</span>" +
                "</button>"
            );
        })
        .join("");

    const visOptionsHtml = [false, true]
        .map(function (val) {
            const isSelected = menuUi.createPrivate === val;
            const icon = createTacticalIcon(val ? "private" : "public");
            const label = val ? SOW_t("lobbies.private_code") : SOW_t("lobbies.public");
            return (
                "<button type='button' class='sow-menu__pill sow-create__toggle-pill" +
                (isSelected ? " active" : "") +
                "' data-command='set_create_private' data-private='" +
                String(val) +
                "' aria-pressed='" +
                String(isSelected) +
                "'>" +
                icon +
                "<span>" +
                esc(label) +
                "</span>" +
                "</button>"
            );
        })
        .join("");

    const mapCatalogOptions = catalog.map(function (m) {
        return { value: m.key, label: formatMapName(m) + " (" + m.width + "×" + m.height + ")" };
    });

    const mapThumbsStripHtml =
        catalog.length > 1
            ? "<div class='sow-create__map-strip' role='listbox' aria-label='" +
              esc(SOW_t("lobbies.select_map")) +
              "'>" +
              catalog
                  .map(function (m) {
                      const active = m.key === selectedMap.key;
                      const thumb = lobbyThumb({ map_name: m.key });
                      return (
                          "<button type='button' role='option' aria-selected='" +
                          String(active) +
                          "' class='sow-create__map-thumb" +
                          (active ? " is-active" : "") +
                          "' data-command='select_create_map' data-map-key='" +
                          esc(m.key) +
                          "' title='" +
                          esc(formatMapName(m)) +
                          "'>" +
                          "<img src='" +
                          esc(thumb) +
                          "' alt='' loading='lazy' decoding='async'>" +
                          "<span>" +
                          esc(formatMapName(m)) +
                          "</span>" +
                          "</button>"
                      );
                  })
                  .join("") +
              "</div>"
            : "";

    const rosterPresets = Array.isArray(selectedMap.roster_presets) ? selectedMap.roster_presets : [];
    if (
        !rosterPresets.some(function (preset) {
            return preset.id === config.map_roster_preset;
        })
    ) {
        config.map_roster_preset =
            selectedMap.default_roster || (rosterPresets[0] && rosterPresets[0].id) || null;
    }
    const rosterControl = rosterPresets.length
        ? "<label class='sow-menu__form-field sow-create__map-select sow-create__roster-select'>" +
          "<span class='sow-create__field-chip'>" +
          createTacticalIcon("roster") +
          "<span>Roster</span></span>" +
          SOW_renderDropdown({
              key: "create-map-roster",
              name: "map_roster_preset",
              value: config.map_roster_preset || selectedMap.default_roster || "",
              options: rosterPresets.map(function (preset) {
                  return {
                      value: preset.id,
                      label: preset.name + (preset.id === selectedMap.default_roster ? " *" : ""),
                  };
              }),
          }) +
          "</label>"
        : "";

    const seedVal = Number(config.seed) || 42;
    const seedPct = sliderFillPercent(seedVal, 1, 9999);

    const spControls = isSp
        ? "<div class='sow-create__session-bar sow-create__session-bar--seed'>" +
          "<div class='sow-create__force-header'>" +
          "<span class='sow-create__force-title'>" +
          createTacticalIcon("seed") +
          "<span>" +
          esc(SOW_t("lobbies.procedural_seed")) +
          "</span></span>" +
          "<b class='sow-create__force-val' data-val-for='seed'>" +
          esc(seedVal) +
          "</b>" +
          "</div>" +
          "<div class='sow-menu__slider-row sow-create__seed-row'>" +
          "<input class='sow-menu__range' name='seed' type='range' min='1' max='9999' step='1' value='" +
          esc(seedVal) +
          "' style='--sow-slider-fill:" +
          seedPct +
          "%'>" +
          "<button class='sow-menu__ghost-mini sow-create__dice-btn' type='button' data-command='randomize_seed' aria-label='" +
          esc(SOW_t("lobbies.randomize_seed")) +
          "' title='" +
          esc(SOW_t("lobbies.randomize_seed")) +
          "'>" +
          menuIcon("dice") +
          createTacticalIcon("spawn_random") +
          "</button>" +
          "</div>" +
          "</div>"
        : "<div class='sow-create__session-bar sow-create__session-bar--online'>" +
          "<div class='sow-create__rule-block'>" +
          "<span class='sow-create__rule-label'>" +
          createTacticalIcon(menuUi.createPrivate ? "private" : "public") +
          "<span>" +
          esc(SOW_t("lobbies.visibility")) +
          "</span></span>" +
          "<div class='sow-menu__pill-group sow-create__segmented'>" +
          visOptionsHtml +
          "</div>" +
          "</div>" +
          (menuUi.createPrivate
              ? "<label class='sow-menu__form-field sow-create__password-inline'>" +
                "<span class='sow-create__password-icon'>" +
                createTacticalIcon("private") +
                "</span>" +
                "<input class='sow-menu__field' name='password' type='password' autocomplete='new-password' value='" +
                esc(menuUi.createPassword) +
                "' placeholder='" +
                esc(SOW_t("lobbies.password_optional")) +
                "' aria-label='" +
                esc(SOW_t("lobbies.password_optional")) +
                "'>" +
                "</label>"
              : "") +
          "</div>";

    const maxPlayersVal = Number(config.max_players) || 8;
    const maxPlayersPct = sliderFillPercent(maxPlayersVal, 2, 16);

    const maxTribesLimit = botLimit("tribes", 400);
    const botCountVal = config.bot_count != null ? Number(config.bot_count) : 128;
    const botCountPct = sliderFillPercent(botCountVal, 0, maxTribesLimit);

    const maxNationsLimit = botLimit("nations", 200);
    const nationCountVal = config.nation_count != null ? Number(config.nation_count) : 32;
    const nationCountPct = sliderFillPercent(nationCountVal, 0, maxNationsLimit);

    const totalEntities = maxPlayersVal + botCountVal + nationCountVal;
    const activeModeMeta = modeMeta[currentMode] || modeMeta.FFA;

    return (
        "<main class='sow-menu__main sow-menu__main--custom sow-create' data-screen-panel='create'>" +
        "<section class='sow-menu__command sow-create__rail'>" +
        "<div class='sow-create__header-left'>" +
        "<button class='sow-menu__secondary sow-create__back-btn' type='button' data-command='close_overlay' aria-label='" +
        esc(SOW_t("lobbies.back")) +
        "'>← <span>" +
        esc(SOW_t("lobbies.back")) +
        "</span></button>" +
        "<div class='sow-create__title-group'>" +
        "<p class='sow-menu__eyebrow'>" +
        esc(SOW_t("lobbies.custom_game")) +
        "</p>" +
        "<h1>" +
        esc(SOW_t("lobbies.match_settings")).replace("\n", " ") +
        "</h1>" +
        "</div>" +
        "</div>" +
        "<div class='sow-create__header-right'>" +
        "<div class='sow-menu__mode-switcher' role='tablist' aria-label='" +
        esc(SOW_t("lobbies.custom_game")) +
        "'>" +
        "<button type='button' role='tab' aria-selected='" +
        String(isSp) +
        "' class='sow-menu__mode-tab" +
        (isSp ? " active" : "") +
        "' data-command='set_session_mode' data-mode='offline'>" +
        createTacticalIcon("solo") +
        " <span>" +
        esc(SOW_t("lobbies.solo_practice")) +
        "</span></button>" +
        "<button type='button' role='tab' aria-selected='" +
        String(!isSp) +
        "' class='sow-menu__mode-tab" +
        (!isSp ? " active" : "") +
        "' data-command='set_session_mode' data-mode='online'>" +
        createTacticalIcon("public") +
        " <span>" +
        esc(SOW_t("lobbies.online_lobby")) +
        "</span></button>" +
        "</div>" +
        "<button type='button' class='sow-menu__secondary sow-create__campaign-shortcut' data-command='open_campaign' title='" +
        esc(SOW_t("menu.campaign")) +
        "'>" +
        createTacticalIcon("campaign") +
        " <span>" +
        esc(SOW_t("menu.campaign")) +
        "</span></button>" +
        "</div>" +
        "</section>" +
        "<section class='sow-menu__battlefield sow-create__workspace'>" +
        "<form class='sow-create__form' data-form='create'>" +
        "<div class='sow-create__scroll'>" +
        "<div class='sow-create__columns'>" +
        "<div class='sow-create__column sow-create__column--stage'>" +
        "<section class='sow-menu__custom-card sow-create__map-panel'>" +
        "<div class='sow-menu__map-preview-wrap" +
        animClass +
        "' data-create-map-stage style=\"background-image:url('" +
        esc(mapThumbUrl) +
        "')\">" +
        "<img class='sow-create__map-art' src='" +
        esc(mapThumbUrl) +
        "' alt='" +
        esc(formatMapName(selectedMap)) +
        "' loading='eager' decoding='async'>" +
        "<div class='sow-create__map-vignette' aria-hidden='true'></div>" +
        "<div class='sow-create__map-hud-top'>" +
        "<span class='sow-create__hud-chip sow-create__hud-chip--mode'>" +
        activeModeMeta.icon +
        "<span>" +
        esc(activeModeMeta.label) +
        "</span></span>" +
        "<span class='sow-create__hud-chip sow-create__hud-chip--forces' data-create-total-forces>" +
        createTacticalIcon("players") +
        "<b>" +
        totalEntities +
        "</b></span>" +
        "</div>" +
        (catalog.length > 1
            ? "<button type='button' class='sow-create__map-nav sow-create__map-nav--prev' data-command='cycle_create_map' data-dir='-1' aria-label='Previous map'>" +
              createTacticalIcon("prev") +
              "</button>" +
              "<button type='button' class='sow-create__map-nav sow-create__map-nav--next' data-command='cycle_create_map' data-dir='1' aria-label='Next map'>" +
              createTacticalIcon("next") +
              "</button>"
            : "") +
        "<div class='sow-menu__map-preview-meta'>" +
        "<div class='sow-create__map-title-block'>" +
        "<strong>" +
        esc(formatMapName(selectedMap)) +
        "</strong>" +
        (catalog.length > 1
            ? "<span class='sow-create__map-counter'>" + (mapIndex + 1) + " / " + catalog.length + "</span>"
            : "") +
        "</div>" +
        "<small>" +
        esc(SOW_t("lobbies.tiles", { width: selectedMap.width, height: selectedMap.height })) +
        "</small>" +
        "</div>" +
        "</div>" +
        mapThumbsStripHtml +
        "<div class='sow-create__map-select-row" +
        (rosterControl ? " has-roster" : "") +
        "'>" +
        "<label class='sow-menu__form-field sow-create__map-select'>" +
        "<span class='sow-create__field-chip'>" +
        createTacticalIcon("map") +
        "<span>" +
        esc(SOW_t("lobbies.select_map")) +
        "</span></span>" +
        SOW_renderDropdown({
            key: "create-map",
            name: "map_name",
            value: selectedMap.key,
            options: mapCatalogOptions,
        }) +
        "</label>" +
        rosterControl +
        "</div>" +
        "</section>" +
        "</div>" +
        "<div class='sow-create__column sow-create__column--controls'>" +
        "<section class='sow-menu__custom-card sow-create__rules-card'>" +
        "<div class='sow-menu__pill-group sow-create__mode-grid' role='group' aria-label='" +
        esc(SOW_t("lobbies.game_type")) +
        "'>" +
        modeOptionsHtml +
        "</div>" +
        "<div class='sow-menu__form-row sow-create__rules-row'>" +
        "<div class='sow-create__rule-block'>" +
        "<span class='sow-create__rule-label'>" +
        createTacticalIcon(currentDiff === "Vanilla" ? "vanilla" : "terminator") +
        "<span>" +
        esc(SOW_t("lobbies.bot_difficulty")) +
        "</span></span>" +
        "<div class='sow-menu__pill-group sow-create__segmented'>" +
        diffOptionsHtml +
        "</div>" +
        "</div>" +
        "<div class='sow-create__rule-block'>" +
        "<span class='sow-create__rule-label'>" +
        createTacticalIcon(currentSpawn ? "spawn_random" : "spawn_preset") +
        "<span>" +
        esc(SOW_t("lobbies.spawn_rules")) +
        "</span></span>" +
        "<div class='sow-menu__pill-group sow-create__segmented'>" +
        spawnOptionsHtml +
        "</div>" +
        "</div>" +
        "</div>" +
        spControls +
        "</section>" +
        "<section class='sow-menu__custom-card sow-create__forces-card'>" +
        "<div class='sow-menu__slider-field sow-create__force-row'>" +
        "<div class='sow-menu__slider-label sow-create__force-header'>" +
        "<span class='sow-create__force-title'>" +
        "<i class='sow-create__force-badge sow-create__force-badge--players'>" +
        createTacticalIcon("players") +
        "</i>" +
        "<span>" +
        esc(SOW_t("lobbies.max_human_players")) +
        "</span></span>" +
        "<b class='sow-create__force-val' data-val-for='max_players'>" +
        maxPlayersVal +
        "</b></div>" +
        "<div class='sow-create__slider-control'>" +
        "<button type='button' class='sow-create__step-btn' data-command='step_create_slider' data-field='max_players' data-step='-1' aria-label='-'>" +
        createTacticalIcon("minus") +
        "</button>" +
        "<input class='sow-menu__range' name='max_players' type='range' min='2' max='16' step='1' value='" +
        maxPlayersVal +
        "' style='--sow-slider-fill:" +
        maxPlayersPct +
        "%'>" +
        "<button type='button' class='sow-create__step-btn' data-command='step_create_slider' data-field='max_players' data-step='1' aria-label='+'>" +
        createTacticalIcon("plus") +
        "</button>" +
        "</div>" +
        "</div>" +
        "<div class='sow-menu__slider-field sow-create__force-row'>" +
        "<div class='sow-menu__slider-label sow-create__force-header'>" +
        "<span class='sow-create__force-title'>" +
        "<i class='sow-create__force-badge sow-create__force-badge--tribes'>" +
        createTacticalIcon("tribes") +
        "</i>" +
        "<span>" +
        esc(SOW_t("lobbies.neutral_bots")) +
        "</span></span>" +
        "<b class='sow-create__force-val' data-val-for='bot_count'>" +
        botCountVal +
        "</b></div>" +
        "<div class='sow-create__slider-control'>" +
        "<button type='button' class='sow-create__step-btn' data-command='step_create_slider' data-field='bot_count' data-step='-8' aria-label='-'>" +
        createTacticalIcon("minus") +
        "</button>" +
        "<input class='sow-menu__range' name='bot_count' type='range' min='0' max='" +
        maxTribesLimit +
        "' step='8' value='" +
        botCountVal +
        "' style='--sow-slider-fill:" +
        botCountPct +
        "%'>" +
        "<button type='button' class='sow-create__step-btn' data-command='step_create_slider' data-field='bot_count' data-step='8' aria-label='+'>" +
        createTacticalIcon("plus") +
        "</button>" +
        "</div>" +
        "</div>" +
        "<div class='sow-menu__slider-field sow-create__force-row'>" +
        "<div class='sow-menu__slider-label sow-create__force-header'>" +
        "<span class='sow-create__force-title'>" +
        "<i class='sow-create__force-badge sow-create__force-badge--nations'>" +
        createTacticalIcon("nations") +
        "</i>" +
        "<span>" +
        esc(SOW_t("lobbies.complex_ai")) +
        "</span></span>" +
        "<b class='sow-create__force-val' data-val-for='nation_count'>" +
        nationCountVal +
        "</b></div>" +
        "<div class='sow-create__slider-control'>" +
        "<button type='button' class='sow-create__step-btn' data-command='step_create_slider' data-field='nation_count' data-step='-4' aria-label='-'>" +
        createTacticalIcon("minus") +
        "</button>" +
        "<input class='sow-menu__range' name='nation_count' type='range' min='0' max='" +
        maxNationsLimit +
        "' step='4' value='" +
        nationCountVal +
        "' style='--sow-slider-fill:" +
        nationCountPct +
        "%'>" +
        "<button type='button' class='sow-create__step-btn' data-command='step_create_slider' data-field='nation_count' data-step='4' aria-label='+'>" +
        createTacticalIcon("plus") +
        "</button>" +
        "</div>" +
        "</div>" +
        "</section>" +
        renderFeedback() +
        "</div>" +
        "</div>" +
        "</div>" +
        "<div class='sow-create__actions'>" +
        "<button class='sow-menu__primary sow-menu__custom-launch-btn' type='submit'>" +
        "<span>" +
        esc(SOW_t(isSp ? "lobbies.start_simulation" : "lobbies.create_lobby")) +
        "</span> <i>▶</i>" +
        "</button>" +
        "<button class='sow-menu__ghost-button sow-create__cancel-btn' type='button' data-command='close_overlay'>" +
        esc(SOW_t("lobbies.cancel")) +
        "</button>" +
        "</div>" +
        "</form>" +
        "</section>" +
        "</main>"
    );
}

export function joinedLobby() {
    const id = menuUi.state!.joined_lobby_id || menuUi.state!.pending_lobby_id;
    return (
        (menuUi.state!.lobbies || []).find(function (lobby) {
            return lobby.id === id;
        }) || null
    );
}

function renderQueuePlayerRow(player: MenuLobbyPlayer, lobby: MenuLobby | null) {
    const avatarUrl = avatarImage(player.leader);
    const isMe = player.player_id === menuUi.state!.my_player_id;
    const isHost = lobby && lobby.host_name === player.name;
    const canModerate = lobby && lobby.kind === "Custom" && menuUi.state!.is_lobby_host && !isMe;

    const status = queuePlayerStatus(player);
    const statusBadge =
        "<span class='sow-menu__sync-badge " +
        status.className +
        "'>" +
        esc(status.label) +
        "</span>";

    const controls = canModerate
        ? "<div class='sow-menu__player-mod-actions'>" +
          (lobby.game_mode === "Teams"
              ? "<button type='button' class='sow-menu__mod-btn' data-command='move_player_team' data-lobby-id='" +
                lobby.id +
                "' data-player-id='" +
                player.player_id +
                "'>" +
                esc(SOW_t("lobbies.move_team")) +
                "</button>"
              : "") +
          "<button type='button' class='sow-menu__mod-btn' data-command='kick_player' data-lobby-id='" +
          lobby.id +
          "' data-player-id='" +
          player.player_id +
          "'>" +
          esc(SOW_t("lobbies.kick")) +
          "</button>" +
          "<button type='button' class='sow-menu__mod-btn danger' data-command='ban_player' data-lobby-id='" +
          lobby.id +
          "' data-player-id='" +
          player.player_id +
          "'>" +
          esc(SOW_t("lobbies.ban")) +
          "</button>" +
          "</div>"
        : "";

    return (
        "" +
        "<div class='sow-menu__roster-row" +
        (isMe ? " is-me" : "") +
        "' data-player-id='" +
        player.player_id +
        "'>" +
        "<div class='sow-menu__roster-left'>" +
        "<div class='sow-menu__roster-avatar' style=\"" +
        avatarBackgroundCss(avatarUrl) +
        "\"></div>" +
        "<div class='sow-menu__roster-name-group'>" +
        "<strong>" +
        esc(player.name) +
        "</strong>" +
        (isMe ? "<small class='sow-menu__me-tag'>" + esc(SOW_t("lobbies.you")) + "</small>" : "") +
        (isHost ? "<small class='sow-menu__host-tag'>" + esc(SOW_t("lobbies.host")) + "</small>" : "") +
        "</div>" +
        "</div>" +
        "<div class='sow-menu__roster-right'>" +
        statusBadge +
        controls +
        "</div>" +
        "</div>"
    );
}

function queuePlayerStatus(player: MenuLobbyPlayer) {
    const ready = player.download_progress === 100 || player.is_ready;
    return {
        className: ready ? "ready" : "syncing",
        label: ready
            ? SOW_t("lobbies.ready")
            : SOW_t("lobbies.sync", { progress: player.download_progress || 0 }),
    };
}

function queueRosterKey(lobby: MenuLobby | null) {
    if (!lobby) return "";
    return JSON.stringify({
        id: lobby.id,
        mode: lobby.game_mode,
        kind: lobby.kind,
        host: lobby.host_name,
        owner: menuUi.state!.is_lobby_host,
        me: menuUi.state!.my_player_id,
        players: (lobby.players || []).map(function (player) {
            return [player.player_id, player.name, player.leader, player.team];
        }),
    });
}

function renderQueueRoster(lobby: MenuLobby | null) {
    const players = (lobby && lobby.players) || [];
    if (!players.length) return "";

    if (lobby && lobby.game_mode === "Teams") {
        const redPlayers = players.filter(function (player) {
            return player.team === "Red";
        });
        const bluePlayers = players.filter(function (player) {
            return player.team !== "Red";
        });
        return (
            "" +
            "<div class='sow-menu__teams-roster'>" +
            "<div class='sow-menu__team-col sow-menu__team-col--red'>" +
            "<div class='sow-menu__team-header'><span class='sow-menu__team-dot sow-menu__team-dot--red'></span>" +
            esc(SOW_t("lobbies.red_team", { count: redPlayers.length })) +
            "</div>" +
            "<div class='sow-menu__team-list'>" +
            redPlayers
                .map(function (player) {
                    return renderQueuePlayerRow(player, lobby);
                })
                .join("") +
            "</div>" +
            "</div>" +
            "<div class='sow-menu__team-col sow-menu__team-col--blue'>" +
            "<div class='sow-menu__team-header'><span class='sow-menu__team-dot sow-menu__team-dot--blue'></span>" +
            esc(SOW_t("lobbies.blue_team", { count: bluePlayers.length })) +
            "</div>" +
            "<div class='sow-menu__team-list'>" +
            bluePlayers
                .map(function (player) {
                    return renderQueuePlayerRow(player, lobby);
                })
                .join("") +
            "</div>" +
            "</div>" +
            "</div>"
        );
    }

    const rows = players
        .map(function (player) {
            return renderQueuePlayerRow(player, lobby);
        })
        .join("");
    return players.length < 2
        ? "<div class='sow-menu__ffa-roster'>" + rows + "</div>"
        : "<div class='sow-menu__ffa-roster sow-menu__ffa-roster--split'>" + rows + "</div>";
}

export function updateQueueRoster(lobby: MenuLobby | null) {
    const panel = activeScreenPanel();
    if (!(panel instanceof HTMLElement) || panel.dataset.screenPanel !== "queue") return;

    const roster = panel.querySelector<HTMLElement>("[data-queue-roster]");
    if (roster) {
        const key = queueRosterKey(lobby);
        if (roster.dataset.rosterKey !== key) {
            const scrollTop = roster.scrollTop;
            roster.innerHTML = renderQueueRoster(lobby);
            roster.dataset.rosterKey = key;
            roster.scrollTop = scrollTop;
        }
        const playersById = new Map<number, MenuLobbyPlayer>(
            (lobby ? lobby.players : []).map(function (player): [number, MenuLobbyPlayer] {
                return [player.player_id, player];
            }),
        );
        roster
            .querySelectorAll<HTMLElement>(".sow-menu__roster-row[data-player-id]")
            .forEach(function (row) {
                const player = playersById.get(Number(row.dataset.playerId));
                const badge = row.querySelector<HTMLElement>(".sow-menu__sync-badge");
                if (!player || !badge) return;
                const status = queuePlayerStatus(player);
                const className = "sow-menu__sync-badge " + status.className;
                if (badge.className !== className) badge.className = className;
                if (badge.textContent !== status.label) badge.textContent = status.label;
            });
    }

    const count = panel.querySelector("[data-queue-player-count]");
    if (count) {
        count.textContent = lobby
            ? SOW_t("lobbies.players_capacity", {
                  players: lobby.num_players || 0,
                  max: lobby.max_players || 8,
              })
            : "—";
    }
}

export function updateQueueDownloadProgress() {
    const panel = activeScreenPanel();
    const state = menuUi.state;
    if (!panel || !state || !state.downloading_map) return;
    const text = panel.querySelector<HTMLElement>(".sow-menu__map-download-text");
    const fill = panel.querySelector<HTMLElement>(".sow-menu__map-download-fill");
    if (!text && !fill) return;

    const lobby = joinedLobby();
    const progress = state.map_download_progress || 0;
    const label = SOW_t("lobbies.downloading_map", {
        map: lobby ? formatMapName(lobby).toUpperCase() : SOW_t("lobbies.matchmaking"),
        progress,
    });
    if (text && text.textContent !== label) text.textContent = label;
    if (fill && fill.style.width !== progress + "%") fill.style.width = progress + "%";
}

export function updateRankedQueue() {
    const panel = activeScreenPanel();
    const state = menuUi.state;
    if (!panel || !state) return;
    const timer = panel.querySelector<HTMLElement>("[data-ranked-elapsed]");
    if (timer)
        timer.textContent = SOW_t("lobbies.ranked_searching", {
            seconds: state.ranked_search_elapsed_secs || 0,
        });
    const status = panel.querySelector<HTMLElement>("[data-ranked-sync]");
    if (status)
        status.textContent = state.map_download_error
            ? SOW_t("lobbies.map_download_failed")
            : state.downloading_map
              ? SOW_t("lobbies.sync", { progress: state.map_download_progress || 0 })
              : SOW_t("menu.loading");
    const retry = panel.querySelector<HTMLButtonElement>("[data-command=retry_map_download]");
    if (retry) retry.hidden = !state.map_download_error;
    const progress = panel.querySelector<HTMLProgressElement>("[data-ranked-download]");
    if (progress) {
        progress.value = state.map_download_progress || 0;
        progress.hidden = !state.downloading_map;
    }
    const label = panel.querySelector<HTMLElement>(".sow-ranked__player > span:last-child");
    if (label) label.textContent = rankLabel(menuUi.rankedRating);
    const badge = panel.querySelector<HTMLElement>(".sow-ranked__player .sow-ranked__emblem");
    const rating = menuUi.rankedRating;
    const tier = rating?.placements_complete ? rating.tier : "unranked";
    const division = rating?.placements_complete ? rating.division : null;
    if (
        badge &&
        (badge.dataset.tier !== rankTier(tier) ||
            (badge.querySelector("b")?.textContent || "") !== (division || ""))
    )
        badge.outerHTML = rankEmblem(tier, division);
}

export function renderQueue() {
    const lobby = joinedLobby();
    const rankedSearch = !!menuUi.state!.ranked_search_active && !lobby;
    if (rankedSearch || lobby?.kind === "Ranked1v1") {
        return (
            "<main class='sow-menu__main sow-menu__main--ranked' data-screen-panel='queue'>" +
            renderRankedDuel(lobby) +
            "<footer class='sow-ranked__queue-footer'>" +
            (lobby
                ? "<span data-ranked-sync role='status'></span><progress data-ranked-download max='100' aria-label='" +
                  esc(SOW_t("menu.loading")) +
                  "'></progress><button type='button' class='sow-menu__secondary' data-command='retry_map_download' hidden>" +
                  esc(SOW_t("lobbies.map_download_retry")) +
                  "</button>"
                : "<span data-ranked-elapsed>" +
                  esc(
                      SOW_t("lobbies.ranked_searching", {
                          seconds: menuUi.state!.ranked_search_elapsed_secs || 0,
                      }),
                  ) +
                  "</span>") +
            "<button type='button' class='sow-menu__secondary' data-command='leave_lobby'>" +
            esc(SOW_t("lobbies.cancel")) +
            "</button></footer></main>"
        );
    }
    const elapsed = menuUi.state!.ranked_search_elapsed_secs || 0;
    const title = lobby
        ? formatMapName(lobby).toUpperCase()
        : rankedSearch
          ? SOW_t("lobbies.ranked_1v1")
          : SOW_t("lobbies.matchmaking");
    const mapThumbUrl = lobbyThumb(lobby || { map_name: "world" });
    const isCustom = lobby && lobby.kind === "Custom";
    const isHost = menuUi.state && menuUi.state.is_lobby_host && isCustom;

    const modeClass = "sow-menu__mode-chip--" + (lobby ? lobby.game_mode.toLowerCase() : "ffa");
    const modeLabel = lobby
        ? lobby.game_mode === "Teams"
            ? SOW_t("lobbies.teams_red_blue")
            : gameModeLabel(lobby.game_mode)
        : rankedSearch
          ? SOW_t("lobbies.ranked_1v1")
          : SOW_t("lobbies.matchmaking");

    let feedback = "";
    if (menuUi.state!.map_download_error) {
        feedback =
            "<div class='sow-menu__queue-feedback sow-menu__queue-feedback--error'>" +
            esc(SOW_t("lobbies.map_download_failed")) +
            " " +
            "<button class='sow-menu__secondary' type='button' data-command='retry_map_download'>" +
            esc(SOW_t("lobbies.map_download_retry")) +
            "</button></div>";
    } else if (menuUi.state!.downloading_map) {
        const pct = menuUi.state!.map_download_progress || 0;
        feedback =
            "" +
            "<div class='sow-menu__map-download-wrap'>" +
            "<div class='sow-menu__map-download-text'>" +
            esc(
                SOW_t("lobbies.downloading_map", {
                    map: title,
                    progress: pct,
                }),
            ) +
            "</div>" +
            "<div class='sow-menu__map-download-bar'><div class='sow-menu__map-download-fill' style='width:" +
            pct +
            "%'></div></div>" +
            "</div>";
    } else if (menuUi.state!.error) {
        feedback =
            "<div class='sow-menu__queue-feedback sow-menu__queue-feedback--error'>" +
            esc(localizedText(menuUi.state!.error)) +
            "</div>";
    } else if (rankedSearch) {
        feedback =
            "<div class='sow-menu__queue-countdown'>" +
            esc(SOW_t("lobbies.ranked_searching", { seconds: elapsed })) +
            "</div>";
    }

    const rosterKey = queueRosterKey(lobby);
    const rosterHtml = renderQueueRoster(lobby);

    return (
        "<main class='sow-menu__main sow-menu__main--queue' data-screen-panel='queue'>" +
        "<section class='sow-menu__queue-summary-card'>" +
        "<div class='sow-menu__queue-map-hero' style=\"background-image:url('" +
        esc(mapThumbUrl) +
        "')\">" +
        "<div class='sow-menu__queue-map-overlay'>" +
        "<span class='sow-menu__mode-chip " +
        modeClass +
        "'>" +
        esc(modeLabel) +
        "</span>" +
        "<h3>" +
        esc(title) +
        "</h3>" +
        "</div>" +
        "</div>" +
        "<div class='sow-menu__queue-countdown' data-live-countdown></div>" +
        feedback +
        "<div class='sow-menu__queue-info-table'>" +
        (lobby && lobby.host_name
            ? "<div class='sow-menu__info-row' data-info='host'><span>" +
              esc(SOW_t("lobbies.host")) +
              "</span><strong>" +
              esc(lobby.host_name) +
              "</strong></div>"
            : "") +
        (isCustom && lobby
            ? "<div class='sow-menu__info-row' data-info='code'><span>" +
              esc(SOW_t("lobbies.room_code")) +
              "</span><div class='sow-menu__code-box'><strong>" +
              lobby.id +
              "</strong><button type='button' data-command='copy_lobby_code' data-lobby-id='" +
              lobby.id +
              "'>" +
              esc(SOW_t("lobbies.copy")) +
              "</button></div></div>"
            : "") +
        (lobby && lobby.bot_count > 0
            ? "<div class='sow-menu__info-row' data-info='bots'><span>" +
              esc(SOW_t("lobbies.tribes")) +
              "</span><strong>" +
              lobby.bot_count +
              " (" +
              esc(lobby.bot_difficulty || SOW_t("lobbies.vanilla")) +
              ")</strong></div>"
            : "") +
        (lobby && lobby.nation_count > 0
            ? "<div class='sow-menu__info-row' data-info='nations'><span>" +
              esc(SOW_t("lobbies.nations")) +
              "</span><strong>" +
              lobby.nation_count +
              "</strong></div>"
            : "") +
        (lobby && lobby.has_password
            ? "<div class='sow-menu__info-row' data-info='access'><span>" +
              esc(SOW_t("lobbies.access")) +
              "</span><strong class='sow-menu__lock-tag'>" +
              menuIcon("lock") +
              " " +
              esc(SOW_t("lobbies.password")) +
              "</strong></div>"
            : "") +
        "</div>" +
        "<div class='sow-menu__queue-action-bar'>" +
        (isHost
            ? "<button class='sow-menu__primary sow-menu__queue-start-btn' type='button' data-command='start_private' data-lobby-id='" +
              lobby.id +
              "'>" +
              esc(SOW_t("lobbies.start_game")) +
              " <span>↗</span></button>"
            : "") +
        "<button class='sow-menu__danger sow-menu__queue-leave-btn' type='button' data-command='leave_lobby'>" +
        esc(SOW_t(rankedSearch ? "lobbies.cancel" : "lobbies.leave_lobby")) +
        " <span>×</span></button>" +
        "</div>" +
        "</section>" +
        "<section class='sow-menu__queue-players-card'>" +
        "<div class='sow-menu__queue-players-head'>" +
        "<p class='sow-menu__panel-label'>" +
        esc(SOW_t("lobbies.players")) +
        "</p>" +
        "<span class='sow-menu__player-count-badge' data-queue-player-count>" +
        (lobby
            ? SOW_t("lobbies.players_capacity", {
                  players: lobby.num_players || 0,
                  max: lobby.max_players || 8,
              })
            : "—") +
        "</span>" +
        "</div>" +
        "<div class='sow-menu__queue-roster-wrap' data-queue-roster data-roster-key='" +
        esc(rosterKey) +
        "'>" +
        (rankedSearch
            ? "<div class='sow-menu__queue-countdown'>" + esc(SOW_t("lobbies.waiting_for_players")) + "</div>"
            : rosterHtml) +
        "</div>" +
        "</section>" +
        "</main>"
    );
}
