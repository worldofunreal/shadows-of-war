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
type Leader = { id: string; artSlug: string; legacyIds: string[]; region: string; wikipedia: string | null };

export async function publish(
  out: string,
  input: {
    leaders: Leader[];
    pages: string[];
    englishOnly?: string[];
    biographies: { leaders: Record<string, Record<string, any>> };
    mapExplorer: string;
  },
  source = import.meta.dir,
) {
  const { leaders } = input;
  const bases = input.pages.map((page) => {
    if (!page.endsWith("index.html")) throw Error(`Invalid public template ${page}`);
    return page.slice(0, -"index.html".length);
  });
  const registry = JSON.parse(await read(join(out, "locales/index.json")));
  const biographies = input.biographies.leaders;
  await writeFile(join(out, "llms.txt"), await read(join(source, "llms.txt")));
  const languages: { code: string; name: string }[] = registry.languages;
  const routes = [...bases, "leaders/", ...leaders.map((h) => `leaders/${h.id}/`)];
  // Long-tail pages that answer English search intent ship unprefixed only; other
  // languages link to their home page instead of a translated duplicate.
  const englishOnly = new Set((input.englishOnly ?? []).map((page) => page.slice(0, -"index.html".length)));
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
  const mapsTemplate = await read(join(source, "maps/index.html"));
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
    for (const h of leaders) {
      const bio = biographies[code]?.[h.id];
      const historyLength =
        (bio?.highlight ?? "").length +
        (bio?.intro ?? []).join("").length +
        (bio?.sections ?? []).flatMap((section: any) => section.paragraphs ?? []).join("").length;
      if (
        historyLength < (bio?.shortArticle ? 700 : 1200) ||
        (!bio?.sections?.length && !bio?.shortArticle) ||
        !bio?.url?.startsWith("https://") ||
        !bio?.lang
      )
        throw Error(`Missing public biography ${code}: ${h.id}`);
    }
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
      if (code !== "en" && englishOnly.has(route)) continue;
      const leader = leaders.find((h) => route === `leaders/${h.id}/`);
      let html: string, title: string, description: string;
      const image = leader
        ? `/assets/shell/leaders/${leader.artSlug}_desktop.webp`
        : "/assets/shell/loader/sow-splash-desktop.webp";
      if (leader) {
        const f = fields(leader);
        const bio = biographies[code][leader.id];
        const sourceNote = bio.isSaga
          ? `<p class="leader-history-source">${escape(bio.sourceNote)}</p>`
          : bio.sources?.length
            ? `<p class="leader-history-source"><strong>${escape(t("site.historical_reference"))}</strong> ${bio.sources.map((source: { title: string; url: string }) => `<a href="${escape(source.url)}" rel="noopener noreferrer" target="_blank">${escape(source.title)}</a>`).join(" · ")}</p>`
            : (() => {
                const attributionParts = t("site.wikipedia_excerpt").split("{article}");
                if (attributionParts.length !== 2)
                  throw Error(`Invalid Wikipedia attribution translation ${code}`);
                return `<p class="leader-history-source">${escape(attributionParts[0])}<a href="${escape(bio.url)}" rel="noopener noreferrer" target="_blank">${escape(bio.title)}</a>${escape(attributionParts[1])} <a href="https://creativecommons.org/licenses/by-sa/4.0/" rel="license noopener noreferrer" target="_blank">CC BY-SA 4.0</a>.</p>`;
              })();
        title = `${f.name} — ${f.civilization} | Shadows of War`;
        description = `${f.historical}. ${bio.highlight ?? bio.intro[0]}`.slice(0, 160).trim();
        const articleParagraphs = (paragraphs: string[]) =>
          paragraphs.map((paragraph) => `<p>${escape(paragraph)}</p>`).join("");
        const chapters = bio.sections
          .map(
            (section: { title: string; paragraphs: string[] }, index: number) =>
              `<section class="leader-history-chapter" id="leader-history-${index + 1}" lang="${escape(bio.lang)}" dir="auto"><h2>${escape(section.title)}</h2>${articleParagraphs(section.paragraphs)}${section.evidence ? `<p class="leader-history-evidence"><strong>${escape(bio.evidenceLabel)}</strong> ${escape(section.evidence)}</p>` : ""}${section.sources?.length ? `<p class="leader-chapter-citations"><strong>${escape(bio.chapterSourcesLabel)}</strong> ${section.sources.map((source: { title: string; url: string }) => `<a href="${escape(source.url)}" rel="noopener noreferrer" target="_blank">${escape(source.title)}</a>`).join(" · ")}</p>` : ""}</section>`,
          )
          .join("");
        const contents = [
          `<li><a href="#historical-context">${escape(bio.isSaga ? bio.highlightLabel : t("site.historical_context"))}</a></li>`,
          ...bio.sections.map(
            (section: { title: string }, index: number) =>
              `<li><a href="#leader-history-${index + 1}">${escape(section.title)}</a></li>`,
          ),
        ].join("");
        const introduction = bio.isSaga
          ? `<section class="leader-history-intro leader-history-highlight" id="historical-context" lang="${escape(bio.lang)}"><p class="eyebrow">${escape(bio.highlightLabel)}</p><p>${escape(bio.highlight)}</p></section>`
          : `<section class="leader-history-intro" id="historical-context" lang="${escape(bio.lang)}" dir="auto"><h2>${escape(t("site.historical_context"))}</h2>${articleParagraphs(bio.intro)}</section>`;
        const content = `<article class="leader-encyclopedia"><div class="container"><nav class="guide-breadcrumbs" aria-label="${escape(t("site.main_navigation"))}"><a href="/">${escape(t("site.nav_home"))}</a><span aria-hidden="true">/</span><a href="/leaders/">${escape(t("site.leaders"))}</a><span aria-hidden="true">/</span><span aria-current="page">${escape(f.name)}</span></nav></div>
<header class="container leader-cover"><picture class="leader-cover-art"><source media="(max-width: 700px)" srcset="/assets/shell/leaders/${leader.artSlug}_mobile.webp"><img src="${image}" alt="${escape(f.name)}" width="1600" height="900" fetchpriority="high"></picture><div class="leader-cover-shade" aria-hidden="true"></div><div class="leader-cover-copy"><p class="leader-cover-meta">${escape(f.civilization)} <span aria-hidden="true">·</span> ${escape(t(`heroes.${leader.region}`))}</p><h1>${escape(f.name)}</h1><p class="leader-cover-historical">${escape(f.historical)}</p><a class="button button-orange" href="/play/">${escape(t("site.play_now"))}</a></div></header>
<div class="container leader-reading-layout"><nav class="leader-toc" aria-label="${escape(t("site.page_contents"))}"><p>${escape(t("site.page_contents"))}</p><ol>${contents}</ol></nav><div class="leader-history">${introduction}${chapters}${sourceNote}</div>
<aside class="leader-sidebar"><section class="leader-trait"><p class="eyebrow">${escape(t("site.command_trait"))}</p><h2>${escape(f.ability)}</h2><p>${escape(f.effect)}</p></section><a class="leader-sidebar-link" href="/leaders/">← ${escape(t("site.all_leaders"))}</a><a class="leader-sidebar-link" href="/#leaders">${escape(t("site.choose_leader"))}</a></aside></div></article>`;
        html = leaderTemplate
          .replace("<!-- PAGE_TITLE -->", escape(title))
          .replace("<!-- LEADER_CONTENT -->", content)
          .replace("<!-- LEADER_FOOTER -->", footer);
      } else if (route === "leaders/") {
        title = `${t("site.leaders")} | Shadows of War`;
        description = t("site.leaders_summary");
        if (
          indexTemplate.split("<!-- LEADER_DIRECTORY_CARDS -->").length !== 2 ||
          indexTemplate.split("<!-- LEADER_DIRECTORY_COUNT -->").length !== 2
        )
          throw Error("Leader directory needs one card and count marker");
        const cards = leaders
          .map((h) => {
            const f = fields(h);
            return `<a class="leader-directory-card" data-leader-directory-card data-leader-id="${escape(h.id)}" data-name="${escape(f.name)}" data-civ="${escape(f.civilization)}" aria-label="${escape(t("site.inspect_leader", { name: f.name }))}" href="${path(code, `leaders/${h.id}/`)}"><img class="leader-directory-art" src="/assets/shell/leaders/${escape(h.artSlug)}_mobile.webp" alt="" width="900" height="1600" loading="lazy" decoding="async"><img class="leader-directory-avatar" src="/assets/gameplay/avatars/${escape(h.artSlug)}.webp" alt="" width="128" height="128" loading="lazy" decoding="async"><span class="leader-directory-copy"><span class="leader-directory-civilization">${escape(f.civilization)}</span><span class="leader-directory-name">${escape(f.name)}</span><span class="leader-directory-ability">${escape(f.ability)}</span><span class="leader-directory-action">${escape(t("site.view_profile"))} <span aria-hidden="true">→</span></span></span></a>`;
          })
          .join("\n");
        html = indexTemplate
          .replace("<!-- LEADER_DIRECTORY_CARDS -->", cards)
          .replace("<!-- LEADER_DIRECTORY_COUNT -->", `${leaders.length} / ${leaders.length}`)
          .replace("<!-- LEADER_FOOTER -->", footer);
      } else if (route === "maps/") {
        title = `${t("site.maps_in_rotation")} | Shadows of War`;
        description = t("site.maps_page_summary");
        if (mapsTemplate.split("<!-- MAP_EXPLORER -->").length !== 2 || !input.mapExplorer)
          throw Error("Map directory needs one shared map explorer");
        html = mapsTemplate
          .replace("<!-- MAP_EXPLORER -->", input.mapExplorer)
          .replace("<!-- MAPS_FOOTER -->", footer);
      } else if (englishOnly.has(route)) {
        // English-only pages carry their own copy; the template is the source of
        // truth so no catalog entry has to exist in fifteen languages.
        const source = templates.get(route)!;
        const text = (html: string | undefined) =>
          (html ?? "").replace(/&amp;/g, "&").replace(/\s+/g, " ").trim();
        title = text(source.match(/<title>([\s\S]*?)<\/title>/)?.[1]);
        description = text(source.match(/<meta\s+name="description"\s+content="([\s\S]*?)"\s*\/>/)?.[1]);
        if (!title || !description) throw Error(`English-only page ${route} needs a title and description`);
        html = source;
      } else {
        const key = route ? route.slice(0, -1) : "";
        title = t(key ? `site.${key}_title` : "site.title");
        description = t(key ? `site.${key}_description` : "site.description");
        html = templates.get(route)!;
      }
      const url = origin + path(code, route);
      const pageLanguages = englishOnly.has(route) ? languages.filter((l) => l.code === "en") : languages;
      const alternates =
        pageLanguages
          .map(
            (l) =>
              `<link rel="alternate" hreflang="${l.code === "fil" ? "tl" : l.code}" href="${origin + path(l.code, route)}">`,
          )
          .join("\n") + `\n<link rel="alternate" hreflang="x-default" href="${origin + path("en", route)}">`;
      const worldIcon = `<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="site-language-icon icon icon-tabler icons-tabler-outline icon-tabler-world" aria-hidden="true"><path stroke="none" d="M0 0h24v24H0z" fill="none"/><path d="M3 12a9 9 0 1 0 18 0a9 9 0 0 0 -18 0"/><path d="M3.6 9h16.8"/><path d="M3.6 15h16.8"/><path d="M11.5 3a17 17 0 0 0 0 18"/><path d="M12.5 3a17 17 0 0 1 0 18"/></svg>`;
      const switches = `<details class="site-language"><summary>${worldIcon}<span>${escape(language.name)}</span></summary><nav aria-label="${escape(t("site.select_language"))}">${languages.map((l) => `<a href="${path(l.code, englishOnly.has(route) ? "" : route)}" hreflang="${l.code}" lang="${l.code}" data-site-language="${l.code}" ${l.code === code ? 'aria-current="page"' : ""}>${escape(l.name)}</a>`).join("")}</nav></details>`;
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
          description: biographies[code][leader.id].intro[0],
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
      if (route === "maps/")
        schemas[0].mainEntity = {
          "@type": "ItemList",
          numberOfItems: (input.mapExplorer.match(/data-map-select/g) ?? []).length,
          itemListElement: [...input.mapExplorer.matchAll(/data-map-name="([^"]+)"/g)].map((match, i) => ({
            "@type": "ListItem",
            position: i + 1,
            name: match[1],
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
    // Generated after this pass, so it cannot be on disk yet.
    if (pathname.startsWith("/api/") || pathname === "/sitemap.xml") continue;
    const file = join(out, pathname);
    try {
      const info = await stat(file);
      if (info.isDirectory()) await stat(join(file, "index.html"));
    } catch {
      throw Error(`Broken public reference ${pathname}`);
    }
  }
  for (const language of languages) {
    for (const leader of leaders) {
      for (const legacyId of leader.legacyIds) {
        const target = path(language.code, `leaders/${leader.id}/`);
        const destination = join(out, prefix(language.code), `leaders/${legacyId}/`, "index.html");
        const redirect = `<!doctype html><html lang="${escape(language.code)}"><head><meta charset="utf-8"><meta name="robots" content="noindex"><link rel="canonical" href="${escape(origin + target)}"><meta http-equiv="refresh" content="0;url=${escape(target)}"><title>Profile moved</title></head><body><a href="${escape(target)}">Continue to the current leader profile</a></body></html>`;
        await mkdir(dirname(destination), { recursive: true });
        await writeFile(destination, redirect);
      }
    }
  }
  const urls = languages
    .flatMap((l) =>
      routes
        .filter((route) => l.code === "en" || !englishOnly.has(route))
        .map((route) => origin + path(l.code, route)),
    )
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
  const input = JSON.parse(await read(metadata));
  await publish(out, input, import.meta.dir);
}
