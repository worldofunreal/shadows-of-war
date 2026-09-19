// Lobby-facing screens: Home, Browser, Create, and Queue.

    function publicLobbies(includeMatchmaking) {
        return (state.lobbies || []).filter(function (lobby) {
            if (lobby.kind === "Matchmaking") {
                if (!includeMatchmaking) return false;
            } else if (lobby.kind !== "Custom" || lobby.is_private) {
                return false;
            }
            return true;
        });
    }

    function lobbyTimerText(lobby) {
        return lobby.is_counting_down ? SOW_t("lobbies.starting_in", { seconds: Math.ceil(lobby.timer_secs || 0) }) :
            (lobby.max_players ? SOW_t("lobbies.players_capacity", { players: lobby.num_players, max: lobby.max_players }) : SOW_t("lobbies.players_count", { players: lobby.num_players }));
    }

    function gameModeLabel(mode) {
        var key = { FFA: "lobbies.free_for_all", Teams: "lobbies.teams_two", HumansVsNations: "lobbies.humans_vs_nations", Matchmaking: "lobbies.matchmaking" }[mode];
        return key ? SOW_t(key) : String(mode || "");
    }

    var lobbyThumbnailCache = Object.create(null);

    function preloadLobbyThumbnail(lobby) {
        var url = lobbyThumb(lobby);
        if (lobbyThumbnailCache[url]) return lobbyThumbnailCache[url];
        lobbyThumbnailCache[url] = new Promise(function (resolve, reject) {
            var image = new Image();
            image.onload = function () {
                if (typeof image.decode !== "function") return resolve(url);
                try {
                    image.decode().then(function () { resolve(url); }, function () { resolve(url); });
                } catch (error) {
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
        return lobbyThumbnailCache[url];
    }

    function lobbyLabel(lobby) {
        return gameModeLabel(lobby.game_mode || "FFA") + " " + formatMapName(lobby);
    }

    function renderLobbyCard(lobby) {
        var lock = lobby.has_password ? "<span class='sow-menu__lobby-lock' aria-label='" + esc(SOW_t("lobbies.password_protected")) + "'>🔒</span>" : "";
        var label = gameModeLabel(lobby.game_mode) + " " + formatMapName(lobby);
        preloadLobbyThumbnail(lobby).catch(function () {});
        return "" +
            "<article class='sow-menu__lobby' role='button' tabindex='0' aria-label='" + esc(label) + "' data-lobby-card data-command='join_lobby' data-lobby-id='" + lobby.id +
                "'>" +
                "<img class='sow-menu__lobby-art' src='" + esc(lobbyThumb(lobby)) + "' alt='' loading='eager' decoding='async'>" +
                "<div class='sow-menu__lobby-top'><span class='sow-menu__lobby-chip' data-lobby-mode>" + esc(gameModeLabel(lobby.game_mode)) + "</span><span class='sow-menu__lobby-chip sow-menu__lobby-chip--status' data-timer-for='" + lobby.id + "'>" + esc(lobbyTimerText(lobby)) + "</span>" + lock + "</div>" +
                "<div class='sow-menu__lobby-bottom'><h3 data-lobby-map>" + esc(formatMapName(lobby)) + "</h3><span class='sow-menu__lobby-join'>" + esc(SOW_t("menu.join")) + " ↗</span></div>" +
            "</article>";
    }

    function renderPublicPanel(listName) {
        return "" +
            "<section class='sow-menu__public'>" +
                "<div class='sow-menu__lobbies' data-lobby-list='" + esc(listName || "home") + "'></div>" +
            "</section>";
    }

    function filteredBrowserLobbies() {
        var lobbies = publicLobbies(false);
        if (!browserSearchQuery) return lobbies;
        var q = browserSearchQuery.toLowerCase().trim();
        return lobbies.filter(function (lobby) {
            return (lobby.map_name && lobby.map_name.toLowerCase().indexOf(q) !== -1) ||
                (lobby.host_name && lobby.host_name.toLowerCase().indexOf(q) !== -1) ||
                (lobby.game_mode && lobby.game_mode.toLowerCase().indexOf(q) !== -1);
        });
    }

    function updateLobbyCard(card, lobby) {
        var previousMap = card.dataset.mapName || "";
        var nextMap = String(lobby.map_name || "world");
        var art = card.querySelector(".sow-menu__lobby-art");
        var mode = card.querySelector("[data-lobby-mode]");
        var map = card.querySelector("[data-lobby-map]");
        var timer = card.querySelector("[data-timer-for]");
        var lock = card.querySelector(".sow-menu__lobby-lock");
        card.dataset.mapName = nextMap;
        card.setAttribute("aria-label", lobbyLabel(lobby));
        if (mode) mode.textContent = gameModeLabel(lobby.game_mode);
        if (map) map.textContent = formatMapName(lobby);
        if (timer) timer.textContent = lobbyTimerText(lobby);
        if (lobby.has_password && !lock) {
            card.querySelector(".sow-menu__lobby-top").insertAdjacentHTML("beforeend", "<span class='sow-menu__lobby-lock' aria-label='" + esc(SOW_t("lobbies.password_protected")) + "'>🔒</span>");
        } else if (!lobby.has_password && lock) {
            lock.remove();
        }
        if (!art || previousMap === nextMap) return;
        var nextUrl = lobbyThumb(lobby);
        preloadLobbyThumbnail(lobby).then(function () {
            if (card.dataset.mapName !== nextMap || !card.isConnected) return;
            card.classList.add("is-map-changing");
            art.src = nextUrl;
            requestAnimationFrame(function () { card.classList.remove("is-map-changing"); });
        }).catch(function () {});
    }

    function makeLobbyCard(lobby) {
        var template = document.createElement("template");
        template.innerHTML = renderLobbyCard(lobby).trim();
        var card = template.content.firstElementChild;
        card.dataset.mapName = String(lobby.map_name || "world");
        var art = card.querySelector(".sow-menu__lobby-art");
        if (art) art.addEventListener("error", function () { art.remove(); }, { once: true });
        return card;
    }

    function syncLobbyList(container, lobbies, emptyMessage) {
        var cards = Object.create(null);
        container.querySelectorAll("[data-lobby-card]").forEach(function (card) {
            cards[String(card.dataset.lobbyId)] = card;
        });
        if (!lobbies.length) {
            Object.keys(cards).forEach(function (id) { cards[id].remove(); });
            var empty = container.querySelector(".sow-menu__empty");
            if (!empty) {
                empty = document.createElement("div");
                empty.className = "sow-menu__empty";
                container.appendChild(empty);
            }
            empty.textContent = emptyMessage;
            return;
        }
        var empty = container.querySelector(".sow-menu__empty");
        if (empty) empty.remove();
        lobbies.forEach(function (lobby) {
            var card = cards[String(lobby.id)];
            if (!card) card = makeLobbyCard(lobby);
            updateLobbyCard(card, lobby);
            container.appendChild(card);
            delete cards[String(lobby.id)];
        });
        Object.keys(cards).forEach(function (id) { cards[id].remove(); });
    }

    function updateLobbyViews() {
        var panel = activeScreenPanel() || root;
        panel.querySelectorAll("[data-lobby-list]").forEach(function (container) {
            var browser = container.dataset.lobbyList === "browser";
            syncLobbyList(container, browser ? filteredBrowserLobbies() : publicLobbies(true), browser ?
                SOW_t("lobbies.no_search_matches") : SOW_t("lobbies.no_active_lobbies"));
        });
    }

    function renderHome() {
        var leader = leaderById(state.selected_leader);
        return "<main class='sow-menu__main' data-screen-panel='home'>" +
            renderCommandPanel() +
            "<section class='sow-menu__battlefield'>" +
                "<div class='sow-menu__leader-copy'><small>" + esc(leaderCivilization(leader)) + "</small><h2>" + esc(leader.name) +
                    "</h2><p>" + esc(leaderPerk(leader)) + "</p></div>" +
            "</section>" +
        "</main>";
    }

    function renderCampaignEpisode(episode, continueId) {
        var completed = !!episode.completed;
        var unlocked = !!episode.unlocked;
        var isNext = unlocked && !completed && episode.id === continueId;
        var title = localizedText(episode.title);
        var subtitle = localizedText(episode.subtitle);
        var status = completed ? SOW_t("lobbies.completed") : unlocked ? (isNext ? SOW_t("lobbies.next") : SOW_t("lobbies.available")) : SOW_t("lobbies.locked");
        var label = completed ? SOW_t("lobbies.replay") : isNext ? SOW_t("lobbies.continue") : SOW_t("lobbies.play");
        var action = unlocked
            ? "<button class='sow-menu__secondary sow-campaign__play' type='button' data-command='start_campaign_episode' data-episode-id='" + esc(episode.id) + "'>" + esc(label) + " <span>↗</span></button>"
            : "<button class='sow-menu__secondary sow-campaign__play' type='button' disabled>" + esc(SOW_t("lobbies.locked")) + "</button>";
        return "<article class='sow-campaign__episode" + (completed ? " is-complete" : unlocked ? " is-unlocked" : " is-locked") + "'>" +
            "<div class='sow-campaign__episode-status'>" + esc(status) + "</div>" +
            "<h2>" + esc(title) + "</h2>" +
            "<p>" + esc(subtitle) + "</p>" +
            action +
        "</article>";
    }

    function renderCampaign() {
        var campaign = state && state.campaign || {};
        var episodes = Array.isArray(campaign.episodes) ? campaign.episodes : [];
        var continueId = campaign.continue_episode || "";
        var continueButton = continueId
            ? "<button class='sow-menu__primary' type='button' data-command='start_campaign_episode' data-episode-id='" + esc(continueId) + "'>" + esc(SOW_t("lobbies.continue_campaign")) + " <span>↗</span></button>"
            : "<div class='sow-campaign__complete'>" + esc(SOW_t("lobbies.saga_complete")) + "</div>";
        return "<main class='sow-menu__main sow-campaign' data-screen-panel='campaign'>" +
            "<section class='sow-menu__command sow-campaign__intro'>" +
                "<p class='sow-menu__eyebrow'>" + esc(SOW_t("lobbies.single_player")) + "</p>" +
                "<h1>" + esc(SOW_t("lobbies.campaign_chronicles")).replace("\n", "<br>") + "</h1>" +
                "<p class='sow-menu__tagline'>" + esc(SOW_t("lobbies.campaign_tagline")) + "</p>" +
                "<button class='sow-menu__secondary' type='button' data-command='close_campaign'>← " + esc(SOW_t("lobbies.back")) + "</button>" +
            "</section>" +
            "<section class='sow-menu__battlefield sow-campaign__battlefield'>" +
                "<div class='sow-campaign__heading'><div><p class='sow-menu__eyebrow'>" + esc(SOW_t("lobbies.the_saga")) + "</p><h2>" + esc(SOW_t("lobbies.episodes")) + "</h2></div>" + continueButton + "</div>" +
                "<div class='sow-campaign__episodes'>" +
                    (episodes.length ? episodes.map(function (episode) { return renderCampaignEpisode(episode, continueId); }).join("") : "<p class='sow-menu__empty'>" + esc(SOW_t("lobbies.campaign_unavailable")) + "</p>") +
                "</div>" +
            "</section>" +
        "</main>";
    }

    function renderBrowser() {
        return "<main class='sow-menu__main' data-screen-panel='browser'>" +
            "<section class='sow-menu__command'>" +
                "<p class='sow-menu__eyebrow'>" + esc(SOW_t("menu.lobby_browser")) + "</p>" +
                "<h1>" + esc(SOW_t("lobbies.active_matches")).replace("\n", "<br>") + "</h1>" +
                "<p class='sow-menu__tagline'>" + esc(SOW_t("lobbies.browser_tagline")) + "</p>" +
                "<button class='sow-menu__secondary' type='button' data-command='close_overlay'>← " + esc(SOW_t("lobbies.back")) + "</button>" +
                "<form class='sow-menu__join' data-form='join'>" +
                    "<input name='code' inputmode='numeric' autocomplete='off' placeholder='" + esc(SOW_t("menu.lobby_code")) + "' aria-label='" + esc(SOW_t("menu.lobby_code")) + "'>" +
                    "<button type='submit'>" + esc(SOW_t("menu.join")) + "</button>" +
                "</form>" +
                "<button class='sow-menu__secondary' type='button' data-command='open_create'>" + esc(SOW_t("menu.create_custom_game")) + " <span>+</span></button>" +
                renderFeedback() +
            "</section>" +
            "<section class='sow-menu__battlefield'>" +
                "<section class='sow-menu__public'>" +
                    "<div class='sow-menu__browser-search'>" +
                        "<input data-role='browser-search' type='search' placeholder='" + esc(SOW_t("lobbies.search_map_host")) + "' value=\"" + esc(browserSearchQuery) + "\" aria-label='" + esc(SOW_t("lobbies.search_map_host")) + "'>" +
                    "</div>" +
                    "<div class='sow-menu__lobbies' data-lobby-list='browser'></div>" +
                "</section>" +
            "</section>" +
        "</main>";
    }

    function cloneConfig() {
        return JSON.parse(JSON.stringify(state.custom_game_config || {}));
    }

    function syncCreateDraft(form) {
        if (!createDraft) createDraft = cloneConfig();
        Array.prototype.forEach.call(form.elements, function (field) {
            if (!field.name) return;
            if (field.name === "session_mode") {
                createOffline = field.value === "offline";
            } else if (field.name === "visibility") {
                createPrivate = field.value === "private";
            } else if (field.name === "password") {
                createPassword = field.value;
            } else if (field.name === "bot_count" || field.name === "nation_count" || field.name === "max_players" || field.name === "seed") {
                createDraft[field.name] = Number(field.value);
            } else if (field.name === "map_name") {
                createDraft.map_name = field.value;
                var map = mapInfo(field.value);
                if (map) {
                    createDraft.map_width = map.width;
                    createDraft.map_height = map.height;
                }
            } else if (field.name !== "visibility") {
                createDraft[field.name] = field.value;
            }
        });
    }

    function renderPasswordModal() {
        if (passwordLobbyId == null) return "";
        var lobby = findLobby(passwordLobbyId);
        var title = lobby ? formatMapName(lobby) : SOW_t("lobbies.private_lobby");
        var error = state.error ? "<div class='sow-menu__status sow-menu__status--error'>" + esc(localizedText(state.error)) + "</div>" : "";
        return "<div class='sow-menu__overlay' data-menu-overlay='password'><form class='sow-menu__modal sow-menu__password-modal' data-form='password' novalidate>" +
            "<div class='sow-menu__modal-head'><div><p class='sow-menu__panel-label'>" + esc(SOW_t("lobbies.password_required")) + "</p><h2>" + esc(title) + "</h2></div>" +
            "<button class='sow-menu__icon-button' type='button' data-command='close_password' aria-label='" + esc(SOW_t("menu.close")) + "'>×</button></div>" +
            "<p class='sow-menu__tagline'>" + esc(SOW_t("lobbies.private_lobby_hint")) + "</p>" +
            "<label class='sow-menu__form-field'>" + esc(SOW_t("lobbies.password")) + "<input class='sow-menu__field' name='password' type='password' autocomplete='current-password' value='" + esc(passwordDraft) + "' autofocus></label>" +
            error +
            "<div class='sow-menu__modal-actions'><button class='sow-menu__ghost-button' type='button' data-command='close_password'>" + esc(SOW_t("lobbies.cancel")) + "</button><button class='sow-menu__primary' type='submit'>" + esc(SOW_t("lobbies.join_lobby")) + " <span>↗</span></button></div></form></div>";
    }

    function renderCreate() {
        var config = createDraft || cloneConfig();
        createDraft = config;
        var isSp = createOffline;
        var selectedMap = mapInfo(config.map_name || "world");
        var mapThumbUrl = lobbyThumb({ map_name: selectedMap.key });

        var modeOptionsHtml = ["FFA", "Teams", "HumansVsNations"].map(function (mode) {
            var labels = { FFA: SOW_t("lobbies.free_for_all"), Teams: SOW_t("lobbies.teams_two"), HumansVsNations: SOW_t("lobbies.hvn") };
            var isSelected = (config.game_mode || "FFA") === mode;
            return "<button type='button' class='sow-menu__pill" + (isSelected ? " active" : "") + "' data-command='set_create_mode' data-mode='" + mode + "'>" + esc(labels[mode] || mode) + "</button>";
        }).join("");

        var diffOptionsHtml = ["Vanilla", "Terminator"].map(function (diff) {
            var isSelected = (config.bot_difficulty || "Vanilla") === diff;
            return "<button type='button' class='sow-menu__pill" + (isSelected ? " active" : "") + "' data-command='set_create_diff' data-diff='" + diff + "'>" + esc(diff === "Vanilla" ? SOW_t("lobbies.vanilla") : SOW_t("lobbies.terminator")) + "</button>";
        }).join("");

        var spawnOptionsHtml = [true, false].map(function (val) {
            var isSelected = (config.random_spawn !== false) === val;
            return "<button type='button' class='sow-menu__pill" + (isSelected ? " active" : "") + "' data-command='set_create_spawn' data-spawn='" + String(val) + "'>" + esc(val ? SOW_t("lobbies.random_on") : SOW_t("lobbies.preset_off")) + "</button>";
        }).join("");

        var visOptionsHtml = [false, true].map(function (val) {
            var isSelected = createPrivate === val;
            return "<button type='button' class='sow-menu__pill" + (isSelected ? " active" : "") + "' data-command='set_create_private' data-private='" + String(val) + "'>" + esc(val ? SOW_t("lobbies.private_code") : SOW_t("lobbies.public")) + "</button>";
        }).join("");

        var mapCatalogOptions = (state && state.map_catalog || []).map(function (m) {
            return { value: m.key, label: formatMapName(m) + " (" + m.width + "×" + m.height + ")" };
        });

        var spControls = isSp ?
            "<div class='sow-menu__slider-field'>" +
                "<div class='sow-menu__slider-label'><span>" + esc(SOW_t("lobbies.procedural_seed")) + "</span><b data-val-for='seed'>" + esc(config.seed || 42) + "</b></div>" +
                "<div class='sow-menu__slider-row'>" +
                    "<input class='sow-menu__range' name='seed' type='range' min='1' max='9999' step='1' value='" + esc(config.seed || 42) + "'>" +
                    "<button class='sow-menu__ghost-mini' type='button' data-command='randomize_seed' aria-label='" + esc(SOW_t("lobbies.randomize_seed")) + "'>🎲</button>" +
                "</div>" +
            "</div>" :
            "<div>" +
                "<label class='sow-menu__form-field'>" + esc(SOW_t("lobbies.visibility")) +
                    "<div class='sow-menu__pill-group'>" + visOptionsHtml + "</div>" +
                "</label>" +
                (createPrivate ?
                    "<label class='sow-menu__form-field' style='margin-top:10px;'>" + esc(SOW_t("lobbies.password_optional")) +
                        "<input class='sow-menu__field' name='password' type='password' autocomplete='new-password' value='" + esc(createPassword) + "' placeholder='" + esc(SOW_t("lobbies.password_empty_hint")) + "'>" +
                    "</label>" : "") +
            "</div>";

        return "<main class='sow-menu__main sow-menu__main--custom sow-create' data-screen-panel='create'>" +
                    "<section class='sow-menu__command sow-create__rail'>" +
                        "<p class='sow-menu__eyebrow'>" + esc(SOW_t("lobbies.custom_game")) + "</p>" +
                        "<h1>" + esc(SOW_t("lobbies.match_settings")).replace("\n", "<br>") + "</h1>" +
                        "<p class='sow-menu__tagline'>" + esc(SOW_t(isSp ? "lobbies.offline_hint" : "lobbies.online_hint")) + "</p>" +
                        "<div class='sow-menu__mode-switcher'>" +
                            "<button type='button' class='sow-menu__mode-tab" + (isSp ? " active" : "") + "' data-command='set_session_mode' data-mode='offline'>🎮 " + esc(SOW_t("lobbies.solo_practice")) + "</button>" +
                            "<button type='button' class='sow-menu__mode-tab" + (!isSp ? " active" : "") + "' data-command='set_session_mode' data-mode='online'>🌐 " + esc(SOW_t("lobbies.online_lobby")) + "</button>" +
                        "</div>" +
                    "</section>" +
                    "<section class='sow-menu__battlefield sow-create__workspace'>" +
                        "<form class='sow-create__form' data-form='create'>" +
                            "<div class='sow-create__scroll'>" +
                                "<div class='sow-create__columns'>" +
                                    "<div class='sow-create__column'>" +
                                        "<section class='sow-menu__custom-card sow-create__map-panel'>" +
                                            "<div class='sow-menu__map-preview-wrap' style=\"background-image:url('" + esc(mapThumbUrl) + "')\">" +
                                                "<div class='sow-menu__map-preview-meta'>" +
                                                    "<strong>" + esc(formatMapName(selectedMap)) + "</strong>" +
                                                    "<small>" + esc(SOW_t("lobbies.tiles", { width: selectedMap.width, height: selectedMap.height })) + "</small>" +
                                                "</div>" +
                                            "</div>" +
                                            "<label class='sow-menu__form-field sow-create__map-select'>" + esc(SOW_t("lobbies.select_map")) +
                                                SOW_renderDropdown({ key: "create-map", name: "map_name", value: selectedMap.key, options: mapCatalogOptions }) +
                                            "</label>" +
                                        "</section>" +
                                        "<section class='sow-menu__custom-card'>" +
                                            "<label class='sow-menu__form-field'>" + esc(SOW_t("lobbies.game_type")) +
                                                "<div class='sow-menu__pill-group'>" + modeOptionsHtml + "</div>" +
                                            "</label>" +
                                            "<div class='sow-menu__form-row sow-create__rules-row'>" +
                                                "<label class='sow-menu__form-field'>" + esc(SOW_t("lobbies.bot_difficulty")) +
                                                    "<div class='sow-menu__pill-group'>" + diffOptionsHtml + "</div>" +
                                                "</label>" +
                                                "<label class='sow-menu__form-field'>" + esc(SOW_t("lobbies.spawn_rules")) +
                                                    "<div class='sow-menu__pill-group'>" + spawnOptionsHtml + "</div>" +
                                                "</label>" +
                                            "</div>" +
                                        "</section>" +
                                        "<section class='sow-menu__custom-card'>" + spControls + "</section>" +
                                    "</div>" +
                                    "<div class='sow-create__column'>" +
                                        "<section class='sow-menu__custom-card'>" +
                                            "<p class='sow-menu__panel-sublabel'>" + esc(SOW_t("lobbies.population_scale")) + "</p>" +
                                            "<div class='sow-menu__slider-field'>" +
                                                "<div class='sow-menu__slider-label'><span>" + esc(SOW_t("lobbies.max_human_players")) + "</span><b data-val-for='max_players'>" + (config.max_players || 8) + "</b></div>" +
                                                "<input class='sow-menu__range' name='max_players' type='range' min='2' max='16' step='1' value='" + (config.max_players || 8) + "'>" +
                                            "</div>" +
                                            "<div class='sow-menu__slider-field'>" +
                                                "<div class='sow-menu__slider-label'><span>" + esc(SOW_t("lobbies.neutral_bots")) + "</span><b data-val-for='bot_count'>" + (config.bot_count != null ? config.bot_count : 128) + "</b></div>" +
                                                "<input class='sow-menu__range' name='bot_count' type='range' min='0' max='1000' step='8' value='" + (config.bot_count != null ? config.bot_count : 128) + "'>" +
                                            "</div>" +
                                            "<div class='sow-menu__slider-field'>" +
                                                "<div class='sow-menu__slider-label'><span>" + esc(SOW_t("lobbies.complex_ai")) + "</span><b data-val-for='nation_count'>" + (config.nation_count != null ? config.nation_count : 32) + "</b></div>" +
                                                "<input class='sow-menu__range' name='nation_count' type='range' min='0' max='400' step='4' value='" + (config.nation_count != null ? config.nation_count : 32) + "'>" +
                                            "</div>" +
                                        "</section>" +
                                        renderFeedback() +
                                    "</div>" +
                                "</div>" +
                            "</div>" +
                            "<div class='sow-create__actions'>" +
                                "<button class='sow-menu__primary sow-menu__custom-launch-btn' type='submit'>" +
                                    esc(SOW_t(isSp ? "lobbies.start_simulation" : "lobbies.create_lobby")) + " <span>↗</span>" +
                                "</button>" +
                                "<button class='sow-menu__ghost-button sow-create__cancel-btn' type='button' data-command='close_overlay'>" + esc(SOW_t("lobbies.cancel")) + "</button>" +
                            "</div>" +
                        "</form>" +
                    "</section>" +
        "</main>";
    }

    function joinedLobby() {
        var id = state.joined_lobby_id || state.pending_lobby_id;
        return (state.lobbies || []).find(function (lobby) { return lobby.id === id; }) || null;
    }

    function renderQueuePlayerRow(player, lobby) {
        var leader = leaderById(player.leader);
        var avatarUrl = asset("gameplay/avatars/" + leader.slug + ".webp");
        var isMe = player.player_id === state.my_player_id;
        var isHost = lobby && lobby.host_name === player.name;
        var canModerate = lobby && lobby.kind === "Custom" && state.is_lobby_host && !isMe;

        var statusBadge = player.download_progress === 100 || player.is_ready ?
            "<span class='sow-menu__sync-badge ready'>" + esc(SOW_t("lobbies.ready")) + "</span>" :
            "<span class='sow-menu__sync-badge syncing'>" + esc(SOW_t("lobbies.sync", { progress: player.download_progress || 0 })) + "</span>";

        var controls = canModerate ?
            "<div class='sow-menu__player-mod-actions'>" +
                (lobby.game_mode === "Teams" ? "<button type='button' class='sow-menu__mod-btn' data-command='move_player_team' data-lobby-id='" + lobby.id + "' data-player-id='" + player.player_id + "'>" + esc(SOW_t("lobbies.move_team")) + "</button>" : "") +
                "<button type='button' class='sow-menu__mod-btn' data-command='kick_player' data-lobby-id='" + lobby.id + "' data-player-id='" + player.player_id + "'>" + esc(SOW_t("lobbies.kick")) + "</button>" +
                "<button type='button' class='sow-menu__mod-btn danger' data-command='ban_player' data-lobby-id='" + lobby.id + "' data-player-id='" + player.player_id + "'>" + esc(SOW_t("lobbies.ban")) + "</button>" +
            "</div>" : "";

        return "" +
            "<div class='sow-menu__roster-row" + (isMe ? " is-me" : "") + "'>" +
                "<div class='sow-menu__roster-left'>" +
                    "<div class='sow-menu__roster-avatar' style=\"background-image:url('" + esc(avatarUrl) + "')\"></div>" +
                    "<div class='sow-menu__roster-name-group'>" +
                        "<strong>" + esc(player.name) + "</strong>" +
                        (isMe ? "<small class='sow-menu__me-tag'>" + esc(SOW_t("lobbies.you")) + "</small>" : "") +
                        (isHost ? "<small class='sow-menu__host-tag'>" + esc(SOW_t("lobbies.host")) + "</small>" : "") +
                    "</div>" +
                "</div>" +
                "<div class='sow-menu__roster-right'>" +
                    statusBadge +
                    controls +
                "</div>" +
                "</div>";
    }

    function queueRosterKey(lobby) {
        if (!lobby) return "";
        return JSON.stringify({
            id: lobby.id,
            mode: lobby.game_mode,
            kind: lobby.kind,
            host: lobby.host_name,
            owner: state.is_lobby_host,
            me: state.my_player_id,
            players: (lobby.players || []).map(function (player) {
                return [
                    player.player_id,
                    player.name,
                    player.leader,
                    player.team,
                    player.is_ready,
                    player.download_progress
                ];
            })
        });
    }

    function renderQueueRoster(lobby) {
        var players = (lobby && lobby.players) || [];
        if (!players.length) return "";

        if (lobby && lobby.game_mode === "Teams") {
            var redPlayers = players.filter(function (player) { return player.team === "Red"; });
            var bluePlayers = players.filter(function (player) { return player.team !== "Red"; });
            return "" +
                "<div class='sow-menu__teams-roster'>" +
                    "<div class='sow-menu__team-col sow-menu__team-col--red'>" +
                        "<div class='sow-menu__team-header'>🔴 " + esc(SOW_t("lobbies.red_team", { count: redPlayers.length })) + "</div>" +
                        "<div class='sow-menu__team-list'>" + redPlayers.map(function (player) { return renderQueuePlayerRow(player, lobby); }).join("") + "</div>" +
                    "</div>" +
                    "<div class='sow-menu__team-col sow-menu__team-col--blue'>" +
                        "<div class='sow-menu__team-header'>🔵 " + esc(SOW_t("lobbies.blue_team", { count: bluePlayers.length })) + "</div>" +
                        "<div class='sow-menu__team-list'>" + bluePlayers.map(function (player) { return renderQueuePlayerRow(player, lobby); }).join("") + "</div>" +
                    "</div>" +
                "</div>";
        }

        return "<div class='sow-menu__ffa-roster'>" + players.map(function (player) {
            return renderQueuePlayerRow(player, lobby);
        }).join("") + "</div>";
    }

    function updateQueueRoster(lobby) {
        var panel = activeScreenPanel();
        if (!panel || panel.dataset.screenPanel !== "queue") return;

        var roster = panel.querySelector("[data-queue-roster]");
        if (roster) {
            var key = queueRosterKey(lobby);
            if (roster.dataset.rosterKey !== key) {
                var scrollTop = roster.scrollTop;
                roster.innerHTML = renderQueueRoster(lobby);
                roster.dataset.rosterKey = key;
                roster.scrollTop = scrollTop;
            }
        }

        var count = panel.querySelector("[data-queue-player-count]");
        if (count) {
            count.textContent = lobby ? SOW_t("lobbies.players_capacity", { players: lobby.num_players || 0, max: lobby.max_players || 8 }) : "—";
        }
    }

    function renderQueue() {
        var lobby = joinedLobby();
        var title = lobby ? formatMapName(lobby).toUpperCase() : SOW_t("lobbies.matchmaking");
        var mapThumbUrl = lobbyThumb(lobby || { map_name: "world" });
        var isCustom = lobby && lobby.kind === "Custom";
        var isHost = state && state.is_lobby_host && isCustom;

        var modeClass = "sow-menu__mode-chip--" + (lobby ? lobby.game_mode.toLowerCase() : "ffa");
        var modeLabel = lobby ? (lobby.game_mode === "Teams" ? SOW_t("lobbies.teams_red_blue") : gameModeLabel(lobby.game_mode)) : SOW_t("lobbies.matchmaking");

        var feedback = "";
        if (state.is_downloading_map) {
            var pct = state.map_download_progress || 0;
            feedback = "" +
                "<div class='sow-menu__map-download-wrap'>" +
                    "<div class='sow-menu__map-download-text'>" + esc(SOW_t("lobbies.downloading_map", { map: formatMapName(state.downloading_map_name || title), progress: pct })) + "</div>" +
                    "<div class='sow-menu__map-download-bar'><div class='sow-menu__map-download-fill' style='width:" + pct + "%'></div></div>" +
                "</div>";
        } else if (state.error) {
            feedback = "<div class='sow-menu__queue-feedback sow-menu__queue-feedback--error'>" + esc(localizedText(state.error)) + "</div>";
        }

        var rosterKey = queueRosterKey(lobby);
        var rosterHtml = renderQueueRoster(lobby);

        return "<main class='sow-menu__main sow-menu__main--queue' data-screen-panel='queue'>" +
                    "<section class='sow-menu__queue-summary-card'>" +
                        "<div class='sow-menu__queue-map-hero' style=\"background-image:url('" + esc(mapThumbUrl) + "')\">" +
                            "<div class='sow-menu__queue-map-overlay'>" +
                                "<span class='sow-menu__mode-chip " + modeClass + "'>" + esc(modeLabel) + "</span>" +
                                "<h3>" + esc(title) + "</h3>" +
                            "</div>" +
                        "</div>" +
                        "<div class='sow-menu__queue-countdown' data-live-countdown></div>" +
                        feedback +
                        "<div class='sow-menu__queue-info-table'>" +
                            (lobby && lobby.host_name ? "<div class='sow-menu__info-row'><span>" + esc(SOW_t("lobbies.host")) + "</span><strong>" + esc(lobby.host_name) + "</strong></div>" : "") +
                            (isCustom && lobby ? "<div class='sow-menu__info-row'><span>" + esc(SOW_t("lobbies.room_code")) + "</span><div class='sow-menu__code-box'><strong>" + lobby.id + "</strong><button type='button' data-command='copy_lobby_code' data-lobby-id='" + lobby.id + "'>" + esc(SOW_t("lobbies.copy")) + "</button></div></div>" : "") +
                            (lobby && lobby.bot_count > 0 ? "<div class='sow-menu__info-row'><span>" + esc(SOW_t("lobbies.tribes")) + "</span><strong>" + lobby.bot_count + " (" + esc(lobby.bot_difficulty || SOW_t("lobbies.vanilla")) + ")</strong></div>" : "") +
                            (lobby && lobby.nation_count > 0 ? "<div class='sow-menu__info-row'><span>" + esc(SOW_t("lobbies.nations")) + "</span><strong>" + lobby.nation_count + "</strong></div>" : "") +
                            (lobby && lobby.has_password ? "<div class='sow-menu__info-row'><span>" + esc(SOW_t("lobbies.access")) + "</span><strong class='sow-menu__lock-tag'>🔒 " + esc(SOW_t("lobbies.password")) + "</strong></div>" : "") +
                        "</div>" +
                        "<div class='sow-menu__queue-action-bar'>" +
                            (isHost ? "<button class='sow-menu__primary sow-menu__queue-start-btn' type='button' data-command='start_private' data-lobby-id='" + lobby.id + "'>" + esc(SOW_t("lobbies.start_game")) + " <span>↗</span></button>" : "") +
                            "<button class='sow-menu__danger sow-menu__queue-leave-btn' type='button' data-command='leave_lobby'>" + esc(SOW_t("lobbies.leave_lobby")) + " <span>×</span></button>" +
                        "</div>" +
                    "</section>" +
                    "<section class='sow-menu__queue-players-card'>" +
                        "<div class='sow-menu__queue-players-head'>" +
                            "<p class='sow-menu__panel-label'>" + esc(SOW_t("lobbies.players")) + "</p>" +
                            "<span class='sow-menu__player-count-badge' data-queue-player-count>" + (lobby ? SOW_t("lobbies.players_capacity", { players: lobby.num_players || 0, max: lobby.max_players || 8 }) : "—") + "</span>" +
                        "</div>" +
                        "<div class='sow-menu__queue-roster-wrap' data-queue-roster data-roster-key='" + esc(rosterKey) + "'>" + rosterHtml + "</div>" +
                    "</section>" +
        "</main>";
    }
