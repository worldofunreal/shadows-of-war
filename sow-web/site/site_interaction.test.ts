import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";
import { vm } from "../shell/typescript-test-support.ts";
const shell = path.resolve(__dirname, "../shell");
const surfacesSource = fs.readFileSync(path.join(shell, "main_menu.surfaces.ts"), "utf8");
const pokiSource = fs.readFileSync(path.join(shell, "main_menu.poki.ts"), "utf8");
const licenseJestSource = fs.readFileSync(path.join(shell, "main_menu.jest.ts"), "utf8");
const landingHtml = fs.readFileSync(path.join(shell, "../../sow-web/site/index.html"), "utf8");
const readme = fs.readFileSync(path.join(shell, "../../README.md"), "utf8");
const siteHeader = fs.readFileSync(path.join(shell, "../../sow-web/site/site-header.html"), "utf8");
const gameControlsCss = fs.readFileSync(path.join(shell, "sow-controls.css"), "utf8");
const siteApp = fs.readFileSync(path.join(shell, "../../sow-web/site/app.ts"), "utf8");
const siteCss = fs.readFileSync(path.join(shell, "../../sow-web/site/styles.css"), "utf8");
const siteFooterPages = ["support", "privacy", "terms", "cookies"].map((page) =>
  fs.readFileSync(path.join(shell, `../../sow-web/site/${page}/index.html`), "utf8"),
);
const siteRedirectConfig = fs.readFileSync(
  path.join(shell, "../../sow-dist/deploy/freebsd/conf.d/shadowsofwar.io.conf"),
  "utf8",
);

test("public landing builds heroes, maps and videos from game data at publish time", () => {
  for (const marker of [
    "SOW_SITE_HERO_MEDIA",
    "SOW_SITE_LEADER_SHOWCASE",
    "SOW_SITE_LEADERS",
    "SOW_SITE_MAPS",
    "SOW_SITE_TRAILER",
  ]) {
    assert.equal(landingHtml.split(`<!-- ${marker} -->`).length, 2, `${marker} must appear once`);
  }
  for (const shot of ["step-expand", "step-build", "step-war"]) {
    assert.ok(
      landingHtml.includes(`/assets/site/media/${shot}.webp`),
      `${shot} media is included in the landing page`,
    );
  }
  const updates = landingHtml.slice(
    landingHtml.indexOf('<div class="footer-updates">'),
    landingHtml.indexOf('<div class="footer-top">'),
  );
  assert.match(updates, /href="https:\/\/discord\.gg\//);
  assert.match(updates, /href="https:\/\/t\.me\//);
  assert.doesNotMatch(updates, /disabled|type="email"/);
});

test("CrazyGames menu links open the matching pages on the public site", () => {
  const helperStart = surfacesSource.indexOf("function renderSiteLink(");
  const helperEnd = surfacesSource.indexOf("\n}", helperStart) + 2;
  const helper =
    helperStart >= 0 && helperEnd > helperStart ? surfacesSource.slice(helperStart, helperEnd) : "";
  assert.ok(helper, "main menu has no shared site-link renderer");
  const portal = { SOW_PORTAL: "crazygames" };
  const render = vm.runInNewContext(`(${helper})`, {
    window: portal,
    SOW_t: (key) => key,
    esc: (value) => String(value),
  });
  assert.equal(
    render("/terms/", "auth.terms", "sow-auth__terms"),
    "<a class='sow-auth__terms' href='https://shadowsofwar.io/terms/' target='_blank' rel='noopener noreferrer'>auth.terms</a>",
  );
  portal.SOW_PORTAL = "site";
  assert.equal(render("/terms/", "menu.terms"), "<a href='/terms/'>menu.terms</a>");

  portal.SOW_PORTAL = "crazygames";
  for (const [path, key] of [
    ["/#faq", "site.faq"],
    ["/support/", "menu.support"],
    ["/terms/", "menu.terms"],
    ["/privacy/", "menu.privacy"],
    ["/cookies/", "menu.cookies"],
  ]) {
    const href = `https://shadowsofwar.io${path}`;
    assert.ok(render(path, key).includes(`href='${href}' target='_blank' rel='noopener noreferrer'`), path);
  }
});

test("CrazyGames font stylesheet and server keep the same cross-origin path", () => {
  const fontLocation = siteRedirectConfig.match(/location \^~ \/fonts\/ \{([^}]*)\}/);
  assert.ok(fontLocation, "font location is missing");
  for (const directive of [
    /add_header Access-Control-Allow-Origin \*;/,
    /include snippets\/security-headers\.conf;/,
    /try_files \$uri =404;/,
  ])
    assert.match(fontLocation[1], directive);
});

test("landing alpha badges link to the iOS and Android tests in every locale", () => {
  const alpha = landingHtml.slice(
    landingHtml.indexOf('class="hero-alpha"'),
    landingHtml.indexOf("</div>\n        </div>", landingHtml.indexOf('class="hero-alpha"')),
  );
  assert.match(alpha, /aria-labelledby="hero-alpha-label"/);
  assert.match(alpha, /data-i18n="site\.mobile_alpha_testing"/);
  assert.match(
    alpha,
    /href="https:\/\/testflight\.apple\.com\/join\/Y8fu4zch"\s+target="_blank"\s+rel="noopener"[\s\S]*?src="\/assets\/site\/media\/testflight-badge\.svg"[\s\S]*?data-i18n="site\.testflight_badge_action"[\s\S]*?TestFlight/,
  );
  assert.match(
    alpha,
    /href="https:\/\/play\.google\.com\/apps\/testing\/com\.shadowsofwar"\s+target="_blank"\s+rel="noopener"[\s\S]*?src="\/assets\/site\/media\/google-play-badge\.svg"[\s\S]*?data-i18n="site\.google_play_badge_action"[\s\S]*?Google Play/,
  );
  assert.doesNotMatch(alpha, /join_alpha|JOIN THE ALPHA/i);
  const siteMedia = path.join(shell, "../../assets/site/media");
  for (const asset of ["testflight-badge.svg", "google-play-badge.svg"]) {
    assert.ok(fs.existsSync(path.join(siteMedia, asset)), `missing local vector asset: ${asset}`);
  }
  const playMark = fs.readFileSync(path.join(siteMedia, "google-play-badge.svg"), "utf8");
  for (const color of ["#4285F4", "#34A853", "#FBBC04", "#EA4335"])
    assert.ok(playMark.includes(color), `Google Play mark is missing ${color}`);
  for (const [, asset] of readme.matchAll(/assets\/site\/media\/([^\s)]+)/g)) {
    assert.ok(fs.existsSync(path.join(shell, "../../", asset)), `README references missing media: ${asset}`);
  }
  const narrowStyles = siteCss.slice(
    siteCss.indexOf("@media (max-width: 479px)"),
    siteCss.indexOf("@media (orientation: landscape)"),
  );
  assert.match(
    narrowStyles,
    /\.hero-alpha-links\s*\{\s*grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\);\s*gap: 6px;\s*max-width: none;\s*\}/,
  );
  assert.match(
    narrowStyles,
    /\.hero-alpha-badge\s*\{\s*min-height: 42px;\s*gap: 6px;\s*padding-inline: 6px;\s*\}/,
  );
  assert.match(
    narrowStyles,
    /\.hero-alpha-icon\s*\{\s*flex-basis: 27px;\s*width: 27px;\s*height: 27px;\s*\}/,
  );
  assert.match(narrowStyles, /\.hero-alpha-copy b\s*\{\s*font-size: 12px;\s*\}/);
});

test("retired marketing pages stay deleted and absent from navigation", () => {
  const siteRoot = path.join(shell, "../../sow-web/site");
  assert.equal(fs.existsSync(path.join(siteRoot, "leaders/index.html")), false);
  assert.equal(fs.existsSync(path.join(siteRoot, "how-to-play/index.html")), false);
  const mobileMenu = siteHeader.match(/<nav\s+class="mobile-menu"[\s\S]*?<\/nav>/)?.[0] ?? "";
  for (const [href, key] of [
    ["/", "nav_home"],
    ["/support/", "support"],
    ["/privacy/", "privacy"],
    ["/terms/", "terms"],
    ["/cookies/", "cookies"],
  ]) {
    assert.match(mobileMenu, new RegExp(`href="${href}" data-i18n="site\\.${key}"`));
  }
  assert.match(
    mobileMenu,
    /<div class="mobile-menu-actions">\s*<div\s+class="site-locale"\s+data-locale-select[\s\S]*?<\/div>\s*<\/div>\s*<a href="\/" data-i18n="site\.nav_home">Home<\/a>/,
  );
  const navInnerCss = siteCss.match(/\.nav-inner\s*\{([^}]+)\}/)?.[1] ?? "";
  assert.doesNotMatch(navInnerCss, /\b(?:border|border-radius|background|box-shadow|backdrop-filter)\s*:/);
  const dropdownTriggerCss =
    gameControlsCss.match(/\.sow-control-dropdown__trigger\s*\{([^}]+)\}/)?.[1] ?? "";
  assert.match(dropdownTriggerCss, /border: 1px solid var\(--sow-button-dark-border\)/);
  assert.match(dropdownTriggerCss, /border-radius: var\(--sow-button-radius\)/);
  assert.match(dropdownTriggerCss, /background: var\(--sow-button-dark-bg\)/);
  const mobileMenuCss = siteCss.match(/\.mobile-menu\s*\{(?=[^}]*position: absolute)([^}]+)\}/)?.[1] ?? "";
  assert.match(mobileMenuCss, /background: var\(--panel\)/);
  assert.match(mobileMenuCss, /box-shadow: var\(--surface-shadow\)/);
  const mobileActionsCss = siteCss.match(/\.mobile-menu-actions \{[^}]+\}/)?.[0] ?? "";
  assert.match(mobileActionsCss, /padding-bottom: 2px;/);
  assert.match(mobileActionsCss, /border-bottom: 1px solid var\(--line\);/);
  assert.doesNotMatch(mobileActionsCss, /padding-top|border-top/);
  for (const html of siteFooterPages) {
    assert.doesNotMatch(html, /href=["']\/(?:leaders|how-to-play)\//);
  }
  for (const html of [landingHtml, ...siteFooterPages]) {
    const footer = html.match(/<div class="footer-links">([\s\S]*?)<\/div>/)?.[1] ?? "";
    assert.ok(footer);
    assert.doesNotMatch(footer, /site\.nav_leaders|href=["']\/#leaders["']/);
  }
});

test("mobile navigation and footers reuse the same localization key per link", () => {
  const mobileMenu = siteHeader.match(/<nav class="mobile-menu"[\s\S]*?<\/nav>/)?.[0] ?? "";
  const localizedLinks = (markup) =>
    new Map(
      Array.from(markup.matchAll(/<a\b([^>]*)>/g), ([, attributes]) => {
        const href = attributes.match(/\bhref=["']([^"']+)["']/)?.[1];
        const key = attributes.match(/\bdata-i18n=["']([^"']+)["']/)?.[1];
        return href && key ? [href, key] : null;
      }).filter(Boolean),
    );
  const mobileLinks = localizedLinks(mobileMenu);

  for (const [href, mobileKey] of mobileLinks) {
    const footerKeys = [landingHtml, ...siteFooterPages].flatMap((html) => {
      const footer = html.match(/<div class="footer-links">([\s\S]*?)<\/div>/)?.[1] ?? "";
      const key = localizedLinks(footer).get(href);
      return key ? [key] : [];
    });
    assert.ok(footerKeys.length, `footer has no localized link for ${href}`);
    assert.ok(
      footerKeys.every((key) => key === mobileKey),
      `${href} uses different menu/footer localization keys`,
    );
  }
});

test("AGPL attribution stays visible while legal links live in settings", () => {
  for (const [platform, source] of [
    ["standard", surfacesSource],
    ["Poki", pokiSource],
    ["Jest", licenseJestSource],
  ]) {
    assert.match(source, /sow-menu__attribution/, `${platform} menu has no visible attribution`);
    assert.equal(
      (source.match(/© OpenFront and Contributors/g) || []).length,
      1,
      `${platform} credit must appear once`,
    );
  }
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
    play() {
      plays += 1;
      return Promise.resolve();
    },
    pause() {
      pauses += 1;
    },
  };
  const motion = {
    matches: false,
    addEventListener: (_event, callback) => {
      onMotionChange = callback;
    },
  };
  class IntersectionObserverMock {
    constructor(callback) {
      observeHero = callback;
    }
    observe(target) {
      assert.equal(target, hero);
    }
  }
  const window = {
    IntersectionObserver: IntersectionObserverMock,
    matchMedia: () => motion,
    addEventListener() {},
  };
  const document = {
    visibilityState: "visible",
    querySelector: (selector) => (selector === ".hero-video" ? video : null),
    querySelectorAll: () => [],
    addEventListener: (_event, callback) => {
      onVisibilityChange = callback;
    },
  };

  vm.runInNewContext(siteApp, {
    window,
    document,
    IntersectionObserver: IntersectionObserverMock,
    location: { hostname: "localhost" },
    sessionStorage: { getItem: () => null, setItem() {} },
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
