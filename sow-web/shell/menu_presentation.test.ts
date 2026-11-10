import { formatCompactNumber } from "./number-format.ts";
import { avatarBackgroundCss, avatarImageAttribute, setAvatarBackground, setAvatarImage } from "./avatar-presentation.ts";
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
const settingsSource = fs.readFileSync(path.join(shell, "main_menu.settings.ts"), "utf8");
const hudSource = fs.readFileSync(path.join(shell, "main_menu.hud.ts"), "utf8");
const dropdownSource = fs.readFileSync(path.join(shell, "sow-dropdown.ts"), "utf8");
const hudCss = fs.readFileSync(path.join(shell, "main_menu.hud.css"), "utf8");
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
const tutorialCss = fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
const controlsCss = fs.readFileSync(path.join(shell, "sow-controls.css"), "utf8");
const profileSource = fs.readFileSync(path.join(shell, "main_menu.profile.ts"), "utf8");
const serverSource = fs.readFileSync(path.join(shell, "../../sow-data/src/main.rs"), "utf8");
const privacyPage = fs.readFileSync(path.join(shell, "../site/privacy/index.html"), "utf8");
const pokiSource = fs.readFileSync(path.join(shell, "main_menu.poki.ts"), "utf8");
const jestSource = fs.readFileSync(path.join(shell, "main_menu.jest.ts"), "utf8");
const campaignModule = fs.readFileSync(path.join(shell, "../../sow-client/src/campaign/mod.rs"), "utf8");

test("in-game GUI panels, tracks, and semantic fills share the tactical theme tokens", () => {
    assert.match(controlsCss, /--sow-tactical-surface-card-bg:/);
    assert.match(controlsCss, /--sow-tactical-surface-subcard-bg:/);
    assert.match(controlsCss, /--sow-tactical-track-bg:/);
    assert.match(controlsCss, /--sow-tactical-focus-outline:/);
    assert.match(controlsCss, /--sow-fill-army-gradient:/);
    assert.match(controlsCss, /--sow-fill-pending-wash:/);
    assert.match(controlsCss, /--sow-fill-construction-gradient:/);

    assert.match(
        menuCss["main_menu.base.css"],
        /\.sow-menu__topbar\s*\{[^}]*background: var\(--sow-tactical-surface-card-bg\)/,
    );
    assert.match(
        menuCss["main_menu.layout.css"],
        /\.sow-menu__main-nav\s*\{[^}]*background: var\(--sow-tactical-surface-card-bg\)/,
    );
    assert.match(
        menuCss["main_menu.lobbies.css"],
        /\.sow-menu__lobby-chip,[\s\S]*?background: var\(--sow-tactical-surface-compact-bg\)/,
    );
    assert.match(
        menuCss["main_menu.create.css"],
        /\.sow-menu__custom-card\s*\{[^}]*background: var\(--sow-tactical-surface-card-bg\)/,
    );
    assert.match(
        menuCss["main_menu.queue.css"],
        /\.sow-menu__queue-summary-card\s*\{[^}]*background: var\(--sow-tactical-surface-card-bg\)/,
    );
    assert.match(
        menuCss["main_menu.settings.css"],
        /\.sow-menu__settings-modal\s*\{[^}]*background: var\(--sow-tactical-surface-card-bg\)/,
    );
    assert.match(
        menuCss["main_menu.campaign.css"],
        /\.sow-campaign__episode\s*\{[^}]*background: var\(--sow-tactical-surface-card-bg\)/,
    );
    assert.match(profileCss, /--sow-profile-panel: var\(--sow-tactical-surface-card-bg\)/);
    assert.match(hudCss, /--sow-nameplate-troop-fill: var\(--sow-fill-army-gradient\)/);
    assert.match(hudCss, /var\(--sow-fill-pending-wash\)/);
    assert.match(
        hudCss,
        /\.sow-hud__building-progress-fill\s*\{[^}]*background: var\(--sow-fill-construction-gradient\)/,
    );
    assert.match(
        tutorialCss,
        /\.sow-story__dialog\s*\{[^}]*background: var\(--sow-tactical-surface-card-bg\)/,
    );

    const themedUiCss = [
        menuCss["main_menu.base.css"],
        menuCss["main_menu.layout.css"],
        menuCss["main_menu.lobbies.css"],
        menuCss["main_menu.create.css"],
        menuCss["main_menu.queue.css"],
        menuCss["main_menu.settings.css"],
        menuCss["main_menu.campaign.css"],
        hudCss,
        tutorialCss,
        profileCss,
    ].join("\n");
    assert.doesNotMatch(themedUiCss, /linear-gradient\(\s*to top,\s*#246e10/);
    assert.doesNotMatch(themedUiCss, /linear-gradient\(90deg,\s*#d88310/);
});

test("campaign heading uses available width before wrapping translated words", () => {
    const campaignCss = menuCss["main_menu.campaign.css"];
    const headingStart = campaignCss.indexOf(".sow-campaign__intro h1 {");
    const headingEnd = campaignCss.indexOf("}", headingStart);
    assert.ok(headingStart >= 0 && headingEnd > headingStart);
    const desktopHeading = campaignCss.slice(headingStart, headingEnd);
    assert.match(desktopHeading, /max-width:\s*100%/);
    assert.match(desktopHeading, /word-break:\s*normal/);
    assert.match(desktopHeading, /overflow-wrap:\s*anywhere/);
    assert.doesNotMatch(desktopHeading, /max-width:\s*7ch/);

    const mobileHeadingStart = campaignCss.indexOf(".sow-campaign__intro h1 {", headingEnd);
    const mobileHeadingEnd = campaignCss.indexOf("}", mobileHeadingStart);
    assert.ok(mobileHeadingStart > headingEnd && mobileHeadingEnd > mobileHeadingStart);
    const mobileHeading = campaignCss.slice(mobileHeadingStart, mobileHeadingEnd);
    assert.match(mobileHeading, /word-break:\s*normal/);
    assert.match(mobileHeading, /overflow-wrap:\s*anywhere/);
});

function renderSettingsBody(source) {
    const start = source.indexOf("function renderSettings()");
    const end = source.indexOf("\n}\n", start);
    assert.notEqual(start, -1);
    assert.notEqual(end, -1);
    return source.slice(start, end + 2);
}

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

test("all menu builds share the ranked Quick Match label and current league", () => {
    const buttonStart = coreSource.indexOf("export function renderQuickMatchButton()");
    const buttonEnd = coreSource.indexOf("\n}", buttonStart);
    assert.ok(buttonStart >= 0 && buttonEnd > buttonStart);
    const button = coreSource.slice(buttonStart, buttonEnd);
    assert.match(button, /data-command='quick_match'/);
    assert.match(button, /lobbies\.ranked_1v1/);
    assert.match(button, /data-ranked-current/);
    assert.match(button, /ranked\.tier/);
    for (const source of [surfacesSource, pokiSource, jestSource]) {
        const commandStart = source.indexOf("export function renderCommandPanel()");
        const commandEnd = source.indexOf("\nexport function ", commandStart + 1);
        assert.ok(commandStart >= 0 && commandEnd > commandStart);
        assert.match(source.slice(commandStart, commandEnd), /renderQuickMatchButton\(\)/);
    }
});

test("ranked result shows a first league after placements, never a first-match demotion", () => {
    const start = shellSource.indexOf("function rankedResultTitle(");
    const end = shellSource.indexOf("\n}", start);
    assert.ok(start >= 0 && end > start);
    const context = { SOW_t: (key) => key };
    const title = vm.runInNewContext(shellSource.slice(start, end + 2) + "; rankedResultTitle;", context);
    const result = {
        score_before: 1000,
        score_after: 838,
        tier_before: "Bronze",
        tier_after: "Bronze",
        division_before: "II",
        division_after: "IV",
        placements_complete: false,
        games_played: 1,
    };
    assert.equal(title(result), "ranked.placements");
    assert.equal(title({ ...result, games_played: 5, placements_complete: true }), "ranked.first_league");
    assert.equal(title({ ...result, games_played: 6, placements_complete: true }), "ranked.rank_changed");
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

test("leader selection confirms immediately, persists after debounce, and reopening uses the staged choice", () => {
    const previewConcreteStart = shellSource.indexOf('if (command === "preview_leader")');
    const previewRandomStart = shellSource.indexOf('if (command === "preview_random_leader")');
    const randomStart = shellSource.indexOf('if (command === "confirm_random_leader")');
    const concreteStart = shellSource.indexOf('if (command === "confirm_leader")', randomStart);
    const concreteEnd = shellSource.indexOf("/* POKI_SHARED_STORE_UNLOCK_BEGIN */", concreteStart);
    const pickerStart = shellSource.indexOf('if (command === "open_leader_picker")');
    const navStart = shellSource.indexOf('if (command === "main_nav")');
    const navEnd = shellSource.indexOf('if (command === "open_leader_picker")', navStart);
    assert.ok(previewConcreteStart >= 0 && previewRandomStart > previewConcreteStart);
    assert.ok(randomStart >= 0 && concreteStart > randomStart && concreteEnd > concreteStart);
    assert.ok(pickerStart >= 0 && navStart >= 0 && navEnd > navStart);

    const previewConcrete = shellSource.slice(previewConcreteStart, previewRandomStart);
    const previewRandom = shellSource.slice(previewRandomStart, randomStart);
    const pickerOpen = shellSource.slice(
        pickerStart,
        shellSource.indexOf('if (command === "toggle_dropdown")', pickerStart),
    );
    const heroesNavigation = shellSource.slice(navStart, navEnd);

    assert.doesNotMatch(previewConcrete, /isHeroSelectionQueueLocked/);
    assert.match(
        previewConcrete,
        /const persistSelection = !!leaderId && leader\.available !== false;\s*if \(persistSelection\) stageLeaderSelection\(false, leaderId\);/,
    );
    assert.match(previewConcrete, /if \(persistSelection\) scheduleLeaderPersist\(false, leaderId\);/);
    assert.match(previewConcrete, /if \(!updateHeroesPreview\(\)\) render\(\);\s*updateTopbar\(\);/);

    assert.doesNotMatch(previewRandom, /isHeroSelectionQueueLocked/);
    assert.match(previewRandom, /stageLeaderSelection\(true, null\);/);
    assert.match(previewRandom, /scheduleLeaderPersist\(true, null\);/);
    assert.ok(
        previewRandom.indexOf("stageLeaderSelection(true, null)") <
            previewRandom.indexOf("scheduleLeaderPersist(true, null)"),
    );
    assert.match(previewRandom, /if \(!updateHeroesPreview\(\)\) render\(\);\s*updateTopbar\(\);/);
    assert.doesNotMatch(previewRandom, /leader_id/, "Random must not carry a previous concrete leader ID");

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

    const updateStart = shellSource.indexOf("function updateTopbar()");
    const updateEnd = shellSource.indexOf("\nfunction ", updateStart + 1);
    assert.ok(updateStart >= 0 && updateEnd > updateStart);
    const topbarUpdate = shellSource.slice(updateStart, updateEnd);
    assert.match(topbarUpdate, /const avatarUrl = avatarImage\(\)/);
    assert.match(topbarUpdate, /setAvatarBackground\(avatar, avatarUrl\)/);

    const baseCss = menuCss["main_menu.base.css"];
    const layoutCss = menuCss["main_menu.layout.css"];
    assert.match(
        baseCss,
        /\.sow-menu__name\s*\{[^}]*min-width:\s*0;[^}]*font-size:\s*var\(--sow-topbar-name-size\)/,
    );
    assert.match(
        baseCss,
        /#sow-menu \.sow-menu__name input\s*\{[^}]*width:\s*100%;[^}]*text-overflow:\s*ellipsis/,
    );
    const nameSizes = [...(baseCss + layoutCss).matchAll(/--sow-topbar-name-size:\s*(\d+)px/g)].map((match) =>
        Number(match[1]),
    );
    assert.ok(
        nameSizes.length >= 4 && nameSizes.every((size) => size >= 16 && size <= 24),
        String(nameSizes),
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
        avatarImageAttribute,
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
    const settings = [renderSettingsBody(surfacesSource), renderSettingsBody(pokiSource)];
    for (const body of settings) {
        assert.match(body, /menu\.settings/);
        assert.match(body, /menu\.settings_preferences/);
        assert.match(body, /menu\.game_links/);
        assert.match(body, /sow-menu__settings-section/);
        assert.match(body, /data-command='toggle_settings'/);
        assert.match(body, /menu\.music_volume/);
        assert.match(body, /renderMusicSpectrum\(\)/);
        assert.doesNotMatch(body, /menu\.motion_animation|settings-motion|data-setting='reduced_motion'/);
        assert.match(body, /renderLocaleSettingRow\(\)/);
        assert.match(body, /renderSettingsPrivacySection/);
        assert.match(body, /account-value/);
        assert.match(body, /settings-account/);
        assert.doesNotMatch(
            body,
            /system_configuration|menu\.done|master_audio|settings-mute|account-row|WOU-ID|\+1/,
        );
    }
    assert.match(renderSettingsBody(jestSource), /renderSettingsPrivacySection/);
    assert.match(renderSettingsBody(jestSource), /renderLocaleSettingRow\(\)/);
    assert.match(renderSettingsBody(jestSource), /renderMusicSpectrum\(\)/);
    assert.match(settingsSource, /SOW_t\("menu\.language"\)/);
    assert.match(settingsSource, /key: dropdownKey/);
    assert.match(settingsSource, /SOW_renderDropdown\([\s\S]*?options: localeOptions\(\)/);
    assert.match(settingsSource, /setting: "locale"/);
    assert.match(settingsSource, /sow-menu__settings-row-icon/);
    assert.match(settingsSource, /data-menu-music-spectrum/);
    assert.match(menuCss["main_menu.settings.css"], /\.sow-menu__music-spectrum/);
    assert.match(hudSource, /data-hud-setting="reduced_motion"/);
});

test("settings command deck supports desktop 2-column, mobile vertical full-screen, and compact horizontal viewports", () => {
    const settingsCss = menuCss["main_menu.settings.css"];
    assert.match(settingsCss, /\.sow-menu__settings-modal\s*\{[^}]*backdrop-filter:\s*none;/);
    assert.match(
        settingsCss,
        /\.sow-menu__settings-body\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1\.08fr\)\s*minmax\(0,\s*0\.92fr\)/,
    );
    assert.match(
        settingsCss,
        /@media\s*\(max-width:\s*680px\),\s*\(orientation:\s*portrait\)[\s\S]*?\.sow-menu__settings-modal\s*\{[^}]*height:\s*100%;[^}]*max-height:\s*none;/,
    );
    assert.match(
        settingsCss,
        /@media\s*\(orientation:\s*landscape\)\s*and\s*\(max-height:\s*640px\),\s*\(orientation:\s*landscape\)\s*and\s*\(max-width:\s*960px\)/,
    );
    for (const body of [
        renderSettingsBody(surfacesSource),
        renderSettingsBody(pokiSource),
        renderSettingsBody(jestSource),
    ]) {
        assert.match(body, /sow-menu__settings-col--primary/);
        assert.match(body, /sow-menu__settings-col--secondary/);
        assert.match(body, /sow-menu__settings-head-icon/);
        assert.match(body, /sow-menu__settings-row-icon/);
    }
});

test("settings social links include SVGs and preserve web and Android link behavior", () => {
    const start = surfacesSource.indexOf("function renderSettingsLink(");
    const end = surfacesSource.indexOf("\n}\n\nfunction renderSettingsLinks", start) + 2;
    assert.ok(start >= 0 && end > start);
    let android = false;
    const renderSettingsLink = vm.runInNewContext(
        surfacesSource.slice(start, end) + "\nrenderSettingsLink;",
        {
            esc: (value) => String(value),
            asset: (path) => "/assets/" + path,
            isAndroidTwa: () => android,
            SOW_t: (key) => key,
        },
    );
    for (const [url, key, icon] of [
        ["https://discord.gg/4mBM6zJyG", "menu.discord", "discord"],
        ["https://t.me/shadowsofwario", "menu.telegram", "telegram"],
        ["https://github.com/worldofunreal/shadows-of-war/tree/test", "menu.source_code", "github"],
    ]) {
        const socialClass = "sow-menu__settings-link--social";
        const image = `<img class='sow-menu__settings-social-icon' src='/assets/site/media/${icon}.svg' alt='' aria-hidden='true'>`;
        const webLink = renderSettingsLink(url, key, socialClass, icon);
        assert.ok(webLink.includes(`href='${url}' target='_blank' rel='noopener noreferrer'`));
        assert.ok(webLink.includes(`${image}${key}`));

        android = true;
        const androidLink = renderSettingsLink(url, key, socialClass, icon);
        assert.ok(androidLink.includes(`data-command='open_android_external_link' data-url='${url}'`));
        assert.ok(androidLink.includes(`${image}${key}`));
        android = false;
    }
    assert.match(
        surfacesSource,
        /renderSettingsLink\(\s*sourceUrl,\s*"menu\.source_code",[\s\S]*?"github"\s*,?\s*\)/,
    );
});

test("language dropdowns keep their width and placement inside the viewport", () => {
    const start = dropdownSource.indexOf("export function resetDropdownPosition(");
    const end = dropdownSource.indexOf("export function syncDropdownScope(", start);
    assert.ok(start >= 0 && end > start);
    const context = vm.createContext({
        document: { documentElement: { clientWidth: 390, clientHeight: 844, dir: "rtl" } },
        window: { innerWidth: 390, innerHeight: 844 },
    });
    vm.runInContext(
        dropdownSource.slice(start, end).replace(/\bexport\s+/g, "") +
            "this.positionDropdown = positionDropdown; this.resetDropdownPosition = resetDropdownPosition;",
        context,
    );
    const menu = { style: {}, scrollHeight: 548 };
    context.positionDropdown(
        { getBoundingClientRect: () => ({ left: 330, right: 380, top: 800, bottom: 840, width: 50 }) },
        menu,
    );
    assert.equal(menu.style.position, "fixed");
    assert.equal(menu.style.left, "330px");
    assert.equal(menu.style.top, "554px");
    assert.equal(menu.style.width, "50px");
    assert.equal(menu.style.minWidth, "50px");
    assert.equal(menu.style.maxHeight, "240px");
    context.resetDropdownPosition(menu);
    assert.equal(menu.style.minWidth, "");
    assert.equal(menu.style.position, "");
    assert.equal(menu.style.left, "");
});

test("custom game map dropdown stays viewport-positioned and keeps its own scroll", () => {
    const createCss = menuCss["main_menu.create.css"];
    const mapPanelRule = createCss.match(
        /#sow-menu\[data-screen="create"\] \.sow-create__map-panel\s*\{([^}]*)\}/,
    );
    assert.ok(mapPanelRule);
    assert.match(mapPanelRule[1], /backdrop-filter:\s*none;/);
    assert.match(lobbiesSource, /key:\s*"create-map"/);
    assert.match(dropdownSource, /menu\.style\.position\s*=\s*"fixed"/);
    assert.match(createCss, /\.sow-create__scroll\s*\{[^}]*overflow-y:\s*auto;/s);
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
        const audioContexts = [];
        const mediaMetadata = [];
        const frames = new Map();
        const spectrumDraws = [];
        const logs = [];
        const windowListeners = new Map();
        const documentListeners = new Map();
        let reducedMotion = false;
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
        class FakeAudioNode {
            constructor() {
                this.connections = [];
            }
            connect(node) {
                this.connections.push(node);
            }
            disconnect() {
                this.connections = [];
            }
        }
        class FakeGainNode extends FakeAudioNode {
            constructor() {
                super();
                this.gain = { value: 1 };
            }
            connect(node) {
                super.connect(node);
                if (node instanceof FakeAnalyserNode) node.inputGain = this;
            }
        }
        class FakeAnalyserNode extends FakeAudioNode {
            constructor() {
                super();
                this.fftSize = 256;
                this.frequencyBinCount = 128;
                this.smoothingTimeConstant = 0;
                this.minDecibels = -100;
                this.maxDecibels = -30;
                this.bins = new Uint8Array(this.frequencyBinCount);
                this.inputGain = null;
            }
            getByteFrequencyData(target) {
                const gain = this.inputGain ? this.inputGain.gain.value : 1;
                for (let index = 0; index < target.length; index += 1)
                    target[index] = Math.round(this.bins[index] * gain);
            }
        }
        class FakeAudioContext {
            constructor() {
                this.state = "suspended";
                this.destination = new FakeAudioNode();
                this.sources = [];
                this.gains = [];
                this.analyzers = [];
                audioContexts.push(this);
            }
            resume() {
                this.state = "running";
                return Promise.resolve();
            }
            close() {
                this.state = "closed";
                return Promise.resolve();
            }
            createMediaElementSource(deck) {
                const source = new FakeAudioNode();
                source.mediaElement = deck;
                this.sources.push(source);
                return source;
            }
            createGain() {
                const gain = new FakeGainNode();
                this.gains.push(gain);
                return gain;
            }
            createAnalyser() {
                const analyser = new FakeAnalyserNode();
                this.analyzers.push(analyser);
                return analyser;
            }
        }
        const spectrumCanvasContext = {
            setTransform() {},
            clearRect() {},
            fillRect(...args) {
                spectrumDraws.push(args);
            },
        };
        const spectrumCanvas = {
            hidden: true,
            width: 300,
            height: 20,
            clientWidth: 300,
            clientHeight: 20,
            getContext(type) {
                return type === "2d" ? spectrumCanvasContext : null;
            },
        };
        const mediaSession = { metadata: null };
        const window = {
            Audio: FakeAudio,
            AudioContext: FakeAudioContext,
            MediaMetadata: FakeMediaMetadata,
            URL,
            location: { origin: "https://cdn.test" },
            devicePixelRatio: 1,
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
            hidden: false,
            contains(target) {
                return Boolean(target && target.insideMenu);
            },
            querySelector(selector) {
                return selector === "[data-menu-music-spectrum]" ? spectrumCanvas : null;
            },
        };
        const document = {
            body,
            baseURI: "https://cdn.test/play/",
            visibilityState: "visible",
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
            menuUi: { settingsOpen: false, state: null },
            reducedRewardMotion: () => reducedMotion,
            Node: Object,
            console: {
                error: (...args) => logs.push(["error", ...args]),
                warn: (...args) => logs.push(["warn", ...args]),
            },
        };
        vm.runInNewContext(
            shellSource.slice(start, end) +
                "this.musicApi = { syncMenuMusic, menuMusic, menuMusicFinishLoaderCycle, menuMusicSetCinematicActive, menuMusicSyncSpectrum, menuMusicDrawSpectrum, menuMusicMixes, menuMusicSetVolumes };",
            context,
        );
        return {
            api: context.musicApi,
            audios,
            logs,
            mediaMetadata,
            mediaSession,
            audioContexts,
            spectrumCanvas,
            spectrumDraws,
            menuUi: context.menuUi,
            setReducedMotion(value) {
                reducedMotion = value;
            },
            frames,
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

    const spectrum = createHarness();
    const mainMenu = state("MainMenu", 0.8, false, "Idle");
    spectrum.api.syncMenuMusic(mainMenu, true);
    spectrum.menuUi.state = mainMenu;
    spectrum.menuUi.settingsOpen = true;
    spectrum.api.menuMusicSyncSpectrum();
    await flushPlay();
    assert.equal(spectrum.audioContexts.length, 1, "opening settings creates one lazy Web Audio graph");
    assert.equal(spectrum.audioContexts[0].sources.length, 2, "both existing music decks feed the graph");
    assert.equal(spectrum.api.menuMusic.spectrumAnalyzers.filter(Boolean).length, 2);
    assert.equal(spectrum.api.menuMusic.spectrumGains[0].gain.value, 0.8);
    assert.equal(spectrum.api.menuMusic.decks[0].volume, 1, "the graph preserves the deck's effective gain");
    assert.equal(spectrum.spectrumCanvas.hidden, false);
    spectrum.audioContexts[0].analyzers[0].bins.fill(160);
    spectrum.api.menuMusicDrawSpectrum(spectrum.spectrumCanvas);
    assert.ok(spectrum.spectrumDraws.length > 0, "live frequency bins render as bars");
    const fullVolumeBarHeight = spectrum.spectrumDraws[0][3];

    spectrum.api.syncMenuMusic(state("MainMenu", 0.4, false, "Idle"), true, 0.4);
    assert.equal(
        spectrum.api.menuMusic.spectrumGains[0].gain.value,
        0.4,
        "the slider lowers the analyzed signal",
    );
    spectrum.spectrumDraws.length = 0;
    spectrum.api.menuMusicDrawSpectrum(spectrum.spectrumCanvas);
    assert.ok(
        spectrum.spectrumDraws[0][3] < fullVolumeBarHeight,
        "the fixed spectrum scale visibly shrinks when the music slider is lowered",
    );
    spectrum.api.menuMusic.crossfade = { outgoing: 0, incoming: 1, progress: 0.5, startedAt: 0 };
    spectrum.api.menuMusicSetVolumes(spectrum.api.menuMusicMixes());
    assert.equal(spectrum.api.menuMusic.spectrumGains[0].gain.value, 0.2);
    assert.equal(
        spectrum.api.menuMusic.spectrumGains[1].gain.value,
        0.2,
        "crossfade keeps both deck gains aligned",
    );

    spectrum.api.syncMenuMusic(state("MainMenu", 0, false, "Idle"), true, 0);
    spectrum.spectrumDraws.length = 0;
    spectrum.api.menuMusicSyncSpectrum();
    assert.deepEqual(spectrum.spectrumDraws.at(-1), [0, 19, 300, 1], "zero volume draws a flat trace");
    spectrum.api.syncMenuMusic(state("MainMenu", 0.4, true, "Idle"), false, 0.4);
    spectrum.spectrumDraws.length = 0;
    spectrum.api.menuMusicSyncSpectrum();
    assert.deepEqual(spectrum.spectrumDraws.at(-1), [0, 19, 300, 1], "global mute draws a flat trace");

    spectrum.api.syncMenuMusic(state("MainMenu", 0.4, false, "Idle"), true, 0.4);
    spectrum.setReducedMotion(true);
    spectrum.api.menuMusicSyncSpectrum();
    assert.equal(spectrum.api.menuMusic.spectrumFrame, 0, "reduced motion freezes the live spectrum");
    spectrum.setReducedMotion(false);
    spectrum.api.menuMusicSyncSpectrum();
    const spectrumFrame = spectrum.api.menuMusic.spectrumFrame;
    assert.ok(spectrumFrame, "the visible settings panel schedules spectrum frames");
    spectrum.document.visibilityState = "hidden";
    spectrum.api.menuMusicSyncSpectrum();
    assert.equal(spectrum.api.menuMusic.spectrumFrame, 0, "a hidden tab stops spectrum sampling");
    assert.equal(spectrum.frames.has(spectrumFrame), false);
    spectrum.document.visibilityState = "visible";
    spectrum.api.menuMusicSyncSpectrum();
    spectrum.menuUi.settingsOpen = false;
    spectrum.api.menuMusicSyncSpectrum();
    assert.equal(spectrum.api.menuMusic.spectrumFrame, 0, "closing settings stops spectrum sampling");

    const fallback = createHarness();
    fallback.window.AudioContext = undefined;
    const fallbackState = state("MainMenu", 0.7, false, "Idle");
    fallback.api.syncMenuMusic(fallbackState, true);
    fallback.menuUi.state = fallbackState;
    fallback.menuUi.settingsOpen = true;
    fallback.api.menuMusicSyncSpectrum();
    assert.equal(fallback.api.menuMusic.decks[0].paused, false, "unsupported analysis keeps music playing");
    assert.equal(
        fallback.api.menuMusic.decks[0].volume,
        0.7,
        "unsupported analysis keeps native volume control",
    );
    assert.equal(fallback.spectrumCanvas.hidden, true);
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
    const loaderStart = coreSource.indexOf("export function renderShowcaseLoader(");
    const loaderEnd = coreSource.indexOf("export function abilityIconMarkup(", loaderStart);
    const abilityStart = coreSource.indexOf("export function abilityIconMarkup(");
    const abilityEnd = coreSource.indexOf("export function leaderArtUrl(", abilityStart);
    assert.ok(
        start >= 0 &&
            end > start &&
            loaderStart >= 0 &&
            loaderEnd > loaderStart &&
            abilityStart >= 0 &&
            abilityEnd > abilityStart,
    );
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
        coreSource.slice(loaderStart, loaderEnd).replace(/\bexport\s+/g, "") +
            coreSource.slice(abilityStart, abilityEnd).replace(/\bexport\s+/g, "") +
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

test("weekly free rotation always keeps its localized hero tag", () => {
    const predicateStart = heroesSource.indexOf("function isWeeklyFreeRotation");
    const predicateEnd = heroesSource.indexOf("\n}", predicateStart) + 2;
    const rarityStart = heroesSource.indexOf("function heroesRarityLabel");
    const rarityEnd = heroesSource.indexOf("\n}", rarityStart) + 2;
    const labelStart = heroesSource.indexOf("function heroesStatusLabel");
    const labelEnd = heroesSource.indexOf("\n}", labelStart) + 2;
    assert.ok(predicateStart >= 0 && predicateEnd > predicateStart);
    assert.ok(rarityStart >= 0 && rarityEnd > rarityStart);
    assert.ok(labelStart >= 0 && labelEnd > labelStart);
    const context = { SOW_t: (key) => key };
    vm.runInNewContext(
        heroesSource.slice(predicateStart, predicateEnd) +
            "\n" +
            heroesSource.slice(rarityStart, rarityEnd) +
            "\n" +
            heroesSource.slice(labelStart, labelEnd) +
            "\nthis.heroesStatusLabel = heroesStatusLabel;",
        context,
    );
    const weeklyLeader = { id: "free", available: true, free_rotation: true, owned: false };
    assert.equal(context.heroesStatusLabel(weeklyLeader), "heroes.weekly_free_rotation");
    assert.equal(
        context.heroesStatusLabel({ ...weeklyLeader, available: false }),
        "",
        "unavailable heroes do not get the free-rotation tag",
    );
    assert.equal(
        context.heroesStatusLabel({ ...weeklyLeader, owned: true }),
        "",
        "owned heroes do not get the free-rotation tag",
    );
    assert.equal(
        context.heroesStatusLabel({ ...weeklyLeader, rarity: "epic" }),
        "heroes.rarity_epic · heroes.weekly_free_rotation",
        "the tier comes first, then the availability tag",
    );
    assert.equal(
        context.heroesStatusLabel({ id: "Boudica", available: true, permanently_free: true, rarity: "legendary" }),
        "heroes.rarity_legendary",
        "Boudica shows her tier but no availability tag",
    );
    assert.equal(
        context.heroesStatusLabel({ ...weeklyLeader, permanently_free: true }),
        "heroes.always_free",
        "permanently free heroes keep their own tag",
    );
    assert.match(heroesSource, /data-hero-status-label/);

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
        assert.match(catalog, /^weekly_free_rotation\s*=/m, `the free-rotation tag exists in ${locale}`);
    }
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
    const modalEnd = heroesSource.indexOf("export function syncDropdowns", modalStart);
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

test("hero selection waits for its skill icon and uses the shared loader", () => {
    const availabilityStart = heroesSource.indexOf("function setFeaturedArtAvailability(");
    const availabilityEnd = heroesSource.indexOf("\n}", availabilityStart);
    const availability = heroesSource.slice(availabilityStart, availabilityEnd);
    assert.match(availability, /\[data-hero-skill-icon\]/);
    assert.match(availability, /if \(!image\.complete \|\| \(skillIcon && !skillIcon\.complete\)\) return/);
    assert.match(heroesSource, /image\.dataset\.heroSkillArtSlug === art\.dataset\.heroArtSlug/);
    assert.match(coreSource, /renderShowcaseLoader\("showcase-loader-arc-skill"\)/);
    assert.match(coreSource, /data-hero-skill-icon[\s\S]*?fetchpriority='high'/);
    assert.match(
        profileCss,
        /\.sow-heroes__featured\.is-loading \.sow-hero-briefing--selection \.sow-hero-briefing__icon\s*\{\s*opacity:\s*0;/,
    );

    const artLoaderStart = heroesSource.indexOf("function renderHeroesArtLoader(");
    const artLoaderEnd = heroesSource.indexOf("export function renderHeroes()", artLoaderStart);
    const loaderStart = coreSource.indexOf("export function renderShowcaseLoader(");
    const loaderEnd = coreSource.indexOf("export function abilityIconMarkup(", loaderStart);
    const context: Record<string, any> = {
        esc: (v: unknown) => String(v ?? ""),
        avatarImageAttribute,
        heroesAvatarUrl: (slug: string) => "/assets/gameplay/avatars/" + slug + ".webp",
    };
    vm.runInNewContext(
        coreSource.slice(loaderStart, loaderEnd).replace(/\bexport\s+/g, "") +
            "\n" +
            heroesSource.slice(artLoaderStart, artLoaderEnd) +
            "\nthis.renderHeroesArtLoader = renderHeroesArtLoader;",
        context,
    );
    const artLoaderMarkup = String(context.renderHeroesArtLoader("artigas"));
    assert.equal(
        (artLoaderMarkup.match(/<div\b/g) || []).length,
        (artLoaderMarkup.match(/<\/div>/g) || []).length,
        "renderHeroesArtLoader must produce balanced <div> tags so it never closes .sow-heroes__workspace early",
    );
    assert.match(
        artLoaderMarkup,
        /^<div class='showcase-loader' data-hero-art-loader[^>]*><svg[\s\S]*<\/svg><img class='showcase-portrait'[^>]*><\/div>$/,
    );

    const renderHeroesBody = heroesSource.slice(artLoaderEnd);
    assert.equal(
        (renderHeroesBody.match(/<div\b/g) || []).length,
        (renderHeroesBody.match(/<\/div>/g) || []).length,
        "renderHeroes must keep balanced <div> tags",
    );
    assert.equal(
        (renderHeroesBody.match(/<section\b/g) || []).length,
        (renderHeroesBody.match(/<\/section>/g) || []).length,
        "renderHeroes must keep balanced <section> tags",
    );

    const artProfiles = fs.readFileSync(path.join(shell, "../../sow-dist/art-optimization.toml"), "utf8");
    const abilityProfile = artProfiles.match(/\[profiles\.ability_icon\]([\s\S]*?)(?=\n\[|$)/)?.[1] || "";
    assert.match(abilityProfile, /max_width = 128/);
    assert.match(abilityProfile, /max_height = 128/);
    assert.match(abilityProfile, /recipe_version = 2/);
});

test("hero cards confirm selection without a lock notice or a confirm button", () => {
    const fullStart = heroesSource.indexOf("export function renderHeroes()");
    const full = heroesSource.slice(fullStart);
    assert.doesNotMatch(full, /class='[^']*sow-heroes__confirm/);
    assert.doesNotMatch(full, /data-hero-queue-notice|isHeroSelectionQueueLocked|heroLockBadgeSvg/);

    const cardStart = heroesSource.indexOf("function renderHeroesCard");
    const cardEnd = heroesSource.indexOf("function randomLeaderName", cardStart);
    const card = heroesSource.slice(cardStart, cardEnd);
    assert.doesNotMatch(card, /is-queue-locked|heroLockBadgeSvg/);
    assert.doesNotMatch(profileCss, /\.sow-heroes__queue-notice|\.sow-heroes__card-lock|is-queue-locked/);
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
    assert.match(compact, /--skill-body:\s*12px;[\s\S]*--skill-icon:\s*48px;/);
    assert.doesNotMatch(compact, /font-size:\s*10px|32px/);
    assert.match(profileCss, /--skill-title:\s*calc\(var\(--skill-body\)\s*\*\s*1\.5\)/);
    assert.match(profileCss, /\.sow-hero-briefing__ability-name\s*\{[^}]*font-size:\s*14px/);
    assert.match(
        profileCss,
        /\.sow-menu__leader-copy \.sow-hero-briefing__ability-name\s*\{[^}]*font-size:\s*clamp\(16px/,
    );
    assert.match(profileCss, /\.sow-menu__leader-copy \.sow-hero-briefing__icon-slot\s*\{[^}]*width:\s*72px/);
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
    assert.match(
        menuCss["main_menu.layout.css"],
        /@media \(orientation: landscape\)[\s\S]*?#sow-menu\[data-screen="home"\] \.sow-menu__battlefield\s*\{[^}]*justify-content:\s*flex-end/,
        "landscape hero details stay anchored near the bottom",
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
        /#sow-menu\[data-screen="home"\] \.sow-menu__leader-copy\s*\{[^}]*position:\s*absolute[^}]*inset-block-end:\s*clamp\(16px,\s*6vh,\s*40px\)[^}]*inset-inline-end:\s*10px[^}]*pointer-events:\s*none/,
        "the narrow home briefing sits near the bottom without reserving layout space",
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

test("campaign featured stage selects hero art, switches PLAY and REPLAY, and supports multi-viewport responsive layout", () => {
    const campaignCss = menuCss["main_menu.campaign.css"];

    // Featured hero backdrop and picture elements
    assert.match(campaignCss, /\.sow-campaign__featured\s*\{/);
    assert.match(campaignCss, /\.sow-campaign__hero-backdrop\s*\{/);
    assert.match(campaignCss, /\.sow-campaign__hero-img\s*\{[^}]*object-fit:\s*cover/);
    assert.match(campaignCss, /@keyframes\s+sow-campaign-hero-enter/);

    // Dynamic action button (PLAY / REPLAY / LOCKED)
    assert.match(campaignCss, /\.sow-campaign__action-play\s*\{/);
    assert.match(campaignCss, /\.sow-campaign__episode\.is-selected/);

    // Expandable Sagas navigation rail
    assert.match(campaignCss, /\.sow-campaign__sagas-nav\s*\{/);
    assert.match(campaignCss, /\.sow-campaign__sagas-rail\s*\{/);
    assert.match(campaignCss, /\.sow-campaign__saga-tab\s*\{/);

    // Mobile vertical responsiveness
    assert.match(
        campaignCss,
        /@media\s*\(max-width:\s*680px\),\s*\(orientation:\s*portrait\)[\s\S]*?\.sow-campaign__featured\s*\{/,
    );

    // Mobile horizontal compact responsiveness
    assert.match(
        campaignCss,
        /@media\s*\(orientation:\s*landscape\)\s*and\s*\(max-height:\s*560px\)[\s\S]*?\.sow-campaign__featured\s*\{/,
    );

    // Logic in lobbiesSource
    assert.match(lobbiesSource, /function campaignEpisodeHeroSlug\(/);
    assert.match(lobbiesSource, /function campaignEpisodeHeroArt\(/);
    assert.match(lobbiesSource, /function updateCampaignSelection\(/);
    assert.match(lobbiesSource, /lobbies\.replay/);
    assert.match(lobbiesSource, /lobbies\.play/);
});

function rankedPresentationHarness() {
    const source = fs
        .readFileSync(path.join(shell, "main_menu.ranked.ts"), "utf8")
        .replace(/^import[\s\S]*?;\n/gm, "")
        .replace(/\bexport\s+/g, "");
    const frames = new Map();
    let frameId = 0;
    const context = {
        asset: (value) => "/assets/" + value,
        avatarImage: (value) => "/avatars/" + (value || "random"),
        avatarImageAttribute, avatarBackgroundCss, setAvatarBackground, setAvatarImage,
        esc: (value) =>
            String(value ?? "")
                .replace(/&/g, "&amp;")
                .replace(/</g, "&lt;")
                .replace(/'/g, "&#39;"),
        formatMapName: (lobby) => lobby.map_name,
        lobbyThumb: (lobby) => "/maps/" + lobby.map_name,
        menuUi: { state: { my_player_id: 1, player_name: "Me" }, rankedRating: null, rankedLadder: null },
        SOW_t: (key, values = {}) => key + JSON.stringify(values),
        requestAnimationFrame(callback) {
            const id = ++frameId;
            frames.set(id, callback);
            return id;
        },
        cancelAnimationFrame(id) {
            frames.delete(id);
        },
    };
    const api = vm.runInNewContext(
        source + "\n({ rankProgress, rankLabel, rankOverview, renderRankedDuel, animateRankResult });",
        context,
    );
    return {
        api,
        context,
        frames,
        tick(time) {
            const pending = [...frames.values()];
            frames.clear();
            pending.forEach((fn) => fn(time));
        },
    };
}

test("rank progress follows supplied boundaries, including jumps and the highest league", () => {
    const { api } = rankedPresentationHarness();
    const ladder = {
        steps: [
            { score: 800, tier: "Bronze", division: "IV" },
            { score: 900, tier: "Bronze", division: "III" },
            { score: 1200, tier: "Silver", division: "IV" },
            { score: 3200, tier: "Grandmaster", division: null },
        ],
        max_score: 4000,
    };
    assert.equal(api.rankProgress(899, ladder).remaining, 1);
    assert.equal(api.rankProgress(900, ladder).fraction, 0);
    assert.equal(api.rankProgress(1200, ladder).step.tier, "Silver");
    assert.equal(api.rankProgress(4000, ladder).fraction, 1);
    assert.equal(api.rankProgress(4000, ladder).next, null);
    assert.equal(api.rankProgress(1000, null), null);
    assert.match(api.rankLabel(null), /placements.*"played":0/);
    assert.match(api.rankLabel({ games_played: 4, placements_complete: false }), /"played":4/);
});

test("ranked search exposes no fictional map or opponent, while confirmed duel escapes names", () => {
    const { api } = rankedPresentationHarness();
    const search = api.renderRankedDuel(null);
    assert.doesNotMatch(search, /sow-ranked__map|\/maps\//);
    assert.match(search, /sow-ranked__search-halo/);
    const found = api.renderRankedDuel(
        {
            id: 7,
            map_name: "europe",
            players: [
                { player_id: 1, name: "Me", leader: "Boudica", is_ready: true },
                { player_id: 2, name: "<img src=x>", leader: "Caesar", is_ready: false },
            ],
        },
        true,
    );
    assert.match(found, /is-found/);
    assert.match(found, /&lt;img/);
    assert.doesNotMatch(found, /<img src=x>/);
    assert.match(found, /\/maps\/europe/);
});

test("rank result animates real points once and can finish immediately or cancel", () => {
    const { api, tick, frames } = rankedPresentationHarness();
    const fields = new Map();
    for (const key of [
        "[data-rank-score]",
        "[data-rank-name]",
        "[data-rank-current-emblem]",
        "[data-rank-target]",
        "[data-rank-remaining]",
    ])
        fields.set(key, { textContent: "", innerHTML: "" });
    const fill = { style: {} };
    const track = { querySelector: () => fill, setAttribute() {} };
    fields.set("[role=progressbar]", track);
    const classes = new Set();
    const card = {
        isConnected: true,
        dataset: {},
        offsetWidth: 1,
        querySelector: (key) => fields.get(key),
        classList: { add: (key) => classes.add(key), remove: (key) => classes.delete(key) },
    };
    const result = {
        score_before: 1190,
        score_after: 1230,
        tier_before: "Bronze",
        division_before: "I",
        tier_after: "Silver",
        division_after: "IV",
        games_played: 6,
        placements_complete: true,
        ladder: {
            steps: [
                { score: 1100, tier: "Bronze", division: "I" },
                { score: 1200, tier: "Silver", division: "IV" },
            ],
            max_score: 1600,
        },
    };
    const motion = api.animateRankResult(card, result, () => false);
    assert.match(fields.get("[data-rank-score]").textContent, /1190/);
    tick(0);
    tick(760);
    assert.match(fields.get("[data-rank-name]").textContent, /silver/);
    motion.finish();
    assert.match(fields.get("[data-rank-score]").textContent, /1230/);
    assert.equal(frames.size, 0);
    assert.ok(classes.has("is-rank-revealed"));
    const reduced = api.animateRankResult(
        card,
        { ...result, score_after: 1180, tier_after: "Bronze", division_after: "I" },
        () => true,
    );
    assert.match(fields.get("[data-rank-score]").textContent, /1180/);
    assert.equal(frames.size, 0);
    reduced.cancel();
    const interrupted = api.animateRankResult(card, result, () => false);
    interrupted.cancel();
    assert.equal(frames.size, 0);
    fields.get("[data-rank-current-emblem]").innerHTML = "unranked";
    api.animateRankResult(
        card,
        { ...result, games_played: 1, placements_complete: false },
        () => false,
    ).finish();
    assert.equal(
        fields.get("[data-rank-current-emblem]").innerHTML,
        "unranked",
        "placement progress cannot reveal an assigned league",
    );
    assert.equal(frames.size, 0);
    const cases = [
        { ...result, games_played: 5 },
        {
            ...result,
            score_before: 1230,
            score_after: 1180,
            tier_before: "Silver",
            division_before: "IV",
            tier_after: "Bronze",
            division_after: "I",
        },
        { ...result, score_before: 1230, score_after: 1230, tier_before: "Silver", division_before: "IV" },
        {
            ...result,
            score_before: 800,
            score_after: 1500,
            division_before: "IV",
            division_after: "I",
            ladder: {
                steps: [
                    { score: 800, tier: "Bronze", division: "IV" },
                    { score: 900, tier: "Bronze", division: "III" },
                    { score: 1200, tier: "Silver", division: "IV" },
                    { score: 1300, tier: "Silver", division: "III" },
                    { score: 1400, tier: "Silver", division: "II" },
                    { score: 1500, tier: "Silver", division: "I" },
                ],
                max_score: 1600,
            },
        },
    ];
    for (const scenario of cases) {
        api.animateRankResult(card, scenario, () => false);
        tick(0);
        tick(760);
        tick(1160);
        assert.match(fields.get("[data-rank-score]").textContent, new RegExp(String(scenario.score_after)));
        assert.equal(card.dataset.tier, scenario.tier_after.toLowerCase());
        assert.match(fields.get("[data-rank-name]").textContent, new RegExp(scenario.division_after));
        assert.equal(frames.size, 0);
    }
});

test("ranked result acknowledges before replay, ignores duplicate presses, and traps keyboard focus", () => {
    class Element {
        children = [];
        listeners = {};
        dataset = {};
        disabled = false;
        isConnected = true;
        innerHTML = "";
        inert = false;
        setAttribute() {}
        addEventListener(key, fn) {
            this.listeners[key] = fn;
        }
        appendChild(child) {
            this.children.push(child);
        }
        querySelector(selector) {
            if (!this.controls) this.controls = {};
            return (this.controls[selector] ||= new Element());
        }
        focus() {
            document.activeElement = this;
        }
    }
    const document = { createElement: () => new Element(), activeElement: new Element() };
    const root = new Element();
    const commands = [];
    let finished = 0;
    const context = {
        document,
        root,
        HTMLElement: Element,
        menuUi: { state: { account_id: "me", menu_return: { match_id: "42" } }, rankedLadder: null },
        SOW_t: (key, values = {}) => key + JSON.stringify(values),
        esc: (value) => String(value ?? ""),
        rankTier: (value) => value.toLowerCase(),
        rankEmblem: () => "<span></span>",
        rankName: (tier, division) => tier + " " + division,
        rankPlacements: (count) => "placements " + count,
        rankProgressMarkup: () => "",
        animateRankResult: () => ({
            finish() {
                finished++;
            },
            cancel() {},
        }),
        reducedRewardMotion: () => true,
        send: (command) => commands.push(command),
        rankedResultDismissedKey: null,
        rankedStageKey: null,
        rankedStageCleanup: null,
        activeRewardStage: null,
        removeRewardStage() {
            context.activeRewardStage = null;
        },
    };
    const start = shellSource.indexOf("function rankedResultKey(");
    const end = shellSource.indexOf("function maybePresentRewards(", start);
    vm.runInNewContext(shellSource.slice(start, end) + "; this.present = presentRankedResult;", context);
    const result = {
        score_before: 1000,
        score_after: 838,
        tier_before: "Bronze",
        division_before: "II",
        tier_after: "Bronze",
        division_after: "IV",
        games_played: 1,
        placements_complete: false,
    };
    context.present(result);
    const layer = root.children.at(-1);
    const card = layer.children[0];
    assert.match(card.innerHTML, /ranked.placements/);
    assert.doesNotMatch(card.innerHTML, /rank_demoted/);
    const again = card.querySelector("[data-ranked-again]");
    const close = card.querySelector("[data-ranked-continue]");
    assert.equal(document.activeElement, again);
    layer.listeners.keydown({ key: "Tab", preventDefault() {} });
    assert.equal(document.activeElement, close);
    const click = { preventDefault() {}, stopPropagation() {} };
    again.listeners.click(click);
    again.listeners.click(click);
    assert.deepEqual(commands, ["dismiss_ranked_result", "quick_match"]);
    assert.equal(finished, 1);
    assert.ok(again.disabled && close.disabled);
});

test("header nameplate owns avatar, nickname, level and XP bar; balances and settings sit at the end", () => {
    const source = shellSource.slice(
        shellSource.indexOf("function headerXp("),
        shellSource.indexOf("function syncDisconnectNotice("),
    );
    const state = { player_name: "LongNickname1234", level: 12, xp: 1150, crowns: 12345, gems: 9999 };
    let shop = true;
    const api = vm.runInNewContext(source + "\n({renderTopbar, formatCompactNumber});", {
        menuUi: { state },
        window: { SOW_LOCALE: "en" },
        SOW_t: (key) => key,
        esc: String,
        avatarImage: () => "random.webp",
        avatarImageAttribute, avatarBackgroundCss, setAvatarBackground, setAvatarImage,
        currencyAsset: (kind) => kind + ".webp",
        menuIcon: () => "settings",
        portalOffersStore: () => shop,
        formatCompactNumber,
    });
    const markup = api.renderTopbar();
    assert.match(markup, /<header[^>]+data-progression>/);
    const plate = markup.slice(
        markup.indexOf("sow-menu__nameplate"),
        markup.indexOf("sow-menu__top-actions"),
    );
    assert.match(plate, /data-command='open_leader_picker'[^>]+random.webp/);
    assert.match(plate, /<label class='sow-menu__name'><input data-role='display-name'[^>]+LongNickname1234/);
    assert.match(plate, /data-command='open_profile'/);
    assert.match(plate, /sow-menu__level'[^>]*><strong data-progression-level-value>12</);
    const portrait = plate.slice(0, plate.indexOf("<label"));
    assert.match(portrait, /data-progression-level-value>12</);
    assert.doesNotMatch(plate, /level_short|<small>menu.level/);
    assert.match(plate, /data-progression-xp-fill style='width:50%'/);
    assert.match(plate, /role='progressbar'[^>]*aria-valuenow='50'/);
    assert.match(plate, /aria-describedby='sow-menu-account-xp'/);
    assert.match(plate, /data-progression-xp-value>50\/100<\/b><small>menu.xp</);
    const actions = markup.slice(markup.indexOf("sow-menu__top-actions"));
    assert.match(
        actions,
        /<button class='sow-menu__currency sow-menu__crowns' type='button' data-command='main_nav' data-nav-screen='store'/,
    );
    assert.match(actions, /data-progression-crowns-value>12.3K/);
    assert.match(actions, /aria-label='store.crowns: 12345'/);
    assert.match(actions, /data-progression-gems-value>10K/);
    assert.ok(actions.indexOf("sow-menu__gems") < actions.indexOf("toggle_settings"));
    assert.doesNotMatch(markup, /sign_in|signin|laurel|account-label|wallet/);
    shop = false;
    const portal = api.renderTopbar();
    assert.match(portal, /<span class='sow-menu__currency sow-menu__gems' title=/);
    assert.doesNotMatch(portal, /data-nav-screen='store'/);
    assert.equal(api.formatCompactNumber(10000), "10K");
    assert.equal(api.formatCompactNumber(1000000), "1M");
    assert.equal(api.formatCompactNumber(0), "0");
    assert.match(
        shellSource,
        /querySelectorAll<HTMLElement>\("\.sow-menu__main-nav-item\[data-nav-screen\]"\)/,
        "only nav items mirror the active screen; header balances also carry data-nav-screen",
    );
});

test("header spans the desktop grid and draws a real XP bar at every size", () => {
    const base = menuCss["main_menu.base.css"];
    const layout = menuCss["main_menu.layout.css"];
    const desktopStart = layout.indexOf("@media (min-width: 681px) and (orientation: landscape) {");
    const desktop = layout.slice(desktopStart, layout.indexOf("\n}\n", desktopStart));
    assert.match(desktop, /\.sow-menu__topbar\s*\{\s*grid-column:\s*1 \/ -1;\s*grid-row:\s*1;/);
    const meters = [...(base + layout).matchAll(/--sow-topbar-meter-height:\s*(\d+)px/g)].map((match) =>
        Number(match[1]),
    );
    assert.ok(meters.length >= 1 && meters.every((height) => height >= 14), String(meters));
    assert.match(
        base,
        /\.sow-menu__xp-track\s*\{[^}]*height:\s*var\(--sow-topbar-meter-height\)[^}]*background:\s*var\(--sow-tactical-track-bg\)/,
    );
    assert.match(base, /\.sow-menu__xp-track i\s*\{[^}]*background:\s*var\(--sow-fill-xp-gradient\)/);
    assert.match(controlsCss, /--sow-fill-xp-gradient:\s*linear-gradient\(\s*to top,/);
    assert.match(base, /\.sow-menu__top-actions\s*\{[^}]*display: flex/);
    assert.doesNotMatch(layout, /\.sow-menu__top-actions\s*\{[^}]*display: grid/);
    assert.match(base, /\.sow-menu__progress\s*\{[^}]*max-width: 116px/);
    assert.match(base, /\.sow-menu__xp-track\s*\{[^}]*border-radius: 3px/);
    assert.doesNotMatch(layout, /identity signin settings|progress progress progress/);
});

test("split header progression updates both blocks and keeps laurels internal", () => {
    const source = shellSource.slice(
        shellSource.indexOf("export function writeProgression("),
        shellSource.indexOf("function ensureProgressionDisplay("),
    );
    const counters = Object.fromEntries(
        ["level-value", "xp-value", "xp-fill", "crowns-value", "gems-value"].map((key) => [
            "[data-progression-" + key + "]",
            {
                textContent: "",
                style: {},
                parentElement: {
                    title: "",
                    setAttribute(key, value) {
                        this[key] = value;
                    },
                },
            },
        ]),
    );
    const xpStart = shellSource.indexOf("function headerXp(");
    const headerXp = shellSource.slice(xpStart, shellSource.indexOf("\n}\n", xpStart) + 2);
    const api = vm.runInNewContext(
        "let displayedProgression;\n" +
            headerXp +
            source.replace(/export /g, "") +
            "\n({writeProgression, values: () => displayedProgression});",
        {
            root: { querySelector: () => ({ querySelector: (selector) => counters[selector] || null }) },
            SOW_t: (key, values) => (values ? key + ":" + values.level : key),
            formatCompactNumber,
        },
    );
    api.writeProgression({ level: 4, xp: 350, crowns: 12345, gems: 25, laurels: 10 });
    assert.equal(counters["[data-progression-level-value]"].textContent, "4");
    assert.equal(counters["[data-progression-level-value]"].parentElement["aria-label"], "profile.level:4");
    assert.equal(counters["[data-progression-xp-value]"].textContent, "50/100");
    assert.equal(counters["[data-progression-xp-fill]"].style.width, "50%");
    assert.equal(counters["[data-progression-crowns-value]"].textContent, "12.3K");
    assert.equal(counters["[data-progression-crowns-value]"].parentElement.title, "store.crowns: 12345");
    assert.equal(counters["[data-progression-gems-value]"].textContent, "25");
    assert.equal(api.values().laurels, 10);
    // The server XP arrives first; each new level is presented afterwards.
    // The bar must stay full under a held level, and reset only with the level change.
    for (const [xp, level, progress] of [
        [99, 1, 99],
        [100, 1, 100],
        [100, 2, 0],
        [350, 1, 100],
        [350, 2, 100],
        [350, 3, 100],
        [350, 4, 50],
        [0, 1, 0],
    ]) {
        api.writeProgression({ level, xp, crowns: 12345, gems: 25, laurels: 10 });
        assert.equal(counters["[data-progression-level-value]"].textContent, String(level));
        assert.equal(counters["[data-progression-xp-value]"].textContent, progress + "/100");
        assert.equal(counters["[data-progression-xp-fill]"].style.width, progress + "%");
        assert.equal(counters["[data-progression-xp-fill]"].parentElement["aria-valuenow"], String(progress));
    }
});

test("Settings owns optional sign-in and respects pending, reauthentication, and portal accounts", () => {
    let auth = { linked: false, pending: false };
    let offersSignIn = true;
    const window = {
        SOW_getAuthState: () => auth,
        SOW_getWouSession: () => ({ user: { email: "player@example.com" } }),
        SOW_WOU_SESSION_STATUS: "",
    };
    const renderSettings = vm.runInNewContext(
        renderSettingsBody(surfacesSource).replace(/export /g, "") + "\nrenderSettings;",
        {
            window,
            menuUi: { state: { settings: {} } },
            SOW_t: (key) => key,
            esc: String,
            portalOffersSignIn: () => offersSignIn,
            isAndroidTwa: () => false,
            renderLocaleSettingRow: () => "",
            renderMusicSpectrum: () => "",
            renderSettingsPrivacySection: () => "",
            renderSettingsLinks: () => "",
            renderSettingsLink: () => "",
            SOW_renderDropdown: () => "",
        },
    );
    assert.match(renderSettings(), /data-command='sign_in'/);
    auth = { linked: false, pending: true };
    assert.doesNotMatch(renderSettings(), /data-command='sign_in'/);
    assert.match(renderSettings(), /menu.connecting/);
    auth = { linked: true, pending: false, canSignOut: true };
    assert.doesNotMatch(renderSettings(), /data-command='sign_in'/);
    assert.match(renderSettings(), /player@example.com/);
    assert.match(renderSettings(), /data-command='sign_out'/);
    window.SOW_WOU_SESSION_STATUS = "reauthentication_required";
    assert.match(renderSettings(), /data-command='sign_in'/);
    offersSignIn = false;
    assert.doesNotMatch(renderSettings(), /data-command='sign_in'/);
});

test("laurel reward finishes safely without a header destination", async () => {
    const start = shellSource.indexOf("export function flyRewardCard(");
    const end = shellSource.indexOf("export function pulseRewardCounter", start);
    const fly = vm.runInNewContext(
        shellSource.slice(start, end).replace(/export /g, "") + "\nflyRewardCard;",
        {
            root: { querySelector: () => null },
        },
    );
    const card = { style: {} };
    assert.equal(await fly({ card, stage: { selector: "[data-progression-laurels-value]" } }, 1), true);
    assert.equal(card.style.visibility, "hidden");
});

test("header refresh preserves a focused nickname and never recreates login", () => {
    const start = shellSource.indexOf("function updateTopbar()");
    const end = shellSource.indexOf("function updateFrameChrome", start);
    const input = { value: "editing" };
    const avatar = { dataset: {}, style: {} };
    const document = { activeElement: input };
    const refresh = vm.runInNewContext(shellSource.slice(start, end) + "\nupdateTopbar;", {
        menuUi: { state: { player_name: "server-name" } },
        document,
        root: {
            querySelector: () => ({
                querySelector: (selector) => (selector.includes("display-name") ? input : avatar),
            }),
        },
        avatarImage: () => "random.webp",
        avatarImageAttribute, avatarBackgroundCss, setAvatarBackground, setAvatarImage,
        SOW_t: (key) => key,
    });
    refresh();
    assert.equal(input.value, "editing");
    assert.match(avatar.style.backgroundImage, /random.webp/);
    document.activeElement = null;
    refresh();
    assert.equal(input.value, "server-name");
    assert.doesNotMatch(shellSource.slice(start, end), /sign_in|createElement|innerHTML|replaceWith/);
});

test("reduced motion suppresses counter effects after splitting the header", () => {
    const start = shellSource.indexOf("export function pulseRewardCounter(");
    const end = shellSource.indexOf("function acknowledgeShownReceipts", start);
    const pulse = vm.runInNewContext(
        shellSource.slice(start, end).replace(/export /g, "") + "\npulseRewardCounter;",
        {
            reducedRewardMotion: () => true,
            root: {
                querySelector: () => {
                    throw new Error("reduced motion must not animate counters");
                },
            },
        },
    );
    pulse("[data-progression-level-value]", true);
    assert.match(
        menuCss["main_menu.base.css"],
        /\[data-progression\] \[data-progression-crowns-value\]\.is-reward-value-hit/,
    );
});

test("heroes skill scales its title above its body, and skins live with the roster filters", () => {
    assert.match(
        profileCss,
        /\.sow-heroes__featured\s*\{[^}]*--skill-title:\s*calc\(var\(--skill-body\) \* 1\.5\)/,
    );
    assert.match(
        profileCss,
        /\.sow-heroes__featured \.sow-hero-briefing__ability-name\s*\{\s*font-size:\s*var\(--skill-title\)/,
    );
    assert.match(
        profileCss,
        /\.sow-heroes__featured \.sow-hero-briefing__description\s*\{\s*font-size:\s*var\(--skill-body\)/,
    );
    assert.match(
        profileCss,
        /\.sow-heroes__featured \.sow-hero-briefing__icon-slot\s*\{[^}]*width:\s*var\(--skill-icon\)/,
    );
    const bodies = [...profileCss.matchAll(/--skill-body:\s*(\d+)px/g)].map((match) => Number(match[1]));
    assert.ok(bodies.length >= 5, "every heroes breakpoint sets its own body size");
    assert.ok(Math.min(...bodies) >= 11, "no heroes breakpoint drops the skill text below 11px");
    assert.ok(Math.max(...bodies) >= 14, "desktop keeps the largest skill text");

    const store = fs.readFileSync(path.join(shell, "main_menu.store.ts"), "utf8");
    const leaderStart = store.indexOf("export function renderLeaderPurchase(");
    const skinsStart = store.indexOf("export function renderSkinsButton(", leaderStart);
    const skinsEnd = store.indexOf("export function renderSkinPickerModal", skinsStart);
    assert.ok(leaderStart >= 0 && skinsStart > leaderStart && skinsEnd > skinsStart);
    assert.doesNotMatch(
        store.slice(leaderStart, skinsStart),
        /open_skin_picker/,
        "a hero's buy row no longer carries the skins entry",
    );
    assert.match(store.slice(skinsStart, skinsEnd), /data-command='open_skin_picker'/);
    const catalogLoader = store.slice(store.indexOf("export function loadStoreCatalog()"));
    assert.match(catalogLoader, /free_rotation: offer\.free_rotation/);
    assert.match(catalogLoader, /send\("sync_free_rotation"/);
    assert.doesNotMatch(
        catalogLoader,
        /free_rotation:\s*local\.free_rotation/,
        "the weekly free lineup comes from the server catalog",
    );
    assert.match(
        shellSource.slice(shellSource.indexOf("function handleMenuStateUpdate(")),
        /phase === "MainMenu" && menuUi\.state!\.profile_ready\) loadStoreCatalog\(\)/,
        "the persisted rotation loads before the player opens the store",
    );
    assert.match(
        heroesSource,
        /renderHeroesRegionDropdown\(\) \+\s*renderSkinsButton\(\)/,
        "skins sit beside the roster filters",
    );
    for (const file of ["main_menu.surfaces.ts", "main_menu.poki.ts", "main_menu.jest.ts"]) {
        assert.match(fs.readFileSync(path.join(shell, file), "utf8"), /renderSkinsButton,/, file);
    }
    assert.match(
        fs.readFileSync(path.join(shell, "main_menu.portal-stubs.ts"), "utf8"),
        /export function renderSkinsButton\(\) \{\s*return "";/,
        "portal builds without the store keep the entry hidden",
    );
});

test("reward landings light their header plate and escalate above the curtain", () => {
    const start = shellSource.indexOf("function restartRewardClass(");
    const end = shellSource.indexOf("function acknowledgeShownReceipts", start);
    const element = (name) => {
        const node = {
            name,
            classes: new Set(),
            children: [],
            style: {
                setProperty(key, value) {
                    this[key] = value;
                },
            },
            offsetWidth: 1,
            classList: {
                add: (value) => node.classes.add(value),
                remove: (value) => node.classes.delete(value),
            },
            setAttribute() {},
            appendChild: (child) => node.children.push(child),
            getBoundingClientRect: () => ({ left: 10, top: 5, width: 20, height: 10 }),
            remove() {},
        };
        return node;
    };
    const nameplate = element("nameplate");
    const pill = element("pill");
    const counters = {
        "[data-progression-level-value]": Object.assign(element("level"), { closest: () => nameplate }),
        "[data-progression-crowns-value]": Object.assign(element("crowns"), {
            closest: (selector) => (selector.includes(".sow-menu__currency") ? pill : null),
        }),
    };
    let reduced = false;
    const layer = element("layer");
    const context = {
        reducedRewardMotion: () => reduced,
        activeRewardStage: layer,
        root: { querySelector: (selector) => counters[selector] || null },
        document: { createElement: element },
        window: { setTimeout: () => 0 },
        Math,
        String,
        SOW_t: (key, values) => key + ":" + values.level,
    };
    const api = vm.runInNewContext(
        shellSource.slice(start, end).replace(/export /g, "") + "\n({pulseRewardCounter, playRewardImpact});",
        context,
    );
    api.pulseRewardCounter("[data-progression-crowns-value]", false);
    assert.ok(counters["[data-progression-crowns-value]"].classes.has("is-reward-value-hit"));
    assert.ok(pill.classes.has("is-reward-hit"), "the currency pill flashes with its counter");
    api.pulseRewardCounter("[data-progression-level-value]", true);
    assert.ok(nameplate.classes.has("is-level-up"), "a level-up flashes the whole nameplate");

    api.playRewardImpact("[data-progression-level-value]", "level", 1.6, 4);
    const impact = layer.children.at(-1);
    assert.match(impact.className, /sow-menu__reward-impact--level/);
    assert.equal(impact.style["--fx-power"], "1.6");
    assert.equal(impact.children.filter((part) => part.className.endsWith("-spark")).length, 12);
    assert.equal(impact.children.at(-1).textContent, "profile.level:4");
    api.playRewardImpact("[data-progression-crowns-value]", "crowns", 1);
    assert.equal(
        layer.children.at(-1).children.some((part) => part.className.endsWith("-rays")),
        false,
    );

    reduced = true;
    const drawn = layer.children.length;
    api.playRewardImpact("[data-progression-level-value]", "level", 1, 2);
    assert.equal(layer.children.length, drawn, "reduced motion draws no arrival light");

    const base = menuCss["main_menu.base.css"];
    assert.match(
        base,
        /\.sow-menu__xp-track\.is-filling i,\s*\.sow-menu__nameplate\.is-level-up \.sow-menu__xp-track i \{\s*transition: none/,
    );
    assert.match(
        base,
        /\.sow-menu__nameplate\.is-level-up \.sow-menu__xp-track::before \{[^}]*animation: sow-xp-flash/,
    );
    assert.match(
        base,
        /@keyframes sow-xp-flash \{\s*from \{\s*opacity: 0\.95/,
        "the reset happens under a full flash",
    );
    assert.match(base, /data-reduced-motion="true"\] \.sow-menu__topbar \*::before/);
});

test("header sizing shares its grid height, clips XP to the track, and compacts narrow landscape windows", () => {
    const base = menuCss["main_menu.base.css"];
    const layout = menuCss["main_menu.layout.css"];
    assert.match(base, /--sow-menu-header-height: 56px/);
    assert.match(base, /\.sow-menu__topbar\s*\{[^}]*min-height: var\(--sow-menu-header-height\)/);
    assert.match(layout, /grid-template-rows: var\(--sow-menu-header-height\) minmax\(0, 1fr\) auto/);
    const compactStart = layout.indexOf("@media (max-width: 680px) {", layout.indexOf("/* Header height"));
    assert.ok(compactStart > 0, "narrow layouts compact without depending on orientation");
    assert.match(layout.slice(compactStart), /\.sow-menu__top-actions\s*\{[^}]*gap: 4px/);
    assert.doesNotMatch(layout.slice(compactStart), /grid-template-rows: auto auto/);
    assert.match(base, /\.sow-menu__xp-track\s*\{[^}]*overflow: hidden/);
    assert.match(
        base,
        /\.sow-menu__currency strong\s*\{[^}]*max-width: 6ch;[^}]*overflow: hidden;[^}]*text-overflow: ellipsis/,
    );
    assert.match(base, /data-reduced-motion="true"[^}]*transition: none;[^}]*animation: none/);
    assert.doesNotMatch(layout, /--sow-topbar-button: 40px/);
});

test("account balances share compact totals with match results and retain full accessible amounts", () => {
    assert.match(shellSource, /import \{ formatCompactNumber \} from "\.\/number-format\.ts"/);
    assert.match(
        hudSource,
        /import \{ formatCompactNumber as formatEndgameCompact \} from "\.\/number-format\.ts"/,
    );
    for (const [value, text] of [
        [0, "0"],
        [475, "475"],
        [1000, "1K"],
        [1250, "1.3K"],
        [12345, "12.3K"],
        [999950, "1M"],
        [1500000, "1.5M"],
    ]) {
        assert.equal(formatCompactNumber(value), text);
    }
});

test("compact account header bounds bars, padding and balance glyphs without shrinking amounts to 11px", () => {
    const base = menuCss["main_menu.base.css"];
    const layout = menuCss["main_menu.layout.css"];
    assert.match(base, /\.sow-menu__topbar\s*\{[^}]*width: 100%/);
    assert.match(base, /\.sow-menu__nameplate\s*\{[^}]*grid-template-rows: auto auto;[^}]*padding: 0;/);
    assert.match(base, /--sow-topbar-pill-text: 18px/);
    assert.match(layout, /--sow-topbar-pill-text: 16px/);
    assert.match(layout, /--sow-topbar-pill-text: 14px/);
    assert.doesNotMatch(
        base + layout,
        /--sow-topbar-pill-min|--sow-topbar-plate-height|--sow-topbar-plate-width/,
    );
    assert.match(base, /\.sow-menu__currency\s*\{[^}]*justify-content: flex-start;[^}]*width: auto/);
    for (const value of [
        0,
        999,
        1000,
        999949,
        999950,
        999949000,
        999950000,
        1e9,
        1e12,
        1e15,
        1e18,
        Number(2n ** 64n - 1n),
    ]) {
        const text = formatCompactNumber(value);
        assert.ok(text.length <= 6, `${value}: ${text}`);
        assert.ok((text.match(/\d/g) || []).length <= 4, text);
    }
    assert.equal(formatCompactNumber(999950000), "1B");
    assert.equal(formatCompactNumber(1e12), "1T");
});

test("main navigation keeps a single outer frame across battle, profile, heroes and store", () => {
    const start = shellSource.indexOf("function renderFrame(");
    const end = shellSource.indexOf("function panelFromMarkup(", start);
    const renderFrame = vm.runInNewContext(shellSource.slice(start, end) + "\nrenderFrame;", {
        screenBackdropClass: (screen) => "backdrop-" + screen,
        renderTopbar: () => "<header data-progression></header>",
        renderScreenPanel: (screen) => "<main data-screen-panel='" + screen + "'></main>",
        renderMainNav: () => "<nav></nav>",
        screenNav: (screen) => screen,
        renderMenuAttribution: () => "",
        renderPasswordModal: () => "",
        renderProfileDetail: () => "",
    });
    for (const screen of ["home", "profile", "heroes", "store"]) {
        const markup = renderFrame(screen);
        assert.match(markup, /<div class='sow-menu__shell'><header data-progression>/, screen);
        assert.match(markup, new RegExp("data-screen-panel='" + screen + "'"));
    }
    assert.doesNotMatch(
        profileCss,
        /\.sow-menu__shell/,
        "screen styles must not resize the common header and navigation container",
    );
    assert.doesNotMatch(
        menuCss["main_menu.layout.css"],
        /width: min\([^;]*680px\)/,
        "battle must not retain a narrower outer frame on tablets",
    );
    const chrome = shellSource.slice(
        shellSource.indexOf("function updateFrameChrome("),
        shellSource.indexOf("export function activeScreenPanel("),
    );
    assert.doesNotMatch(chrome, /shell\.className|screenShellClass/);
});
