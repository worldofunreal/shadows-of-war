// Public-site publisher. Bun's native HTML rewriter keeps translation and URL
// changes on parsed elements; all pages use the game's exported catalogs.
import { createHash } from "node:crypto";
import { readFile, writeFile, mkdir, stat } from "node:fs/promises";
import { join, dirname } from "node:path";

const origin = "https://shadowsofwar.io";
const escape = (value: unknown) =>
  String(value ?? "").replace(
    /[&<>"']/g,
    (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!,
  );
const json = (value: unknown) => JSON.stringify(value).replace(/</g, "\\u003c");
const read = (path: string) => readFile(path, "utf8");
type Leader = { id: string; artSlug: string; region: string; wikipedia: string | null };

export async function publish(
  out: string,
  input: { leaders: Leader[]; pages: string[] },
  source = import.meta.dir,
) {
  const { leaders } = input;
  const bases = input.pages.map((page) => {
    if (!page.endsWith("index.html")) throw Error(`Invalid public template ${page}`);
    return page.slice(0, -"index.html".length);
  });
  const registry = JSON.parse(await read(join(out, "locales/index.json")));
  await writeFile(join(out, "llms.txt"), await read(join(source, "llms.txt")));
  const languages: { code: string; name: string }[] = registry.languages;
  const routes = [...bases, "leaders/", ...leaders.map((h) => `leaders/${h.id}/`)];
  if (!leaders.length || new Set(leaders.map((h) => h.id)).size !== leaders.length)
    throw Error("Invalid public leader catalog");
  const prefix = (code: string) => (code === "en" ? "" : `${code.toLowerCase()}/`);
  const path = (code: string, route: string) => `/${prefix(code)}${route}`;
  const templates = new Map(
    await Promise.all(
      bases.map(async (route) => [route, await read(join(source, route, "index.html"))] as const),
    ),
  );
  // Rust owns the data-driven landing sections, rendered once before this step.
  templates.set("", await read(join(out, "index.html")));
  const sourceGraph = JSON.parse(
    templates.get("")!.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/)?.[1] ||
      '{"@graph":[]}',
  )["@graph"];
  const faqKeys: { question: string; answer?: string }[] = [];
  await new HTMLRewriter()
    .on(".faq-item summary[data-i18n]", {
      element(e) {
        faqKeys.push({ question: e.getAttribute("data-i18n")! });
      },
    })
    .on(".faq-item p[data-i18n]", {
      element(e) {
        if (faqKeys.length) faqKeys[faqKeys.length - 1].answer = e.getAttribute("data-i18n")!;
      },
    })
    .transform(new Response(templates.get("")!))
    .text();
  const header = await read(join(source, "site-header.html"));
  const leaderTemplate = await read(join(source, "leader.html"));
  const indexTemplate = await read(join(source, "leaders/index.html"));
  const footer = templates.get("")!.match(/<footer\b[\s\S]*?<\/footer>/)?.[0];
  if (!footer) throw Error("Landing footer missing");
  const fingerprints = new Map<string, string>();
  for (const name of [
    "styles.css",
    "app.js",
    "site-chrome.js",
    "analytics.js",
    "sow-i18n.js",
    "sow-dropdown.js",
    "sow-controls.css",
    "wou-auth.js",
  ]) {
    fingerprints.set(
      `/${name}`,
      createHash("sha256")
        .update(await readFile(join(out, name)))
        .digest("hex")
        .slice(0, 10),
    );
  }
  const localeVersions = Object.fromEntries(
    await Promise.all(
      languages.map(async (l) => [
        `/locales/${l.code.toLowerCase()}`,
        createHash("sha256")
          .update(await readFile(join(out, "locales", l.code.toLowerCase())))
          .digest("hex")
          .slice(0, 10),
      ]),
    ),
  );
  const localLinks = new Set<string>();
  let count = 0;
  for (const language of languages) {
    const code = language.code;
    const catalog = JSON.parse(await read(join(out, "locales", code.toLowerCase()))).strings;
    const t = (key: string, values: Record<string, unknown> = {}) => {
      let value: any = key.split(".").reduce((v: any, k) => v?.[k], catalog);
      if (typeof value !== "string" || !value.trim())
        throw Error(`Missing public translation ${code}: ${key}`);
      value = value.replace(/\{(\w+)\}/g, (m: string, k: string) => (k in values ? String(values[k]) : m));
      return value as string;
    };
    const text = (h: Leader, key: string) =>
      t(`${key === "ability" || key === "description" ? "site" : "heroes"}.leader_${h.id}_${key}`);
    const fields = (h: Leader) => ({
      name: text(h, "name"),
      civilization: text(h, "civilization"),
      historical: text(h, "historical"),
      ability: text(h, "ability"),
      effect: text(h, "description"),
    });
    for (const route of routes) {
      const leader = leaders.find((h) => route === `leaders/${h.id}/`);
      let html: string, title: string, description: string;
      const image = leader
        ? `/assets/shell/leaders/${leader.artSlug}_desktop.webp`
        : "/assets/shell/loader/sow-splash-desktop.webp";
      if (leader) {
        const f = fields(leader);
        title = `${f.name} — ${f.civilization} | Shadows of War`;
        description = `${f.name} · ${f.civilization}. ${f.ability}: ${f.effect}`;
        const panel = (label: string, value: string) => `<dt>${escape(label)}</dt><dd>${escape(value)}</dd>`;
        const content = `<div class="container"><nav class="guide-breadcrumbs" aria-label="${escape(t("site.main_navigation"))}"><a href="/">${escape(t("site.nav_home"))}</a> / <a href="/leaders/">${escape(t("site.leaders"))}</a> / ${escape(f.name)}</nav></div>
<article class="container leader-profile"><header class="leader-profile-header"><p class="eyebrow">${escape(f.civilization)}</p><h1>${escape(f.name)}</h1><p class="leader-subtitle">${escape(f.historical)}</p></header>
<div class="leader-profile-grid"><div class="leader-profile-media"><picture class="leader-portrait-frame"><source media="(max-width: 700px)" srcset="/assets/shell/leaders/${leader.artSlug}_mobile.webp"><img src="${image}" alt="${escape(f.name)}" width="1600" height="900" fetchpriority="high"></picture><a class="button button-orange button-full" href="/play/">${escape(t("site.play_now"))}</a></div>
<div class="leader-profile-content"><section class="leader-card-panel"><h2>${escape(t("site.command_trait"))}</h2><div class="ability-block"><h3 class="ability-tag">${escape(f.ability)}</h3><p class="ability-effect">${escape(f.effect)}</p></div></section><section class="leader-card-panel"><dl class="leader-dossier">${panel(t("site.historical_name"), f.historical)}${panel(t("site.civilization"), f.civilization)}${panel(t("site.region"), t(`heroes.${leader.region}`))}</dl>${leader.wikipedia ? `<p><a href="${escape(leader.wikipedia)}" rel="noopener noreferrer" target="_blank">${escape(t("site.historical_reference"))} ↗</a></p>` : ""}</section><p><a href="/leaders/">← ${escape(t("site.all_leaders"))}</a></p><p><a href="/#leaders">${escape(t("site.choose_leader"))}</a></p></div></div></article>`;
        html = leaderTemplate
          .replace("<!-- PAGE_TITLE -->", escape(title))
          .replace("<!-- LEADER_CONTENT -->", content)
          .replace("<!-- LEADER_FOOTER -->", footer);
      } else if (route === "leaders/") {
        title = `${t("site.leaders")} | Shadows of War`;
        description = t("site.leaders_summary");
        const cards = leaders
          .map((h) => {
            const f = fields(h);
            return `<article class="compendium-card" data-compendium-card data-search="${escape(`${f.name} ${f.civilization}`)}"><a class="compendium-card-link" href="/leaders/${h.id}/"><picture class="compendium-thumb"><img src="/assets/gameplay/avatars/${h.artSlug}.webp" alt="${escape(f.name)}" width="256" height="256" loading="lazy" decoding="async"></picture><div class="compendium-card-body"><p class="compendium-civ">${escape(f.civilization)}</p><h2>${escape(f.name)}</h2><p class="compendium-ability"><strong>${escape(f.ability)}:</strong> ${escape(f.effect)}</p></div></a></article>`;
          })
          .join("\n");
        html = indexTemplate
          .replace("<!-- LEADER_CONTENT -->", cards)
          .replace("<!-- LEADER_FOOTER -->", footer);
      } else {
        const key = route ? route.slice(0, -1) : "";
        title = t(key ? `site.${key}_title` : "site.title");
        description = t(key ? `site.${key}_description` : "site.description");
        html = templates.get(route)!;
      }
      const url = origin + path(code, route);
      const alternates =
        languages
          .map(
            (l) =>
              `<link rel="alternate" hreflang="${l.code === "fil" ? "tl" : l.code}" href="${origin + path(l.code, route)}">`,
          )
          .join("\n") + `\n<link rel="alternate" hreflang="x-default" href="${origin + path("en", route)}">`;
      const switches = `<details class="site-language"><summary>${escape(language.name)}</summary><nav aria-label="${escape(t("site.select_language"))}">${languages.map((l) => `<a href="${path(l.code, route)}" hreflang="${l.code}" lang="${l.code}" data-site-language="${l.code}" ${l.code === code ? 'aria-current="page"' : ""}>${escape(l.name)}</a>`).join("")}</nav></details>`;
      const bootstrap = `<script data-cfasync="false">window.SOW_SITE_LOCALE = ${json(code)}; window.SOW_LOCALE_BEFORE_PAINT = true; window.SOW_LOCALE_CATALOG_VERSION = ${registry.version}; window.SOW_LOCALE_CODES = ${json(languages.map((l) => l.code))}; window.SOW_ASSET_TS = ${json(localeVersions)};</script><script data-cfasync="false" src="/sow-i18n.js?v=${fingerprints.get("/sow-i18n.js")}"></script>`;
      for (const marker of ["<!-- SOW_SITE_HEADER -->", "<!-- SOW_SITE_LOCALE -->"]) {
        if (html.split(marker).length !== 2) throw Error(`Public page ${url} must contain one ${marker}`);
      }
      html = html.replace("<!-- SOW_SITE_HEADER -->", header).replace("<!-- SOW_SITE_LOCALE -->", bootstrap);
      const schemas: any[] = [
        {
          "@type": "WebPage",
          "@id": url,
          url,
          name: title,
          description,
          inLanguage: code,
          isPartOf: { "@id": origin + "/#website" },
        },
        { "@type": "WebSite", "@id": origin + "/#website", url: origin + "/", name: "Shadows of War" },
      ];
      schemas[0].image = origin + image;
      const organization = sourceGraph.find((node) => node["@type"] === "Organization");
      if (organization) {
        schemas.push(organization);
        schemas[1].publisher = { "@id": organization["@id"] };
      }
      if (!route) {
        const game = sourceGraph.find((node) => node["@type"] === "VideoGame");
        if (game) {
          schemas.push({ ...game, description, url, inLanguage: code });
          schemas[0].about = { "@id": game["@id"] };
        }
        if (faqKeys.length)
          schemas.push({
            "@type": "FAQPage",
            "@id": url + "#faq",
            inLanguage: code,
            mainEntity: faqKeys.map((item) => ({
              "@type": "Question",
              name: t(item.question),
              acceptedAnswer: { "@type": "Answer", text: t(item.answer!) },
            })),
          });
      }
      if (leader)
        schemas[0].about = {
          "@type": "Person",
          name: fields(leader).historical,
          ...(leader.wikipedia ? { sameAs: leader.wikipedia } : {}),
        };
      if (route === "leaders/")
        schemas[0].mainEntity = {
          "@type": "ItemList",
          numberOfItems: leaders.length,
          itemListElement: leaders.map((h, i) => ({
            "@type": "ListItem",
            position: i + 1,
            name: text(h, "name"),
            url: origin + path(code, `leaders/${h.id}/`),
          })),
        };
      if (route)
        schemas.push({
          "@type": "BreadcrumbList",
          itemListElement: [
            { "@type": "ListItem", position: 1, name: t("site.nav_home"), item: origin + path(code, "") },
            ...(leader
              ? [
                  {
                    "@type": "ListItem",
                    position: 2,
                    name: t("site.leaders"),
                    item: origin + path(code, "leaders/"),
                  },
                ]
              : []),
            {
              "@type": "ListItem",
              position: leader ? 3 : 2,
              name: leader ? fields(leader).name : title,
              item: url,
            },
          ],
        });
      const headExtra = `${alternates}<script type="application/ld+json">${json({ "@context": "https://schema.org", "@graph": schemas })}</script>`;
      const routeForLink = (raw: string, asset = false) => {
        if (!raw || raw.startsWith("#") || /^(?:[a-z]+:|\/\/)/i.test(raw)) return raw;
        const u = new URL(raw, origin + "/" + (bases.includes(route) ? route : ""));
        if (!asset && routes.includes(u.pathname.slice(1))) u.pathname = path(code, u.pathname.slice(1));
        if (fingerprints.has(u.pathname)) u.search = `?v=${fingerprints.get(u.pathname)}`;
        return u.pathname + u.search + u.hash;
      };
      html = await new HTMLRewriter()
        .on("html", {
          element(e) {
            e.setAttribute("lang", code);
            e.setAttribute("dir", code === "ar" ? "rtl" : "ltr");
            e.setAttribute("data-site-page", route);
            e.setAttribute("data-language-suggestion", t("site.suggest_language"));
            e.setAttribute("data-site-prefix", `/${prefix(code)}`);
            e.setAttribute(
              "data-locale-script",
              code === "ar"
                ? "arabic"
                : ["ja", "ko", "zh-CN"].includes(code)
                  ? "cjk"
                  : code === "ru"
                    ? "cyrillic"
                    : "latin",
            );
          },
        })
        .on("head", {
          element(e) {
            e.append(headExtra, { html: true });
          },
        })
        .on('script[type="application/ld+json"], link[rel="alternate"]', {
          element(e) {
            e.remove();
          },
        })
        .on("[data-i18n]", {
          element(e) {
            const key = e.getAttribute("data-i18n")!;
            const vals = JSON.parse(e.getAttribute("data-i18n-values") || "{}");
            const value = t(key, Object.fromEntries(Object.entries(vals).map(([k, v]) => [k, t(String(v))])));
            const attr = e.getAttribute("data-i18n-attr");
            if (attr) e.setAttribute(attr, value);
            else e.setInnerContent(value);
            const placeholder = e.getAttribute("data-i18n-placeholder");
            if (placeholder) e.setAttribute("placeholder", t(placeholder));
          },
        })
        .on(".roster-card[data-leader-id]", {
          element(e) {
            const h = leaders.find((h) => h.id === e.getAttribute("data-leader-id"));
            if (!h) throw Error("Unknown roster leader");
            const f = fields(h);
            for (const [attr, key] of Object.entries({
              "data-name": "name",
              "data-civ": "civilization",
              "data-historical": "historical",
              "data-ability": "ability",
              "data-description": "effect",
            }))
              e.setAttribute(attr, f[key]);
          },
        })
        .on("title", {
          element(e) {
            e.setInnerContent(title);
          },
        })
        .on('meta[name="description"],meta[property="og:description"],meta[name="twitter:description"]', {
          element(e) {
            e.setAttribute("content", description);
          },
        })
        .on('meta[property="og:title"],meta[name="twitter:title"]', {
          element(e) {
            e.setAttribute("content", title);
          },
        })
        .on('meta[property="og:image"],meta[name="twitter:image"]', {
          element(e) {
            e.setAttribute("content", origin + image);
          },
        })
        .on('meta[property="og:url"]', {
          element(e) {
            e.setAttribute("content", url);
          },
        })
        .on('link[rel="canonical"]', {
          element(e) {
            e.setAttribute("href", url);
          },
        })
        .on("[data-locale-select]", {
          element(e) {
            e.replace(switches, { html: true });
          },
        })
        .on("*", {
          element(e) {
            for (const attr of ["href", "src", "poster"]) {
              const value = e.getAttribute(attr);
              if (value) e.setAttribute(attr, routeForLink(value, attr !== "href"));
            }
            const srcset = e.getAttribute("srcset");
            if (srcset)
              e.setAttribute(
                "srcset",
                srcset
                  .split(",")
                  .map((v) => {
                    const [u, ...rest] = v.trim().split(/\s+/);
                    return [routeForLink(u, true), ...rest].join(" ");
                  })
                  .join(", "),
              );
          },
        })
        .transform(new Response(html))
        .text();
      // Inserted markup is not revisited by HTMLRewriter. Check the actual output,
      // including responsive artwork and scripts, before a package can publish.
      const required = {
        "header.site-nav": 0,
        h1: 0,
        title: 0,
        'link[rel="canonical"]': 0,
        'meta[name="description"]': 0,
      };
      const verify = new HTMLRewriter();
      for (const selector of Object.keys(required))
        verify.on(selector, {
          element(e) {
            required[selector]++;
            if (e.tagName === "meta" && !e.getAttribute("content")?.trim())
              throw Error(`Empty public description ${url}`);
          },
        });
      const reference = (value: string) => {
        const target = new URL(value, url);
        if (target.origin === origin) localLinks.add(target.pathname);
      };
      await verify
        .on("*", {
          element(e) {
            for (const attr of ["href", "src", "poster"]) {
              const value = e.getAttribute(attr);
              if (value) reference(value);
            }
            const srcset = e.getAttribute("srcset");
            if (srcset)
              for (const candidate of srcset.split(",")) reference(candidate.trim().split(/\s+/)[0]);
          },
        })
        .transform(new Response(html))
        .text();
      for (const [selector, total] of Object.entries(required)) {
        if (total !== 1) throw Error(`Public page ${url} must contain one ${selector}; found ${total}`);
      }
      if (html.includes("<!-- SOW_SITE_") || html.includes("<!-- LEADER_"))
        throw Error(`Unexpanded page ${url}`);
      const destination = join(out, prefix(code), route, "index.html");
      await mkdir(dirname(destination), { recursive: true });
      await writeFile(destination, html);
      count++;
    }
  }
  for (const pathname of localLinks) {
    if (pathname.startsWith("/api/")) continue;
    const file = join(out, pathname);
    try {
      const info = await stat(file);
      if (info.isDirectory()) await stat(join(file, "index.html"));
    } catch {
      throw Error(`Broken public reference ${pathname}`);
    }
  }
  const urls = languages
    .flatMap((l) => routes.map((route) => origin + path(l.code, route)))
    .concat(origin + "/play/");
  await writeFile(
    join(out, "sitemap.xml"),
    `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.map((url) => `<url><loc>${escape(url)}</loc></url>`).join("\n")}\n</urlset>\n`,
  );
  console.log(
    `✅ Public site: ${count} pages, ${languages.length} languages, ${leaders.length} leaders; links verified`,
  );
}
if (import.meta.main) {
  const [out, metadata] = process.argv.slice(2);
  if (!out || !metadata) throw Error("Site publisher requires output and catalog paths");
  await publish(out, JSON.parse(await read(metadata)));
}
