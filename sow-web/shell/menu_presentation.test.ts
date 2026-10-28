import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";
import { vm } from "./typescript-test-support.ts";
const shell = __dirname;
const coreSource = fs.readFileSync(path.join(shell, "main_menu.core.ts"), "utf8");
const leaderArtSlugStart = coreSource.indexOf("export function leaderArtSlug(");
const leaderArtSlugEnd = coreSource.indexOf("\n}", leaderArtSlugStart);
if (leaderArtSlugStart < 0 || leaderArtSlugEnd <= leaderArtSlugStart)
    throw new Error("main_menu.core.ts must export leaderArtSlug()");
const leaderArtSlug = vm.runInNewContext(
    coreSource.slice(leaderArtSlugStart, leaderArtSlugEnd + 2).replace(/\bexport\s+/g, "") +
        "\nleaderArtSlug;",
);
const shellSource = fs.readFileSync(path.join(shell, "main_menu.shell.ts"), "utf8");
const surfacesSource = fs.readFileSync(path.join(shell, "main_menu.surfaces.ts"), "utf8");
const heroesSource = fs.readFileSync(path.join(shell, "main_menu.heroes.ts"), "utf8");
const lobbiesSource = fs.readFileSync(path.join(shell, "main_menu.lobbies.ts"), "utf8");
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.ts"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const menuCssFiles = [
    "main_menu.base.css",
    "main_menu.layout.css",
    "main_menu.lobbies.css",
    "main_menu.create.css",
    "main_menu.queue.css",
    "main_menu.settings.css",
    "main_menu.auth.css",
    "main_menu.campaign.css",
];
const menuCss = Object.fromEntries(
    menuCssFiles.map((file) => [file, fs.readFileSync(path.join(shell, file), "utf8")]),
);
const profileCss = fs.readFileSync(path.join(shell, "main_menu.profile.css"), "utf8");
const profileSource = fs.readFileSync(path.join(shell, "main_menu.profile.ts"), "utf8");
const serverSource = fs.readFileSync(path.join(shell, "../../sow-data/src/main.rs"), "utf8");
const privacyPage = fs.readFileSync(path.join(shell, "../site/privacy/index.html"), "utf8");
const pokiSource = fs.readFileSync(path.join(shell, "main_menu.poki.ts"), "utf8");
const jestSource = fs.readFileSync(path.join(shell, "main_menu.jest.ts"), "utf8");
const siteChrome = fs.readFileSync(path.join(shell, "../../sow-web/site/site-chrome.ts"), "utf8");
const siteDropdown = fs.readFileSync(path.join(shell, "sow-dropdown.ts"), "utf8");
const campaignModule = fs.readFileSync(path.join(shell, "../../sow-client/src/campaign/mod.rs"), "utf8");

function renderSettingsBody(source, endMarker) {
    const start = source.indexOf("function renderSettings()");
    const end = source.indexOf(endMarker, start);
    assert.notEqual(start, -1);
    assert.notEqual(end, -1);
    return source.slice(start, end);
}

test("mobile menu renders its language selector after locale bootstrap", async () => {
    const controls = Array.from({ length: 2 }, () => ({
        className: "",
        innerHTML: "",
        querySelector: () => null,
    }));
    const menu = { classList: { toggle() {} }, setAttribute() {}, querySelectorAll: () => [] };
    const menuToggle = { setAttribute() {}, addEventListener() {} };
    const document = {
        documentElement: { dataset: {} },
        querySelectorAll: (selector) => (selector === "[data-locale-select]" ? controls : []),
        querySelector: (selector) =>
            selector === "#mobile-menu" ? menu : selector === "[data-menu-toggle]" ? menuToggle : null,
        addEventListener() {},
    };
    const window = {
        SOW_I18N_READY: Promise.resolve(),
        SOW_t: (key) => key,
        SOW_getLocale: () => "en",
        SOW_getLocaleTag: (locale) => locale,
        SOW_getSupportedLocales: () => ["en", "es"],
        SOW_getLocaleLabel: (locale) => locale,
        addEventListener() {},
    };
    const context = { document, window };
    vm.runInNewContext(siteDropdown, context);
    vm.runInNewContext(siteChrome, context);
    await window.SOW_I18N_READY;
    await Promise.resolve();
    assert.match(controls[1].innerHTML, /sow-control-dropdown__trigger/);
});

test("profile presents verified history, career commanders, localized achievements, and a paged global board", () => {
    const matchRowStart = profileSource.indexOf("function profileMatchRow");
    const matchRowEnd = profileSource.indexOf("function profileHeaderMarkup", matchRowStart);
    assert.ok(matchRowStart >= 0 && matchRowEnd > matchRowStart);
    const matchRow = profileSource.slice(matchRowStart, matchRowEnd);
    assert.match(matchRow, /match\.verified[\s\S]*profile\.match_pending[\s\S]*profile\.match_unverified/);
    assert.match(matchRow, /const verified = !!match\.verified;/);
    assert.match(
        matchRow,
        /const result = verified[\s\S]*?SOW_t\("profile\.win"\)[\s\S]*?SOW_t\("profile\.loss"\)/,
    );
});

test("profile banner always uses landscape leader art", () => {
    const start = coreSource.indexOf("export function profileHeroImage(");
    const end = coreSource.indexOf("export function avatarImage(", start);
    assert.ok(start >= 0 && end > start);
    const context = {
        state: null,
        window: {},
        heroImage: (leaderId, variant) =>
            `${String(leaderId).toLowerCase()}:${variant || "viewport-default"}`,
    };
    vm.runInNewContext(
        coreSource.slice(start, end).replace(/\bexport\s+/g, "") +
            "; this.profileHeroImage = profileHeroImage;",
        context,
    );
    assert.equal(context.profileHeroImage("Boudica"), "boudica:desktop");
});

test("the main-menu art and hero briefing both honor Random", () => {
    const start = coreSource.indexOf("export function heroImage(");
    const end = coreSource.indexOf("export function profileHeroImage(", start);
    assert.ok(start >= 0 && end > start);
    const prepared = [];
    const context = {
        menuUi: {
            state: {
                prefer_random_leader: true,
                phase: "MainMenu",
                loader_job: "Boot",
                selected_leader: "Caesar",
            },
        },
        state: {
            prefer_random_leader: true,
            phase: "MainMenu",
            loader_job: "Boot",
            selected_leader: "Caesar",
        },
        window: { SOW_prepareLeaderArt: (...args) => prepared.push(args) },
        leaderById: (id) => ({ slug: String(id).toLowerCase() }),
        leaderArtUrl: (slug, variant) => slug + ":" + (variant || "viewport-default"),
        currentLeaderSelection: () => ({ selected_leader: "Caesar", prefer_random_leader: true }),
        prepareLeaderArt() {},
    };
    vm.runInNewContext(
        coreSource.slice(start, end).replace(/\bexport\s+/g, "") + "; this.heroImage = heroImage;",
        context,
    );
    assert.equal(context.heroImage(), "random:viewport-default");
    assert.deepEqual(prepared, [["random", "low"]]);
});

test("confirmed leader choice stays current until the published state catches up", () => {
    const selectionStart = coreSource.indexOf("function normalizeLeaderSelectionId(");
    const selectionEnd = coreSource.indexOf("function asset(", selectionStart);
    const artStart = coreSource.indexOf("export function heroImage(");
    const artEnd = coreSource.indexOf("export function lobbyThumb(", artStart);
    assert.ok(selectionStart >= 0 && selectionEnd > selectionStart);
    assert.ok(artStart >= 0 && artEnd > artStart);

    const initialState = {
        account_id: "account-a",
        selected_leader: "Boudica",
        prefer_random_leader: false,
        phase: "MainMenu",
        loader_job: "Idle",
    };
    const context = {
        state: initialState,
        menuUi: { state: initialState },
        pendingLeaderSelection: null,
        asset: (path) => path,
        leaderById: (id) => ({ slug: String(id || "caesar").toLowerCase() }),
        leaderArtSlug,
        leaderArtUrl: (slug) => "hero:" + slug,
        prepareLeaderArt() {},
        window: { SOW_prepareLeaderArt() {} },
    };
    vm.runInNewContext(
        (coreSource.slice(selectionStart, selectionEnd) + coreSource.slice(artStart, artEnd)).replace(
            /\bexport\s+/g,
            "",
        ) +
            "this.currentLeaderSelection = currentLeaderSelection;" +
            "this.stageLeaderSelection = stageLeaderSelection;" +
            "this.reconcileLeaderSelection = reconcileLeaderSelection;" +
            "this.leaderSelectionMatches = leaderSelectionMatches;" +
            "this.avatarImage = avatarImage; this.heroImage = heroImage;",
        context,
    );

    context.stageLeaderSelection(true, null);
    assert.equal(context.currentLeaderSelection().prefer_random_leader, true);
    assert.equal(context.currentLeaderSelection().selected_leader, null);
    assert.equal(
        context.leaderSelectionMatches(
            { selected_leader: "SunTzu", prefer_random_leader: false },
            false,
            "sun_tzu",
        ),
        true,
    );
    assert.equal(context.avatarImage(), "gameplay/avatars/random.webp");
    assert.equal(context.heroImage(), "hero:random");
    context.reconcileLeaderSelection(initialState);
    assert.equal(
        context.pendingLeaderSelection.prefer_random_leader,
        true,
        "the old concrete state cannot clear a pending Random choice",
    );

    const randomState = { ...initialState, prefer_random_leader: true };
    assert.equal(
        randomState.selected_leader,
        "Boudica",
        "Random keeps the concrete preference as a hidden server-side backup",
    );
    context.state = randomState;
    context.menuUi.state = randomState;
    context.reconcileLeaderSelection(randomState);
    assert.equal(context.pendingLeaderSelection, null);
    assert.equal(context.currentLeaderSelection().prefer_random_leader, true);

    context.stageLeaderSelection(false, "napoleon");
    assert.equal(context.currentLeaderSelection().selected_leader, "napoleon");
    assert.equal(context.currentLeaderSelection().prefer_random_leader, false);
    assert.equal(context.avatarImage(), "gameplay/avatars/napoleon.webp");
    assert.equal(context.heroImage(), "hero:napoleon");
    context.reconcileLeaderSelection(randomState);
    assert.equal(
        context.pendingLeaderSelection.selected_leader,
        "napoleon",
        "the old Random state cannot clear a pending concrete choice",
    );

    const concreteState = { ...randomState, selected_leader: "Napoleon", prefer_random_leader: false };
    context.state = concreteState;
    context.menuUi.state = concreteState;
    context.reconcileLeaderSelection(concreteState);
    assert.equal(context.pendingLeaderSelection, null);
    assert.equal(context.currentLeaderSelection().selected_leader, "Napoleon");
    assert.equal(context.currentLeaderSelection().prefer_random_leader, false);

    context.stageLeaderSelection(false, "caesar");
    const switchedAccount = { ...concreteState, account_id: "account-b", selected_leader: "SunTzu" };
    context.state = switchedAccount;
    context.menuUi.state = switchedAccount;
    context.reconcileLeaderSelection(switchedAccount);
    assert.equal(
        context.pendingLeaderSelection,
        null,
        "a pending choice from one account cannot leak into another",
    );
    assert.equal(context.currentLeaderSelection().selected_leader, "SunTzu");
});

test("leader confirmation and reopening use the staged choice, not the previous snapshot", () => {
    const randomStart = shellSource.indexOf('if (command === "confirm_random_leader")');
    const concreteStart = shellSource.indexOf('if (command === "confirm_leader")', randomStart);
    const concreteEnd = shellSource.indexOf("/* POKI_SHARED_STORE_UNLOCK_BEGIN */", concreteStart);
    const pickerStart = shellSource.indexOf('if (command === "open_leader_picker")');
    const navStart = shellSource.indexOf('if (command === "main_nav")');
    const navEnd = shellSource.indexOf('if (command === "open_leader_picker")', navStart);
    assert.ok(randomStart >= 0 && concreteStart > randomStart && concreteEnd > concreteStart);
    assert.ok(pickerStart >= 0 && navStart >= 0 && navEnd > navStart);

    const randomConfirm = shellSource.slice(randomStart, concreteStart);
    const concreteConfirm = shellSource.slice(concreteStart, concreteEnd);
    const pickerOpen = shellSource.slice(
        pickerStart,
        shellSource.indexOf('if (command === "toggle_dropdown")', pickerStart),
    );
    const heroesNavigation = shellSource.slice(navStart, navEnd);
    assert.match(randomConfirm, /stageLeaderSelection\(true, null\);\s*send\("set_random_leader"\)/);
    assert.doesNotMatch(randomConfirm, /leader_id/, "Random must not carry a previous concrete leader ID");
    assert.match(
        concreteConfirm,
        /stageLeaderSelection\(false, leaderId\);\s*send\("set_leader", \{ leader_id: leaderId \}\)/,
    );
    assert.match(randomConfirm, /menuUi\.tempSelectedLeader = null;\s*menuUi\.tempSelectedRandom = false;/);
    assert.match(concreteConfirm, /menuUi\.tempSelectedLeader = null;\s*menuUi\.tempSelectedRandom = false;/);
    assert.match(pickerOpen, /currentLeaderSelection\(\)/);
    assert.match(
        heroesNavigation,
        /menuUi\.tempSelectedLeader = null;\s*menuUi\.tempSelectedRandom = false;/,
    );
    assert.match(heroesNavigation, /currentLeaderSelection\(\)/);
});

test("header keeps only the nickname and sizes it to the available space", () => {
    const topbarStart = shellSource.indexOf("function renderTopbar()");
    const topbarEnd = shellSource.indexOf("\nfunction ", topbarStart + 1);
    assert.ok(topbarStart >= 0 && topbarEnd > topbarStart);
    const topbar = shellSource.slice(topbarStart, topbarEnd);
    assert.match(topbar, /data-role='display-name'/);
    assert.doesNotMatch(topbar, /selected_leader_name|selected_civilization|kingdom/i);

    const baseCss = menuCss["main_menu.base.css"];
    const layoutCss = menuCss["main_menu.layout.css"];
    assert.match(baseCss, /\.sow-menu__profile\s*\{[^}]*flex:\s*1 1 auto;[^}]*min-width:\s*0;/);
    assert.match(
        baseCss,
        /#sow-menu \.sow-menu__profile input\s*\{[^}]*width:\s*100%;[^}]*font-size:\s*clamp\(16px, 1\.8vw, 24px\)/,
    );
    assert.match(
        layoutCss,
        /\.sow-menu__profile input\s*\{[^}]*width:\s*100%;[^}]*font-size:\s*clamp\(15px, 4vw, 21px\)/,
    );
});

test("desktop navigation stacks battle, heroes, profile, and shop", () => {
    const layout = menuCss["main_menu.layout.css"];
    const start = layout.indexOf("@media (min-width: 681px) and (orientation: landscape) {");
    const end = layout.indexOf(
        "@media (min-width: 681px) and (orientation: landscape) and (max-height: 560px)",
        start,
    );
    assert.ok(start >= 0 && end > start);
    const orders = Array.from(
        layout
            .slice(start, end)
            .matchAll(
                /\.sow-menu__main-nav-item\[data-nav-screen="(battle|heroes|profile|store)"\]\s*\{\s*order:\s*(\d+);\s*\}/g,
            ),
        (match) => [match[1], Number(match[2])],
    );
    assert.deepEqual(orders, [
        ["battle", 1],
        ["heroes", 2],
        ["profile", 3],
        ["store", 4],
    ]);
});

test("match history row shows the leader and verified KDA while provisional exits never become losses", () => {
    const rowStart = profileSource.indexOf("function profileMatchRow");
    const rowEnd = profileSource.indexOf("function profileHeaderMarkup", rowStart);
    const context = {
        SOW_t: (key, values) =>
            key === "profile.kda_value" ? `${values.kills}/${values.deaths}/${values.assists}` : key,
        asset: (path) => `/assets/${path}`,
        esc: (value) => String(value),
        formatMapName: () => "Britain",
        leaderById: (id) => ({ slug: id.toLowerCase() }),
        leaderArtSlug,
        leaderDisplayName: () => "Boudica",
        profileMatchDate: () => "Today",
        profileMatchDuration: () => "12m 4s",
    };
    vm.runInNewContext(
        `${profileSource.slice(rowStart, rowEnd)}; this.profileMatchRow = profileMatchRow;`,
        context,
    );

    const verified = context.profileMatchRow({
        match_id: "verified-match",
        verified: true,
        won: false,
        leader: "Boudica",
        kills: 4,
        deaths: 2,
        assists: 1,
    });
    assert.match(verified, /Boudica/);
    assert.match(verified, /4\/2\/1/);
    assert.match(verified, /profile\.loss/);

    const provisional = context.profileMatchRow({
        match_id: "provisional-match",
        verified: false,
        provisional: true,
        leader: "Boudica",
        kills: 0,
        deaths: 0,
        assists: 0,
    });
    assert.match(provisional, /profile\.match_pending/);
    assert.doesNotMatch(provisional, /profile\.loss/);
});

test("settings panel keeps only useful controls and real account state", () => {
    const settings = [
        renderSettingsBody(surfacesSource, "export function renderAuthModal"),
        renderSettingsBody(pokiSource, "function pokiLicenseUrl"),
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
        assert.match(body, /renderSettingsPrivacySection/);
        assert.match(body, /account-value/);
        assert.match(body, /settings-account/);
        assert.doesNotMatch(
            body,
            /system_configuration|menu\.done|master_audio|settings-mute|account-row|WOU-ID|\+1/,
        );
    }
    assert.match(
        renderSettingsBody(jestSource, "function renderMenuAttribution"),
        /renderSettingsPrivacySection/,
    );
});

test("profile stays full-width and account erasure requires settings confirmation and server success", () => {
    assert.match(profileCss, /\.sow-profile__page\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/);
    assert.doesNotMatch(
        profileSource,
        /profile\.find_player|data-form='profile-search'|data-command='request_delete'|data-command='confirm_delete'/,
    );

    const requestDelete = shellSource.slice(
        shellSource.indexOf('if (command === "request_delete")'),
        shellSource.indexOf('if (command === "cancel_delete")'),
    );
    const confirmDelete = shellSource.slice(
        shellSource.indexOf('if (command === "confirm_delete")'),
        shellSource.indexOf('if (command === "open_leader_picker")'),
    );
    assert.match(requestDelete, /!menuUi\.settingsOpen/);
    assert.match(confirmDelete, /!menuUi\.settingsOpen \|\| !menuUi\.deleteArmed/);
    assert.match(confirmDelete, /profile\/anonymous\/delete/);
    assert.match(confirmDelete, /data\.deleted !== true/);
    assert.ok(
        confirmDelete.indexOf("data.deleted !== true") < confirmDelete.indexOf("localStorage.removeItem"),
    );
    assert.match(privacyPage, /Settings → Privacy → Delete my account/);

    const selfDelete = serverSource.slice(
        serverSource.indexOf("async fn handle_self_delete"),
        serverSource.indexOf("/// /profile/anonymous response"),
    );
    assert.match(selfDelete, /verify_self_service/);
    assert.match(selfDelete, /state\.db\.delete_account\(account_id\)/);
    assert.match(selfDelete, /deleted: true/);
});

test("Opus theme preserves its crossfade and resumes from ordinary trusted clicks", async () => {
    const start = shellSource.indexOf("const menuMusic");
    const end = shellSource.indexOf("function renderTopbar()", start);
    assert.ok(start >= 0 && end > start, "shared shell has no menu music lifecycle");

    function createHarness(assetsUrl = "https://cdn.test/assets/") {
        const audios = [];
        const mediaMetadata = [];
        const frames = new Map();
        const logs = [];
        const windowListeners = new Map();
        const documentListeners = new Map();
        const body = {
            children: [],
            appendChild(element) {
                this.children.push(element);
                element.parentNode = this;
                return element;
            },
        };
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
            addEventListener(type, callback) {
                (this.listeners[type] ||= []).push(callback);
            }
            dispatch(type) {
                (this.listeners[type] || []).forEach((callback) => callback());
            }
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
            pause() {
                this.pauseCalls += 1;
                this.paused = true;
            }
            load() {
                this.loadCalls += 1;
            }
            set src(value) {
                this._src = value;
            }
            get src() {
                return this._src;
            }
        }
        class FakeMediaMetadata {
            constructor(value) {
                Object.assign(this, value);
                mediaMetadata.push(this);
            }
        }
        const mediaSession = { metadata: null };
        const window = {
            Audio: FakeAudio,
            MediaMetadata: FakeMediaMetadata,
            URL,
            navigator: { mediaSession },
            SOW_ASSETS_URL: assetsUrl,
            SOW_BUILD_TS: "build-42",
            SOW_t: (key) => key,
            addEventListener(type, callback, options) {
                (windowListeners.get(type) || windowListeners.set(type, []).get(type)).push({
                    callback,
                    once: Boolean(options && options.once),
                });
            },
            dispatchEvent(event) {
                const listeners = windowListeners.get(event.type) || [];
                listeners.slice().forEach((listener) => {
                    listener.callback(event);
                    if (listener.once) listeners.splice(listeners.indexOf(listener), 1);
                });
            },
            requestAnimationFrame(callback) {
                const id = ++frameId;
                frames.set(id, callback);
                return id;
            },
            cancelAnimationFrame(id) {
                frames.delete(id);
            },
        };
        const root = {
            contains(target) {
                return Boolean(target && target.insideMenu);
            },
        };
        const document = {
            body,
            baseURI: "https://cdn.test/play/",
            querySelector(selector) {
                return selector === 'meta[name="sow-media-artwork"]' ? { content: "../icon-192.png" } : null;
            },
            addEventListener(type, callback, capture) {
                (documentListeners.get(type) || documentListeners.set(type, []).get(type)).push({
                    callback,
                    capture: Boolean(capture),
                });
            },
            dispatchClick(target, isTrusted = true) {
                const event = {
                    target,
                    isTrusted,
                    defaultPrevented: false,
                    preventDefault() {
                        this.defaultPrevented = true;
                    },
                };
                (documentListeners.get("click") || [])
                    .filter((listener) => listener.capture)
                    .forEach((listener) => listener.callback(event));
                return event;
            },
        };
        const context = {
            window,
            document,
            root,
            Node: Object,
            console: {
                error: (...args) => logs.push(["error", ...args]),
                warn: (...args) => logs.push(["warn", ...args]),
            },
        };
        vm.runInNewContext(
            shellSource.slice(start, end) +
                "this.musicApi = { syncMenuMusic, menuMusic, menuMusicFinishLoaderCycle, menuMusicSetCinematicActive };",
            context,
        );
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
            },
        };
    }
    const flushPlay = async () => {
        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();
    };
    const state = (
        phase,
        volume = 0.8,
        muted = false,
        loader_job = "Boot",
        loader_cycle_id = 0,
        waiting = false,
    ) => ({
        phase,
        loader_job,
        loader_cycle_id,
        waiting,
        settings: { music_volume: volume, mute_all: muted },
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
    assert.deepEqual(
        Array.from(h.mediaMetadata[0].artwork, (art) => ({
            src: art.src,
            sizes: art.sizes,
            type: art.type,
        })),
        [{ src: "https://cdn.test/icon-192.png", sizes: "192x192", type: "image/png" }],
    );
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
    assert.equal(
        first.currentTime,
        55,
        "menu, tutorial, and gameplay state changes keep the current position",
    );
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
    assert.equal(
        h.api.menuMusic.crossfade,
        null,
        "late media events cannot restart the loop during a loader",
    );
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
    assert.equal(
        h.api.menuMusicFinishLoaderCycle(2),
        false,
        "an old loader event cannot reset the current track",
    );
    assert.equal(
        h.api.menuMusicFinishLoaderCycle(3),
        false,
        "a duplicate completion cannot reset the current track",
    );
    assert.equal(first.currentTime, 23);

    const returning = createHarness();
    returning.api.syncMenuMusic(state("MainMenu"), false);
    assert.equal(
        returning.api.menuMusic.decks[0].playCalls,
        0,
        "Boot stays silent while its loader is visible",
    );
    returning.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    assert.equal(
        returning.api.menuMusic.decks[0].paused,
        false,
        "a returning account starts music on the menu route",
    );

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
    assert.ok(
        Math.abs(first.volume - 0.15) < 0.001 && Math.abs(second.volume - 0.15) < 0.001,
        "the loop crossfade reaches an even mix halfway through three seconds",
    );
    h.frame(5000);
    assert.equal(h.api.menuMusic.active, 1);
    assert.equal(first.paused, true);
    assert.equal(first.currentTime, 0);

    const blocked = createHarness();
    blocked.api.syncMenuMusic(state("MainMenu"), false);
    blocked.audios[0].nextPlayError = Object.assign(new Error("gesture required"), {
        name: "NotAllowedError",
    });
    blocked.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    assert.equal(
        blocked.api.menuMusic.playBlocked,
        true,
        "the browser rejection is remembered for the next user gesture",
    );
    assert.equal(
        blocked.api.menuMusic.unlockButton,
        undefined,
        "blocked autoplay creates no recovery button",
    );
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
    assert.equal(
        resumedDeck.playCalls,
        callsBeforeSleep + 1,
        "a normal game click resumes music paused by the platform",
    );
    assert.equal(resumedDeck.paused, false);
    assert.equal(wakeClick.defaultPrevented, false, "audio recovery leaves the game click intact");

    const unsupported = createHarness();
    unsupported.api.syncMenuMusic(state("MainMenu"), true);
    await flushPlay();
    const failedOpusDeck = unsupported.api.menuMusic.decks[0];
    failedOpusDeck.dispatch("error");
    assert.equal(
        failedOpusDeck.src,
        "https://cdn.test/assets/gameplay/music/shadows-of-war.webm?v=build-42",
        "an unsupported Opus file gets no alternate format",
    );
    assert.ok(
        unsupported.logs.some((entry) => entry[0] === "error"),
        "Opus failure is visible in the console",
    );

    const withoutMediaSession = createHarness();
    delete withoutMediaSession.window.navigator;
    withoutMediaSession.api.syncMenuMusic(state("MainMenu"), true);
    withoutMediaSession.window.dispatchEvent({ type: "sow:loader-ready" });
    await flushPlay();
    assert.equal(
        withoutMediaSession.api.menuMusic.decks[0].paused,
        false,
        "missing platform metadata support does not block music playback",
    );

    const native = createHarness("../assets");
    native.api.syncMenuMusic(state("MainMenu"), false);
    assert.equal(
        native.api.menuMusic.decks[0].src,
        "../assets/gameplay/music/shadows-of-war.webm?v=build-42",
        "Tauri resolves the same music through its local SOW_ASSETS_URL",
    );
});

test("music volume no longer controls SFX and music assets reach every shared-shell package", () => {
    const musicCommand = webMenu.match(/WebMenuCommand::SetMusicVolume \{ value \} => \{[\s\S]*?\n {16}\}/);
    assert.ok(musicCommand);
    assert.match(musicCommand[0], /settings_state\.music_volume = volume/);
    assert.doesNotMatch(musicCommand[0], /set_master_volume/);
});

test("leader display names use the catalog translation keys for every leader", () => {
    const start = coreSource.indexOf("function leaderTranslationSlug(");
    const end = coreSource.indexOf("function heroBriefingLeader(", start);
    assert.ok(start >= 0 && end > start);
    const requestedKeys = [];
    const context = {
        SOW_t(key) {
            requestedKeys.push(key);
            return "translated:" + key;
        },
    };
    vm.runInNewContext(
        coreSource.slice(start, end).replace(/\bexport\s+/g, "") +
            "this.leaderDisplayName = leaderDisplayName; this.leaderHistoricalName = leaderHistoricalName;",
        context,
    );

    const slugs = [
        "caesar",
        "cleopatra",
        "ragnar",
        "sun_tzu",
        "alexander",
        "genghis_khan",
        "richard_the_lionheart",
        "vercingetorix",
        "boudica",
        "lady_six_sky",
        "leonidas",
        "napoleon",
    ];
    const expectedSlugs = slugs;
    for (let i = 0; i < slugs.length; i++) {
        const expectedKey = "heroes.leader_" + expectedSlugs[i] + "_name";
        assert.equal(
            context.leaderDisplayName({ slug: slugs[i], name: "Fallback" }),
            "translated:" + expectedKey,
        );
        assert.equal(requestedKeys[i], expectedKey);
    }
    assert.equal(
        context.leaderHistoricalName({ slug: "richard_the_lionheart", name: "Richard the Lionheart" }),
        "translated:heroes.leader_richard_the_lionheart_historical",
    );
});

test("one hero briefing renders identity and the named ability without exposing missing translations", () => {
    const start = coreSource.indexOf("function heroBriefingLeader");
    const end = coreSource.indexOf("function mapInfo(", start);
    assert.ok(start >= 0 && end > start);
    const translations = {
        "heroes.leader_amina_de_zazzau_name": "Amina of Zazzau",
        "heroes.leader_amina_de_zazzau_civilization": "Kingdom of Zazzau",
        "site.leader_amina_de_zazzau_ability": "Walls of Zazzau",
        "site.leader_amina_de_zazzau_description": "Enemies need 50% more effort to capture your cities.",
        "heroes.random_leader": "Random",
        "heroes.random_leader_description": "A surprise hero from your collection.",
    };
    const context = {
        state: { leaders: [] },
        window: {},
        document: { addEventListener() {} },
        SOW_t: (key) => translations[key] || `[${key}]`,
        leaderArtSlug,
        esc: (value) =>
            String(value)
                .replace(/&/g, "&amp;")
                .replace(/</g, "&lt;")
                .replace(/>/g, "&gt;")
                .replace(/"/g, "&quot;")
                .replace(/'/g, "&#39;"),
        asset: (path) => "/assets/" + path,
    };
    vm.runInNewContext(
        coreSource.slice(start, end).replace(/\bexport\s+/g, "") +
            "; this.renderHeroBriefing = renderHeroBriefing; this.hideBrokenHeroBriefingIcon = hideBrokenHeroBriefingIcon;",
        context,
    );

    const amina = context.renderHeroBriefing(
        {
            id: "AminaDeZazzau",
            slug: "amina_de_zazzau",
            name: "Amina of Zazzau",
            civilization_key: "heroes.leader_amina_de_zazzau_civilization",
            perk_key: "site.leader_amina_de_zazzau_description",
        },
        "selection",
        "sow-heroes-selected",
    );
    assert.match(
        amina,
        /Kingdom of Zazzau[\s\S]*Amina of Zazzau[\s\S]*Walls of Zazzau[\s\S]*Enemies need 50% more effort/,
    );
    assert.equal(
        (amina.match(/<h2\b[^>]*class='sow-hero-briefing__name'[^>]*>Amina of Zazzau<\/h2>/g) || []).length,
        1,
        "Amina's visible title must not repeat her name",
    );
    assert.match(amina, /sow-hero-briefing__icon[\s\S]*gameplay\/icons\/abilities\/amina_de_zazzau\.webp/);
    assert.doesNotMatch(amina, /\[site\.|site\.leader_/);

    const random = context.renderHeroBriefing("random", "home");
    assert.match(random, /Random[\s\S]*A surprise hero from your collection/);
    assert.doesNotMatch(random, /sow-hero-briefing__icon|site\.leader_|Walls of Zazzau/);

    const unavailable = context.renderHeroBriefing(
        { id: "Caesar", slug: "caesar", name: "Caesar" },
        "loader",
    );
    assert.doesNotMatch(unavailable, /\[site\.|site\.leader_/);
    assert.doesNotMatch(unavailable, /sow-hero-briefing__icon/);

    const iconSlot = {
        removed: false,
        classList: { contains: (value) => value === "sow-hero-briefing__icon-slot" },
        remove() {
            this.removed = true;
        },
    };
    const image = {
        parentNode: iconSlot,
        classList: { contains: (value) => value === "sow-hero-briefing__icon" },
    };
    context.hideBrokenHeroBriefingIcon({ target: image });
    assert.equal(
        iconSlot.removed,
        true,
        "a failed ability image removes its frame instead of leaving a broken-image box",
    );
});

test("weekly free rotation is a splash-only status", () => {
    const cardStart = heroesSource.indexOf("function renderHeroesCard");
    const cardEnd = heroesSource.indexOf("function heroesRoster", cardStart);
    assert.ok(cardStart >= 0 && cardEnd > cardStart);
    const predicateStart = heroesSource.indexOf("function isWeeklyFreeRotation");
    const predicateEnd = heroesSource.indexOf("function renderHeroesCard", predicateStart);
    assert.ok(predicateStart >= 0 && predicateEnd > predicateStart);
    const context = {};
    vm.runInNewContext(
        heroesSource.slice(predicateStart, predicateEnd).replace(/\bexport\s+/g, "") +
            "this.isWeeklyFreeRotation = isWeeklyFreeRotation;",
        context,
    );
    assert.equal(
        context.isWeeklyFreeRotation({ available: false, free_rotation: true, owned: false }),
        false,
    );
    assert.equal(context.isWeeklyFreeRotation({ available: true, free_rotation: true, owned: false }), true);
});

test("weekly rotation countdown replaces its label, observes the four-day window, and updates only its own nodes", () => {
    const helperStart = heroesSource.indexOf("function isWeeklyFreeRotation");
    const helperEnd = heroesSource.indexOf("// OWNER DECISION", helperStart);
    assert.ok(helperStart >= 0 && helperEnd > helperStart);
    const countdownNode = { textContent: "", hidden: true };
    const statusLabelNode = { textContent: "", hidden: false };
    const statusNode = { hidden: true, querySelector: () => statusLabelNode };
    const weeklyLeader = { id: "free", available: true, free_rotation: true, owned: false };
    const ownedLeader = { id: "owned", available: true, free_rotation: false, owned: true };
    const leaders = { free: weeklyLeader, owned: ownedLeader };
    let locale = "en";
    let nowMs = 1_700_000_000_000;
    let screen = "heroes";
    let nextTimerId = 1;
    const timers = new Map();
    const cleared = [];
    let renders = 0;
    const context = {
        SOW_t(key, values) {
            assert.equal(key, "heroes.rotation_available_for");
            return `${locale}: Available for ${values.days}d ${values.hours}h ${values.minutes}m ${values.seconds}s`;
        },
        Date: { now: () => nowMs },
        state: { phase: "MainMenu", selected_leader: "free", store: { rotation_ends_at: 0 } },
        menuUi: {
            state: null,
            tempSelectedLeader: "",
            tempSelectedRandom: false,
        },
        tempSelectedLeader: "",
        tempSelectedRandom: false,
        leaderById: (id) => leaders[id],
        currentScreen: () => screen,
        root: {
            hidden: false,
            querySelector(selector) {
                return selector === "[data-hero-status]" ? statusNode : countdownNode;
            },
        },
        document: { visibilityState: "visible" },
        window: {
            setInterval(callback, delay) {
                assert.equal(delay, 1000);
                const id = nextTimerId++;
                timers.set(id, callback);
                return id;
            },
            clearInterval(id) {
                cleared.push(id);
                timers.delete(id);
            },
        },
        render() {
            renders++;
        },
    };
    context.menuUi.state = context.state;
    context.state.store.rotation_ends_at = nowMs / 1000 + 4 * 86400;
    vm.runInNewContext(
        "var HEROES_ROTATION_COUNTDOWN_SECS = 4 * 24 * 60 * 60; var heroesRotationCountdownInterval = null;\n" +
            heroesSource.slice(helperStart, helperEnd).replace(/\bexport\s+/g, "") +
            "this.heroesStatusLabel = heroesStatusLabel; this.rotationCountdownText = rotationCountdownText; this.syncHeroesRotationCountdown = syncHeroesRotationCountdown;",
        context,
    );

    assert.equal(
        context.heroesStatusLabel(weeklyLeader),
        "",
        "the weekly title is omitted when the countdown is shown",
    );
    context.syncHeroesRotationCountdown();
    assert.equal(timers.size, 1);
    assert.equal(countdownNode.textContent, "en: Available for 4d 00h 00m 00s");
    assert.equal(statusNode.hidden, false, "the countdown badge is visible without the weekly title");
    assert.equal(statusLabelNode.hidden, true, "the countdown always replaces the badge label");
    nowMs += 1000;
    [...timers.values()][0]();
    assert.equal(countdownNode.textContent, "en: Available for 3d 23h 59m 59s");
    assert.equal(renders, 0, "each tick writes to the countdown node without rendering the menu");

    screen = "profile";
    context.syncHeroesRotationCountdown();
    assert.equal(timers.size, 0, "the timer stops outside the heroes panel");
    assert.deepEqual(cleared, [1]);
    context.document.visibilityState = "hidden";
    screen = "heroes";
    context.syncHeroesRotationCountdown();
    assert.equal(timers.size, 0, "the timer stays stopped while the app is hidden");

    context.document.visibilityState = "visible";
    context.state.selected_leader = "owned";
    locale = "es";
    context.syncHeroesRotationCountdown();
    assert.equal(timers.size, 1, "the timer resumes when the heroes panel becomes visible again");
    assert.equal(countdownNode.textContent, "", "a newly selected non-rotation hero has no countdown");
    assert.equal(statusNode.hidden, true, "the empty countdown badge is hidden for other heroes");
    assert.equal(statusLabelNode.hidden, false, "the badge label is restored when there is no countdown");
    context.state.selected_leader = "free";
    [...timers.values()][0]();
    assert.equal(
        countdownNode.textContent,
        "es: Available for 3d 23h 59m 59s",
        "the countdown follows the selected hero and uses the current locale",
    );
    context.state.store.rotation_ends_at = nowMs / 1000;
    [...timers.values()][0]();
    assert.equal(countdownNode.textContent, "", "the countdown hides when the rotation ends");
    screen = "profile";
    context.syncHeroesRotationCountdown();
    assert.equal(timers.size, 0);

    const leader = { available: true, free_rotation: true, owned: false };
    const nowSeconds = nowMs / 1000;
    assert.equal(context.rotationCountdownText(leader, nowSeconds + 4 * 86400 + 1, nowSeconds), "");
    assert.equal(context.rotationCountdownText(leader, nowSeconds, nowSeconds), "");
    assert.equal(context.rotationCountdownText({ ...leader, owned: true }, nowSeconds + 60, nowSeconds), "");
});

test("weekly rotation countdown is compact and localized in all 15 locales", () => {
    const locales = [
        "en",
        "es",
        "fr",
        "de",
        "it",
        "tr",
        "zh-cn",
        "ja",
        "ko",
        "ar",
        "ru",
        "vi",
        "id",
        "fil",
        "pt-br",
    ];
    for (const locale of locales) {
        const catalog = fs.readFileSync(
            path.join(shell, `../../sow-i18n/strings/${locale}/web.toml`),
            "utf8",
        );
        const line = catalog.split("\n").find((entry) => entry.startsWith("rotation_available_for ="));
        assert.ok(line, `heroes.rotation_available_for must exist in ${locale}`);
        assert.match(
            line,
            /\{days\}\S+.*\{hours\}\S+.*\{minutes\}\S+.*\{seconds\}\S+/,
            `each localized number must touch its unit in ${locale}`,
        );
        assert.doesNotMatch(
            catalog,
            /^weekly_free_rotation\s*=/m,
            `the duplicate weekly title must not remain in ${locale}`,
        );
    }
    assert.doesNotMatch(heroesSource, /heroes\.weekly_free_rotation/);
    assert.match(heroesSource, /data-hero-status-label/);
    assert.match(heroesSource, /data-hero-rotation-countdown/);
    assert.match(shellSource, /function updateDynamic\(\) \{\s*syncHeroesRotationCountdown\(\);/);
    assert.match(
        shellSource,
        /document\.addEventListener\("visibilitychange", function \(\) \{\s*syncHeroesRotationCountdown\(\);/,
    );
    assert.match(coreSource, /rotation_ends_at:\s*state\.store\.rotation_ends_at/);
});

test("heroes preview never resolves the random option through leader i18n keys", () => {
    const previewStart = heroesSource.indexOf("function updateHeroesPreview");
    const previewEnd = heroesSource.indexOf("function renderHeroInfoModal", previewStart);
    assert.ok(previewStart >= 0 && previewEnd > previewStart);
    const previewBody = heroesSource.slice(previewStart, previewEnd);
    assert.match(
        previewBody,
        /image\.alt = randomSelected \? randomLeaderName\(\) : leaderDisplayName\(activeLeader\)/,
        "image alt must use randomLeaderName() when random is selected, or it reports heroes.leader_random_name missing",
    );
    assert.match(
        previewBody,
        /briefing\.innerHTML[\s\S]*?renderHeroBriefing\([\s\S]*?randomSelected \? "random" : activeLeader[\s\S]*?"selection"[\s\S]*?"sow-heroes-selected"/,
    );
    assert.doesNotMatch(previewBody, /leaderHistoricalName\(|sow-heroes__ability-icon/);
});

test("skill icons stay beside full hero details and out of compact avatar cards", () => {
    const cardStart = heroesSource.indexOf("function renderHeroesCard");
    const cardEnd = heroesSource.indexOf("function randomLeaderName", cardStart);
    const card = heroesSource.slice(cardStart, cardEnd);
    assert.doesNotMatch(card, /sow-heroes__ability-icon/);
    assert.doesNotMatch(card, /renderHeroBriefing/);
    const fullStart = heroesSource.indexOf("export function renderHeroes()");
    const full = heroesSource.slice(fullStart);
    assert.match(
        full,
        /data-hero-briefing[\s\S]*renderHeroBriefing\(activeRandom \? "random" : activeLeader, "selection", "sow-heroes-selected"\)/,
    );
    assert.doesNotMatch(full, /leaderHistoricalName\(/);
    const modalStart = heroesSource.indexOf("function renderHeroInfoModal");
    const modalEnd = heroesSource.indexOf("function resetDropdownPosition", modalStart);
    assert.match(
        heroesSource.slice(modalStart, modalEnd),
        /renderHeroBriefing\(activeLeader, "modal", "sow-hero-info-selected"\)/,
    );

    const homeStart = lobbiesSource.indexOf("function renderHome()");
    const homeEnd = lobbiesSource.indexOf("const campaignCinematicAvailability", homeStart);
    const home = lobbiesSource.slice(homeStart, homeEnd);
    assert.match(
        home,
        /menuUi\.state!\.prefer_random_leader \? "random" : leaderById\(menuUi\.state!\.selected_leader\)/,
    );
    assert.match(home, /renderHeroBriefing\(leader, "home", "sow-home-selected"\)/);
});

test("hero selection stacks the skill below identity and the home briefing stays behind controls and hides in portrait", () => {
    assert.match(
        profileCss,
        /\.sow-hero-briefing\s*\{[\s\S]*grid-template-columns:\s*minmax\(0, 1\.2fr\) minmax\(0, 1fr\)/,
    );
    assert.match(
        profileCss,
        /#sow-menu\[data-screen="heroes"\] \.sow-hero-briefing--selection\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/,
        "the full selection card must stack identity above the ability at every width",
    );
    assert.match(
        profileCss,
        /\.sow-menu__leader-copy \.sow-hero-briefing\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)[^}]*text-align:\s*end/,
        "home hero and skill copy must align to the right",
    );
    assert.match(
        profileCss,
        /@media \(orientation: landscape\)[\s\S]*?\.sow-menu__leader-copy \.sow-hero-briefing__ability\s*\{[^}]*flex-direction:\s*row-reverse/,
        "the horizontal home skill icon must sit at the right edge of its copy",
    );
    const compactStart = profileCss.indexOf(
        "@media (orientation: landscape) and (max-height: 560px)",
        profileCss.indexOf(".sow-heroes__featured"),
    );
    const compactEnd = profileCss.indexOf("\n@media ", compactStart + 1);
    const compact = profileCss.slice(compactStart, compactEnd);
    assert.ok(compactStart >= 0 && compactEnd > compactStart, "compact landscape rules must remain explicit");
    assert.match(
        compact,
        /\.sow-heroes__featured-copy\s*\{[^}]*padding:\s*14px/,
        "compact landscape reduces the featured-card padding",
    );
    assert.match(compact, /\.sow-hero-briefing__ability-name\s*\{\s*font-size:\s*10px/);
    assert.match(compact, /\.sow-hero-briefing__description\s*\{\s*font-size:\s*10px/);
    assert.match(compact, /\.sow-hero-briefing__icon-slot\s*\{\s*flex-basis:\s*32px;\s*width:\s*32px/);
    assert.doesNotMatch(
        compact,
        /\.sow-hero-briefing--selection\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*\.92fr/,
        "compact landscape must not put selection identity beside the skill",
    );
    assert.match(
        profileCss,
        /\.sow-heroes__confirm\s*\{\s*min-height:\s*44px/,
        "the compact card keeps the confirm action reachable",
    );
    assert.match(
        profileCss,
        /\.sow-menu__leader-copy\s+\.sow-hero-briefing__name\s*\{\s*font-size:\s*clamp\(17px,\s*3vh,\s*22px\);\s*\}/,
        "the compact home title must fit above the skill briefing",
    );
    assert.match(
        profileCss,
        /#sow-menu\[data-screen="heroes"\]\s+\.sow-heroes__featured-copy\s+\.sow-hero-briefing__name\s*\{\s*font-size:\s*clamp\(17px,\s*3vh,\s*22px\);\s*\}/,
        "the compact selection title must fit above the skill briefing",
    );
    assert.match(
        profileCss,
        /@media \(max-width: 680px\), \(orientation: portrait\)[\s\S]*?\.sow-hero-briefing\s*\{\s*grid-template-columns:\s*minmax\(0, 1fr\)/,
    );
    assert.match(
        menuCss["main_menu.layout.css"],
        /#sow-menu\[data-screen="home"\] \.sow-menu__battlefield\s*\{[^}]*position:\s*absolute[^}]*pointer-events:\s*none/,
        "narrow home hero copy must overlay the background without taking layout space",
    );
    assert.doesNotMatch(
        menuCss["main_menu.layout.css"],
        /#sow-menu\[data-screen="home"\] \.sow-menu__battlefield\s*\{[^}]*order:\s*-1/,
        "the home hero overlay must not reorder the menu",
    );
    assert.match(
        menuCss["main_menu.layout.css"],
        /#sow-menu\[data-screen="home"\] \.sow-menu__battlefield\s*\{[^}]*z-index:\s*0/,
        "the narrow hero briefing must sit below the command panel",
    );
    assert.match(
        menuCss["main_menu.layout.css"],
        /#sow-menu\[data-screen="home"\] \.sow-menu__leader-copy\s*\{[^}]*position:\s*absolute[^}]*inset-block-start:\s*0[^}]*inset-inline-end:\s*10px[^}]*pointer-events:\s*none/,
        "the narrow home briefing stays positioned without reserving layout space",
    );
    assert.match(
        menuCss["main_menu.layout.css"],
        /#sow-menu\[data-screen="home"\] \.sow-menu__command\s*\{[^}]*z-index:\s*1/,
        "the map and menu controls retain their layer and normal flow",
    );
    const commandStart = surfacesSource.indexOf("export function renderCommandPanel()");
    const commandEnd = surfacesSource.indexOf("\nexport function ", commandStart + 1);
    const commandPanel = surfacesSource.slice(commandStart, commandEnd);
    assert.match(
        commandPanel,
        /<section class='sow-menu__command'>[\s\S]*sow-menu__home-public[\s\S]*renderPublicPanel\("home"\)[\s\S]*sow-menu__home-actions/,
        "the map and controls remain inside the normal-flow command panel",
    );
    const homeStart = lobbiesSource.indexOf("function renderHome()");
    const homeEnd = lobbiesSource.indexOf("const campaignCinematicAvailability", homeStart);
    const home = lobbiesSource.slice(homeStart, homeEnd);
    assert.match(
        home,
        /renderCommandPanel\(\)\s*\+\s*"<section class='sow-menu__battlefield'>[\s\S]*sow-menu__leader-copy[\s\S]*renderHeroBriefing/,
        "only the decorative leader briefing sits in the mobile overlay",
    );
    assert.match(
        menuCss["main_menu.layout.css"],
        /\.sow-menu__leader-copy\s*\{[^}]*padding:\s*0[^}]*border:\s*0[^}]*background:\s*transparent/,
        "mobile hero copy must not render as an in-flow card",
    );
    assert.match(
        menuCss["main_menu.layout.css"],
        /@media \(orientation: portrait\)\s*\{[^}]*#sow-menu\[data-screen="home"\] \.sow-menu__leader-copy\s*\{[^}]*display:\s*none/,
        "the portrait home screen hides the briefing while the loader keeps its separate copy",
    );
    assert.doesNotMatch(
        profileCss,
        /sow-heroes__featured-copy \.sow-heroes__perk-row\s*\{\s*display:\s*none/,
    );
    assert.match(
        profileCss,
        /\.sow-menu__leader-copy \.sow-hero-briefing__name,[\s\S]*?text-shadow:\s*0 2px 10px rgba\(0, 0, 0, (?:0)?\.92\)/,
        "the floating hero copy stays legible over the background art",
    );
    assert.match(
        profileCss,
        /#sow-menu\[data-screen="home"\] \.sow-menu__leader-copy \.sow-hero-briefing__name\s*\{[^}]*font-size:\s*clamp\(18px, 4\.5vw, 22px\)/,
        "the portrait home briefing uses a compact title that fits the clear area above the controls",
    );
});

test("random hero keeps a translated label and a short description in every locale", () => {
    for (const locale of [
        "en",
        "es",
        "de",
        "fr",
        "it",
        "pt-br",
        "ru",
        "ja",
        "ko",
        "zh-cn",
        "ar",
        "tr",
        "vi",
        "id",
        "fil",
    ]) {
        const catalog = fs.readFileSync(
            path.join(shell, `../../sow-i18n/strings/${locale}/web.toml`),
            "utf8",
        );
        const line = catalog.split("\n").find((entry) => entry.startsWith("random_leader_description"));
        assert.ok(
            line && line.split("=")[1].trim().length > 2,
            `heroes.random_leader_description must exist in ${locale}`,
        );
        assert.doesNotMatch(
            line,
            /server|servidor|serveur|Servidor|Server|Serveur|сервер|Сервер|サーバー|서버|服务器|الخادم|Sunucu|Máy chủ|khí/i,
            `heroes.random_leader_description must not talk about the server in ${locale}`,
        );
        const label = catalog.split("\n").find((entry) => entry.startsWith("random_leader ="));
        assert.ok(
            label && !/hero/i.test(label),
            `random_leader must be a single word, no HERO, in ${locale}`,
        );
    }
});

test("campaign episode cards use leader portraits and emphasize the next playable chapter", () => {
    const campaignCss = menuCss["main_menu.campaign.css"];
    assert.match(campaignCss, /\.sow-campaign__episode-art img[\s\S]*?object-fit: cover/);
    assert.match(campaignCss, /\.sow-campaign__map-art[\s\S]*?brightness\((?:0)?\.58\)/);
    assert.match(campaignCss, /\.sow-campaign__leader-art[\s\S]*?border-radius: 50%/);
    assert.match(campaignCss, /\.sow-campaign__episode\.is-next/);
    assert.match(
        campaignCss,
        /\.sow-campaign__episode\.is-locked \.sow-campaign__episode-art img[\s\S]*?grayscale/,
    );
});

test("campaign card map and leader art match every Rust episode and its JSON assets", () => {
    const start = campaignModule.indexOf("pub fn episode_id(");
    const end = campaignModule.indexOf("pub fn from_episode_id(", start);
    assert.ok(start >= 0 && end > start);
    const episodeIds = [...campaignModule.slice(start, end).matchAll(/=> "([^"]+)"/g)].map(
        (match) => match[1],
    );
    assert.deepEqual(episodeIds, ["boudica", "six_sky_ep1", "six_sky_ep2", "six_sky_ep3"]);
    const expected = {
        boudica: ["eastanglia", "boudica"],
        six_sky_ep1: ["northamerica", "lady_six_sky"],
        six_sky_ep2: ["northamerica", "lady_six_sky"],
        six_sky_ep3: ["northamerica", "lady_six_sky"],
    };
    const campaignDir = path.join(shell, "../../assets/campaign");
    const cinematicEpisodes = [];
    for (const episodeId of episodeIds) {
        const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, episodeId + ".json"), "utf8"));
        const definition = JSON.parse(
            fs.readFileSync(path.join(campaignDir, episodeId + ".triggers.json"), "utf8"),
        );
        assert.equal(definition.episode_id, episodeId);
        const opening = definition.steps.find((step) => step.id === definition.entry);
        if (opening && opening.video_src) {
            cinematicEpisodes.push(episodeId);
            assert.equal(opening.type, "scene", `${episodeId} opening cinematic is a scene`);
            assert.equal(
                opening.presentation,
                "cinematic",
                `${episodeId} opening cinematic uses the shared cinematic presentation`,
            );
            assert.ok(
                opening.video_src.startsWith(`/assets/campaign/${episodeId}/`),
                `${episodeId} cinematic stays in its asset folder`,
            );
            const mediaPath = path.join(shell, "../..", opening.video_src.replace(/^\/assets\//, "assets/"));
            assert.ok(fs.existsSync(mediaPath), `${episodeId} video asset exists`);
            const stem = mediaPath.replace(/\.(webm|mp4)$/i, "");
            assert.ok(
                fs.existsSync(stem + ".mp4") && fs.existsSync(stem + ".webm"),
                `${episodeId} cinematic ships both WebM and MP4 renditions`,
            );
            assert.ok(fs.existsSync(stem + ".poster.webp"), `${episodeId} cinematic ships a poster`);
            assert.ok(
                fs.statSync(mediaPath).size <= 6 * 1024 * 1024,
                `${episodeId} cinematic rendition stays under 6 MB`,
            );
        }
        if (episodeId === "boudica") {
            const ratioOverrides = definition.steps
                .filter((step) => Object.hasOwn(step, "attack_ratio_on_enter"))
                .map((step) => ({ id: step.id, ratio: step.attack_ratio_on_enter }));
            assert.deepEqual(
                ratioOverrides,
                [{ id: definition.entry, ratio: 0.25 }],
                "Boudica sets 25% once at entry and never resets the player's allocation",
            );
        }
        assert.equal(roster.map, expected[episodeId][0], episodeId + " card map");
        assert.ok(
            Object.values(definition.speakers || {}).some(
                (speaker) => speaker.avatar === expected[episodeId][1],
            ),
            episodeId + " card leader portrait",
        );
        for (const faction of roster.factions) {
            assert.match(faction.id, /^[a-z][a-z0-9_]{0,95}$/i, `${episodeId}/${faction.name}: stable ID`);
            assert.ok(
                Number.isInteger(faction.starting_troops) && faction.starting_troops >= 0,
                `${episodeId}/${faction.name}: starting troops`,
            );
            assert.ok(
                ["allied", "neutral", "enemy"].includes(faction.relation),
                `${episodeId}/${faction.name}: relationship`,
            );
            assert.ok(
                typeof faction.civ === "string" && faction.civ,
                `${episodeId}/${faction.name}: civilization`,
            );
            assert.ok(
                typeof faction.leader === "string" && faction.leader,
                `${episodeId}/${faction.name}: leader`,
            );
            for (const removed of ["role", "betrayal"])
                assert.equal(
                    Object.hasOwn(faction, removed),
                    false,
                    `${episodeId}/${faction.name}: no ${removed}`,
                );
            assert.equal(
                Object.hasOwn(faction, "hostility"),
                false,
                `${episodeId}/${faction.name}: no combat override`,
            );
            assert.match(faction.color, /^#[0-9a-f]{6}$/i, `${episodeId}/${faction.name}: map color`);
        }
    }
    assert.deepEqual(cinematicEpisodes, ["boudica"]);
    const watchFlow = tutorial.slice(
        tutorial.indexOf("function watchCampaignCinematic("),
        tutorial.indexOf("function showError(", tutorial.indexOf("function watchCampaignCinematic(")),
    );
    assert.match(
        watchFlow,
        /SOWCampaign\.create\(\s*data\.definition,\s*data\.definition\.entry,\s*data\.roster,?\s*\)/,
    );
    assert.doesNotMatch(watchFlow, /send\("start_campaign_episode"/);
});
