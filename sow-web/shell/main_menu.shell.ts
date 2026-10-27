// Shared menu chrome, overlays, lifecycle, events, and state bridge.

    // ── Conduct & privacy (Terms/Privacy enforcement in-client) ──
    const REPORT_REASONS = [
        ["cheating", "profile.cheating"],
        ["harassment", "profile.harassment"],
        ["hate_speech", "profile.hate_speech"],
        ["threats", "profile.threats"],
        ["spam", "profile.spam"],
        ["inappropriate_name", "profile.inappropriate_name"],
        ["exploiting", "profile.exploiting"],
        ["other", "profile.other_explain"]
    ];
    let reportOpen = false;
    let reportTarget = null;
    let reportSent = false;
    let reportBusy = false;
    let latestLoaderCycleId = -1;
    let completedLoaderCycleId = -1;
    let deleteArmed = false;
    let deleteBusy = false;
    let deleteError = "";
    let activeMenuPointerId = null;
    let renderPending = false;
    let renderFlushTimer = null;
    let rewardAnimationTimer = null;
    let rewardAckRetryTimer = null;
    let rewardAnimationRunning = false;
    let rewardAnimationAccount = null;
    const rewardAnimationShown = new Set();
    let rewardAckTimes = Object.create(null);
    let rewardAnimationToken = 0;
    let activeRewardStage = null;
    let rewardPresentationReady = false;
    const menuFlow = window.SOWMenuFlow.create();
    window.SOW_menu_flow = menuFlow;
    let lastMenuFlowPhase = null;
    let displayedProgression = null;
    let progressionAnimationFrame = 0;
    let progressionAnimationTarget = null;

    const menuMusic = {
        decks: null,
        active: 0,
        state: null,
        activated: false,
        completedCycleId: -1,
        playBlocked: false,
        blockedDeck: null,
        pendingVolume: null,
        crossfade: null,
        cinematicActive: false,
        mediaMetadataSet: false,
        fetchRequested: false,
        frame: 0
    };

    function menuMusicVolume() {
        const settings = menuMusic.state && menuMusic.state.settings || {};
        const raw = menuMusic.pendingVolume == null
            ? (settings.music_volume == null ? 0.8 : settings.music_volume)
            : menuMusic.pendingVolume;
        const value = Number(raw);
        return Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0.8;
    }

    function menuMusicMuted() {
        return Boolean(menuMusic.state && menuMusic.state.settings && menuMusic.state.settings.mute_all);
    }

    function menuMusicAssetUrl() {
        const base = String(window.SOW_ASSETS_URL || "").replace(/\/$/, "");
        if (!base) {
            console.error("[SOW MUSIC] SOW_ASSETS_URL is missing; theme cannot load.");
            return null;
        }
        const url = base + "/gameplay/music/shadows-of-war.webm";
        const versions = window.SOW_ASSET_TS || {};
        let version = String(versions["/assets/gameplay/music/shadows-of-war.webm"] || window.SOW_BUILD_TS || "");
        if (version === "__BUILD_TS__") version = "";
        return version ? url + "?v=" + encodeURIComponent(version) : url;
    }

    function menuMusicSetVolumes(mixes, fade?) {
        if (!menuMusic.decks) return;
        const base = menuMusicMuted() ? 0 : menuMusicVolume();
        const factor = fade == null ? 1 : fade;
        menuMusic.decks.forEach(function (deck, index) {
            deck.volume = base * (mixes[index] || 0) * factor;
        });
    }

    function menuMusicCancelFrame() {
        if (menuMusic.frame) window.cancelAnimationFrame(menuMusic.frame);
        menuMusic.frame = 0;
    }

    function menuMusicLoaderActive(state) {
        return state && (state.loader_job === "Boot" || state.loader_job === "EnterGame" || state.loader_job === "ExitGame") &&
            (!Number.isSafeInteger(state.loader_cycle_id) || state.loader_cycle_id > menuMusic.completedCycleId);
    }

    function menuMusicPausePlayback() {
        menuMusicCancelFrame();
        menuMusic.crossfade = null;
        if (!menuMusic.decks) return;
        menuMusic.decks.forEach(function (deck) { deck.pause(); });
        menuMusicSetVolumes(menuMusicMixes());
    }

    function menuMusicReset() {
        menuMusicPausePlayback();
        menuMusic.active = 0;
        if (menuMusic.decks) {
            menuMusic.decks.forEach(function (deck) {
                try { deck.currentTime = 0; } catch {}
            });
            menuMusicSetVolumes(menuMusicMixes());
        }
    }

    function menuMusicFinishLoaderCycle(cycleId) {
        const currentState = menuMusic.state;
        if (!Number.isSafeInteger(cycleId) || cycleId <= menuMusic.completedCycleId || !currentState ||
            currentState.loader_cycle_id !== cycleId ||
            (currentState.loader_job !== "Boot" && currentState.loader_job !== "EnterGame" &&
                currentState.loader_job !== "ExitGame")) return false;
        menuMusic.completedCycleId = cycleId;
        menuMusicReset();
        syncMenuMusic(currentState, false);
        return true;
    }

    function menuMusicResetOnMenuReturn(previousState, nextState) {
        if (!previousState || nextState.phase !== "MainMenu") return false;
        const returnedFromGame = previousState.phase !== "MainMenu";
        const leftLobby = previousState.phase === "MainMenu" && previousState.waiting && !nextState.waiting;
        return (returnedFromGame || leftLobby) &&
            nextState.loader_job !== "EnterGame" && nextState.loader_job !== "ExitGame";
    }

    function menuMusicMixes() {
        if (!menuMusic.crossfade) return menuMusic.active === 0 ? [1, 0] : [0, 1];
        const progress = menuMusic.crossfade.progress;
        const mixes = [0, 0];
        mixes[menuMusic.crossfade.outgoing] = 1 - progress;
        mixes[menuMusic.crossfade.incoming] = progress;
        return mixes;
    }

    function menuMusicFinishCrossfade() {
        if (!menuMusic.crossfade) return;
        menuMusicCancelFrame();
        const outgoing = menuMusic.crossfade.outgoing;
        menuMusic.active = menuMusic.crossfade.incoming;
        menuMusic.crossfade = null;
        menuMusic.decks[outgoing].pause();
        try { menuMusic.decks[outgoing].currentTime = 0; } catch {}
        menuMusicSetVolumes(menuMusicMixes());
    }

    function menuMusicScheduleCrossfade() {
        if (!menuMusic.crossfade || menuMusic.frame) return;
        menuMusic.frame = window.requestAnimationFrame(function (timestamp) {
            menuMusic.frame = 0;
            if (!menuMusic.crossfade) return;
            if (menuMusic.crossfade.startedAt == null) menuMusic.crossfade.startedAt = timestamp;
            menuMusic.crossfade.progress = Math.min(1, Math.max(0,
                (timestamp - menuMusic.crossfade.startedAt) / 3000));
            menuMusicSetVolumes(menuMusicMixes());
            if (menuMusic.crossfade.progress >= 1) menuMusicFinishCrossfade();
            else menuMusicScheduleCrossfade();
        });
    }

    function menuMusicPlaybackFailed(error, index) {
        if (error && error.name === "NotAllowedError") {
            menuMusic.playBlocked = true;
            menuMusic.blockedDeck = index;
            return true;
        }
        console.error("[SOW MUSIC] Opus playback failed:", error);
        return false;
    }

    function menuMusicSetMediaMetadata() {
        const mediaSession = window.navigator && window.navigator.mediaSession;
        if (menuMusic.mediaMetadataSet || !mediaSession || typeof window.MediaMetadata !== "function") return;
        const artworkMeta = document.querySelector('meta[name="sow-media-artwork"]');
        if (!artworkMeta || !artworkMeta.content) return;
        try {
            mediaSession.metadata = new window.MediaMetadata({
                title: "Shadows of War",
                artwork: [{
                    src: new window.URL(artworkMeta.content, document.baseURI).href,
                    sizes: "192x192",
                    type: "image/png"
                }]
            });
            menuMusic.mediaMetadataSet = true;
        } catch (error) {
            console.warn("[SOW MUSIC] could not set player metadata:", error);
        }
    }

    function menuMusicPlayDeck(index, userGesture) {
        const deck = menuMusic.decks && menuMusic.decks[index];
        if (!deck || !menuMusic.activated || menuMusicLoaderActive(menuMusic.state) ||
            menuMusicMuted() || menuMusicVolume() <= 0) return false;
        if (!menuMusic.fetchRequested) menuMusicBeginLoading();
        if (userGesture) {
            menuMusic.playBlocked = false;
            menuMusic.blockedDeck = null;
        }
        if (menuMusic.playBlocked && !userGesture) return false;
        if (!deck.paused) return true;
        try {
            const result = deck.play();
            const started = function () {
                menuMusic.playBlocked = false;
                menuMusic.blockedDeck = null;
                menuMusicSetMediaMetadata();
                if (menuMusic.crossfade && menuMusic.crossfade.incoming === index) menuMusicScheduleCrossfade();
            };
            if (result && typeof result.then === "function") {
                result.then(started).catch(function (error) {
                    const blocked = menuMusicPlaybackFailed(error, index);
                    if (!blocked && menuMusic.crossfade && menuMusic.crossfade.incoming === index) {
                        menuMusic.crossfade = null;
                        menuMusicSetVolumes(menuMusicMixes());
                    }
                });
            } else {
                started();
            }
            return true;
        } catch (error) {
            const blocked = menuMusicPlaybackFailed(error, index);
            if (!blocked && menuMusic.crossfade && menuMusic.crossfade.incoming === index) {
                menuMusic.crossfade = null;
                menuMusicSetVolumes(menuMusicMixes());
            }
            return false;
        }
    }

    function menuMusicBeginCrossfade() {
        if (!menuMusic.decks || menuMusic.crossfade || menuMusicLoaderActive(menuMusic.state)) return;
        const incoming = 1 - menuMusic.active;
        try { menuMusic.decks[incoming].currentTime = 0; } catch {}
        menuMusic.crossfade = {
            outgoing: menuMusic.active,
            incoming: incoming,
            progress: 0,
            startedAt: null
        };
        menuMusicSetVolumes(menuMusicMixes());
        if (!menuMusicPlayDeck(incoming, false)) menuMusic.crossfade = null;
    }

    function menuMusicCreatePlayer() {
        try {
            const url = menuMusicAssetUrl();
            if (!url) return false;
            menuMusic.decks = [new window.Audio(), new window.Audio()];
            menuMusic.decks.forEach(function (deck, index) {
                deck.preload = "none";
                deck.loop = false;
                deck.volume = 0;
                deck.addEventListener("error", function () {
                    console.error("[SOW MUSIC] Opus theme failed to load:", deck.error || "media error");
                });
                deck.addEventListener("timeupdate", function () {
                    if (index !== menuMusic.active || menuMusic.crossfade || menuMusicLoaderActive(menuMusic.state)) return;
                    if (Number.isFinite(deck.duration) && deck.duration > 3.1 && deck.currentTime >= deck.duration - 3) {
                        menuMusicBeginCrossfade();
                    }
                });
                deck.addEventListener("ended", function () {
                    if (index !== menuMusic.active || menuMusicLoaderActive(menuMusic.state)) return;
                    if (menuMusic.crossfade && menuMusic.crossfade.outgoing === index) {
                        menuMusicFinishCrossfade();
                        return;
                    }
                    console.warn("[SOW MUSIC] loop crossfade missed the track end; restarting at the opening.");
                    menuMusic.active = 1 - index;
                    try { menuMusic.decks[menuMusic.active].currentTime = 0; } catch {}
                    menuMusicSetVolumes(menuMusicMixes());
                    menuMusicPlayDeck(menuMusic.active, false);
                });
                deck.src = url;
            });
            return true;
        } catch (error) {
            console.error("[SOW MUSIC] could not create the WebView audio player:", error);
            return false;
        }
    }

    function syncMenuMusic(nextState, userGesture, volumeOverride?) {
        if (!nextState) return;
        const previousState = menuMusic.state;
        menuMusic.state = nextState;
        const reportedVolume = nextState.settings && Number(nextState.settings.music_volume);
        if (volumeOverride != null && Number.isFinite(Number(volumeOverride))) {
            menuMusic.pendingVolume = Math.max(0, Math.min(1, Number(volumeOverride)));
        } else if (menuMusic.pendingVolume != null && Number.isFinite(reportedVolume) &&
            Math.abs(reportedVolume - menuMusic.pendingVolume) < 0.001) {
            menuMusic.pendingVolume = null;
        }
        if (userGesture) {
            menuMusic.activated = true;
            menuMusic.playBlocked = false;
        }
        if (menuMusicLoaderActive(nextState)) {
            menuMusicPausePlayback();
            return;
        }
        if (menuMusic.cinematicActive) {
            menuMusicPausePlayback();
            return;
        }
        if (menuMusicResetOnMenuReturn(previousState, nextState)) menuMusicReset();
        if (menuMusicMuted() || menuMusicVolume() <= 0) {
            menuMusic.playBlocked = false;
            menuMusic.blockedDeck = null;
            menuMusicSetVolumes(menuMusicMixes());
            return;
        }
        if (!menuMusic.activated) return;
        if (!menuMusic.decks && !menuMusicCreatePlayer()) return;
        menuMusicSetVolumes(menuMusicMixes());
        const nextDeck = menuMusic.crossfade ? menuMusic.crossfade.incoming
            : menuMusic.blockedDeck == null ? menuMusic.active : menuMusic.blockedDeck;
        menuMusicPlayDeck(nextDeck, Boolean(userGesture));
    }

    function menuMusicSetCinematicActive(active) {
        active = Boolean(active);
        if (menuMusic.cinematicActive === active) return;
        menuMusic.cinematicActive = active;
        if (active) menuMusicPausePlayback();
        else if (menuMusic.state) syncMenuMusic(menuMusic.state, false);
    }

    window.SOW_menu_music_cinematic = menuMusicSetCinematicActive;

    // Music bytes wait for their own window: the boot template fires
    // sow:wasm-bytes-ready once the engine download completes, so the theme
    // buffers against CPU/identity work instead of the WASM wire.
    function menuMusicBeginLoading() {
        if (menuMusic.fetchRequested || !menuMusic.decks) return;
        if (menuMusicMuted() || menuMusicVolume() <= 0) return;
        menuMusic.fetchRequested = true;
        try { menuMusic.decks[menuMusic.active].load(); } catch {}
    }

    function menuMusicStartAtLoaderEnd() {
        menuMusicBeginLoading();
        menuMusic.activated = true;
        if (menuMusic.state && Number.isSafeInteger(menuMusic.state.loader_cycle_id)) {
            menuMusic.completedCycleId = Math.max(menuMusic.completedCycleId, menuMusic.state.loader_cycle_id);
        }
        if (menuMusic.state) syncMenuMusic(menuMusic.state, false);
    }

    function menuMusicUnlockFromClick(event) {
        if (!event || !event.isTrusted || !event.target || typeof root.contains !== "function" ||
            root.contains(event.target) || !menuMusic.activated || !menuMusic.state ||
            menuMusicLoaderActive(menuMusic.state) || menuMusic.cinematicActive ||
            menuMusicMuted() || menuMusicVolume() <= 0) return;
        const deckIndex = menuMusic.crossfade ? menuMusic.crossfade.incoming
            : menuMusic.blockedDeck == null ? menuMusic.active : menuMusic.blockedDeck;
        const deck = menuMusic.decks && menuMusic.decks[deckIndex];
        if (!menuMusic.playBlocked && (!deck || !deck.paused)) return;
        syncMenuMusic(menuMusic.state, true);
    }

    menuMusicCreatePlayer();
    window.addEventListener("sow:loader-ready", menuMusicStartAtLoaderEnd, { once: true });
    window.addEventListener("sow:wasm-bytes-ready", menuMusicBeginLoading, { once: true });
    document.addEventListener("click", menuMusicUnlockFromClick, true);

    function renderTopbar() {
        const name = state.player_name || SOW_t("menu.anonymous");
        const auth: SowAuthState = { linked: false, pending: false };
        /* POKI_SHARED_AUTH_LOOKUP_BEGIN */
        if (typeof window.SOW_getAuthState === "function") Object.assign(auth, window.SOW_getAuthState());
        /* POKI_SHARED_AUTH_LOOKUP_END */
        const accountXp = Math.max(0, Number(state.xp) || 0);
        const crowns = state.crowns || 0;
        const laurels = state.laurels || 0;
        const gems = state.gems || 0;
        const showSignIn = portalOffersSignIn() && !(auth.linked || auth.pending);
        const signInMarkup = { value: "" };
        /* POKI_SHARED_SIGNIN_MARKUP_BEGIN */
        signInMarkup.value = "<button class='sow-menu__signin' type='button' data-command='sign_in'>" + esc(SOW_t("menu.sign_in")) + "</button>";
        /* POKI_SHARED_SIGNIN_MARKUP_END */
        return "" +
            "<header class='sow-menu__topbar'>" +
                "<div class='sow-menu__identity'>" +
                    "<button class='sow-menu__avatar' type='button' data-command='open_leader_picker' " +
                        "aria-label='" + esc(SOW_t("menu.select_leader")) + "' style=\"background-image:url('" + esc(avatarImage()) + "')\"></button>" +
                    "<div class='sow-menu__profile'>" +
                        "<input data-role='display-name' name='display_name' value=\"" + esc(name) + "\" maxlength='16' aria-label='" + esc(SOW_t("menu.display_name")) + "'>" +
                    "</div>" +
                "</div>" +
                "<div class='sow-menu__top-actions'>" +
                    "<div class='sow-menu__progress' data-progression data-command='open_profile' role='button' tabindex='0' title='" + esc(SOW_t("menu.open_profile")) + "' aria-label='" + esc(SOW_t("menu.open_profile")) + "'>" +
                        "<span class='sow-menu__progress-cell sow-menu__level'><small>" + esc(SOW_t("menu.level_short")) + "</small><strong data-progression-level-value>" + esc(state.level) + "</strong></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__xp'><span class='sow-menu__xp-value' data-progression-xp-value>" + esc(Math.floor(accountXp)) + " " + esc(SOW_t("menu.xp")) + "</span><span class='sow-menu__xp-track' aria-hidden='true'><i data-progression-xp-fill style='width:" + (accountXp % 100) + "%'></i></span></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__crowns'><img class='sow-menu__currency-icon' src='" + esc(currencyAsset("crown")) + "' alt='' aria-hidden='true'><strong data-progression-crowns-value>" + esc(crowns) + "</strong></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__laurels' aria-label='" + esc(SOW_t("profile.laurels")) + "'><img class='sow-menu__currency-icon' src='" + esc(currencyAsset("laurel")) + "' alt='' aria-hidden='true'><strong data-progression-laurels-value>" + esc(laurels) + "</strong></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__gems'><img class='sow-menu__currency-icon' src='" + esc(currencyAsset("gem")) + "' alt='' aria-hidden='true'><strong data-progression-gems-value>" + esc(gems) + "</strong></span>" +
                    "</div>" +
                    (showSignIn ? signInMarkup.value : (!portalOffersSignIn() ? "<span class='sow-menu__account-label'>" + esc(SOW_t("menu.anonymous")) + "</span>" : "")) +
                    "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='" + esc(SOW_t("menu.settings")) + "'>" + menuIcon("settings") + "</button>" +
                "</div>" +
            "</header>";
    }

    function localeOptions() {
        const codes = typeof window.SOW_getSupportedLocales === "function" ? window.SOW_getSupportedLocales() : [];
        return codes.map(function (code) {
            return { value: code, label: typeof window.SOW_getLocaleLabel === "function" ? window.SOW_getLocaleLabel(code) : SOW_t("menu.language_" + String(code).replace(/-/g, "_")) };
        });
    }

    /* POKI_RENDER_REPLACEMENT_BEGIN */
    function renderCommandPanel() {
        return "" +
            "<section class='sow-menu__command'>" +
                "<div class='sow-menu__home-public'>" + renderPublicPanel("home") + "</div>" +
                "<div class='sow-menu__home-actions'>" +
                    "<button class='sow-menu__primary' type='button' data-command='quick_match'>" + esc(SOW_t("menu.quick_match")) + " <span>↗</span></button>" +
                    "<button class='sow-menu__secondary' type='button' data-command='open_campaign'>" + esc(SOW_t("menu.campaign")) + " " + menuIcon("attack") + "</button>" +
                    "<button class='sow-menu__secondary' type='button' data-command='open_browser'>" + esc(SOW_t("menu.lobby_browser")) + " <span>→</span></button>" +
                    "<form class='sow-menu__join' data-form='join'>" +
                        "<input name='code' inputmode='numeric' autocomplete='off' placeholder='" + esc(SOW_t("menu.lobby_code")) + "' aria-label='" + esc(SOW_t("menu.lobby_code")) + "'>" +
                        "<button type='submit'>" + esc(SOW_t("menu.join")) + "</button>" +
                    "</form>" +
                    "<button class='sow-menu__secondary' type='button' data-command='open_create'>" + esc(SOW_t("menu.create_custom_game")) + " <span>+</span></button>" +
                    renderFeedback() +
                "</div>" +
            "</section>";
    }

    function renderFeedback() {
        const error = state.error ? "<div class='sow-menu__status sow-menu__status--error'>" + esc(localizedText(state.error)) + "</div>" : "";
        const notice = state.notice ? "<div class='sow-menu__status sow-menu__status--notice'>" +
            esc(SOW_t(({ host_left: "menu.host_left", kicked: "menu.removed_from_lobby", banned: "menu.banned_from_lobby", connection_lost: "menu.connection_lost" }[state.notice] || "menu.connection_lost"))) +
            "</div>" : "";
        return error + notice;
    }

    function renderSiteLink(path, key, className) {
        const crazyGames = window.SOW_PORTAL === "crazygames";
        const href = crazyGames ? "https://shadowsofwar.io" + path : path;
        const classAttr = className ? " class='" + esc(className) + "'" : "";
        const targetAttr = crazyGames ? " target='_blank' rel='noopener noreferrer'" : "";
        return "<a" + classAttr + " href='" + esc(href) + "'" + targetAttr + ">" + esc(SOW_t(key)) + "</a>";
    }

    function renderMenuAttribution() {
        return "<span class='sow-menu__attribution'>© OpenFront and Contributors</span>";
    }

    function renderSettingsLink(url, key, className?) {
        if (!url) return "";
        if (isAndroidTwa()) {
            return "<button class='sow-menu__settings-link" + (className ? " " + esc(className) : "") + "' type='button' data-command='open_android_external_link' data-url='" + esc(url) + "'>" + esc(SOW_t(key)) + "</button>";
        }
        return "<a class='sow-menu__settings-link" + (className ? " " + esc(className) : "") + "' href='" + esc(url) + "' target='_blank' rel='noopener noreferrer'>" + esc(SOW_t(key)) + "</a>";
    }

    function renderSettingsLinks() {
        const sourceUrl = typeof window.SOW_SOURCE_URL === "string" ? window.SOW_SOURCE_URL : "";
        const licenseUrl = sourceUrl ? sourceUrl.replace("/tree/", "/blob/") + "/LICENSE" : "";
        const androidOssLicenses = isAndroidTwa() &&
            typeof window.SOW_androidOssLicensesAvailable === "function" &&
            window.SOW_androidOssLicensesAvailable()
            ? "<button class='sow-menu__settings-link' type='button' data-command='open_android_oss_licenses'>" + esc(SOW_t("menu.third_party_licenses")) + "</button>"
            : "";
        const cookieSettings = typeof window.SOW_analyticsManage === "function"
            ? "<button class='sow-menu__settings-link' type='button' data-sow-cookie-settings>" + esc(SOW_t("menu.cookie_settings")) + "</button>"
            : "";
        return renderSettingsLink("https://shadowsofwar.io/#faq", "site.faq") +
            renderSettingsLink("https://shadowsofwar.io/support/", "menu.support") +
            renderSettingsLink("https://shadowsofwar.io/terms/", "menu.terms") +
            renderSettingsLink("https://shadowsofwar.io/cookies/", "menu.cookies") +
            cookieSettings +
            renderSettingsLink("https://discord.gg/d6ZDeChSE", "menu.discord") +
            renderSettingsLink("https://t.me/shadowsofwario", "menu.telegram") +
            renderSettingsLink(sourceUrl, "menu.source_code", "sow-menu__source-link") +
            renderSettingsLink(licenseUrl, "menu.license", "sow-menu__license-link") + androidOssLicenses;
    }

    function renderMainNav(active) {
        return renderMainNavMarkup(active, MAIN_NAV_ITEMS);
    }

    function renderSettings() {
        const settings = state.settings || {};
        const vol = settings.music_volume == null ? 0.8 : settings.music_volume;
        const volPct = Math.round(vol * 100);
        const auth = typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() : { platform: isAndroidTwa() ? "twa" : "web", linked: false, pending: false };
        const session = typeof window.SOW_getWouSession === "function" ? window.SOW_getWouSession() : authSession();
        const accountEmail = session && session.user && session.user.email ? String(session.user.email).trim() : "";
        const providers = (auth.linkedProviders || []).map(function (provider) {
            return String(provider).replace(/_/g, " ").toUpperCase();
        });
        const providerLabel = auth.platform === "twa" ? SOW_t("menu.google_play_games") :
            auth.provider === "crazygames" ? "CRAZYGAMES" :
            providers.join(" · ") || (auth.provider ? String(auth.provider).replace(/_/g, " ").toUpperCase() : "");
        const accountDetail = auth.linked ? (accountEmail || providerLabel || SOW_t("menu.anonymous")) : SOW_t("menu.anonymous");
        const accountControl = auth.pending
            ? "<div class='sow-menu__settings-account sow-menu__settings-account--pending'><strong class='sow-menu__account-pending'>" + esc(SOW_t("menu.connecting")) + "</strong></div>"
            : "<div class='sow-menu__settings-account'><strong class='sow-menu__account-value'>" + esc(accountDetail) + "</strong></div>";
        let signOutControl = "";
        if (auth.canSignOut) {
            signOutControl = signOutConfirmOpen
                ? "<section class='sow-menu__settings-confirm' role='alertdialog' aria-label='" + esc(SOW_t("menu.sign_out")) + "'>" +
                    "<strong>" + esc(SOW_t("menu.sign_out")) + "?</strong>" +
                    "<div class='sow-menu__settings-confirm-actions'>" +
                        "<button class='sow-menu__secondary' type='button' data-command='cancel_sign_out'>" + esc(SOW_t("lobbies.cancel")) + "</button>" +
                        "<button class='sow-menu__danger' type='button' data-command='confirm_sign_out'>" + esc(SOW_t("menu.sign_out")) + "</button>" +
                    "</div>" +
                "</section>"
                : "<button class='sow-menu__danger sow-menu__settings-signout' type='button' data-command='sign_out'>" + esc(SOW_t("menu.sign_out")) + "</button>";
        }
        return "" +
            "<div class='sow-menu__overlay' data-menu-overlay='settings'>" +
                "<section class='sow-menu__modal sow-menu__settings-modal'>" +
                    "<div class='sow-menu__modal-head'>" +
                        "<div>" +
                            "<h2>" + esc(SOW_t("menu.settings")) + "</h2>" +
                        "</div>" +
                        "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='" + esc(SOW_t("menu.close")) + "'>×</button>" +
                    "</div>" +
                    "<div class='sow-menu__settings-body'>" +
                        "<section class='sow-menu__settings-section'><h3>" + esc(SOW_t("menu.account")) + "</h3>" + accountControl +
                        (signOutControl ? "<div class='sow-menu__settings-actions'>" + signOutControl + "</div>" : "") + "</section>" +
                        "<section class='sow-menu__settings-section'><h3>" + esc(SOW_t("menu.settings_preferences")) + "</h3><div class='sow-menu__settings-controls'>" +
                            "<label class='sow-hud__setting-row sow-hud__setting-row--slider'>" +
                                "<div class='sow-menu__slider-label'><span>" + esc(SOW_t("menu.music_volume")) + "</span><b data-val-for='music_vol'>" + volPct + "%</b></div>" +
                                "<input type='range' name='music_volume' min='0' max='1' step='0.05' value='" + esc(vol) + "' data-setting='music_volume'>" +
                            "</label>" +
                            "<div class='sow-hud__setting-row sow-menu__settings-dropdown-row'>" +
                                "<span>" + esc(SOW_t("menu.motion_animation")) + "</span>" +
                                SOW_renderDropdown({ key: "settings-motion", className: "sow-menu__settings-dropdown", label: SOW_t("menu.motion_animation"), name: "reduced_motion", setting: "reduced_motion", value: settings.reduced_motion ? "reduced" : "full", options: [
                                    { value: "full", label: SOW_t("menu.full") },
                                    { value: "reduced", label: SOW_t("menu.reduced_motion") }
                                ] }) +
                            "</div>" +
                            "<label class='sow-hud__setting-row'>" +
                                "<span>" + esc(SOW_t("menu.free_zoom_out")) + "</span>" +
                                "<input class='sow-hud__switch' type='checkbox' role='switch' name='free_zoom_out' data-setting='free_zoom_out'" + (settings.free_zoom_out ? " checked" : "") + ">" +
                            "</label>" +
                            "<label class='sow-hud__setting-row'>" +
                                "<span>" + esc(SOW_t("menu.show_fps_ping")) + "</span>" +
                                "<input class='sow-hud__switch' type='checkbox' role='switch' name='show_fps_ping' data-setting='show_fps_ping'" + (settings.show_fps_ping === true ? " checked" : "") + ">" +
                            "</label>" +
                            (settings.show_dev_tools_available ? "<label class='sow-hud__setting-row'>" +
                                "<span>" + esc(SOW_t("menu.show_dev_tools")) + "</span>" +
                                "<input class='sow-hud__switch' type='checkbox' role='switch' name='show_dev_tools' data-setting='show_dev_tools'" + (settings.show_dev_tools ? " checked" : "") + ">" +
                            "</label>" : "") +
                            "<div class='sow-hud__setting-row sow-menu__settings-dropdown-row'><span>" + esc(SOW_t("menu.language")) + "</span>" +
                                SOW_renderDropdown({ key: "settings-language", className: "sow-menu__settings-dropdown", label: SOW_t("menu.language"), name: "locale", setting: "locale", value: typeof window.SOW_getLocale === "function" ? window.SOW_getLocale() : "en", options: localeOptions() }) +
                            "</div>" +
                        "</div></section>" +
                        renderSettingsPrivacySection(renderSettingsLink("https://shadowsofwar.io/privacy/", "menu.privacy")) +
                        "<section class='sow-menu__settings-section'><h3>" + esc(SOW_t("menu.game_links")) + "</h3><nav class='sow-menu__settings-links' aria-label='" + esc(SOW_t("menu.game_links")) + "'>" + renderSettingsLinks() + "</nav></section>" +
                    "</div>" +
                "</section>" +
            "</div>";
    }

    function authApi(path) {
        return "https://id.worldofunreal.com" + path;
    }

    function authSession() {
        if (typeof window.SOW_getWouSession === "function") return window.SOW_getWouSession();
        try {
            const token = window.localStorage.getItem("wou_session_token") || "";
            const user = JSON.parse(window.localStorage.getItem("wou_user_data") || "null");
            return { token: token, accountId: user && user.account_id || "", user: user };
        } catch {
            return { token: "", accountId: "", user: null };
        }
    }

    function authHeaders() {
        const headers: Record<string, string> = { "Content-Type": "application/json", "Accept": "application/json" };
        const session = authSession();
        if (session.token) headers.Authorization = "Bearer " + session.token;
        return headers;
    }

    function ensureAuthSession() {
        if (typeof window.SOW_ensureWouAnonymousSession === "function") {
            return window.SOW_ensureWouAnonymousSession();
        }
        return Promise.resolve(authSession());
    }

    function authIcon(provider) {
        if (provider === "google") return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='#4285F4' d='M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z'/><path fill='#34A853' d='M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z'/><path fill='#FBBC05' d='M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.06H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.94l2.85-2.22z'/><path fill='#EA4335' d='M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.06l3.66 2.84C6.71 7.3 9.14 5.38 12 5.38z'/></svg>";
        if (provider === "discord") return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='currentColor' d='M20.3 4.37a19.8 19.8 0 0 0-4.88-1.52.08.08 0 0 0-.08.04c-.21.38-.45.87-.61 1.25a18.27 18.27 0 0 0-5.49 0c-.17-.38-.41-.87-.62-1.25a.08.08 0 0 0-.08-.04A19.74 19.74 0 0 0 3.68 4.37a.07.07 0 0 0-.03.03C.53 9.05-.32 13.58.1 18.06a.08.08 0 0 0 .03.06 19.9 19.9 0 0 0 5.99 3.03.08.08 0 0 0 .08-.03c.46-.63.87-1.3 1.23-1.99a.08.08 0 0 0-.04-.11 12.3 12.3 0 0 1-1.87-.89.08.08 0 0 1-.01-.13 10.2 10.2 0 0 0 .37-.29.07.07 0 0 1 .08-.01c3.93 1.79 8.18 1.79 12.06 0a.07.07 0 0 1 .08.01c.12.1.25.2.37.29a.08.08 0 0 1-.01.13c-.59.35-1.21.65-1.87.89a.08.08 0 0 0-.04.11c.36.7.77 1.36 1.23 1.99a.08.08 0 0 0 .08.03 19.84 19.84 0 0 0 6-3.03.08.08 0 0 0 .03-.05c.5-5.18-.84-9.67-3.55-13.66a.06.06 0 0 0-.03-.03zM8.02 15.33c-1.18 0-2.16-1.09-2.16-2.42s.96-2.42 2.16-2.42 2.18 1.1 2.16 2.42-.96 2.42-2.16 2.42zm7.98 0c-1.18 0-2.16-1.09-2.16-2.42s.96-2.42 2.16-2.42 2.18 1.1 2.16 2.42-.95 2.42-2.16 2.42z'/></svg>";
        if (provider === "twitter") return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='currentColor' d='M18.24 2.25h3.31l-7.23 8.26 8.5 11.24h-6.66l-5.21-6.82-5.97 6.82H1.68l7.73-8.84L1.25 2.25h6.82l4.71 6.23 5.46-6.23zm-1.16 17.52h1.83L7.08 4.13H5.12l11.96 15.64z'/></svg>";
        return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='currentColor' d='M24 12.07a12 12 0 1 0-13.88 11.86v-8.38H7.08v-3.48h3.04V9.43c0-3.01 1.79-4.67 4.53-4.67 1.31 0 2.69.24 2.69.24v2.95h-1.51c-1.49 0-1.96.93-1.96 1.88v2.25h3.33l-.53 3.47h-2.8v8.38A12 12 0 0 0 24 12.07z'/></svg>";
    }

    function renderAuthSocial() {
        return [["google", "auth.google"], ["discord", "auth.discord_provider"], ["twitter", "auth.x_provider"], ["meta", "auth.meta"]].map(function (provider) {
            return "<button class='sow-auth__provider' type='button' data-command='wou_provider' data-provider='" + provider[0] + "'><span class='sow-auth__provider-icon sow-auth__provider-icon--" + provider[0] + "'>" + authIcon(provider[0]) + "</span><span>" + esc(SOW_t("auth.continue_with", { provider: SOW_t(provider[1]) })) + "</span><i>↗</i></button>";
        }).join("");
    }

    function finishWouLogin(data) {
        if (!data || !data.session_token || !data.account) throw new Error(SOW_t("auth.identity_incomplete"));
        try {
            window.localStorage.setItem("wou_session_token", data.session_token);
            window.localStorage.setItem("wou_user_data", JSON.stringify(data.account));
        } catch (error) {
            throw new Error(SOW_t("auth.account_save_failed"), { cause: error });
        }
        authBusy = false;
        authModalOpen = false;
        authOtpSent = false;
        authError = "";
        authNotice = "";
        window.dispatchEvent(new CustomEvent("wou:auth-state-change", {
            detail: { authenticated: true, isAuthenticated: true, user: data.account, token: data.session_token }
        }));
        window.location.reload();
    }

    function authJson(response) {
        return response.json().catch(function () { return {}; }).then(function (data) {
            if (!response.ok) throw new Error(data.error || SOW_t("auth.sign_in_unavailable"));
            return data;
        });
    }

    function requestAuthOtp() {
        if (authBusy) return;
        const email = String(authEmail || "").trim();
        if (!/^\S+@\S+\.\S+$/.test(email)) {
            authError = SOW_t("auth.valid_email");
            authNotice = "";
            render();
            return;
        }
        authEmail = email;
        authBusy = true;
        authError = "";
        authNotice = "";
        render();
        ensureAuthSession().then(function (session) {
            return fetch(authApi("/api/v1/auth/otp/request"), {
            method: "POST",
            headers: authHeaders(),
            body: JSON.stringify({
                email: authEmail,
                account_id: session.accountId || null,
                context: "shadows_of_war",
                newsletter_opt_in: false
            })
            }).then(authJson);
        }).then(function (data) {
            authOtpSent = true;
            authBusy = false;
            authNotice = data.message || SOW_t("auth.verification_sent");
            render();
        }).catch(function (error) {
            authBusy = false;
            authError = error && error.message ? error.message : SOW_t("auth.send_code_failed");
            render();
        });
    }

    function verifyAuthOtp() {
        if (authBusy) return;
        const code = String(authCode || "").trim();
        if (!/^\d{6}$/.test(code)) {
            authError = SOW_t("auth.six_digit_code");
            authNotice = "";
            render();
            return;
        }
        authBusy = true;
        authError = "";
        authNotice = "";
        render();
        ensureAuthSession().then(function (session) {
            return fetch(authApi("/api/v1/auth/otp/verify"), {
            method: "POST",
            headers: authHeaders(),
            body: JSON.stringify({
                email: authEmail,
                code: code,
                account_id: session.accountId || null,
                context: "shadows_of_war"
            })
            }).then(authJson);
        }).then(finishWouLogin).catch(function (error) {
            authBusy = false;
            authError = error && error.message ? error.message : SOW_t("auth.invalid_code");
            render();
        });
    }

    function resetAuthFlow() {
        authEmail = "";
        authCode = "";
        authOtpSent = false;
        authBusy = false;
        authError = "";
        authNotice = "";
    }

    function renderAuthModal() {
        const emailPanel = authOtpSent
            ? "<form class='sow-auth__form' data-auth-form='verify'><label>" + esc(SOW_t("auth.code_sent_to")) + " <strong>" + esc(authEmail) + "</strong></label><input class='sow-auth__code' data-auth-field='code' inputmode='numeric' autocomplete='one-time-code' maxlength='6' value='" + esc(authCode) + "' placeholder='000000' aria-label='" + esc(SOW_t("auth.verification_code")) + "' required><button class='sow-auth__submit sow-auth__submit--cyan' type='submit'" + (authBusy ? " disabled" : "") + ">" + esc(authBusy ? SOW_t("auth.checking") : SOW_t("auth.verify_code")) + "</button><div class='sow-auth__form-links'><button type='button' data-command='auth_change_email'>" + esc(SOW_t("auth.change_email")) + "</button><button type='button' data-command='auth_resend'" + (authBusy ? " disabled" : "") + ">" + esc(SOW_t("auth.resend")) + "</button></div></form>"
            : "<form class='sow-auth__form' data-auth-form='request'><label for='sow-auth-email'>" + esc(SOW_t("auth.email_address")) + "</label><input id='sow-auth-email' class='sow-auth__input' data-auth-field='email' type='email' autocomplete='email' value='" + esc(authEmail) + "' placeholder='" + esc(SOW_t("auth.email_placeholder")) + "' required><button class='sow-auth__submit' type='submit'" + (authBusy ? " disabled" : "") + ">" + esc(authBusy ? SOW_t("auth.sending") : SOW_t("auth.send_code")) + "</button></form>";
        const error = authError ? "<div class='sow-auth__message sow-auth__message--error' role='alert'>" + esc(authError) + "</div>" : "";
        const notice = authNotice ? "<div class='sow-auth__message sow-auth__message--notice' role='status'>" + esc(authNotice) + "</div>" : "";
        return "<div class='sow-menu__overlay' data-menu-overlay='auth' data-auth-overlay><section class='sow-menu__modal sow-auth' role='dialog' aria-modal='true' aria-label='" + esc(SOW_t("auth.account")) + "'>" +
            "<div class='sow-auth__glow sow-auth__glow--cyan'></div><div class='sow-auth__head'><div class='sow-auth__logos'><img class='sow-auth__game-logo' src='/sow-long.svg' alt='" + esc(SOW_t("menu.brand")) + "'><span class='sow-auth__logo-divider' aria-hidden='true'></span><img class='sow-auth__wou-logo' src='https://worldofunreal.com/wouid.svg' alt='WouID'></div><button class='sow-menu__icon-button' type='button' data-command='close_auth' aria-label='" + esc(SOW_t("auth.close")) + "'>×</button></div>" +
            error + notice + "<div class='sow-auth__body'>" + emailPanel + "<div class='sow-auth__social-list'>" + renderAuthSocial() + "</div></div>" +
            renderSiteLink("/terms/", "auth.terms", "sow-auth__terms") + "</section></div>";
    }

    /* POKI_RENDER_REPLACEMENT_END */

    function renderSettingsPrivacySection(privacyLinkMarkup) {
        let deleteControl = "";
        if (selfCreds()) {
            deleteControl = deleteBusy
                ? "<p class='sow-menu__settings-delete-status' role='status'>" + esc(SOW_t("profile.deleting_account")) + "</p>"
                : deleteArmed
                    ? "<section class='sow-menu__settings-confirm' role='alertdialog' aria-label='" + esc(SOW_t("profile.delete_account")) + "'><p>" + esc(SOW_t("profile.permanent_delete")) + "</p><div class='sow-menu__settings-confirm-actions'><button class='sow-menu__secondary' type='button' data-command='cancel_delete'>" + esc(SOW_t("lobbies.cancel")) + "</button><button class='sow-menu__danger' type='button' data-command='confirm_delete'>" + esc(SOW_t("profile.delete_confirm")) + "</button></div></section>"
                    : "<button class='sow-menu__danger sow-menu__settings-delete' type='button' data-command='request_delete'>" + esc(SOW_t("profile.delete_account")) + "</button>";
        }
        const error = deleteError
            ? "<p class='sow-menu__settings-delete-error' role='alert'>" + esc(deleteError) + "</p>"
            : "";
        const privacyLink = privacyLinkMarkup
            ? "<nav class='sow-menu__settings-links sow-menu__settings-privacy-links' aria-label='" + esc(SOW_t("menu.privacy")) + "'>" + privacyLinkMarkup + "</nav>"
            : "";
        return "<section class='sow-menu__settings-section sow-menu__settings-privacy'><h3>" + esc(SOW_t("menu.privacy")) + "</h3>" + privacyLink + deleteControl + error + "</section>";
    }

    function renderScreenPanel(screen) {
        if (screen === "home") return renderHome();
        if (screen === "campaign") return renderCampaign();
        if (screen === "browser") return renderBrowser();
        if (screen === "create") return renderCreate();
        if (screen === "queue") return renderQueue();
        if (screen === "profile") return renderProfile();
        if (screen === "heroes") return renderHeroes();
        if (screen === "store") return renderStore();
        return "";
    }

    function screenNav(screen) {
        return screen === "store" ? "store" : screen === "heroes" ? "heroes" : screen === "profile" ? "profile" : "battle";
    }

    function screenShellClass(screen) {
        return "sow-menu__shell" + (screen === "store" ? " sow-menu__store" : screen === "heroes" ? " sow-menu__heroes" : screen === "profile" ? " sow-profile" : "");
    }

    function screenBackdropClass(screen) {
        return "sow-menu__backdrop" + (screen === "store" ? " sow-store__backdrop" : screen === "heroes" ? " sow-heroes__backdrop" : "");
    }

    function renderFrame(screen) {
        return "<div class='" + screenBackdropClass(screen) + "' data-menu-backdrop></div>" +
            "<div class='" + screenShellClass(screen) + "'>" +
                renderTopbar() +
                "<div class='sow-menu__screen-stage' data-screen-stage>" + renderScreenPanel(screen) + "</div>" +
                renderMainNav(screenNav(screen)) + renderMenuAttribution() +
            "</div>" + renderPasswordModal() + renderProfileDetail();
    }

    function panelFromMarkup(markup) {
        const template = document.createElement("template");
        template.innerHTML = String(markup || "").trim();
        return template.content.firstElementChild;
    }

    function syncOverlay(key, markup) {
        const selector = "[data-menu-overlay='" + key + "']";
        const current = root.querySelector(selector);
        if (!markup) {
            if (current) current.remove();
            return;
        }
        const next = panelFromMarkup(markup);
        if (!next) return;
        if (!current) root.appendChild(next);
        else if (current.outerHTML !== next.outerHTML) current.replaceWith(next);
    }

    function updateTopbar() {
        const topbar = root.querySelector(".sow-menu__topbar");
        if (!topbar || !state) return;
        const nameInput = topbar.querySelector("[data-role='display-name']");
        const name = state.player_name || SOW_t("menu.anonymous");
        if (nameInput && document.activeElement !== nameInput) nameInput.value = name;
        const avatar = topbar.querySelector(".sow-menu__avatar");
        const avatarUrl = avatarImage();
        if (avatar && avatar.dataset.avatarUrl !== avatarUrl) {
            avatar.style.backgroundImage = "url(" + JSON.stringify(avatarUrl) + ")";
            avatar.dataset.avatarUrl = avatarUrl;
        }
        const auth: SowAuthState = { linked: false, pending: false };
        /* POKI_SHARED_AUTH_LOOKUP_BEGIN */
        if (typeof window.SOW_getAuthState === "function") Object.assign(auth, window.SOW_getAuthState());
        /* POKI_SHARED_AUTH_LOOKUP_END */
        const showSignIn = portalOffersSignIn() && !(auth.linked || auth.pending);
        const actions = topbar.querySelector(".sow-menu__top-actions");
        const settingsButton = actions && actions.querySelector("[data-command='toggle_settings']");
        let signIn = actions && actions.querySelector<HTMLButtonElement>(".sow-menu__signin");
        if (showSignIn) {
            if (!signIn && actions) {
                signIn = document.createElement("button");
                signIn.className = "sow-menu__signin";
                signIn.type = "button";
                signIn.dataset.command = "sign_in";
                actions.insertBefore(signIn, settingsButton);
            }
            if (signIn) signIn.textContent = SOW_t("menu.sign_in");
        } else if (signIn) {
            signIn.remove();
        }
    }

    function updateFrameChrome(screen) {
        const backdrop = root.querySelector("[data-menu-backdrop]");
        const shell = root.querySelector(".sow-menu__shell");
        const backdropClass = screenBackdropClass(screen);
        const shellClass = screenShellClass(screen);
        if (backdrop && backdrop.className !== backdropClass) backdrop.className = backdropClass;
        if (shell && shell.className !== shellClass) shell.className = shellClass;
        root.querySelectorAll("[data-nav-screen]").forEach(function (item) {
            const active = item.dataset.navScreen === screenNav(screen);
            const navItem = MAIN_NAV_ITEMS.find(function (candidate) {
                return candidate[0] === item.dataset.navScreen;
            });
            const labelText = navItem ? SOW_t(navItem[2]) : "";
            const labelNode = item.querySelector("small");
            if (labelNode && labelText) labelNode.textContent = labelText;
            if (labelText) item.setAttribute("aria-label", labelText);
            item.classList.toggle("is-active", active);
            if (active) item.setAttribute("aria-current", "page");
            else item.removeAttribute("aria-current");
        });
        updateTopbar();
    }

    function activeScreenPanel() {
        const stage = root.querySelector("[data-screen-stage]");
        return stage && (stage.querySelector("[data-screen-panel].is-active") || stage.lastElementChild);
    }

    function syncScreenPanel(screen, screenChanged) {
        const stage = root.querySelector("[data-screen-stage]");
        const next = panelFromMarkup(renderScreenPanel(screen));
        if (!stage || !next) return;
        if (screenChanged) {
            const settings = state && state.settings || {};
            sowScreenMotion.show(stage, next, !!settings.reduced_motion);
            return;
        }
        const current = activeScreenPanel();
        if (current) current.replaceWith(next);
        else stage.appendChild(next);
        next.classList.add("is-active");
    }

    function render() {
        if (!state) return;
        if (activeMenuPointerId !== null) {
            renderPending = true;
            return;
        }
        renderPending = false;
        const screen = currentScreen();
        if (screen !== "heroes") skinPickerOpen = false;
        if (screen !== "heroes") heroInfoOpen = false;
        const screenChanged = previousScreen !== screen;
        const sameScreen = !screenChanged;
        if (screenChanged) dropdownOpenKey = null;
        if (screen === "create" && previousScreen !== "create") {
            createDraft = cloneConfig();
            createOffline = !!state.custom_game_is_sp;
            createPrivate = !!state.custom_game_is_private;
            createPassword = "";
        }
        if (screen !== "create") {
            createDraft = null;
            createOffline = false;
            createPrivate = false;
            createPassword = "";
        }
        previousScreen = screen;
        const nextHero = screen === "profile"
            ? profileHeroImage(profileData && profileData.preferred_leader)
            : heroImage();
        if (root.dataset.hero !== nextHero) {
            root.style.setProperty("--sow-hero", "url(\"" + nextHero + "\")");
            root.dataset.hero = nextHero;
        }
        root.dataset.screen = screen;
        root.dataset.ready = typeof window.SOW_menu_command === "function" ? "true" : "false";
        root.hidden = state.phase !== "MainMenu";

        const activeEl = document.activeElement;
        const isTyping = activeEl && root.contains(activeEl) && (activeEl.tagName === "INPUT" || activeEl.tagName === "TEXTAREA" || activeEl.tagName === "SELECT");
        const activeRole = activeEl && activeEl.dataset ? activeEl.dataset.role : null;
        const activeName = activeEl ? activeEl.name : null;
        const activeVal = isTyping ? activeEl.value : null;
        const selStart = isTyping && typeof activeEl.selectionStart === "number" ? activeEl.selectionStart : null;
        const selEnd = isTyping && typeof activeEl.selectionEnd === "number" ? activeEl.selectionEnd : null;
        const currentPanel = activeScreenPanel();
        let scrollTop = null;
        if (sameScreen) {
            const currentScrollOwner = screen === "create"
                ? currentPanel && currentPanel.querySelector(".sow-create__scroll")
                : currentPanel;
            if (currentScrollOwner) scrollTop = currentScrollOwner.scrollTop;
        }

        if (!root.querySelector("[data-screen-stage]")) {
            root.innerHTML = renderFrame(screen);
            const initialPanel = activeScreenPanel();
            if (initialPanel) initialPanel.classList.add("is-active");
        } else {
            updateFrameChrome(screen);
            syncScreenPanel(screen, screenChanged);
        }
        syncOverlay("password", renderPasswordModal());
        syncOverlay("settings", settingsOpen ? renderSettings() : "");
        syncOverlay("auth", authModalOpen ? renderAuthModal() : "");
        syncOverlay("profile-detail", profileOpen ? renderProfileDetail() : "");
        syncOverlay("skin-picker", typeof renderSkinPickerModal === "function" ? renderSkinPickerModal() : "");
        syncOverlay("hero-info", typeof renderHeroInfoModal === "function" ? renderHeroInfoModal() : "");
        syncOverlay("purchase", typeof renderPurchaseModal === "function" ? renderPurchaseModal() : "");

        if (sameScreen && isTyping) {
            let restored = null;
            if (activeRole) {
                restored = root.querySelector("[data-role='" + activeRole + "']");
            } else if (activeName) {
                restored = root.querySelector("[name='" + activeName + "']");
            }
            if (restored) {
                if (activeVal != null) restored.value = activeVal;
                try {
                    restored.focus();
                    if (selStart != null && selEnd != null) {
                        restored.setSelectionRange(selStart, selEnd);
                    }
                } catch {}
            }
        }
        updateDynamic();
        if (screen === "store") loadStoreCatalog();
        if (screen === "store" && screenChanged && typeof window.SOW_trackExperienceEvent === "function") {
            window.SOW_trackExperienceEvent({ name: "store_open" });
        }
        if (sameScreen && scrollTop !== null) {
            const nextScrollOwner = screen === "create"
                ? activeScreenPanel() && activeScreenPanel().querySelector(".sow-create__scroll")
                : activeScreenPanel();
            if (nextScrollOwner) nextScrollOwner.scrollTop = scrollTop;
        }
        syncDropdowns();
        lastRenderKey = renderKey();
        updateLobbyViews();
    }

    function progressionFromState() {
        const xp = Math.max(0, Number(state && state.xp) || 0);
        return {
            xp: xp,
            level: Math.max(1, Number(state && state.level) || Math.floor(xp / 100) + 1),
            crowns: Math.max(0, Number(state && state.crowns) || 0),
            laurels: Math.max(0, Number(state && state.laurels) || 0),
            gems: Math.max(0, Number(state && state.gems) || 0)
        };
    }

    function pendingRewardReceipts() {
        return (state && state.reward_receipts || []).filter(function (receipt) {
            const tutorialApplied = receipt && receipt.id === "tutorial" && receipt.match_id === "tutorial" &&
                receipt.status === "applied";
            const campaignApplied = receipt && typeof receipt.id === "string" &&
                /^campaign:[a-z][a-z0-9_]{0,63}$/.test(receipt.id) && receipt.match_id === receipt.id;
            return receipt && receipt.id && receipt.status === "applied" && !receipt.presented_at &&
                (tutorialApplied || campaignApplied || receipt.verification_status === "verified" || receipt.verification_status === "rejected");
        });
    }

    function reducedRewardMotion() {
        return !!(state && state.settings && state.settings.reduced_motion) ||
            !!(window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
    }

    function writeProgression(values) {
        if (!values) return;
        displayedProgression = {
            xp: Math.max(0, Number(values.xp) || 0),
            level: Math.max(1, Number(values.level) || Math.floor((Number(values.xp) || 0) / 100) + 1),
            crowns: Math.max(0, Number(values.crowns) || 0),
            laurels: Math.max(0, Number(values.laurels) || 0),
            gems: Math.max(0, Number(values.gems) || 0)
        };
        const progression = root.querySelector("[data-progression]");
        if (!progression) return;
        const levelValue = progression.querySelector("[data-progression-level-value]");
        const xpValue = progression.querySelector("[data-progression-xp-value]");
        const xpFill = progression.querySelector("[data-progression-xp-fill]");
        const crownsValue = progression.querySelector("[data-progression-crowns-value]");
        const laurelsValue = progression.querySelector("[data-progression-laurels-value]");
        const gemsValue = progression.querySelector("[data-progression-gems-value]");
        if (levelValue) levelValue.textContent = displayedProgression.level;
        if (xpValue) xpValue.textContent = Math.floor(displayedProgression.xp) + " " + SOW_t("menu.xp");
        if (xpFill) xpFill.style.width = (displayedProgression.xp % 100) + "%";
        if (crownsValue) crownsValue.textContent = String(Math.round(displayedProgression.crowns));
        if (laurelsValue) laurelsValue.textContent = String(Math.round(displayedProgression.laurels));
        if (gemsValue) gemsValue.textContent = String(Math.round(displayedProgression.gems));
    }

    function ensureProgressionDisplay() {
        if (displayedProgression) return;
        const canonical = progressionFromState();
        const pending = pendingRewardReceipts().filter(function (receipt) { return !rewardAnimationShown.has(receipt.id); });
        if (pending.length) {
            const totals = pending.reduce(function (sum, receipt) {
                sum.xp += Math.max(0, Number(receipt.xp) || 0);
                sum.crowns += Math.max(0, Number(receipt.crowns) || 0);
                sum.laurels += Math.max(0, Number(receipt.laurels) || 0);
                return sum;
            }, { xp: 0, crowns: 0, laurels: 0 });
            canonical.xp = Math.max(0, canonical.xp - totals.xp);
            canonical.crowns = Math.max(0, canonical.crowns - totals.crowns);
            canonical.laurels = Math.max(0, canonical.laurels - totals.laurels);
            canonical.level = Math.floor(canonical.xp / 100) + 1;
        }
        writeProgression(canonical);
    }

    function animateProgressionTo(target, duration, token, levelOverride?) {
        const from = Object.assign({}, displayedProgression || progressionFromState());
        if (reducedRewardMotion() || duration <= 0) {
            writeProgression(target);
            if (token === rewardAnimationToken) progressionAnimationTarget = null;
            return Promise.resolve(true);
        }
        progressionAnimationTarget = Object.assign({}, target);
        const start = performance.now();
        return new Promise(function (resolve) {
            let frameId = 0;
            function frame(now) {
                if (token !== rewardAnimationToken) {
                    if (progressionAnimationFrame === frameId) progressionAnimationFrame = 0;
                    resolve(false);
                    return;
                }
                if (root.hidden || document.visibilityState !== "visible") {
                    progressionAnimationFrame = 0;
                    progressionAnimationTarget = null;
                    writeProgression(progressionFromState());
                    resolve(false);
                    return;
                }
                const t = Math.min(1, (now - start) / duration);
                const eased = 1 - Math.pow(1 - t, 3);
                const currentXp = from.xp + (target.xp - from.xp) * eased;
                writeProgression({
                    xp: currentXp,
                    level: levelOverride == null ? Math.floor(currentXp / 100) + 1 : levelOverride,
                    crowns: from.crowns + (target.crowns - from.crowns) * eased,
                    laurels: from.laurels + (target.laurels - from.laurels) * eased,
                    gems: from.gems + (target.gems - from.gems) * eased
                });
                if (t < 1) {
                    frameId = requestAnimationFrame(frame);
                    progressionAnimationFrame = frameId;
                }
                else {
                    if (progressionAnimationFrame === frameId) progressionAnimationFrame = 0;
                    writeProgression(target);
                    progressionAnimationTarget = null;
                    if (!activeRewardStage && duration >= 900) {
                        if (from.xp !== target.xp) pulseRewardCounter("[data-progression-xp-value]", false);
                        if (from.crowns !== target.crowns) pulseRewardCounter("[data-progression-crowns-value]", false);
                        if (from.laurels !== target.laurels) pulseRewardCounter("[data-progression-laurels-value]", false);
                        if (from.gems !== target.gems) pulseRewardCounter("[data-progression-gems-value]", false);
                        if (target.level > from.level) pulseRewardCounter("[data-progression-level-value]", true);
                    }
                    resolve(true);
                }
            }
            frameId = requestAnimationFrame(frame);
            progressionAnimationFrame = frameId;
        });
    }

    function removeRewardStage() {
        const layer = activeRewardStage;
        activeRewardStage = null;
        if (!layer) return;
        if (typeof layer.getAnimations === "function") layer.getAnimations({ subtree: true }).forEach(function (motion) { motion.cancel(); });
        layer.remove();
        if (layer._shell && "inert" in layer._shell) layer._shell.inert = layer._shellWasInert;
        if (layer._restoreFocus && layer._restoreFocus.isConnected && !root.hidden) layer._restoreFocus.focus({ preventScroll: true });
    }

    function cancelRewardAnimation() {
        rewardAnimationToken += 1;
        rewardAnimationRunning = false;
        progressionAnimationTarget = null;
        removeRewardStage();
    }

    function createRewardStage(stages, skip) {
        const layer = document.createElement("section");
        layer.className = "sow-menu__reward-stage";
        layer.setAttribute("role", "dialog");
        layer.setAttribute("aria-modal", "true");
        layer.setAttribute("aria-label", SOW_t("endgame.match_result"));
        const cards = document.createElement("div");
        cards.className = "sow-menu__reward-cards";
        const items = stages.map(function (stage, index) {
            const card = document.createElement("div");
            card.className = "sow-menu__reward-card sow-menu__reward-card--" + stage.kind;
            card.style.animationDelay = (120 + index * 140) + "ms";
            card.setAttribute("role", "group");
            const levelLabel = stage.kind === "level" ? SOW_t("profile.level", { level: stage.level }) : null;
            card.setAttribute("aria-label", stage.kind === "leader" ? stage.label :
                levelLabel || stage.amount + " " + SOW_t(stage.kind === "xp" ? "menu.xp" : stage.kind === "crowns" ? "store.crowns" : "store.laurels"));
            const medallion = document.createElement("span");
            medallion.className = "sow-menu__reward-medallion";
            medallion.setAttribute("aria-hidden", "true");
            const emblem = document.createElement(stage.kind === "xp" || stage.kind === "level" ? "span" : "img");
            emblem.className = "sow-menu__reward-emblem";
            if (stage.kind === "xp") emblem.textContent = "XP";
            else if (stage.kind === "level") emblem.textContent = "↑";
            else if (stage.kind === "leader") {
                emblem.src = asset("gameplay/avatars/" + stage.slug + ".webp");
                emblem.alt = "";
            }
            else emblem.src = currencyAsset(stage.kind === "crowns" ? "crown" : "laurel");
            medallion.appendChild(emblem);
            const amount = document.createElement("strong");
            amount.className = "sow-menu__reward-amount";
            if (stage.kind !== "leader") amount.setAttribute("aria-hidden", "true");
            amount.textContent = levelLabel || (stage.kind === "leader" ? stage.label : "+0");
            card.appendChild(medallion);
            card.appendChild(amount);
            cards.appendChild(card);
            return { stage: stage, card: card, amount: amount };
        });
        const close = document.createElement("button");
        close.className = "sow-menu__reward-skip sow-menu__icon-button";
        close.type = "button";
        close.textContent = "×";
        close.setAttribute("aria-label", SOW_t("menu.close"));
        layer.addEventListener("click", function (event) {
            event.preventDefault();
            event.stopPropagation();
            skip();
        });
        layer.addEventListener("keydown", function (event) {
            if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); skip(); }
        });
        layer.appendChild(cards);
        layer.appendChild(close);
        layer._shell = root.querySelector(".sow-menu__shell");
        layer._shellWasInert = layer._shell && layer._shell.inert;
        layer._restoreFocus = document.activeElement;
        root.appendChild(layer);
        activeRewardStage = layer;
        if (layer._shell && "inert" in layer._shell) layer._shell.inert = true;
        close.focus({ preventScroll: true });
        return items;
    }

    function revealRewardAmounts(items, token) {
        const start = performance.now();
        return new Promise(function (resolve) {
            function frame(now) {
                if (token !== rewardAnimationToken || root.hidden || document.visibilityState !== "visible") { resolve(false); return; }
                const elapsed = now - start;
                items.forEach(function (item, index) {
                    const t = Math.max(0, Math.min(1, (elapsed - 300 - index * 140) / 650));
                    const value = item.stage.kind === "level"
                        ? SOW_t("profile.level", { level: item.stage.level })
                        : item.stage.kind === "leader" ? item.stage.label
                        : "+" + Math.round(item.stage.amount * (1 - Math.pow(1 - t, 3)));
                    if (item.amount.textContent !== value) item.amount.textContent = value;
                });
                if (elapsed < 1450) requestAnimationFrame(frame);
                else resolve(true);
            }
            requestAnimationFrame(frame);
        });
    }

    function flyRewardCard(item, token) {
        const target = root.querySelector(item.stage.selector);
        if (!target || typeof item.card.animate !== "function") { item.card.style.visibility = "hidden"; return Promise.resolve(true); }
        const from = item.card.getBoundingClientRect();
        const to = target.getBoundingClientRect();
        const dx = to.left + to.width / 2 - from.left - from.width / 2;
        const dy = to.top + to.height / 2 - from.top - from.height / 2;
        const bend = Math.min(160, Math.max(70, Math.hypot(dx, dy) * .2));
        const controlX = dx / 2 + (dx < 0 ? -bend * .6 : bend * .6);
        const controlY = dy / 2 - bend;
        const path = [];
        for (let i = 0; i <= 12; i++) {
            const t = i / 12;
            const inverse = 1 - t;
            const x = 2 * inverse * t * controlX + t * t * dx;
            const y = 2 * inverse * t * controlY + t * t * dy;
            path.push({
                offset: t,
                transform: "translate3d(" + x + "px," + y + "px,0) scale(" + (1 - .82 * t) + ") rotate(" + (Math.sin(Math.PI * t) * 8) + "deg)",
                opacity: 1 - .4 * Math.max(0, (t - .8) / .2)
            });
        }
        return new Promise(function (resolve) {
            const motion = item.card.animate(path, { duration: 520, easing: "cubic-bezier(.45,0,.55,1)", fill: "forwards" });
            motion.onfinish = function () { item.card.style.visibility = "hidden"; resolve(token === rewardAnimationToken); };
            motion.oncancel = function () { resolve(false); };
        });
    }

    function pulseRewardCounter(selector, levelUp) {
        if (reducedRewardMotion()) return;
        const counter = root.querySelector(selector);
        if (!counter) return;
        const className = levelUp ? "is-reward-level-up" : "is-reward-value-hit";
        counter.classList.remove(className);
        void counter.offsetWidth;
        counter.classList.add(className);
        window.setTimeout(function () { counter.classList.remove(className); }, levelUp ? 900 : 640);
    }

    function acknowledgeShownReceipts(accountId) {
        if (!state || !accountId || state.account_id !== accountId) return;
        const now = Date.now();
        const ids = pendingRewardReceipts().filter(function (receipt) {
            return rewardAnimationShown.has(receipt.id) && now - (rewardAckTimes[receipt.id] || 0) >= 30000;
        }).map(function (receipt) { return receipt.id; });
        if (ids.length) {
            ids.forEach(function (id) { rewardAckTimes[id] = now; });
            send("acknowledge_reward_receipts", { account_id: accountId, receipt_ids: ids });
            if (rewardAckRetryTimer === null) {
                rewardAckRetryTimer = window.setTimeout(function () {
                    rewardAckRetryTimer = null;
                    if (document.visibilityState === "visible" && state && state.phase === "MainMenu") acknowledgeShownReceipts(accountId);
                }, 30000);
            }
        }
    }

    function runRewardPresentation(receipts) {
        if (rewardAnimationRunning) return;
        rewardAnimationRunning = true;
        const token = ++rewardAnimationToken;
        const accountId = state && state.account_id;
        const ids = receipts.map(function (receipt) { return receipt.id; });
        const totals = receipts.reduce(function (sum, receipt) {
            sum.xp += Math.max(0, Number(receipt.xp) || 0);
            sum.crowns += Math.max(0, Number(receipt.crowns) || 0);
            sum.laurels += Math.max(0, Number(receipt.laurels) || 0);
            return sum;
        }, { xp: 0, crowns: 0, laurels: 0 });
        const unlockedLeaderIds = Array.from(new Set(receipts.reduce(function (ids, receipt) {
            return ids.concat(Array.isArray(receipt.unlocked_leaders) ? receipt.unlocked_leaders : []);
        }, []))).filter(function (id: string) { return /^[a-z][a-z0-9_]{0,63}$/.test(id); });
        const leaderStages = unlockedLeaderIds.map(function (slug: string) {
            const leader = leaderById(slug);
            const name = leader ? leaderDisplayName(leader) : slug.replace(/_/g, " ");
            return { kind: "leader", amount: 0, slug: slug,
                label: SOW_t("heroes.leader_unlocked", { name: name }) };
        });
        const canonical = progressionFromState();
        const target = canonical;
        const startLevel = (displayedProgression || canonical).level;
        const gainedLevels = new Set<number>();
        receipts.forEach(function (receipt) {
            if (Number.isSafeInteger(receipt.level_before) && Number.isSafeInteger(receipt.level_after)) {
                for (let level = Number(receipt.level_before) + 1; level <= Number(receipt.level_after); level++) gainedLevels.add(level);
            }
        });
        if (!receipts.every(function (receipt) { return Number.isSafeInteger(receipt.level_before); })) {
            for (let level = startLevel + 1; level <= target.level; level++) gainedLevels.add(level);
        }
        const levelStages = Array.from(gainedLevels).sort(function (a, b) { return a - b; }).map(function (level) {
            return {
                kind: "level",
                amount: level,
                level: level,
                selector: "[data-progression-level-value]"
            };
        });
        const stages = [
            { kind: "xp", amount: totals.xp, selector: "[data-progression-xp-value]" },
            { kind: "crowns", amount: totals.crowns, selector: "[data-progression-crowns-value]" },
            { kind: "laurels", amount: totals.laurels, selector: "[data-progression-laurels-value]" },
            ...leaderStages,
            ...levelStages
        ].filter(function (stage) { return stage.kind === "leader" || stage.amount > 0; });

        function finish(shown) {
            if (token !== rewardAnimationToken || !state || state.account_id !== accountId) return;
            rewardAnimationToken += 1;
            rewardAnimationRunning = false;
            removeRewardStage();
            if (!shown) {
                writeProgression(progressionFromState());
                if (document.visibilityState === "visible") maybePresentRewards();
                return;
            }
            writeProgression(progressionFromState());
            ids.forEach(function (id) { rewardAnimationShown.add(id); });
            console.info("[menu-flow] rewards and levels presented", ids, "cycle", state.loader_cycle_id);
            acknowledgeShownReceipts(accountId);
            maybePresentRewards();
        }

        function complete(shown) {
            if (!shown || !activeRewardStage) { finish(shown); return; }
            activeRewardStage.classList.add("is-exiting");
            window.setTimeout(function () { finish(true); }, 240);
        }

        function next(index) {
            if (token !== rewardAnimationToken || root.hidden || document.visibilityState !== "visible") {
                finish(false);
                return;
            }
            if (index >= stages.length) {
                const latest = progressionFromState();
                const current = displayedProgression || progressionFromState();
                const changed = latest.xp !== current.xp || latest.level !== current.level || latest.crowns !== current.crowns ||
                    latest.laurels !== current.laurels || latest.gems !== current.gems;
                if (changed) animateProgressionTo(latest, 900, token).then(complete);
                else complete(true);
                return;
            }
            const item = items[index];
            const stage = item.stage;
            if (stage.kind === "leader") {
                window.setTimeout(function () { next(index + 1); }, 900);
                return;
            }
            const nextValues = Object.assign({}, displayedProgression || progressionFromState());
            const oldLevel = nextValues.level;
            if (stage.kind === "level") {
                nextValues.level = stage.level;
            } else {
                nextValues[stage.kind] = target[stage.kind];
                nextValues.level = levelStages.length
                    ? oldLevel
                    : Math.floor(nextValues.xp / 100) + 1;
            }
            flyRewardCard(item, token).then(function (arrived) {
                if (!arrived) { finish(false); return; }
                if (stage.kind === "level") {
                    writeProgression(nextValues);
                    pulseRewardCounter(stage.selector, true);
                    window.setTimeout(function () { next(index + 1); }, 250);
                    return;
                }
                const heldLevel = levelStages.length ? oldLevel : null;
                animateProgressionTo(nextValues, 380, token, heldLevel).then(function (shown) {
                    if (!shown) { finish(false); return; }
                    pulseRewardCounter(stage.selector, false);
                    next(index + 1);
                });
            });
        }
        if (reducedRewardMotion()) {
            if (!leaderStages.length) { finish(true); return; }
            const staticItems = createRewardStage(stages, function () { finish(true); });
            staticItems.forEach(function (item) {
                item.amount.textContent = item.stage.kind === "leader" ? item.stage.label
                    : item.stage.kind === "level" ? SOW_t("profile.level", { level: item.stage.level })
                    : "+" + item.stage.amount;
            });
            return;
        }
        if (!stages.length) { finish(true); return; }
        const items = createRewardStage(stages, function () { finish(true); });
        revealRewardAmounts(items, token).then(function (shown) { if (shown) next(0); else finish(false); });
    }

    function maybePresentRewards() {
        if (!state) return;
        const pending = pendingRewardReceipts().filter(function (receipt) { return !rewardAnimationShown.has(receipt.id); });
        menuFlow.update(state, {
            visible: !root.hidden && document.visibilityState === "visible",
            ready: rewardPresentationReady,
            running: rewardAnimationRunning,
            receipts: pending,
            screen: currentScreen()
        });
        if (lastMenuFlowPhase !== menuFlow.phase()) {
            lastMenuFlowPhase = menuFlow.phase();
            console.info("[menu-flow]", lastMenuFlowPhase, "cycle", state.loader_cycle_id,
                "match", state.menu_return && state.menu_return.match_id);
        }
        const guide = window.SOW_menu_guide;
        if (guide) guide.update(state, menuGuideFacts());
        if (menuFlow.canPresent() && !rewardAnimationRunning && pending.length) {
            ensureProgressionDisplay();
            runRewardPresentation(pending);
            return;
        }
        if (state.phase === "MainMenu") acknowledgeShownReceipts(state.account_id);
        const destination = menuFlow.takeDestination();
        if (destination === "store" && window.SOW_PORTAL !== "poki" && window.SOW_PORTAL !== "jest") {
            storeOpen = true;
            render();
            return;
        }
    }

    function menuGuideFacts() {
        return { screen: currentScreen() };
    }

    function updateDynamic() {
        syncHeroesRotationCountdown();
        if (!state || root.hidden) return;
        if (storeOpen && typeof loadStoreCatalog === "function") loadStoreCatalog();
        ensureProgressionDisplay();
        if (reducedRewardMotion() && (rewardAnimationRunning || progressionAnimationTarget)) {
            cancelRewardAnimation();
            writeProgression(progressionFromState());
        }
        const pendingRewards = pendingRewardReceipts().filter(function (receipt) { return !rewardAnimationShown.has(receipt.id); });
        const holdBundleGems = purchaseModal && purchaseModal.type === "bundle" && !purchaseModal.gemPresented;
        if (!rewardAnimationRunning && !pendingRewards.length) {
            const canonical = progressionFromState();
            const changed = canonical.xp !== displayedProgression.xp || canonical.level !== displayedProgression.level ||
                canonical.crowns !== displayedProgression.crowns || canonical.laurels !== displayedProgression.laurels || canonical.gems !== displayedProgression.gems;
            const sameTarget = progressionAnimationTarget && canonical.xp === progressionAnimationTarget.xp &&
                canonical.level === progressionAnimationTarget.level && canonical.crowns === progressionAnimationTarget.crowns &&
                canonical.laurels === progressionAnimationTarget.laurels && canonical.gems === progressionAnimationTarget.gems;
            if (changed && !sameTarget && !holdBundleGems) {
                if (reducedRewardMotion() || root.hidden || document.visibilityState !== "visible") writeProgression(canonical);
                else animateProgressionTo(canonical, 900, ++rewardAnimationToken);
            }
        }
        const panel = activeScreenPanel() || root;
        const settings = state.settings || {};
        const musicInput = root.querySelector("[data-setting='music_volume']");
        const motionInput = root.querySelector("[data-setting='reduced_motion']");
        const freeZoomInput = root.querySelector("[data-setting='free_zoom_out']");
        const fpsPingInput = root.querySelector("[data-setting='show_fps_ping']");
        const devToolsInput = root.querySelector("[data-setting='show_dev_tools']");
        if (musicInput && document.activeElement !== musicInput) musicInput.value = settings.music_volume == null ? 0.8 : settings.music_volume;
        if (motionInput && document.activeElement !== motionInput) motionInput.value = settings.reduced_motion ? "reduced" : "full";
        if (fpsPingInput && document.activeElement !== fpsPingInput) fpsPingInput.checked = settings.show_fps_ping === true;
        if (devToolsInput && document.activeElement !== devToolsInput) devToolsInput.checked = Boolean(settings.show_dev_tools);
        if (freeZoomInput && document.activeElement !== freeZoomInput) freeZoomInput.checked = Boolean(settings.free_zoom_out);
        const musicValBadge = root.querySelector("[data-val-for='music_vol']");
        if (musicValBadge && musicInput) musicValBadge.textContent = Math.round(Number(musicInput.value) * 100) + "%";
        const timer = panel.querySelector("[data-live-countdown]");
        const lobby = joinedLobby();
        if (timer && lobby) timer.textContent = state.match_preparing ? SOW_t("menu.loading") : lobby.is_counting_down ? SOW_t("lobbies.starting_in", { seconds: Math.ceil(lobby.timer_secs) }) : SOW_t("lobbies.waiting_for_players");
        updateQueueRoster(lobby);
        const cardStatuses = panel.querySelectorAll("[data-lobby-status-for]");
        for (let i = 0; i < cardStatuses.length; i++) {
            const status = cardStatuses[i];
            const cardLobby = findLobby(Number(status.dataset.lobbyStatusFor));
            status.textContent = cardLobby ? lobbyStatusText(cardLobby) : "";
            const card = status.closest("[data-lobby-card]");
            const count = card && card.querySelector("[data-lobby-count]");
            if (count) count.textContent = cardLobby ? lobbyPlayerCount(cardLobby) : "";
            if (cardLobby && card) card.setAttribute("aria-label", lobbyLabel(cardLobby));
        }
        maybePresentRewards();
    }

    root.addEventListener("click", function (event) {
        if (state && state.phase === "MainMenu") syncMenuMusic(state, true);
        const purchaseOverlay = event.target.closest("[data-menu-overlay='purchase']");
        if (purchaseOverlay && event.target === purchaseOverlay) {
            if (typeof canDismissPurchaseModal === "function" && !canDismissPurchaseModal()) return;
            if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
            else {
                purchaseModal = null;
                purchaseIntent = null;
            }
            render();
            return;
        }
        const skinPickerOverlay = event.target.closest("[data-menu-overlay='skin-picker']");
        if (skinPickerOverlay && event.target === skinPickerOverlay) {
            skinPickerOpen = false;
            render();
            return;
        }
        const heroInfoOverlay = event.target.closest("[data-menu-overlay='hero-info']");
        if (heroInfoOverlay && event.target === heroInfoOverlay) {
            heroInfoOpen = false;
            render();
            return;
        }
        const settingsOverlay = event.target.closest("[data-menu-overlay='settings']");
        if (settingsOverlay && event.target === settingsOverlay) {
            settingsOpen = false;
            signOutConfirmOpen = false;
            dropdownOpenKey = null;
            render();
            return;
        }
        const target = event.target.closest("[data-command]");
        if (!target || !root.contains(target)) return;
        const command = target.dataset.command;
        const experienceEvent = {
            open_campaign: "menu_campaign_open",
            open_browser: "menu_lobby_browser_open",
            open_create: "menu_custom_create_open",
            confirm_leader: "menu_leader_confirm"
        }[command];
        if (experienceEvent && typeof window.SOW_trackExperienceEvent === "function") {
            window.SOW_trackExperienceEvent({ name: experienceEvent });
        }
        if (command === "open_campaign") {
            profileOpen = false;
            profileAccountId = null;
            profileMatchDetail = null;
            heroesOpen = false;
            storeOpen = false;
            campaignOpen = true;
            render();
            return;
        }
        if (command === "close_campaign") {
            campaignOpen = false;
            render();
            return;
        }
        if (command === "watch_campaign_cinematic") {
            const cinematicEpisodeId = target.dataset.episodeId;
            if (cinematicEpisodeId && typeof window.SOW_watchCampaignCinematic === "function") {
                window.SOW_watchCampaignCinematic(cinematicEpisodeId);
            }
            return;
        }
        if (command === "start_campaign_episode") {
            const episodeId = target.dataset.episodeId;
            if (!episodeId) return;
            campaignOpen = false;
            if (typeof window.SOW_startCampaignEpisode === "function") {
                window.SOW_startCampaignEpisode(episodeId);
            }
            render();
            return;
        }
        /* POKI_SHARED_STORE_ACTIONS_BEGIN */
        if (command === "buy_product") {
            beginStorePurchase(target.dataset.productId);
            return;
        }
        if (command === "restore_purchases") {
            beginStoreRestore();
            return;
        }
        /* POKI_SHARED_STORE_ACTIONS_END */
        if (command === "main_nav") {
            const navScreen = target.dataset.navScreen || "battle";
            storeCheckoutRequestId = null;
            storeCheckoutBusy = false;
            purchaseModal = null;
            purchaseIntent = null;
            settingsOpen = false;
            signOutConfirmOpen = false;
            campaignOpen = false;
            passwordLobbyId = null;
            passwordDraft = "";
            tempSelectedLeader = null;
            tempSelectedRandom = false;
            if (navScreen === "heroes") {
                profileOpen = false;
                profileAccountId = null;
                profileMatchDetail = null;
                profileDetailLoading = false;
                profileDetailRequestKey = null;
                storeOpen = false;
                heroesOpen = true;
                heroesSearchQuery = "";
                heroesRegionFilter = "all";
                dropdownOpenKey = null;
                const navLeaderSelection = currentLeaderSelection();
                tempSelectedLeader = navLeaderSelection.selected_leader;
                tempSelectedRandom = navLeaderSelection.prefer_random_leader;
                render();
                return;
            }
            if (navScreen === "profile") {
                storeOpen = false;
                if (!profileOpen || profileAccountId !== (state && state.account_id)) {
                    openProfile(null);
                } else {
                    render();
                }
                return;
            }
            if (navScreen === "store") {
                profileOpen = false;
                heroesOpen = false;
                storeOpen = true;
                render();
                return;
            }
            profileOpen = false;
            profileAccountId = null;
            profileMatchDetail = null;
            profileDetailLoading = false;
            profileDetailRequestKey = null;
            heroesOpen = false;
            storeOpen = false;
            if (state.show_browser || state.show_create) {
                send("close_overlay");
            } else {
                render();
            }
            return;
        }
        if (command === "open_profile") {
            storeOpen = false;
            heroesOpen = false;
            campaignOpen = false;
            openProfile(target.dataset.accountId || null);
            return;
        }
        if (command === "close_profile") {
            profileOpen = false;
            storeOpen = false;
            heroesOpen = false;
            profileAccountId = null;
            profileData = null;
            profileMatchDetail = null;
            profileDetailLoading = false;
            profileDetailRequestKey = null;
            render();
            return;
        }
        if (command === "profile_tab") {
            profileTab = target.dataset.profileTab || "overview";
            render();
            if (profileTab === "history") {
                const historyEntry = activeProfileEntry();
                if (historyEntry && (historyEntry.historyHasMore || historyEntry.historyError || !historyEntry.historyLoaded || historyEntry.stale)) loadMoreProfileHistory();
            } else if (profileTab === "victories") {
                loadVictoryLeaderboard();
            }
            return;
        }
        if (command === "open_play_games") {
            if (typeof window.SOW_openAndroidPlayGames === "function") {
                window.SOW_openAndroidPlayGames(target.dataset.playGamesSection || "");
            }
            return;
        }
        if (command === "open_android_oss_licenses") {
            event.preventDefault();
            if (typeof window.SOW_openAndroidOssLicenses === "function") {
                window.SOW_openAndroidOssLicenses();
            }
            return;
        }
        if (command === "open_android_external_link") {
            event.preventDefault();
            const url = target.dataset.url || "";
            const opened = typeof window.SOW_openAndroidExternalLink === "function" && window.SOW_openAndroidExternalLink(url);
            if (!opened && url) window.open(url, "_blank", "noopener,noreferrer");
            return;
        }
        if (command === "load_profile_more") {
            loadMoreProfileHistory();
            return;
        }
        if (command === "load_victory_more") {
            loadVictoryLeaderboard(true);
            return;
        }
        if (command === "retry_profile") {
            invalidateProfileCache(profileAccountId);
            profileSnapshotLoading = false;
            profileHistoryLoading = false;
            loadProfile(profileAccountId);
            syncProfileDom();
            return;
        }
        if (command === "open_match") {
            const matchId = target.dataset.matchId;
            const detailEntry = activeProfileEntry();
            if (!matchId || !detailEntry || profileDetailLoading) return;
            if (detailEntry.details[matchId]) {
                profileDetailId = matchId;
                profileDetailError = "";
                profileDetailRequestKey = null;
                profileMatchDetail = detailEntry.details[matchId];
                render();
                return;
            }
            const detailProfileId = profileAccountId;
            const detailRequestKey = detailProfileId + ":" + matchId;
            if (detailEntry.detailRequests[matchId]) {
                profileDetailId = matchId;
                profileDetailError = "";
                profileDetailLoading = true;
                profileDetailRequestKey = detailRequestKey;
                return;
            }
            profileDetailId = matchId;
            profileDetailError = "";
            profileMatchDetail = null;
            profileDetailLoading = true;
            profileDetailRequestKey = detailRequestKey;
            detailEntry.detailRequests[matchId] = fetch(profileApi("/matches/" + encodeURIComponent(matchId)), {
                headers: { "Accept": "application/json" }
            }).then(function (response) {
                if (!response.ok) throw new Error("match detail failed");
                return response.json();
            }).then(function (detail) {
                detailEntry.details[matchId] = detail;
                if (profileOpen && profileAccountId === detailProfileId) {
                    profileDetailError = "";
                    profileMatchDetail = detail;
                }
            }).catch(function () {
                if (profileOpen && profileAccountId === detailProfileId) profileDetailError = SOW_t("profile.match_details_unavailable");
            }).finally(function () {
                delete detailEntry.detailRequests[matchId];
                if (profileDetailRequestKey === detailRequestKey) {
                    profileDetailLoading = false;
                    profileDetailRequestKey = null;
                }
                if (profileOpen && profileAccountId === detailProfileId && profileDetailId === matchId) render();
            });
            return;
        }
        if (command === "close_match") {
            profileMatchDetail = null;
            profileDetailError = "";
            profileDetailId = null;
            profileDetailLoading = false;
            profileDetailRequestKey = null;
            render();
            return;
        }
        if (command === "open_report") {
            reportOpen = true;
            reportTarget = target.dataset.accountId || profileAccountId;
            reportSent = false;
            render();
            return;
        }
        if (command === "cancel_report") {
            reportOpen = false;
            reportTarget = null;
            render();
            return;
        }
        if (command === "request_delete") {
            if (!settingsOpen || !selfCreds() || deleteBusy) return;
            deleteArmed = true;
            deleteError = "";
            render();
            return;
        }
        if (command === "cancel_delete") {
            deleteArmed = false;
            deleteError = "";
            render();
            return;
        }
        if (command === "confirm_delete") {
            const delCreds = selfCreds();
            if (!settingsOpen || !deleteArmed || !delCreds || deleteBusy) {
                deleteArmed = false;
                render();
                return;
            }
            deleteBusy = true;
            deleteError = "";
            render();
            fetch(profileApi("/profile/anonymous/delete"), {
                method: "POST",
                headers: { "Content-Type": "application/json", "Accept": "application/json" },
                body: JSON.stringify({ account_id: delCreds.account_id, auth_secret: delCreds.auth_secret })
            }).then(function (response) {
                return response.json().catch(function () { return {}; }).then(function (data) {
                    if (!response.ok || data.deleted !== true) throw new Error("delete failed");
                });
            }).then(function () {
                try {
                    ["sow_account_id", "sow_account_secret", "sow_player_progress", "poki_ignore_sow_pending_display_name"].forEach(function (k) {
                        window.localStorage.removeItem(k);
                    });
                } catch {}
                window.location.reload();
            }).catch(function () {
                deleteBusy = false;
                deleteArmed = false;
                deleteError = SOW_t("menu.account_deletion_unavailable");
                render();
            });
            return;
        }
        if (command === "open_leader_picker") {
            profileOpen = false;
            profileAccountId = null;
            profileMatchDetail = null;
            storeOpen = false;
            campaignOpen = false;
            heroesOpen = true;
            heroesSearchQuery = "";
            heroesRegionFilter = "all";
            dropdownOpenKey = null;
            const pickerLeaderSelection = currentLeaderSelection();
            tempSelectedLeader = pickerLeaderSelection.selected_leader;
            tempSelectedRandom = pickerLeaderSelection.prefer_random_leader;
            settingsOpen = false;
            signOutConfirmOpen = false;
            render();
            return;
        }
        if (command === "toggle_dropdown") {
            const dropdownKey = target.dataset.dropdownKey;
            setDropdownOpen(dropdownKey, dropdownOpenKey !== dropdownKey, target);
            return;
        }
        if (command === "select_dropdown") {
            selectDropdown(target.dataset.dropdownKey, target.dataset.dropdownOptionValue);
            return;
        }
        if (command === "open_hero_info") {
            heroInfoOpen = true;
            render();
            return;
        }
        if (command === "close_hero_info") {
            heroInfoOpen = false;
            render();
            return;
        }
        if (command === "preview_leader") {
            tempSelectedLeader = target.dataset.leaderId;
            tempSelectedRandom = false;
            if (!updateHeroesPreview()) render();
            maybePresentRewards();
            return;
        }
        if (command === "preview_random_leader") {
            tempSelectedLeader = null;
            tempSelectedRandom = true;
            if (!updateHeroesPreview()) render();
            return;
        }
        if (command === "confirm_random_leader") {
            stageLeaderSelection(true, null);
            send("set_random_leader");
            heroesOpen = false;
            heroesSearchQuery = "";
            heroesRegionFilter = "all";
            tempSelectedLeader = null;
            tempSelectedRandom = false;
            render();
            return;
        }
        if (command === "confirm_leader") {
            const leaderId = target.dataset.leaderId || tempSelectedLeader;
            const leader = leaderById(leaderId);
            if (leaderId && leader.available !== false) {
                stageLeaderSelection(false, leaderId);
                send("set_leader", { leader_id: leaderId });
                heroesOpen = false;
                heroesSearchQuery = "";
                heroesRegionFilter = "all";
                tempSelectedLeader = null;
                tempSelectedRandom = false;
                render();
            }
            return;
        }
        /* POKI_SHARED_STORE_UNLOCK_BEGIN */
        if (command === "unlock_leader") {
            const unlockLeaderId = target.dataset.leaderId;
            const unlockCurrency = target.dataset.currency || "crowns";
            if (unlockLeaderId) openLeaderPurchase(unlockLeaderId, unlockCurrency);
            return;
        }
        if (command === "open_skin_purchase") {
            const purchaseSkinId = target.dataset.skinId;
            if (purchaseSkinId) openSkinPurchase(purchaseSkinId);
            return;
        }
        if (command === "open_skin_picker") {
            skinPickerOpen = true;
            loadStoreCatalog();
            render();
            return;
        }
        if (command === "close_skin_picker") {
            skinPickerOpen = false;
            render();
            return;
        }
        if (command === "confirm_purchase") {
            if (!purchaseModal || purchaseModal.submitted) return;
            purchaseModal.submitted = true;
            if (purchaseModal.type === "leader") {
                if (!(state && state.store_busy)) send("unlock_leader", { leader_id: purchaseModal.leaderId, currency: purchaseModal.currency });
            } else if (purchaseModal.type === "skin") {
                if (!(state && state.store_busy)) send("unlock_skin", { skin_id: purchaseModal.skinId });
            }
            render();
            return;
        }
        if (command === "cancel_purchase") {
            if (typeof canDismissPurchaseModal === "function" && !canDismissPurchaseModal()) return;
            if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
            else {
                purchaseModal = null;
                purchaseIntent = null;
            }
            render();
            return;
        }
        if (command === "unlock_skin" || command === "equip_skin") {
            const skinId = target.dataset.skinId;
            if (skinId && !(state && state.store_busy)) {
                send(command, { skin_id: skinId });
            }
            return;
        }
        /* POKI_SHARED_STORE_UNLOCK_END */
        if (command === "set_session_mode") {
            createOffline = target.dataset.mode === "offline";
            render();
            return;
        }
        if (command === "set_create_mode") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.game_mode = target.dataset.mode;
            render();
            return;
        }
        if (command === "set_create_diff") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.bot_difficulty = target.dataset.diff;
            render();
            return;
        }
        if (command === "set_create_spawn") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.random_spawn = target.dataset.spawn === "true";
            render();
            return;
        }
        if (command === "set_create_private") {
            createPrivate = target.dataset.private === "true";
            render();
            return;
        }
        if (command === "randomize_seed") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.seed = Math.floor(Math.random() * 9999) + 1;
            render();
            return;
        }
        if (command === "toggle_settings") {
            settingsOpen = !settingsOpen;
            if (!settingsOpen) {
                signOutConfirmOpen = false;
                deleteArmed = false;
                deleteError = "";
            }
            authModalOpen = false;
            render();
            return;
        }
        /* POKI_SHARED_AUTH_ACTIONS_BEGIN */
        if (command === "close_auth") {
            authModalOpen = false;
            render();
            return;
        }
        if (command === "auth_change_email") {
            authOtpSent = false;
            authCode = "";
            authError = "";
            authNotice = "";
            render();
            return;
        }
        if (command === "auth_resend") {
            requestAuthOtp();
            return;
        }
        if (command === "wou_provider") {
            if (typeof window.SOW_startWouOAuth === "function") window.SOW_startWouOAuth(target.dataset.provider);
            return;
        }
        if (command === "sign_in") {
            if (isAndroidTwa()) {
                send(command);
            } else {
                settingsOpen = false;
                signOutConfirmOpen = false;
                resetAuthFlow();
                authModalOpen = true;
                render();
            }
            return;
        }
        if (command === "sign_out") {
            signOutConfirmOpen = true;
            render();
            return;
        }
        if (command === "cancel_sign_out") {
            signOutConfirmOpen = false;
            render();
            return;
        }
        if (command === "confirm_sign_out") {
            signOutConfirmOpen = false;
            if (isAndroidTwa()) send("sign_out");
            else if (typeof window.SOW_signOutWou === "function") window.SOW_signOutWou();
            return;
        }
        /* POKI_SHARED_AUTH_ACTIONS_END */
        if (command === "close_password") {
            passwordLobbyId = null;
            passwordDraft = "";
            render();
            return;
        }
        if (command === "copy_lobby_code") {
            const lobbyCode = String(target.dataset.lobbyId || "");
            if (navigator.clipboard && lobbyCode) {
                navigator.clipboard.writeText(lobbyCode).then(function () {
                    target.textContent = SOW_t("menu.copied");
                }).catch(function () {
                    target.textContent = SOW_t("menu.copy_failed");
                });
            } else if (lobbyCode) {
                target.textContent = SOW_t("menu.copy_unavailable");
            }
            return;
        }
        if (command === "ban_player" && !window.confirm(SOW_t("auth.ban_player_confirm"))) {
            return;
        }
        if (command === "join_lobby") {
            const selectedLobby = findLobby(Number(target.dataset.lobbyId));
            if (selectedLobby && selectedLobby.has_password) {
                passwordLobbyId = selectedLobby.id;
                passwordDraft = "";
                render();
            } else {
                send("join_lobby", { lobby_id: Number(target.dataset.lobbyId) });
            }
            return;
        }
        if (command === "set_leader") {
            if (send("set_leader", { leader_id: target.dataset.leaderId })) {
                heroesOpen = false;
            }
            return;
        }
        const payload: { lobby_id?: number; target_player_id?: number } = {};
        if (target.dataset.lobbyId) payload.lobby_id = Number(target.dataset.lobbyId);
        if (target.dataset.playerId) payload.target_player_id = Number(target.dataset.playerId);
        send(command, payload);
    });

    document.addEventListener("pointerdown", function (event) {
        const target = event.target;
        const interactive = target && target.closest ? target.closest("[data-command], button, input, textarea, a, [role='button']") : null;
        if (interactive && root.contains(interactive)) activeMenuPointerId = event.pointerId;
        const dropdown = target && target.closest ? target.closest("[data-control-dropdown]") : null;
        if (dropdownOpenKey && (!dropdown || dropdown.dataset.dropdownKey !== dropdownOpenKey)) {
            setDropdownOpen(dropdownOpenKey, false);
        }
    });

    function finishMenuPointer(event) {
        if (activeMenuPointerId !== event.pointerId || renderFlushTimer !== null) return;
        renderFlushTimer = setTimeout(function () {
            renderFlushTimer = null;
            activeMenuPointerId = null;
            if (renderPending) render();
        }, 0);
    }

    document.addEventListener("pointerup", finishMenuPointer);
    document.addEventListener("pointercancel", finishMenuPointer);

    /* POKI_SHARED_AUTH_EVENTS_BEGIN */
    window.addEventListener("wou:auth-state-change", function () {
        if (!state || root.hidden) return;
        const auth = typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() || {} : {};
        if (auth.linked && authModalOpen) {
            authModalOpen = false;
            resetAuthFlow();
        }
        if (!root.querySelector("[data-screen-stage]")) {
            render();
            return;
        }
        updateFrameChrome(currentScreen());
        updateDynamic();
        if (settingsOpen) syncOverlay("settings", renderSettings());
        if (authModalOpen) syncOverlay("auth", renderAuthModal());
        syncProfileDom();
    });

    window.addEventListener("wou:auth-error", function (event) {
        authBusy = false;
        authModalOpen = true;
        const detail = event.detail as { message?: unknown } | null;
        authError = detail && typeof detail.message === "string" ? detail.message : SOW_t("auth.sign_in_unavailable");
        render();
    });
    /* POKI_SHARED_AUTH_EVENTS_END */

    /* POKI_SHARED_STORE_EVENTS_BEGIN */
    window.addEventListener("sow:android-purchase-bridge-ready", function () {
        if (state && !root.hidden) render();
    });

    window.addEventListener("sow:android-purchase-result", function (event) {
        const result = event.detail as { request_id?: string; status?: string; product_id?: string };
        if (result.request_id && storeCheckoutRequestId &&
            result.request_id !== storeCheckoutRequestId) return;
        storeCheckoutBusy = false;
        if (!state) return;
        storeCheckoutRequestId = null;
        if (result.status === "success") {
            state.error = null;
            const attempt = typeof currentExternalPurchaseAttempt === "function" ? currentExternalPurchaseAttempt() : null;
            if (attempt && (!result.product_id || attempt.product_id === result.product_id)) {
                completeExternalPurchase(attempt.product_id);
            } else {
                purchaseIntent = null;
                send("refresh_profile");
            }
        } else if (result.status === "restored") {
            purchaseIntent = null;
            send("refresh_profile");
        } else if (result.status === "cancelled") {
            if (typeof abandonExternalPurchaseAttempt === "function") abandonExternalPurchaseAttempt(result.product_id);
            if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
            else {
                purchaseModal = null;
                purchaseIntent = null;
            }
            state.error = SOW_t("menu.purchase_cancelled");
        } else {
            if (typeof abandonExternalPurchaseAttempt === "function") abandonExternalPurchaseAttempt(result.product_id);
            if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
            else {
                purchaseModal = null;
                purchaseIntent = null;
            }
            state.error = SOW_t("menu.purchase_failed");
        }
        render();
    });
    /* POKI_SHARED_STORE_EVENTS_END */

    root.addEventListener("keydown", function (event) {
        const profileTab = event.target.closest("[role='tab'][data-command='profile_tab']");
        if (profileTab && ["ArrowLeft", "ArrowRight", "Home", "End"].indexOf(event.key) >= 0) {
            const profileTabs = Array.prototype.slice.call(root.querySelectorAll("[role='tab'][data-command='profile_tab']"));
            const profileTabIndex = profileTabs.indexOf(profileTab);
            if (profileTabs.length && profileTabIndex >= 0) {
                const nextProfileTabIndex = event.key === "Home" ? 0 : event.key === "End" ? profileTabs.length - 1 :
                    (profileTabIndex + (event.key === "ArrowRight" ? 1 : -1) + profileTabs.length) % profileTabs.length;
                event.preventDefault();
                profileTabs[nextProfileTabIndex].click();
                const selectedProfileTab = root.querySelector("[role='tab'][aria-selected='true']");
                if (selectedProfileTab) selectedProfileTab.focus();
                return;
            }
        }
        const dropdownTrigger = event.target.closest("[data-role='dropdown-trigger']");
        const dropdownOption = event.target.closest("[data-role='dropdown-option'], [data-command='select_dropdown']");
        if (dropdownTrigger) {
            if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                const triggerKey = dropdownTrigger.dataset.dropdownKey;
                setDropdownOpen(triggerKey, true);
                const triggerOptions = dropdownTrigger.parentElement.querySelectorAll("[data-command='select_dropdown']");
                if (triggerOptions.length) triggerOptions[event.key === "ArrowUp" ? triggerOptions.length - 1 : 0].focus();
                return;
            }
        }
        if (dropdownOption) {
            const dropdown = dropdownOption.closest("[data-control-dropdown]");
            const options = dropdown ? Array.prototype.slice.call(dropdown.querySelectorAll("[data-command='select_dropdown']")) : [];
            const optionIndex = options.indexOf(dropdownOption);
            let nextIndex = optionIndex;
            if (event.key === "ArrowDown") nextIndex = Math.min(options.length - 1, optionIndex + 1);
            if (event.key === "ArrowUp") nextIndex = Math.max(0, optionIndex - 1);
            if (event.key === "Home") nextIndex = 0;
            if (event.key === "End") nextIndex = options.length - 1;
            if (nextIndex !== optionIndex) {
                event.preventDefault();
                options[nextIndex].focus();
                return;
            }
            if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                dropdownOption.click();
                return;
            }
        }
        if (event.key === "Escape" && dropdownOpenKey) {
            event.preventDefault();
            const openKey = dropdownOpenKey;
            setDropdownOpen(openKey, false, root.querySelector("[data-control-dropdown][data-dropdown-key='" + openKey + "'] [data-role='dropdown-trigger']"));
            return;
        }
        if (event.key === "Escape" && settingsOpen) {
            event.preventDefault();
            if (signOutConfirmOpen) {
                signOutConfirmOpen = false;
                render();
                return;
            }
            settingsOpen = false;
            signOutConfirmOpen = false;
            deleteArmed = false;
            deleteError = "";
            dropdownOpenKey = null;
            render();
            return;
        }
        if (event.key === "Escape" && purchaseModal) {
            event.preventDefault();
            if (typeof canDismissPurchaseModal === "function" && !canDismissPurchaseModal()) return;
            if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
            else {
                purchaseModal = null;
                purchaseIntent = null;
            }
            render();
            return;
        }
        if (event.key === "Escape" && (skinPickerOpen || heroInfoOpen)) {
            event.preventDefault();
            skinPickerOpen = false;
            heroInfoOpen = false;
            render();
            return;
        }
        if (event.key === "Escape" && profileMatchDetail) {
            profileMatchDetail = null;
            render();
            return;
        }
        if (event.key === "Enter") {
            const input = event.target;
            if (input && input.dataset && input.dataset.role === "display-name") {
                event.preventDefault();
                input.blur();
                return;
            }
        }
        if (event.key !== "Enter" && event.key !== " ") return;
        const target = event.target.closest("[data-command='join_lobby']");
        if (!target || !root.contains(target)) return;
        event.preventDefault();
        target.click();
    });

    root.addEventListener("submit", function (event) {
        const form = event.target as HTMLFormElement;
        /* POKI_SHARED_AUTH_SUBMIT_BEGIN */
        if (form.dataset.authForm === "request") {
            event.preventDefault();
            requestAuthOtp();
            return;
        }
        if (form.dataset.authForm === "verify") {
            event.preventDefault();
            verifyAuthOtp();
            return;
        }
        /* POKI_SHARED_AUTH_SUBMIT_END */
        if (form.dataset.form === "report") {
            event.preventDefault();
            if (reportBusy) return;
            const repCreds = selfCreds();
            const reasonField = form.elements.namedItem("reason") as RadioNodeList | HTMLInputElement | HTMLSelectElement | null;
            const detailsField = form.elements.namedItem("details") as HTMLInputElement | HTMLTextAreaElement | null;
            const reason = reasonField ? reasonField.value : "";
            const details = detailsField ? detailsField.value.trim() : "";
            if (!repCreds || !reportTarget) {
                reportOpen = false;
                render();
                return;
            }
            if (reason === "other" && !details) {
                render();
                return;
            }
            reportBusy = true;
            render();
            fetch(profileApi("/profile/anonymous/report"), {
                method: "POST",
                headers: { "Content-Type": "application/json", "Accept": "application/json" },
                body: JSON.stringify({
                    account_id: repCreds.account_id,
                    auth_secret: repCreds.auth_secret,
                    reported_account_id: reportTarget,
                    reason: reason,
                    details: details || null
                })
            }).then(function (response) {
                if (!response.ok) throw new Error("report failed");
                return response.json();
            }).then(function () {
                reportSent = true;
                reportOpen = false;
                try {
                    const list = window.SOW_BLOCKED_IDS || [];
                    if (list.indexOf(reportTarget) === -1) list.push(reportTarget);
                    window.SOW_BLOCKED_IDS = list;
                } catch {}
            }).catch(function () {
            }).finally(function () {
                reportBusy = false;
                render();
            });
            return;
        }
        if (form.dataset.form === "join") {
            event.preventDefault();
            const code = (form.elements.namedItem("code") as HTMLInputElement).value.trim();
            send("join_code", { code: code });
        }
        if (form.dataset.form === "password") {
            event.preventDefault();
            passwordDraft = (form.elements.namedItem("password") as HTMLInputElement).value;
            if (passwordLobbyId != null && passwordDraft) {
                send("join_with_password", { lobby_id: passwordLobbyId, password: passwordDraft });
            }
        }
        if (form.dataset.form === "create") {
            event.preventDefault();
            syncCreateDraft(form);
            const config = createDraft || cloneConfig();
            if (createOffline) {
                send("start_single_player", { config: config });
            } else {
                send("create_game", { config: config, is_private: createPrivate, password: createPassword || null });
            }
        }
    });

    root.addEventListener("input", function (event) {
        const input = event.target;
        if (input.dataset && input.dataset.authField === "email") authEmail = input.value;
        if (input.dataset && input.dataset.authField === "code") authCode = input.value.replace(/\D/g, "").slice(0, 6);
        if (input.dataset && input.dataset.role === "browser-search") {
            browserSearchQuery = input.value;
            const publicPanel = root.querySelector(".sow-menu__public");
            if (publicPanel) {
                updateLobbyViews();
            }
        }
        if (input.dataset && input.dataset.role === "heroes-search") {
            heroesSearchQuery = input.value;
            refreshHeroesRoster(true);
        }
        const createForm = input.closest("form[data-form='create']");
        if (createForm) {
            syncCreateDraft(createForm);
            const valBadge = createForm.querySelector("[data-val-for='" + input.name + "']");
            if (valBadge) valBadge.textContent = input.value;
        }
        if (input.dataset && input.dataset.setting === "music_volume") {
            const musicValBadge = root.querySelector("[data-val-for='music_vol']");
            if (musicValBadge) musicValBadge.textContent = Math.round(Number(input.value) * 100) + "%";
            if (state && state.phase === "MainMenu") syncMenuMusic(state, true, Number(input.value));
        }
        if (input.name === "password" && input.closest("form[data-form='password']")) {
            passwordDraft = input.value;
        }
    });

    root.addEventListener("change", function (event) {
        const input = event.target;
        if (input.dataset && input.dataset.role === "display-name") {
            const name = input.value.trim();
            if (name && name !== state.player_name) send("save_display_name", { name: name });
            return;
        }
        const createForm = input.closest("form[data-form='create']");
        if (createForm) {
            syncCreateDraft(createForm);
            render();
            return;
        }
        if (input.dataset.setting === "locale") {
            if (typeof window.SOW_setLocale === "function") window.SOW_setLocale(input.value);
            return;
        }
        if (!input.dataset.setting) return;
        if (input.dataset.setting === "music_volume") send("set_music_volume", { value: Number(input.value) });
        if (input.dataset.setting === "reduced_motion") send("set_reduced_motion", { value: input.value === "reduced" });
        if (input.dataset.setting === "free_zoom_out") send("set_free_zoom_out", { value: input.checked });
        if (input.dataset.setting === "show_fps_ping") send("set_show_fps_ping", { value: input.checked });
        if (input.dataset.setting === "show_dev_tools") send("set_show_dev_tools", { value: input.checked });
    });

    function normalizeCompletedExitState(nextState) {
        if (nextState.phase === "MainMenu" && nextState.loader_job === "ExitGame" &&
            Number.isSafeInteger(nextState.loader_cycle_id) && nextState.loader_cycle_id <= completedLoaderCycleId) {
            return Object.assign({}, nextState, { loader_job: "Boot", loader_leader: null });
        }
        return nextState;
    }

    function syncWebLoaderForState(nextState) {
        if (typeof window.SOW_syncWebLoader === "function") window.SOW_syncWebLoader(nextState);
    }

    function scheduleRewardPresentation(delay) {
        if (!state || state.phase !== "MainMenu" || pendingExitScreenIntro || rewardPresentationReady || rewardAnimationTimer !== null) return;
        rewardAnimationTimer = window.setTimeout(function () {
            rewardAnimationTimer = null;
            if (!state || state.phase !== "MainMenu") return;
            rewardPresentationReady = true;
            maybePresentRewards();
        }, delay);
    }

    window.addEventListener("sow:loader-cycle-ready", function (event) {
        const detail = event && event.detail as { cycle_id?: number } | null;
        const cycleId = detail && detail.cycle_id;
        if (!Number.isSafeInteger(cycleId) || cycleId < latestLoaderCycleId || cycleId < completedLoaderCycleId) return;
        completedLoaderCycleId = cycleId;
        menuMusicFinishLoaderCycle(cycleId);
        if (!state || state.phase !== "MainMenu" || state.loader_job !== "ExitGame" || state.loader_cycle_id !== cycleId) return;

        const replayExitIntro = pendingExitScreenIntro;
        state = normalizeCompletedExitState(state);
        reconcileLeaderSelection(state);
        syncMenuMusic(state, false);
        if (replayExitIntro) {
            pendingExitScreenIntro = false;
            previousScreen = null;
        }
        render();
        if (replayExitIntro) scheduleRewardPresentation(250);
    });

    document.addEventListener("visibilitychange", function () {
        syncHeroesRotationCountdown();
        if (document.visibilityState !== "visible") {
            if (window.SOW_menu_guide) window.SOW_menu_guide.pause();
            if (state && (rewardAnimationRunning || progressionAnimationTarget)) {
                cancelRewardAnimation();
                writeProgression(progressionFromState());
                const unpresentedReceipt = pendingRewardReceipts().some(function (receipt) { return !rewardAnimationShown.has(receipt.id); });
                if (unpresentedReceipt) displayedProgression = null;
            }
            return;
        }
        if (!state || state.phase !== "MainMenu") return;
        if (typeof resumeExternalPurchaseDelivery === "function") resumeExternalPurchaseDelivery(true);
        if (!rewardPresentationReady) scheduleRewardPresentation(0);
        else maybePresentRewards();
    });

    function handleMenuStateUpdate(raw) {
        if (typeof raw !== "string" || raw === lastRaw) {
            updateDynamic();
            return;
        }
        lastRaw = raw;
        try {
            state = JSON.parse(raw);
        } catch (error) {
            console.warn("[WEB MENU] invalid state:", error);
            return;
        }
        if (!Number.isSafeInteger(state.loader_cycle_id) || state.loader_cycle_id < latestLoaderCycleId) return;
        latestLoaderCycleId = state.loader_cycle_id;
        state = normalizeCompletedExitState(state);
        syncMenuMusic(state, false);
        const accountId = state.account_id || "";
        if (rewardAnimationAccount !== accountId) {
            rewardAnimationAccount = accountId;
            rewardAnimationShown.clear();
            rewardAckTimes = Object.create(null);
            if (rewardAckRetryTimer !== null) window.clearTimeout(rewardAckRetryTimer);
            rewardAckRetryTimer = null;
            displayedProgression = null;
            rewardPresentationReady = false;
            cancelRewardAnimation();
            if (rewardAnimationTimer !== null) window.clearTimeout(rewardAnimationTimer);
            rewardAnimationTimer = null;
        }
        if (typeof resumeExternalPurchaseDelivery === "function") resumeExternalPurchaseDelivery(false);
        if (typeof resolvePurchaseModal === "function") resolvePurchaseModal();
        const returnedFromGame = lastMenuPhase !== null &&
            lastMenuPhase !== "MainMenu" &&
            state.phase === "MainMenu" &&
            state.loader_job === "ExitGame";
        lastMenuPhase = state.phase;
        if (state.phase !== "MainMenu") {
            rewardPresentationReady = false;
            if (rewardAnimationRunning) {
                cancelRewardAnimation();
                writeProgression(progressionFromState());
            }
            if (rewardAnimationTimer !== null) window.clearTimeout(rewardAnimationTimer);
            rewardAnimationTimer = null;
            maybePresentRewards();
        }
        if (returnedFromGame) {
            profileOpen = false;
            heroesOpen = false;
            campaignOpen = false;
            storeOpen = false;
            pendingExitScreenIntro = true;
        }
        if (typeof window.SOW_tutorial_menu_state_update === "function") {
            window.SOW_tutorial_menu_state_update(state);
        }
        syncProfilePreload(state);
        if (state.waiting && passwordLobbyId != null) {
            passwordLobbyId = null;
            passwordDraft = "";
        }
        const key = renderKey();
        if (key !== lastRenderKey) {
            render();
        } else {
            root.hidden = state.phase !== "MainMenu";
            updateLobbyViews();
            updateDynamic();
        }
        syncWebLoaderForState(state);
        if (state.phase === "MainMenu") {
            scheduleRewardPresentation(250);
            maybePresentRewards();
        }
    }

    window.addEventListener("sow:locale-change", function () {
        if (state && !root.hidden) render();
    });

    window.SOW_menu_state_update = handleMenuStateUpdate;
    root.hidden = true;
})();
