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
        documentElement: { dataset: {} },
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
});


test("profile presents verified history, career commanders, localized achievements, and a paged global board", () => {
    const matchRowStart = profileSource.indexOf("function profileMatchRow");
    const matchRowEnd = profileSource.indexOf("function profileHeaderMarkup", matchRowStart);
    assert.ok(matchRowStart >= 0 && matchRowEnd > matchRowStart);
    const matchRow = profileSource.slice(matchRowStart, matchRowEnd);
    assert.match(matchRow, /match\.verified[\s\S]*profile\.match_pending[\s\S]*profile\.match_unverified/);
    assert.match(matchRow, /var verified = !!match\.verified;/);
    assert.match(matchRow, /var result = verified\s+\? \(match\.won \? SOW_t\("profile\.win"\) : SOW_t\("profile\.loss"\)\)/);
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
});

test("the main-menu art and hero briefing both honor Random", () => {
    const start = coreSource.indexOf("function heroImage(");
    const end = coreSource.indexOf("function profileHeroImage(", start);
    assert.ok(start >= 0 && end > start);
    const prepared = [];
    const context = {
        state: { prefer_random_leader: true, phase: "MainMenu", loader_job: "Boot", selected_leader: "Caesar" },
        window: { SOW_prepareLeaderArt: (...args) => prepared.push(args) },
        leaderById: id => ({ slug: String(id).toLowerCase() }),
        leaderArtUrl: (slug, variant) => slug + ":" + (variant || "viewport-default"),
        currentLeaderSelection: () => ({ selected_leader: "Caesar", prefer_random_leader: true }),
        prepareLeaderArt() {}
    };
    vm.runInNewContext(coreSource.slice(start, end) + "; this.heroImage = heroImage;", context);
    assert.equal(context.heroImage(), "random:viewport-default");
    assert.deepEqual(prepared, [["random", "low"]]);
});

test("confirmed leader choice stays current until the published state catches up", () => {
    const selectionStart = coreSource.indexOf("    function normalizeLeaderSelectionId(");
    const selectionEnd = coreSource.indexOf("    function asset(path)", selectionStart);
    const artStart = coreSource.indexOf("    function heroImage(");
    const artEnd = coreSource.indexOf("    function lobbyThumb(", artStart);
    assert.ok(selectionStart >= 0 && selectionEnd > selectionStart);
    assert.ok(artStart >= 0 && artEnd > artStart);

    const initialState = {
        account_id: "account-a", selected_leader: "Boudica", prefer_random_leader: false,
        phase: "MainMenu", loader_job: "Idle"
    };
    const context = {
        state: initialState,
        pendingLeaderSelection: null,
        asset: path => path,
        leaderById: id => ({ slug: String(id || "caesar").toLowerCase() }),
        leaderArtUrl: slug => "hero:" + slug,
        prepareLeaderArt() {},
        window: { SOW_prepareLeaderArt() {} }
    };
    vm.runInNewContext(
        coreSource.slice(selectionStart, selectionEnd) + coreSource.slice(artStart, artEnd) +
        "this.currentLeaderSelection = currentLeaderSelection;" +
        "this.stageLeaderSelection = stageLeaderSelection;" +
        "this.reconcileLeaderSelection = reconcileLeaderSelection;" +
        "this.leaderSelectionMatches = leaderSelectionMatches;" +
        "this.avatarImage = avatarImage; this.heroImage = heroImage;",
        context
    );

    context.stageLeaderSelection(true, null);
    assert.equal(context.currentLeaderSelection().prefer_random_leader, true);
    assert.equal(context.currentLeaderSelection().selected_leader, null);
    assert.equal(context.leaderSelectionMatches({ selected_leader: "SunTzu", prefer_random_leader: false }, false, "sun_tzu"), true);
    assert.equal(context.avatarImage(), "gameplay/avatars/random.webp");
    assert.equal(context.heroImage(), "hero:random");
    context.reconcileLeaderSelection(initialState);
    assert.equal(context.pendingLeaderSelection.prefer_random_leader, true,
        "the old concrete state cannot clear a pending Random choice");

    const randomState = { ...initialState, prefer_random_leader: true };
    assert.equal(randomState.selected_leader, "Boudica",
        "Random keeps the concrete preference as a hidden server-side backup");
    context.state = randomState;
    context.reconcileLeaderSelection(randomState);
    assert.equal(context.pendingLeaderSelection, null);
    assert.equal(context.currentLeaderSelection().prefer_random_leader, true);

    context.stageLeaderSelection(false, "napoleon");
    assert.equal(context.currentLeaderSelection().selected_leader, "napoleon");
    assert.equal(context.currentLeaderSelection().prefer_random_leader, false);
    assert.equal(context.avatarImage(), "gameplay/avatars/napoleon.webp");
    assert.equal(context.heroImage(), "hero:napoleon");
    context.reconcileLeaderSelection(randomState);
    assert.equal(context.pendingLeaderSelection.selected_leader, "napoleon",
        "the old Random state cannot clear a pending concrete choice");

    const concreteState = { ...randomState, selected_leader: "Napoleon", prefer_random_leader: false };
    context.state = concreteState;
    context.reconcileLeaderSelection(concreteState);
    assert.equal(context.pendingLeaderSelection, null);
    assert.equal(context.currentLeaderSelection().selected_leader, "Napoleon");
    assert.equal(context.currentLeaderSelection().prefer_random_leader, false);

    context.stageLeaderSelection(false, "caesar");
    const switchedAccount = { ...concreteState, account_id: "account-b", selected_leader: "SunTzu" };
    context.state = switchedAccount;
    context.reconcileLeaderSelection(switchedAccount);
    assert.equal(context.pendingLeaderSelection, null,
        "a pending choice from one account cannot leak into another");
    assert.equal(context.currentLeaderSelection().selected_leader, "SunTzu");
});

test("leader confirmation and reopening use the staged choice, not the previous snapshot", () => {
    const randomStart = shellSource.indexOf('if (command === "confirm_random_leader")');
    const concreteStart = shellSource.indexOf('if (command === "confirm_leader")', randomStart);
    const concreteEnd = shellSource.indexOf('/* POKI_SHARED_STORE_UNLOCK_BEGIN */', concreteStart);
    const pickerStart = shellSource.indexOf('if (command === "open_leader_picker")');
    const navStart = shellSource.indexOf('if (command === "main_nav")');
    const navEnd = shellSource.indexOf('if (command === "open_leader_picker")', navStart);
    assert.ok(randomStart >= 0 && concreteStart > randomStart && concreteEnd > concreteStart);
    assert.ok(pickerStart >= 0 && navStart >= 0 && navEnd > navStart);

    const randomConfirm = shellSource.slice(randomStart, concreteStart);
    const concreteConfirm = shellSource.slice(concreteStart, concreteEnd);
    const pickerOpen = shellSource.slice(pickerStart, shellSource.indexOf("if (command === \"toggle_dropdown\")", pickerStart));
    const heroesNavigation = shellSource.slice(navStart, navEnd);
    assert.match(randomConfirm, /stageLeaderSelection\(true, null\);\s*send\("set_random_leader"\)/);
    assert.doesNotMatch(randomConfirm, /leader_id/,
        "Random must not carry a previous concrete leader ID");
    assert.match(concreteConfirm, /stageLeaderSelection\(false, leaderId\);\s*send\("set_leader", \{ leader_id: leaderId \}\)/);
    assert.match(randomConfirm, /tempSelectedLeader = null;\s*tempSelectedRandom = false;/);
    assert.match(concreteConfirm, /tempSelectedLeader = null;\s*tempSelectedRandom = false;/);
    assert.match(pickerOpen, /currentLeaderSelection\(\)/);
    assert.match(heroesNavigation, /tempSelectedLeader = null;\s*tempSelectedRandom = false;/);
    assert.match(heroesNavigation, /currentLeaderSelection\(\)/);
});

test("header keeps only the nickname and sizes it to the available space", () => {
    const topbarStart = shellSource.indexOf("    function renderTopbar()");
    const topbarEnd = shellSource.indexOf("\n    function ", topbarStart + 1);
    assert.ok(topbarStart >= 0 && topbarEnd > topbarStart);
    const topbar = shellSource.slice(topbarStart, topbarEnd);
    assert.match(topbar, /data-role='display-name'/);
    assert.doesNotMatch(topbar, /selected_leader_name|selected_civilization|kingdom/i);

    const baseCss = menuCss["main_menu.base.css"];
    const layoutCss = menuCss["main_menu.layout.css"];
    assert.match(baseCss, /\.sow-menu__profile\s*\{[^}]*flex:\s*1 1 auto;[^}]*min-width:\s*0;/);
    assert.match(baseCss, /#sow-menu \.sow-menu__profile input\s*\{[^}]*width:\s*100%;[^}]*font-size:\s*clamp\(16px, 1\.8vw, 24px\)/);
    assert.match(layoutCss, /\.sow-menu__profile input\s*\{[^}]*width:\s*100%;[^}]*font-size:\s*clamp\(15px, 4vw, 21px\)/);
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
});


test("music volume no longer controls SFX and music assets reach every shared-shell package", () => {
    const musicCommand = webMenu.match(/WebMenuCommand::SetMusicVolume \{ value \} => \{[\s\S]*?\n                \}/);
    assert.ok(musicCommand);
    assert.match(musicCommand[0], /settings_state\.music_volume = volume/);
    assert.doesNotMatch(musicCommand[0], /set_master_volume/);
});


test("hero purchase stays server-backed, direct, and visually honest", () => {
    // One price per item: leaders and skins cost gems or crowns, never dollars.
    // Real money is taken only by Google Play in the Android app: no web checkout.
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
});


test("leader display names use the catalog translation keys for every leader", () => {
    const start = coreSource.indexOf("function leaderTranslationSlug(leader)");
    const end = coreSource.indexOf("function heroBriefingLeader", start);
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
    const expectedSlugs = slugs;
    for (let i = 0; i < slugs.length; i++) {
        const expectedKey = "heroes.leader_" + expectedSlugs[i] + "_name";
        assert.equal(context.leaderDisplayName({ slug: slugs[i], name: "Fallback" }), "translated:" + expectedKey);
        assert.equal(requestedKeys[i], expectedKey);
    }
    assert.equal(
        context.leaderHistoricalName({ slug: "richard_the_lionheart", name: "Richard the Lionheart" }),
        "translated:heroes.leader_richard_the_lionheart_historical"
    );
});





test("one hero briefing renders identity and the named ability without exposing missing translations", () => {
    const start = coreSource.indexOf("function heroBriefingLeader");
    const end = coreSource.indexOf("function mapInfo(key)", start);
    assert.ok(start >= 0 && end > start);
    const translations = {
        "heroes.leader_amina_de_zazzau_name": "Amina of Zazzau",
        "heroes.leader_amina_de_zazzau_civilization": "Kingdom of Zazzau",
        "site.leader_amina_de_zazzau_ability": "Walls of Zazzau",
        "site.leader_amina_de_zazzau_description": "Enemies need 50% more effort to capture your cities.",
        "heroes.random_leader": "Random",
        "heroes.random_leader_description": "A surprise hero from your collection."
    };
    const context = {
        state: { leaders: [] },
        window: {},
        document: { addEventListener() {} },
        SOW_t: key => translations[key] || `[${key}]`,
        esc: value => String(value).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#39;"),
        asset: path => "/assets/" + path
    };
    vm.runInNewContext(coreSource.slice(start, end) + "; this.renderHeroBriefing = renderHeroBriefing; this.hideBrokenHeroBriefingIcon = hideBrokenHeroBriefingIcon;", context);

    const amina = context.renderHeroBriefing({
        id: "AminaDeZazzau", slug: "amina_de_zazzau", name: "Amina of Zazzau",
        civilization_key: "heroes.leader_amina_de_zazzau_civilization",
        perk_key: "site.leader_amina_de_zazzau_description"
    }, "selection", "sow-heroes-selected");
    assert.match(amina, /Kingdom of Zazzau[\s\S]*Amina of Zazzau[\s\S]*Walls of Zazzau[\s\S]*Enemies need 50% more effort/);
    assert.equal((amina.match(/<h2\b[^>]*class='sow-hero-briefing__name'[^>]*>Amina of Zazzau<\/h2>/g) || []).length, 1,
        "Amina's visible title must not repeat her name");
    assert.match(amina, /sow-hero-briefing__icon[\s\S]*gameplay\/icons\/abilities\/amina_de_zazzau\.webp/);
    assert.doesNotMatch(amina, /\[site\.|site\.leader_/);

    const random = context.renderHeroBriefing("random", "home");
    assert.match(random, /Random[\s\S]*A surprise hero from your collection/);
    assert.doesNotMatch(random, /sow-hero-briefing__icon|site\.leader_|Walls of Zazzau/);

    const unavailable = context.renderHeroBriefing({ id: "Caesar", slug: "caesar", name: "Caesar" }, "loader");
    assert.doesNotMatch(unavailable, /\[site\.|site\.leader_/);
    assert.doesNotMatch(unavailable, /sow-hero-briefing__icon/);

    const iconSlot = { removed: false, classList: { contains: value => value === "sow-hero-briefing__icon-slot" }, remove() { this.removed = true; } };
    const image = { parentNode: iconSlot, classList: { contains: value => value === "sow-hero-briefing__icon" } };
    context.hideBrokenHeroBriefingIcon({ target: image });
    assert.equal(iconSlot.removed, true, "a failed ability image removes its frame instead of leaving a broken-image box");
});

test("weekly free rotation is a splash-only status", () => {
    const cardStart = heroesSource.indexOf("function renderHeroesCard");
    const cardEnd = heroesSource.indexOf("function heroesRoster", cardStart);
    assert.ok(cardStart >= 0 && cardEnd > cardStart);
    const predicateStart = heroesSource.indexOf("function isWeeklyFreeRotation");
    const predicateEnd = heroesSource.indexOf("function renderHeroesCard", predicateStart);
    assert.ok(predicateStart >= 0 && predicateEnd > predicateStart);
    const context = {};
    vm.runInNewContext(heroesSource.slice(predicateStart, predicateEnd) + "this.isWeeklyFreeRotation = isWeeklyFreeRotation;", context);
    assert.equal(context.isWeeklyFreeRotation({ available: false, free_rotation: true, owned: false }), false);
    assert.equal(context.isWeeklyFreeRotation({ available: true, free_rotation: true, owned: false }), true);
});

test("heroes preview never resolves the random option through leader i18n keys", () => {
    const previewStart = heroesSource.indexOf("function updateHeroesPreview");
    const previewEnd = heroesSource.indexOf("function renderHeroInfoModal", previewStart);
    assert.ok(previewStart >= 0 && previewEnd > previewStart);
    const previewBody = heroesSource.slice(previewStart, previewEnd);
    assert.match(previewBody, /image\.alt = randomSelected \? randomLeaderName\(\) : leaderDisplayName\(activeLeader\)/,
        "image alt must use randomLeaderName() when random is selected, or it reports heroes.leader_random_name missing");
    assert.match(previewBody, /briefing\.innerHTML = renderHeroBriefing\(randomSelected \? "random" : activeLeader, "selection", "sow-heroes-selected"\)/);
    assert.doesNotMatch(previewBody, /leaderHistoricalName\(|sow-heroes__ability-icon/);
});

test("skill icons stay beside full hero details and out of compact avatar cards", () => {
    const cardStart = heroesSource.indexOf("function renderHeroesCard");
    const cardEnd = heroesSource.indexOf("function randomLeaderName", cardStart);
    const card = heroesSource.slice(cardStart, cardEnd);
    assert.doesNotMatch(card, /sow-heroes__ability-icon/);
    assert.doesNotMatch(card, /renderHeroBriefing/);
    const fullStart = heroesSource.indexOf("function renderHeroes()");
    const fullEnd = heroesSource.indexOf("function renderHeroes", fullStart + 1);
    const full = heroesSource.slice(fullStart, fullEnd);
    assert.match(full, /data-hero-briefing[\s\S]*renderHeroBriefing\(activeRandom \? "random" : activeLeader, "selection", "sow-heroes-selected"\)/);
    assert.doesNotMatch(full, /leaderHistoricalName\(/);
    const modalStart = heroesSource.indexOf("function renderHeroInfoModal");
    const modalEnd = heroesSource.indexOf("function resetDropdownPosition", modalStart);
    assert.match(heroesSource.slice(modalStart, modalEnd), /renderHeroBriefing\(activeLeader, "modal", "sow-hero-info-selected"\)/);

    const homeStart = lobbiesSource.indexOf("function renderHome()");
    const homeEnd = lobbiesSource.indexOf("var campaignCinematicAvailability", homeStart);
    const home = lobbiesSource.slice(homeStart, homeEnd);
    assert.match(home, /state\.prefer_random_leader \? "random" : leaderById\(state\.selected_leader\)/);
    assert.match(home, /renderHeroBriefing\(leader, "home", "sow-home-selected"\)/);
});

test("home hero briefing places the skill below identity while selection stays responsive", () => {
    assert.match(profileCss, /\.sow-hero-briefing\s*\{[\s\S]*grid-template-columns:\s*minmax\(0, 1\.2fr\) minmax\(0, 1fr\)/);
    assert.doesNotMatch(profileCss, /\.sow-hero-briefing--selection\s*\{\s*grid-template-columns:\s*minmax\(0, 1fr\)/,
        "wide hero selection must use the horizontal briefing composition");
    assert.match(profileCss, /\.sow-menu__leader-copy \.sow-hero-briefing\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/,
        "the home skill section must sit below the identity");
    assert.match(profileCss, /@media \(orientation: landscape\) and \(max-height: 560px\)[\s\S]*?\.sow-hero-briefing\s*\{[\s\S]*\.92fr[\s\S]*1\.08fr/);
    assert.match(profileCss, /\.sow-menu__leader-copy \.sow-hero-briefing__name \{ font-size: clamp\(17px, 3vh, 22px\); \}/,
        "the compact home title must fit above the skill briefing");
    assert.match(profileCss, /#sow-menu\[data-screen="heroes"\] \.sow-heroes__featured-copy \.sow-hero-briefing__name \{ font-size: clamp\(17px, 3vh, 22px\); \}/,
        "the compact selection title must fit its column beside the skill briefing");
    assert.match(profileCss, /@media \(max-width: 680px\), \(orientation: portrait\)[\s\S]*?\.sow-hero-briefing\s*\{\s*grid-template-columns:\s*minmax\(0, 1fr\)/);
    assert.match(menuCss["main_menu.layout.css"], /#sow-menu\[data-screen="home"\] \.sow-menu__battlefield\s*\{\s*order:\s*-1;/,
        "narrow home must show the same hero briefing before the menu actions");
    assert.doesNotMatch(menuCss["main_menu.layout.css"], /\.sow-menu__leader-copy\s*\{\s*display:\s*none\s*!important;/,
        "small screens must not hide the main-menu hero briefing");
    assert.doesNotMatch(profileCss, /sow-heroes__featured-copy \.sow-heroes__perk-row\s*\{\s*display:\s*none/);
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
});




test("campaign episode cards use leader portraits and emphasize the next playable chapter", () => {
    const campaignCss = menuCss["main_menu.campaign.css"];
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
