const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const shell = __dirname;
const coreSource = fs.readFileSync(path.join(shell, "main_menu.core.js"), "utf8");
const shellSource = fs.readFileSync(path.join(shell, "main_menu.shell.js"), "utf8");
const storeSource = fs.readFileSync(path.join(shell, "main_menu.store.js"), "utf8");
const heroesSource = fs.readFileSync(path.join(shell, "main_menu.heroes.js"), "utf8");
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
const loaderSource = fs.readFileSync(path.join(shell, "loader.js"), "utf8");
const pokiSource = fs.readFileSync(path.join(shell, "main_menu.poki.js"), "utf8");
const licenseJestSource = fs.readFileSync(path.join(shell, "main_menu.jest.js"), "utf8");
const licenseMenuLayout = fs.readFileSync(path.join(shell, "main_menu.layout.css"), "utf8");
const lobbiesSource = fs.readFileSync(path.join(shell, "main_menu.lobbies.js"), "utf8");
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const campaignEngine = fs.readFileSync(path.join(shell, "sow-campaign.js"), "utf8");
const campaignView = fs.readFileSync(path.join(shell, "sow-campaign-view.js"), "utf8");
const storyCss = fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
const campaignEditor = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/editor.js"), "utf8");
const localeRuntime = fs.readFileSync(path.join(shell, "sow-i18n.js"), "utf8");
const campaignEditorServer = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/server.mjs"), "utf8");
const campaignEditorHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/logic.html"), "utf8");
const campaignEditorCss = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/editor.css"), "utf8");
const mapRostersHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/map-rosters.html"), "utf8");
const atlasEntityPickerSource = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/atlas-entity-picker.js"), "utf8");
const mapRosterWriter = fs.readFileSync(path.join(shell, "../../sow-tools/src/map_roster.rs"), "utf8");
const gameFonts = fs.readFileSync(path.join(shell, "../../sow-web/site/fonts/fonts.css"), "utf8");
const landingHtml = fs.readFileSync(path.join(shell, "../../sow-web/site/index.html"), "utf8");
const siteDistSource = fs.readFileSync(path.join(shell, "../../sow-dist/src/main.rs"), "utf8");
const siteHeader = fs.readFileSync(path.join(shell, "../../sow-web/site/site-header.html"), "utf8");
const siteChrome = fs.readFileSync(path.join(shell, "../../sow-web/site/site-chrome.js"), "utf8");
const siteDropdown = fs.readFileSync(path.join(shell, "sow-dropdown.js"), "utf8");
const gameControlsCss = fs.readFileSync(path.join(shell, "sow-controls.css"), "utf8");
const siteApp = fs.readFileSync(path.join(shell, "../../sow-web/site/app.js"), "utf8");
const siteCss = fs.readFileSync(path.join(shell, "../../sow-web/site/styles.css"), "utf8");
const siteFooterPages = ["support", "privacy", "terms", "cookies"].map((page) =>
    fs.readFileSync(path.join(shell, `../../sow-web/site/${page}/index.html`), "utf8")
);
const siteRedirectConfig = fs.readFileSync(path.join(shell, "../../sow-dist/deploy/freebsd/conf.d/shadowsofwar.io.conf"), "utf8");
const campaignMapEditorHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/index.html"), "utf8");
const campaignMapPreview = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/map-preview.js"), "utf8");
const campaignMapPreviewApi = require(path.join(shell, "../../sow-tools/editors/campaign-editor/map-preview.js"));
const atlasHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/atlas.html"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");
const hudCss = fs.readFileSync(path.join(shell, "main_menu.hud.css"), "utf8");
const indexTemplate = fs.readFileSync(path.join(shell, "index.html.template"), "utf8");
const windowInput = fs.readFileSync(path.join(shell, "../../sow-client/src/input/window.rs"), "utf8");
const intentInput = fs.readFileSync(path.join(shell, "../../sow-client/src/input/intents.rs"), "utf8");
const hudStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/ui/hud/state.rs"), "utf8");
const avatarIdentitySource = fs.readFileSync(path.join(shell, "../../sow-core/src/player/mod.rs"), "utf8");
const matchStartSource = fs.readFileSync(path.join(shell, "../../sow-client/src/loader/match_start.rs"), "utf8");
const campaignModule = fs.readFileSync(path.join(shell, "../../sow-client/src/campaign/mod.rs"), "utf8");
const sessionSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/session.rs"), "utf8");
const netMessagesSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/update/messages.rs"), "utf8");
const simUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/update.rs"), "utf8");
const simEventsSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/events/mod.rs"), "utf8");
const eliminationSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/events/elimination.rs"), "utf8");
const snapshotFxSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/snapshot_fx.rs"), "utf8");
const actionsSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/interact/actions.rs"), "utf8");
const surfaceSource = fs.readFileSync(path.join(shell, "../../sow-client/src/input/surface.rs"), "utf8");
const cameraFrameUiSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/frame/ui.rs"), "utf8");
const netUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/update/mod.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const appStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/state.rs"), "utf8");
const buildingOverlaySource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/world/overlays.rs"), "utf8");
const nameplatesSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/world/nameplates.rs"), "utf8");
const progressSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/progress.rs"), "utf8");
const dataDbSource = fs.readFileSync(path.join(shell, "../../sow-data/src/db.rs"), "utf8");
const dataMainSource = fs.readFileSync(path.join(shell, "../../sow-data/src/main.rs"), "utf8");
const accountSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/account.rs"), "utf8");
const bootstrapSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/bootstrap.rs"), "utf8");
const identitySource = fs.readFileSync(path.join(shell, "../../sow-client/src/anonymous_identity.rs"), "utf8");
const assetSource = fs.readFileSync(path.join(shell, "../../sow-client/src/asset.rs"), "utf8");
const relaySource = fs.readFileSync(path.join(shell, "../../sow-relay/src/main.rs"), "utf8");
const serverSource = fs.readFileSync(path.join(shell, "../../sow-server/src/main.rs"), "utf8");
const mapClick = fs.readFileSync(path.join(shell, "../../sow-client/src/input/map_click.rs"), "utf8");
const frameSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/frame/mod.rs"), "utf8");
const mapShader = fs.readFileSync(path.join(shell, "../../sow-render/src/shaders/map.wgsl"), "utf8");
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

function renderSettingsBody(source, endMarker) {
    const start = source.indexOf("function renderSettings()");
    const end = source.indexOf(endMarker, start);
    assert.notEqual(start, -1);
    assert.notEqual(end, -1);
    return source.slice(start, end);
}

test("public landing keeps the video hero, interactive leader roster, community links, and FAQ", () => {
    assert.match(landingHtml, /<section class="hero section-rule">[\s\S]*class="hero-video"[\s\S]*class="hero-copy"[\s\S]*href="\/play\/"[\s\S]*href="#media"[\s\S]*<\/section>/);
    assert.doesNotMatch(landingHtml, /hero-stage|data-leader-rail/);
    assert.match(siteApp, /new IntersectionObserver\(/);
    assert.match(siteApp, /document\.visibilityState === 'hidden' \|\| !heroVisible/);
    assert.match(siteApp, /const cards = \$\$\('\.leader-card\[data-leader-id\]'\)/);
    assert.match(siteApp, /const leaders = cards\.map/);
    assert.match(landingHtml, /id="faq"[\s\S]*class="faq-list"/);
    const updates = landingHtml.slice(landingHtml.indexOf('<div class="footer-updates">'), landingHtml.indexOf('<div class="footer-top">'));
    assert.match(updates, /href="https:\/\/discord\.gg\//);
    assert.match(updates, /href="https:\/\/t\.me\//);
    assert.doesNotMatch(updates, /disabled|type="email"/);
    assert.match(siteCss, /\.hero-grid[^\n]*760px/);
    assert.match(siteCss, /\.faq-list \{ display: grid; grid-template-columns: 1fr;/);
});

test("CrazyGames menu links open the matching pages on the public site", () => {
    const helper = shellSource.match(/function renderSiteLink\(path, key, className\) \{[\s\S]*?\n    \}/)?.[0];
    assert.ok(helper, "main menu has no shared site-link renderer");
    const portal = { SOW_PORTAL: "crazygames" };
    const render = vm.runInNewContext(`(${helper})`, {
        window: portal,
        SOW_t: key => key,
        esc: value => String(value)
    });
    assert.equal(
        render("/terms/", "auth.terms", "sow-auth__terms"),
        "<a class='sow-auth__terms' href='https://shadowsofwar.io/terms/' target='_blank' rel='noopener noreferrer'>auth.terms</a>"
    );
    portal.SOW_PORTAL = "site";
    assert.equal(render("/terms/", "menu.terms"), "<a href='/terms/'>menu.terms</a>");

    for (const [path, key] of [
        ["/#faq", "site.faq"],
        ["/support/", "menu.support"],
        ["/terms/", "menu.terms"],
        ["/privacy/", "menu.privacy"],
        ["/cookies/", "menu.cookies"]
    ]) {
        assert.ok(shellSource.includes(`renderSiteLink("${path}", "${key}")`), `${path} is not routed through the shared link renderer`);
    }
    assert.ok(shellSource.includes('renderSiteLink("/terms/", "auth.terms", "sow-auth__terms")'));
});

test("CrazyGames font stylesheet and server keep the same cross-origin path", () => {
    assert.ok(siteDistSource.includes('"https://shadowsofwar.io/fonts/fonts.css"'));
    assert.match(siteRedirectConfig, /location \^~ \/fonts\/ \{\s*add_header Access-Control-Allow-Origin \*;\s*try_files \$uri =404;/);
});

test("landing alpha badges link to the iOS and Android tests in every locale", () => {
    const alpha = landingHtml.slice(landingHtml.indexOf('class="hero-alpha"'), landingHtml.indexOf('</div>\n        </div>', landingHtml.indexOf('class="hero-alpha"')));
    assert.match(alpha, /aria-labelledby="hero-alpha-label"/);
    assert.match(alpha, /data-i18n="site\.mobile_alpha_testing"/);
    assert.match(alpha, /href="https:\/\/testflight\.apple\.com\/join\/Y8fu4zch" target="_blank" rel="noopener"[\s\S]*?src="\/assets\/site\/media\/testflight-badge\.svg"[\s\S]*?data-i18n="site\.testflight_badge_action"[\s\S]*?TestFlight/);
    assert.match(alpha, /href="https:\/\/play\.google\.com\/apps\/testing\/com\.shadowsofwar" target="_blank" rel="noopener"[\s\S]*?src="\/assets\/site\/media\/google-play-badge\.svg"[\s\S]*?data-i18n="site\.google_play_badge_action"[\s\S]*?Google Play/);
    assert.doesNotMatch(alpha, /join_alpha|JOIN THE ALPHA/i);
    const siteMedia = path.join(shell, "../../assets/site/media");
    for (const asset of ["testflight-badge.svg", "google-play-badge.svg"]) {
        assert.ok(fs.existsSync(path.join(siteMedia, asset)), `missing local vector asset: ${asset}`);
        assert.match(fs.readFileSync(path.join(siteMedia, asset), "utf8"), /<svg\b/);
    }
    const playMark = fs.readFileSync(path.join(siteMedia, "google-play-badge.svg"), "utf8");
    for (const color of ["#4285F4", "#34A853", "#FBBC04", "#EA4335"]) assert.ok(playMark.includes(color), `Google Play mark is missing ${color}`);
    assert.match(siteDistSource, /copy_dir\(&paths\.assets_site\.join\("media"\), &assets\.join\("site\/media"\)\)/);
    assert.match(siteCss, /\.hero-alpha-links \{ display: grid; grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);/);
    assert.match(siteCss, /\.hero-alpha-links \{ display: grid; grid-template-columns: repeat\(2, minmax\(0, 1fr\)\); gap: 6\.5px; max-width: 299px; \}/);
    assert.match(siteCss, /\.hero-alpha-badge \{ display: flex; align-items: center; gap: 8px; min-width: 0; min-height: 46px;/);
    assert.match(siteCss, /\.hero-alpha-icon \{ flex: 0 0 30px; width: 30px; height: 30px; \}/);
    const narrowStyles = siteCss.slice(siteCss.indexOf("@media (max-width: 479px)"), siteCss.indexOf("@media (orientation: landscape)"));
    assert.match(narrowStyles, /\.hero-alpha-links \{ grid-template-columns: repeat\(2, minmax\(0, 1fr\)\); gap: 6px; max-width: none; \}/);
    assert.match(narrowStyles, /\.hero-alpha-badge \{ min-height: 42px; gap: 6px; padding-inline: 6px; \}/);
    assert.match(narrowStyles, /\.hero-alpha-icon \{ flex-basis: 27px; width: 27px; height: 27px; \}/);
    assert.match(narrowStyles, /\.hero-alpha-copy b \{ font-size: 12px; \}/);
    const locales = path.join(shell, "../../sow-i18n/strings");
    for (const locale of fs.readdirSync(locales)) {
        const file = path.join(locales, locale, "web.toml");
        if (!fs.existsSync(file)) continue;
        const source = fs.readFileSync(file, "utf8");
        assert.match(source, /^mobile_alpha_testing\s*=\s*".+"$/m, `${locale} is missing the alpha label`);
        assert.match(source, /^testflight_badge_action\s*=\s*".+"$/m, `${locale} is missing the TestFlight badge action`);
        assert.match(source, /^google_play_badge_action\s*=\s*".+"$/m, `${locale} is missing the Google Play badge action`);
        assert.doesNotMatch(source, /^join_alpha\s*=/m, `${locale} has the removed redundant alpha action`);
    }
});

test("retired marketing pages stay deleted and redirect to the landing", () => {
    const siteRoot = path.join(shell, "../../sow-web/site");
    assert.equal(fs.existsSync(path.join(siteRoot, "leaders/index.html")), false);
    assert.equal(fs.existsSync(path.join(siteRoot, "how-to-play/index.html")), false);
    assert.doesNotMatch(siteHeader, /href=["']\/(?:leaders|how-to-play)\//);
    assert.doesNotMatch(siteHeader, /href=["']\/#(?:leaders|faq)["']/);
    const mobileMenu = siteHeader.match(/<nav class="mobile-menu"[\s\S]*?<\/nav>/)?.[0] ?? "";
    for (const [href, key] of [["/", "nav_home"], ["/support/", "support"], ["/privacy/", "privacy"], ["/terms/", "terms"], ["/cookies/", "cookies"]]) {
        assert.match(mobileMenu, new RegExp(`href="${href}" data-i18n="site\\.${key}"`));
    }
    assert.match(mobileMenu, /<div class="mobile-menu-actions">\s*<div class="site-locale" data-locale-select[\s\S]*?<\/div>\s*<\/div>\s*<a href="\/" data-i18n="site\.nav_home">Home<\/a>/);
    const navInnerCss = siteCss.match(/\.nav-inner\s*\{([^}]+)\}/)?.[1] ?? "";
    assert.doesNotMatch(navInnerCss, /\b(?:border|border-radius|background|box-shadow|backdrop-filter)\s*:/);
    const dropdownTriggerCss = gameControlsCss.match(/\.sow-control-dropdown__trigger\s*\{([^}]+)\}/)?.[1] ?? "";
    assert.match(dropdownTriggerCss, /border: 1px solid var\(--sow-button-dark-border\)/);
    assert.match(dropdownTriggerCss, /border-radius: var\(--sow-button-radius\)/);
    assert.match(dropdownTriggerCss, /background: var\(--sow-button-dark-bg\)/);
    assert.match(siteCss, /\.site-locale-dropdown \.sow-control-dropdown__trigger\s*\{\s*height: 42px;\s*\}/);
    const mobileMenuCss = siteCss.match(/\.mobile-menu\s*\{(?=[^}]*position: absolute)([^}]+)\}/)?.[1] ?? "";
    assert.match(mobileMenuCss, /background: var\(--panel\)/);
    assert.match(mobileMenuCss, /box-shadow: var\(--surface-shadow\)/);
    const mobileActionsCss = siteCss.match(/\.mobile-menu-actions \{[^}]+\}/)?.[0] ?? "";
    assert.match(mobileActionsCss, /padding-bottom: 2px;/);
    assert.match(mobileActionsCss, /border-bottom: 1px solid var\(--line\);/);
    assert.doesNotMatch(mobileActionsCss, /padding-top|border-top/);
    assert.match(siteCss, /\.mobile-menu-actions \.site-locale \.sow-control-dropdown__trigger \{ height: 48px; \}/);
    assert.match(siteCss, /\.site-locale-dropdown \.sow-control-dropdown__menu:not\(\[hidden\]\) \{\s*max-height: min\(560px, calc\(100dvh - 140px\)\);\s*\}/);
    assert.doesNotMatch(siteChrome, /syncCurrentNavigation/);
    assert.match(siteChrome, /control\.innerHTML = window\.SOW_renderDropdown/);
    assert.match(siteChrome, /window\.SOW_I18N_READY\.then\(applyLocale\)/);
    assert.match(landingHtml, /id="leaders"/);
    assert.match(landingHtml, /href="\/#faq"/);
    for (const html of siteFooterPages) {
        assert.doesNotMatch(html, /href=["']\/(?:leaders|how-to-play)\//);
    }
    for (const html of [landingHtml, ...siteFooterPages]) {
        const footer = html.match(/<div class="footer-links">([\s\S]*?)<\/div>/)?.[1] ?? "";
        assert.ok(footer);
        assert.doesNotMatch(footer, /site\.nav_leaders|href=["']\/#leaders["']/);
    }
    assert.ok(shellSource.includes('renderSiteLink("/#faq", "site.faq")'));
    assert.doesNotMatch(shellSource, /\/how-to-play\//);
    assert.ok(siteRedirectConfig.includes("location ~ ^/(leaders|how-to-play)/?$"));
    assert.ok(siteRedirectConfig.includes("return 301 https://shadowsofwar.io/;"));
    assert.doesNotMatch(siteCss, /compendium-|stat-meters-grid|stat-meter/);
});

test("mobile navigation and footers reuse the same localization key per link", () => {
    const mobileMenu = siteHeader.match(/<nav class="mobile-menu"[\s\S]*?<\/nav>/)?.[0] ?? "";
    const localizedLinks = (markup) => new Map(
        Array.from(markup.matchAll(/<a\b([^>]*)>/g), ([, attributes]) => {
            const href = attributes.match(/\bhref=["']([^"']+)["']/)?.[1];
            const key = attributes.match(/\bdata-i18n=["']([^"']+)["']/)?.[1];
            return href && key ? [href, key] : null;
        }).filter(Boolean)
    );
    const mobileLinks = localizedLinks(mobileMenu);

    for (const [href, mobileKey] of mobileLinks) {
        const footerKeys = [landingHtml, ...siteFooterPages].flatMap((html) => {
            const footer = html.match(/<div class="footer-links">([\s\S]*?)<\/div>/)?.[1] ?? "";
            const key = localizedLinks(footer).get(href);
            return key ? [key] : [];
        });
        assert.ok(footerKeys.length, `footer has no localized link for ${href}`);
        assert.ok(footerKeys.every((key) => key === mobileKey), `${href} uses different menu/footer localization keys`);
    }
});

test("AGPL legal notice links to each release license and stays visible in the compact footer", () => {
    for (const [platform, source] of [["standard", shellSource], ["Poki", pokiSource], ["Jest", licenseJestSource]]) {
        assert.match(source, /sow-menu__license-link/, `${platform} menu has no license link`);
        assert.match(source, /menu\.license_notice/, `${platform} menu has no localized legal notice`);
        assert.equal((source.match(/© OpenFront and Contributors/g) || []).length, 1, `${platform} credit must appear once`);
    }
    assert.match(shellSource, /sourceUrl\.replace\("\/tree\/", "\/blob\/"\) \+ "\/LICENSE"/);
    assert.match(licenseJestSource, /sourceUrl\.replace\("\/tree\/", "\/blob\/"\) \+ "\/LICENSE"/);
    assert.match(pokiSource, /function pokiLicenseUrl\(\)[\s\S]*sourceUrl\.replace\("\/tree\/", "\/blob\/"\) \+ "\/LICENSE"/);
    assert.match(pokiSource, /data-command='poki_license'/);
    assert.match(pokiSource, /window\.SOW_pokiOpenExternalLink\(licenseUrl\)/);
    assert.match(licenseMenuLayout, /:not\(\.sow-menu__source-link\):not\(\.sow-menu__license-link\)/);

    const localeRoot = path.join(shell, "../../sow-i18n/strings");
    const localeFiles = fs.readdirSync(localeRoot)
        .map((locale) => path.join(localeRoot, locale, "web.toml"))
        .filter((file) => fs.existsSync(file));
    assert.equal(localeFiles.length, 15);
    for (const file of localeFiles) {
        assert.match(fs.readFileSync(file, "utf8"), /^license_notice = ".+"$/m, `${file} lacks the notice translation`);
    }
});

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

test("landing background video follows hero visibility and motion settings", () => {
    let observeHero;
    let onVisibilityChange;
    let onMotionChange;
    let plays = 0;
    let pauses = 0;
    const hero = {};
    const video = {
        closest: () => hero,
        play() { plays += 1; return Promise.resolve(); },
        pause() { pauses += 1; }
    };
    const motion = {
        matches: false,
        addEventListener: (_event, callback) => { onMotionChange = callback; }
    };
    class IntersectionObserverMock {
        constructor(callback) { observeHero = callback; }
        observe(target) { assert.equal(target, hero); }
    }
    const window = {
        IntersectionObserver: IntersectionObserverMock,
        matchMedia: () => motion,
        addEventListener() {}
    };
    const document = {
        visibilityState: "visible",
        querySelector: selector => selector === ".hero-video" ? video : null,
        querySelectorAll: () => [],
        addEventListener: (_event, callback) => { onVisibilityChange = callback; }
    };

    vm.runInNewContext(siteApp, {
        window,
        document,
        IntersectionObserver: IntersectionObserverMock,
        location: { hostname: "localhost" },
        sessionStorage: { getItem: () => null, setItem() {} }
    });
    assert.equal(pauses, 1, "starts paused until the hero is observed");

    observeHero([{ isIntersecting: true }]);
    assert.equal(plays, 1);
    document.visibilityState = "hidden";
    onVisibilityChange();
    assert.equal(pauses, 2);
    document.visibilityState = "visible";
    onVisibilityChange();
    assert.equal(plays, 2);
    motion.matches = true;
    onMotionChange();
    assert.equal(pauses, 3);
    motion.matches = false;
    onMotionChange();
    assert.equal(plays, 3);
    observeHero([{ isIntersecting: false }]);
    assert.equal(pauses, 4);
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

test("match rewards present in the main menu, while endgame keeps only match stats", () => {
    assert.match(hud, /id="sow-hud-endgame-modal"/);
    assert.match(hud, /endgame\.classList\.toggle\("hidden", !isOver\)/);
    assert.match(webMenu, /endgame_active,[\s\S]*endgame_winner:[\s\S]*endgame_team:/);
    assert.match(hud, /id="sow-hud-surrender-kda"/);
    assert.doesNotMatch(hud, /sow-hud-endgame-(xp|leader-xp|crowns|laurels|rewards)/);
    assert.doesNotMatch(sessionSource, /submit_online_stats_on_exit/);
    assert.doesNotMatch(progressSource, /submit_online_stats|SubmitMatchReport|SubmitStatsWithLeader/);
    assert.match(progressSource, /capture_online_reward_preview[\s\S]*RewardInput::default\(\)[\s\S]*preview_participation_laurels/);
    assert.doesNotMatch(progressSource, /reward_cache/);
    assert.match(dataDbSource, /pub async fn record_match_participation/);
    assert.match(dataDbSource, /reward_receipts\.contains_key/);
    assert.doesNotMatch(shellSource, /data-reward-summary|reward-toast/);
    assert.match(shellSource, /sow:loader-cycle-ready/);
    assert.match(shellSource, /runRewardPresentation/);
    assert.match(shellSource, /createRewardStage\(stages/);
    assert.match(shellSource, /layer\.addEventListener\("click", function \(event\) \{\s*event\.preventDefault\(\);\s*event\.stopPropagation\(\);\s*skip\(\);\s*\}\);/);
    assert.match(shellSource, /createRewardStage\(stages, function \(\) \{ finish\(true\); \}\)/);
    assert.match(shellSource, /revealRewardAmounts\(items, token\)/);
    assert.match(shellSource, /flyRewardCard\(item, token\)\.then[\s\S]*animateProgressionTo\(nextValues, 380, token\)/);
    assert.match(shellSource, /2 \* inverse \* t \* controlX \+ t \* t \* dx/);
    assert.match(shellSource, /gems: from\.gems \+ \(target\.gems - from\.gems\) \* eased/);
    assert.doesNotMatch(shellSource, /writeProgression\(Object\.assign\(\{\}, displayedProgression, \{ gems: Number\(state\.gems\)/);
    assert.match(shellSource, /function pulseRewardCounter\(selector, levelUp\)/);
    assert.match(menuCss["main_menu.base.css"], /\.sow-menu__reward-stage/);
    assert.match(menuCss["main_menu.base.css"], /sow-reward-card-reveal/);
    assert.match(menuCss["main_menu.base.css"], /sow-reward-value-impact/);
    assert.match(shellSource, /acknowledge_reward_receipts/);
    assert.match(shellSource, /acknowledge_reward_presentation/);
    assert.match(shellSource, /send\("acknowledge_reward_receipts", \{ account_id: accountId, receipt_ids: ids \}\)/);
    assert.match(shellSource, /send\("acknowledge_reward_presentation", \{ account_id: accountId, receipt_id: preview\.receipt_id \}\)/);
    assert.match(webMenu, /AcknowledgeRewardReceipts \{[\s\S]*account_id,[\s\S]*receipt_ids,[\s\S]*progress_account_id\.as_deref\(\) == Some\(account_id\.as_str\(\)\)/);
    assert.match(webMenu, /AcknowledgeRewardPresentation \{[\s\S]*account_id,[\s\S]*receipt_id,[\s\S]*progress_account_id\.as_deref\(\) == Some\(account_id\.as_str\(\)\)/);
    assert.match(shellSource, /if \(!shown\) \{\s*writeProgression\(progressionFromState\(\)\);\s*if \(document\.visibilityState === "visible"\) maybePresentRewards\(\);/);
    assert.doesNotMatch(relaySource, /record_client_stats|record_match_report|SubmitStatsWithLeader|SubmitMatchReport/);
    assert.match(serverSource, /SubmitStatsWithLeader \{ \.\. \}[\s\S]*SubmitMatchReport \{ \.\. \} => \{\}/);
    assert.doesNotMatch(webMenu, /hud\.rewards/);
    assert.match(coreSource, /receipt\.leader_xp/);
    assert.match(coreSource, /return leaderArtUrl\(leader\.slug\);/);
    // The heroes featured art mapping is guarded by its own test:
    // "heroes featured art follows the panel shape, not the device name".
});

test("participation receipt stays synced until replay verification resolves", () => {
    assert.match(dataDbSource, /pub async fn mark_match_reward_verification_status/);
    assert.match(dataDbSource, /receipt\.verification_status = Some\(ReplayVerificationStatus::Verified\)/);
    assert.match(dataMainSource, /async fn reject_queued_replay/);
    assert.match(dataMainSource, /else if let Err\(error\) = reject_queued_replay\(&state, &payload\.match_id\)\.await/);
    assert.match(accountSource, /receipt\.verification_status\s*!=\s*Some\(sow_data::profile::ReplayVerificationStatus::Pending\)/);
    assert.match(accountSource, /receipt\.verification_status\s*==\s*Some\(sow_data::profile::ReplayVerificationStatus::Pending\)/);
    assert.match(coreSource, /receipt\.verification_status/);
});

test("pending reward lookups survive reload without crossing accounts", () => {
    assert.match(identitySource, /PENDING_REWARD_RECEIPTS_STORAGE_PREFIX/);
    assert.match(identitySource, /load_pending_reward_receipt_ids\(account_id: &str\)/);
    assert.match(accountSource, /track_reward_receipt_sync[\s\S]*save_pending_reward_receipt_ids/);
    assert.match(accountSource, /load_pending_reward_receipt_ids\(&account_id\)/);
    assert.match(accountSource, /save_pending_reward_receipt_ids\([\s\S]*&account_id,[\s\S]*&self\.pending_reward_receipt_ids/);
    assert.match(bootstrapSource, /stored_account_id[\s\S]*load_pending_reward_receipt_ids/);
});

test("queued identity refresh is applied before an older profile response", () => {
    const profileStart = assetSource.indexOf("DbEvent::ProfileLoaded {");
    const profileEnd = assetSource.indexOf("DbEvent::DisplayNameSaved", profileStart);
    assert.ok(profileStart >= 0 && profileEnd > profileStart);
    const profileEvent = assetSource.slice(profileStart, profileEnd);
    assert.match(profileEvent, /profile_refresh_pending[\s\S]*fetch_cloud_progress\(\);[\s\S]*continue;/);
    assert.ok(profileEvent.indexOf("continue;") < profileEvent.indexOf("profile_last_applied_request = request_id"));

    const failedStart = assetSource.indexOf("DbEvent::LoadFailed { request_id, status }");
    const failedEnd = assetSource.indexOf("DbEvent::TutorialCompletionFailed", failedStart);
    assert.ok(failedStart >= 0 && failedEnd > failedStart);
    assert.match(assetSource.slice(failedStart, failedEnd), /profile_refresh_pending[\s\S]*fetch_cloud_progress\(\);[\s\S]*continue;/);
});

test("lobby cards keep live counts and one DOM card per lobby id", () => {
    assert.match(lobbiesSource, /function lobbyPlayerCount\(lobby\)/);
    assert.match(lobbiesSource, /function lobbyStatusText\(lobby\)/);
    assert.match(lobbiesSource, /data-lobby-status-for/);
    assert.match(lobbiesSource, /data-lobby-count/);
    assert.match(lobbiesSource, /var seen = Object\.create\(null\)/);
    assert.match(lobbiesSource, /card\.remove\(\)/);
    assert.doesNotMatch(lobbiesSource, /sow-menu__lobby-join/);
    assert.doesNotMatch(shellSource, /cardTimers|data-timer-for|lobbyTimerText/);
    assert.match(lobbiesSource, /data-command='join_lobby'/);
    assert.match(lobbiesSource, /menu\.join/);
    assert.match(menuCss["main_menu.lobbies.css"], /\.sow-menu__lobby-count/);
    assert.doesNotMatch(menuCss["main_menu.lobbies.css"], /\.sow-menu__lobby-join/);
});

test("lobby exit keeps the live connection and match exit owns teardown", () => {
    assert.match(sessionSource, /fn leave_lobby_to_main_menu\(&mut self\)/);
    assert.match(sessionSource, /fn finish_lobby_exit_to_main_menu\(&mut self\)/);
    const lobbyActionStart = actionsSource.indexOf("UiAction::LeaveLobby =>");
    const matchActionStart = actionsSource.indexOf("UiAction::ReturnToMenu =>");
    assert.ok(lobbyActionStart >= 0 && matchActionStart > lobbyActionStart);
    const lobbyAction = actionsSource.slice(lobbyActionStart, matchActionStart);
    assert.match(lobbyAction, /leave_lobby_to_main_menu\(\)/);
    assert.doesNotMatch(lobbyAction, /begin_exit_to_main_menu|client = None|reconnect_now/);
    const matchAction = actionsSource.slice(matchActionStart);
    assert.match(matchAction, /send_leave_message\(\);[\s\S]*begin_exit_to_main_menu\(\);/);
    assert.match(webMenu, /WebMenuCommand::ReturnToMenu[\s\S]*UiAction::ReturnToMenu/);
    assert.doesNotMatch(hud, /send\("leave_lobby"\)/);
    assert.match(hud, /send\("return_to_menu"\)/);
    assert.doesNotMatch(sessionSource, /reconnect_now/);
    assert.doesNotMatch(netUpdateSource, /reconnect_now/);
});

test("game exit resets Battle and replays the screen entrance after the loader", () => {
    assert.match(coreSource, /var lastMenuPhase = null/);
    assert.match(coreSource, /var pendingExitScreenIntro = false/);
    const resetStart = shellSource.indexOf("if (returnedFromGame) {");
    const resetEnd = shellSource.indexOf("if (typeof window.SOW_tutorial_menu_state_update", resetStart);
    assert.ok(resetStart >= 0 && resetEnd > resetStart);
    const resetBlock = shellSource.slice(resetStart, resetEnd);
    for (const flag of ["profileOpen", "heroesOpen", "campaignOpen", "storeOpen"]) {
        assert.match(resetBlock, new RegExp(flag + " = false"));
    }
    assert.match(resetBlock, /pendingExitScreenIntro = true/);
    assert.match(shellSource, /sow:loader-cycle-ready/);
    assert.match(shellSource, /previousScreen = null;[\s\S]*render\(\);/);
    assert.match(loaderSource, /sow:loader-cycle-ready/);
    assert.match(loaderSource, /if \(!loaderReadyDispatched\)/);
    assert.ok(shellSource.indexOf("if (returnedFromGame)") < shellSource.indexOf("SOW_open_store_after_match"));
    assert.match(menuCss["main_menu.layout.css"], /sow-screen-panel-enter 240ms/);
});

test("first-run tutorial keeps one boot loader and splash art until Rust signals completion", async () => {
    assert.match(loaderSource, /window\.SOW_leaderArtUrl = leaderArtUrl/);
    assert.match(loaderSource, /window\.SOW_prepareLeaderArt = prepareLeaderArt/);
    assert.match(loaderSource, /if \(loaderArtKey === 'boot' && state && state\.loader_job === 'EnterGame'\) \{[\s\S]*?prepareLeaderArt\(slug\);[\s\S]*?activeMatchArt = preparedLeaderArt;/,
        "tutorial art is prepared behind the original splash and pinned to the match");
    const attachExitArt = loaderSource.indexOf("const pendingArt = syncLoaderArt(state);");
    const closeAfterArt = loaderSource.indexOf("if (state.loader_done === true) {\n            if (pendingArt) return;\n            finish(cycleId);", attachExitArt);
    assert.ok(attachExitArt >= 0 && closeAfterArt > attachExitArt,
        "exit art is attached before the loader begins closing");
    assert.match(loaderSource, /function waitForExitLeaderArt\(prepared, slug, cycleId\)[\s\S]*?syncLoaderArt\(current\)[\s\S]*?finish\(cycleId\);/,
        "a pending exit reuses the same image and closes only its own loader cycle");
    assert.doesNotMatch(loaderSource, /prepared\.variant !== leaderArtVariant\(\)/,
        "orientation changes do not discard the art used by this match");
    assert.match(coreSource, /return window\.SOW_leaderArtUrl\(slug, variant\)/);
    assert.match(heroesSource, /prepareLeaderArt\(activeLeader\)/);
    assert.doesNotMatch(shellSource, /state\.phase === "Playing" && state\.loader_leader[\s\S]*prepareLeaderArt\(leaderById\(state\.loader_leader\)\)/,
        "finishing EnterGame does not trigger a redundant image preparation");
    assert.match(shellSource, /function normalizeCompletedExitState\([\s\S]*?loader_cycle_id <= completedLoaderCycleId[\s\S]*?loader_job: "Boot"/,
        "a completed exit is normalized before later menu snapshots reach the loader");
    assert.match(shellSource, /sow:loader-cycle-ready", function \(event\)[\s\S]*?event\.detail\.cycle_id[\s\S]*?normalizeCompletedExitState\(state\)/,
        "menu completion is tied to the exact loader cycle");
    for (const source of [coreSource, heroesSource, storeSource, profileSource]) {
        assert.doesNotMatch(source, /asset\(["']shell\/leaders\//, "leader art must use the same versioned URL as the loader");
    }
    const introStart = matchStartSource.indexOf('log::info!("Portal boot: new player -> JavaScript campaign bootstrap")');
    const introEnd = matchStartSource.indexOf("crate::store_portals::gameplay_stop();", introStart);
    assert.ok(introStart >= 0 && introEnd > introStart);
    const bootIntro = matchStartSource.slice(introStart, introEnd);
    assert.match(bootIntro, /let campaign = crate::campaign::CampaignId::Boudica/);
    assert.match(bootIntro, /boot_campaign_pending = Some\(campaign\.episode_id\(\)\.to_string\(\)\)/);
    assert.match(bootIntro, /begin_enter_game_loader\(campaign\.advisor\(\)\)/);
    assert.doesNotMatch(bootIntro, /hide_web_loader|splash_state\.done = true|phase = ClientPhase::MainMenu/);
    assert.match(webMenu, /"phase": "Playing",\s*"loader_cycle_id": app\.ui\.app\.splash_state\.cycle_id,\s*"loader_job": splash_job_name\(&app\.ui\.app\.splash_state\.job\),\s*"loader_leader": app\.ui\.app\.splash_state\.loader_leader\.map\(leader_id\),\s*"loader_progress": app\.ui\.app\.splash_state\.progress\.clamp\(0\.0, 1\.0\),\s*"loader_done": app\.ui\.app\.splash_state\.done/);
    assert.match(webMenu, /"loader_cycle_id": app\.ui\.app\.splash_state\.cycle_id/);
    assert.match(loaderSource, /if \(loaderArtKey === key \|\| loaderArtKey === key \+ ':failed'\) return null;/);
    assert.match(loaderSource, /if \(!prepared\.ready\) \{[\s\S]*?return state\.loader_job === 'ExitGame' \? prepared : null;\s*\}\s*const image = prepared\.image;/,
        "a completed existing preload can recover from an earlier pending state");
    assert.doesNotMatch(loaderSource, /state\.phase !== 'MainMenu' && state\.phase !== 'Playing'/);
    assert.doesNotMatch(loaderSource, /window\.hideWebLoader/);
    assert.doesNotMatch(appStateSource, /web_loader_hidden/);
    assert.match(tutorial, /state\.boot_campaign && runtime\.bootEpisode !== state\.boot_campaign\) startEpisode\(state\.boot_campaign, true\)/);
    const tutorialStateUpdate = shellSource.indexOf("window.SOW_tutorial_menu_state_update(state)");
    const loaderStateSync = shellSource.indexOf("syncWebLoaderForState(state)", tutorialStateUpdate);
    assert.ok(tutorialStateUpdate >= 0 && loaderStateSync > tutorialStateUpdate,
        "the same menu-state update starts the tutorial request before syncing the existing loader");
    const campaignCommandStart = webMenu.indexOf("WebMenuCommand::StartCampaignEpisode {");
    const campaignCommandEnd = webMenu.indexOf("WebMenuCommand::CompleteCampaignEpisode {", campaignCommandStart);
    assert.ok(campaignCommandStart >= 0 && campaignCommandEnd > campaignCommandStart);
    assert.match(webMenu.slice(campaignCommandStart, campaignCommandEnd),
        /start_campaign_episode_from_web\(campaign, roster, match_config\)/);
    assert.match(matchStartSource, /self\.start_offline_match\([\s\S]*?\},\s*true,\s*\);/);
    const campaignStartStart = matchStartSource.indexOf("pub(crate) fn start_campaign_episode_from_web");
    const offlineStartStart = matchStartSource.indexOf("pub(crate) fn start_offline_match", campaignStartStart);
    const campaignStart = matchStartSource.slice(campaignStartStart, offlineStartStart);
    const offlineStart = matchStartSource.slice(offlineStartStart);
    assert.doesNotMatch(campaignStart, /set_selected_leader\(/,
        "starting a campaign does not replace the account's menu leader");
    assert.doesNotMatch(offlineStart, /set_selected_leader\(/,
        "starting a tutorial match keeps its leader separate from menu selection");
    assert.match(offlineStart, /color: leader\.filler_rgb\(\)/);
    assert.match(offlineStart, /civilization: leader\.civilization\(\),\s*leader,/,
        "the tutorial's actual game player still uses its campaign leader");
    const serverStartStart = netMessagesSource.indexOf("ServerMessage::Start(start_msg) =>");
    const serverStartEnd = netMessagesSource.indexOf("self.ui.app.main_menu_state.is_waiting = false;", serverStartStart);
    assert.ok(serverStartStart >= 0 && serverStartEnd > serverStartStart);
    assert.match(netMessagesSource.slice(serverStartStart, serverStartEnd),
        /sync_selected_leader_from_match\(\s*player\.leader,\s*start_msg\.config\.tutorial,?\s*\)/,
        "the initialized tutorial player cannot overwrite the account leader");
    const exitStart = sessionSource.indexOf("pub(crate) fn begin_exit_to_main_menu");
    const exitEnd = sessionSource.indexOf("/// Enter the EnterGame splash", exitStart);
    assert.match(sessionSource.slice(exitStart, exitEnd),
        /sync_selected_leader_from_match\(\s*player\.leader,\s*was_offline && self\.ui\.tutorial_active,?\s*\)/,
        "leaving a tutorial keeps its leader for the splash but not as the menu selection");
    assert.match(nativeProfileSource,
        /fn sync_selected_leader_from_match\([\s\S]*?if !tutorial \{[\s\S]*?set_selected_leader\(leader, false\)/,
        "MainMenuState owns the rule for keeping campaign heroes separate from account selection");
    assert.match(nativeProfileSource, /fn tutorial_match_does_not_replace_account_leader\([\s\S]*?sync_selected_leader_from_match\(Leader::Boudica, true\)[\s\S]*?assert_eq!\(state\.selected_leader, Leader::Caesar\)/,
        "the Boudica tutorial cannot replace the account's selected hero");
    const applyCloudProfileStart = accountSource.indexOf("pub(crate) fn apply_cloud_profile(");
    const applyCloudProfileEnd = accountSource.indexOf("\n    pub(crate) fn ", applyCloudProfileStart + 1);
    assert.ok(applyCloudProfileStart >= 0);
    const applyCloudProfile = accountSource.slice(applyCloudProfileStart, applyCloudProfileEnd);
    assert.match(applyCloudProfile,
        /cloud_preferred_leader\.unwrap_or_else\([\s\S]*?assigned_leader_for_account\([\s\S]*?current_rotation_period\(\)/,
        "a missing cloud preference resolves to the account's free-rotation leader, not Boudica");
    const tutorialCompleteStart = dataDbSource.indexOf("pub async fn complete_tutorial(");
    const tutorialCompleteEnd = dataDbSource.indexOf("/// Register expected players", tutorialCompleteStart);
    const tutorialComplete = dataDbSource.slice(tutorialCompleteStart, tutorialCompleteEnd);
    assert.match(tutorialComplete,
        /assigned_leader_for_account\(\s*account_id,\s*crate::commerce::current_rotation_period\(\)\s*,?\s*\)/,
        "an account missing its preference receives its normal free-rotation assignment");
    assert.doesNotMatch(tutorialComplete, /Leader::Boudica/);
    const engineLoaderSource = fs.readFileSync(path.join(shell, "../../sow-client/src/loader/engine.rs"), "utf8");
    assert.match(engineLoaderSource,
        /if step == 3 && self\.sim\.current_snapshot\.is_some\(\) \{[\s\S]*?if ready_to_release \{[\s\S]*?self\.ui\.app\.splash_state\.done = true;/,
        "Rust reports done only after the initialized game snapshot is ready");
    const enterLoaderStart = sessionSource.indexOf("pub(crate) fn begin_enter_game_loader");
    const enterLoaderEnd = sessionSource.indexOf("/// Whether the map/mover GPU path", enterLoaderStart);
    assert.ok(enterLoaderStart >= 0 && enterLoaderEnd > enterLoaderStart);
    const enterLoader = sessionSource.slice(enterLoaderStart, enterLoaderEnd);
    assert.match(enterLoader, /job == SplashJob::EnterGame[\s\S]*loader_leader == Some\(leader\)[\s\S]*!self\.ui\.app\.splash_state\.done[\s\S]*return;/);
    assert.match(sessionSource.slice(enterLoaderStart, enterLoaderEnd), /SplashJob::Boot[\s\S]*?&& !self\.ui\.app\.splash_state\.done/);

    class Element {
        constructor(tagName) {
            this.tagName = tagName;
            this.children = [];
            this.style = {};
            this.dataset = {};
            this.attributes = {};
            this.parentNode = null;
        }
        appendChild(child) {
            if (child.parentNode) child.remove();
            this.children.push(child);
            child.parentNode = this;
            return child;
        }
        insertBefore(child, before) {
            if (child.parentNode) child.remove();
            const index = this.children.indexOf(before);
            this.children.splice(index < 0 ? this.children.length : index, 0, child);
            child.parentNode = this;
            return child;
        }
        replaceChild(next, previous) {
            const index = this.children.indexOf(previous);
            if (index < 0) throw new Error("old loader picture is not attached");
            this.children[index] = next;
            next.parentNode = this;
            previous.parentNode = null;
            return previous;
        }
        remove() {
            if (!this.parentNode) return;
            const siblings = this.parentNode.children;
            siblings.splice(siblings.indexOf(this), 1);
            this.parentNode = null;
        }
        setAttribute(name, value) { this.attributes[name] = String(value); }
        getAttribute(name) {
            if (Object.prototype.hasOwnProperty.call(this.attributes, name)) return this.attributes[name];
            return this[name] == null ? null : String(this[name]);
        }
        removeAttribute(name) { delete this.attributes[name]; }
        get firstChild() { return this.children[0] || null; }
    }

    const root = new Element("div");
    root.id = "web-loader";
    const initialPicture = new Element("picture");
    initialPicture.className = "splash-picture";
    const initialSource = new Element("source");
    initialSource.id = "splash-mobile";
    const initialImage = new Element("img");
    initialImage.id = "splash-bg";
    const embeddedSplash = "data:image/webp;base64,embedded-splash";
    initialImage.setAttribute("src", embeddedSplash);
    initialImage.src = embeddedSplash;
    initialPicture.appendChild(initialSource);
    initialPicture.appendChild(initialImage);
    root.appendChild(initialPicture);
    for (const id of ["loader-bar-wrap", "loader-bar-fill", "loader-bar-full", "loader-bar-empty", "loader-text"]) {
        const child = new Element("div");
        child.id = id;
        root.appendChild(child);
    }
    const find = (node, predicate) => predicate(node) ? node : node.children.map(child => find(child, predicate)).find(Boolean) || null;
    const findPicture = () => find(root, node => node.className === "splash-picture");
    const document = {
        readyState: "loading",
        body: new Element("body"),
        createElement: tagName => new Element(tagName),
        getElementById: id => find(root, node => node.id === id),
        querySelector: selector => selector === "#web-loader .splash-picture" ? findPicture() : null,
        addEventListener() {}
    };
    const timers = new Map();
    const events = [];
    const completedLoaderCycles = [];
    const leaderPreloads = [];
    class PreloadImage {
        constructor() { this.parentNode = null; this.children = []; }
        set src(value) { this._src = value; leaderPreloads.push(this); }
        get src() { return this._src; }
        decode() { return Promise.resolve(); }
        remove() {
            if (!this.parentNode) return;
            const siblings = this.parentNode.children;
            siblings.splice(siblings.indexOf(this), 1);
            this.parentNode = null;
        }
    }
    let nextTimer = 0;
    const window = {
        SOW_BOOT_UI_BASE: "/assets/shell/loader",
        SOW_BUILD_TS: "regression",
        SOW_PORTAL: "jest",
        innerWidth: 1024,
        innerHeight: 768,
        matchMedia: () => ({ matches: window.innerHeight >= window.innerWidth }),
        location: { href: "https://game.test/", origin: "https://game.test" },
        addEventListener() {},
        dispatchEvent(event) {
            events.push(event.type);
            if (event.type === "sow:loader-cycle-ready") completedLoaderCycles.push(event.detail.cycle_id);
        }
    };
    const context = {
        window, document, URL, Image: PreloadImage,
        Event: class { constructor(type) { this.type = type; } },
        CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options.detail; } },
        performance: { now: () => 0 },
        requestAnimationFrame: () => 1,
        cancelAnimationFrame() {},
        setTimeout(callback) { const id = ++nextTimer; timers.set(id, callback); return id; },
        clearTimeout(id) { timers.delete(id); }
    };
    vm.runInNewContext(loaderSource, context);
    window.SOW_initWebLoader();
    assert.equal(findPicture(), initialPicture, "initialization preserves the HTML splash picture");
    assert.equal(findPicture().children[1], initialImage, "initialization preserves the embedded splash image node");
    assert.equal(initialImage.src, embeddedSplash, "initialization does not replace the embedded splash with a network URL");
    assert.equal(leaderPreloads.length, 0, "initialization creates no duplicate hero-image request");

    const leaders = [
        { id: "Boudica", slug: "boudica" },
        { id: "Caesar", slug: "caesar" },
        { id: "Ragnar", slug: "ragnar" }
    ];
    let cycleId = -1;
    let previousJob = null;
    let previousDone = false;
    const state = (job, leader, progress = 0.4, phase = "Splash", done = false) => {
        if (job !== previousJob || (previousDone && !done)) cycleId += 1;
        previousJob = job;
        previousDone = done;
        return {
            phase, loader_cycle_id: cycleId, loader_job: job, loader_leader: leader,
            loader_progress: progress, loader_done: done, leaders
        };
    };
    window.SOW_syncWebLoader(state("Boot", null, 0.2, "Splash"));
    const bootPicture = findPicture();
    const bootImage = bootPicture.children[1];
    assert.equal(bootPicture, initialPicture, "Boot updates keep the original splash picture");
    assert.equal(bootImage, initialImage, "Boot updates keep the original splash image");
    const loaderBar = document.getElementById("loader-bar-fill");

    const campaign = require(path.join(shell, "sow-campaign.js"));
    const pendingTutorialFetches = [];
    const tutorialCommands = [];
    window.SOW_t = key => key;
    window.SOW_menu_command = message => tutorialCommands.push(JSON.parse(message));
    window.SOWCampaign = campaign;
    window.SOWCampaignView = { mount: () => ({ render() {}, destroy() {} }) };
    vm.runInNewContext(tutorial, {
        window, document, performance: { now: () => 0 }, console,
        fetch(url) {
            return new Promise(resolve => pendingTutorialFetches.push({ url, resolve }));
        }
    });
    const bootCampaignState = {
        ...state("EnterGame", "Boudica", 0.95, "Splash"),
        boot_campaign: "boudica"
    };
    window.SOW_tutorial_menu_state_update(bootCampaignState);
    assert.equal(pendingTutorialFetches.length, 2, "the tutorial's paired files are genuinely pending");
    assert.deepEqual(
        pendingTutorialFetches.map(request => request.url).sort(),
        ["/assets/campaign/boudica.json", "/assets/campaign/boudica.triggers.json"].sort()
    );
    window.SOW_syncWebLoader(bootCampaignState);
    assert.equal(findPicture(), bootPicture, "Boot -> EnterGame keeps the original picture node");
    assert.equal(findPicture().children[1], bootImage, "the tutorial does not replace the startup image");
    assert.equal(bootImage.src, embeddedSplash, "Boot states do not reload the embedded splash image");
    assert.equal(leaderPreloads.length, 1, "Boudica is preloaded during the tutorial's existing loader");
    const boudicaReady = window.SOW_prepareLeaderArt("boudica");
    const boudicaPreload = leaderPreloads[0];
    assert.equal(boudicaPreload.src, window.SOW_leaderArtUrl("boudica", "desktop"));
    assert.equal(leaderPreloads.length, 1, "the tutorial loader owns one Boudica image request");
    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 0.96, "Splash"));
    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 0.97, "Playing"));
    assert.equal(findPicture(), bootPicture, "the same splash remains while tutorial and game initialization are pending");
    assert.equal(document.getElementById("loader-bar-fill"), loaderBar, "the progress bar node is never recreated");
    assert.equal(root.style.visibility, "visible");
    assert.equal(loaderBar.style.width, "97.0%", "progress advances monotonically through tutorial startup");
    assert.equal(events.length, 0, "phase changes cannot close the loader before Rust reports done");

    const roster = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    for (const request of pendingTutorialFetches) {
        request.resolve({
            ok: true,
            json: () => Promise.resolve(request.url.endsWith(".triggers.json") ? definition : roster)
        });
    }
    await new Promise(setImmediate);
    assert.equal(tutorialCommands.length, 1, "the resolved tutorial data starts the campaign through the existing command");
    assert.equal(tutorialCommands[0].type, "start_campaign_episode");
    assert.equal(tutorialCommands[0].episode_id, "boudica");
    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 0.98, "Playing"));
    assert.equal(findPicture(), bootPicture, "the splash remains through game initialization after tutorial download");
    assert.equal(root.style.visibility, "visible");
    assert.equal(events.length, 0);

    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 1, "Playing", true));
    [...timers.values()][0]();
    timers.clear();
    assert.equal(findPicture(), null, "the art is released after the tutorial finishes loading");
    assert.ok(events.includes("sow:loader-ready"));
    assert.equal(window.SOW_leaderArtUrl("boudica", "desktop"), "/assets/shell/leaders/boudica_desktop.webp?v=regression");
    assert.equal(window.SOW_leaderArtUrl("boudica", "mobile"), "/assets/shell/leaders/boudica_mobile.webp?v=regression");
    assert.equal(window.SOW_leaderArtUrl("boudica"), "/assets/shell/leaders/boudica_desktop.webp?v=regression",
        "the menu backdrop resolver defaults to the landscape variant on a landscape screen");
    assert.equal(window.SOW_prepareLeaderArt("boudica"), boudicaReady,
        "requesting the same prepared URL reuses its existing promise");
    assert.equal(leaderPreloads.length, 1);

    window.innerWidth = 390;
    window.innerHeight = 844;
    window.SOW_syncWebLoader(state("ExitGame", "Boudica", 0.6));
    assert.equal(findPicture(), bootPicture, "the original tutorial splash remains while its one Boudica preload is pending");
    assert.equal(findPicture().children[1], bootImage);
    assert.equal(leaderPreloads.length, 1, "showing the loader starts no additional hero-image request");
    const tutorialExitDone = state("ExitGame", "Boudica", 1, "MainMenu", true);
    window.SOW_syncWebLoader(tutorialExitDone);
    assert.equal(root.style.visibility, "visible", "loader_done cannot hide the bar before the prepared image is attached");
    assert.equal(document.getElementById("loader-bar-fill"), loaderBar);
    assert.equal(events.filter(event => event === "sow:loader-cycle-ready").length, 1);
    boudicaPreload.onload();
    assert.equal(await boudicaReady, true);
    await new Promise(setImmediate);
    assert.equal(findPicture().children[0], boudicaPreload,
        "the same Boudica image that began loading behind Boot is attached on tutorial exit");
    const exitBoudicaPicture = findPicture();
    const exitBoudicaImage = exitBoudicaPicture.children[0];
    assert.equal(exitBoudicaImage, boudicaPreload);
    assert.match(boudicaPreload.src, /leaders\/boudica_desktop\.webp/,
        "exit keeps the exact prepared variant after orientation changes");
    assert.equal(leaderPreloads.length, 1, "the full tutorial cycle creates one Boudica image");
    assert.equal(document.getElementById("loader-bar-fill"), loaderBar, "tutorial exit keeps the one original progress bar");
    [...timers.values()][0]();
    timers.clear();
    assert.ok(events.includes("sow:loader-cycle-ready"));
    assert.equal(completedLoaderCycles.filter(id => id === tutorialExitDone.loader_cycle_id).length, 1);
    window.SOW_syncWebLoader({ ...tutorialExitDone, loader_progress: 1 });
    window.SOW_syncWebLoader({ ...tutorialExitDone, loader_progress: 1 });
    assert.equal(root.style.visibility, "hidden", "repeated completed MainMenu snapshots cannot reopen the old ExitGame cycle");
    assert.equal(root.style.pointerEvents, "none", "the completed loader cannot block menu or lobby input");
    assert.equal(findPicture(), null, "the old Boudica image stays detached after the cycle is consumed");
    assert.equal(completedLoaderCycles.filter(id => id === tutorialExitDone.loader_cycle_id).length, 1,
        "one cycle dispatches one completion event");
    window.innerWidth = 1024;
    window.innerHeight = 768;
    const reusedBoudicaReady = window.SOW_prepareLeaderArt("boudica");
    assert.equal(reusedBoudicaReady, boudicaReady, "the decoded image remains prepared after the loader cycle");
    assert.equal(leaderPreloads.length, 1, "the finished cycle does not trigger another Boudica image");
    assert.equal(await reusedBoudicaReady, true);

    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 0.45));
    const normalEntryPicture = findPicture();
    assert.equal(normalEntryPicture.children[0], boudicaPreload,
        "a normal game entry displays the already-decoded selected hero");
    assert.equal(leaderPreloads.length, 1);
    window.SOW_syncWebLoader(state("EnterGame", "Boudica", 1, "Playing", true));
    [...timers.values()][0]();
    timers.clear();
    window.innerWidth = 390;
    window.innerHeight = 844;
    window.SOW_syncWebLoader(state("ExitGame", "Boudica", 0.45));
    const normalExitPicture = findPicture();
    assert.equal(normalExitPicture.children[0], boudicaPreload,
        "the same decoded image is reused for game exit");
    window.SOW_syncWebLoader(state("ExitGame", "Boudica", 1, "MainMenu", true));
    assert.equal(findPicture().children[0], boudicaPreload,
        "the final done update adopts the exit image before closing");
    assert.equal(leaderPreloads.length, 1, "entry and exit create no second Boudica image");
    [...timers.values()][0]();
    timers.clear();
    window.innerWidth = 1024;
    window.innerHeight = 768;

    const failedReady = window.SOW_prepareLeaderArt("caesar");
    const failedPreload = leaderPreloads[leaderPreloads.length - 1];
    failedPreload.onerror();
    assert.equal(await failedReady, false);
    const requestCountBeforeUnreadyLoader = leaderPreloads.length;
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 0.35));
    assert.equal(findPicture(), bootPicture, "failed preload keeps the original splash instead of leaving the loader blank");
    assert.equal(root.style.visibility, "visible");
    assert.equal(loaderBar.style.width, "35.0%");
    assert.equal(leaderPreloads.length, requestCountBeforeUnreadyLoader);
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 1, "Playing", true));
    [...timers.values()][0]();
    timers.clear();

    window.innerWidth = 390;
    window.innerHeight = 844;
    assert.equal(window.SOW_leaderArtUrl("caesar"), "/assets/shell/leaders/caesar_mobile.webp?v=regression",
        "the menu backdrop resolver defaults to the portrait variant on a portrait screen");
    const caesarReady = window.SOW_prepareLeaderArt("caesar");
    const caesarPreload = leaderPreloads.at(-1);
    assert.match(caesarPreload.src, /leaders\/caesar_mobile\.webp\?v=regression/);
    caesarPreload.onload();
    assert.equal(await caesarReady, true);
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 0.45));
    const caesarPicture = findPicture();
    const caesarImage = caesarPicture.children[0];
    assert.notEqual(caesarPicture, exitBoudicaPicture, "a separate game load gets fresh art");
    assert.equal(exitBoudicaPicture.parentNode, null, "finished-cycle art is removed");
    assert.equal(caesarImage, caesarPreload, "the selected hero's decoded art is adopted by the loader");
    assert.match(caesarImage.src, /leaders\/caesar_mobile\.webp/);
    assert.equal(leaderPreloads.length, 3, "the loader adds no request after the selection preload");
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 0.55));
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 0.65));
    assert.equal(findPicture(), caesarPicture, "repeated progress updates do not replace the active hero picture");
    assert.equal(findPicture().children[0], caesarImage, "repeated progress updates preserve the decoded image node");
    assert.equal(leaderPreloads.length, 3, "repeated progress updates create no new Image");
    assert.equal(loaderBar.style.width, "65.0%");
    exitBoudicaImage.onerror();
    assert.equal(findPicture(), caesarPicture, "a delayed old-image error cannot touch the current hero");

    caesarImage.onerror();
    assert.equal(findPicture(), bootPicture, "a failed current image falls back to the original splash");
    assert.equal(root.style.visibility, "visible", "art failure does not close the loader");
    assert.equal(document.getElementById("loader-bar-fill").style.width, "65.0%");

    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 1, "Playing", true));
    [...timers.values()][0]();
    timers.clear();
    assert.equal(events.filter(event => event === "sow:loader-ready").length, 1);
    assert.equal(events.filter(event => event === "sow:loader-cycle-ready").length, 6,
        "each completed loader cycle emits one ready event");

    window.innerWidth = 1024;
    window.innerHeight = 768;
    const nextCaesarReady = window.SOW_prepareLeaderArt("caesar");
    const nextCaesarPreload = leaderPreloads.at(-1);
    nextCaesarPreload.onload();
    assert.equal(await nextCaesarReady, true);
    window.SOW_syncWebLoader(state("EnterGame", "Caesar", 0.45));
    const nextCaesarPicture = findPicture();
    assert.equal(nextCaesarPicture.children[0], nextCaesarPreload);
    assert.match(nextCaesarPreload.src, /leaders\/caesar_desktop\.webp\?v=regression/,
        "the loader follows the same landscape resolver after orientation changes");
    caesarImage.onerror();
    assert.equal(findPicture(), nextCaesarPicture, "an old cycle cannot invalidate the same hero in a new cycle");
    nextCaesarPicture.children[0].onerror();
    assert.equal(findPicture(), bootPicture, "a failed current image falls back to the original splash");
    assert.equal(root.style.visibility, "visible", "art failure does not close the loader");
    assert.equal(document.getElementById("loader-bar-fill").style.width, "45.0%");

    const pendingExitReady = window.SOW_prepareLeaderArt("ragnar");
    const pendingExitImage = leaderPreloads.at(-1);
    window.SOW_syncWebLoader(state("ExitGame", "Ragnar", 0.45));
    assert.equal(findPicture(), bootPicture, "a pending exit keeps the original splash until that same request is ready");
    const imageCountBeforePendingExitReady = leaderPreloads.length;
    pendingExitImage.onload();
    assert.equal(await pendingExitReady, true);
    window.SOW_syncWebLoader(state("ExitGame", "Ragnar", 1, "MainMenu", true));
    assert.equal(findPicture().children[0], pendingExitImage,
        "a completed pending preload recovers from unready and is attached before finish");
    assert.equal(leaderPreloads.length, imageCountBeforePendingExitReady,
        "the final exit sync reuses the in-flight image");
    [...timers.values()][0]();
    timers.clear();
    assert.equal(findPicture(), null);
    assert.equal(document.getElementById("loader-bar-fill"), loaderBar,
        "entry and exit preserve the same single progress bar");
    assert.equal(events.filter(event => event === "sow:loader-ready").length, 1);
    assert.equal(events.filter(event => event === "sow:loader-cycle-ready").length, 7);
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
    assert.match(shellSource, /heroImage\(profileData && profileData\.preferred_leader\)/);
    assert.doesNotMatch(profileSource.slice(profileSource.indexOf('profileTab === "overview"'), profileSource.indexOf('profileTab === "leaders"')), /profileLeaderCard|profileAchievementsMarkup/);
    assert.doesNotMatch(profileSource, /loadProfileRatings|profileTab === "ranked"|profileRecentLeadersPanel/);
    assert.doesNotMatch(profileCss, /sow-profile__favorites|sow-profile__rating/);
    assert.doesNotMatch(nativeProfileSource, /LoadProfileRatings|ProfileRatingsLoaded|ratings_loaded|ProfileTab::Ranked|ranked records/i);
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
        renderSettingsBody(pokiSource, "function renderFooter")
    ];
    for (const body of settings) {
        assert.match(body, /menu\.settings/);
        assert.match(body, /data-command='toggle_settings'/);
        assert.match(body, /menu\.music_volume/);
        assert.match(body, /menu\.motion_animation/);
        assert.match(body, /menu\.language/);
        assert.match(body, /account-value/);
        assert.match(body, /settings-account/);
        assert.doesNotMatch(body, /system_configuration|menu\.done|master_audio|settings-mute|menu\.account|account-row|WOU-ID|\+1/);
    }
    assert.match(shellSource, /SOW_getWouSession/);
    assert.match(shellSource, /data-command='confirm_sign_out'/);
    assert.match(shellSource, /data-command='cancel_sign_out'/);
    assert.match(shellSource, /data-menu-overlay='settings'/);
    assert.match(shellSource, /event\.target === settingsOverlay/);
    assert.match(shellSource, /event\.key === "Escape" && settingsOpen/);
    assert.doesNotMatch(lobbiesSource, /lobbies\.connecting_server/);
});

test("hero purchase stays server-backed, direct, and visually honest", () => {
    assert.match(storeSource, /var leaderActions = !offer \|\| offer\.owned \? ""/);
    assert.doesNotMatch(storeSource, /offer\.free_rotation \|\| offer\.available/);
    assert.match(storeSource, /data-command='unlock_leader'/);
    assert.match(storeSource, /data-command='open_skin_purchase'/);
    assert.match(storeSource, /sow-store__currency-amount--insufficient/);
    assert.match(storeSource, /command = confirm \? "open_product_purchase" : "buy_product"/);
    assert.match(storeSource, /skin\.direct_product_id, SOW_t\("store\.buy"\), true/);
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
    assert.match(profileCss, /\.sow-heroes__rotation/);
});

// GUARD — owner decision. Read this before changing anything in it.
// The heroes featured picture picks its art by the SHAPE OF THE PANEL, not by
// the device or the file name: desktop/landscape = tall column panel ->
// *_mobile.webp (1080x1920); phone/portrait = short wide band ->
// *_desktop.webp (1920x1080). The file names are backwards, so this mapping
// was flipped without authorization three times (80c7043d 2026-09-24,
// b945feb3 2026-09-24, 12486980 2026-10-01) and the last one shipped broken.
// It runs in the ./sow p preflight: a red run here means someone flipped it
// again. Full rationale: heroesFeaturedArt() in main_menu.heroes.js and
// AGENTS.md -> "Heroes featured art (owner decision — do not flip)".
test("heroes featured art follows the panel shape, not the device name", () => {
    const why = "OWNER DECISION: desktop gets *_mobile.webp (tall column panel), " +
        "portrait screens get *_desktop.webp (short band panel). The file names are backwards on purpose. " +
        "Flipping this broke production in 80c7043d, b945feb3 and 12486980. " +
        "Read heroesFeaturedArt() and AGENTS.md before touching it.";
    const helperStart = heroesSource.indexOf("function heroesFeaturedArt");
    const helperEnd = heroesSource.indexOf("function updateHeroesPreview", helperStart);
    assert.ok(helperStart >= 0 && helperEnd > helperStart, "heroesFeaturedArt() must exist as the single owner of the mapping — " + why);
    const helper = heroesSource.slice(helperStart, helperEnd);
    assert.match(helper, /band: leaderArtUrl\(slug, "desktop"\)/, why);
    assert.match(helper, /column: leaderArtUrl\(slug, "mobile"\)/, why);
    assert.match(heroesSource, /OWNER DECISION — DO NOT "FIX" THIS MAPPING/,
        "the mapping must keep its explanation in the source — " + why);
    assert.equal((heroesSource.match(/heroesFeaturedArt\(activeLeader\.slug\)/g) || []).length, 2,
        "renderHeroes() and updateHeroesPreview() must both go through the helper — " + why);
    const previewStart = heroesSource.indexOf("function updateHeroesPreview");
    const previewEnd = heroesSource.indexOf("function renderHeroInfoModal", previewStart);
    const renderStart = heroesSource.indexOf("function renderHeroes()");
    assert.ok(previewStart >= 0 && previewEnd > previewStart && renderStart > previewEnd);
    const previewBody = heroesSource.slice(previewStart, previewEnd);
    const renderBody = heroesSource.slice(renderStart);
    assert.doesNotMatch(previewBody, /leaderArtUrl\(/, "no second inline art mapping in updateHeroesPreview() — " + why);
    assert.doesNotMatch(renderBody, /leaderArtUrl\(/, "no second inline art mapping in renderHeroes() — " + why);
    assert.match(previewBody, /source\.srcset = featuredArt\.band/, why);
    assert.match(previewBody, /image\.src = featuredArt\.column/, why);
    assert.match(renderBody, /source media='\(max-width: 680px\), \(orientation: portrait\)' srcset='" \+ esc\(featuredArt\.band\)/, why);
    assert.match(renderBody, /img src='" \+ esc\(featuredArt\.column\) \+ "' alt='[^']*' width='1080' height='1920'/,
        "the fallback <img> is the tall art, so 1080x1920 must stay — " + why);
    // The <picture> switches on exactly the media query that turns the panel
    // into a band, so art and layout can never disagree again.
    assert.match(profileCss,
        /@media \(max-width: 680px\), \(orientation: portrait\) \{[^@]*?\.sow-heroes__workspace \{\s*grid-template-columns: 1fr;/,
        "picture media query must equal the CSS band-layout query — " + why);
    // The info dialog is one tall card on every device; it is never swapped.
    assert.match(heroesSource, /function renderHeroInfoModal[\s\S]*?leaderArtUrl\(activeLeader\.slug, "mobile"\)/, why);
});

test("header exposes server progress currencies and keeps the real XP remainder", () => {
    assert.match(shellSource, /data-progression-gems-value/);
    assert.match(shellSource, /data-progression-laurels-value/);
    assert.doesNotMatch(pokiSource, /function renderTopbar|data-progression-gems-value/);
    assert.match(shellSource, /accountXp % 100/);
    assert.match(shellSource, /state\.gems/);
    assert.match(webMenu, /"gems": progress\.gems/);
});

test("campaign runtime shares the JSON interpreter and cinematic view", () => {
    assert.match(tutorial, /SOWCampaign\.validate/);
    assert.match(tutorial, /SOWCampaign\.create/);
    assert.match(tutorial, /SOWCampaignView\.mount/);
    assert.match(tutorial, /start_campaign_episode/);
    assert.match(tutorial, /set_tutorial_paused/);
    assert.match(tutorial, /complete_campaign_episode/);
    assert.match(webMenu, /WebMenuCommand::SetTutorialPaused \{\s*paused,\s*camera_only,\s*\} => \{[\s\S]*?self\.sim\.paused = paused/);
    assert.match(simUpdateSource, /if self\.sim\.paused \{\s*self\.sim\.offline_tick_timer = 0\.0;/);
    assert.match(campaignEngine, /paused: \["scene", "choice", "end"\]/);
    assert.match(campaignView, /data-story-choice/);
    assert.match(storyCss, /\.sow-story__choices/);
    assert.doesNotMatch(hudCss, /\.sow-hud__tutorial-overlay/);
});

test("campaign episode cards use leader portraits and emphasize the next playable chapter", () => {
    const campaignCss = menuCss["main_menu.campaign.css"];
    assert.doesNotMatch(lobbiesSource, /campaign_tagline/);
    assert.match(lobbiesSource, /var mapName = episode\.id === "boudica" \? "eastanglia" : "northamerica"/);
    assert.match(lobbiesSource, /var mapArt = lobbyThumb\(\{ map_name: mapName \}\)/);
    assert.match(lobbiesSource, /asset\("gameplay\/avatars\/" \+ \(episode\.id === "boudica" \? "boudica" : "lady_six_sky"\) \+ "\.webp"\)/);
    assert.match(lobbiesSource, /sow-campaign__map-art[\s\S]*?sow-campaign__leader-art/);
    assert.match(lobbiesSource, /isNext \? " is-next"/);
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
    for (const episodeId of episodeIds) {
        const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, episodeId + ".json"), "utf8"));
        const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, episodeId + ".triggers.json"), "utf8"));
        assert.equal(definition.episode_id, episodeId);
        assert.equal(roster.map, expected[episodeId][0], episodeId + " card map");
        assert.ok(Object.values(definition.speakers || {}).some(speaker => speaker.avatar === expected[episodeId][1]), episodeId + " card leader portrait");
        for (const faction of roster.factions) {
            assert.match(faction.id, /^[a-z][a-z0-9_]{0,95}$/i, `${episodeId}/${faction.name}: stable ID`);
            assert.ok(Number.isInteger(faction.starting_troops) && faction.starting_troops >= 0, `${episodeId}/${faction.name}: starting troops`);
            assert.ok(["allied", "neutral", "enemy"].includes(faction.relation), `${episodeId}/${faction.name}: relationship`);
            assert.ok(typeof faction.civ === "string" && faction.civ, `${episodeId}/${faction.name}: civilization`);
            assert.ok(typeof faction.leader === "string" && faction.leader, `${episodeId}/${faction.name}: leader`);
            for (const removed of ["role", "betrayal"]) assert.equal(Object.hasOwn(faction, removed), false, `${episodeId}/${faction.name}: no ${removed}`);
            if (episodeId !== "boudica") assert.equal(Object.hasOwn(faction, "hostility"), false, `${episodeId}/${faction.name}: no Boudica combat override`);
            assert.match(faction.color, /^#[0-9a-f]{6}$/i, `${episodeId}/${faction.name}: map color`);
        }
        const actualTroops = roster.factions.reduce((counts, faction) => {
            counts[faction.starting_troops] = (counts[faction.starting_troops] || 0) + 1;
            return counts;
        }, {});
        const expectedTroops = { boudica: { 0: 1, 500: 74, 1000: 15, 2500: 3, 5000: 4 }, six_sky_ep1: { 500: 10, 1000: 4, 2500: 3, 5000: 1 }, six_sky_ep2: { 500: 8, 1000: 3, 2500: 5, 5000: 1 }, six_sky_ep3: { 500: 7, 1000: 3, 2500: 4, 5000: 1 } };
        assert.deepEqual(actualTroops, expectedTroops[episodeId]);
    }
});

test("campaign zoom input mode prioritizes TWA and touch devices, then Mac trackpads", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.equal(campaign.zoomInputMode({ androidTwa: true, platform: "MacIntel" }), "pinch");
    assert.equal(campaign.zoomInputMode({ mobile: true, platform: "MacIntel" }), "pinch");
    assert.equal(campaign.zoomInputMode({ userAgent: "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X)", platform: "MacIntel", maxTouchPoints: 5 }), "pinch");
    assert.equal(campaign.zoomInputMode({ platform: "MacIntel" }), "trackpad");
    assert.equal(campaign.zoomInputMode({ platform: "Win32" }), "wheel");
    assert.equal(campaign.zoomInputMode({ platform: "Linux x86_64" }), "wheel");
});

test("Boudica camera practice stays paused through desktop and touch routes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    for (const id of ["boudica_zoom_out", "boudica_zoom_in", "boudica_camera_drag", "boudica_camera_keys", "boudica_camera_hover"]) {
        const step = definition.steps.find(candidate => candidate.id === id);
        assert.equal(step.pause_game, true, `${id} pauses the simulation`);
        assert.equal(step.camera_only, true, `${id} permits only camera input`);
    }
    const desktop = campaign.create(definition, "boudica_zoom_out", { factions: [] });
    desktop.update({ zoom_out_events: 0 }, {}, 0);
    assert.equal(desktop.view().paused, true);
    assert.equal(desktop.advance(null, "boudica_zoom_out"), false);
    assert.equal(desktop.update({ zoom_out_events: 1 }, {}, 1).step.id, "boudica_zoom_in");
    assert.equal(desktop.view().paused, true);
    assert.equal(desktop.update({ zoom_out_events: 1, zoom_in_events: 1 }, {}, 2).step.id, "boudica_camera_drag");
    assert.equal(desktop.update({ zoom_out_events: 1, zoom_in_events: 1, camera_drag_events: 1, touch_controls: 0 }, {}, 3).step.id, "boudica_camera_keys");
    assert.equal(desktop.view().paused, true);
    assert.equal(desktop.update({ zoom_out_events: 1, zoom_in_events: 1, camera_drag_events: 1, camera_key_pan_events: 1, touch_controls: 0 }, {}, 4).step.id, "boudica_first_contact");
    assert.equal(desktop.view().paused, false);

    const touch = campaign.create(definition, "boudica_camera_drag", { factions: [] });
    touch.update({ camera_drag_events: 0, touch_controls: 1 }, {}, 0);
    assert.equal(touch.update({ camera_drag_events: 1, touch_controls: 1 }, {}, 1).step.id, "boudica_first_contact");
    assert.equal(touch.view().paused, false);
});

test("renaming a campaign faction changes prose but stable story references remain IDs", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        settings: { buildings_unlock_after_defeated: "roman_outpost", campaign_support: { after_defeated: "roman_outpost", share_percent: 50 } },
        speakers: { roman: { faction: "roman_outpost" } },
        steps: [{ trigger: { target: "roman_outpost", targets: ["roman_outpost", "other"] }, marker: { target: "roman_outpost" } }],
        reactions: [{ when: { type: "contact", target: "roman_outpost" } }],
        strings: { en: { hint: "Attack Old Name!", similar: "Old Namesake" }, es: { hint: "¡Ataca a Old Name!" } }
    };
    const renamed = campaign.renameFactionText(definition, [{ from: "Old Name", to: "Roman Outpost" }]);
    assert.equal(renamed.settings.buildings_unlock_after_defeated, "roman_outpost");
    assert.equal(renamed.settings.campaign_support.after_defeated, "roman_outpost");
    assert.equal(renamed.speakers.roman.faction, "roman_outpost");
    assert.deepEqual(renamed.steps[0].trigger, { target: "roman_outpost", targets: ["roman_outpost", "other"] });
    assert.equal(renamed.steps[0].marker.target, "roman_outpost");
    assert.equal(renamed.reactions[0].when.target, "roman_outpost");
    assert.equal(renamed.strings.en.hint, "Attack Roman Outpost!");
    assert.equal(renamed.strings.en.similar, "Old Namesake", "similar faction names stay unchanged");
    assert.equal(renamed.strings.es.hint, "¡Ataca a Roman Outpost!");
    assert.equal(definition.steps[0].trigger.target, "roman_outpost", "renaming preserves stable references");
    assert.deepEqual([...campaign.factionReferenceIds(renamed)].sort(), ["other", "roman_outpost"]);
});

test("disabled neutral offers stay hidden while named story dialogues and legacy rosters work", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const roster = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    const waiting = { id: "waiting", type: "objective", trigger: { type: "elapsed", value: 30 } };
    const storyOffer = JSON.parse(JSON.stringify(definition.reactions.find(reaction => reaction.id === "contact_trinovantes")));
    delete storyOffer.after;
    const neutralOffer = JSON.parse(JSON.stringify(definition.reactions.find(reaction => reaction.id === "neutral_contact_terms")));
    delete neutralOffer.after;
    const facts = { contact_faction_ids: ["trinovantes"] };

    const story = campaign.create({ entry: "waiting", steps: [waiting], reactions: [storyOffer] }, undefined, roster);
    assert.equal(story.update(facts, {}).reaction, "contact_trinovantes", "an exact story response ignores the generic-offer flag");

    const genericDefinition = { entry: "waiting", steps: [waiting], reactions: [neutralOffer] };
    const disabledGeneric = campaign.create(genericDefinition, undefined, roster);
    assert.equal(disabledGeneric.update(facts, {}).reaction, undefined, "false hides a generic neutral offer");

    const legacyRoster = JSON.parse(JSON.stringify(roster));
    delete legacyRoster.factions.find(faction => faction.id === "trinovantes").can_request_alliance;
    const legacyGeneric = campaign.create(genericDefinition, undefined, legacyRoster);
    assert.equal(legacyGeneric.update(facts, {}).reaction, "neutral_contact_terms", "a missing legacy field keeps the old offer behavior");
});

test("Boudica opens with a choice, then guides allied support, rebuilding and three Roman outposts", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const roster = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    const step = id => definition.steps.find(candidate => candidate.id === id);
    assert.deepEqual(campaign.validate(definition, roster, { hasText: () => true, hasAvatar: () => true }).errors, []);
    const renamedCamulodunum = campaign.renameFactionText(definition, [{ from: "Camulodunum", to: "Camulodunum Augusta" }]);
    assert.equal(renamedCamulodunum.strings.en["tutorial.boudica_camulodunum_title"], "Camulodunum Augusta");
    assert.equal(renamedCamulodunum.strings.es["tutorial.boudica_camulodunum_title"], "Camulodunum Augusta");
    assert.equal(renamedCamulodunum.steps.find(candidate => candidate.id === "boudica_camulodunum").trigger.target, "camulodunum");
    assert.equal(renamedCamulodunum.steps.find(candidate => candidate.id === "boudica_camulodunum").marker.target, "camulodunum");
    assert.ok(campaign.factionReferenceIds(renamedCamulodunum).has("camulodunum"));
    assert.equal(definition.settings.starting_troops, 2000);
    assert.match(campaignEditor, /inputField\("Starting troops", settings\.starting_troops/);
    assert.equal(roster.factions.some(faction => faction.relation === "allied"), false, "Boudica starts without pre-made allies");
    assert.equal(roster.factions.filter(faction => faction.relation === "enemy").length, 11, "declared Roman enemies start red");
    const target = roster.factions.find(faction => faction.name === "Roman Outpost");
    assert.deepEqual({ id: target.id, civ: target.civ, x: target.x, y: target.y }, { id: "roman_outpost", civ: "Roman Empire", x: 706, y: 64 });
    assert.deepEqual({ troops: target.starting_troops, relation: target.relation, hostility: target.hostility, canRequestAlliance: target.can_request_alliance, loot: target.gold_loot_override }, { troops: 0, relation: "enemy", hostility: "non_combatant", canRequestAlliance: false, loot: 200 });
    assert.ok(roster.factions.filter(faction => faction.civ === "Roman Empire").every(faction => faction.relation === "enemy" && faction.can_request_alliance === false), "Roman factions stay enemies and never send alliance offers");
    assert.ok(roster.factions.filter(faction => ["colonia_veterans", "tax_collectors", "roman_supply_depot"].includes(faction.id)).every(faction => faction.relation === "enemy" && faction.hostility === "passive"));
    for (const name of ["Colonia Veterans", "Tax Collectors"]) {
        const faction = roster.factions.find(item => item.name === name);
        assert.equal(faction.civ, "Roman Empire");
    }
    assert.deepEqual(roster.factions.filter(item => ["Colonia Veterans", "Tax Collectors"].includes(item.name)).map(({ name, x, y }) => ({ name, x, y })), [
        { name: "Colonia Veterans", x: 736, y: 213 },
        { name: "Tax Collectors", x: 670, y: 240 }
    ]);
    assert.equal(definition.settings.buildings_enabled, false);
    assert.equal(definition.settings.buildings_unlock_after_defeated, target.id);
    assert.deepEqual(definition.settings.campaign_support, { after_defeated: target.id, share_percent: 50 });
    assert.match(campaignMapEditorHtml, /starting_troops/);
    assert.match(campaignMapEditorHtml, /Stable ID/);
    const attackStep = definition.steps.find(step => step.id === "boudica_first_victory");
    assert.equal(attackStep.trigger.target, target.id);
    assert.equal(attackStep.marker.target, target.id);
    assert.equal(attackStep.guide.target, "target_action");
    assert.equal(attackStep.attack_ratio_on_enter, 1);

    function attackAfter(choice) {
        const machine = campaign.create(definition, undefined, roster);
        machine.update({}, {}, 0);
        machine.advance(null, "boudica_rise_of_the_iceni_intro");
        machine.advance(choice, "boudica_council_decision");
        if (choice === "wait") {
            machine.advance(null, "boudica_council_refusal");
            assert.equal(machine.view().step.id, "boudica_council_decision");
            machine.advance("attack", "boudica_council_decision");
        }
        assert.equal(machine.view().step.id, "boudica_first_victory");
        return machine;
    }
    const direct = attackAfter("attack");
    const reluctant = attackAfter("wait");
    assert.equal(reluctant.view().step.id, attackStep.id);
    const victoryFacts = { defeated_faction_ids: [target.id] };
    const victory = direct.update(victoryFacts, {});
    assert.equal(victory.step.id, "boudica_first_victory_scene");
    assert.equal(victory.step.attack_ratio_on_enter, 0.5);
    direct.advance(null, victory.step.id);
    assert.equal(direct.view().step.id, "boudica_first_expansion");
    const expansion = direct.update({ ...victoryFacts, tiles_gained: 250 }, {});
    assert.equal(expansion.step.id, "boudica_zoom_out");
    assert.equal(expansion.step.pause_game, true);
    assert.equal(direct.update({ ...victoryFacts, tiles_gained: 250, zoom_out_events: 0 }, {}).step.id, "boudica_zoom_out");
    assert.equal(direct.advance(null, "boudica_zoom_out"), false, "the paused exercise cannot be dismissed before the gesture");
    const zoomIn = direct.update({ ...victoryFacts, tiles_gained: 250, zoom_out_events: 1, zoom_in_events: 0 }, {});
    assert.equal(zoomIn.step.id, "boudica_zoom_in");
    assert.equal(zoomIn.step.camera_only, true);
    assert.equal(direct.update({ ...victoryFacts, tiles_gained: 250, zoom_out_events: 1, zoom_in_events: 1 }, {}).step.id, "boudica_camera_drag");
    assert.equal(direct.update({ ...victoryFacts, tiles_gained: 250, zoom_out_events: 1, zoom_in_events: 1, camera_drag_events: 1, touch_controls: 0 }, {}).step.id, "boudica_camera_keys");
    assert.equal(direct.update({ ...victoryFacts, tiles_gained: 250, zoom_out_events: 1, zoom_in_events: 1, camera_drag_events: 1, camera_key_pan_events: 1, touch_controls: 0 }, {}).step.id, "boudica_first_contact");
    assert.equal(direct.view().step.id, "boudica_first_contact");
    const touch = campaign.create(definition, "boudica_camera_drag", roster);
    touch.update({ camera_drag_events: 0, touch_controls: 1 }, {});
    assert.equal(touch.update({ camera_drag_events: 1, touch_controls: 1 }, {}).step.id, "boudica_first_contact", "touch skips the desktop key lesson and early entity inspection");
    assert.deepEqual(direct.view().step.trigger.targets, ["stonea", "snettisham", "thetford"]);
    const stoneaContact = direct.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["stonea"] }, {});
    assert.equal(stoneaContact.reaction, "contact_stonea");
    assert.equal(stoneaContact.reactionTarget, "stonea");
    assert.equal(stoneaContact.reactionData.outcome, "allied");
    assert.equal(direct.advance(null, "reaction-contact_stonea@stonea"), true);
    const snettishamContact = direct.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["stonea", "snettisham"] }, {});
    assert.equal(snettishamContact.reaction, "contact_snettisham");
    assert.equal(snettishamContact.reactionTarget, "snettisham");
    assert.equal(snettishamContact.choices.length, 2);
    assert.equal(snettishamContact.choices.find(choice => choice.id === "accept").gold_cost, 200);
    assert.equal(snettishamContact.choices.find(choice => choice.id === "refuse").relation, "enemy");
    assert.equal(direct.advance("refuse", "reaction-contact_snettisham@snettisham"), true);
    const thetfordContact = direct.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["stonea", "snettisham", "thetford"] }, {});
    assert.equal(thetfordContact.reaction, "contact_thetford");
    assert.deepEqual(thetfordContact.choices.map(choice => [choice.id, choice.relation, choice.gold_cost]), [
        ["respect", "allied", 0], ["demand", "enemy", 0]
    ]);
    assert.equal(direct.advance("respect", "reaction-contact_thetford@thetford"), true);
    const earlyTrinovantesContact = direct.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["stonea", "snettisham", "thetford", "trinovantian_farms", "trinovantes"] }, {});
    assert.equal(earlyTrinovantesContact.reaction, undefined, "Trinovantes terms wait until the three Roman outposts fall");
    assert.equal(direct.view().step.id, "boudica_outpost_colonia_veterans");
    const coloniaDown = direct.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["stonea", "snettisham", "thetford", "trinovantian_farms", "trinovantes"], defeated_faction_ids: ["roman_outpost", "colonia_veterans"] }, {});
    assert.equal(coloniaDown.step.id, "boudica_outpost_tax_collectors");
    const taxOfficeDown = direct.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["stonea", "snettisham", "thetford", "trinovantian_farms", "trinovantes"], defeated_faction_ids: ["roman_outpost", "colonia_veterans", "tax_collectors"] }, {});
    assert.equal(taxOfficeDown.step.id, "boudica_outpost_roman_supply_depot");
    const lastOutpostDown = direct.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["stonea", "snettisham", "thetford", "trinovantian_farms", "trinovantes"], defeated_faction_ids: ["roman_outpost", "colonia_veterans", "tax_collectors", "roman_supply_depot"] }, {});
    assert.equal(lastOutpostDown.reaction, "contact_trinovantes");
    assert.equal(direct.advance("accept", "reaction-contact_trinovantes@trinovantes"), true);
    const allianceFormed = direct.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["stonea", "snettisham", "thetford", "trinovantian_farms", "trinovantes"], defeated_faction_ids: ["roman_outpost", "colonia_veterans", "tax_collectors", "roman_supply_depot"], alliance_faction_ids: ["trinovantes"] }, {});
    assert.equal(allianceFormed.reaction, "neutral_contact_terms");
    assert.equal(allianceFormed.reactionTarget, "trinovantian_farms");
    assert.equal(direct.advance("accept", "reaction-neutral_contact_terms@trinovantian_farms"), true);
    const supportWait = direct.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["stonea", "snettisham", "thetford", "trinovantian_farms", "trinovantes"], defeated_faction_ids: ["roman_outpost", "colonia_veterans", "tax_collectors", "roman_supply_depot"], alliance_faction_ids: ["trinovantes"] }, {});
    assert.equal(supportWait.step.id, "boudica_ally_support_wait");
    const declined = campaign.create(definition, "boudica_outpost_roman_supply_depot", roster);
    const finalOutpostFacts = { tiles_gained: 250, defeated_faction_ids: ["roman_supply_depot"], contact_faction_ids: [], alliance_faction_ids: [] };
    assert.equal(declined.update(finalOutpostFacts, {}).step.id, "boudica_trinovantes_alliance");
    const trinovantesOffer = declined.update({ ...finalOutpostFacts, contact_faction_ids: ["trinovantes"] }, {});
    assert.equal(trinovantesOffer.reaction, "contact_trinovantes");
    assert.equal(declined.advance("later", "reaction-contact_trinovantes@trinovantes"), true);
    const afterDecline = declined.update({ ...finalOutpostFacts, contact_faction_ids: ["trinovantes"] }, {});
    assert.equal(afterDecline.step.id, "boudica_trinovantes_alliance", "declining the story offer leaves the real alliance objective active");
    assert.equal(afterDecline.reaction, undefined, "the disabled Trinovantes do not fall through to generic neutral terms");
    assert.equal(declined.update({ ...finalOutpostFacts, contact_faction_ids: ["trinovantes"], alliance_faction_ids: ["trinovantes"] }, {}).step.id, "boudica_ally_support_wait", "a later map-menu alliance completes the objective");
    const genericClanContact = direct.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["stonea", "snettisham", "thetford", "trinovantian_farms", "trinovantes", "catuvellauni"] }, {});
    assert.equal(genericClanContact.reaction, "neutral_contact_terms");
    assert.equal(genericClanContact.reactionTarget, "catuvellauni");
    assert.equal(genericClanContact.step.speaker, undefined, "generic clan terms resolve their portrait from the reaction target");
    const outOfOrder = campaign.create(definition, "boudica_first_contact", roster);
    outOfOrder.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: [] }, {});
    assert.equal(outOfOrder.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["thetford"] }, {}).reaction, "contact_thetford");
    const contactsOnly = campaign.create({ ...definition, reactions: [] }, "boudica_first_contact");
    contactsOnly.update({ ...victoryFacts, tiles_gained: 250, contact_faction_ids: [] }, {});
    const twoContacts = { ...victoryFacts, tiles_gained: 250, contact_faction_ids: ["snettisham", "thetford"] };
    assert.equal(contactsOnly.update(twoContacts, {}).progress.current, 2);
    assert.equal(contactsOnly.view().progress.target, 3);
    const threeContacts = { ...twoContacts, contact_faction_ids: ["stonea", "snettisham", "thetford"] };
    assert.equal(contactsOnly.update(threeContacts, {}).step.id, "boudica_outpost_colonia_veterans");
    const support = definition.steps.find(step => step.id === "boudica_ally_support_wait");
    assert.deepEqual(support.trigger, { type: "support", value: 1, scope: "total" });
    const iceniReactions = definition.reactions.filter(reaction => ["stonea", "snettisham", "thetford"].includes(reaction.when.target));
    assert.equal(iceniReactions.length, 3);
    assert.deepEqual(roster.factions.filter(faction => ["stonea", "snettisham", "thetford"].includes(faction.id)).map(faction => faction.name).sort(), ["Snettisham", "Stonea", "Thetford"]);
    assert.equal(roster.factions.find(faction => faction.name === "Stonea").support_interval_seconds, 15);
    assert.equal(definition.steps.find(step => step.id === "boudica_transfer_send").trigger.recipient, "stonea");
    assert.equal(definition.steps.find(step => step.id === "boudica_transfer_target").marker.target, "stonea");
    for (const removed of ["Iceni Coast", "Venta Icenorum"]) assert.equal(roster.factions.some(faction => faction.name === removed), false);
    for (const locale of ["en", "es"]) {
        for (const key of ["tutorial.boudica_snettisham_tribute_title", "tutorial.boudica_snettisham_tribute_body", "tutorial.boudica_snettisham_refuse_detail", "tutorial.boudica_thetford_respect_title", "tutorial.boudica_thetford_respect_body", "tutorial.boudica_thetford_respect", "tutorial.boudica_thetford_respect_detail", "tutorial.boudica_thetford_demand", "tutorial.boudica_thetford_demand_detail"]) assert.ok(definition.strings[locale][key], `${locale}/${key}`);
        assert.match(definition.strings[locale]["tutorial.boudica_transfer_target_title"], /Stonea/);
        assert.match(definition.strings[locale]["tutorial.boudica_transfer_target_hint"], /Stonea/);
    }
    for (const reaction of iceniReactions) {
        const factionId = reaction.when.target;
        const faction = roster.factions.find(item => item.id === factionId);
        assert.ok(faction.support_interval_seconds >= 5);
        assert.equal(definition.steps.some(step => step.id === reaction.id), false);
        assert.equal(reaction.when.type, "contact");
        assert.equal(reaction.after, undefined);
    }
    const pactFactions = roster.factions.filter(item => ["Trinovantes", "Trinovantian Farms"].includes(item.name));
    assert.ok(pactFactions.every(item => item.relation === "neutral" && !item.alliance_group));
    const unsupportedPayout = JSON.parse(JSON.stringify(roster));
    unsupportedPayout.factions.find(item => item.name === "Colonia Veterans").support_interval_seconds = 20;
    assert.deepEqual(campaign.validate(definition, unsupportedPayout, { hasText: () => true, hasAvatar: () => true }).errors, [], "any contacted campaign entity may be configured to send support");
    const trinovantesStep = definition.steps.find(step => step.id === "boudica_trinovantes_alliance");
    assert.equal(trinovantesStep.trigger.target, "trinovantes");
    const pactCheck = campaign.create({
        entry: "alliance_check",
        steps: [{ id: "alliance_check", type: "objective", title_key: "tutorial.test", trigger: trinovantesStep.trigger }]
    });
    pactCheck.update({ alliance_faction_ids: ["other_tribe"] }, {}, 0);
    assert.equal(pactCheck.view().progress.current, 0);
    pactCheck.update({ alliance_faction_ids: ["trinovantes"] }, {}, 1000);
    assert.equal(pactCheck.view().progress.current, 1);
    assert.equal(definition.steps.find(step => step.id === "boudica_choose_city").guide.target, "dock_city");
    assert.deepEqual(definition.steps.find(step => step.id === "boudica_build_city").trigger, { type: "city", value: 1, scope: "step" });
    assert.deepEqual(definition.steps.find(step => step.id === "boudica_first_expansion").trigger, { type: "territory", value: 250, scope: "step" });
    assert.deepEqual(step("boudica_zoom_out").trigger, { type: "zoom_out", value: 1, scope: "step" });
    assert.equal(step("boudica_zoom_out").pause_game, true);
    assert.equal(step("boudica_zoom_out").camera_only, true);
    assert.equal(step("boudica_zoom_out").guide.gesture, "zoom_out");
    assert.equal(step("boudica_zoom_out").next, "boudica_zoom_in");
    assert.deepEqual(step("boudica_zoom_in").trigger, { type: "zoom_in", value: 1, scope: "step" });
    assert.deepEqual(step("boudica_camera_drag").trigger, { type: "camera_drag", value: 1, scope: "step" });
    assert.equal(step("boudica_camera_drag").routes[0].when.fact, "touch_controls");
    assert.deepEqual(step("boudica_camera_keys").trigger, { type: "camera_key_pan", value: 1, scope: "step" });
    assert.deepEqual(step("boudica_camera_hover").trigger, { type: "hover", value: 1, scope: "step" });
    for (const id of ["boudica_zoom_out", "boudica_zoom_in", "boudica_camera_drag", "boudica_camera_keys", "boudica_camera_hover"]) {
        assert.equal(step(id).pause_game, true, `${id} keeps the simulation paused`);
        assert.equal(step(id).camera_only, true, `${id} allows camera inputs only`);
    }
    assert.equal(definition.strings.en["tutorial.boudica_zoom_out_title"], "Zoom out");
    assert.equal(definition.strings.es["tutorial.boudica_zoom_out_title"], "Aleja el mapa");
    for (const locale of ["en", "es"]) {
        for (const input of ["pinch", "trackpad", "wheel"]) assert.ok(definition.strings[locale][`tutorial.boudica_zoom_out_${input}_hint`]);
        for (const input of ["pinch", "trackpad", "wheel"]) assert.ok(definition.strings[locale][`tutorial.boudica_zoom_in_${input}_label`]);
        assert.ok(definition.strings[locale]["tutorial.boudica_camera_hover_title"]);
    }
    assert.match(windowInput, /record_tutorial_zoom\(self\.input\.camera_zoom - previous_zoom\)/);
    assert.match(windowInput, /record_tutorial_zoom\(self\.input\.target_zoom - previous_zoom\)/);
    assert.match(windowInput, /tutorial_camera_only[\s\S]*?record_tutorial_camera_key_pan\(\)/);
    assert.match(windowInput, /tutorial_camera_only[\s\S]*?record_tutorial_camera_drag\(\)/);
    const pointerMoveStart = windowInput.indexOf("fn handle_pointer_move(");
    const wheelStart = windowInput.indexOf("fn handle_wheel(", pointerMoveStart);
    const pointerMove = windowInput.slice(pointerMoveStart, wheelStart);
    assert.match(pointerMove, /matches!\(source, winit::event::PointerSource::Mouse\)[\s\S]*?self\.input\.dragging[\s\S]*?map_pointer_start[\s\S]*?\|start\| !start\.is_touch\)/);
    assert.match(windowInput, /fn record_tutorial_camera_drag_if_moved\([\s\S]*?tutorial_camera_drag_recorded[\s\S]*?moved_sq > 400\.0[\s\S]*?tutorial_camera_drag_recorded = true/);
    assert.match(tutorial, /hoveredEntityId !== null && hoveredEntityId !== runtime\.hoveredEntityId/);
    assert.match(webMenu, /SetTutorialPaused \{\s*paused,\s*camera_only,\s*\}/);
    assert.match(webMenu, /tutorial_camera_only = paused && camera_only[\s\S]*?if self\.ui\.tutorial_camera_only \{\s*self\.sim\.offline_intents\.clear\(\);/);
    assert.match(intentInput, /if self\.ui\.tutorial_camera_only\s*&&\s*!matches!\(&intent, sow_core::protocol::GameplayIntent::Resign\)/);
    assert.match(webMenu, /WebMenuCommand::Surrender => \{\s*self\.send_intent\(sow_core::protocol::GameplayIntent::Resign\)/);
    assert.match(mapClick, /if self\.ui\.tutorial_camera_only/);
    assert.match(appStateSource, /pub tutorial_camera_only: bool/);
    assert.match(surfaceSource, /process_camera_zoom[\s\S]*?self\.input\.target_zoom = self\.input\.camera_zoom/);
    assert.match(webMenu, /WebMenuCommand::FocusWorld \{ x, y \}[\s\S]*?camera_focus_target = Some\(\(x, y\)\);\s*self\.input\.tutorial_camera_focus = true/);
    assert.match(cameraFrameUiSource, /target_zoom - self\.input\.camera_zoom\)\.abs\(\) < 0\.001/);
    assert.match(cameraFrameUiSource, /clamp_camera_offset\(/);
    assert.doesNotMatch(actionsSource, /record_tutorial_zoom/);
    assert.match(tutorial, /step\.trigger\.type === "territory" && step\.guide\.target === "expand" && view && view\.progress\.current > 0\) return null/);
    assert.match(tutorial, /window\.SOWCampaign\.zoomInputMode/);
    assert.match(tutorial, /SOW_isAndroidTwa/);
    assert.match(tutorial, /userAgentData/);
    assert.deepEqual(definition.steps.find(step => step.id === "boudica_outpost_colonia_veterans").trigger, { type: "defeated", target: "colonia_veterans", scope: "total" });
    assert.deepEqual(definition.steps.find(step => step.id === "boudica_outpost_tax_collectors").trigger, { type: "defeated", target: "tax_collectors", scope: "total" });
    assert.deepEqual(definition.steps.find(step => step.id === "boudica_outpost_roman_supply_depot").trigger, { type: "defeated", target: "roman_supply_depot", scope: "total" });
    assert.match(webMenu, /"notifications": notifications/);
    assert.match(webMenu, /"map_feedback": map_feedback/);
    assert.match(simUpdateSource, /push_notification_for_players\(/);
    assert.match(simUpdateSource, /\[Some\(transfer\.sender_id\), Some\(my_id\)\]/);
    assert.doesNotMatch(simUpdateSource, /hud\.resource_sent_/);
    assert.doesNotMatch(snapshotFxSource, /hud\.(alliance_request_sent|resource_request_sent)/);
    assert.match(snapshotFxSource, /if !nuke_alert_targets_player\(alert\.owner_id, victim_id, my_id\) \{\s*continue;\s*\}/);
    assert.doesNotMatch(simEventsSource, /push_notification_for_players\(/);
    assert.doesNotMatch(mapClick, /hud\.(attack_launched|observer_feedback|water_feedback)/);
    assert.match(hudStateSource, /push_map_feedback/);
    assert.match(hud, /function renderMapFeedback\(entry\)/);
    assert.match(webMenu, /"avatars": avatars/);
    assert.match(webMenu, /notification_avatar_identities\(\s*players,\s*notification\.players/);
    assert.match(webMenu, /serde_json::to_value\(avatar\)/);
    assert.doesNotMatch(webMenu, /hud\.players/);
    assert.doesNotMatch(webMenu, /notification\.players\[0\] == Some\(player_id\)/);
    assert.doesNotMatch(hudStateSource, /pub players: Vec<PlayerSnapshot>/);
    assert.match(avatarIdentitySource, /serde\(tag = "kind", rename_all = "snake_case"\)[\s\S]*pub enum AvatarIdentity/);
    assert.match(avatarIdentitySource, /pub fn avatar_identity_for_player_id/);
    assert.match(avatarIdentitySource, /pub fn notification_avatar_identities/);
    assert.match(nameplatesSource, /sow_core::player::avatar_identity_ref\(player\)/);
    assert.match(hud, /notificationCards = Array\.from\(\{ length: 3 \}/);
    assert.match(hud, /activeNotifications\.length < 3/);
    assert.match(hud, /function renderNotifications\(entries, forceRefresh\)/);
    assert.match(hud, /if \(!changed\) return;/);
    assert.match(hud, /renderNotifications\(\[\], true\)/);
    assert.match(hud, /renderHud\(true\)/);
    assert.match(hud, /gameplay\/avatars\/null\.webp/);
    assert.match(hud, /kind === "portrait"/);
    assert.match(hud, /kind === "emblem"/);
    assert.match(hudCss, /\.sow-hud__notification-avatar--overlap/);
    assert.match(hud, /var slug = kind === "portrait"/);
    assert.match(hud, /slot\.image\.hidden = false/);
    assert.match(hudCss, /\.sow-hud__notification--support/);
    assert.match(hudCss, /\.sow-hud__notification--contextual/);
    assert.match(campaignModule, /support_interval_seconds/);
    assert.match(campaignMapEditorHtml, /support_interval_seconds/);
    const invalidRatio = JSON.parse(JSON.stringify(definition));
    invalidRatio.steps.find(step => step.id === attackStep.id).attack_ratio_on_enter = 1.01;
    assert.ok(campaign.validate(invalidRatio, roster, { hasText: () => true, hasAvatar: () => true }).errors.some(issue => issue.field === "attack_ratio_on_enter"));
    assert.match(campaignEditor, /Set send percentage on entry/);
    assert.match(campaignEditor, /previewActionStep/);
    assert.match(campaignEditor, /if \(actionRatio != null\) \$\("#sow-hud-slider"\)\.value/);
    assert.match(tutorial, /machineView\.step\.id !== runtime\.lastActionStepId/);
});

test("gameplay chrome: vertical right panel with exit on top, fps in dock, notices on the left", () => {
    const statusRight = hud.slice(hud.indexOf("sow-hud__status-right"), hud.indexOf("</header>"));
    const exitAt = statusRight.indexOf("prompt_surrender");
    assert.ok(exitAt > 0 && exitAt < statusRight.indexOf("toggle_settings")
        && exitAt < statusRight.indexOf("toggle_inbox")
        && exitAt < statusRight.indexOf("toggle_leaderboard"),
        "exit stays first in the vertical panel");
    assert.ok(!statusRight.includes("sow-hud-fps"), "fps meter leaves the topbar");
    assert.match(hud, /sow-hud__res-gold[\s\S]{0,400}sow-hud-fps/);
    assert.match(hudCss, /\.sow-hud__status-right \{[^}]*flex-direction: column/);
    assert.match(hudCss, /\.sow-hud__status-left:not\(:has\(> :not\(\.hidden\)\)\)/);
    assert.match(hudCss, /\.sow-hud__notifications \{[^}]*left: max\(16px, var\(--sow-sal\)\)/);
    assert.match(campaignView, /objective\.hidden = false;/);
    assert.doesNotMatch(campaignView, /objective\.hidden = modal/);
});

test("campaign support reactions queue by delivery order, wait for their gate and preserve objective progress", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "test", entry: "gate",
        settings: { buildings_enabled: false, starting_troops: 1000 },
        steps: [
            { id: "gate", type: "objective", title_key: "gate", trigger: { type: "territory", value: 1, scope: "step" }, next: "talk" },
            { id: "talk", type: "scene", title_key: "talk", next: "contact" },
            { id: "contact", type: "objective", title_key: "contact", trigger: { type: "contact", targets: ["a", "b"], value: 2, scope: "episode" }, next: "end" },
            { id: "end", type: "end", title_key: "end" }
        ],
        reactions: [
            { id: "aid_a", after: "gate", when: { type: "support", target: "a" }, title_key: "aid.a.title", body_key: "aid.a.body" },
            { id: "aid_b", after: "gate", when: { type: "support", target: "b" }, title_key: "aid.b.title", body_key: "aid.b.body" }
        ]
    };
    const machine = campaign.create(definition);
    const receipts = {
        a: { deliveries: 2, first_tick: 9 },
        b: { deliveries: 1, first_tick: 3 }
    };
    machine.update({ tiles_gained: 0, support_deliveries_by_faction_id: {} }, {});
    assert.equal(machine.view().step.id, "gate");
    machine.update({ tiles_gained: 1, support_deliveries_by_faction_id: {} }, {});
    assert.equal(machine.view().step.id, "talk");
    const deferred = machine.update({ contact_faction_ids: [], support_deliveries_by_faction_id: receipts }, {});
    assert.equal(deferred.reaction, undefined, "support received during a scene waits until gameplay resumes");
    assert.equal(deferred.step.id, "talk");
    machine.advance(null, "talk");
    assert.equal(machine.view().step.id, "contact");

    const firstReaction = machine.update({ contact_faction_ids: ["a"], support_deliveries_by_faction_id: receipts }, {});
    assert.equal(firstReaction.reaction, "aid_b");
    assert.equal(firstReaction.paused, true);
    assert.equal(firstReaction.progress.current, 1);
    assert.equal(machine.advance(null, "reaction-aid_b"), true);
    assert.equal(machine.advance(null, "reaction-aid_b"), false);
    const secondReaction = machine.update({ contact_faction_ids: ["a"], support_deliveries_by_faction_id: receipts }, {});
    assert.equal(secondReaction.reaction, "aid_a");
    machine.advance(null, "reaction-aid_a");
    assert.equal(machine.update({ contact_faction_ids: ["a"], support_deliveries_by_faction_id: receipts }, {}).step.id, "contact");
    assert.equal(machine.view().progress.current, 1);
    assert.deepEqual(machine.state.reactionsShown, ["aid_b", "aid_a"]);
    assert.deepEqual(campaign.create(definition).state.reactionsShown, [], "a new run starts with no previous ally responses shown");
});

test("first-contact responses open on the contact fact and queue each faction once", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "instant_contact", entry: "objective",
        settings: { buildings_enabled: false, starting_troops: 1000 },
        steps: [
            { id: "objective", type: "objective", title_key: "objective", trigger: { type: "territory", value: 10, scope: "total" }, next: "after" },
            { id: "after", type: "scene", title_key: "after", next: "end" },
            { id: "end", type: "end", title_key: "end" }
        ],
        reactions: [
            { id: "contact_a", when: { type: "contact", target: "a" }, title_key: "a", body_key: "a_body" },
            { id: "contact_b", when: { type: "contact", target: "b" }, title_key: "b", body_key: "b_body" }
        ]
    });
    machine.update({ tiles_gained: 0, contact_faction_ids: [] }, {});
    const first = machine.update({ tiles_gained: 0, contact_faction_ids: ["b"] }, {});
    assert.equal(first.reaction, "contact_b");
    machine.advance(null, "reaction-contact_b@b");
    const second = machine.update({ tiles_gained: 0, contact_faction_ids: ["b", "a"] }, {});
    assert.equal(second.reaction, "contact_a");
    machine.advance(null, "reaction-contact_a@a");
    assert.equal(machine.update({ tiles_gained: 0, contact_faction_ids: ["b", "a"] }, {}).step.id, "objective");
    assert.match(campaignEditor, /Add contact response/);
    assert.match(campaignEditor, /On contact with/);
    assert.match(campaignEditor, /Simulate first " \+ \(isContact \? "contact"/);
});

test("first Iceni contact is friendly, then each neutral contact negotiates independently", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const roster = { factions: [
        { id: "kin_one", name: "Kin One", relation: "neutral" },
        { id: "kin_two", name: "Kin Two", relation: "neutral" },
        { id: "neutral", name: "Neutral", relation: "neutral" },
        { id: "neutral_two", name: "Neutral Two", relation: "neutral" },
        { id: "rome", name: "Rome", relation: "enemy" }
    ] };
    const definition = {
        version: 2, episode_id: "contact_order", entry: "objective",
        settings: { buildings_enabled: false, starting_troops: 1000 },
        steps: [
            { id: "objective", type: "objective", title_key: "objective", trigger: { type: "territory", value: 10, scope: "total" }, next: "end" },
            { id: "end", type: "end", title_key: "end" }
        ],
        reactions: [
            { id: "first", when: { type: "first_contact", targets: ["kin_one", "kin_two"] }, outcome: "allied", title_key: "first_title", body_key: "first_body" },
            { id: "kin_two", when: { type: "contact", target: "kin_two" }, choices: [{ id: "pay", label_key: "pay", relation: "allied", gold_cost: 200 }, { id: "refuse", label_key: "refuse", relation: "enemy" }], title_key: "kin_title", body_key: "kin_body" },
            { id: "neutral_terms", when: { type: "contact", relation: "neutral" }, choices: [{ id: "pay", label_key: "pay", relation: "allied", gold_cost: 200 }, { id: "refuse", label_key: "refuse", relation: "enemy" }], title_key: "neutral_title", body_key: "neutral_body" }
        ]
    };
    const machine = campaign.create(definition, undefined, roster);
    const contacts = { tiles_gained: 0, contact_faction_ids: ["rome", "neutral", "neutral_two", "kin_two", "kin_one"] };
    const first = machine.update(contacts, {});
    assert.equal(first.reaction, "first");
    assert.equal(first.reactionTarget, "kin_one");
    assert.equal(machine.advance(null, "reaction-first@kin_one"), true);
    const second = machine.update(contacts, {});
    assert.equal(second.reaction, "kin_two", "specific faction dialogue takes precedence over the neutral template");
    assert.equal(second.reactionTarget, "kin_two");
    assert.equal(machine.advance("pay", "reaction-kin_two@kin_two"), true);
    const third = machine.update(contacts, {});
    assert.equal(third.reaction, "neutral_terms");
    assert.equal(third.reactionTarget, "neutral");
    assert.equal(third.choices.find(choice => choice.id === "pay").gold_cost, 200);
    assert.equal(machine.update(contacts, {}).reactionTarget, "neutral", "a pending choice stays active until selected");
    assert.equal(machine.advance("refuse", "reaction-neutral_terms@neutral"), true);
    const fourth = machine.update(contacts, {});
    assert.equal(fourth.reaction, "neutral_terms");
    assert.equal(fourth.reactionTarget, "neutral_two", "each initially neutral faction negotiates on its own first contact");
});

test("campaign negotiation uses the canonical HUD gold balance, not player-list fields", async () => {
    async function chooseAtBalance(balance) {
        const commands = [], root = { hidden: true, isConnected: false };
        let viewOptions, rendered;
        const document = {
            body: { appendChild(node) { node.isConnected = true; } },
            documentElement: { dir: "ltr" },
            getElementById: () => null,
            createElement: () => root,
            addEventListener() {}
        };
        const model = {
            step: { id: "terms", type: "choice" }, paused: true, state: { choices: {} },
            reactionData: { id: "terms", choices: [
                { id: "pay", relation: "allied", gold_cost: 200 },
                { id: "refuse", relation: "enemy", gold_cost: 0 }
            ] },
            reactionTarget: "snettisham",
            choices: [
                { id: "pay", label_key: "pay", relation: "allied", gold_cost: 200 },
                { id: "refuse", label_key: "refuse", relation: "enemy", gold_cost: 0 }
            ]
        };
        let advanced;
        const window = {
            addEventListener() {},
            SOW_menu_command(message) { commands.push(JSON.parse(message)); },
            SOWCampaign: {
                validate: () => ({ errors: [] }), UI_TARGETS: {}, zoomInputMode: () => "wheel",
                create: () => ({ state: { choices: {} }, update: () => model, view: () => model,
                    advance(choiceId) { advanced = choiceId; return true; } })
            },
            SOWCampaignView: { mount: (_root, options) => {
                viewOptions = options;
                return { render(value) { rendered = value; }, destroy() {} };
            } }
        };
        vm.runInNewContext(tutorial, { window, document, performance: { now: () => 0 }, console,
            fetch(url) {
                const data = url.endsWith(".triggers.json")
                    ? { episode_id: "boudica", settings: {}, strings: {}, speakers: {} }
                    : { factions: [] };
                return Promise.resolve({ ok: true, json: () => Promise.resolve(data) });
            }
        });
        window.SOW_tutorial_state_update({ phase: "Playing", hud: {
            gold: balance,
            players: [{ id: 1, name: "Boudica", is_me: true }, { id: 2, name: "Snettisham", campaign_faction_id: "snettisham", centroid_x: 12, centroid_y: 34 }],
            settings: {}, tutorial: { active: true, episode_id: "boudica", facts: {} }
        } });
        await new Promise(setImmediate);
        const displayed = rendered.choices[0];
        viewOptions.onChoice("pay");
        return { commands, displayed, advanced };
    }

    const funded = await chooseAtBalance(6020);
    assert.equal(funded.displayed.gold_available, 6020);
    assert.equal(funded.displayed.gold_insufficient, false);
    const focus = funded.commands.find(command => command.type === "focus_world");
    assert.deepEqual([focus.x, focus.y], [12.5, 34.5]);
    assert.equal(funded.commands.find(command => command.type === "resolve_campaign_diplomacy").relation, "allied");
    assert.equal(funded.commands.find(command => command.type === "resolve_campaign_diplomacy").gold_cost, 200);
    assert.equal(funded.advanced, "pay");

    const short = await chooseAtBalance(0);
    assert.equal(short.displayed.gold_insufficient, true);
    assert.equal(short.commands.find(command => command.type === "resolve_campaign_diplomacy").relation, "enemy");
    assert.equal(short.commands.find(command => command.type === "resolve_campaign_diplomacy").gold_cost, 0);
    assert.equal(short.advanced, "refuse");
});

test("campaign objectives distinguish the intended contact, alliance, fleet, transfer and foundry action", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    function currentFor(trigger, before, after) {
        const machine = campaign.create({
            version: 2, episode_id: "fact_test", entry: "objective",
            settings: { buildings_enabled: false, starting_troops: 1000 },
            steps: [
                { id: "objective", type: "objective", title_key: "test", trigger: trigger, next: "pending" }
            ]
        });
        machine.update(before, {});
        return machine.update(after, {}).progress.current;
    }
    assert.equal(currentFor({ type: "contact", targets: ["snettisham", "stonea"], scope: "total" }, { contact_faction_ids: [] }, { contact_faction_ids: ["stonea"] }), 1, "multi-contact targets count stable faction IDs");
    assert.equal(currentFor({ type: "alliance", target: "catuvellauni", value: 1, scope: "total" }, { alliance_faction_ids: [] }, { alliance_faction_ids: ["other_tribe"] }), 0);
    assert.equal(currentFor({ type: "fleet", unit: "TransportShip", target: "legio_xiv", value: 1, scope: "step" },
        { fleets_by_type: {}, transport_fleets_by_faction_id: {} },
        { fleets_by_type: { TradeShip: 1 }, transport_fleets_by_faction_id: { legio_xiv: 1 } }), 0);
    assert.equal(currentFor({ type: "fleet", unit: "TransportShip", target: "legio_xiv", value: 1, scope: "step" },
        { fleets_by_type: {}, transport_fleets_by_faction_id: {} },
        { fleets_by_type: { TransportShip: 1 }, transport_fleets_by_faction_id: { other_legion: 1 } }), 0);
    assert.equal(currentFor({ type: "fleet", unit: "TransportShip", target: "legio_xiv", value: 1, scope: "step" },
        { fleets_by_type: {}, transport_fleets_by_faction_id: {} },
        { fleets_by_type: { TransportShip: 1 }, transport_fleets_by_faction_id: { legio_xiv: 1 } }), 1);
    const transfer = { type: "resource_transfer", recipient: "snettisham", resources: ["gold", "troops"], value: 1, scope: "step" };
    const emptyTransfer = { resource_transfers_by_recipient_faction_id: {} };
    assert.equal(currentFor(transfer, emptyTransfer, { resource_transfers_by_recipient_faction_id: { other: { gold_troops: 1 } } }), 0);
    assert.equal(currentFor(transfer, emptyTransfer, { resource_transfers_by_recipient_faction_id: { snettisham: { gold: 1, troops: 0, gold_troops: 0 } } }), 0);
    assert.equal(currentFor(transfer, emptyTransfer, { resource_transfers_by_recipient_faction_id: { snettisham: { gold: 1, troops: 1, gold_troops: 1 } } }), 1);
    assert.equal(currentFor({ type: "foundry_level", value: 1, scope: "step" }, { city_levels: 0, foundry_level: 0 }, { city_levels: 3, foundry_level: 0 }), 0);
    assert.equal(currentFor({ type: "foundry_level", value: 1, scope: "step" }, { city_levels: 0, foundry_level: 0 }, { city_levels: 0, foundry_level: 1 }), 1);
});

test("campaign validation rejects unsupported rules and duplicate event responses", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const roster = { map: "test", player_spawn: [0, 0], factions: [{ id: "ally", name: "Ally", x: 1, y: 0, starting_troops: 500, relation: "neutral", civ: "Iceni Kingdom", leader: "Boudica", support_interval_seconds: 5 }] };
    const definition = {
        version: 2, episode_id: "fact_test", default_locale: "en", entry: "objective",
        settings: { buildings_enabled: false, starting_troops: 1000 },
        speakers: {}, strings: { en: { "tutorial.objective": "Objective", "tutorial.end": "End", "tutorial.aid_title": "Aid", "tutorial.aid_body": "Support arrived." } },
        steps: [
            { id: "objective", type: "objective", title_key: "tutorial.objective", trigger: { type: "fleet", unit: "TransportShip", target: "ally", value: 1, scope: "step" }, next: "end" },
            { id: "end", type: "end", title_key: "tutorial.end" }
        ],
        reactions: [{ id: "aid", after: "objective", when: { type: "support", target: "ally" }, title_key: "tutorial.aid_title", body_key: "tutorial.aid_body" }]
    };
    assert.deepEqual(campaign.validate(definition, roster).errors, []);
    const supportedCombatBehavior = JSON.parse(JSON.stringify(roster));
    supportedCombatBehavior.factions[0].hostility = "aggressive";
    assert.deepEqual(campaign.validate(definition, supportedCombatBehavior).errors, []);
    const invalidHostility = JSON.parse(JSON.stringify(roster));
    invalidHostility.factions[0].hostility = "friendly";
    assert.ok(campaign.validate(definition, invalidHostility).errors.some(issue => issue.field === "roster.factions.hostility"));
    const legacyTransportTarget = JSON.parse(JSON.stringify(definition));
    delete legacyTransportTarget.steps[0].trigger.unit;
    assert.deepEqual(campaign.validate(legacyTransportTarget, roster).errors, [], "version 2 target-only fleet goals remain valid");
    const invalidUnit = JSON.parse(JSON.stringify(definition));
    invalidUnit.steps[0].trigger.unit = "Submarine";
    assert.ok(campaign.validate(invalidUnit, roster).errors.some(issue => issue.field === "trigger.unit"));
    const invalidTarget = JSON.parse(JSON.stringify(definition));
    invalidTarget.steps[0].trigger.target = "missing_legion";
    assert.ok(campaign.validate(invalidTarget, roster).errors.some(issue => issue.field === "trigger.target"));
    const invalidRecipient = JSON.parse(JSON.stringify(definition));
    invalidRecipient.steps[0].trigger = { type: "resource_transfer", recipient: "missing_faction", resources: ["gold"], value: 1, scope: "step" };
    assert.ok(campaign.validate(invalidRecipient, roster).errors.some(issue => issue.field === "trigger.recipient"));
    const invalidResources = JSON.parse(JSON.stringify(definition));
    invalidResources.steps[0].trigger = { type: "resource_transfer", recipient: "ally", resources: ["crowns"], value: 1, scope: "step" };
    assert.ok(campaign.validate(invalidResources, roster).errors.some(issue => issue.field === "trigger.resources"));
    const duplicate = JSON.parse(JSON.stringify(definition));
    duplicate.reactions.push({ ...duplicate.reactions[0], id: "aid_again" });
    assert.ok(campaign.validate(duplicate, roster).errors.some(issue => issue.field === "reactions.when"));
    const contactResponse = JSON.parse(JSON.stringify(definition));
    contactResponse.reactions.push({ id: "contact", when: { type: "contact", target: "ally" }, title_key: "tutorial.aid_title", body_key: "tutorial.aid_body" });
    assert.deepEqual(campaign.validate(contactResponse, roster).errors, [], "contact and support can each have their own response");
    const duplicateContact = JSON.parse(JSON.stringify(contactResponse));
    duplicateContact.reactions.push({ ...duplicateContact.reactions[1], id: "contact_again" });
    assert.ok(campaign.validate(duplicateContact, roster).errors.some(issue => issue.field === "reactions.when"));
    const delayedContact = JSON.parse(JSON.stringify(contactResponse));
    delayedContact.reactions[1].after = "missing_objective";
    assert.ok(campaign.validate(delayedContact, roster).errors.some(issue => issue.field === "reactions.after"));
});

test("all 53 HUD message classes have one explicit presentation", () => {
    const local = [
        "action_unavailable", "attack_out_of_range", "fleet_need_troops", "fleet_invalid_target",
        "fleet_own_target", "fleet_teammate", "fleet_alliance", "fleet_no_port",
        "fleet_no_water_access", "fleet_no_landing_shore", "fleet_no_water_path", "need_gold",
        "build_owned_land", "build_land", "build_spacing_city", "build_spacing_structure",
        "build_no_space", "build_no_farm_plots", "building_in_progress", "spawn_too_close",
        "alliance_request_pending", "alliance_renewal_pending", "resources_allies_only"
    ];
    const global = [
        "elimination", "elimination_bounty", "elimination_assist", "attack_incoming",
        "alliance_request", "resource_request", "alliance_formed", "alliance_renewed",
        "betrayal", "alliance_ended", "nuke_struck", "resource_received_both",
        "resource_received_gold", "resource_received_troops", "resource_request_declined"
    ];
    const silent = [
        "attack_launched", "observer_feedback", "water_feedback", "transport_landed",
        "structure_started", "structure_ready", "structure_upgraded", "tile_upgraded",
        "wilderness_expanded", "enemy_territory_captured", "alliance_request_sent",
        "resource_request_sent", "resource_sent_both", "resource_sent_gold", "resource_sent_troops"
    ];
    const all = [...local, ...global, ...silent];
    assert.equal(local.length, 23);
    assert.equal(global.length, 15);
    assert.equal(silent.length, 15);
    assert.equal(new Set(all).size, 53);
    const emitters = [mapClick, simEventsSource, eliminationSource, snapshotFxSource, simUpdateSource].join("\n");
    for (const key of local) assert.ok(mapClick.includes(`hud.${key}`), `missing local feedback: ${key}`);
    for (const key of global) assert.ok(emitters.includes(`hud.${key}`), `missing global message: ${key}`);
    for (const key of silent) assert.ok(!emitters.includes(`hud.${key}`), `silent message still emitted: ${key}`);
});

test("HUD notifications coalesce, prioritize, reuse both entity avatars, and expire within fixed bounds", () => {
    const start = hud.indexOf("    function createNotificationCard(container, contextual) {");
    const end = hud.indexOf("\n    function renderHud(", start);
    assert.ok(start >= 0 && end > start);
    const avatarSlot = () => ({
        wrapper: { hidden: true, className: "" },
        image: {
            hidden: true,
            src: "",
            getAttribute(name) { return name === "src" ? this.src : null; },
            removeAttribute(name) { if (name === "src") this.src = ""; }
        },
        emblem: { hidden: true, textContent: "" }
    });
    const cards = Array.from({ length: 3 }, () => ({
        card: { hidden: true, className: "" },
        seal: { innerHTML: "" },
        avatars: [avatarSlot(), avatarSlot()],
        copy: { textContent: "" },
        renderKey: ""
    }));
    let now = 1000;
    const timers = [];
    const context = {
        hudRefs: { notifications: {} },
        notificationCursor: 0,
        notificationTimer: null,
        activeNotifications: [],
        notificationCards: cards,
        Date: { now: () => now },
        asset: path => "/assets/" + path,
        hudIcon: name => name,
        SOW_t: key => key,
        window: {
            SOW_LOCALE: "en",
            clearTimeout(timer) { timer.cleared = true; },
            setTimeout(callback, delay) {
                const timer = { callback, delay, cleared: false };
                timers.push(timer);
                return timer;
            }
        }
    };
    const renderer = hud.slice(start, end) + "\nthis.renderNotifications = renderNotifications;";
    vm.runInNewContext(renderer, context);
    const add = (id, key, priority, group, values, avatars, sumValues = false) => {
        now += 100;
        context.renderNotifications([{
            id, key, priority, group, values: values || {}, avatars: avatars || [],
            sum_values: sumValues, age_ms: 0
        }]);
    };

    add(1, "hud.resource_received_gold", 3, "resource:received:7", { gold: "100" }, [
        { kind: "portrait", slug: "boudica" }, { kind: "portrait", slug: "caesar" }
    ], true);
    const firstTimerCount = timers.length;
    context.renderNotifications([], false);
    assert.equal(timers.length, firstTimerCount, "unchanged HUD updates must not reset the expiry timer");
    add(2, "hud.resource_received_troops", 3, "resource:received:7", { troops: "25" }, [
        { kind: "portrait", slug: "boudica" }, { kind: "portrait", slug: "caesar" }
    ], true);
    assert.equal(context.activeNotifications.length, 1);
    assert.equal(context.activeNotifications[0].entry.key, "hud.resource_received_both");
    assert.equal(context.activeNotifications[0].entry.values.gold, "100");
    assert.equal(context.activeNotifications[0].entry.values.troops, "25");
    assert.deepEqual(cards[0].avatars.map(item => item.wrapper.hidden), [false, false]);
    assert.deepEqual(cards[0].avatars.map(item => item.image.src), [
        "/assets/gameplay/avatars/boudica.webp", "/assets/gameplay/avatars/caesar.webp"
    ]);
    assert.deepEqual(cards[0].avatars.map(item => item.emblem.hidden), [true, true]);
    assert.match(cards[0].avatars[1].wrapper.className, /--overlap/);

    const portraitPair = [
        { kind: "portrait", slug: "caesar" }, { kind: "portrait", slug: "boudica" }
    ];
    add(3, "hud.nuke_struck", 4, "nuke:9:7", {}, portraitPair);
    add(4, "hud.structure_ready", 2, "building:7", {});
    add(5, "hud.resource_request_declined", 1, "resource-rejected:7", {});
    assert.equal(context.activeNotifications.length, 3, "a burst must never create a backlog");
    assert.ok(context.activeNotifications.some(item => item.group === "nuke:9:7"));
    assert.ok(context.activeNotifications.some(item => item.group === "resource:received:7"));
    assert.ok(!context.activeNotifications.some(item => item.group === "resource-rejected:7"), "lower-priority feedback must yield to active event cards");
    const timersBeforeAttack = timers.length;
    add(6, "hud.attack_incoming", 4, "incoming-attack:7", { count: "2" }, portraitPair);
    add(7, "hud.attack_incoming", 4, "incoming-attack:7", { count: "3" }, portraitPair);
    assert.equal(context.activeNotifications.length, 3);
    assert.equal(context.activeNotifications.find(item => item.group === "incoming-attack:7").entry.values.count, "3");
    const timersBeforeDuplicate = timers.length;
    context.renderNotifications([{
        id: 7, key: "hud.attack_incoming", priority: 4, group: "incoming-attack:7",
        values: { count: "3" }, avatars: portraitPair, age_ms: 0
    }]);
    assert.equal(timers.length, timersBeforeDuplicate, "duplicate IDs must not refresh or duplicate cards");
    assert.ok(timersBeforeAttack < timersBeforeDuplicate);

    const burst = Array.from({ length: 100 }, (_, index) => ({
        id: index + 8, key: "hud.resource_request_declined", priority: 1, group: "resource-rejected:" + index,
        values: {}, avatars: [{ kind: "portrait", slug: "boudica" }], age_ms: 0
    }));
    const timersBeforeBurst = timers.length;
    context.renderNotifications(burst);
    assert.equal(context.activeNotifications.length, 3);
    assert.equal(timers.length, timersBeforeBurst, "a low-priority spam burst must not churn timers");

    now += 5000;
    timers.at(-1).callback();
    assert.equal(context.activeNotifications.length, 0);
    assert.ok(cards.every(parts => parts.card.hidden), "expired cards release their visible slots");

    add(108, "hud.attack_incoming", 4, "tribe-attack", {}, [
        { kind: "emblem", symbol: "🐺" }, { kind: "emblem", symbol: "🦅" }
    ]);
    const emblemCard = cards.find(parts => !parts.card.hidden);
    assert.deepEqual(emblemCard.avatars.map(item => item.emblem.textContent), ["🐺", "🦅"]);
    assert.deepEqual(emblemCard.avatars.map(item => item.image.hidden), [true, true]);
    assert.deepEqual(emblemCard.avatars.map(item => item.image.src), ["", ""]);
    assert.match(emblemCard.avatars[1].wrapper.className, /--overlap/);

    add(109, "hud.nuke_struck", 4, "single-avatar", {}, [
        null, { kind: "emblem", symbol: "🏛️" }
    ]);
    const singleAvatarCard = cards.find(parts => parts.copy.textContent === "hud.nuke_struck");
    assert.deepEqual(singleAvatarCard.avatars.map(item => item.wrapper.hidden), [true, false]);
    assert.doesNotMatch(singleAvatarCard.avatars[1].wrapper.className, /--overlap/);
    assert.equal(singleAvatarCard.avatars[0].emblem.textContent, "");

    add(110, "hud.resource_received_gold", 4, "missing-avatar", {}, [
        { kind: "fallback" }, { kind: "future-avatar-kind" }
    ]);
    const fallbackCard = cards.find(parts => parts.copy.textContent === "hud.resource_received_gold");
    assert.deepEqual(fallbackCard.avatars.map(item => item.image.src), [
        "/assets/gameplay/avatars/null.webp", "/assets/gameplay/avatars/null.webp"
    ]);
});

test("notification cards render typed avatars, recover failed art, and clear reused slots", () => {
    const start = hud.indexOf("    function createNotificationCard(container, contextual) {");
    const end = hud.indexOf("\n    function renderHud(", start);
    assert.ok(start >= 0 && end > start);
    class Element {
        constructor(tagName) {
            this.tagName = tagName;
            this.children = [];
            this.attributes = Object.create(null);
            this.hidden = false;
            this.className = "";
            this.src = "";
        }
        append(...items) { this.children.push(...items); }
        appendChild(item) { this.children.push(item); }
        setAttribute(name, value) { this.attributes[name] = String(value); }
        getAttribute(name) {
            if (name === "src") return this.src || null;
            return this.attributes[name] || null;
        }
        removeAttribute(name) {
            delete this.attributes[name];
            if (name === "src") this.src = "";
        }
    }
    const container = new Element("container");
    const context = {
        document: { createElement: tagName => new Element(tagName) },
        asset: path => "/assets/" + path,
        hudIcon: name => name,
        SOW_t: key => key
    };
    const renderer = hud.slice(start, end)
        + "\nthis.createNotificationCard = createNotificationCard;"
        + "\nthis.renderNotificationCard = renderNotificationCard;"
        + "\nthis.clearNotificationCard = clearNotificationCard;";
    vm.runInNewContext(renderer, context);

    const parts = context.createNotificationCard(container, false);
    context.renderNotificationCard(parts, {
        entry: {
            id: 1, key: "hud.attack_incoming", values: {},
            avatars: [
                { kind: "portrait", slug: "boudica" },
                { kind: "emblem", symbol: "🐺" }
            ]
        },
        priority: 4
    }, false);
    assert.equal(container.children.length, 1);
    assert.equal(parts.avatars[0].image.src, "/assets/gameplay/avatars/boudica.webp");
    assert.equal(parts.avatars[0].emblem.hidden, true);
    assert.equal(parts.avatars[1].image.hidden, true);
    assert.equal(parts.avatars[1].emblem.textContent, "🐺");
    assert.match(parts.avatars[1].wrapper.className, /--overlap/);

    parts.avatars[0].image.onerror.call(parts.avatars[0].image);
    assert.equal(parts.avatars[0].image.src, "/assets/gameplay/avatars/null.webp");

    context.renderNotificationCard(parts, {
        entry: { id: 2, key: "hud.alliance_formed", values: {}, avatars: [{ kind: "emblem", symbol: "🏛️" }] },
        priority: 2
    }, false);
    assert.equal(parts.avatars[0].image.src, "");
    assert.equal(parts.avatars[0].emblem.textContent, "🏛️");
    assert.equal(parts.avatars[1].wrapper.hidden, true);
    assert.equal(parts.avatars[1].emblem.textContent, "");

    context.renderNotificationCard(parts, {
        entry: { id: 3, key: "hud.nuke_struck", values: {}, avatars: [null, { kind: "emblem", symbol: "🏺" }] },
        priority: 4
    }, false);
    assert.equal(parts.avatars[0].wrapper.hidden, true);
    assert.equal(parts.avatars[1].emblem.textContent, "🏺");
    assert.doesNotMatch(parts.avatars[1].wrapper.className, /--overlap/);

    context.clearNotificationCard(parts);
    assert.equal(parts.avatars[0].wrapper.hidden, true);
    assert.equal(parts.avatars[1].wrapper.hidden, true);
    assert.equal(parts.avatars[1].emblem.textContent, "");
});

test("map feedback reuses one compact card, clamps to the viewport, and releases message data", () => {
    const start = hud.indexOf("    function createNotificationCard(container, contextual) {");
    const end = hud.indexOf("\n    function renderHud(", start);
    const durationMatch = hud.match(/var MAP_FEEDBACK_DURATION_MS = (\d+);/);
    const fadeOutMatch = hud.match(/var MAP_FEEDBACK_FADE_OUT_MS = (\d+);/);
    assert.ok(durationMatch, "map feedback duration is declared");
    assert.ok(fadeOutMatch, "map feedback fade-out duration is declared");
    const durationMs = Number(durationMatch[1]);
    const fadeOutMs = Number(fadeOutMatch[1]);
    assert.equal(durationMs, 2000, "map feedback stays brief");
    assert.equal(fadeOutMs, 180, "map feedback fades out gently");
    assert.match(hudCss, /\.sow-hud__notification--contextual\s*\{[^}]*opacity: 0;[^}]*transition: opacity 180ms ease, transform 180ms cubic-bezier/);
    assert.match(hudCss, /\.sow-hud__notification--contextual\.sow-hud__map-feedback-card--visible\s*\{[^}]*opacity: 1;/);
    assert.ok(start >= 0 && end > start);
    let showTransitions = 0;
    const classes = new Set();
    const card = {
        hidden: true,
        className: "",
        classList: {
            add(name) {
                classes.add(name);
                if (name === "sow-hud__map-feedback-card--visible") showTransitions += 1;
            },
            remove(name) { classes.delete(name); },
            contains(name) { return classes.has(name); }
        },
        get offsetWidth() { return 220; },
        getBoundingClientRect() { return { width: 220, height: 46 }; }
    };
    const parts = {
        card,
        seal: { innerHTML: "" },
        avatars: Array.from({ length: 2 }, () => ({
            wrapper: { hidden: true, className: "" },
            image: {
                hidden: true,
                src: "",
                getAttribute(name) { return name === "src" ? this.src : null; },
                removeAttribute(name) { if (name === "src") this.src = ""; }
            },
            emblem: { hidden: true, textContent: "" }
        })),
        copy: { textContent: "" },
        renderKey: ""
    };
    const style = {
        left: "",
        top: "",
        removeProperty(name) { this[name] = ""; }
    };
    const container = { hidden: true, style };
    let now = 1000;
    const timers = [];
    const context = {
        hudRefs: { mapFeedback: container },
        mapFeedbackCursor: 0,
        mapFeedbackTimer: null,
        activeMapFeedback: null,
        mapFeedbackCard: parts,
        MAP_FEEDBACK_DURATION_MS: durationMs,
        MAP_FEEDBACK_FADE_OUT_MS: fadeOutMs,
        Date: { now: () => now },
        asset: path => "/assets/" + path,
        hudIcon: name => name,
        SOW_t: key => key,
        window: {
            innerWidth: 375,
            innerHeight: 667,
            clearTimeout(timer) { timer.cleared = true; },
            setTimeout(callback, delay) {
                const timer = { callback, delay, cleared: false };
                timers.push(timer);
                return timer;
            }
        }
    };
    const renderer = hud.slice(start, end) + "\nthis.renderMapFeedback = renderMapFeedback;";
    vm.runInNewContext(renderer, context);

    context.renderMapFeedback({ id: 1, key: "hud.build_no_space", values: {}, x: 0, y: 0, age_ms: 0 });
    assert.equal(container.hidden, false);
    assert.equal(parts.copy.textContent, "hud.build_no_space");
    assert.ok(card.classList.contains("sow-hud__map-feedback-card--visible"), "the card fades in");
    assert.ok(Number.parseFloat(style.left) >= 118, "card stays inside the left edge");
    assert.equal(style.top, "72px", "card clears the HUD top bar");
    const timerCount = timers.length;
    context.renderMapFeedback({ id: 1, key: "hud.build_no_space", values: {}, x: 200, y: 300, age_ms: 0 });
    assert.equal(timers.length, timerCount, "the same event is not rendered twice");

    context.renderMapFeedback({ id: 2, key: "hud.build_no_space", values: {}, x: 200, y: 300, age_ms: 0 });
    assert.equal(timers.length, timerCount + 1, "a repeated click restarts the feedback timer");
    assert.ok(timers.at(-2).cleared, "repeated feedback releases its previous timer");
    assert.equal(style.left, "200px", "repeated feedback follows the latest click");
    assert.equal(style.top, "240px");
    assert.equal(timers.at(-1).delay, 2000);
    assert.equal(showTransitions, 2, "repeated feedback restarts its entrance");

    context.renderMapFeedback({ id: 3, key: "hud.build_land", values: {}, x: 375, y: 650, age_ms: 0 });
    assert.equal(parts.copy.textContent, "hud.build_land");
    assert.ok(timers.at(-2).cleared, "replaced feedback releases its previous timer");
    assert.ok(Number.parseFloat(style.left) <= 257, "card stays inside the right edge");
    now += 2000;
    timers.at(-1).callback();
    assert.equal(container.hidden, false, "the card remains during its fade-out");
    assert.equal(parts.copy.textContent, "hud.build_land");
    assert.equal(card.classList.contains("sow-hud__map-feedback-card--visible"), false);
    assert.equal(timers.at(-1).delay, fadeOutMs);
    now += fadeOutMs;
    timers.at(-1).callback();
    assert.equal(container.hidden, true);
    assert.equal(parts.copy.textContent, "");
    assert.ok(parts.avatars.every(avatar => avatar.wrapper.hidden && avatar.image.src === ""));

    context.renderMapFeedback({ id: 4, key: "hud.build_land", values: {}, x: 375, y: 650, age_ms: 2000 });
    assert.equal(container.hidden, true, "expired feedback is not displayed");
});

test("campaign editor refresh serves current source files without browser caching", () => {
    assert.match(campaignEditorServer, /pathname === "\/tools\/campaign-editor\/"[\s\S]*?path\.join\(editorDir, "index\.html"\)/);
    assert.match(campaignEditorServer, /pathname\.startsWith\("\/tools\/campaign-editor\/"\)\) return safeFile\(editorDir/);
    assert.match(campaignEditorServer, /const body = await fs\.readFile\(file\)/);
    assert.match(campaignEditorServer, /"Cache-Control": "no-store"/);
});

test("campaign saves use the current validator and call Save by its real action", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    const roster = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.json"), "utf8"));
    const zoomOut = definition.steps.find(step => step.id === "boudica_zoom_out");
    assert.equal(zoomOut.pause_game, true);
    const report = campaign.validate(definition, roster, { allowMissingFactionReferences: true, hasText: () => true, hasAvatar: () => true });
    assert.deepEqual(report.errors.filter(issue => issue.step === zoomOut.id), []);
    assert.match(campaignEditor, /Allow camera controls only while paused/);
    assert.match(campaignEditor, /state\.facts\.touch_controls = \$\("#device"\)\.value === "mobile" \? 1 : 0/);
    const unpausedCameraOnly = JSON.parse(JSON.stringify(definition));
    unpausedCameraOnly.steps.find(step => step.id === zoomOut.id).pause_game = false;
    assert.ok(campaign.validate(unpausedCameraOnly, roster, { allowMissingFactionReferences: true, hasText: () => true, hasAvatar: () => true }).errors.some(issue => issue.step === zoomOut.id && issue.field === "camera_only"));

    const pausedScene = JSON.parse(JSON.stringify(definition));
    pausedScene.steps.find(step => step.id === zoomOut.id).type = "scene";
    const sceneReport = campaign.validate(pausedScene, roster, { allowMissingFactionReferences: true, hasText: () => true, hasAvatar: () => true });
    assert.equal(sceneReport.errors.some(issue => issue.step === zoomOut.id && issue.field === "fields" && issue.message.includes("pause_game")), false,
        "pause_game is accepted on every step type where the runtime reads it");

    assert.match(campaignEditorServer, /function loadCampaignRuntime\(\) \{\s*const modulePath = campaignRequire\.resolve\(campaignFile\);\s*delete campaignRequire\.cache\[modulePath\];\s*return campaignRequire\(modulePath\);\s*\}/);
    assert.match(campaignEditorServer, /loadCampaignRuntime\(\)\.validate\(definition, roster,/);
    assert.match(campaignEditorServer, /renameFactionText\([\s\S]*?validatePair\(episodeId, value, definition\)/);
    assert.match(campaignMapEditorHtml, /<h2>Save<\/h2>[\s\S]*?<button class="primary" id="exportBtn">Save<\/button>/);
    assert.match(campaignMapEditorHtml, /<button id="reloadBtn">Reload saved<\/button>/);
    assert.match(campaignMapEditorHtml, /faction_renames/);
    assert.match(campaignMapEditorHtml, /async function saveRoster\(\)/);
    assert.match(campaignMapEditorHtml, /Save failed: /);
    assert.match(campaignEditorHtml, /<button id="reloadBtn">Reload saved<\/button><button id="exportBtn" class="primary">Save<\/button>/);
});

test("campaign selector removes the multiplayer filler from every locale", () => {
    const strings = path.join(shell, "../../sow-i18n/strings");
    const files = fs.readdirSync(strings).map(locale => path.join(strings, locale, "web.toml")).filter(file => fs.existsSync(file));
    assert.ok(files.length > 0);
    for (const file of files) assert.doesNotMatch(fs.readFileSync(file, "utf8"), /^campaign_tagline\s*=/m, file);
});

test("campaign studio preview loads the game's shared typography", () => {
    assert.match(campaignMapEditorHtml, /<link rel="stylesheet" href="\/fonts\/fonts\.css"\s*\/>/);
    assert.match(campaignEditorHtml, /<link rel="stylesheet" href="\/fonts\/fonts\.css">/);
    assert.match(campaignEditorServer, /pathname\.startsWith\("\/fonts\/"\).*sow-web\/site\/fonts/s);
    assert.match(campaignEditorServer, /"\.woff2": "font\/woff2"/);
    assert.match(campaignEditorServer, /"\.ttc": "font\/collection"/);
    assert.match(storyCss, /font-family: var\(--sow-ui-font/);
    assert.match(campaignView, /root\.dataset\.localeScript = context\.localeScript \|\| doc\.documentElement\.dataset\.localeScript/);
    assert.match(campaignEditor, /localeScript: previewLocaleScript\(state\.previewLanguage\)/);
    assert.match(localeRuntime, /function localeScript\(locale\) \{[\s\S]*?if \(code === "ar"\) return "arabic";[\s\S]*?if \(code === "zh-cn" \|\| code === "ja" \|\| code === "ko"\) return "cjk";[\s\S]*?if \(code === "ru"\) return "cyrillic";/);
    assert.match(campaignEditor, /function previewLocaleScript\(locale\)[\s\S]*?code === "ar"[\s\S]*?\["zh-cn", "ja", "ko"\][\s\S]*?code === "ru"/);
    for (const script of ["arabic", "cjk", "cyrillic"]) assert.match(gameFonts, new RegExp("\\.sow-story\\[data-locale-script='" + script + "'\\]"));
});

test("campaign editor never treats a missing catalog entry as valid text", () => {
    assert.match(campaignEditorServer, /catch \{\s*return false;\s*\}/);
});

test("campaign interpreter facts are all published by the WASM HUD payload", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const facts = new Set([...Object.values(campaign.METRICS).filter(fact => fact !== campaign.METRICS.hover), "tiles", "defeated", "defeated_faction_ids", "contacts", "contact_faction_ids", "attacks_by_faction_id", "fleets_by_type", "transport_fleets_by_faction_id", "support_deliveries_by_faction_id", "resource_transfers_by_recipient_faction_id", "alliance_faction_ids"]);
    for (const fact of facts) assert.match(webMenu, new RegExp('"' + fact + '"\\s*:'), `WASM HUD is missing campaign fact ${fact}`);
    assert.match(tutorial, /hover_events: runtime\.hoverEvents/);
    assert.match(tutorial, /runtime\.machine\.update\(facts, runtime\.uiCounts\)/);
});

test("Campaign Studio avoids rebuilding the route graph on progress-only updates", () => {
    const start = campaignEditor.indexOf("function paintPreview(updateMachine) {");
    const end = campaignEditor.indexOf("function previewLocaleScript", start);
    assert.ok(start >= 0 && end > start);
    const paint = campaignEditor.slice(start, end);
    assert.match(paint, /if \(previousStep !== model\.step\.id\) renderGraph\(\)/);
    assert.equal((paint.match(/renderGraph\(\)/g) || []).length, 1);
    assert.match(campaignEditor, /function markDirty\(\)[\s\S]*?refresh\(true\); syncPreview\(\); renderGraph\(\)/);
});

test("campaign dialogue starts each new beat at the top of its scroll panel", () => {
    assert.match(campaignView, /if \(renderKey !== key\) \{\s*renderKey = key;\s*conversation\.scrollTop = 0;\s*scrollContent\.scrollTop = 0;\s*actions\.scrollTop = 0;/);
    assert.match(storyCss, /\.sow-story__scroll\s*\{[^}]*overflow-y: auto/);
});

test("campaign editor preview and game share RTL locale coverage", () => {
    const localeSet = /new Set\(\["ar", "arc", "ckb", "dv", "fa", "he", "iw", "nqo", "pnb", "ps", "sd", "syr", "ug", "ur", "yi"\]\)/;
    assert.match(localeRuntime, localeSet);
    assert.match(campaignEditor, localeSet);
    assert.match(campaignEditor, /direction: rtlLanguages\.has\(state\.previewLanguage\.toLowerCase\(\)\.split\("-"\)\[0\]\) \? "rtl" : "ltr"/);
});

test("an active campaign keeps its loaded definition when the editor draft changes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "snapshot_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.opening": "Original" } }, speakers: {}, entry: "opening",
        steps: [{ id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" }, { id: "ending", type: "end", title_key: "tutorial.ending" }]
    };
    const activeRun = campaign.create(definition);
    definition.strings.en["tutorial.opening"] = "Edited draft";
    definition.steps[0].next = "missing_step";
    assert.equal(activeRun.definition.strings.en["tutorial.opening"], "Original");
    assert.equal(activeRun.view().step.next, "ending");
});

test("campaign episode JSON is cached only while paired files are loading", () => {
    assert.match(tutorial, /var request = Promise\.all\(\[/);
    assert.match(tutorial, /runtime\.loading\[episodeId\] = request;\s*function releaseRequest\(\) \{\s*if \(runtime\.loading\[episodeId\] === request\) delete runtime\.loading\[episodeId\];\s*}\s*request\.then\(releaseRequest, releaseRequest\)/);
    assert.match(tutorial, /runtime\.modalOpen = false;\s*runtime\.resumeAfterModal = false;/);
});

test("campaign start keeps its loaded script and replay clicks belong to that episode", async () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "boudica", default_locale: "en",
        settings: { buildings_enabled: false, starting_troops: 1000 }, speakers: {},
        strings: { en: { "tutorial.open": "Original", "tutorial.end": "End" } }, entry: "opening",
        steps: [{ id: "opening", type: "scene", title_key: "tutorial.open", next: "ending" }, { id: "ending", type: "end", title_key: "tutorial.end" }]
    };
    const roster = { map: "eastanglia", player_spawn: [1, 1], factions: [{ id: "enemy", name: "Enemy", x: 2, y: 2, starting_troops: 500, relation: "enemy", civ: "Roman Empire", leader: "Caesar" }] };
    const clone = value => JSON.parse(JSON.stringify(value));
    const buttons = ["boudica", "six_sky_ep2"].map(episodeId => ({
        dataset: { episodeId }, disabled: false, getClientRects: () => [{}], matches: () => false,
        contains(target) { return target === this; }
    }));
    const listeners = {}, storyRoot = { hidden: true, isConnected: false, contains: () => false };
    const document = {
        body: { appendChild(node) { node.isConnected = true; } }, documentElement: { dir: "ltr" },
        createElement: () => storyRoot, getElementById: () => null,
        querySelector: () => null,
        querySelectorAll: selector => selector === campaign.UI_TARGETS.campaign_replay ? buttons : [],
        addEventListener(type, callback) { listeners[type] = callback; }
    };
    let fetches = 0, activeRun, lastUi;
    const window = {
        addEventListener() {}, SOW_menu_command() {},
        SOWCampaign: { ...campaign, create(data) {
            activeRun = campaign.create(data);
            const update = activeRun.update;
            activeRun.update = (facts, ui, now) => { lastUi = { ...ui }; return update(facts, ui, now); };
            return activeRun;
        } },
        SOWCampaignView: { mount: () => ({ render() {}, destroy() {} }) }
    };
    vm.runInNewContext(tutorial, { window, document, performance: { now: () => 0 }, console,
        fetch(url) { fetches++; return Promise.resolve({ ok: true, json: () => Promise.resolve(clone(url.endsWith(".triggers.json") ? definition : roster)) }); }
    });
    window.SOW_startCampaignEpisode("boudica");
    await new Promise(setImmediate);
    definition.strings.en["tutorial.open"] = "Edited while the match was starting";
    const state = { phase: "Playing", hud: { tutorial: { active: true, episode_id: "boudica", facts: {} }, players: [], settings: {} } };
    window.SOW_tutorial_state_update(state);
    await new Promise(setImmediate);
    assert.equal(fetches, 2);
    assert.equal(activeRun.definition.strings.en["tutorial.open"], "Original");
    listeners.click({ type: "click", target: buttons[1] });
    window.SOW_tutorial_state_update(state);
    assert.equal(lastUi.campaign_replay, undefined);
    listeners.click({ type: "click", target: buttons[0] });
    window.SOW_tutorial_state_update(state);
    assert.equal(lastUi.campaign_replay, 1);
});

test("campaign decisions keep their selected branch and guard stale callbacks", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "branch_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "decision" },
            { id: "decision", type: "choice", title_key: "tutorial.decision", choices: [
                { id: "gather", label_key: "tutorial.gather", next: "gather_step" },
                { id: "strike", label_key: "tutorial.strike", next: "strike_step" }
            ] },
            { id: "gather_step", type: "scene", title_key: "tutorial.gather_step", next: "rejoin" },
            { id: "strike_step", type: "scene", title_key: "tutorial.strike_step", next: "rejoin" },
            { id: "rejoin", type: "scene", title_key: "tutorial.rejoin", next: "ending", routes: [{ when: { choice: "decision", equals: "strike" }, next: "strike_echo" }] },
            { id: "strike_echo", type: "scene", title_key: "tutorial.strike_echo", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.equal(machine.advance(null, "opening"), true);
    assert.equal(machine.advance("strike", "decision"), true);
    assert.equal(machine.view().step.id, "strike_step");
    assert.equal(machine.advance(null, "decision"), false);
    assert.equal(machine.state.choices.decision, "strike");
    assert.equal(machine.advance(null, "strike_step"), true);
    assert.equal(machine.advance(null, "rejoin"), true);
    assert.equal(machine.view().step.id, "strike_echo");
    assert.match(campaignEngine, /expectedStepId/);
});

test("campaign validator rejects conditional branches that can loop forever", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const report = campaign.validate({
        version: 2, episode_id: "route_loop_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "decision", steps: [
            { id: "decision", type: "choice", title_key: "tutorial.decision", choices: [
                { id: "repeat", label_key: "tutorial.repeat", next: "loop_scene" },
                { id: "finish", label_key: "tutorial.finish", next: "ending" }
            ] },
            { id: "loop_scene", type: "scene", title_key: "tutorial.loop_scene", next: "ending", routes: [
                { when: { fact: "tiles_gained", gte: 1 }, next: "loop_scene" }
            ] },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(report.errors.some(issue => issue.step === "loop_scene" && issue.message === "This path cannot reach an ending."));
    const legacy = campaign.validate({
        version: 2, episode_id: "legacy_flag_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending", routes: [{ when: { flag: "old_flag", equals: true }, next: "ending" }] },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(legacy.errors.some(issue => issue.step === "opening" && issue.field === "routes"));
});

test("campaign validator warns when route order makes a later threshold unreachable", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const report = campaign.validate({
        version: 2, episode_id: "route_order_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending", routes: [
                { when: { fact: "tiles_gained", gte: 1 }, next: "ending" },
                { when: { fact: "tiles_gained", gte: 10 }, next: "ending" }
            ] },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(report.warnings.some(issue => issue.step === "opening" && issue.message.includes("Route 2 is unreachable")));
});

test("campaign validator rejects ignored fields and dialog keeps its speaker row", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const report = campaign.validate({
        version: 2, episode_id: "unsupported_field_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "opening", actions: [{ type: "spawn" }], steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending", actions: [{ type: "grant_troops" }] },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(report.errors.some(issue => issue.field === "campaign" && issue.message.includes("actions")));
    assert.ok(report.errors.some(issue => issue.step === "opening" && issue.field === "fields" && issue.message.includes("actions")));
    const unusedSpeaker = campaign.validate({
        version: 2, episode_id: "step_field_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: { narrator: { name: "Narrator" } }, entry: "objective", steps: [
            { id: "objective", type: "objective", title_key: "tutorial.objective", speaker: "narrator", trigger: { type: "territory", scope: "step", value: 1 }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.deepEqual(unusedSpeaker.errors, []);
    assert.match(campaignView, /sow-story__speaker/);
    assert.doesNotMatch(campaignView, /sow-story__objective-speaker/);
});

test("campaign preview can seed prior decisions and facts to inspect conditional scenes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "preview_branch_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "decision", steps: [
            { id: "decision", type: "choice", title_key: "tutorial.decision", choices: [
                { id: "gather", label_key: "tutorial.gather", next: "rejoin" },
                { id: "strike", label_key: "tutorial.strike", next: "rejoin" }
            ] },
            { id: "rejoin", type: "scene", title_key: "tutorial.rejoin", next: "ending", routes: [
                { when: { choice: "decision", equals: "strike" }, next: "strike_echo" },
                { when: { fact: "tiles_gained", gte: 3 }, next: "growth_echo" }
            ] },
            { id: "strike_echo", type: "scene", title_key: "tutorial.strike_echo", next: "ending" },
            { id: "growth_echo", type: "scene", title_key: "tutorial.growth_echo", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    machine.jump("rejoin", {}, { decision: "strike", unknown_decision: "strike", decision_bad_answer: "invalid" });
    assert.equal(machine.advance(null, "rejoin"), true);
    assert.equal(machine.view().step.id, "strike_echo");
    assert.deepEqual({ ...machine.view().state.choices }, { decision: "strike" });
    const factPreview = campaign.create(machine.definition);
    factPreview.jump("rejoin", { tiles_gained: 3 });
    assert.equal(factPreview.advance(null, "rejoin"), true);
    assert.equal(factPreview.view().step.id, "growth_echo");
    assert.match(campaignEditorHtml, /id="previewBranchSelect"/);
    assert.match(campaignEditor, /condition\.fact && Number\.isFinite\(condition\.gte\)/);
    assert.match(campaignEditor, /state\.machine\.jump\(start, context\.facts, context\.choices\)/);
});

test("live campaign edits preserve prior decisions and conditional story branches", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "live_edit_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "decision" },
            { id: "decision", type: "choice", title_key: "tutorial.decision", choices: [
                { id: "gather", label_key: "tutorial.gather", next: "gather_step" },
                { id: "strike", label_key: "tutorial.strike", next: "strike_step" }
            ] },
            { id: "gather_step", type: "scene", title_key: "tutorial.gather_step", next: "rejoin" },
            { id: "strike_step", type: "scene", title_key: "tutorial.strike_step", next: "rejoin" },
            { id: "rejoin", type: "scene", title_key: "tutorial.rejoin", next: "ending", routes: [{ when: { choice: "decision", equals: "strike" }, next: "strike_echo" }] },
            { id: "strike_echo", type: "scene", title_key: "tutorial.strike_echo", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    };
    const machine = campaign.create(definition);
    machine.advance(null, "opening");
    machine.advance("strike", "decision");
    machine.advance(null, "strike_step");
    const edited = JSON.parse(JSON.stringify(definition));
    edited.steps.find(step => step.id === "rejoin").title_key = "tutorial.rejoin_edited";
    assert.equal(machine.replaceDefinition(edited), true);
    assert.equal(machine.view().state.choices.decision, "strike");
    assert.equal(machine.view().step.title_key, "tutorial.rejoin_edited");
    machine.advance(null, "rejoin");
    assert.equal(machine.view().step.id, "strike_echo");
});

test("campaign validator rejects speakers without a display name", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const report = campaign.validate({
        version: 2, episode_id: "speaker_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.opening": "Opening", "tutorial.ending": "The end" } },
        speakers: { empty: { name: "  " } }, entry: "opening", steps: [
            { id: "opening", type: "scene", speaker: "empty", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(report.errors.some(issue => issue.field === "speakers" && /empty/.test(issue.message)));
});

test("faction deletion is blocked while stable story references remain", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "roster_edit", default_locale: "en",
        settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {
            "tutorial.open": "Opening", "tutorial.contact": "Contact the tribe", "tutorial.end": "The end",
            "tutorial.response_title": "An ally answers", "tutorial.response_body": "We stand together."
        } },
        speakers: { removed_ally: { faction: "removed_tribe" } }, entry: "opening",
        steps: [
            { id: "opening", type: "scene", speaker: "removed_ally", title_key: "tutorial.open", next: "contact" },
            { id: "contact", type: "objective", title_key: "tutorial.contact", trigger: { type: "contact", targets: ["existing_tribe", "removed_tribe"], value: 2, scope: "episode" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.end" }
        ],
        reactions: [{ id: "removed_ally_contact", when: { type: "contact", target: "removed_tribe" }, speaker: "removed_ally", title_key: "tutorial.response_title", body_key: "tutorial.response_body" }]
    };
    const roster = { map: "eastanglia", player_spawn: [10, 10], factions: [{ id: "existing_tribe", name: "Existing Tribe", x: 20, y: 20, starting_troops: 500, relation: "neutral", civ: "Iceni Kingdom", leader: "Boudica" }] };

    assert.deepEqual(campaign.validate(definition, roster, { allowMissingFactionReferences: true }).errors, []);
    assert.ok(campaign.validate(definition, roster).errors.length > 0);
    assert.ok(campaign.factionReferenceIds(definition).has("removed_tribe"));
    assert.ok(campaign.factionReferenceIds(definition).has("existing_tribe"));
});

test("campaign editor rejects maps and spawns that the game would refuse", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "boudica", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.open": "Open", "tutorial.end": "End" } }, speakers: {}, entry: "opening",
        steps: [{ id: "opening", type: "scene", title_key: "tutorial.open", next: "ending" }, { id: "ending", type: "end", title_key: "tutorial.end" }]
    };
    const roster = { map: "eastanglia", player_spawn: [895, 503], factions: [{ id: "enemy", name: "Enemy", x: 1, y: 1, starting_troops: 500, relation: "enemy", civ: "Roman Empire", leader: "Caesar" }] };
    const errors = () => campaign.validate(definition, roster).errors;
    assert.deepEqual(errors(), []);
    roster.map = "northamerica";
    assert.ok(errors().some(issue => issue.field === "roster.map"));
    roster.map = "eastanglia"; roster.player_spawn[0] = 896;
    assert.ok(errors().some(issue => issue.field === "roster.player_spawn"));
    roster.player_spawn[0] = 895; roster.factions[0].y = 504;
    assert.ok(errors().some(issue => /Faction spawn is outside/.test(issue.message)));
    definition.episode_id = "six_sky_ep1"; roster.map = "northamerica";
    roster.player_spawn = [999, 515]; roster.factions[0].y = 515;
    assert.deepEqual(errors(), []);
});

test("campaign speaker picker is sourced from existing avatar assets and rejects missing files", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.match(campaignEditorServer, /url\.pathname === "\/__avatars"/);
    assert.match(campaignEditor, /fetch\("\/__avatars"/);
    assert.match(campaignEditor, /selectField\("Portrait"/);
    assert.match(campaignEditor, /hasAvatar: function \(avatar\) \{ return state\.avatars\.includes\(avatar\); \}/);
    const definition = {
        version: 2, episode_id: "avatar_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.opening": "Opening", "tutorial.ending": "End" } }, speakers: { leader: { name: "Leader", avatar: "not_real" } }, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    };
    assert.ok(campaign.validate(definition, null, { hasAvatar: avatar => avatar === "boudica" }).errors.some(issue => issue.field === "speakers"));
});

test("campaign studio edits speaker names per language when localized", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.match(campaignEditor, /var keyBase = "tutorial\.speaker_" \+ speakerId \+ "_name"/);
    assert.match(campaignEditor, /while \(Object\.keys\(state\.definition\.strings\)/);
    assert.match(campaignEditor, /Character name · " \+ state\.language/);
    assert.match(campaignEditor, /dictionary\[speaker\.name_key\] = value/);
    assert.match(campaignEditor, /Use one name for every language/);
    assert.match(campaignEditor, /var locale = this\.value; state\.language = locale; renderSettings\(\)/);
    assert.match(campaignEditor, /loadCatalog\(locale\)\.then\(function \(\) \{[\s\S]*?state\.language === locale/);
    assert.match(campaignView, /character\.name_key \? t\(character\.name_key\)/);
    const report = campaign.validate({
        version: 2, episode_id: "localized_speaker", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.speaker_leader_name": "Leader", "tutorial.opening": "Opening", "tutorial.ending": "End" }, es: { "tutorial.speaker_leader_name": "Líder" } },
        speakers: { leader: { name_key: "tutorial.speaker_leader_name" } }, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.deepEqual(report.errors, []);
});

test("campaign studio identifies episode text falling back to the default language", () => {
    assert.match(campaignEditor, /var hasLocaleText = Boolean\(dictionary\[key\] \|\| catalogValue\(state\.language, key\)\)/);
    assert.match(campaignEditor, /var hasDefaultText = Boolean\(textValue\(key, state\.definition\.default_locale\)\)/);
    assert.match(campaignEditor, /Showing episode default-language text\. Editing adds this language\./);
    assert.match(campaignEditor, /fallbackNote\.hidden = state\.language === state\.definition\.default_locale \|\| Boolean\(dictionary\[key\] \|\| catalogValue\(state\.language, key\)\) \|\| !hasDefaultText/);
});

test("campaign studio loads locale catalogs on demand and keeps episode text editable on failure", () => {
    assert.match(campaignEditor, /localeFailures: \[\], localeRegistryFailed: false/);
    assert.match(campaignEditor, /function loadCatalog\(locale\)/);
    assert.match(campaignEditor, /if \(catalogLoads\[code\]\) return catalogLoads\[code\]/);
    assert.match(campaignEditor, /fetch\("\/locales\/" \+ encodeURIComponent\(code\)/);
    assert.match(campaignEditor, /if \(!state\.localeFailures\.includes\(code\)\) state\.localeFailures\.push\(code\)/);
    assert.match(campaignEditor, /Promise\.all\(\[loadCatalog\(state\.language\), loadCatalog\(state\.previewLanguage\)\]\)/);
    assert.match(campaignEditor, /Catalog unavailable for .*episode text remains editable/);
    assert.match(campaignEditor, /var version = Number\(registry && registry\.version\)/);
    assert.match(campaignEditor, /registry\.schema !== 1[\s\S]*?Array\.isArray\(registry\.languages\)/);
    assert.match(campaignEditor, /locales = \["en", "es"\]; state\.localeRegistryFailed = true/);
});

test("campaign studio generates distinct fallback text keys for decisions and dialogue lines", () => {
    assert.match(campaignEditor, /parts\[0\] === "choices" && step\.choices\[index\]/);
    assert.match(campaignEditor, /step\.choices\[index\]\.id \+ \(keyPath === "label_key"/);
    assert.match(campaignEditor, /keyPath === "label_key" \? "_label" : "_detail"/);
    assert.match(campaignEditor, /parts\[0\] === "lines" && step\.lines\[index\]/);
    assert.match(campaignEditor, /"_line_" \+ \(index \+ 1\) \+ "_body"/);
});

test("campaign editor and runtime normalize regional locale codes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.match(campaignEditor, /entry\.code\)\.toLowerCase\(\)/);
    assert.match(tutorial, /toLowerCase\(\)\.replace\(\/_\/g, "-"\)/);
    const report = campaign.validate({
        version: 2, episode_id: "regional_locale_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.opening": "Opening", "tutorial.ending": "End" }, "pt-br": {}, "zh-cn": {} },
        speakers: {}, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.deepEqual(report.errors, []);
});

test("campaign graph Fit centers every node using its actual dimensions", () => {
    assert.match(campaignEditor, /function fitGraph\(\)[\s\S]*?node\.offsetWidth[\s\S]*?node\.offsetHeight[\s\S]*?state\.pan = \{ x: \(stage\.clientWidth - width \* zoom\) \/ 2 - bounds\.left \* zoom/);
    assert.match(campaignEditor, /button\.dataset\.zoom === "fit"\) fitGraph\(\)/);
    assert.doesNotMatch(campaignEditor, /button\.dataset\.zoom === "fit"\) \{ state\.zoom = 1; state\.pan = \{ x: 36, y: 44 \}/);
});

test("campaign graph Arrange lays out both entries and follows every branch", () => {
    assert.match(campaignEditorHtml, /id="arrangeGraphBtn"/);
    assert.match(campaignEditor, /\[state\.definition\.entry, state\.definition\.menu_guide && state\.definition\.menu_guide\.entry\]/);
    assert.match(campaignEditor, /portDefinitions\(source\)\.forEach/);
    assert.match(campaignEditor, /state\.definition\.layout\[step\.id\] = \{ x: 48 \+ x \* 320, y: y \}/);
    assert.match(campaignEditor, /markDirty\(\); fitGraph\(\)/);
});

test("campaign studio episode picker derives options from paired campaign files", () => {
    assert.match(campaignEditorServer, /url\.pathname === "\/__episodes"/);
    assert.match(campaignEditorServer, /name\.endsWith\("\.triggers\.json"\)/);
    assert.match(campaignEditorServer, /files\.has\(id \+ "\.json"\)/);
    assert.match(campaignEditor, /fetch\("\/__episodes", \{ cache: "no-store" \}\)/);
    assert.match(campaignEditor, /fillEpisodes\(\)/);
    assert.match(campaignEditorHtml, /id="episodeSelect"/);
    assert.match(campaignMapEditorHtml, /id="episodeSelect"/);
    assert.match(campaignMapEditorHtml, /fetch\("\/__episodes"/);
    assert.match(campaignMapEditorHtml, /Discard unsaved map changes and switch episode/);
});

test("campaign studio language selectors use readable names from the locale registry", () => {
    assert.match(campaignEditor, /localeNames\[code\] = entry\.name/);
    assert.match(campaignEditor, /function localeOptions\(\)[\s\S]*?label: localeNames\[code\] \|\| code\.toUpperCase\(\)/);
    assert.match(campaignEditor, /Default story language"[^\n]*localeOptions\(\)/);
    assert.match(campaignEditor, /optionList\(\$\("#editLocale"\), localeOptions\(\)/);
    assert.match(campaignEditor, /optionList\(\$\("#previewLocale"\), localeOptions\(\)/);
});

test("campaign map coordinate fields retain focus and refresh placement feedback", () => {
    assert.match(campaignMapEditorHtml, /const updateFactionHint = \(\) =>/);
    assert.match(campaignMapEditorHtml, /getElementById\("fx"\)\.oninput = e => \{ f\.x=.*updateFactionHint\(\); renderList\(\); draw\(\); \}/);
    assert.match(campaignMapEditorHtml, /getElementById\("fy"\)\.oninput = e => \{ f\.y=.*updateFactionHint\(\); renderList\(\); draw\(\); \}/);
    assert.doesNotMatch(campaignMapEditorHtml, /getElementById\("f[xy]"\)\.oninput = e => \{[^\n]*renderSide\(\)/);
    assert.match(campaignMapEditorHtml, /const scrollTop = list\.scrollTop;[\s\S]*?list\.innerHTML = html;[\s\S]*?list\.scrollTop = scrollTop;/);
});

test("campaign map editor renders faction names as text in its HTML templates", () => {
    assert.match(campaignMapEditorHtml, /function escapeHtml\(value\)/);
    assert.match(campaignMapEditorHtml, /value="\$\{escapeHtml\(f\.name\)\}"/);
    assert.match(campaignMapEditorHtml, /class="nm">\$\{escapeHtml\(f\.name\)\}/);
    assert.match(campaignMapEditorHtml, /inWater\.map\(escapeHtml\)/);
});

test("troop objectives advance on the first update that satisfies the minimum", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "troop_minimum_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "minimum", steps: [
            { id: "minimum", type: "objective", title_key: "tutorial.minimum", hint_key: "tutorial.minimum_hint", trigger: { type: "troops", value: 1500, scope: "step" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.deepEqual(machine.update({ troops: 1400 }, {}, 0).progress, { current: 1400, target: 1500 });
    assert.equal(machine.update({ troops: 1500 }, {}).step.id, "ending");
});

test("a completed objective advances immediately after an exit modal closes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "modal_pause_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "objective", steps: [
            { id: "objective", type: "objective", title_key: "tutorial.objective", hint_key: "tutorial.objective_hint", trigger: { type: "territory", value: 1, scope: "step" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    machine.update({ tiles_gained: 0 }, {});
    machine.setPaused(true);
    assert.equal(machine.update({ tiles_gained: 1 }, {}).step.id, "objective");
    assert.equal(machine.advance(null, "objective"), false);
    machine.setPaused(false);
    assert.equal(machine.update({ tiles_gained: 1 }, {}).step.id, "ending");
});

test("UI objectives respect step, episode, and total scope", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const completed = scope => {
        const machine = campaign.create({
            version: 2, episode_id: "ui_scope_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 0 },
            strings: { en: {} }, speakers: {}, entry: "opening", steps: [
                { id: "opening", type: "scene", title_key: "tutorial.opening", next: "objective" },
                { id: "objective", type: "objective", title_key: "tutorial.objective", hint_key: "tutorial.objective_hint", trigger: { type: "ui", action: "attack_ratio", scope }, next: "ending" },
                { id: "ending", type: "end", title_key: "tutorial.ending" }
            ]
        });
        machine.update({}, { attack_ratio: 0 }, 0);
        machine.update({}, { attack_ratio: 1 }, 100);
        machine.advance(null, "opening");
        return machine.update({}, { attack_ratio: 1 }, 200).step.id === "ending";
    };

    assert.equal(completed("step"), false);
    assert.equal(completed("episode"), true);
    assert.equal(completed("total"), true);
});

test("campaign studio authors cinematic endings and navigable validation notes", () => {
    assert.match(campaignEditor, /step\.type === "scene" \|\| step\.type === "end"/);
    assert.match(campaignEditor, /Add closing text/);
    assert.match(campaignEditor, /warningsByStep\[issue\.step\]/);
    assert.match(campaignEditor, /report\.warnings\.forEach[\s\S]*?if \(issue\.step\) button\.addEventListener\("click", function \(\) \{ selectStep\(issue\.step\); \}\)/);
    const demo = campaignEditor.slice(campaignEditor.indexOf("function demo()"), campaignEditor.indexOf("function createMenuGuide()"));
    assert.match(demo, /lines: \[\{ speaker: leaderId[\s\S]*?speaker: advisorId/);
    assert.match(demo, /type: "choice"[\s\S]*?type: "choice"/);
    assert.doesNotMatch(demo, /Boudica|Rome/);
    assert.match(campaignEditor, /label: textValue\(choice\.label_key, state\.language\) \|\| choice\.id/);
    assert.match(campaignEditor, /Add consequence detail/);
    assert.match(campaignEditorHtml, /<summary>Quick start<\/summary>/);
    assert.match(campaignEditorHtml, /Use Create next step on each decision answer to build its path in place/);
});

test("branching sample preview preserves the episode draft and cannot be exported", () => {
    const demo = campaignEditor.slice(campaignEditor.indexOf("function demo()"), campaignEditor.indexOf("function createMenuGuide()"));
    assert.match(demo, /definition: JSON\.parse\(JSON\.stringify\(state\.definition\)\)/);
    assert.match(demo, /state\.definition = previous\.definition[\s\S]*?state\.dirty = previous\.dirty/);
    assert.doesNotMatch(demo, /confirm\(/);
    assert.match(campaignEditor, /function markDirty\(\) \{\s*if \(state\.demoBackup\) return;/);
    assert.match(campaignEditor, /async function save\(\) \{\s*if \(state\.demoBackup\) return false;/);
    assert.match(campaignEditor, /function download\(\) \{\s*if \(state\.demoBackup\) return;/);
    assert.match(campaignEditor, /async function watchExternalFiles\(\) \{\s*if \(state\.demoBackup \|\|/);
    assert.match(campaignEditorHtml, /id="demoBtn" class="sample-toggle"[^>]*>Preview branching sample/);
    assert.match(campaignEditorHtml, /id="graphPanel"[\s\S]*?id="inspectorPanel"/);
});

test("campaign dialogue regains keyboard focus after the leave modal is canceled", () => {
    assert.match(campaignView, /const wasHidden = root\.hidden/);
    assert.match(campaignView, /const focusModal = modal && \(!wasModal \|\| wasHidden\)/);
    assert.match(campaignView, /else if \(focusModal\) focusAction\(\)/);
});

test("campaign dialog X never exits the match and outside-click attempts to continue paused objectives", () => {
    assert.match(storyCss, /\.sow-story\.is-modal \{ pointer-events: none; \}/);
    assert.match(storyCss, /\.sow-story__shade \{[^}]*pointer-events: none/s);
    assert.match(storyCss, /\.sow-story__dialog \{[^}]*pointer-events: auto/s);
    assert.match(campaignView, /doc\.addEventListener\("click", outsideClick, true\)/);
    assert.match(campaignView, /doc\.removeEventListener\("click", outsideClick, true\)/);
    assert.match(campaignView, /event\.target\.closest\("button, a\[href\], input:not\(\[type='hidden'\]\), select, textarea, \[role='button'\], \[data-command\], \[data-map-action\]"\)/);
    assert.match(campaignView, /model\.step\.type === "choice" \|\| model\.choices\.length[\s\S]*?triggerNudge\(\)/);
    assert.match(campaignView, /function dismissDialog\(\)[\s\S]*?if \(model\.step\.type === "choice" \|\| model\.choices\.length\) \{ triggerNudge\(\); return; \}[\s\S]*?if \(options\.onDismiss\) options\.onDismiss\(\);\s*else if \(options\.onContinue\) options\.onContinue\(\)/);
    assert.match(campaignView, /if \(control\) return;[\s\S]*?model\.step\.type !== "choice"[\s\S]*?options\.onContinue\(\)/);
    assert.doesNotMatch(tutorial, /: openLeaveMatch/);
});

test("outside tap on a pending choice ignites the golden button nudge", () => {
    assert.match(campaignView, /model\.step\.type === "choice"[\s\S]*?triggerNudge\(\)/);
    assert.match(campaignView, /dialog\.classList\.add\("is-nudged"\)/);
    assert.match(campaignView, /vibrate\(12\)/);
    assert.match(campaignView, /dialog\.classList\.add\("is-waiting"\)/);
    assert.match(campaignView, /if \(beatChanged \|\| step\.type !== "choice"\) clearNudge\(\)/);
    assert.match(storyCss, /\.sow-story__dialog\.is-nudged \.sow-story__choice \{[^}]*animation: story-nudge-breathe 1\.6s ease-in-out infinite/);
    assert.match(storyCss, /@keyframes story-nudge-breathe \{[\s\S]*?border-color: #ffd98a/);
    assert.match(storyCss, /@keyframes story-nudge-sweep \{/);
    assert.match(storyCss, /@keyframes story-nudge-twinkle \{/);
    assert.match(storyCss, /\.sow-story__dialog\.is-nudged \.sow-story__choice::after \{[^}]*linear-gradient\(100deg, transparent, rgb\(255 236 190 \/ 50%\), transparent\)/);
    assert.match(storyCss, /\.sow-story\.is-reduced \.sow-story__dialog\.is-nudged \.sow-story__choice/);
});

test("campaign dialogue keeps a compact speaker portrait on narrow screens", () => {
    const mobileStart = storyCss.indexOf("@container (max-width: 640px)");
    const mobileEnd = storyCss.indexOf("@container (max-width: 380px)", mobileStart);
    const landscapeStart = storyCss.indexOf("@media (orientation: landscape) and (max-height: 560px)");
    const landscapeEnd = storyCss.indexOf("/* Decision nudge", landscapeStart);
    const mobileStory = storyCss.slice(mobileStart, mobileEnd);
    const landscapeStory = storyCss.slice(landscapeStart, landscapeEnd);
    assert.match(mobileStory, /\.sow-story\.has-portrait \.sow-story__main \{ grid-template-columns: var\(--story-portrait-size, min\(36cqw, 40svh\)\) minmax\(0, 1fr\); \}/);
    assert.match(storyCss, /\.sow-story__portrait \{ width: var\(--story-portrait-size,[^}]*height: var\(--story-portrait-size,[^}]*aspect-ratio: 1;[^}]*align-self: start/);
    assert.match(landscapeStory, /\.sow-story\.has-portrait \.sow-story__main \{ grid-template-columns: var\(--story-portrait-size, min\(36cqw, 40svh\)\) minmax\(0, 1fr\); \}/);
    assert.doesNotMatch(mobileStory + landscapeStory, /(?:144px|64px)/);
    assert.match(campaignView, /function syncMobilePortrait\(\)[\s\S]*?heading\.getBoundingClientRect\(\)\.height \+ headingGap \+ scrollContent\.scrollHeight[\s\S]*?Math\.min\(root\.clientWidth \* 0\.36, root\.clientHeight \* 0\.4\)[\s\S]*?const size = Math\.min\(contentHeight, maxSize\)[\s\S]*?new ResizeObserverCtor\(\(\) => \{[\s\S]*?view\.requestAnimationFrame\([\s\S]*?syncMobilePortrait\(\)/);
    assert.doesNotMatch(campaignView, /const contentHeight = conversation\.getBoundingClientRect\(\)\.height/);
    assert.match(campaignView, /portraitObserver\.observe\(conversation\)/);
    assert.match(campaignView, /portraitObserver\.observe\(root\)/);
    assert.match(campaignView, /portraitObserver\.disconnect\(\)/);
    assert.doesNotMatch(storyCss, /grid-template-columns: (?:72px|64px|clamp\(64px, 17vw, 72px\)) minmax\(0, 1fr\)/);
    assert.doesNotMatch(storyCss, /@container \(max-width: 380px\) \{[\s\S]*?\.sow-story__portrait \{ display: none/);
});

test("campaign dialogue resolves episode-local copy before the global catalog", () => {
    const resolverStart = tutorial.indexOf("function tr(key)");
    const resolverEnd = tutorial.indexOf("function zoomMode()", resolverStart);
    assert.ok(resolverStart >= 0 && resolverEnd > resolverStart);
    const resolver = tutorial.slice(resolverStart, resolverEnd);
    assert.ok(resolver.indexOf("return text;") < resolver.indexOf("window.SOW_t(key)"));
    assert.match(resolver, /strings\[definition\.default_locale\]/);
    assert.match(campaignView, /options\.translate\(key\)/);
});

test("mobile chapter dialogue hugs its copy while docked at the bottom", () => {
    const mobileStory = storyCss.slice(storyCss.indexOf("@container (max-width: 640px)"), storyCss.indexOf("@container (max-width: 380px)"));
    assert.match(mobileStory, /\.sow-story\.is-chapter \.sow-story__main \{[^}]*flex: 0 1 auto;[^}]*padding-block: 8px/);
    assert.match(mobileStory, /\.sow-story\.is-chapter \.sow-story__conversation \{ padding-block: 0; \}/);
    assert.match(mobileStory, /\.sow-story\.is-chapter \.sow-story__dialog \{ bottom: var\(--story-safe-bottom\); transform: none; \}/);
    assert.match(storyCss, /\.sow-story\.is-chapter \.sow-story__dialog \{ bottom: 50%; transform: translateY\(50%\)/);
});

test("mobile story dialogs scroll long text and choices while keeping speaker controls reachable", () => {
    assert.match(storyCss, /@container \(max-width: 640px\) \{[\s\S]*?max-height: min\(calc\(100% - var\(--story-safe-top\) - var\(--story-safe-bottom\)\), 60svh\)/);
    assert.match(storyCss, /@media \(orientation: landscape\) and \(max-height: 560px\) and \(hover: none\) and \(pointer: coarse\) \{[\s\S]*?max-height: min\(calc\(100% - var\(--story-safe-top\) - var\(--story-safe-bottom\)\), 72svh\)/);
    assert.match(storyCss, /\.sow-story__portrait img \{[^}]*width: 100%; height: 100%;[^}]*object-fit: cover/);
    assert.match(storyCss, /\.sow-story__scroll\s*\{[^}]*flex: 1 1 auto; min-height: 0; overflow-y: auto/);
    assert.match(storyCss, /\.sow-story__actions \{[^}]*overflow-y: auto/);
    assert.match(storyCss, /\.sow-story__choices \{ grid-template-columns: repeat\(auto-fit, minmax\(min\(100%, 280px\), 1fr\)\)/);
    assert.match(campaignView, /<div class="sow-story__main"><div class="sow-story__portrait"[\s\S]*?<div class="sow-story__conversation"><header class="sow-story__heading"[\s\S]*?data-story-dismiss[\s\S]*?<div class="sow-story__scroll"><h2[\s\S]*?<p class="sow-story__body"[\s\S]*?<\/div><\/div><\/div>'\s*\+\s*'<div class="sow-story__actions"><div class="sow-story__choices"><\/div><footer class="sow-story__footer"[\s\S]*?data-story-continue/);
    assert.match(campaignView, /footer\.hidden = step\.type === "choice"/);
    assert.match(campaignView, /button\.className = "sow-story__choice"[\s\S]*?choices\.appendChild\(button\)/);
});

test("quest panel has no fold state and mobile docks it under the nameplate", () => {
    assert.doesNotMatch(campaignView, /data-story-objective-toggle/);
    assert.doesNotMatch(campaignView, /setObjectiveExpanded/);
    assert.doesNotMatch(campaignView, /sow-story__objective-toggle/);
    assert.doesNotMatch(storyCss, /\.sow-story__objective-toggle/);
    assert.doesNotMatch(storyCss, /\.sow-story__objective\.is-expanded/);
    assert.match(hud, /sow-hud-nameplate/);
    assert.match(hud, /hudRefs\.plateTroops/);
    assert.match(hudCss, /\.sow-hud__nameplate:not\(\[hidden\]\) \{ display: flex/);
    const mobileObjective = storyCss.slice(storyCss.indexOf("@container (max-width: 720px)"), storyCss.indexOf("@container (max-width: 640px)"));
    assert.match(mobileObjective, /top: calc\(max\(8px, var\(--story-inset-top\)\) \+ 250px\)/);
    assert.match(mobileObjective, /inset-inline: auto max\(8px, var\(--story-inset-right\)\)/);
});

test("short landscape compaction stays on mobile and the editor reuses the shared dialogue", () => {
    assert.match(storyCss, /\.sow-story__dialog \{[^}]*max-height: calc\(100% - var\(--story-safe-top\) - var\(--story-safe-bottom\)\)/);
    assert.match(storyCss, /\.sow-story__main \{ display: contents; \}/);
    assert.match(storyCss, /\.sow-story\.has-portrait \.sow-story__dialog \{ grid-template-columns: clamp\(130px, 22cqw, 220px\) minmax\(0, 1fr\); \}/);
    assert.match(storyCss, /\.sow-story\.has-portrait \.sow-story__actions \{ grid-column: 2; \}/);
    assert.match(storyCss, /@media \(orientation: landscape\) and \(max-height: 560px\) and \(hover: none\) and \(pointer: coarse\)/);
    assert.match(storyCss, /\.sow-story\.is-chapter \.sow-story__dialog \{ bottom: 50%; transform: translateY\(50%\)/);
    assert.match(campaignEditorHtml, /href="\/shell\/main_menu\.tutorial\.css"/);
    assert.match(campaignEditorHtml, /src="\/shell\/sow-campaign-view\.js"/);
    assert.match(campaignEditor, /window\.SOWCampaignView\.mount/);
});

test("campaign dialogue announces each new spoken line to screen readers", () => {
    assert.match(campaignView, /class="sow-story__body"[^>]*aria-live="polite" aria-atomic="true"/);
});

test("campaign scene transitions are brief, shared, and disabled for reduced motion", () => {
    assert.match(campaignView, /const beatChanged = Boolean\(lastBeatKey && beatKey !== lastBeatKey\)/);
    assert.match(campaignView, /matchMedia\("\(prefers-reduced-motion: reduce\)"\)/);
    assert.match(campaignView, /if \(beatChanged && !reducedMotion\)/);
    assert.match(campaignView, /\[speaker, title, body, image, choices\]\.forEach/);
    assert.match(campaignView, /duration: 180, easing: "cubic-bezier\(\.2,\.7,\.2,1\)"/);
});

test("objective completion has no artificial wait or delayed preview timer", () => {
    assert.doesNotMatch(campaignEngine, /readyAt|reactionOpenedAt|now - readyAt/);
    assert.doesNotMatch(campaignView, /is-ready|model\.ready/);
    assert.doesNotMatch(storyCss, /story-objective-complete|is-ready/);
    assert.doesNotMatch(campaignEditor, /setTimeout\(function \(\) \{ if \(state\.machine\) paintPreview\(\); \}, 840\)/);
    assert.match(storyCss, /\.sow-story\.is-reduced [^{]*\{ animation: none !important/);
    assert.match(storyCss, /\.sow-story__meter progress \{[^}]*height: 10px[^}]*border-radius: 999px/);
});

test("campaign pacing can wait without a hand and guide steps require a target", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "timing_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.wait": "Wait", "tutorial.end": "Done" } }, speakers: {}, entry: "wait", steps: [
            { id: "wait", type: "objective", title_key: "tutorial.wait", trigger: { type: "elapsed", value: 5, scope: "step" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.end" }
        ]
    };
    assert.deepEqual(campaign.validate(definition).errors, []);
    definition.steps[0].guide = { kind: "world", target: "expand", gesture: "tap" };
    assert.ok(campaign.validate(definition).errors.some(issue => issue.step === "wait" && issue.field === "guide"));
    delete definition.steps[0].guide;
    definition.steps[0] = { id: "wait", type: "guide", title_key: "tutorial.wait", trigger: { type: "territory", value: 1, scope: "step" }, next: "ending" };
    assert.ok(campaign.validate(definition).errors.some(issue => issue.step === "wait" && issue.field === "guide"));
    assert.match(campaignEditor, /Add hand guide/);
    assert.match(campaignEditor, /Remove hand guide/);
    assert.match(campaignEditor, /tabindex: "0", role: "group"/);
    assert.match(campaignEditor, /direction: rtlLanguages\.has\(state\.previewLanguage\.toLowerCase\(\)\.split\("-"\)\[0\]\) \? "rtl" : "ltr"/);
    assert.match(campaignView, /root\.dir = context\.direction/);
    assert.match(tutorial, /direction: document\.documentElement\.dir/);
});

test("campaign hand guides model the actual slider and support local or world drags", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.equal(campaign.UI_TARGETS.troops, undefined);
    assert.equal(campaign.UI_TARGETS.attack_ratio, "#sow-hud-slider");
    const definition = {
        version: 2, episode_id: "guide_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "slider", steps: [
            { id: "slider", type: "objective", title_key: "tutorial.slider", trigger: { type: "ui", action: "attack_ratio", scope: "step" }, guide: { kind: "ui", target: "attack_ratio", gesture: "drag" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.end" }
        ]
    };
    assert.deepEqual(campaign.validate(definition).errors, []);
    definition.steps[0].guide.to = "not_a_control";
    assert.ok(campaign.validate(definition).errors.some(issue => issue.step === "slider" && issue.field === "guide.to"));
    definition.steps[0].guide = { kind: "world", target: "expand", gesture: "drag", to: "assault" };
    assert.deepEqual(campaign.validate(definition).errors, []);
    const demo = campaignEditor.slice(campaignEditor.indexOf("function demo()"), campaignEditor.indexOf("function createMenuGuide()"));
    assert.match(demo, /trigger: \{ type: "ui", action: "attack_ratio", scope: "step" \}, guide: \{ kind: "ui", target: "attack_ratio", gesture: "drag" \}/);
    assert.match(campaignEditor, /Within this control/);
    assert.match(campaignEditor, /if \(guide\.kind === "world"\) end = previewWorldMarker\(frame, guide\.to, step\)/);
});

test("campaign story preview uses the selected episode map and faction positions", () => {
    assert.match(campaignEditorHtml, /id="campaignMapPreview"/);
    assert.match(campaignEditorHtml, /id="campaignPreviewMarkers"/);
    assert.match(campaignEditor, /SOWCampaignMapPreview\.load\(mapId\)/);
    assert.match(campaignEditor, /dataset: \{ factionId: faction\.id \}/);
    assert.match(campaignEditor, /Number\(faction\.x\) \/ map\.width \* 100/);
    assert.match(campaignEditor, /function previewWorldMarker\(frame, target, step\)/);
    assert.match(campaignMapEditorHtml, /SOWCampaignMapPreview\.load\(mapId\)/);
    assert.match(campaignMapPreview, /!== "SOWM"/);
    assert.match(campaignMapPreview, /function terrainCanvas\(terrain, width, height\)/);
    assert.match(campaignMapEditorHtml, /SOWCampaignMapPreview\.isLand\(terrain, MAPW, MAPH, mx, my\)/);
    assert.match(campaignMapEditorHtml, /sel=\+el\.dataset\.i; focusFaction\(sel\)/);
    assert.match(atlasHtml, /button\.onclick=\(\)=>select\(entity\.id,true\)/);
    assert.match(atlasHtml, /SOWCampaignMapPreview\.load\("world"\)/);
    assert.match(atlasHtml, /thumbnail_frames\.json/);
    assert.match(atlasHtml, /giantworldmap\.png/);
    assert.match(atlasHtml, /const frame=worldReference&&!worldReferenceError&&worldReference\.source\.map_grid_frame/);
    assert.match(atlasHtml, /ctx\.drawImage\(worldReference\.image,frame\[0\],frame\[1\],frame\[2\],frame\[3\],0,0,world\.width,world\.height\)/);
    assert.doesNotMatch(atlasHtml, /drawEquirectangularImage/);
    assert.match(atlasHtml, /ctx\.globalAlpha=0\.2/);
    assert.match(atlasHtml, /zoomMapView\(viewState\(\),point,factor,minScale,fitScale\*64\)/);
    assert.match(campaignMapEditorHtml, /canvas \{ position:absolute; top:0; left:0; width:100%; height:100%;/);
    for (const viewer of [campaignMapEditorHtml, atlasHtml, mapRostersHtml]) {
        assert.match(viewer, /SOWCampaignMapPreview\.resizeCanvas\(/);
        assert.match(viewer, /SOWCampaignMapPreview\.fitMapView\(/);
        assert.match(viewer, /SOWCampaignMapPreview\.zoomMapView\(/);
    }
    assert.match(mapRostersHtml, /window\.addEventListener\("resize",resize\);resize\(\);\s*document\.addEventListener/);
    assert.match(atlasHtml, /mapPointToGeo/);
    assert.match(atlasHtml, /world_island_guides\.json/);
    assert.match(atlasHtml, /new Path2D\(\)/);
    assert.match(atlasHtml, /ctx\.globalAlpha=1;ctx\.fillStyle=[^;]+;ctx\.fill\(worldIslandPath,"evenodd"\);ctx\.strokeStyle=[^;]+;ctx\.lineWidth=0\.75\/scale;[^;]+;[^;]+;ctx\.stroke\(worldIslandPath\)/);
    assert.match(atlasHtml, /pointerStart=pointer\.slice\(\)/);
    assert.match(atlasHtml, /Math\.hypot\(point\[0\]-pointerStart\[0\],point\[1\]-pointerStart\[1\]\)>Math\.min\(2,scale\*0\.5\)/);
    assert.match(atlasHtml, /body:JSON\.stringify\(catalog\)/);
    assert.match(campaignEditorServer, /JSON\.stringify\(value, null, 2\)/);
    const atlasScript = atlasHtml.match(/<script>\s*([\s\S]*?)<\/script>/);
    assert.ok(atlasScript, "Atlas inline script exists");
    assert.doesNotThrow(() => new vm.Script(atlasScript[1]));
    assert.match(atlasHtml, /geoToTile\(entity\.lat,entity\.lon,world\.geoBounds/);
    assert.match(atlasHtml, /SOWCampaignMapPreview\.isLand\(world\.terrain,world\.width,world\.height,tile\.x,tile\.y\)/);

    const islandGuides = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/map_sources/world_island_guides.json"), "utf8"));
    assert.equal(islandGuides.coordinate_reference, "WGS84 longitude,latitude");
    assert.ok(islandGuides.sources.some(source => source.name === "Natural Earth 10m Land"));
    assert.ok(islandGuides.sources.some(source => source.name === "Natural Earth 10m Minor Islands"));
    const entities = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/geo_entities.json"), "utf8")).entities;
    for (const id of ["rapa_nui", "tahitians", "marquesans"]) {
        const entity = entities.find(item => item.id === id);
        const rings = islandGuides.features.filter(feature => feature.properties.entity_ids.includes(id)).map(feature => feature.geometry.coordinates);
        assert.ok(entity && rings.length, `${id} has a sourced island outline`);
        const nearestVertex = Math.min(...rings.flatMap(ring => ring.map(([lon, lat]) => Math.hypot(lon - entity.lon, lat - entity.lat))));
        assert.ok(nearestVertex < 0.2, `${id} outline remains anchored to its catalog coordinates`);
    }
});

test("selecting a map entity centers it in the current view", () => {
    const offsets = campaignMapPreviewApi.centerMapOffsets({ x: 120, y: 40 }, 2, 800, 600);
    assert.deepEqual(offsets, { x: 160, y: 220 });
    assert.equal(120 * 2 + offsets.x, 400);
    assert.equal(40 * 2 + offsets.y, 300);
});

test("Atlas deletes the selected entity with Delete outside editable fields", () => {
    assert.match(atlasHtml, /function deleteSelected\(\)[\s\S]*?catalog\.entities\.splice\(index,1\);[\s\S]*?selectedId=null;[\s\S]*?markDirty\(\)/);
    assert.match(atlasHtml, /document\.addEventListener\("keydown",event=>\{[\s\S]*?event\.key!=="Delete"[\s\S]*?event\.key!=="Backspace"[\s\S]*?input,textarea,select,\[contenteditable='true'\][\s\S]*?event\.preventDefault\(\);deleteSelected\(\)/);
});

test("playable map preview shares game geography and land checks", () => {
    function encodeMap(bounds) {
        const width = 4, height = 2, name = Buffer.from("test"), terrain = Buffer.from([0x80, 0, 0x80, 0, 0, 0x80, 0x80, 0]);
        const bytes = Buffer.alloc(4 + 2 + 2 + 4 + 4 + 4 + 2 + name.length + 2 + terrain.length + (bounds ? 17 : 0));
        let offset = 0;
        bytes.write("SOWM", offset); offset += 4;
        bytes.writeUInt16LE(1, offset); offset += 4;
        bytes.writeUInt32LE(width, offset); offset += 4;
        bytes.writeUInt32LE(height, offset); offset += 4;
        bytes.writeUInt32LE(4, offset); offset += 4;
        bytes.writeUInt16LE(name.length, offset); offset += 2;
        name.copy(bytes, offset); offset += name.length;
        bytes.writeUInt16LE(0, offset); offset += 2;
        terrain.copy(bytes, offset); offset += terrain.length;
        if (bounds) {
            bytes.writeUInt8(1, offset++);
            for (const value of bounds) { bytes.writeInt32LE(Math.round(value * 1e6), offset); offset += 4; }
        }
        return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    }

    const bounds = { minLon: 170, minLat: -10, maxLon: 190, maxLat: 10 };
    const map = campaignMapPreviewApi.parse(encodeMap([170, -10, 190, 10]));
    assert.deepEqual(map.geoBounds, bounds);
    assert.deepEqual(campaignMapPreviewApi.geoToTile(0, -180, map.geoBounds, map.width, map.height), { x: 2, y: 1 });
    assert.deepEqual(campaignMapPreviewApi.geoToTile(10, 190, map.geoBounds, map.width, map.height), { x: 3, y: 0 });
    const preciseGeo = campaignMapPreviewApi.mapPointToGeo(2.25, 0.4, map.geoBounds, map.width, map.height);
    assert.ok(Math.abs(preciseGeo.lat - 6) < 1e-12);
    assert.ok(Math.abs(preciseGeo.lon + 178.75) < 1e-12);
    const precisePoints = campaignMapPreviewApi.geoToMapPoints(preciseGeo.lat, preciseGeo.lon, map.geoBounds, map.width, map.height);
    assert.ok(precisePoints.some(point => Math.abs(point.x - 2.25) < 1e-9 && Math.abs(point.y - 0.4) < 1e-9));
    assert.equal(campaignMapPreviewApi.mapPointToGeo(map.width, 0, map.geoBounds, map.width, map.height), null);
    const geo = campaignMapPreviewApi.tileToGeo(2, 1, map.geoBounds, map.width, map.height);
    assert.deepEqual(campaignMapPreviewApi.geoToTile(geo.lat, geo.lon, map.geoBounds, map.width, map.height), { x: 2, y: 1 });
    assert.equal(campaignMapPreviewApi.isLand(map.terrain, map.width, map.height, 2, 1), true);
    assert.equal(campaignMapPreviewApi.isLand(map.terrain, map.width, map.height, 3, 1), false);
    assert.equal(campaignMapPreviewApi.isLand(map.terrain, map.width, map.height, map.width, 0), false);
    assert.equal(campaignMapPreviewApi.parse(encodeMap(null)).geoBounds, null);
    assert.equal(campaignMapPreviewApi.geoToTile(0, 0, map.geoBounds, map.width, map.height), null);

    const worldBytes = fs.readFileSync(path.join(shell, "../../assets/maps/world/map.bin"));
    const worldBuffer = worldBytes.buffer.slice(worldBytes.byteOffset, worldBytes.byteOffset + worldBytes.byteLength);
    const world = campaignMapPreviewApi.parse(worldBuffer);
    assert.deepEqual(world.geoBounds, { minLon: -168.69, minLat: -78.8, maxLon: 192.37, maxLat: 82.78 });
    const london = campaignMapPreviewApi.geoToTile(51.51, -0.13, world.geoBounds, world.width, world.height);
    assert.deepEqual(london, { x: 466, y: 96 });
    assert.equal(campaignMapPreviewApi.isLand(world.terrain, world.width, world.height, london.x, london.y), true);
    assert.equal(campaignMapPreviewApi.tileToGeo(999, 200, world.geoBounds, world.width, world.height), null);
    const wrappedGeo = campaignMapPreviewApi.mapPointToGeo(999.25, 250, world.geoBounds, world.width, world.height);
    const wrappedPoints = campaignMapPreviewApi.geoToMapPoints(wrappedGeo.lat, wrappedGeo.lon, world.geoBounds, world.width, world.height);
    assert.ok(wrappedPoints.some(point => Math.abs(point.x - 999.25) < 1e-9 && Math.abs(point.y - 250) < 1e-9));

    const source = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/map_sources/thumbnail_frames.json"), "utf8")).sources["giantworldmap.png"];
    assert.deepEqual(source.map_grid_frame, [16, 16, 4094, 1932]);
    const sourceBytes = fs.readFileSync(path.join(shell, "../../assets/map_sources/giantworldmap.png"));
    assert.equal(sourceBytes.toString("hex", 0, 8), "89504e470d0a1a0a");
    assert.equal(sourceBytes.readUInt32BE(16), source.width);
    assert.equal(sourceBytes.readUInt32BE(20), source.height);
    const referenceImage = { naturalWidth: source.width, naturalHeight: source.height };
    const icelandSource = { x: 1717, y: 237 }, icelandTarget = campaignMapPreviewApi.geoToMapPoint(65, -18.5, world.geoBounds, world.width, world.height);
    const [sx, sy, sw, sh] = source.map_grid_frame;
    const icelandAligned = { x: (icelandSource.x - sx) / sw * world.width, y: (icelandSource.y - sy) / sh * world.height };
    assert.ok(Math.abs(icelandAligned.x - icelandTarget.x) < 1, "Iceland source anchor aligns to its catalog coordinate");
    assert.ok(Math.abs(icelandAligned.y - icelandTarget.y) < 2.5, "Iceland source anchor aligns to its catalog coordinate");
});

test("invalid campaign drafts preserve the last valid preview and disable simulation", () => {
    assert.match(campaignEditor, /function syncPreview\(\) \{\s*if \(state\.validation\.errors\.length\)/);
    assert.match(campaignEditor, /Fix validation errors to preview this draft/);
    assert.match(campaignEditor, /\["playSelected", "restartBtn", "simulateBtn", "tickBtn", "previewBranchSelect"\][\s\S]*?\.disabled = blocked/);
    assert.match(campaignEditor, /function paintPreview\(updateMachine\) \{\s*if \(!state\.machine \|\| state\.validation\.errors\.length\) return/);
    assert.match(campaignEditor, /function renderPreview\(forceStep\) \{\s*if \(!state\.definition\) return;\s*if \(state\.validation\.errors\.length\)/);
});

test("campaign map preview accepts valid SOWM terrain and rejects a truncated map", () => {
    const mapPreview = require(path.join(shell, "../../sow-tools/editors/campaign-editor/map-preview.js"));
    const buffer = new ArrayBuffer(26), view = new DataView(buffer), bytes = new Uint8Array(buffer);
    bytes.set([83, 79, 87, 77]);
    view.setUint16(4, 1, true); view.setUint32(8, 2, true); view.setUint32(12, 1, true);
    view.setUint16(20, 0, true); view.setUint16(22, 0, true);
    bytes.set([0x80, 0], 24);
    const map = mapPreview.parse(buffer);
    assert.equal(map.width, 2);
    assert.equal(map.height, 1);
    assert.deepEqual(Array.from(map.terrain), [0x80, 0]);
    assert.throws(() => mapPreview.parse(buffer.slice(0, 25)), /Invalid SOW map dimensions/);
});

test("campaign map preview shares canvas sizing and anchored viewport math", () => {
    const canvas = {
        width: 300,
        height: 150,
        getBoundingClientRect: () => ({ width: 720, height: 400 })
    };
    assert.deepEqual(campaignMapPreviewApi.resizeCanvas(canvas, 2), { width: 720, height: 400, dpr: 2 });
    assert.equal(canvas.width, 1440);
    assert.equal(canvas.height, 800);

    const fit = campaignMapPreviewApi.fitMapView(1000, 500, 720, 400);
    assert.deepEqual(fit, { scale: 0.72, fitScale: 0.72, minScale: 0.36, x: 0, y: 20 });
    const anchor = [360, 200];
    const before = campaignMapPreviewApi.screenToMap(anchor, fit);
    const zoomed = campaignMapPreviewApi.zoomMapView(fit, anchor, 0.1, fit.minScale, fit.fitScale * 64);
    const after = campaignMapPreviewApi.screenToMap(anchor, zoomed);
    assert.ok(Math.abs(zoomed.scale - fit.minScale) < 1e-12, "zoom out stops at half Fit");
    assert.ok(Math.abs(after[0] - before[0]) < 1e-12);
    assert.ok(Math.abs(after[1] - before[1]) < 1e-12);
    assert.deepEqual(campaignMapPreviewApi.panMapView(zoomed, 12, -8), {
        scale: zoomed.scale, x: zoomed.x + 12, y: zoomed.y - 8
    });
    assert.deepEqual(campaignMapPreviewApi.mapToScreen(before, fit), anchor);
});

test("Map Rosters reads binary presets and leaves unmatched map anchors intact", () => {
    const mapPreview = require(path.join(shell, "../../sow-tools/editors/campaign-editor/map-preview.js"));
    const u16 = value => { const b = Buffer.alloc(2); b.writeUInt16LE(value); return b; };
    const u32 = value => { const b = Buffer.alloc(4); b.writeUInt32LE(value); return b; };
    const string = value => { const b = Buffer.from(value); return Buffer.concat([u16(b.length), b]); };
    const spawn = Buffer.concat([string("Unlinked local anchor"), string(""), u32(0), u32(0)]);
    const terrain = Buffer.from([0x80, 0x80, 0x80, 0x80]);
    const roster = Buffer.concat([
        u16(1), string("historical"), u16(1), string("historical"), string("Historical"), u16(1),
        string("iceland"), Buffer.from([0]), u32(2), u32(0), u16(0)
    ]);
    const bytes = Buffer.concat([
        Buffer.from("SOWM"), u16(1), u16(0), u32(4), u32(1), u32(4), string("Test map"),
        u16(1), spawn, terrain, Buffer.from([0, 2]), u32(roster.length), roster
    ]);
    const parsed = mapPreview.parse(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));

    assert.equal(parsed.defaultRoster, "historical");
    assert.deepEqual(parsed.spawns, [{ name: "Unlinked local anchor", flag: "", x: 0, y: 0 }]);
    assert.deepEqual(parsed.rosters, [{
        id: "historical", name: "Historical", entries: [
            { entity_id: "iceland", role: "nation", x: 2, y: 0, legacy_anchor: 0 }
        ]
    }]);
    assert.match(mapRostersHtml, /SOWCampaignMapPreview\.load\(id\)/);
    assert.match(mapRostersHtml, /assets\/gameplay\/avatars/);
    assert.match(mapRostersHtml, /SOWCampaignMapPreview\.drawMapLabel/);
    const rosterScript = mapRostersHtml.match(/<script>\s*([\s\S]*?)<\/script>/);
    assert.ok(rosterScript, "Map Rosters inline script exists");
    assert.doesNotThrow(() => new vm.Script(rosterScript[1]));
    assert.match(mapRosterWriter, /map_file::encode\(&map\)/);
    assert.match(mapRosterWriter, /map\.bin\.br/);
    assert.doesNotMatch(mapRosterWriter.split("#[cfg(test)]")[0], /roster\.json/i);
});

test("Atlas picker visibly selects a catalog entity by stable ID in both editors", async () => {
    class FakeElement {
        constructor(id = "") { this.id = id; this.children = []; this.listeners = {}; this.attributes = {}; this.value = ""; this.textContent = ""; }
        setAttribute(name, value) { this.attributes[name] = value; }
        addEventListener(name, callback) { (this.listeners[name] ||= []).push(callback); }
        append(...children) { this.children.push(...children); }
        replaceChildren(...children) { this.children = children; }
        async dispatch(name, event = {}) { for (const callback of this.listeners[name] || []) await callback(event); }
    }
    const context = { document: { createElement: () => new FakeElement() } };
    vm.runInNewContext(atlasEntityPickerSource, context);
    const pickerApi = context.SOWAtlasEntityPicker;
    const entities = [
        { id: "iceland", name: "Iceland", kind: "country", region: "europe" },
        { id: "iceni", name: "Iceni", kind: "tribe", region: "europe" }
    ];
    const input = new FakeElement("search");
    const results = new FakeElement("results");
    const status = new FakeElement("status");
    const picker = pickerApi.attach({ input, results, status, getEntities: () => entities });

    input.value = "icel";
    await input.dispatch("input");
    assert.equal(results.children.length, 1);
    assert.equal(results.children[0].children[0].textContent, "Iceland");
    await results.children[0].dispatch("click");
    assert.equal(input.value, "Iceland");
    assert.equal(picker.getSelected().id, "iceland");

    input.value = "ICENI";
    await input.dispatch("input");
    assert.equal(picker.getSelected().id, "iceni", "exact name input resolves to its stable Atlas ID");
    assert.match(mapRostersHtml, /SOWAtlasEntityPicker\.attach/);
    assert.match(mapRostersHtml, /entityPicker\.select\(selectedEntity\)/);
    assert.match(mapRostersHtml, /entity_id:found\.id,role:/);
    assert.match(mapRostersHtml, /pendingId=found\.id/);
    assert.match(mapRostersHtml, /if\(placeAt\(pendingId,point\)\)entityPicker\?\.clear\(\)/);
    assert.match(campaignMapEditorHtml, /SOWAtlasEntityPicker\.attach/);
    assert.match(campaignMapEditorHtml, /atlasEntityPicker\.getSelected\(\)/);
    assert.match(campaignMapEditorHtml, /color:FACTION_COLORS\[roster\.factions\.length%FACTION_COLORS\.length\]/);
    const episodeEditorScript = campaignMapEditorHtml.match(/<script>\s*([\s\S]*?)<\/script>/);
    assert.ok(episodeEditorScript, "Campaign Studio Episode script exists");
    assert.doesNotThrow(() => new vm.Script(episodeEditorScript[1]));
    assert.match(atlasHtml, /map-rosters-link/);
    assert.match(atlasHtml, /entity=\$\{encodeURIComponent\(selectedId\)\}/);
});

test("campaign studio preserves edits made while a save is in flight", () => {
    assert.match(campaignEditor, /var savedDraft = JSON\.stringify\(state\.definition, null, 2\) \+ "\\n"/);
    assert.match(campaignEditor, /if \(report\.errors\.length\)[\s\S]*?if \(state\.saving\) return false/);
    assert.match(campaignEditor, /finally \{ state\.saving = false; refresh\(\); \}/);
    assert.match(campaignEditor, /body: savedDraft/);
    assert.match(campaignEditor, /JSON\.stringify\(state\.definition, null, 2\) \+ "\\n" !== savedDraft/);
});

test("campaign studio reports each external draft conflict only once per file version", () => {
    assert.match(campaignEditor, /externalChangeTag: null/);
    assert.match(campaignEditor, /var externalTag = responses\.map\(function \(response\) \{ return response\.headers\.get\("ETag"\); \}\)\.join\("\\n"\)/);
    assert.match(campaignEditor, /if \(externalTag === state\.externalChangeTag\) return/);
    assert.match(campaignEditor, /state\.externalChangeTag = null;\s*await load\(\)/);
});

test("menu guide uses the shared flow engine from its own entry", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "menu_guide_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "game_open", menu_guide: { entry: "menu_open", dismissible: true },
        steps: [
            { id: "game_open", type: "scene", title_key: "tutorial.game", next: "game_end" },
            { id: "game_end", type: "end", title_key: "tutorial.game_end" },
            { id: "menu_open", type: "scene", title_key: "tutorial.menu", next: "menu_campaign" },
            { id: "menu_campaign", type: "guide", title_key: "tutorial.campaign", trigger: { type: "ui", action: "menu_campaign", scope: "step" }, guide: { kind: "ui", target: "menu_campaign", gesture: "tap" }, next: "menu_end" },
            { id: "menu_end", type: "end", title_key: "tutorial.menu_end" }
        ]
    };
    const report = campaign.validate(definition);
    assert.deepEqual(report.errors, []);
    assert.deepEqual(report.warnings, []);
    const machine = campaign.create(definition, definition.menu_guide.entry);
    assert.equal(machine.view().step.id, "menu_open");
    machine.update({}, {}, 0);
    machine.advance(null, "menu_open");
    machine.update({}, { menu_campaign: 1 }, 1);
    machine.update({}, { menu_campaign: 1 }, 801);
    assert.equal(machine.view().step.id, "menu_end");
});

test("Boudica completion points the menu guide hand at an active lobby", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const episode = path.join(shell, "../../assets/campaign");
    const definition = JSON.parse(fs.readFileSync(path.join(episode, "boudica.triggers.json"), "utf8"));
    const roster = JSON.parse(fs.readFileSync(path.join(episode, "boudica.json"), "utf8"));
    const avatarDir = path.join(shell, "../../assets/gameplay/avatars");
    const avatars = new Set(fs.readdirSync(avatarDir).filter(file => file.endsWith(".webp")).map(file => file.slice(0, -5)));
    const report = campaign.validate(definition, roster, { hasText: () => true, hasAvatar: avatar => avatars.has(avatar) });
    assert.deepEqual(report.errors, []);
    assert.deepEqual(report.warnings, []);
    const combatBehaviorRoster = structuredClone(roster);
    combatBehaviorRoster.factions.find(faction => faction.id === "camulodunum").hostility = "aggressive";
    assert.deepEqual(campaign.validate(definition, combatBehaviorRoster, { hasText: () => true, hasAvatar: avatar => avatars.has(avatar) }).errors, []);
    assert.equal(definition.menu_guide.dismissible, true);
    assert.equal(definition.menu_guide.entry, "boudica_return_lobby");
    assert.equal(definition.steps.find(step => step.id === definition.menu_guide.entry).trigger.action, "menu_lobby");
    assert.equal(definition.steps.find(step => step.id === definition.menu_guide.entry).guide.target, "menu_lobby");
    assert.equal(definition.steps.find(step => step.id === definition.menu_guide.entry).next, "boudica_return_end");
    for (const obsolete of ["boudica_return_campaign", "boudica_return_replay", "boudica_return_multiplayer"]) {
        assert.equal(definition.steps.find(step => step.id === obsolete), undefined, obsolete + " must not come back");
    }
    assert.equal(definition.steps.find(step => step.id === "boudica_return_end").speaker, "boudica");
    assert.equal(definition.steps.find(step => step.id === "boudica_end").body_key, "tutorial.boudica_end_body");
    assert.equal(definition.steps.find(step => step.id === "boudica_complete").type, "end");
    for (const type of ["farm", "factory", "port", "bunker", "structure_upgrade", "city_upgrade", "city_level", "foundry_level", "port_upgrade", "port_level", "tile_upgrade", "resource_transfer", "alliance", "fleet", "zoom_in", "zoom_out", "camera_drag", "camera_key_pan", "hover"]) {
        assert.ok(campaign.METRICS[type], `missing campaign metric ${type}`);
        assert.match(campaignEditor, new RegExp(`value: "${type}"`), `Campaign Studio cannot select ${type}`);
    }
    const step = id => definition.steps.find(candidate => candidate.id === id);
    for (const obsolete of ["boudica_first_contact_intro", "boudica_first_contact_response", "boudica_trinovantes_intro", "boudica_trinovantes_pact", "boudica_share_intro", "boudica_rebuild_intro", "boudica_farm_intro", "boudica_ratio_intro", "boudica_posts_fall", "boudica_foundry_ready", "boudica_factory_intro", "boudica_factory_ready", "boudica_bunker_intro", "boudica_bunker_ready", "boudica_pact_formed", "boudica_port_intro", "boudica_fleet_intro", "boudica_fleet_ready"]) {
        assert.equal(step(obsolete), undefined, `obsolete dialogue step returned: ${obsolete}`);
    }
    assert.equal(step("boudica_first_victory").attack_ratio_on_enter, 1);
    assert.equal(step("boudica_ratio_guide").attack_ratio_on_enter, 0.5);
    assert.equal(step("boudica_first_victory_scene").next, "boudica_first_expansion");
    assert.equal(step("boudica_first_expansion").next, "boudica_zoom_out");
    assert.equal(step("boudica_zoom_out").pause_game, true);
    assert.equal(step("boudica_zoom_out").next, "boudica_zoom_in");
    assert.equal(step("boudica_zoom_in").next, "boudica_camera_drag");
    assert.equal(step("boudica_camera_drag").routes[0].next, "boudica_first_contact");
    assert.equal(step("boudica_camera_keys").next, "boudica_first_contact");
    assert.equal(step("boudica_camera_hover").marker.target, "camulodunum");
    assert.equal(step("boudica_camera_hover").next, "boudica_camulodunum_intro");
    assert.equal(step("boudica_bunker_choice").choices.find(choice => choice.id === "skip").next, "boudica_camera_hover");
    assert.equal(step("boudica_build_bunker").next, "boudica_camera_hover");
    assert.deepEqual(step("boudica_first_contact").trigger.targets, ["stonea", "snettisham", "thetford"]);
    assert.equal(step("boudica_first_contact").trigger.value, 3);
    assert.equal(step("boudica_first_contact").next, "boudica_outpost_colonia_veterans");
    assert.equal(definition.steps.some(candidate => /boudica_contact_.*pledge/.test(candidate.id)), false);
    assert.equal(step("boudica_first_contact").guide.target, "target_action");
    assert.deepEqual(roster.factions.filter(faction => ["stonea", "snettisham", "thetford"].includes(faction.id)).map(faction => faction.name).sort(), ["Snettisham", "Stonea", "Thetford"]);
    const friendly = definition.reactions.find(reaction => reaction.id === "contact_stonea");
    assert.equal(friendly.when.target, "stonea");
    assert.equal(friendly.outcome, "allied");
    const tribute = definition.reactions.find(reaction => reaction.id === "contact_snettisham");
    assert.deepEqual(tribute.choices.map(choice => [choice.id, choice.relation, choice.gold_cost]), [["accept", "allied", 200], ["refuse", "enemy", 0]]);
    const respect = definition.reactions.find(reaction => reaction.id === "contact_thetford");
    assert.deepEqual(respect.choices.map(choice => [choice.id, choice.relation, choice.gold_cost]), [["respect", "allied", 0], ["demand", "enemy", 0]]);
    assert.equal(roster.factions.find(faction => faction.name === "Stonea").support_interval_seconds, 15);
    assert.equal(step("boudica_transfer_send").trigger.recipient, "stonea");
    assert.equal(roster.factions.some(faction => ["Iceni Coast", "Venta Icenorum"].includes(faction.name)), false);
    assert.equal(definition.reactions.find(reaction => reaction.id === "neutral_contact_terms").when.relation, "neutral");
    const unnamedClan = roster.factions.find(faction => faction.name === "Trinovantian Farms");
    assert.ok(unnamedClan && avatars.has(unnamedClan.avatar));
    assert.equal(Object.values(definition.speakers).some(speaker => speaker.faction === unnamedClan.id), false);
    assert.match(campaignView, /if \(!speakerKey && model\.reactionTarget\) character\.faction = model\.reactionTarget;/);
    const paidTerms = definition.reactions.find(reaction => reaction.choices && reaction.choices.some(choice => choice.gold_cost > 0));
    assert.ok(paidTerms, "campaign includes a gold negotiation");
    const withoutRefusal = JSON.parse(JSON.stringify(definition));
    const brokenTerms = withoutRefusal.reactions.find(reaction => reaction.id === paidTerms.id);
    brokenTerms.choices = brokenTerms.choices.filter(choice => choice.relation === "allied");
    assert.ok(campaign.validate(withoutRefusal, roster, { hasText: () => true, hasAvatar: avatar => avatars.has(avatar) }).errors.some(issue => issue.field === "reactions.choices"),
        "a paid offer cannot be authored without its free refusal path");
    const lootRoster = JSON.parse(JSON.stringify(roster));
    lootRoster.factions[0].gold_loot_bonus = 275;
    assert.deepEqual(campaign.validate(definition, lootRoster, { hasText: () => true, hasAvatar: avatar => avatars.has(avatar) }).errors, []);
    lootRoster.factions[0].gold_loot_bonus = 1_000_001;
    assert.ok(campaign.validate(definition, lootRoster, { hasText: () => true, hasAvatar: avatar => avatars.has(avatar) }).errors.some(issue => issue.field === "roster.factions.gold_loot_bonus"));
    assert.match(tutorial, /gold_insufficient: Number\(choice\.gold_cost \|\| 0\) > gold/);
    assert.match(tutorial, /choice\.relation !== "allied" && Number\(choice\.gold_cost \|\| 0\) === 0/);
    assert.match(tutorial, /choiceId = answer\.id/);
    assert.match(campaignView, /gameplay\/currency\/gold\.webp/);
    assert.match(campaignView, /sow-story__choice-price.*is-insufficient/);
    assert.doesNotMatch(campaignView, /button\.disabled = Boolean\(choice\.disabled\)/);
    assert.match(campaignMapEditorHtml, /id="fgoldloot"/);
    assert.match(campaignMapEditorHtml, /gold_loot_bonus:Number\(f\.gold_loot_bonus\)/);
    assert.ok(roster.factions.filter(faction => faction.civ === "Roman Empire").every(faction => faction.relation === "enemy"));
    assert.ok(roster.factions.filter(faction => ["stonea", "snettisham", "thetford"].includes(faction.id)).every(faction => faction.relation === "neutral"));
    assert.match(campaignEditor, /Factions to contact/);
    assert.match(campaignEditor, /isContact \? "Contact · " : "First support · "\) \+ reaction\.id/);
    assert.match(campaignMapEditorHtml, /factionColor\(f\).*ALLY_COLOR.*ENEMY_COLOR/s);
    assert.match(campaignMapEditorHtml, /ctx\.fillStyle = roster\.player_color \|\| PLAYER_COLOR/);
    assert.doesNotMatch(campaignMapEditorHtml, /Alliance betrayal|BETRAYALS|ROLES/);
    assert.match(campaignMapEditorHtml, /id="fhostility"/);
    assert.match(campaignMapEditorHtml, /hostility:f\.hostility/);
    assert.match(campaignMapEditorHtml, /id="fcanrequest"/);
    assert.match(campaignMapEditorHtml, /Allow AI requests and generic neutral terms/);
    assert.equal(step("boudica_outpost_colonia_veterans").next, "boudica_outpost_tax_collectors");
    assert.equal(step("boudica_outpost_tax_collectors").next, "boudica_outpost_roman_supply_depot");
    assert.equal(step("boudica_outpost_roman_supply_depot").next, "boudica_trinovantes_alliance");
    assert.equal(step("boudica_ratio_guide").next, "boudica_upgrade_choice");
    assert.equal(step("boudica_trinovantes_alliance").next, "boudica_ally_support_wait");
    assert.match(definition.strings.en["tutorial.boudica_upgrade_choice_body"], /Three Roman posts are down/);
    assert.match(definition.strings.es["tutorial.boudica_upgrade_choice_body"], /Tres puestos romanos cayeron/);
    assert.equal(step("boudica_structure_upgrade").trigger.type, "city_level");
    assert.equal(step("boudica_structure_upgrade").trigger.value, 3);
    assert.deepEqual(step("boudica_pact_offer").trigger, { type: "contact", target: "catuvellauni", value: 1, scope: "total" });
    assert.equal(step("boudica_pact_offer").next, "boudica_port_choice");
    assert.ok(step("boudica_factory_choice").choices.some(choice => choice.id === "foundry"));
    assert.deepEqual(step("boudica_foundry_select_menu").trigger, { type: "ui", action: "map_build", scope: "step" });
    assert.deepEqual(step("boudica_foundry_upgrade").trigger, { type: "foundry_level", value: 1, scope: "step" });
    assert.equal(step("boudica_foundry_upgrade").guide.target, "map_upgrade_foundry");
    assert.match(mapClick, /self\.sim\.config\.tutorial && building\.kind == sow_core::game::BuildingKind::City[\s\S]*?actions\.push\(MapMenuAction::UpgradeFoundry\)/);
    assert.match(mapClick, /if self\.sim\.config\.tutorial \{\s*actions\.push\(MapMenuAction::UpgradeTile\)/);
    assert.equal(step("boudica_foundry_upgrade").next, "boudica_bunker_choice");
    assert.equal(step("boudica_port_upgrade").trigger.type, "port_upgrade");
    assert.equal(step("boudica_port_ready").trigger.type, "port_level");
    assert.equal(step("boudica_port_ready").next, "boudica_fleet_target");
    assert.equal(step("boudica_fleet_target").trigger.type, "fleet");
    assert.deepEqual(step("boudica_fleet_target").trigger, { type: "fleet", unit: "TransportShip", target: "legio_xiv_gemina", value: 1, scope: "step" });
    assert.equal(step("boudica_fleet_target").guide.target, "player");
    assert.equal(step("boudica_fleet_target").next, "boudica_ship_choice");
    assert.match(definition.strings.en["tutorial.boudica_ship_choice_body"], /The sea is open/);
    assert.match(definition.strings.es["tutorial.boudica_ship_choice_body"], /El mar está abierto/);
    assert.equal(step("boudica_ship_choice").choices.find(choice => choice.id === "march").next, "boudica_final_battle_intro");
    assert.deepEqual(step("boudica_trade_city").trigger, { type: "city_level", value: 4, scope: "total" });
    assert.deepEqual(step("boudica_trade_port").trigger, { type: "port_level", value: 3, scope: "total" });
    assert.equal(step("boudica_trade_menu").trigger.action, "map_build");
    assert.deepEqual(step("boudica_trade_ship").trigger, { type: "fleet", unit: "TradeShip", value: 1, scope: "step" });
    assert.equal(step("boudica_trade_ship").guide.target, "map_build_trade_ship");
    assert.equal(step("boudica_trade_ready").next, "boudica_final_battle");
    assert.match(mapClick, /BuildingKind::Port if building\.level >= 3\s*=>\s*\{\s*actions\.push\(MapMenuAction::BuildTradeShip\);/);
    assert.match(campaignEngine, /map_build_trade_ship: '#sow-hud \[data-map-action="build_trade_ship"\]'/);
    assert.equal(step("boudica_camulodunum").next, "boudica_ninth_legion_intro");
    assert.equal(step("boudica_ninth_legion").next, "boudica_londinium_intro");
    assert.equal(step("boudica_londinium").next, "boudica_verulamium_intro");
    assert.equal(step("boudica_verulamium").next, "boudica_pact_choice");
    for (const id of ["camulodunum", "londinium", "verulamium"]) {
        const faction = roster.factions.find(item => item.id === id);
        assert.equal(faction.name, id[0].toUpperCase() + id.slice(1));
        assert.equal(faction.relation, "enemy");
        assert.equal(faction.hostility, "aggressive");
        assert.equal(faction.can_request_alliance, false);
    }
    assert.equal(roster.factions.find(faction => faction.name === "Catuvellauni").iq, 60);
    assert.match(campaignEditor, /world:player/);
    assert.match(campaignEngine, /upgrade_structure: "#sow-hud-building-card-upgrade"/);
    assert.match(campaignEditorHtml, /id="sow-hud-building-card-upgrade"/);
    assert.match(tutorial, /if \(runtime\.definition\.menu_guide\) pendingMenuGuide = \{ episodeId: runtime\.episodeId \}/);
    const rewardGate = shellSource.indexOf("!rewardPresentationReady || rewardAnimationRunning) return;");
    const guideReadyEvent = shellSource.indexOf('dispatchEvent(new CustomEvent("sow:campaign-menu-guide-ready"))');
    assert.ok(rewardGate >= 0 && guideReadyEvent > rewardGate, "return guide must wait for reward presentation");
    for (const locale of ["en", "es"]) {
        assert.ok(definition.strings[locale]["tutorial.boudica_menu_lobby_hint"]);
        assert.ok(definition.strings[locale]["tutorial.boudica_menu_campaign_hint"]);
        assert.ok(definition.strings[locale]["tutorial.boudica_menu_replay_hint"]);
        assert.ok(definition.strings[locale]["tutorial.boudica_menu_multiplayer_hint"]);
        assert.ok(definition.strings[locale]["tutorial.boudica_port_ready_hint"]);
        assert.ok(definition.strings[locale]["tutorial.boudica_trade_ship_hint"]);
    }
    const machine = campaign.create(definition, definition.menu_guide.entry);
    machine.update({}, {}, 0);
    assert.equal(machine.view().step.id, "boudica_return_lobby");
    machine.update({}, { menu_lobby: 1 }, 1);
    assert.equal(machine.view().step.id, "boudica_return_end");
    assert.equal(machine.view().step.type, "end");
    assert.equal(machine.view().done, false);
});

// Owner decision: the return guide in the main menu is the hand only. The
// objective card had no close button, and the closing "You're ready" panel was
// the same nuisance, so both stay hidden until the tour is redesigned.
test("the main menu guide points the hand at a lobby and never shows a card", () => {
    assert.match(campaignEngine, /menu_lobby: '#sow-menu \.sow-menu__home-public \[data-lobby-card\]'/);
    assert.match(tutorial, /if \(machineView\.done \|\| machineView\.step\.type === "end"\) \{ dismissMenuGuide\(\); return; \}/);
    assert.match(tutorial, /context\.hideObjective = true/);
    // The card stays visible by default (gameplay) and only the menu guide turns it off.
    assert.match(campaignView, /dialog\.hidden = !modal; shade\.hidden = !modal; objective\.hidden = false;/);
    assert.match(campaignView, /if \(context\.hideObjective\) objective\.hidden = true;/);
    assert.doesNotMatch(campaignView, /objective\.hidden = modal/);
});

// Owner decision: episode 1 ("Arrival") is not finished yet, so the whole Six Sky
// chain stays locked and the campaign screen only lets players replay the tutorial.
test("campaign episode 1 stays locked so only the tutorial can be replayed", () => {
    const unlockStart = campaignModule.indexOf("pub fn is_unlocked(");
    const unlock = campaignModule.slice(unlockStart, campaignModule.indexOf("pub fn advisor(", unlockStart));
    assert.ok(unlockStart >= 0, "is_unlocked must exist");
    assert.match(unlock, /CampaignId::SixSkyEp1 => false/);
    assert.doesNotMatch(unlock, /SixSkyEp1 => CampaignId::Boudica\.is_completed/);
    // A locked episode renders the same disabled card as the rest of the chain.
    assert.match(lobbiesSource, /: "<button class='sow-menu__secondary sow-campaign__play' type='button' disabled>"/);
    // "Saga complete" may only appear when every episode is really finished.
    assert.match(lobbiesSource, /episodes\.every\(function \(episode\) \{ return episode && episode\.completed; \}\)/);
});

test("tutorial locale and UI guides resolve per episode and point to actual HUD controls", () => {
    assert.match(tutorial, /strings\[locale\]/);
    assert.match(tutorial, /SOWCampaign\.resolveUiTarget\(guide\.target, document, runtime\.episodeId\)/);
    assert.match(tutorial, /tutorial\[guide\.target\]/);
    assert.match(campaignView, /sow-story__gesture/);
    assert.match(campaignView, /sow-story__gesture-label/);
    assert.match(campaignView, /sow-story__zoom-fingers/);
    assert.match(campaignView, /const modal = \["scene", "choice", "end"\]\.includes\(step\.type\)/);
    assert.match(storyCss, /\.sow-story__gesture-label \{[^}]*text-transform: uppercase/);
    assert.match(storyCss, /data-gesture="zoom_in"/);
    assert.match(storyCss, /data-gesture="zoom_out"/);
    assert.match(storyCss, /\.sow-story__zoom \{[^}]*width: 108px; height: 108px/);
    assert.match(storyCss, /data-gesture="zoom_out"[^\n]*story-left-together/);
    assert.match(storyCss, /data-gesture="zoom_out"[^\n]*story-right-together/);
    assert.match(storyCss, /data-gesture="pan_keys"/);
    assert.match(campaignEditor, /var zoomMode = \$\("#device"\)\.value === "mobile" \? "pinch" : "wheel"/);
    assert.match(campaignEngine, /menu_campaign:/);
    assert.match(campaignEngine, /map_attack:/);
    assert.match(tutorial, /addEventListener\("resize", redrawCampaign/);
    assert.match(tutorial, /var actionEvent = control && control\.matches\("input"\) \? "change" : "click"/);
});

test("campaign studio preview matches the real build submenu and direct nuke action", () => {
    const targetBlock = campaignEngine.match(/const UI_TARGETS = \{([\s\S]*?)\n    \};/);
    assert.ok(targetBlock);
    const selectors = [...targetBlock[1].matchAll(/:\s*'([^']+)'/g)].flatMap((match) => match[1].split(","));
    for (const selector of selectors) {
        const target = selector.trim().replace(/^#sow-(?:hud|menu)\s*/, "");
        const attributes = [...target.matchAll(/\[([^\]]+)\]/g)].map((match) => match[1]);
        const exists = target.startsWith("#")
            ? campaignEditorHtml.includes(`id="${target.slice(1)}"`)
            : attributes.length
                ? attributes.every((attribute) => campaignEditorHtml.includes(attribute))
                : campaignEditorHtml.includes(target);
        assert.ok(exists, `Campaign studio preview is missing ${selector.trim()}`);
    }
    assert.match(campaignEditorHtml, /data-preview-map-menu="build"[\s\S]*?data-map-action="build_city"/);
    assert.match(campaignEditorHtml, /<button[^>]*data-map-action="nuke"/);
    assert.doesNotMatch(campaignEditorHtml, /data-preview-map-menu="nuke"|data-map-group="nuke"/);
    assert.match(campaignEditorCss, /\.sample-menu\{[^}]*z-index:3/);
    assert.match(campaignEditorCss, /#previewRoot\s*\{[^}]*z-index:\s*4/);
    assert.match(hud, /else if \(nukeItem\) \{\s*mapSector\(radial, nukeItem, 3, radialCount, "nuke"\)/);
    assert.doesNotMatch(hud, /mapMenuView === "nuke"|mapGroupSector\([^;]*"nuke"/);
    assert.match(campaignEngine, /map_nuke: '#sow-hud \[data-map-action="nuke"\]'/);
    assert.equal([...campaignEditorHtml.matchAll(/data-map-back/g)].length, 1);
    assert.doesNotMatch(campaignEditor, /syncPreviewMapMenu/);
    assert.match(campaignEditor, /else if \(group\) hudRoot\.dataset\.previewMapMenu = group\.dataset\.mapGroup;[\s\S]*?paintPreview\(\);/);
    assert.match(campaignEditor, /var menuHint = requiredMenu && \$\("#sow-hud"\)\.dataset\.previewMapMenu !== requiredMenu/);
    assert.match(campaignEditor, /function previewAnchor\(step\)[\s\S]*?target\.getClientRects\(\)\.length/);
});

test("campaign menu-guide preview opens Campaign before exposing the replay target", () => {
    assert.match(campaignEditorHtml, /data-preview-menu-screen="home"[\s\S]*?data-command="open_campaign"/);
    assert.match(campaignEditorHtml, /data-preview-menu-screen="campaign"[\s\S]*?data-preview-menu-back[\s\S]*?data-preview-episode-title/);
    assert.match(campaignEditor, /function setPreviewMenuScreen\(screen\)[\s\S]*?panel\.hidden = panel\.dataset\.previewMenuScreen !== screen/);
    assert.match(campaignEditor, /closest\('\[data-command="open_campaign"\]'\)\) setPreviewMenuScreen\("campaign"\)/);
    assert.match(campaignEditor, /replayButton\.dataset\.episodeId = episodeId/);
    assert.match(campaignEditor, /guideTarget === "campaign_replay"[\s\S]*?open Campaign in the preview to reveal Replay/);
    assert.match(campaignEditor, /resolveUiTarget\("campaign_replay", document, episodeId\)/);
    assert.match(lobbiesSource, /data-command='close_campaign'/);
});

test("campaign replay hand resolves the selected episode card, not always Boudica", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const boudica = { dataset: { episodeId: "boudica" } };
    const sixSky = { dataset: { episodeId: "six_sky_ep2" } };
    const root = { querySelectorAll: () => [boudica, sixSky] };
    assert.equal(campaign.resolveUiTarget("campaign_replay", root, "six_sky_ep2"), sixSky);
    assert.equal(campaign.resolveUiTarget("campaign_replay", root, "unknown_episode"), null);
    assert.equal(campaign.resolveUiTarget("campaign_replay", root), null);
    assert.match(tutorial, /resolveUiTarget\(guide\.target, document, runtime\.episodeId\)/);
    assert.doesNotMatch(campaignEngine, /data-episode-id="boudica"/);
});

test("tutorial hand selectors stay tied to controls rendered by the game", () => {
    const targetBlock = campaignEngine.match(/const UI_TARGETS = \{([\s\S]*?)\n    \};/);
    assert.ok(targetBlock);
    const sources = [shellSource, lobbiesSource, coreSource, hud].join("\n");
    for (const [, selector] of targetBlock[1].matchAll(/:\s*'([^']+)'/g)) {
        for (const [, attribute, value] of selector.matchAll(/\[([a-z-]+)="([^"]+)"\]/g)) {
            if (attribute === "data-map-action") {
                assert.match(hud, /dataset\.mapAction = action/);
                assert.ok(hud.includes(value), `No game map action ${value}`);
            } else if (attribute === "data-map-group") {
                assert.match(hud, /dataset\.mapGroup = group/);
                assert.ok(hud.includes(value), `No game map group ${value}`);
            } else if (attribute === "data-episode-id") {
                assert.match(lobbiesSource, /data-episode-id=.*episode\.id/);
                assert.match(campaignModule, new RegExp('Boudica => "' + value + '"'));
            } else {
                assert.ok(sources.includes(`${attribute}='${value}'`) || sources.includes(`${attribute}="${value}"`) || sources.includes(value), `No game UI target ${attribute}=${value}`);
            }
        }
        if (selector === "#sow-hud-slider") assert.match(hud, /id="sow-hud-slider"/);
    }
    assert.match(hud, /data-map-back/);
});

test("tutorial X is not wired to leave match and menu exit clears runtime", () => {
    assert.match(tutorial, /data-command=.*prompt_surrender/);
    assert.match(tutorial, /function update\(hud\) \{\s*if \(!runtime\.machine \|\| runtime\.modalOpen \|\|/);
    assert.match(tutorial, /runtime\.machine\.setPaused\(open\)/);
    assert.match(tutorial, /onDismiss: runtime\.menuGuide[\s\S]*?: null/);
    assert.match(tutorial, /SOW_tutorial_menu_state_update/);
    assert.match(tutorial, /function reset\(\)/);
    assert.match(hud, /id="sow-hud-surrender-modal"/);
    assert.match(hudCss, /\.sow-campaign-error/);
    assert.doesNotMatch(hudCss, /sow-hud__tutorial-overlay|sow-hud__tutorial-hand/);
});

test("map menu sends the Rust-validated session, tile, and action", () => {
    assert.match(hud, /button\.dataset\.mapAction = action/);
    assert.match(hud, /send\("map_menu_action", \{/);
    assert.match(hud, /session: Number\(mapMenu\.dataset\.session\)/);
    assert.match(hud, /tile_idx: Number\(mapMenu\.dataset\.tileIdx\)/);
    assert.match(hud, /action: mapButton\.dataset\.mapAction/);
    assert.match(hud, /menu\.dataset\.renderKey/);
    assert.match(hud, /Math\.min\(Math\.max\(x, minX\), maxX\)/);
    assert.match(hudCss, /\.sow-hud__map-menu\.hidden\s*\{\s*display: none/);
});

test("building mode upgrades same-kind targets, marks them, and has a cancel control", () => {
    const buildStart = mapClick.indexOf("    fn build_structure_at(");
    const buildEnd = mapClick.indexOf("    fn city_module_is_available", buildStart);
    const buildBody = mapClick.slice(buildStart, buildEnd);
    assert.ok(buildStart >= 0 && buildEnd > buildStart);
    assert.match(buildBody, /find_stack_target_tile\([\s\S]*?snapshot\.buildings/);
    assert.match(buildBody, /if let Some\(building\) = stack_target \{[\s\S]*?cancel_hold_build\(\)[\s\S]*?GameplayIntent::UpgradeStructure \{[\s\S]*?building_id: building\.id/);
    assert.match(buildBody, /building\.under_construction[\s\S]*?add_action_feedback\("Building under construction\./);
    assert.match(mapClick, /"Building under construction\. 🏗️" => UiText::new\("hud\.building_in_progress"\)/);
    assert.match(buildBody, /building\.level >= kind\.max_level\(\)[\s\S]*?building_max_level/);
    assert.match(buildBody, /!self\.structure_upgrade_requirement_met\(&building\)[\s\S]*?building_requires_city_level/);
    assert.match(buildBody, /return true;\s*\}\s*let Some\(snapshot\)/);
    assert.match(frameSource, /nobuild_slots\[slot\]\s*=\s*\[[\s\S]*?0\.0,[\s\S]*?2\.0/);
    assert.match(mapShader, /active_flag > 1\.0 && b_dist == 0[\s\S]*?is_upgrade_target = true/);
    assert.match(mapShader, /overlay_color = vec3<f32>\(1\.0, 0\.72, 0\.16\)/);
    assert.match(hud, /data-command="cancel_building_mode"/);
    assert.match(hud, /if \(selectedBuilding\) send\("select_building", \{ kind: selectedBuilding \}\)/);
    assert.match(hud, /buildingCancel\.hidden = !hud\.selected_building/);
    assert.match(hudCss, /\.sow-hud__building-cancel[\s\S]*?width: 48px;[\s\S]*?height: 48px;/);
    const dockStart = hud.indexOf('<footer class="sow-hud__dock"');
    const dockEnd = hud.indexOf('</footer>', dockStart);
    assert.ok(dockStart >= 0 && dockEnd > dockStart);
    assert.ok(!hud.slice(dockStart, dockEnd).includes('data-command="cancel_building_mode"'));
    assert.ok(hud.slice(dockEnd).includes('data-command="cancel_building_mode"'));
    assert.match(mapClick, /\*selected != Some\(kind\)\)\.then_some\(kind\)/);
    assert.match(buildingOverlaySource, /celebration_budget = 24/);
    assert.match(buildingOverlaySource, /!reduced_motion[\s\S]*?celebration_budget >= BUILDING_CELEBRATION_OFFSETS\.len\(\)/);
});

test("building upgrade uses snapshot gold and keeps the card open for the server result", () => {
    assert.match(simUpdateSource, /pub\(crate\) fn current_player_gold\(&self\)[\s\S]*?player\.gold[\s\S]*?unwrap_or\(self\.ui\.app\.hud_state\.gold\)/);
    assert.match(mapClick, /let gold = self\.current_player_gold\(\);[\s\S]*?if !cost\.is_finite\(\) \|\| gold < cost/);
    assert.ok(webMenu.includes("let has_gold = app.current_player_gold() >= cost"));
    assert.match(hud, /renderBuildingCard\(hud\.map_menu, hud\.gold\)/);
    assert.match(hud, /id="sow-hud-building-card-upgrade" data-map-action="upgrade_structure"/);
    assert.ok(mapClick.includes("let keep_building_card_open = action == MapMenuAction::UpgradeStructure"));
    assert.ok(mapClick.includes("if !keep_building_card_open {"));
});

test("building upgrade sends the selected card identity instead of the radial menu", () => {
    const start = hud.indexOf('hudRoot.addEventListener("click", function (event) {');
    const end = hud.indexOf('            var btn = event.target.closest("[data-command]");', start);
    const calls = [];
    const card = { dataset: { session: "12", tileIdx: "240" } };
    const button = {
        dataset: { mapAction: "upgrade_structure" }, disabled: false,
        getAttribute() { return null; },
        closest(selector) { return selector === ".sow-hud__building-card" ? card : null; }
    };
    const click = vm.runInNewContext("(" + hud.slice(start + 'hudRoot.addEventListener("click", '.length, end) + "})", {
        hudRefs: { mapMenu: { dataset: { session: "3", tileIdx: "99" } } },
        send: (command, payload) => calls.push({ command, payload })
    });
    click({ target: { closest: selector => selector === "[data-map-action]" ? button : null }, preventDefault() {}, stopPropagation() {} });
    assert.equal(calls.length, 1);
    assert.equal(calls[0].payload.session, 12);
    assert.equal(calls[0].payload.tile_idx, 240);
    assert.equal(calls[0].payload.action, "upgrade_structure");
});

test("building card anchors to the building and shows compact visual stats", () => {
    const renderStart = hud.indexOf("    function escapeHudText(value) {");
    const renderEnd = hud.indexOf("\n    function updateLeaderboard", renderStart);
    assert.ok(renderStart >= 0 && renderEnd > renderStart);

    const element = () => {
        const classes = new Set();
        return {
            dataset: {},
            style: {},
            innerHTML: "",
            textContent: "",
            disabled: false,
            hidden: false,
            title: "",
            offsetWidth: 180,
            offsetHeight: 120,
            classes,
            attributes: {},
            setAttribute(name, value) { this.attributes[name] = value; },
            classList: {
                toggle(name, force) {
                    if (force) classes.add(name);
                    else classes.delete(name);
                }
            }
        };
    };
    const hudRefs = {
        buildingCard: element(),
        buildingCardIcon: element(),
        buildingCardKind: element(),
        buildingCardLevel: element(),
        buildingCardBenefit: element(),
        buildingCardNext: element(),
        buildingCardGold: element(),
        buildingCardUpgrade: element(),
        buildingCardUpgradeLevel: element()
    };
    const renderBuildingCard = vm.runInNewContext(
        hud.slice(renderStart, renderEnd) + "\nrenderBuildingCard;",
        {
            hudRefs,
            window: { innerWidth: 400, innerHeight: 300 },
            SOW_t: key => key === "hud.building_requires_city_level" ? "Requires a level 3 City." : key,
            hudIcon: (name) => "<svg data-icon=\"" + name + "\"></svg>",
            buildingIcon: (kind) => "<img data-building=\"" + kind + "\">"
        }
    );
    const menu = (building, view = "building_details") => ({
        open: true, view, session: 4, tile_idx: 12, x: 90, y: 100, building
    });

    renderBuildingCard(menu({
        name: "Bunker",
        kind: "Bunker",
        level: 0,
        under_construction: true,
        remaining_seconds: 7.4,
        can_upgrade: false
    }));
    assert.match(hudRefs.buildingCardLevel.innerHTML, /7\.4s/);
    assert.doesNotMatch(hudRefs.buildingCardLevel.innerHTML, /Lv 0/);
    assert.equal(hudRefs.buildingCardKind.textContent, "Bunker");
    assert.equal(hudRefs.buildingCardBenefit.hidden, true);
    assert.equal(hudRefs.buildingCardNext.hidden, true);
    assert.equal(hudRefs.buildingCardUpgrade.hidden, true);
    assert.equal(hudRefs.buildingCardUpgrade.disabled, true);

    renderBuildingCard(menu({
        name: "Bastion",
        kind: "Bunker",
        level: 2,
        benefit_label: "Raises enemy losses by 10% within range 16.",
        metrics: [
            { icon: "defense", label: "Enemy attack losses", value: 10, prefix: "+", unit: "%" },
            { icon: "range", label: "Defense range", value: 16 }
        ],
        next_level: 3,
        next_benefit_label: "Raises enemy losses by 15% within range 18.",
        next_metrics: [
            { icon: "defense", label: "Enemy attack losses", value: 15, prefix: "+", unit: "%" },
            { icon: "range", label: "Defense range", value: 18 }
        ],
        duration_seconds: 6,
        cost: 150,
        owns: true,
        can_upgrade: true
    }), 120);
    assert.match(hudRefs.buildingCardLevel.innerHTML, /Lv 2/);
    assert.equal(hudRefs.buildingCardBenefit.hidden, true);
    assert.doesNotMatch(hudRefs.buildingCardNext.innerHTML, /Lv 3|15%|18/);
    assert.match(hudRefs.buildingCardNext.innerHTML, /5%/);
    assert.match(hudRefs.buildingCardNext.innerHTML, /2/);
    assert.match(hudRefs.buildingCardNext.innerHTML, /6s/);
    assert.match(hudRefs.buildingCardGold.innerHTML, /120/);
    assert.match(hudRefs.buildingCardGold.innerHTML, /150/);
    assert.equal(hudRefs.buildingCardGold.classes.has("is-short"), true);
    assert.equal(hudRefs.buildingCardUpgrade.attributes["aria-label"], "Upgrade to level 3");
    assert.equal(hudRefs.buildingCardUpgrade.hidden, false);
    assert.equal(hudRefs.buildingCardUpgrade.disabled, true);
    renderBuildingCard(menu({
        kind: "Bunker",
        level: 2,
        next_level: 3,
        metrics: [{ icon: "defense", label: "Enemy attack losses", value: 10, prefix: "+", unit: "%" }],
        next_metrics: [{ icon: "defense", label: "Enemy attack losses", value: 15, prefix: "+", unit: "%" }],
        cost: 150,
        can_upgrade: true
    }), 200);
    assert.equal(hudRefs.buildingCardGold.classes.has("is-short"), false);
    assert.equal(hudRefs.buildingCardUpgrade.disabled, false);
    assert.equal(hudRefs.buildingCard.style.left, "104px");
    assert.equal(hudRefs.buildingCard.style.top, "40px");

    renderBuildingCard(menu({
        kind: "Factory",
        level: 1,
        metrics: [{ icon: "gold", label: "Gold income", value: 10, prefix: "+", unit: "/s" }],
        next_level: 2,
        next_metrics: [{ icon: "gold", label: "Gold income", value: 20, prefix: "+", unit: "/s" }],
        requirements: [{ key: "hud.building_requires_city_level", met: false }],
        cost: 50,
        can_upgrade: false
    }), 200);
    assert.match(hudRefs.buildingCardNext.innerHTML, /title="Requires a level 3 City\."/);
    assert.equal(hudRefs.buildingCardUpgrade.disabled, true);

    renderBuildingCard(menu({ kind: "Bunker", level: 2 }, "radial"));
    assert.equal(hudRefs.buildingCard.classes.has("hidden"), true);
    renderBuildingCard(null);
    assert.equal(hudRefs.buildingCard.classes.has("hidden"), true);
});

test("primary click selects owned buildings without changing other map gestures", () => {
    const clickStart = mapClick.indexOf("pub(crate) fn handle_map_click");
    const clickEnd = mapClick.indexOf("pub(crate) fn open_map_context_menu", clickStart);
    const clickBody = mapClick.slice(clickStart, clickEnd);
    assert.ok(clickBody.indexOf("selected_nuke_kind") < clickBody.indexOf("select_owned_building"));
    assert.ok(clickBody.indexOf("selected_building_kind") < clickBody.indexOf("select_owned_building"));
    assert.ok(clickBody.indexOf("select_warships_at") < clickBody.indexOf("select_owned_building"));
    assert.match(clickBody, /select_owned_building\(x, y\)[\s\S]*?primary_target\(tile_idx/);
    assert.match(mapClick, /building_at_pointer\(/);
    assert.match(mapClick, /target\.owner != target\.my_id/);
    assert.match(buildingOverlaySource, /pub\(crate\) fn building_at_pointer/);
    assert.match(buildingOverlaySource, /cached_buildings\(/);
    assert.match(buildingOverlaySource, /if building\.owner_id != my_id/);
    assert.match(buildingOverlaySource, /building\.kind\.footprint_dimensions\(\)/);
    assert.match(buildingOverlaySource, /pointer_world_x - building\.bx\).*width as f32 \* 0\.5/);
    assert.match(buildingOverlaySource, /pointer_world_y - building\.by\).*height as f32 \* 0\.5/);
    assert.match(buildingOverlaySource, /nearest_building_in_cluster/);
    assert.match(buildingOverlaySource, /building_marker_size\(building, zoom_scaled\)/);
    assert.match(appStateSource, /pub enum MapContextMenuView\s*\{\s*BuildingDetails,\s*Radial/);
    assert.match(mapClick, /MapContextMenuView::BuildingDetails/);
    assert.match(mapClick, /MapContextMenuView::Radial/);
    assert.match(windowInput, /if is_quick_tap\(elapsed_ms, distance_sq\)\s*\{\s*self\.handle_map_click/);
    assert.match(windowInput, /else if right[\s\S]*?self\.open_map_context_menu\(x, y\)/);
    assert.match(windowInput, /pub\(crate\) fn poll_pointer_hold[\s\S]*?self\.open_map_context_menu\(x, y\)/);
    assert.match(webMenu, /"view": match menu\.view[\s\S]*?"building_details"[\s\S]*?"radial"/);
    assert.match(hud, /var showRadial = Boolean\(open && mapMenu\.view === "radial"\)/);
    assert.match(hud, /mapMenu\.view === "building_details" && detail/);
    assert.match(hud, /send\("close_map_context_menu"\)/);
    assert.match(webMenu, /WebMenuCommand::CloseMapContextMenu => self\.close_map_context_menu\(\)/);
    assert.match(mapClick, /pub\(crate\) fn close_map_context_menu\(&mut self\)\s*\{\s*self\.input\.map_context_menu = None;/);
    assert.match(webMenu, /map_menu_view: map_menu\.map\(\|menu\| menu\.view\)/);

    const targetStart = mapClick.indexOf("fn primary_target(&mut self");
    const targetEnd = mapClick.indexOf("fn attack_from_tile", targetStart);
    const targetBody = mapClick.slice(targetStart, targetEnd);
    assert.match(targetBody, /target\.owner == target\.my_id\s*\{\s*return;/);
    assert.match(targetBody, /if target\.is_friendly\(\)\s*\{\s*self\.open_transfer_from_tile\(tile_idx\);\s*\} else \{\s*self\.attack_from_tile\(tile_idx\);/);

    const actionStart = mapClick.indexOf("pub(crate) fn handle_map_menu_action");
    const actionEnd = mapClick.indexOf("fn map_menu_cost", actionStart);
    const actionBody = mapClick.slice(actionStart, actionEnd);
    assert.match(actionBody, /menu\.session != session \|\| menu\.tile_idx != tile_idx/);
    assert.match(actionBody, /!self\.map_menu_actions\(tile_idx\)\.contains\(&action\)/);
});

test("WASM right-click opens on pointer press; touch hold stays intact", () => {
    assert.match(windowInput, /let right = matches!\([\s\S]*?MouseButton::Right[\s\S]*?\);/);
    const rightClickStart = windowInput.indexOf("} else if right");
    const rightClickEnd = windowInput.indexOf("\n        }\n    }\n\n    fn handle_pointer_move", rightClickStart);
    assert.notEqual(rightClickStart, -1);
    assert.notEqual(rightClickEnd, -1);
    const rightClickBody = windowInput.slice(rightClickStart, rightClickEnd);
    assert.match(rightClickBody, /self\.ui\.app\.phase == ClientPhase::Playing/);
    assert.match(rightClickBody, /selected_building_kind\.is_some\(\)[\s\S]*selected_nuke_kind\.is_some\(\)[\s\S]*self\.clear_placement\(\)/);
    assert.match(rightClickBody, /else if !self\.move_selected_warships\(x, y\) \{\s*self\.open_map_context_menu\(x, y\);/);
    assert.match(rightClickBody, /else \{\s*self\.close_map_context_menu\(\);/);
    assert.match(windowInput, /pub\(crate\) fn poll_pointer_hold[\s\S]*?self\.open_map_context_menu\(x, y\);/);
    assert.match(indexTemplate, /<canvas id="blade" oncontextmenu="return false;"/);
    assert.doesNotMatch(hud, /addEventListener\("contextmenu"/);
    assert.doesNotMatch(webMenu, /OpenMapContextMenu/);
    assert.doesNotMatch(mapClick, /handle_secondary_click/);
});

test("map menu stays visible with disabled sectors when no action is available", () => {
    const openStart = mapClick.indexOf("pub(crate) fn open_map_context_menu");
    const openEnd = mapClick.indexOf("pub(crate) fn close_map_context_menu", openStart);
    const openBody = mapClick.slice(openStart, openEnd);
    assert.match(openBody, /if self\.map_menu_actions\(tile_idx\)\.is_empty\(\) \{\s*self\.show_map_menu_unavailable\(tile_idx\);\s*\}/);
    assert.match(openBody, /self\.input\.map_context_menu = Some\(MapContextMenu/);

    const renderStart = hud.indexOf("function renderMapMenu(mapMenu)");
    const renderEnd = hud.indexOf("function updateLeaderboard", renderStart);
    const renderBody = hud.slice(renderStart, renderEnd);
    assert.doesNotMatch(renderBody, /if \(!items\.length\)/);
    assert.match(renderBody, /mapDisabledSector\(radial, 1, radialCount, "fleet"/);
});

test("map menu keeps the radial sectors and recovered submenu actions", () => {
    assert.match(hud, /sow-hud__map-radial/);
    assert.match(hud, /function radialSectorGeometry/);
    assert.match(hud, /function mapDisabledSector/);
    assert.match(hud, /var radialCount = 4/);
    assert.match(hud, /mapSector\(radial, transfer, 0, radialCount/);
    assert.match(hud, /mapSector\(radial, fleet, 1, radialCount/);
    assert.match(hud, /mapSector\(radial, alliance, 2, radialCount/);
    assert.match(hud, /dataset\.mapGroup = group/);
    assert.match(hud, /upgrade_arsenal/);
    assert.match(hud, /build_warship/);
    assert.match(hud, /build_trade_ship/);
    assert.match(hudCss, /\.sow-hud__map-sector--build/);
    assert.match(hudCss, /\.sow-hud__map-sector--nuke/);
    assert.match(hudCss, /isolation: isolate/);
    assert.match(hudCss, /\.sow-hud__map-submenu/);
    assert.match(hudCss, /@keyframes sow-map-menu-in/);
    assert.match(hud, /title\.innerHTML = hudIcon\(label\.icon/);
    assert.match(hud, /parts\.push\(item\.level/);
    assert.match(hud, /sow-hud__buildings-strip/);
    assert.match(hudCss, /\.sow-hud__buildings-strip/);
    assert.match(hudCss, /\.sow-hud__building-btn/);
    assert.doesNotMatch(hud, /build_structure|cancel_placement/);
    assert.doesNotMatch(hudCss, /sow-hud__bld-btn|sow-hud__cancel-btn/);
    assert.doesNotMatch(hud, /mapClose|data-map-close|close_map_menu|STRATEGIC STRIKE|CONSTRUCT/);
    assert.doesNotMatch(hudCss, /sow-hud__map-sector-caption|sow-hud__map-close/);
});

test("building menus use generated pixel-art sprites with emoji fallback", () => {
    const iconStart = hud.indexOf("var BUILDING_EMOJIS = {");
    const iconEnd = hud.indexOf("function leaderById(id)", iconStart);
    const artManifest = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/gameplay/buildings/atlas.json"), "utf8"));
    const buildingIcon = vm.runInNewContext(hud.slice(iconStart, iconEnd) + "\nbuildingIcon;", {
        window: { SOW_BUILDING_ART: artManifest, SOW_ASSETS_URL: "/assets" },
        asset: (name) => "/assets/" + name,
        escapeHudText: (value) => String(value),
    });
    const expected = { City: "🏕️", Factory: "🛠️", Port: "⚓", Bunker: "👁️" };
    for (const [kind, emoji] of Object.entries(expected)) {
        assert.equal(buildingIcon(kind), '<span class="sow-hud__building-icon is-emoji" aria-hidden="true">' + emoji + "</span>");
        assert.ok(buildingOverlaySource.includes('(BuildingKind::' + kind + ', 1) => "' + emoji + '"'));
    }
    for (const level of [1, 2, 3]) {
        const icon = buildingIcon("Farm", level);
        const sprite = artManifest.sprites.Farm[String(level)];
        assert.match(icon, /class="sow-hud__building-icon is-pixel-art"/);
        assert.match(icon, /assets\/gameplay\/buildings\/atlas\.webp/);
        const x = (sprite.x * 100 / (artManifest.atlas_width - artManifest.cell_size)).toFixed(4);
        const y = (sprite.y * 100 / (artManifest.atlas_height - artManifest.cell_size)).toFixed(4);
        assert.ok(icon.includes("background-position:" + x + "% " + y + "%"));
        assert.equal(sprite.width, 64);
        assert.equal(sprite.height, 64);
        assert.match(sprite.sha256, /^[a-f0-9]{64}$/);
    }
    assert.equal(buildingIcon("Farm", 4), '<span class="sow-hud__building-icon is-emoji" aria-hidden="true">🌱</span>');
    assert.match(buildingOverlaySource, /building_sprite_uv/);
    assert.match(hudCss, /\.sow-hud__building-icon\.is-pixel-art[\s\S]*?image-rendering: pixelated/);
    assert.match(hud, /var buildIcon = emojiIcon\("🏗️", "sow-hud__map-action-icon"\)/);
    assert.match(hud, /mapDisabledSector\(radial, 3, radialCount, "build", emojiIcon\("🏗️", "sow-hud__map-action-icon"\)/);
    assert.match(hud, /heading\.innerHTML = emojiIcon\("🏗️", "sow-hud__map-action-icon"\)/);
    assert.match(hud, /var buildingImg = buildingKind \? buildingIcon\(buildingKind\) : ""/);
    assert.match(hud, /buildingIcon\(detail\.kind, detail\.level\)/);
    assert.match(hudCss, /\.sow-hud__building-icon[\s\S]*?font-size: 30px/);
    assert.match(hudCss, /\.sow-hud__map-action-icon\.is-emoji/);

    const kinds = [...hud.matchAll(/data-command="select_building" data-kind="(City|Factory|Port|Bunker|Farm)"/g)]
        .map((match) => match[1]);
    assert.deepEqual(kinds, ["City", "Factory", "Port", "Bunker", "Farm"]);
    assert.match(hud, /send\("select_building", \{ kind: btn\.dataset\.kind \}\)/);
    assert.match(webMenu, /SelectBuilding\s*\{\s*kind: sow_core::game::BuildingKind/);
    assert.match(webMenu, /WebMenuCommand::SelectBuilding\s*\{\s*kind\s*\}\s*=>\s*\{\s*if !self\.ui\.tutorial_camera_only\s*\{\s*self\.select_building_kind\(kind\)/);
    assert.match(windowInput, /fn rebase_single_touch_drag/);
    assert.match(windowInput, /\} else if is_touch \{[\s\S]*?camera_x \+=/);
    assert.doesNotMatch(windowInput, /self\.input\.dragging && !is_touch/);
    assert.match(mapClick, /MapMenuAction::BuildCity[\s\S]*?self\.select_building_kind\(kind\)/);
    assert.doesNotMatch(hud, /cancel_placement/);
});

test("mobile hover is Rust-owned and the HUD only presents its state", () => {
    assert.match(hud, /var hov = hud\.hovered/);
    assert.match(hud, /hoverCard\.classList\.remove\("hidden"\)/);
    assert.match(hud, /hoverCard\.classList\.add\("hidden"\)/);
    assert.doesNotMatch(hud, /addEventListener\("(?:touchstart|touchmove|pointermove)"/);
    assert.match(hud, /hudRefs\.slider\.addEventListener\("pointerdown"/);
});

test("army allocation starts centered and only formats send/keep while shown", () => {
    assert.match(hudStateSource, /attack_ratio: 0\.5/);
    assert.match(hud, /type="range" min="5" max="100" value="50"/);
    assert.doesNotMatch(hud, /allocationHideTimer|setTimeout\([^\n]*allocation/);
    assert.match(hud, /detailsVisible = forceDetails \|\|[\s\S]*?if \(!detailsVisible\) return;[\s\S]*?SOW_t\("hud\.attack_allocation"/);
    assert.match(hud, /pointerup[\s\S]*?remove\("is-adjusting"\)/);
    assert.match(hudCss, /range-vertical::-webkit-slider-runnable-track \{ height: 12px/);
    assert.match(hudCss, /range-vertical::-moz-range-track \{ height: 12px/);
    assert.match(hudCss, /range-vertical::-webkit-slider-thumb/);
    assert.match(hudCss, /range-vertical::-moz-range-thumb/);
    const allocationRail = hud.slice(hud.indexOf("'<aside class=\"sow-hud__left-rail\""), hud.indexOf("'<aside class=\"sow-hud__right-rail\""));
    assert.doesNotMatch(allocationRail, /hudIcon\("troops", "sow-hud__action-icon"\)/);
    assert.match(hud, /slider\.style\.setProperty\("--sow-crossed-swords", 'url\("' \+ asset\(HUD_ICONS\.troops\)/);
    assert.match(hudCss, /background: var\(--sow-gold\) var\(--sow-crossed-swords\) center/);
});

test("campaign players use the active leader name in both map and leaderboard", () => {
    assert.match(matchStartSource, /name: \{\s*if tutorial \{\s*leader\.name\(\)\.to_string\(\)/);
    const leaderboard = hud.slice(hud.indexOf("function updateLeaderboard"), hud.indexOf("function appendPanelRows"));
    assert.match(leaderboard, /var displayName = player\.name \|\| SOW_t\("hud\.player_name"\)/);
    assert.doesNotMatch(leaderboard, /campaignActive|leaderById/);
});

test("tutorial hand follows the selected campaign target", () => {
    assert.match(webMenu, /WebMenuCommand::SetTutorialMarker \{ player_id \} => \{[\s\S]*?tutorial_marker_player_id = player_id/);
    assert.match(webMenu, /tutorial_target_action_tile\([\s\S]*?target_owner/);
    assert.match(tutorial, /send\("set_tutorial_marker", \{ player_id: markerId \}\)/);
    assert.match(tutorial, /Array\.isArray\(step\.trigger\.targets\)/);
    assert.match(campaignView, /const hasMapTarget = step\.marker \|\| \(step\.guide && step\.guide\.kind === "world" && step\.guide\.target === "target_action" && step\.trigger && \(step\.trigger\.target \|\| Array\.isArray\(step\.trigger\.targets\)\)\)/);
    assert.match(campaignEditor, /Array\.isArray\(step\.trigger && step\.trigger\.targets\)/);
    assert.match(campaignView, /data-tutorial-hand/);
    assert.doesNotMatch(tutorial, /tutorial\.markers|data-tutorial-marker|SOW_tutorial_marker_update/);
    assert.doesNotMatch(hudCss, /sow-hud__tutorial-marker/);
});

test("tutorial hand uses the map radial action's exact icon anchor", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.match(hud, /button\.dataset\.tutorialAnchorX = String\(anchorX\)/);
    assert.match(hud, /button\.dataset\.tutorialAnchorY = String\(anchorY\)/);
    assert.match(tutorial, /SOWCampaign\.resolveUiAnchor\(source\)/);
    assert.match(tutorial, /SOWCampaign\.resolveUiAnchor\(destination\)/);
    assert.match(campaignEditor, /SOWCampaign\.resolveUiAnchor\(target\)/);
    assert.match(campaignEditor, /SOWCampaign\.resolveUiAnchor\(end\)/);
    assert.match(campaignEditor, /window\.addEventListener\("resize", function \(\) \{ drawWires\(\); if \(state\.machine\) paintPreview\(false\); \}/);
    const rect = (left, top, width, height) => ({ left, top, width, height });
    const icon = { getClientRects: () => [{}], getBoundingClientRect: () => rect(265, 75, 24, 28) };
    const radial = {
        dataset: { tutorialAnchorX: "75", tutorialAnchorY: "25" },
        getBoundingClientRect: () => rect(100, 50, 200, 100),
        querySelector: (selector) => selector === ".sow-hud__map-action-title" ? icon : null
    };
    assert.deepEqual(campaign.resolveUiAnchor(radial), { x: 250, y: 75, width: 24, height: 28 });
    assert.deepEqual(campaign.resolveUiAnchor({ getBoundingClientRect: () => rect(10, 20, 40, 60) }), { x: 30, y: 50, width: 40, height: 60 });
});

test("tutorial hand uses the optimized pointer and hides after the opening attack", () => {
    const hand = fs.readFileSync(path.join(shell, "../../assets/gameplay/icons/tutorial_hand.webp"));
    assert.equal(hand.subarray(0, 4).toString(), "RIFF");
    assert.equal(hand.subarray(8, 12).toString(), "WEBP");
    assert.match(campaignView, /options\.asset\("gameplay\/icons\/tutorial_hand\.webp"\)/);
    assert.match(storyCss, /\.sow-story__hand \{[^}]*left: 0; top: 0; transform-origin: 0 0/);
    assert.match(tutorial, /step\.id === "boudica_first_victory"[\s\S]*attacks_by_faction_id/);
});
