const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const shell = path.resolve(__dirname, "../shell");
const shellSource = fs.readFileSync(path.join(shell, "main_menu.shell.js"), "utf8");
const portalSource = fs.readFileSync(path.join(shell, "sdk/store_portals.js"), "utf8");
const pokiSource = fs.readFileSync(path.join(shell, "main_menu.poki.js"), "utf8");
const licenseJestSource = fs.readFileSync(path.join(shell, "main_menu.jest.js"), "utf8");
const licenseMenuLayout = fs.readFileSync(path.join(shell, "main_menu.layout.css"), "utf8");
const landingHtml = fs.readFileSync(path.join(shell, "../../sow-web/site/index.html"), "utf8");
const siteDistSource = fs.readFileSync(path.join(shell, "../../sow-dist/src/main.rs"), "utf8");
const readme = fs.readFileSync(path.join(shell, "../../README.md"), "utf8");
const siteHeader = fs.readFileSync(path.join(shell, "../../sow-web/site/site-header.html"), "utf8");
const siteChrome = fs.readFileSync(path.join(shell, "../../sow-web/site/site-chrome.js"), "utf8");
const gameControlsCss = fs.readFileSync(path.join(shell, "sow-controls.css"), "utf8");
const siteApp = fs.readFileSync(path.join(shell, "../../sow-web/site/app.js"), "utf8");
const siteCss = fs.readFileSync(path.join(shell, "../../sow-web/site/styles.css"), "utf8");
const siteFooterPages = ["support", "privacy", "terms", "cookies"].map((page) =>
    fs.readFileSync(path.join(shell, `../../sow-web/site/${page}/index.html`), "utf8")
);
const siteRedirectConfig = fs.readFileSync(path.join(shell, "../../sow-dist/deploy/freebsd/conf.d/shadowsofwar.io.conf"), "utf8");
const indexTemplate = fs.readFileSync(path.join(shell, "index.html.template"), "utf8");

test("public landing builds heroes, maps and videos from game data at publish time", () => {
    assert.match(landingHtml, /<section class="hero" id="top">[\s\S]*<!-- SOW_SITE_HERO_MEDIA -->[\s\S]*href="\/play\/"[\s\S]*href="#game"[\s\S]*<\/section>/);
    for (const marker of ["SOW_SITE_HERO_MEDIA", "SOW_SITE_LEADER_SHOWCASE", "SOW_SITE_LEADERS", "SOW_SITE_MAPS", "SOW_SITE_TRAILER"]) {
        assert.equal(landingHtml.split(`<!-- ${marker} -->`).length, 2, `${marker} must appear once`);
        assert.ok(siteDistSource.includes(`"<!-- ${marker} -->"`), `sow-dist does not fill ${marker}`);
    }
    assert.doesNotMatch(landingHtml, /data-leader-id|1 of 12|twelve/i, "heroes come from the catalog and no count is written by hand");
    assert.match(siteDistSource, /fn landing_leaders\([\s\S]*?sow_core::player::Leader::all\(\)/);
    assert.match(siteDistSource, /fn landing_maps\([\s\S]*?multiplayer_frequency > 0/);
    for (const shot of ["step-expand", "step-build", "step-war"]) {
        assert.ok(landingHtml.includes(`/assets/site/media/${shot}.webp`), `landing does not show ${shot}`);
        assert.match(siteDistSource, new RegExp(`\\(\\s*"${shot}",\\s*\\(`), `${shot} has no provisional art`);
    }
    assert.match(siteApp, /new IntersectionObserver\(/);
    assert.match(siteApp, /document\.visibilityState === 'hidden' \|\| !heroVisible/);
    assert.match(siteApp, /const cards = \$\$\('\.roster-card\[data-leader-id\]'\)/);
    assert.match(siteApp, /element\.setAttribute\('data-i18n', textKey\(leader, field\)\)/);
    assert.match(landingHtml, /id="faq"[\s\S]*class="faq-list"/);
    const updates = landingHtml.slice(landingHtml.indexOf('<div class="footer-updates">'), landingHtml.indexOf('<div class="footer-top">'));
    assert.match(updates, /href="https:\/\/discord\.gg\//);
    assert.match(updates, /href="https:\/\/t\.me\//);
    assert.doesNotMatch(updates, /disabled|type="email"/);
    assert.match(siteCss, /@supports \(animation-timeline: scroll\(\)\) \{\s*@media \(prefers-reduced-motion: no-preference\)/);
    assert.doesNotMatch(siteCss, /gameplay-card|leader-card|proof-grid|platform-card/);
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
        assert.ok(shellSource.includes(`renderSettingsLink("https://shadowsofwar.io${path}", "${key}")`), `${path} is not routed through the shared settings link renderer`);
    }
    assert.ok(shellSource.includes('renderSiteLink("/terms/", "auth.terms", "sow-auth__terms")'));
});


test("menu analytics use the shared event dispatcher instead of a Poki-only click listener", () => {
    assert.match(shellSource, /open_campaign: "menu_campaign_open"[\s\S]*open_browser: "menu_lobby_browser_open"[\s\S]*open_create: "menu_custom_create_open"[\s\S]*confirm_leader: "menu_leader_confirm"/);
    assert.match(shellSource, /window\.SOW_trackExperienceEvent\(\{ name: experienceEvent \}\)/);
    assert.doesNotMatch(pokiSource, /SOW_pokiMeasure/);
    assert.match(indexTemplate, /__INLINE_EXPERIENCE_EVENTS_JS__/);
});


test("CrazyGames font stylesheet and server keep the same cross-origin path", () => {
    assert.ok(siteDistSource.includes('"https://shadowsofwar.io/fonts/fonts.css"'));
    const fontLocation = siteRedirectConfig.match(/location \^~ \/fonts\/ \{([^}]*)\}/);
    assert.ok(fontLocation, "font location is missing");
    for (const directive of [
        /add_header Access-Control-Allow-Origin \*;/,
        /include snippets\/security-headers\.conf;/,
        /try_files \$uri =404;/
    ]) assert.match(fontLocation[1], directive);
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
    assert.match(siteDistSource, /for badge in SITE_BADGES \{\s*publish\(&media\.join\(badge\), &format!\("site\/media\/\{badge\}"\), None\)\?/);
    for (const [, asset] of readme.matchAll(/assets\/site\/media\/([^\s)]+)/g)) {
        assert.ok(siteDistSource.includes(`"${asset}"`), `README media is not accounted for by the asset pipeline: ${asset}`);
    }
    assert.match(siteCss, /\.hero-alpha-links \{ display: grid; grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);/);
    assert.match(siteCss, /\.hero-alpha-links \{ display: grid; grid-template-columns: repeat\(2, minmax\(0, 1fr\)\); gap: 6\.5px; max-width: 299px; \}/);
    assert.match(siteCss, /\.hero-alpha-badge \{ display: flex; align-items: center; gap: 8px; min-width: 0; min-height: 46px;/);
    assert.match(siteCss, /\.hero-alpha-icon \{ flex: 0 0 30px; width: 30px; height: 30px; \}/);
    const narrowStyles = siteCss.slice(siteCss.indexOf("@media (max-width: 479px)"), siteCss.indexOf("@media (orientation: landscape)"));
    assert.match(narrowStyles, /\.hero-alpha-links \{ grid-template-columns: repeat\(2, minmax\(0, 1fr\)\); gap: 6px; max-width: none; \}/);
    assert.match(narrowStyles, /\.hero-alpha-badge \{ min-height: 42px; gap: 6px; padding-inline: 6px; \}/);
    assert.match(narrowStyles, /\.hero-alpha-icon \{ flex-basis: 27px; width: 27px; height: 27px; \}/);
    assert.match(narrowStyles, /\.hero-alpha-copy b \{ font-size: 12px; \}/);
});


test("retired marketing pages stay deleted and absent from navigation", () => {
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
    assert.ok(shellSource.includes('renderSettingsLink("https://shadowsofwar.io/#faq", "site.faq")'));
    assert.doesNotMatch(shellSource, /\/how-to-play\//);
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


test("AGPL attribution stays visible while legal links live in settings", () => {
    for (const [platform, source] of [["standard", shellSource], ["Poki", pokiSource], ["Jest", licenseJestSource]]) {
        assert.match(source, /sow-menu__attribution/, `${platform} menu has no visible attribution`);
        assert.equal((source.match(/© OpenFront and Contributors/g) || []).length, 1, `${platform} credit must appear once`);
    }
    assert.match(shellSource, /function renderSettingsLinks\(\)/);
    assert.match(shellSource, /menu\.source_code/);
    assert.match(shellSource, /menu\.license/);
    assert.match(shellSource, /https:\/\/shadowsofwar\.io\/terms\//);
    assert.match(shellSource, /https:\/\/shadowsofwar\.io\/privacy\//);
    assert.match(shellSource, /sourceUrl\.replace\("\/tree\/", "\/blob\/"\) \+ "\/LICENSE"/);
    assert.match(pokiSource, /function pokiLicenseUrl\(\)[\s\S]*sourceUrl\.replace\("\/tree\/", "\/blob\/"\) \+ "\/LICENSE"/);
    assert.match(pokiSource, /data-command='poki_license'/);
    assert.match(pokiSource, /window\.SOW_pokiOpenExternalLink\(licenseUrl\)/);
    assert.match(licenseMenuLayout, /\.sow-menu__attribution/);
    assert.match(licenseMenuLayout, /\.sow-menu__attribution\s*\{[^}]*position:\s*absolute;[^}]*opacity:\s*0\.75;/s);
    assert.match(licenseMenuLayout, /OpenFront AGPL section 7\(b\).*attribution/);
    assert.doesNotMatch(licenseMenuLayout, /sow-menu__footer/);
    assert.match(shellSource, /SOW_androidOssLicensesAvailable/);
    assert.match(shellSource, /data-command='open_android_oss_licenses'/);
    assert.match(shellSource, /window\.SOW_openAndroidOssLicenses\(\)/);
    assert.match(portalSource, /androidBridgeCapabilities\.indexOf\("open_source_licenses"\)/);
    assert.match(portalSource, /open_external_link/);
    assert.match(shellSource, /window\.SOW_openAndroidExternalLink\(url\)/);
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

