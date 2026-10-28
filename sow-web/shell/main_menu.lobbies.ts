import type { CampaignDefinition } from "./campaign/types.ts";
import {
    asset,
    avatarImage,
    esc,
    findLobby,
    formatMapName,
    leaderById,
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

function renderCampaignEpisode(episode: MenuCampaignEpisode, continueId: string) {
    const completed = !!episode.completed;
    const unlocked = !!episode.unlocked;
    const isNext = unlocked && !completed && episode.id === continueId;
    const title = localizedText(episode.title);
    const subtitle = localizedText(episode.subtitle);
    const mapName = episode.id === "boudica" ? "eastanglia" : "northamerica";
    const mapArt = lobbyThumb({ map_name: mapName });
    const avatar = asset(
        "gameplay/avatars/" + (episode.id === "boudica" ? "boudica" : "lady_six_sky") + ".webp",
    );
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
          " <span>↗</span></button>"
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
        "'>" +
        "<div class='sow-campaign__episode-art' aria-hidden='true'><img class='sow-campaign__map-art' src='" +
        esc(mapArt) +
        "' alt='' loading='lazy' decoding='async'><img class='sow-campaign__leader-art' src='" +
        esc(avatar) +
        "' alt='' loading='lazy' decoding='async'></div>" +
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
    const continueButton = continueId
        ? "<button class='sow-menu__primary' type='button' data-command='start_campaign_episode' data-episode-id='" +
          esc(continueId) +
          "'>" +
          esc(SOW_t("lobbies.continue_campaign")) +
          " <span>↗</span></button>"
        : episodes.length &&
            episodes.every(function (episode) {
                return episode && episode.completed;
            })
          ? "<div class='sow-campaign__complete'>" + esc(SOW_t("lobbies.saga_complete")) + "</div>"
          : "";
    return (
        "<main class='sow-menu__main sow-campaign' data-screen-panel='campaign'>" +
        "<section class='sow-menu__command sow-campaign__intro'>" +
        "<p class='sow-menu__eyebrow'>" +
        esc(SOW_t("lobbies.single_player")) +
        "</p>" +
        "<h1>" +
        esc(SOW_t("lobbies.campaign_chronicles")).replace("\n", "<br>") +
        "</h1>" +
        "<button class='sow-menu__secondary' type='button' data-command='close_campaign'>← " +
        esc(SOW_t("lobbies.back")) +
        "</button>" +
        "</section>" +
        "<section class='sow-menu__battlefield sow-campaign__battlefield'>" +
        "<div class='sow-campaign__heading'><div><p class='sow-menu__eyebrow'>" +
        esc(SOW_t("lobbies.the_saga")) +
        "</p><h2>" +
        esc(SOW_t("lobbies.episodes")) +
        "</h2></div>" +
        continueButton +
        "</div>" +
        "<div class='sow-campaign__episodes'>" +
        (episodes.length
            ? episodes
                  .map(function (episode) {
                      return renderCampaignEpisode(episode, continueId);
                  })
                  .join("")
            : "<p class='sow-menu__empty'>" + esc(SOW_t("lobbies.campaign_unavailable")) + "</p>") +
        "</div>" +
        "</section>" +
        "</main>"
    );
}

export function renderBrowser() {
    return (
        "<main class='sow-menu__main' data-screen-panel='browser'>" +
        "<section class='sow-menu__command'>" +
        "<p class='sow-menu__eyebrow'>" +
        esc(SOW_t("menu.lobby_browser")) +
        "</p>" +
        "<h1>" +
        esc(SOW_t("lobbies.active_matches")).replace("\n", "<br>") +
        "</h1>" +
        "<p class='sow-menu__tagline'>" +
        esc(SOW_t("lobbies.browser_tagline")) +
        "</p>" +
        "<button class='sow-menu__secondary' type='button' data-command='close_overlay'>← " +
        esc(SOW_t("lobbies.back")) +
        "</button>" +
        "<form class='sow-menu__join' data-form='join'>" +
        "<input name='code' inputmode='numeric' autocomplete='off' placeholder='" +
        esc(SOW_t("menu.lobby_code")) +
        "' aria-label='" +
        esc(SOW_t("menu.lobby_code")) +
        "'>" +
        "<button type='submit'>" +
        esc(SOW_t("menu.join")) +
        "</button>" +
        "</form>" +
        "<button class='sow-menu__secondary' type='button' data-command='open_create'>" +
        esc(SOW_t("menu.create_custom_game")) +
        " <span>+</span></button>" +
        renderFeedback() +
        "</section>" +
        "<section class='sow-menu__battlefield'>" +
        "<section class='sow-menu__public'>" +
        "<div class='sow-menu__browser-search'>" +
        "<input data-role='browser-search' type='search' placeholder='" +
        esc(SOW_t("lobbies.search_map_host")) +
        "' value=\"" +
        esc(menuUi.browserSearchQuery) +
        "\" aria-label='" +
        esc(SOW_t("lobbies.search_map_host")) +
        "'>" +
        "</div>" +
        "<div class='sow-menu__lobbies' data-lobby-list='browser'></div>" +
        "</section>" +
        "</section>" +
        "</main>"
    );
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

    const modeOptionsHtml = ["FFA", "Teams", "HumansVsNations"]
        .map(function (mode) {
            const labels: Record<string, string> = {
                FFA: SOW_t("lobbies.free_for_all"),
                Teams: SOW_t("lobbies.teams_two"),
                HumansVsNations: SOW_t("lobbies.hvn"),
            };
            const isSelected = (config.game_mode || "FFA") === mode;
            return (
                "<button type='button' class='sow-menu__pill" +
                (isSelected ? " active" : "") +
                "' data-command='set_create_mode' data-mode='" +
                mode +
                "'>" +
                esc(labels[mode] || mode) +
                "</button>"
            );
        })
        .join("");

    const diffOptionsHtml = ["Vanilla", "Terminator"]
        .map(function (diff) {
            const isSelected = (config.bot_difficulty || "Vanilla") === diff;
            return (
                "<button type='button' class='sow-menu__pill" +
                (isSelected ? " active" : "") +
                "' data-command='set_create_diff' data-diff='" +
                diff +
                "'>" +
                esc(diff === "Vanilla" ? SOW_t("lobbies.vanilla") : SOW_t("lobbies.terminator")) +
                "</button>"
            );
        })
        .join("");

    const spawnOptionsHtml = [true, false]
        .map(function (val) {
            const isSelected = (config.random_spawn !== false) === val;
            return (
                "<button type='button' class='sow-menu__pill" +
                (isSelected ? " active" : "") +
                "' data-command='set_create_spawn' data-spawn='" +
                String(val) +
                "'>" +
                esc(val ? SOW_t("lobbies.random_on") : SOW_t("lobbies.preset_off")) +
                "</button>"
            );
        })
        .join("");

    const visOptionsHtml = [false, true]
        .map(function (val) {
            const isSelected = menuUi.createPrivate === val;
            return (
                "<button type='button' class='sow-menu__pill" +
                (isSelected ? " active" : "") +
                "' data-command='set_create_private' data-private='" +
                String(val) +
                "'>" +
                esc(val ? SOW_t("lobbies.private_code") : SOW_t("lobbies.public")) +
                "</button>"
            );
        })
        .join("");

    const mapCatalogOptions = ((menuUi.state && menuUi.state.map_catalog) || []).map(function (m) {
        return { value: m.key, label: formatMapName(m) + " (" + m.width + "×" + m.height + ")" };
    });
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
        ? "<label class='sow-menu__form-field sow-create__map-select'>Roster" +
          SOW_renderDropdown({
              key: "create-map-roster",
              name: "map_roster_preset",
              value: config.map_roster_preset || selectedMap.default_roster || "",
              options: rosterPresets.map(function (preset) {
                  return {
                      value: preset.id,
                      label: preset.name + (preset.id === selectedMap.default_roster ? " (map default)" : ""),
                  };
              }),
          }) +
          "</label>"
        : "";

    const spControls = isSp
        ? "<div class='sow-menu__slider-field'>" +
          "<div class='sow-menu__slider-label'><span>" +
          esc(SOW_t("lobbies.procedural_seed")) +
          "</span><b data-val-for='seed'>" +
          esc(config.seed || 42) +
          "</b></div>" +
          "<div class='sow-menu__slider-row'>" +
          "<input class='sow-menu__range' name='seed' type='range' min='1' max='9999' step='1' value='" +
          esc(config.seed || 42) +
          "'>" +
          "<button class='sow-menu__ghost-mini' type='button' data-command='randomize_seed' aria-label='" +
          esc(SOW_t("lobbies.randomize_seed")) +
          "'>" +
          menuIcon("dice") +
          "</button>" +
          "</div>" +
          "</div>"
        : "<div>" +
          "<label class='sow-menu__form-field'>" +
          esc(SOW_t("lobbies.visibility")) +
          "<div class='sow-menu__pill-group'>" +
          visOptionsHtml +
          "</div>" +
          "</label>" +
          (menuUi.createPrivate
              ? "<label class='sow-menu__form-field' style='margin-top:10px;'>" +
                esc(SOW_t("lobbies.password_optional")) +
                "<input class='sow-menu__field' name='password' type='password' autocomplete='new-password' value='" +
                esc(menuUi.createPassword) +
                "' placeholder='" +
                esc(SOW_t("lobbies.password_empty_hint")) +
                "'>" +
                "</label>"
              : "") +
          "</div>";

    return (
        "<main class='sow-menu__main sow-menu__main--custom sow-create' data-screen-panel='create'>" +
        "<section class='sow-menu__command sow-create__rail'>" +
        "<p class='sow-menu__eyebrow'>" +
        esc(SOW_t("lobbies.custom_game")) +
        "</p>" +
        "<h1>" +
        esc(SOW_t("lobbies.match_settings")).replace("\n", "<br>") +
        "</h1>" +
        "<p class='sow-menu__tagline'>" +
        esc(SOW_t(isSp ? "lobbies.offline_hint" : "lobbies.online_hint")) +
        "</p>" +
        "<div class='sow-menu__mode-switcher'>" +
        "<button type='button' class='sow-menu__mode-tab" +
        (isSp ? " active" : "") +
        "' data-command='set_session_mode' data-mode='offline'>" +
        menuIcon("solo") +
        " " +
        esc(SOW_t("lobbies.solo_practice")) +
        "</button>" +
        "<button type='button' class='sow-menu__mode-tab" +
        (!isSp ? " active" : "") +
        "' data-command='set_session_mode' data-mode='online'>" +
        menuIcon("globe") +
        " " +
        esc(SOW_t("lobbies.online_lobby")) +
        "</button>" +
        "</div>" +
        "</section>" +
        "<section class='sow-menu__battlefield sow-create__workspace'>" +
        "<form class='sow-create__form' data-form='create'>" +
        "<div class='sow-create__scroll'>" +
        "<div class='sow-create__columns'>" +
        "<div class='sow-create__column'>" +
        "<section class='sow-menu__custom-card sow-create__map-panel'>" +
        "<div class='sow-menu__map-preview-wrap' style=\"background-image:url('" +
        esc(mapThumbUrl) +
        "')\">" +
        "<div class='sow-menu__map-preview-meta'>" +
        "<strong>" +
        esc(formatMapName(selectedMap)) +
        "</strong>" +
        "<small>" +
        esc(SOW_t("lobbies.tiles", { width: selectedMap.width, height: selectedMap.height })) +
        "</small>" +
        "</div>" +
        "</div>" +
        "<label class='sow-menu__form-field sow-create__map-select'>" +
        esc(SOW_t("lobbies.select_map")) +
        SOW_renderDropdown({
            key: "create-map",
            name: "map_name",
            value: selectedMap.key,
            options: mapCatalogOptions,
        }) +
        "</label>" +
        rosterControl +
        "</section>" +
        "<section class='sow-menu__custom-card'>" +
        "<label class='sow-menu__form-field'>" +
        esc(SOW_t("lobbies.game_type")) +
        "<div class='sow-menu__pill-group'>" +
        modeOptionsHtml +
        "</div>" +
        "</label>" +
        "<div class='sow-menu__form-row sow-create__rules-row'>" +
        "<label class='sow-menu__form-field'>" +
        esc(SOW_t("lobbies.bot_difficulty")) +
        "<div class='sow-menu__pill-group'>" +
        diffOptionsHtml +
        "</div>" +
        "</label>" +
        "<label class='sow-menu__form-field'>" +
        esc(SOW_t("lobbies.spawn_rules")) +
        "<div class='sow-menu__pill-group'>" +
        spawnOptionsHtml +
        "</div>" +
        "</label>" +
        "</div>" +
        "</section>" +
        "<section class='sow-menu__custom-card'>" +
        spControls +
        "</section>" +
        "</div>" +
        "<div class='sow-create__column'>" +
        "<section class='sow-menu__custom-card'>" +
        "<p class='sow-menu__panel-sublabel'>" +
        esc(SOW_t("lobbies.population_scale")) +
        "</p>" +
        "<div class='sow-menu__slider-field'>" +
        "<div class='sow-menu__slider-label'><span>" +
        esc(SOW_t("lobbies.max_human_players")) +
        "</span><b data-val-for='max_players'>" +
        (config.max_players || 8) +
        "</b></div>" +
        "<input class='sow-menu__range' name='max_players' type='range' min='2' max='16' step='1' value='" +
        (config.max_players || 8) +
        "'>" +
        "</div>" +
        "<div class='sow-menu__slider-field'>" +
        "<div class='sow-menu__slider-label'><span>" +
        esc(SOW_t("lobbies.neutral_bots")) +
        "</span><b data-val-for='bot_count'>" +
        (config.bot_count != null ? config.bot_count : 128) +
        "</b></div>" +
        "<input class='sow-menu__range' name='bot_count' type='range' min='0' max='" +
        botLimit("tribes", 400) +
        "' step='8' value='" +
        (config.bot_count != null ? config.bot_count : 128) +
        "'>" +
        "</div>" +
        "<div class='sow-menu__slider-field'>" +
        "<div class='sow-menu__slider-label'><span>" +
        esc(SOW_t("lobbies.complex_ai")) +
        "</span><b data-val-for='nation_count'>" +
        (config.nation_count != null ? config.nation_count : 32) +
        "</b></div>" +
        "<input class='sow-menu__range' name='nation_count' type='range' min='0' max='" +
        botLimit("nations", 200) +
        "' step='4' value='" +
        (config.nation_count != null ? config.nation_count : 32) +
        "'>" +
        "</div>" +
        "</section>" +
        renderFeedback() +
        "</div>" +
        "</div>" +
        "</div>" +
        "<div class='sow-create__actions'>" +
        "<button class='sow-menu__primary sow-menu__custom-launch-btn' type='submit'>" +
        esc(SOW_t(isSp ? "lobbies.start_simulation" : "lobbies.create_lobby")) +
        " <span>↗</span>" +
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

    const statusBadge =
        player.download_progress === 100 || player.is_ready
            ? "<span class='sow-menu__sync-badge ready'>" + esc(SOW_t("lobbies.ready")) + "</span>"
            : "<span class='sow-menu__sync-badge syncing'>" +
              esc(SOW_t("lobbies.sync", { progress: player.download_progress || 0 })) +
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
        "'>" +
        "<div class='sow-menu__roster-left'>" +
        "<div class='sow-menu__roster-avatar' style=\"background-image:url('" +
        esc(avatarUrl) +
        "')\"></div>" +
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
            return [
                player.player_id,
                player.name,
                player.leader,
                player.team,
                player.is_ready,
                player.download_progress,
            ];
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

    return (
        "<div class='sow-menu__ffa-roster'>" +
        players
            .map(function (player) {
                return renderQueuePlayerRow(player, lobby);
            })
            .join("") +
        "</div>"
    );
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

export function renderQueue() {
    const lobby = joinedLobby();
    const rankedSearch = !!menuUi.state!.ranked_search_active && !lobby;
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
