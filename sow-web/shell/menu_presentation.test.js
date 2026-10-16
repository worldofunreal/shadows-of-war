const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const shell = __dirname;
const coreSource = fs.readFileSync(path.join(shell, "main_menu.core.js"), "utf8");
const shellSource = fs.readFileSync(path.join(shell, "main_menu.shell.js"), "utf8");
const heroesSource = fs.readFileSync(path.join(shell, "main_menu.heroes.js"), "utf8");
const lobbiesSource = fs.readFileSync(path.join(shell, "main_menu.lobbies.js"), "utf8");
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const menuCssFiles = [
    "main_menu.base.css",
    "main_menu.layout.css",
    "main_menu.lobbies.css",
    "main_menu.create.css",
    "main_menu.queue.css",
    "main_menu.settings.css",
    "main_menu.auth.css",
    "main_menu.campaign.css"
];
const menuCss = Object.fromEntries(menuCssFiles.map((file) => [file, fs.readFileSync(path.join(shell, file), "utf8")]));
const storeSource = fs.readFileSync(path.join(shell, "main_menu.store.js"), "utf8");
const profileCss = fs.readFileSync(path.join(shell, "main_menu.profile.css"), "utf8");
const profileSource = fs.readFileSync(path.join(shell, "main_menu.profile.js"), "utf8");
const nativeProfileSource = [
    "../../sow-client/src/app/account.rs",
    "../../sow-client/src/asset.rs",
    "../../sow-client/src/lib.rs",
    "../../sow-client/src/player_progress.rs",
    "../../sow-client/src/render/interact/actions.rs",
    "../../sow-client/src/ui/main_menu/mod.rs",
    "../../sow-client/src/ui.rs"
].map((file) => fs.readFileSync(path.join(shell, file), "utf8")).join("\n");
const pokiSource = fs.readFileSync(path.join(shell, "main_menu.poki.js"), "utf8");
const storyCss = fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
const siteDistSource = fs.readFileSync(path.join(shell, "../../sow-dist/src/main.rs"), "utf8");
const prodDistSource = fs.readFileSync(path.join(shell, "../../sow-dist/src/prod.rs"), "utf8");
const siteChrome = fs.readFileSync(path.join(shell, "../../sow-web/site/site-chrome.js"), "utf8");
const siteDropdown = fs.readFileSync(path.join(shell, "sow-dropdown.js"), "utf8");
const campaignModule = fs.readFileSync(path.join(shell, "../../sow-client/src/campaign/mod.rs"), "utf8");

function renderSettingsBody(source, endMarker) {
    const start = source.indexOf("function renderSettings()");
    const end = source.indexOf(endMarker, start);
    assert.notEqual(start, -1);
    assert.notEqual(end, -1);
    return source.slice(start, end);
}


test("mobile menu renders its language selector after locale bootstrap", async () => {
    const controls = Array.from({ length: 2 }, () => ({ className: "", innerHTML: "", querySelector: () => null }));
    const menu = { classList: { toggle() {} }, setAttribute() {}, querySelectorAll: () => [] };
    const menuToggle = { setAttribute() {}, addEventListener() {} };
    const document = {
        documentElement: {},
        querySelectorAll: selector => selector === "[data-locale-select]" ? controls : [],
        querySelector: selector => selector === "#mobile-menu" ? menu : selector === "[data-menu-toggle]" ? menuToggle : null,
        addEventListener() {}
    };
    const window = {
        SOW_I18N_READY: Promise.resolve(),
        SOW_t: key => key,
        SOW_getLocale: () => "en",
        SOW_getLocaleTag: locale => locale,
        SOW_getSupportedLocales: () => ["en", "es"],
        SOW_getLocaleLabel: locale => locale,
        addEventListener() {}
    };
    const context = { document, window };
    vm.runInNewContext(siteDropdown, context);
    vm.runInNewContext(siteChrome, context);
    await window.SOW_I18N_READY;
    await Promise.resolve();
    assert.match(controls[1].innerHTML, /sow-control-dropdown__trigger/);
});


test("menu CSS keeps shared layout separate from live screen domains", () => {
    for (const file of menuCssFiles) assert.ok(menuCss[file].length > 0, file);
    assert.match(menuCss["main_menu.base.css"], /#sow-menu/);
    assert.match(menuCss["main_menu.layout.css"], /sow-menu__main-nav/);
    assert.match(menuCss["main_menu.lobbies.css"], /sow-menu__lobbies/);
    assert.match(menuCss["main_menu.create.css"], /sow-create__/);
    assert.match(menuCss["main_menu.queue.css"], /sow-menu__queue-summary-card/);
    assert.match(menuCss["main_menu.settings.css"], /sow-menu__settings-modal/);
    assert.match(menuCss["main_menu.auth.css"], /sow-auth__/);
    assert.match(menuCss["main_menu.campaign.css"], /sow-campaign__/);
    assert.doesNotMatch(menuCss["main_menu.base.css"], /sow-(create|auth|campaign)__/);
    assert.doesNotMatch(menuCss["main_menu.base.css"], /\.sow-menu__queue(?![-\w])/);
    assert.doesNotMatch(menuCss["main_menu.layout.css"], /\.sow-menu__queue(?![-\w])/);
    assert.match(lobbiesSource, /modeClass = "sow-menu__mode-chip--"/);
    assert.match(shellSource, /sow-auth__provider-icon--/);
});


test("profile presents verified history, career commanders, localized achievements, and a paged global board", () => {
    const matchRowStart = profileSource.indexOf("function profileMatchRow");
    const matchRowEnd = profileSource.indexOf("function profileHeaderMarkup", matchRowStart);
    assert.ok(matchRowStart >= 0 && matchRowEnd > matchRowStart);
    const matchRow = profileSource.slice(matchRowStart, matchRowEnd);
    assert.match(matchRow, /match\.verified[\s\S]*profile\.match_pending[\s\S]*profile\.match_unverified/);
    assert.match(matchRow, /var verified = !!match\.verified;/);
    assert.match(matchRow, /var result = verified\s+\? \(match\.won \? SOW_t\("profile\.win"\) : SOW_t\("profile\.loss"\)\)/);
    assert.match(profileSource, /profile\.achievement_category_/);
    assert.match(profileSource, /profile\.achievement_" \+ achievement\.id/);
    assert.match(profileSource, /\(data\.leaders \|\| \[\]\)\.map\(profileLeaderCard\)/);
    assert.doesNotMatch(profileSource, /achievement\.id === "first_command"|achievement\.id === "first_victory"/);
    assert.match(profileSource, /leaderboard\?kind=victories&cursor=/);
    assert.match(profileSource, /profileVictoryLeaderboardCursor = data\.next_cursor/);
    assert.match(profileSource, /data-command='load_victory_more'/);
    assert.match(profileSource, /profile\.preferred_commander/);
    assert.match(profileSource, /sow-profile__dossier/);
    assert.match(profileSource, /sow-profile__tools/);
    assert.match(profileSource, /aria-controls='sow-profile-panel-active'/);
    assert.match(profileSource, /id='sow-profile-panel-active'[\s\S]*role='tabpanel'[\s\S]*aria-labelledby='sow-profile-tab-/);
    assert.match(shellSource, /\["ArrowLeft", "ArrowRight", "Home", "End"\][\s\S]*profileTabs\[nextProfileTabIndex\]\.click\(\)[\s\S]*selectedProfileTab\.focus\(\)/);
    assert.match(profileCss, /--sow-profile-gold-line/);
    assert.match(profileCss, /grid-template-columns: minmax\(0, 1fr\) minmax\(228px, 272px\)/);
    assert.match(profileCss, /@media \(max-width: 900px\)[\s\S]*sow-profile__page[\s\S]*grid-template-columns: minmax\(0, 1fr\)/);
    assert.match(profileCss, /sow-profile__tools \.sow-profile__section[\s\S]*background: var\(--sow-profile-panel\)/);
    assert.match(profileCss, /@media \(max-width: 700px\), \(orientation: portrait\)[\s\S]*sow-profile__stats[\s\S]*repeat\(2, minmax\(0, 1fr\)\)/);
    assert.match(profileCss, /:focus-visible\s*\{\s*outline: 2px solid var\(--sow-cyan\)/);
    assert.match(profileCss, /sow-profile__victory-row > b:first-child[\s\S]*color: var\(--sow-cyan\)/);
    assert.match(profileCss, /\.sow-profile__heading[\s\S]*var\(--sow-hero\)/);
    assert.match(profileCss, /\.sow-profile__match-kda\s*\{\s*grid-column: 3;\s*grid-row: 2;/);
    assert.doesNotMatch(profileCss, /\.sow-profile__match-kda\s*\{\s*display:\s*none/);
    assert.match(shellSource, /profileHeroImage\(profileData && profileData\.preferred_leader\)/);
    assert.match(profileSource, /profileHeroImage\(data && data\.preferred_leader\)/);
    assert.doesNotMatch(profileSource.slice(profileSource.indexOf('profileTab === "overview"'), profileSource.indexOf('profileTab === "leaders"')), /profileLeaderCard|profileAchievementsMarkup/);
    assert.doesNotMatch(profileSource, /loadProfileRatings|profileTab === "ranked"|profileRecentLeadersPanel/);
    assert.doesNotMatch(profileCss, /sow-profile__favorites|sow-profile__rating/);
    assert.doesNotMatch(nativeProfileSource, /LoadProfileRatings|ProfileRatingsLoaded|ratings_loaded|ProfileTab::Ranked|ranked records/i);
});


test("profile banner always uses landscape leader art", () => {
    const start = coreSource.indexOf("function heroImage(");
    const end = coreSource.indexOf("function avatarImage(", start);
    assert.ok(start >= 0 && end > start);
    const context = {
        state: null,
        window: {},
        leaderById: id => ({ slug: String(id).toLowerCase() }),
        leaderArtUrl: (slug, variant) => `${slug}:${variant || "viewport-default"}`
    };
    vm.runInNewContext(coreSource.slice(start, end) + "; this.profileHeroImage = profileHeroImage;", context);
    assert.equal(context.profileHeroImage("Boudica"), "boudica:desktop");
    assert.match(coreSource, /OWNER DECISION: the profile identity banner always uses landscape art/);
});


test("desktop navigation stacks battle, heroes, profile, and shop", () => {
    const layout = menuCss["main_menu.layout.css"];
    const start = layout.indexOf("@media (min-width: 681px) and (orientation: landscape) {");
    const end = layout.indexOf("@media (min-width: 681px) and (orientation: landscape) and (max-height: 560px)", start);
    assert.ok(start >= 0 && end > start);
    const orders = Array.from(layout.slice(start, end).matchAll(
        /\.sow-menu__main-nav-item\[data-nav-screen="(battle|heroes|profile|store)"\]\s*\{\s*order:\s*(\d+);\s*\}/g
    ), match => [match[1], Number(match[2])]);
    assert.deepEqual(orders, [["battle", 1], ["heroes", 2], ["profile", 3], ["store", 4]]);
});


test("match history row shows the leader and verified KDA while provisional exits never become losses", () => {
    const rowStart = profileSource.indexOf("function profileMatchRow");
    const rowEnd = profileSource.indexOf("function profileHeaderMarkup", rowStart);
    const context = {
        SOW_t: (key, values) => key === "profile.kda_value"
            ? `${values.kills}/${values.deaths}/${values.assists}`
            : key,
        asset: (path) => `/assets/${path}`,
        esc: (value) => String(value),
        formatMapName: () => "Britain",
        leaderById: (id) => ({ slug: id.toLowerCase() }),
        leaderDisplayName: () => "Boudica",
        profileMatchDate: () => "Today",
        profileMatchDuration: () => "12m 4s"
    };
    vm.runInNewContext(`${profileSource.slice(rowStart, rowEnd)}; this.profileMatchRow = profileMatchRow;`, context);

    const verified = context.profileMatchRow({
        match_id: "verified-match", verified: true, won: false,
        leader: "Boudica", kills: 4, deaths: 2, assists: 1
    });
    assert.match(verified, /Boudica/);
    assert.match(verified, /4\/2\/1/);
    assert.match(verified, /profile\.loss/);

    const provisional = context.profileMatchRow({
        match_id: "provisional-match", verified: false, provisional: true,
        leader: "Boudica", kills: 0, deaths: 0, assists: 0
    });
    assert.match(provisional, /profile\.match_pending/);
    assert.doesNotMatch(provisional, /profile\.loss/);
});


test("settings panel keeps only useful controls and real account state", () => {
    const settings = [
        renderSettingsBody(shellSource, "function authApi"),
        renderSettingsBody(pokiSource, "function pokiLicenseUrl")
    ];
    for (const body of settings) {
        assert.match(body, /menu\.settings/);
        assert.match(body, /menu\.settings_preferences/);
        assert.match(body, /menu\.game_links/);
        assert.match(body, /sow-menu__settings-section/);
        assert.match(body, /data-command='toggle_settings'/);
        assert.match(body, /menu\.music_volume/);
        assert.match(body, /menu\.motion_animation/);
        assert.match(body, /menu\.language/);
        assert.match(body, /account-value/);
        assert.match(body, /settings-account/);
        assert.doesNotMatch(body, /system_configuration|menu\.done|master_audio|settings-mute|account-row|WOU-ID|\+1/);
    }
    assert.match(shellSource, /SOW_getWouSession/);
    assert.match(shellSource, /data-command='confirm_sign_out'/);
    assert.match(shellSource, /data-command='cancel_sign_out'/);
    assert.match(shellSource, /data-menu-overlay='settings'/);
    assert.match(shellSource, /event\.target === settingsOverlay/);
    assert.match(shellSource, /event\.key === "Escape" && settingsOpen/);
    assert.doesNotMatch(lobbiesSource, /lobbies\.connecting_server/);
});


test("Opus theme preserves its crossfade and resumes from ordinary trusted clicks", async () => {
    const start = shellSource.indexOf("    var menuMusic = {");
    const end = shellSource.indexOf("    function renderTopbar()", start);
    assert.ok(start >= 0 && end > start, "shared shell has no menu music lifecycle");

    function createHarness(assetsUrl = "https://cdn.test/assets/") {
        const audios = [];
        const mediaMetadata = [];
        const frames = new Map();
        const logs = [];
        const windowListeners = new Map();
        const documentListeners = new Map();
        const body = { children: [], appendChild(element) { this.children.push(element); element.parentNode = this; return element; } };
        let frameId = 0;
        class FakeAudio {
            constructor() {
                this.listeners = {};
                this.paused = true;
                this.currentTime = 0;
                this.duration = 240;
                this.volume = 1;
                this.preload = "auto";
                this.loop = false;
                this.playCalls = 0;
                this.pauseCalls = 0;
                this.loadCalls = 0;
                this.nextPlayError = null;
                this.error = null;
                audios.push(this);
            }
            addEventListener(type, callback) { (this.listeners[type] ||= []).push(callback); }
            dispatch(type) { (this.listeners[type] || []).forEach(callback => callback()); }
            play() {
                this.playCalls += 1;
                if (this.nextPlayError) {
                    const error = this.nextPlayError;
                    this.nextPlayError = null;
                    return Promise.reject(error);
                }
                this.paused = false;
                return Promise.resolve();
            }
            pause() { this.pauseCalls += 1; this.paused = true; }
            load() { this.loadCalls += 1; }
            set src(value) { this._src = value; }
            get src() { return this._src; }
        }
        class FakeMediaMetadata {
            constructor(value) { Object.assign(this, value); mediaMetadata.push(this); }
        }
        const mediaSession = { metadata: null };
        const window = {
            Audio: FakeAudio,
            MediaMetadata: FakeMediaMetadata,
            URL,
            navigator: { mediaSession },
            SOW_ASSETS_URL: assetsUrl,
            SOW_BUILD_TS: "build-42",
            SOW_t: key => key,
            addEventListener(type, callback, options) {
                (windowListeners.get(type) || windowListeners.set(type, []).get(type)).push({ callback, once: Boolean(options && options.once) });
            },
            dispatchEvent(event) {
                const listeners = windowListeners.get(event.type) || [];
                listeners.slice().forEach(listener => {
                    listener.callback(event);
                    if (listener.once) listeners.splice(listeners.indexOf(listener), 1);
                });
            },
            requestAnimationFrame(callback) { const id = ++frameId; frames.set(id, callback); return id; },
            cancelAnimationFrame(id) { frames.delete(id); }
        };
        const root = { contains(target) { return Boolean(target && target.insideMenu); } };
        const document = {
            body,
            baseURI: "https://cdn.test/play/",
            querySelector(selector) {
                return selector === 'meta[name="sow-media-artwork"]'
                    ? { content: "../icon-192.png" }
                    : null;
            },
            addEventListener(type, callback, capture) {
                (documentListeners.get(type) || documentListeners.set(type, []).get(type)).push({ callback, capture: Boolean(capture) });
            },
            dispatchClick(target, isTrusted = true) {
                const event = { target, isTrusted, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
                (documentListeners.get("click") || []).filter(listener => listener.capture)
                    .forEach(listener => listener.callback(event));
                return event;
            }
        };
        const context = {
            window,
            document,
            root,
            console: {
                error: (...args) => logs.push(["error", ...args]),
                warn: (...args) => logs.push(["warn", ...args])
            }
        };
        vm.runInNewContext(shellSource.slice(start, end) +
            "this.musicApi = { syncMenuMusic, menuMusic, menuMusicFinishLoaderCycle, menuMusicSetCinematicActive };", context);
        return {
            api: context.musicApi,
            audios,
            logs,
            mediaMetadata,
            mediaSession,
            window,
            document,
            body,
            frame(timestamp) {
                const next = frames.entries().next().value;
                assert.ok(next, "expected a scheduled music fade frame");
                frames.delete(next[0]);
                next[1](timestamp);
            }
        };
    }
    const flushPlay = async () => { await Promise.resolve(); await Promise.resolve(); await Promise.resolve(); };
    const state = (phase, volume = 0.8, muted = false, loader_job = "Boot", loader_cycle_id = 0, waiting = false) => ({
        phase, loader_job, loader_cycle_id, waiting, settings: { music_volume: volume, mute_all: muted }
    });

    const h = createHarness();
    h.api.syncMenuMusic(state("Playing", 0.8, false, "EnterGame", 1), false);
    assert.equal(h.audios.length, 2, "both routes prepare the existing loop player during the loader");
    const [first, second] = h.audios;
    assert.equal(first.src, "https://cdn.test/assets/gameplay/music/shadows-of-war.webm?v=build-42");
    assert.equal(second.src, first.src, "both decks use Opus");
    assert.equal(first.preload, "none", "music never competes with the WASM download");
    assert.equal(second.preload, "none", "the loop's second deck does not preload a duplicate copy");
    assert.equal(first.loadCalls, 0, "no theme bytes are requested during the engine download");
    assert.equal(second.loadCalls, 0);
    h.window.dispatchEvent({ type: "sow:wasm-bytes-ready" });
    assert.equal(first.loadCalls, 1, "the theme starts fetching once the engine bytes are down");
    assert.equal(second.loadCalls, 0, "only the active deck buffers early");
    assert.equal(first.playCalls, 0, "music waits until the initial loader finishes");
    h.api.menuMusicSetCinematicActive(true);
    h.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    assert.equal(first.paused, true, "an opening tutorial cinematic keeps music silent after the loader");
    assert.equal(first.playCalls, 0, "the theme does not start underneath the cinematic");
    h.api.menuMusicSetCinematicActive(false);
    await flushPlay();
    assert.equal(first.paused, false, "the theme starts when the tutorial cinematic ends");
    assert.equal(first.playCalls, 1);
    assert.equal(first.volume, 0.8);
    assert.equal(h.mediaMetadata.length, 1, "the existing player publishes metadata when playback starts");
    assert.equal(h.mediaMetadata[0].title, "Shadows of War");
    assert.deepEqual(Array.from(h.mediaMetadata[0].artwork, art => ({
        src: art.src,
        sizes: art.sizes,
        type: art.type
    })), [{ src: "https://cdn.test/icon-192.png", sizes: "192x192", type: "image/png" }]);
    assert.equal(h.mediaSession.metadata, h.mediaMetadata[0]);

    first.currentTime = 55;
    h.api.menuMusicSetCinematicActive(true);
    assert.equal(first.paused, true, "a later cinematic pauses the current theme");
    h.api.menuMusicSetCinematicActive(false);
    await flushPlay();
    assert.equal(first.currentTime, 55, "the theme resumes where it paused after a later cinematic");
    h.api.syncMenuMusic(state("Splash", 0.8, false, "Boot", 1), false);
    h.api.syncMenuMusic(state("Playing", 0.8, false, "Boot", 1), false);
    await flushPlay();
    assert.equal(first.currentTime, 55, "menu, tutorial, and gameplay state changes keep the current position");
    assert.equal(first.paused, false, "playable phases keep the theme running");

    first.currentTime = 237.01;
    first.dispatch("timeupdate");
    await flushPlay();
    assert.ok(h.api.menuMusic.crossfade, "the existing loop crossfade starts near the track end");
    h.api.syncMenuMusic(state("Playing", 0.8, false, "EnterGame", 2), false);
    assert.equal(h.api.menuMusic.crossfade, null, "entering a loader cancels an in-progress loop crossfade");
    assert.equal(first.paused, true, "the active deck pauses as the game loader begins");
    assert.equal(second.paused, true, "the incoming crossfade deck also pauses");
    first.dispatch("timeupdate");
    assert.equal(h.api.menuMusic.crossfade, null, "late media events cannot restart the loop during a loader");
    assert.equal(h.api.menuMusicFinishLoaderCycle(2), true);
    await flushPlay();
    assert.equal(first.currentTime, 0, "a new game starts the theme from its opening");
    assert.equal(second.currentTime, 0, "both decks reset at a game boundary");
    assert.equal(first.paused, false, "the theme resumes after the game loader closes");
    assert.equal(second.paused, true, "only one deck resumes after the loader");

    h.api.syncMenuMusic(state("MainMenu", 0.8, false, "ExitGame", 3), false);
    assert.equal(first.paused, true, "the return-to-menu loader is silent");
    assert.equal(h.api.menuMusicFinishLoaderCycle(3), true);
    await flushPlay();
    assert.equal(first.currentTime, 0, "returning to the menu restarts the theme");
    assert.equal(first.paused, false);

    h.api.syncMenuMusic(state("MainMenu", 0.8, false, "Boot", 3, true), false);
    first.currentTime = 37;
    h.api.syncMenuMusic(state("MainMenu", 0.8, false, "Boot", 3, false), false);
    await flushPlay();
    assert.equal(first.currentTime, 0, "leaving a lobby without a loader restarts the theme");
    assert.equal(first.paused, false);

    first.currentTime = 23;
    assert.equal(h.api.menuMusicFinishLoaderCycle(2), false, "an old loader event cannot reset the current track");
    assert.equal(h.api.menuMusicFinishLoaderCycle(3), false, "a duplicate completion cannot reset the current track");
    assert.equal(first.currentTime, 23);

    const returning = createHarness();
    returning.api.syncMenuMusic(state("MainMenu"), false);
    assert.equal(returning.api.menuMusic.decks[0].playCalls, 0, "Boot stays silent while its loader is visible");
    returning.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    assert.equal(returning.api.menuMusic.decks[0].paused, false, "a returning account starts music on the menu route");

    h.api.syncMenuMusic(state("MainMenu", 0.3), true, 0.3);
    assert.equal(first.volume, 0.3, "the slider changes music gain");
    h.api.syncMenuMusic(state("MainMenu", 0.3, true), false);
    assert.equal(first.volume, 0, "global mute silences music");
    h.api.syncMenuMusic(state("MainMenu", 0.3, false), false);
    assert.equal(first.volume, 0.3, "unmute restores the music slider level");

    first.duration = 240;
    first.currentTime = 237.01;
    first.dispatch("timeupdate");
    await flushPlay();
    h.frame(2000);
    h.frame(3500);
    assert.ok(Math.abs(first.volume - 0.15) < 0.001 && Math.abs(second.volume - 0.15) < 0.001,
        "the loop crossfade reaches an even mix halfway through three seconds");
    h.frame(5000);
    assert.equal(h.api.menuMusic.active, 1);
    assert.equal(first.paused, true);
    assert.equal(first.currentTime, 0);

    const blocked = createHarness();
    blocked.api.syncMenuMusic(state("MainMenu"), false);
    blocked.audios[0].nextPlayError = Object.assign(new Error("gesture required"), { name: "NotAllowedError" });
    blocked.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    assert.equal(blocked.api.menuMusic.playBlocked, true, "the browser rejection is remembered for the next user gesture");
    assert.equal(blocked.api.menuMusic.unlockButton, undefined, "blocked autoplay creates no recovery button");
    assert.equal(blocked.body.children.length, 0, "music recovery adds no floating UI");
    blocked.document.dispatchClick({ insideMenu: false }, false);
    await flushPlay();
    assert.equal(blocked.api.menuMusic.decks[0].playCalls, 1, "an untrusted event cannot unlock playback");
    const musicClick = blocked.document.dispatchClick({ insideMenu: false });
    await flushPlay();
    assert.equal(blocked.api.menuMusic.decks[0].paused, false, "an ordinary trusted click starts the theme");
    assert.equal(musicClick.defaultPrevented, false, "music recovery does not consume the clicked action");

    const resumed = createHarness();
    resumed.api.syncMenuMusic(state("Playing"), false);
    resumed.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    const resumedDeck = resumed.api.menuMusic.decks[0];
    const callsBeforeSleep = resumedDeck.playCalls;
    resumedDeck.pause();
    const wakeClick = resumed.document.dispatchClick({ insideMenu: false });
    await flushPlay();
    assert.equal(resumedDeck.playCalls, callsBeforeSleep + 1, "a normal game click resumes music paused by the platform");
    assert.equal(resumedDeck.paused, false);
    assert.equal(wakeClick.defaultPrevented, false, "audio recovery leaves the game click intact");

    const unsupported = createHarness();
    unsupported.api.syncMenuMusic(state("MainMenu"), true);
    await flushPlay();
    const failedOpusDeck = unsupported.api.menuMusic.decks[0];
    failedOpusDeck.dispatch("error");
    assert.equal(failedOpusDeck.src, "https://cdn.test/assets/gameplay/music/shadows-of-war.webm?v=build-42",
        "an unsupported Opus file gets no alternate format");
    assert.ok(unsupported.logs.some(entry => entry[0] === "error"), "Opus failure is visible in the console");
    assert.doesNotMatch(shellSource, /shadows-of-war\.mp3|menuMusicSwitchToMp3|canPlayType/,
        "the player has no codec fallback");

    const withoutMediaSession = createHarness();
    delete withoutMediaSession.window.navigator;
    withoutMediaSession.api.syncMenuMusic(state("MainMenu"), true);
    withoutMediaSession.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    assert.equal(withoutMediaSession.api.menuMusic.decks[0].paused, false,
        "missing platform metadata support does not block music playback");

    const native = createHarness("../assets");
    native.api.syncMenuMusic(state("MainMenu"), false);
    assert.equal(native.api.menuMusic.decks[0].src, "../assets/gameplay/music/shadows-of-war.webm?v=build-42",
        "Tauri resolves the same music through its local SOW_ASSETS_URL");
    assert.match(shellSource, /addEventListener\("sow:loader-ready", menuMusicStartAtLoaderEnd, \{ once: true \}\)/);
    assert.match(shellSource, /window\.addEventListener\("sow:loader-cycle-ready", function \(event\)[\s\S]*?menuMusicFinishLoaderCycle\(cycleId\)/,
        "the existing loader completion event releases the paused theme once per cycle");
    assert.doesNotMatch(shellSource, /menuMusicPauseForGame|menuMusic\.inMenu|menuMusic\.fade/,
        "music is not tied to a menu-only playback mode");
    assert.doesNotMatch(shellSource, /menuMusicShowUnlockPrompt|data-sow-music-unlock|sow-menu__music-unlock/);
    assert.doesNotMatch(storyCss, /sow-menu__music-unlock/);
});


test("music volume no longer controls SFX and music assets reach every shared-shell package", () => {
    const musicCommand = webMenu.match(/WebMenuCommand::SetMusicVolume \{ value \} => \{[\s\S]*?\n                \}/);
    assert.ok(musicCommand);
    assert.match(musicCommand[0], /settings_state\.music_volume = volume/);
    assert.doesNotMatch(musicCommand[0], /set_master_volume/);
    assert.match(webMenu, /WebMenuCommand::SetMute \{ value \} => \{[\s\S]*set_master_volume\(if value \{ 0\.0 \} else \{ 0\.8 \}\)/);
    assert.match(shellSource, /syncMenuMusic\(state, false\)/);
    assert.match(shellSource, /syncMenuMusic\(state, true\)/);
    assert.match(siteDistSource, /copy_dir\(&src\.join\("gameplay\/music"\), &dst\.join\("gameplay\/music"\)\)/);
    assert.match(siteDistSource, /assets_gameplay\.join\("music"\)/);
    assert.equal((siteDistSource.match(/"assets\/gameplay\/music\/shadows-of-war\.webm"/g) || []).length, 4,
        "web, Poki, Jest, and native layout checks require the theme assets");
    assert.match(siteDistSource, /paths\.assets_gameplay\.join\("music"\)/,
        "native package fingerprint and copy include music assets");
    assert.doesNotMatch(siteDistSource, /shadows-of-war\.mp3/,
        "no package requires an MP3 duplicate");
    assert.match(prodDistSource, /&paths\.assets_gameplay,/,
        "the web cache fingerprint includes gameplay music");
});


test("hero purchase stays server-backed, direct, and visually honest", () => {
    assert.match(storeSource, /var leaderActions = !offer \|\| offer\.owned \|\| offer\.permanently_free \? ""/);
    assert.doesNotMatch(storeSource, /offer\.free_rotation \|\| offer\.available/);
    assert.match(storeSource, /data-command='unlock_leader'/);
    assert.match(storeSource, /data-command='open_skin_purchase'/);
    assert.match(storeSource, /sow-store__currency-amount--insufficient/);
    // One price per item: leaders and skins cost gems or crowns, never dollars.
    assert.doesNotMatch(storeSource, /direct_product_id|open_product_purchase/);
    assert.match(storeSource, /data-command='buy_product'/);
    assert.match(storeSource, /SOW_t\("store\.buy"\) \+ \(!isAndroidTwa\(\) && bundle\.price_label/);
    assert.match(shellSource, /data-menu-overlay='purchase'/);
    assert.match(shellSource, /openLeaderPurchase/);
    assert.match(shellSource, /send\("unlock_leader", \{ leader_id: purchaseModal\.leaderId, currency: purchaseModal\.currency \}\)/);
    assert.match(shellSource, /send\("unlock_skin", \{ skin_id: purchaseModal\.skinId \}\)/);
    assert.match(storeSource, /purchaseModal\.phase = "success"/);
    assert.match(storeSource, /purchaseItemOwned/);
    assert.match(storeSource, /reducedRewardMotion\(\) \? " is-reduced-motion"/);
    assert.match(storeSource, /sow-purchase-modal__visual/);
    assert.match(menuCss["main_menu.base.css"], /sow-purchase-modal__layout/);
    assert.doesNotMatch(menuCss["main_menu.base.css"], /sow-purchase-art-reveal/);
    assert.match(menuCss["main_menu.base.css"], /sow-purchase-impact-ring/);
    assert.match(menuCss["main_menu.base.css"], /sow-purchase-modal-particle/);
    const leaderStart = coreSource.indexOf("function leaderById(id)");
    const leaderEnd = coreSource.indexOf("function leaderPerk", leaderStart);
    const context = { state: { leaders: [{ id: "SunTzu", slug: "sun_tzu" }] } };
    vm.runInNewContext(coreSource.slice(leaderStart, leaderEnd) + "this.leaderById = leaderById;", context);
    assert.equal(context.leaderById("sun_tzu").slug, "sun_tzu");
});


test("hero selection opens the shared catalog for equipped and purchasable skins", () => {
    const rowStart = storeSource.indexOf("function renderLeaderPurchase");
    const rowEnd = storeSource.indexOf("function renderSkinPickerModal", rowStart);
    assert.ok(rowStart >= 0 && rowEnd > rowStart);
    assert.match(storeSource.slice(rowStart, rowEnd), /data-command='open_skin_picker'/);
    assert.doesNotMatch(storeSource.slice(rowStart, rowEnd), /if \(!offer \|\| offer\.owned\) return/);
    assert.match(storeSource, /function renderSkinPickerModal\(\)[\s\S]*skins\.map\(renderStoreSkinPromo\)/);
    assert.match(storeSource, /data-menu-overlay='skin-picker'/);
    assert.match(storeSource, /data-command='equip_skin'/);
    assert.match(storeSource, /data-command='open_skin_purchase'/);
    assert.match(shellSource, /command === "open_skin_picker"[\s\S]*loadStoreCatalog\(\)/);
    assert.match(shellSource, /command === "close_skin_picker"/);
    assert.match(shellSource, /syncOverlay\("skin-picker"[\s\S]*syncOverlay\("purchase"/);
    assert.match(profileCss, /\.sow-menu__modal\.sow-skin-picker/);
});


test("leader display names use the catalog translation keys for every leader", () => {
    const start = coreSource.indexOf("function leaderTranslationSlug(leader)");
    const end = coreSource.indexOf("function mapInfo(key)", start);
    assert.ok(start >= 0 && end > start);
    const requestedKeys = [];
    const context = {
        SOW_t(key) {
            requestedKeys.push(key);
            return "translated:" + key;
        }
    };
    vm.runInNewContext(coreSource.slice(start, end) + "this.leaderDisplayName = leaderDisplayName; this.leaderHistoricalName = leaderHistoricalName;", context);

    const slugs = [
        "caesar", "cleopatra", "ragnar", "sun_tzu", "alexander", "genghis_khan",
        "richard_the_lionheart", "vercingetorix", "boudica", "lady_six_sky", "leonidas", "napoleon"
    ];
    const expectedSlugs = [
        "caesar", "cleopatra", "ragnar", "suntzu", "alexander", "genghiskhan",
        "richard", "vercingetorix", "boudica", "ladysixsky", "leonidas", "napoleon"
    ];
    for (let i = 0; i < slugs.length; i++) {
        const expectedKey = "heroes.leader_" + expectedSlugs[i] + "_name";
        assert.equal(context.leaderDisplayName({ slug: slugs[i], name: "Fallback" }), "translated:" + expectedKey);
        assert.equal(requestedKeys[i], expectedKey);
    }
    assert.equal(
        context.leaderHistoricalName({ slug: "richard_the_lionheart", name: "Richard the Lionheart" }),
        "translated:heroes.leader_richard_historical"
    );
});


test("direct purchases wait for delivery before celebrating", () => {
    assert.match(storeSource, /EXTERNAL_PURCHASE_POLL_MS = 3000/);
    assert.match(storeSource, /EXTERNAL_PURCHASE_WAIT_MS = 120000/);
    assert.match(storeSource, /sow_pending_store_purchase_v1/);
    assert.match(storeSource, /profileApi\("\/store\/purchases"\)/);
    assert.match(storeSource, /record\.status === "granted"/);
    assert.match(storeSource, /onComplete: function \(\) \{\s*completeExternalPurchase\(productId\);/);
    assert.match(storeSource, /purchaseModal\.type === "product"[\s\S]*attempt\.delivery_id[\s\S]*profileHasExternalPurchase/);
    assert.match(storeSource, /purchaseModal\.type === "bundle"[\s\S]*beginBundlePurchasePresentation/);
    assert.match(storeSource, /purchaseModal\.phase === "spending" \|\| purchaseModal\.phase === "success"/);
    assert.match(shellSource, /resumeExternalPurchaseDelivery\(true\)/);
    assert.doesNotMatch(shellSource, /URLSearchParams\(window\.location\.search\)\.get\("purchase"\)/);
});


test("weekly free rotation is a splash-only status", () => {
    const cardStart = heroesSource.indexOf("function renderHeroesCard");
    const cardEnd = heroesSource.indexOf("function heroesRoster", cardStart);
    assert.ok(cardStart >= 0 && cardEnd > cardStart);
    assert.doesNotMatch(heroesSource.slice(cardStart, cardEnd), /data-hero-status|weekly_free_rotation/);
    const predicateStart = heroesSource.indexOf("function isWeeklyFreeRotation");
    const predicateEnd = heroesSource.indexOf("function renderHeroesCard", predicateStart);
    assert.ok(predicateStart >= 0 && predicateEnd > predicateStart);
    const context = {};
    vm.runInNewContext(heroesSource.slice(predicateStart, predicateEnd) + "this.isWeeklyFreeRotation = isWeeklyFreeRotation;", context);
    assert.equal(context.isWeeklyFreeRotation({ available: false, free_rotation: true, owned: false }), false);
    assert.equal(context.isWeeklyFreeRotation({ available: true, free_rotation: true, owned: false }), true);
    assert.equal((heroesSource.match(/isWeeklyFreeRotation\(activeLeader\)/g) || []).length, 2);
    assert.match(profileCss, /\.sow-heroes__rotation\[hidden\][\s\S]*display:\s*none/);
    assert.match(profileCss, /\.sow-heroes__perk\[hidden\][\s\S]*display:\s*none/);
    assert.match(heroesSource, /status\.hidden = randomSelected/);
    assert.match(heroesSource, /status\.textContent = randomSelected \? ""/);
});

test("heroes preview never resolves the random option through leader i18n keys", () => {
    const previewStart = heroesSource.indexOf("function updateHeroesPreview");
    const previewEnd = heroesSource.indexOf("function renderHeroInfoModal", previewStart);
    assert.ok(previewStart >= 0 && previewEnd > previewStart);
    const previewBody = heroesSource.slice(previewStart, previewEnd);
    assert.match(previewBody, /image\.alt = randomSelected \? randomLeaderName\(\) : leaderDisplayName\(activeLeader\)/,
        "image alt must use randomLeaderName() when random is selected, or it reports heroes.leader_random_name missing");
    assert.match(previewBody, /randomSelected \? "" : leaderHistoricalName\(activeLeader\)/,
        "historical name must not call leaderHistoricalName() for the random option, or it reports heroes.leader_random_historical missing");
    assert.doesNotMatch(previewBody, /historical\.textContent = leaderHistoricalName\(activeLeader\)/,
        "historical text must go through the randomSelected branch");
});


test("random hero keeps a translated label and a short description in every locale", () => {
    for (const locale of ["en", "es", "de", "fr", "it", "pt-br", "ru", "ja", "ko", "zh-cn", "ar", "tr", "vi", "id", "fil"]) {
        const catalog = fs.readFileSync(path.join(shell, `../../sow-i18n/strings/${locale}/web.toml`), "utf8");
        const line = catalog.split("\n").find((entry) => entry.startsWith("random_leader_description"));
        assert.ok(line && line.split("=")[1].trim().length > 2, `heroes.random_leader_description must exist in ${locale}`);
        assert.doesNotMatch(line, /server|servidor|serveur|Servidor|Server|Serveur|сервер|Сервер|サーバー|서버|服务器|الخادم|Sunucu|Máy chủ|khí/i,
            `heroes.random_leader_description must not talk about the server in ${locale}`);
        const label = catalog.split("\n").find((entry) => entry.startsWith("random_leader ="));
        assert.ok(label && !/hero/i.test(label), `random_leader must be a single word, no HERO, in ${locale}`);
    }
    assert.match(heroesSource, /SOW_t\("heroes\.random_leader_description"\)/);
});


test("header exposes server progress currencies and keeps the real XP remainder", () => {
    assert.match(shellSource, /data-progression-gems-value/);
    assert.match(shellSource, /data-progression-laurels-value/);
    assert.doesNotMatch(pokiSource, /function renderTopbar|data-progression-gems-value/);
    assert.match(shellSource, /accountXp % 100/);
    assert.match(shellSource, /state\.gems/);
    assert.match(webMenu, /"gems": progress\.gems/);
});


test("campaign episode cards use leader portraits and emphasize the next playable chapter", () => {
    const campaignCss = menuCss["main_menu.campaign.css"];
    assert.doesNotMatch(lobbiesSource, /campaign_tagline/);
    assert.match(lobbiesSource, /var mapName = episode\.id === "boudica" \? "eastanglia" : "northamerica"/);
    assert.match(lobbiesSource, /var mapArt = lobbyThumb\(\{ map_name: mapName \}\)/);
    assert.match(lobbiesSource, /asset\("gameplay\/avatars\/" \+ \(episode\.id === "boudica" \? "boudica" : "lady_six_sky"\) \+ "\.webp"\)/);
    assert.match(lobbiesSource, /sow-campaign__map-art[\s\S]*?sow-campaign__leader-art/);
    assert.match(lobbiesSource, /isNext \? " is-next"/);
    assert.match(lobbiesSource, /var watch = campaignCinematicAvailability\[episode\.id\]/);
    assert.match(lobbiesSource, /function discoverCampaignCinematics\(episodes\)[\s\S]*?definition\.entry[\s\S]*?video_src/);
    assert.match(lobbiesSource, /data-command='watch_campaign_cinematic'/);
    assert.match(campaignCss, /\.sow-campaign__episode-art img[\s\S]*?object-fit: cover/);
    assert.match(campaignCss, /\.sow-campaign__map-art[\s\S]*?brightness\(\.58\)/);
    assert.match(campaignCss, /\.sow-campaign__leader-art[\s\S]*?border-radius: 50%/);
    assert.match(campaignCss, /\.sow-campaign__episode\.is-next/);
    assert.match(campaignCss, /\.sow-campaign__episode\.is-locked \.sow-campaign__episode-art img[\s\S]*?grayscale/);
});


test("campaign card map and leader art match every Rust episode and its JSON assets", () => {
    const start = campaignModule.indexOf("pub fn episode_id(");
    const end = campaignModule.indexOf("pub fn from_episode_id(", start);
    assert.ok(start >= 0 && end > start);
    const episodeIds = [...campaignModule.slice(start, end).matchAll(/=> "([^"]+)"/g)].map(match => match[1]);
    assert.deepEqual(episodeIds, ["boudica", "six_sky_ep1", "six_sky_ep2", "six_sky_ep3"]);
    const expected = { boudica: ["eastanglia", "boudica"], six_sky_ep1: ["northamerica", "lady_six_sky"], six_sky_ep2: ["northamerica", "lady_six_sky"], six_sky_ep3: ["northamerica", "lady_six_sky"] };
    const campaignDir = path.join(shell, "../../assets/campaign");
    const cinematicEpisodes = [];
    for (const episodeId of episodeIds) {
        const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, episodeId + ".json"), "utf8"));
        const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, episodeId + ".triggers.json"), "utf8"));
        assert.equal(definition.episode_id, episodeId);
        const opening = definition.steps.find(step => step.id === definition.entry);
        if (opening && opening.video_src) {
            cinematicEpisodes.push(episodeId);
            assert.equal(opening.type, "scene", `${episodeId} opening cinematic is a scene`);
            assert.equal(opening.presentation, "cinematic", `${episodeId} opening cinematic uses the shared cinematic presentation`);
            assert.ok(opening.video_src.startsWith(`/assets/campaign/${episodeId}/`), `${episodeId} cinematic stays in its asset folder`);
            const mediaPath = path.join(shell, "../..", opening.video_src.replace(/^\/assets\//, "assets/"));
            assert.ok(fs.existsSync(mediaPath), `${episodeId} video asset exists`);
            const stem = mediaPath.replace(/\.(webm|mp4)$/i, "");
            assert.ok(fs.existsSync(stem + ".mp4") && fs.existsSync(stem + ".webm"), `${episodeId} cinematic ships both WebM and MP4 renditions`);
            assert.ok(fs.existsSync(stem + ".poster.webp"), `${episodeId} cinematic ships a poster`);
            assert.ok(fs.statSync(mediaPath).size <= 6 * 1024 * 1024, `${episodeId} cinematic rendition stays under 6 MB`);
        }
        if (episodeId === "boudica") {
            const ratioOverrides = definition.steps.filter(step => Object.hasOwn(step, "attack_ratio_on_enter"))
                .map(step => ({ id: step.id, ratio: step.attack_ratio_on_enter }));
            assert.deepEqual(ratioOverrides, [{ id: definition.entry, ratio: 0.25 }], "Boudica sets 25% once at entry and never resets the player's allocation");
        }
        assert.equal(roster.map, expected[episodeId][0], episodeId + " card map");
        assert.ok(Object.values(definition.speakers || {}).some(speaker => speaker.avatar === expected[episodeId][1]), episodeId + " card leader portrait");
        for (const faction of roster.factions) {
            assert.match(faction.id, /^[a-z][a-z0-9_]{0,95}$/i, `${episodeId}/${faction.name}: stable ID`);
            assert.ok(Number.isInteger(faction.starting_troops) && faction.starting_troops >= 0, `${episodeId}/${faction.name}: starting troops`);
            assert.ok(["allied", "neutral", "enemy"].includes(faction.relation), `${episodeId}/${faction.name}: relationship`);
            assert.ok(typeof faction.civ === "string" && faction.civ, `${episodeId}/${faction.name}: civilization`);
            assert.ok(typeof faction.leader === "string" && faction.leader, `${episodeId}/${faction.name}: leader`);
            for (const removed of ["role", "betrayal"]) assert.equal(Object.hasOwn(faction, removed), false, `${episodeId}/${faction.name}: no ${removed}`);
            assert.equal(Object.hasOwn(faction, "hostility"), false, `${episodeId}/${faction.name}: no combat override`);
            assert.match(faction.color, /^#[0-9a-f]{6}$/i, `${episodeId}/${faction.name}: map color`);
        }
    }
    assert.deepEqual(cinematicEpisodes, ["boudica"]);
    const watchFlow = tutorial.slice(tutorial.indexOf("function watchCampaignCinematic("), tutorial.indexOf("function showError(", tutorial.indexOf("function watchCampaignCinematic(")));
    assert.match(watchFlow, /SOWCampaign\.create\(data\.definition, data\.definition\.entry, data\.roster\)/);
    assert.doesNotMatch(watchFlow, /send\("start_campaign_episode"/);
});
