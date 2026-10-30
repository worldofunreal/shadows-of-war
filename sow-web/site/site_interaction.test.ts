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

test("landing navigation retains the leader compendium and omits retired how-to-play pages", () => {
  const siteRoot = path.join(shell, "../../sow-web/site");
  assert.equal(fs.existsSync(path.join(siteRoot, "leaders/index.html")), true);
  assert.equal(fs.existsSync(path.join(siteRoot, "how-to-play/index.html")), false);
  const mobileMenu = siteHeader.match(/<nav\s+class="mobile-menu"[\s\S]*?<\/nav>/)?.[0] ?? "";
  for (const [href, key] of [
    ["/", "nav_home"],
    ["/leaders/", "leaders"],
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
    assert.doesNotMatch(html, /href=["']\/how-to-play\//);
  }
  for (const html of [landingHtml, ...siteFooterPages]) {
    const footer = html.match(/<div class="footer-links">([\s\S]*?)<\/div>/)?.[1] ?? "";
    assert.ok(footer);
    assert.doesNotMatch(footer, /href=["']\/how-to-play\//);
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

// Run the actual landing controller against catalog-backed elements. No browser
// or game process is needed to cover selection, search and dialog ownership.
function landingExplorer({ video = false } = {}) {
  const catalog = JSON.parse(fs.readFileSync(path.join(__dirname, "../../assets/geo_entities.json"), "utf8"));
  const catalogs = Object.fromEntries(
    ["en", "es"].map((locale) => [
      locale,
      Bun.TOML.parse(
        fs.readFileSync(path.join(__dirname, `../../sow-i18n/strings/${locale}/web.toml`), "utf8"),
      ),
    ]),
  );
  let locale = "en";
  const doc: any = { activeElement: null, visibilityState: "visible" };
  function element(dataset = {}) {
    const handlers = new Map();
    const attributes = new Map();
    const classes = new Set();
    const node: any = {
      dataset,
      hidden: false,
      textContent: "",
      value: "",
      children: [],
      scrollTop: 0,
      complete: false,
      naturalWidth: 0,
      currentSrc: "",
      bounds: { top: 0, bottom: 0 },
      getBoundingClientRect() {
        return node.bounds;
      },
      classList: {
        add: (...names) => names.forEach((name) => classes.add(name)),
        remove: (...names) => names.forEach((name) => classes.delete(name)),
        contains: (name) => classes.has(name),
        toggle: (name, active) => (active ? classes.add(name) : classes.delete(name)),
      },
      addEventListener(name, fn) {
        if (!handlers.has(name)) handlers.set(name, []);
        handlers.get(name).push(fn);
      },
      emit(name, event = {}) {
        for (const fn of handlers.get(name) || []) fn({ currentTarget: node, preventDefault() {}, ...event });
      },
      setAttribute(name, value) {
        attributes.set(name, String(value));
      },
      getAttribute(name) {
        return attributes.get(name) ?? null;
      },
      focus() {
        doc.activeElement = node;
      },
      append(child) {
        if (child.parent) child.parent.children = child.parent.children.filter((item) => item !== child);
        node.children.push(child);
        child.parent = node;
      },
      showModal() {
        node.open = true;
      },
      close() {
        node.open = false;
        node.emit("close");
      },
      animate() {},
    };
    return node;
  }
  const t = (key, values = {}) => {
    let result = key.split(".").reduce((value, part) => value?.[part], catalogs[locale]) || `[${key}]`;
    for (const [name, value] of Object.entries(values))
      result = result.replaceAll(`{${name}}`, String(value));
    return result;
  };
  const entities = catalog.entities.filter((entity) => entity.hero);
  const cards = entities.map((entity, index) => {
    const card = element({
      leaderId: entity.id,
      artSlug: entity.hero.art_slug || entity.id,
      name: t(`heroes.leader_${entity.id}_name`),
      civ: t(`heroes.leader_${entity.id}_civilization`),
      historical: t(`heroes.leader_${entity.id}_historical`),
      ability: t(`site.leader_${entity.id}_ability`),
      description: t(`site.leader_${entity.id}_description`),
    });
    const img = element();
    img.tagName = "IMG";
    img.src = index === 0
      ? `/assets/gameplay/avatars/expressions/${entity.hero.art_slug || entity.id}_angry.webp`
      : `/assets/gameplay/avatars/${entity.hero.art_slug || entity.id}.webp`;
    card.append(img);
    card.querySelector = (sel: string) => (sel === "img" ? img : null);
    return card;
  });
  cards[0].classList.add("is-active");
  const nodes = new Map();
  for (const name of [
    "motion-toggle",
    "motion-label",
    "detail-image",
    "detail-image-mobile",
    "detail-portrait",
    "detail-art",
    "detail-name",
    "detail-civ",
    "detail-historical",
    "detail-ability",
    "detail-description",
    "leader-detail",
    "leader-search",
    "leader-browser",
    "leader-browser-home",
    "leader-picker",
    "leader-browse",
    "roster-count",
    "roster-empty",
    "roster-scroll",
    "leader-position",
    "picker-close",
    "leader-prev",
    "leader-next",
    "clear-search",
    "leader-total",
    "leaders-grid",
    "map-image",
    "map-name",
    "map-frame",
  ])
    nodes.set(`[data-${name}]`, element());
  const node = (name) => nodes.get(`[data-${name}]`);
  node("leader-browser-home").append(node("leader-browser"));
  const maps = ["World", "Europe"].map((name, index) => {
    const card = element({ mapName: name, mapImage: `/maps/${name.toLowerCase()}/thumbnail.webp?v=1` });
    card.setAttribute("aria-pressed", String(index === 0));
    return card;
  });
  node("map-image").setAttribute("src", maps[0].dataset.mapImage);
  const media = new Map();
  function mediaQuery(query) {
    if (!media.has(query)) {
      const value = element();
      value.matches = false;
      media.set(query, value);
    }
    return media.get(query);
  }
  const win: any = element();
  win.scrollY = 620;
  win.matchMedia = mediaQuery;
  win.SOW_t = t;
  win.scrollTo = ({ top }) => {
    win.scrollY = top;
  };
  let visibilityCallback;
  if (video) {
    const player = element();
    player.playCount = 0;
    player.pauseCount = 0;
    player.play = () => {
      player.playCount++;
      return Promise.resolve();
    };
    player.pause = () => {
      player.pauseCount++;
    };
    player.closest = () => element();
    nodes.set(".hero-video", player);
    win.IntersectionObserver = class {
      constructor(callback) {
        visibilityCallback = callback;
      }
      observe() {}
    };
  }
  Object.assign(doc, {
    documentElement: element(),
    querySelector: (selector) => nodes.get(selector) || null,
    querySelectorAll: (selector) =>
      selector === ".roster-card[data-leader-id]"
        ? cards
        : selector === "[data-map-select]"
          ? maps
          : selector === "[data-leader-total]"
            ? [node("leader-total")]
            : [],
    addEventListener: win.addEventListener,
  });
  vm.runInNewContext(siteApp, {
    window: win,
    document: doc,
    getComputedStyle: () => ({ gridTemplateColumns: "90px 90px 90px" }),
  });
  return {
    cards,
    entities,
    maps,
    nodes,
    node,
    win,
    doc,
    t,
    click: (name) => node(name).emit("click"),
    search: (value) => {
      node("leader-search").value = value;
      node("leader-search").emit("input");
    },
    setLocale: (value) => {
      locale = value;
      win.emit("sow:locale-change");
    },
    setMedia: (query, matches) => {
      const value = mediaQuery(query);
      value.matches = matches;
      value.emit("change");
    },
    setVisible: (visible) => visibilityCallback([{ isIntersecting: visible }]),
  };
}

test("landing explorer selects all 80 catalog leaders and keeps failed or late art matched", () => {
  const app = landingExplorer();
  assert.equal(app.cards.length, 80);
  assert.equal(app.node("leader-total").textContent, "80");
  for (const [index, card] of app.cards.entries()) {
    card.emit("click");
    assert.equal(app.node("detail-name").textContent, card.dataset.name);
    assert.equal(app.node("detail-ability").textContent, card.dataset.ability);
    assert.equal(app.node("detail-description").textContent, card.dataset.description);
    assert.equal(app.node("detail-image").src, `/assets/shell/leaders/${card.dataset.artSlug}_desktop.webp`);
    assert.equal(
      app.node("detail-image-mobile").srcset,
      `/assets/shell/leaders/${card.dataset.artSlug}_mobile.webp`,
    );
    assert.equal(app.node("detail-portrait").src, `/assets/gameplay/avatars/${card.dataset.artSlug}.webp`);
    assert.equal(card.getAttribute("aria-pressed"), "true");
    assert.equal(app.cards.filter((item) => item.classList.contains("is-active")).length, 1);
    assert.ok(app.node("leader-position").textContent.endsWith(`${index + 1} / 80`));
  }
  app.cards[0].emit("click");
  app.cards[79].emit("click");
  const image = app.node("detail-image");
  image.currentSrc = `/assets/shell/leaders/${app.cards[0].dataset.artSlug}_desktop.webp`;
  image.complete = true;
  image.naturalWidth = 1600;
  app.cards[1].emit("click");
  app.cards[79].emit("click");
  image.emit("load");
  assert.ok(
    app.node("detail-art").classList.contains("is-loading"),
    "old art must not be revealed under the new name",
  );
  image.currentSrc = image.src;
  image.emit("error");
  assert.ok(app.node("detail-art").classList.contains("is-unavailable"));
  assert.equal(app.node("detail-portrait").hidden, false);
  image.emit("load");
  assert.equal(app.node("detail-art").classList.contains("is-unavailable"), false);
  app.click("leader-next");
  assert.equal(app.cards[0].getAttribute("aria-pressed"), "true");
  app.cards[79].bounds = { top: 800, bottom: 920 };
  app.node("roster-scroll").bounds = { top: 100, bottom: 500 };
  app.click("leader-prev");
  assert.equal(app.cards[79].getAttribute("aria-pressed"), "true");
  assert.equal(app.node("roster-scroll").scrollTop, 420);
  assert.equal(app.win.scrollY, 620, "revealing the selection must not move the preview");
});

test("landing explorer searches localized names and civilizations, preserves selection and clears empty results", () => {
  const app = landingExplorer();
  const selected = app.cards.findIndex((card) => card.dataset.leaderId === "bolivar");
  app.cards[selected].emit("click");
  app.setLocale("es");
  assert.equal(app.node("detail-name").textContent, app.t("heroes.leader_bolivar_name"));
  assert.equal(app.cards[selected].getAttribute("aria-pressed"), "true");
  app.search("simon");
  assert.equal(app.cards[selected].hidden, false, "search ignores accent marks");
  const civilization = app.t("heroes.leader_bolivar_civilization");
  app.search(civilization);
  assert.equal(app.cards[selected].hidden, false);
  app.search("a civilization that does not exist");
  assert.equal(app.cards.filter((card) => !card.hidden).length, 0);
  assert.equal(app.node("roster-empty").hidden, false);
  app.click("clear-search");
  assert.equal(app.node("leader-search").value, "");
  assert.equal(app.cards.filter((card) => !card.hidden).length, 80);
  assert.equal(app.node("roster-empty").hidden, true);
  assert.equal(app.node("detail-name").textContent, app.t("heroes.leader_bolivar_name"));
  app.cards[0].focus();
  app.node("leaders-grid").emit("keydown", { key: "End" });
  assert.equal(app.doc.activeElement, app.cards[79]);
  app.doc.documentElement.dir = "rtl";
  app.cards[10].focus();
  app.node("leaders-grid").emit("keydown", { key: "ArrowRight" });
  assert.equal(app.doc.activeElement, app.cards[9]);
});

test("landing explorer moves one roster into the mobile picker and restores the selected preview and page position", () => {
  const app = landingExplorer();
  const browser = app.node("leader-browser");
  const picker = app.node("leader-picker");
  app.node("leader-browse").focus();
  app.click("leader-browse");
  assert.equal(picker.open, true);
  assert.equal(browser.parent, picker);
  assert.equal(app.doc.activeElement, app.node("leader-search"));
  app.cards[79].emit("click");
  assert.equal(picker.open, false);
  assert.equal(browser.parent, app.node("leader-browser-home"));
  assert.equal(app.cards[79].getAttribute("aria-pressed"), "true");
  assert.equal(app.win.scrollY, 620);
  assert.equal(app.doc.activeElement, app.node("leader-browse"));
  app.click("leader-browse");
  app.click("picker-close");
  assert.equal(app.cards[79].getAttribute("aria-pressed"), "true", "cancel preserves selection");
  app.click("leader-browse");
  picker.close(); // Native Escape follows the same close event.
  assert.equal(browser.parent, app.node("leader-browser-home"));
  app.click("leader-browse");
  app.setMedia("(max-width: 960px)", false);
  assert.equal(picker.open, false, "resizing to desktop restores the inline roster");
});

test("landing explorer switches avatar expressions on hover and selection for desktop and mobile", () => {
  const app = landingExplorer();
  const firstCard = app.cards[0];
  const secondCard = app.cards[1];
  const firstImg = firstCard.querySelector("img");
  const secondImg = secondCard.querySelector("img");
  const firstSlug = firstCard.dataset.artSlug;
  const secondSlug = secondCard.dataset.artSlug;

  // Initial load: selected card is angry, unselected is calm
  assert.equal(firstImg.src, `/assets/gameplay/avatars/expressions/${firstSlug}_angry.webp`);
  assert.equal(secondImg.src, `/assets/gameplay/avatars/${secondSlug}.webp`);

  // Hover unselected: becomes happy
  secondCard.emit("mouseenter");
  assert.equal(secondImg.src, `/assets/gameplay/avatars/expressions/${secondSlug}_happy.webp`);

  // Leave unselected: returns to calm
  secondCard.emit("mouseleave");
  assert.equal(secondImg.src, `/assets/gameplay/avatars/${secondSlug}.webp`);

  // Hover selected: becomes happy
  firstCard.emit("mouseenter");
  assert.equal(firstImg.src, `/assets/gameplay/avatars/expressions/${firstSlug}_happy.webp`);

  // Leave selected: restores to angry
  firstCard.emit("mouseleave");
  assert.equal(firstImg.src, `/assets/gameplay/avatars/expressions/${firstSlug}_angry.webp`);

  // Select second card: second becomes angry, first becomes calm
  secondCard.emit("click");
  assert.equal(secondImg.src, `/assets/gameplay/avatars/expressions/${secondSlug}_angry.webp`);
  assert.equal(firstImg.src, `/assets/gameplay/avatars/${firstSlug}.webp`);

  // On mobile media query: detail-image selects mobile portrait art
  app.setMedia("(max-width: 960px)", true);
  assert.equal(app.node("detail-image").src, `/assets/shell/leaders/${secondSlug}_mobile.webp`);
});

test("landing explorer switches map previews and pauses optional video for visibility and reduced motion", () => {
  const app = landingExplorer({ video: true });
  app.maps[1].emit("click");
  assert.equal(app.node("map-image").src, app.maps[1].dataset.mapImage);
  assert.equal(app.node("map-name").textContent, "Europe");
  assert.equal(app.maps[0].getAttribute("aria-pressed"), "false");
  assert.equal(app.maps[1].getAttribute("aria-pressed"), "true");
  app.node("map-image").currentSrc = app.node("map-image").src;
  app.node("map-image").emit("error");
  assert.ok(app.node("map-frame").classList.contains("is-unavailable"));
  const player = app.nodes.get(".hero-video");
  assert.equal(player.playCount, 0);
  app.setVisible(true);
  assert.equal(player.playCount, 1);
  app.doc.visibilityState = "hidden";
  app.win.emit("visibilitychange");
  assert.equal(player.hidden, true);
  const hiddenPlays = player.playCount;
  app.setVisible(true);
  assert.equal(player.playCount, hiddenPlays);
  app.doc.visibilityState = "visible";
  app.win.emit("visibilitychange");
  assert.equal(player.playCount, hiddenPlays + 1);
  app.click("motion-toggle");
  assert.equal(app.node("motion-toggle").getAttribute("aria-pressed"), "true");
  const plays = player.playCount;
  app.setVisible(false);
  app.setVisible(true);
  assert.equal(player.playCount, plays, "manual pause survives leaving and reentering the opening");
  app.click("motion-toggle");
  assert.ok(player.playCount > plays);
  app.setMedia("(prefers-reduced-motion: reduce)", true);
  assert.equal(app.node("motion-toggle").disabled, true);
  const reducedPlays = player.playCount;
  app.setVisible(true);
  assert.equal(player.playCount, reducedPlays);
  player.emit("error");
  assert.equal(player.hidden, true);
  assert.equal(app.node("motion-toggle").hidden, true);
});

test("public publisher delivers localized linked profiles and rejects incomplete output", async () => {
  const { publish } = await import("./generate.ts");
  const { mkdtemp, mkdir, writeFile, readFile, rm } = await import("node:fs/promises");
  const { tmpdir } = await import("node:os");
  const temp = await mkdtemp(path.join(tmpdir(), "sow-public-site-"));
  const source = path.join(temp, "source"), out = path.join(temp, "out");
  const put = async (name, value) => {
    await mkdir(path.dirname(name), { recursive: true });
    await writeFile(name, value);
  };
  try {
    const template = '<html lang="en"><head><!-- SOW_SITE_LOCALE --><title data-i18n="site.title">Home</title><meta name="description" content=""><link rel="canonical" href="https://shadowsofwar.io/"></head><body><!-- SOW_SITE_HEADER --><!-- PAGE_HEADING --><!-- LEADER_CONTENT --><footer><a href="/leaders/">Leaders</a></footer></body></html>';
    const languages = [{ code: "en", name: "English" }, { code: "es", name: "Español" }];
    const catalogs = {};
    for (const code of ["en", "es"]) {
      const site = Object.fromEntries(["title", "description", "leaders", "leaders_summary", "nav_home", "main_navigation", "select_language", "command_trait", "historical_name", "civilization", "region", "historical_reference", "all_leaders", "choose_leader", "play_now", "suggest_language", "leader_caesar_ability", "leader_caesar_description", ...["support", "privacy", "cookies", "terms"].flatMap(p => [p+"_title",p+"_description"])].map(key => [key, `${code}: ${key}`]));
      catalogs[code] = { site, heroes: { leader_caesar_name: code === "en" ? "Julius Caesar" : "Julio César", leader_caesar_civilization: code === "en" ? "Roman Republic" : "República romana", leader_caesar_historical: "Gaius Julius Caesar", europe: code === "en" ? "Europe" : "Europa" } };
      await put(path.join(out, "locales", code), JSON.stringify({strings:catalogs[code]}));
    }
    await put(path.join(out,"locales/index.json"),JSON.stringify({version:1,languages}));
    for(const name of ["styles.css","app.js","site-chrome.js","analytics.js","sow-i18n.js","sow-dropdown.js","sow-controls.css","wou-auth.js"]) await put(path.join(out,name), "/* fixture */");
    await put(path.join(out,"play/index.html"),"game");
    await put(path.join(source,"llms.txt"),"Game facts");
    for(const page of ["", "support", "privacy", "cookies", "terms", "leaders"]) await put(path.join(source,page,"index.html"),template.replace("<!-- PAGE_HEADING -->", '<h1 data-i18n="site.title">Home</h1>').replace("<!-- LEADER_CONTENT -->", page==="leaders" ? "<!-- LEADER_CONTENT -->" : ""));
    await put(path.join(source,"leader.html"),template.replace("<!-- PAGE_HEADING -->", ""));
    await put(path.join(source,"site-header.html"),'<header class="site-nav"><a href="/leaders/">Leaders</a><div data-locale-select></div></header>');
    const resetLanding = () => put(path.join(out,"index.html"),template.replace("<!-- PAGE_HEADING -->", '<h1 data-i18n="site.title">Home</h1>').replace("<!-- LEADER_CONTENT -->", ""));
    await resetLanding();
    for (const asset of ["assets/shell/leaders/caesar_desktop.webp", "assets/shell/leaders/caesar_mobile.webp", "assets/gameplay/avatars/caesar.webp"]) await put(path.join(out,asset),"fixture image");
    const leaders = [{id:"caesar",artSlug:"caesar",region:"europe",wikipedia:"https://en.wikipedia.org/wiki/Julius_Caesar"}];
    await publish(out,{leaders,pages:["index.html","support/index.html","privacy/index.html","cookies/index.html","terms/index.html"]},source);
    const profile = await readFile(path.join(out,"es/leaders/caesar/index.html"),"utf8");
    assert.match(profile, /<h1>Julio César<\/h1>/);
    assert.match(profile, /República romana/);
    assert.match(profile, /href="\/es\/leaders\/"/);
    assert.match(profile, /hreflang="en" href="https:\/\/shadowsofwar.io\/leaders\/caesar\/"/);
    assert.match(profile, /rel="canonical" href="https:\/\/shadowsofwar.io\/es\/leaders\/caesar\/"/);
    assert.match(profile, /href="\/play\/"/);
    assert.match(profile, /"inLanguage":"es"/);
    assert.doesNotMatch(profile, /<!-- (SOW_SITE|LEADER)_/);
    const sitemap = await readFile(path.join(out,"sitemap.xml"),"utf8");
    assert.equal((sitemap.match(/<url>/g)||[]).length,15);
    const effect = catalogs.es.site.leader_caesar_description;
    await resetLanding();
    delete catalogs.es.site.leader_caesar_description;
    await put(path.join(out,"locales/es"),JSON.stringify({strings:catalogs.es}));
    await assert.rejects(publish(out,{leaders,pages:["index.html","support/index.html","privacy/index.html","cookies/index.html","terms/index.html"]},source), /Missing public translation es: site.leader_caesar_description/);
    catalogs.es.site.leader_caesar_description = effect;
    await put(path.join(out,"locales/es"),JSON.stringify({strings:catalogs.es}));
    await resetLanding();
    await rm(path.join(out,"assets/shell/leaders/caesar_mobile.webp"));
    await assert.rejects(publish(out,{leaders,pages:["index.html","support/index.html","privacy/index.html","cookies/index.html","terms/index.html"]},source), /Broken public reference \/assets\/shell\/leaders\/caesar_mobile.webp/);
    await resetLanding();
    await put(path.join(source,"site-header.html"),'<div>Missing header</div>');
    await assert.rejects(publish(out,{leaders,pages:["index.html","support/index.html","privacy/index.html","cookies/index.html","terms/index.html"]},source), /must contain one header.site-nav/);
  } finally { await rm(temp,{recursive:true,force:true}); }
});

test("Escape closes the language menu before mobile navigation and restores visible focus", () => {
  const listeners = new Map();
  const classes = new Set();
  let focused;
  const summary = { focus() { focused = summary; } };
  const language = { open: true, querySelector: () => summary, contains: () => false };
  const menu = {
    classList: { toggle(key, on) { if (on) classes.add(key); else classes.delete(key); }, contains: key => classes.has(key) },
    setAttribute() {}, querySelectorAll: () => [], contains: () => false,
  };
  const toggle = { addEventListener() {}, setAttribute() {}, contains: () => false, focus() { focused = toggle; } };
  const document = {
    documentElement: { dataset: { sitePage: "leaders/" } },
    querySelectorAll: selector => selector === ".site-language" ? [language] : [],
    querySelector: selector => selector === "#mobile-menu" ? menu : selector === "[data-menu-toggle]" ? toggle : null,
    addEventListener(type, listener) { const list = listeners.get(type) || []; list.push(listener); listeners.set(type, list); },
  };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname,"site-chrome.ts"),"utf8"), { document, window: { location: { pathname: "/leaders/" } } });
  classes.add("is-open");
  const escape = () => {
    const event = { key: "Escape", defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
    listeners.get("keydown").forEach(handler => handler(event));
  };
  escape();
  assert.equal(language.open, false);
  assert.equal(classes.has("is-open"), true);
  assert.equal(focused, summary);
  escape();
  assert.equal(classes.has("is-open"), false);
  assert.equal(focused, toggle);
});
