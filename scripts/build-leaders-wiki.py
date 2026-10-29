#!/usr/bin/env python3
"""Build static encyclopedic wiki pages for all historical heroes in Shadows of War.

Outputs:
  - sow-web/site/leaders/index.html (Global compendium)
  - sow-web/site/leaders/<hero_id>/index.html (English canonical)
  - sow-web/site/es/leaders/<hero_id>/index.html (Spanish localized)

Includes JSON-LD structured data (Person/HistoricalRole, ImageObject, VideoGameCharacter, BreadcrumbList),
semantic alt attributes, OpenGraph image tags, and hreflang annotations.
"""
import html
import json
import os
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOG_PATH = ROOT / "assets" / "geo_entities.json"
I18N_DIR = ROOT / "sow-i18n" / "strings"
SITE_DIR = ROOT / "sow-web" / "site"

LANGUAGES = [
    ("en", "English", "en"),
    ("es", "Español", "es"),
]


def load_strings(lang: str) -> dict:
    path = I18N_DIR / lang / "web.toml"
    if not path.is_file():
        return {}
    with open(path, "rb") as f:
        return tomllib.loads(f.read().decode("utf-8"))


def get_hero_art_slug(hero: dict) -> str:
    return hero.get("hero", {}).get("art_slug") or hero["id"]


def format_ability_summary(ability: dict) -> str:
    kind = ability.get("kind", "")
    percent = ability.get("percent")
    pct_str = f"{percent}%" if percent else ""
    return f"{kind.replace('_', ' ').title()} {pct_str}".strip()


def render_leader_page(
    hero: dict,
    lang: str,
    strings: dict,
    all_heroes: list,
    root_rel: str,
) -> str:
    hid = hero["id"]
    art_slug = get_hero_art_slug(hero)
    wiki_url = hero.get("wikipedia", "")
    region = (hero.get("region") or "World").capitalize()
    
    # Text lookups
    site = strings.get("site", {})
    heroes_dict = strings.get("heroes", {})

    name = heroes_dict.get(f"leader_{hid}_name") or site.get(f"leader_{hid}_name") or hero["name"]
    civ = heroes_dict.get(f"leader_{hid}_civilization") or hero.get("entity", "")
    historical = heroes_dict.get(f"leader_{hid}_historical") or name
    ability_name = site.get(f"leader_{hid}_ability") or hero.get("hero", {}).get("name", "Special Trait")
    ability_desc = site.get(f"leader_{hid}_description") or "Unique historical command bonus."

    is_es = lang == "es"
    page_title = (
        f"{name} — {civ} | Líderes de Shadows of War MMORTS"
        if is_es
        else f"{name} — {civ} | Shadows of War MMORTS Leader Compendium"
    )
    meta_desc = (
        f"Biografía histórica, bonificación militar y estadísticas tácticas de {name} ({civ}) en el juego de estrategia online Shadows of War."
        if is_es
        else f"Historical biography, command trait, and tactical conquest stats for {name} ({civ}) in the online strategy MMORTS Shadows of War."
    )

    canonical_url = f"https://shadowsofwar.io/leaders/{hid}/" if lang == "en" else f"https://shadowsofwar.io/es/leaders/{hid}/"
    en_url = f"https://shadowsofwar.io/leaders/{hid}/"
    es_url = f"https://shadowsofwar.io/es/leaders/{hid}/"

    # Breadcrumbs
    home_label = "Inicio" if is_es else "Home"
    compendium_label = "Líderes" if is_es else "Leaders"
    compendium_url = f"{root_rel}es/leaders/" if is_es else f"{root_rel}leaders/"

    # Section headings
    overview_heading = "Resumen Histórico y Rol Táctico" if is_es else "Historical Overview & Tactical Role"
    command_heading = "Habilidad de Mando en el Juego" if is_es else "In-Game Command Trait"
    stats_heading = "Ficha de Datos" if is_es else "Leader Dossier"
    civ_label = "Civilización / Nación" if is_es else "Civilization / Nation"
    region_label = "Región Geográfica" if is_es else "Geographic Region"
    historical_name_label = "Nombre Histórico" if is_es else "Historical Name"
    wiki_label = "Artículo oficial en Wikipedia" if is_es else "Official Wikipedia Biography"
    play_cta = "Jugar como " + name if is_es else "Play as " + name
    play_sub = "Sin descargas · En tu navegador" if is_es else "Play instantly in browser · No download"

    # Schema.org JSON-LD
    schema = {
        "@context": "https://schema.org",
        "@graph": [
            {
                "@type": "Person",
                "@id": f"{canonical_url}#person",
                "name": historical,
                "alternateName": name,
                "description": f"Historical ruler of {civ}.",
                "sameAs": [wiki_url] if wiki_url else [],
                "image": f"https://shadowsofwar.io/assets/shell/leaders/{art_slug}_desktop.webp",
            },
            {
                "@type": "ImageObject",
                "@id": f"{canonical_url}#primaryimage",
                "url": f"https://shadowsofwar.io/assets/shell/leaders/{art_slug}_desktop.webp",
                "contentUrl": f"https://shadowsofwar.io/assets/shell/leaders/{art_slug}_desktop.webp",
                "caption": f"{name} {civ} leader portrait artwork in Shadows of War",
                "name": f"{name} Portrait",
            },
            {
                "@type": "BreadcrumbList",
                "@id": f"{canonical_url}#breadcrumb",
                "itemListElement": [
                    {"@type": "ListItem", "position": 1, "name": home_label, "item": "https://shadowsofwar.io/"},
                    {"@type": "ListItem", "position": 2, "name": compendium_label, "item": "https://shadowsofwar.io/leaders/"},
                    {"@type": "ListItem", "position": 3, "name": name, "item": canonical_url},
                ],
            },
            {
                "@type": "WebPage",
                "@id": canonical_url,
                "url": canonical_url,
                "name": page_title,
                "description": meta_desc,
                "inLanguage": lang,
                "breadcrumb": {"@id": f"{canonical_url}#breadcrumb"},
                "primaryImageOfPage": {"@id": f"{canonical_url}#primaryimage"},
            },
        ],
    }

    # Render HTML
    return f"""<!doctype html>
<html lang="{lang}" data-theme="dark">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
  <meta name="theme-color" content="#0a0a0e">
  <title>{html.escape(page_title)}</title>
  <meta name="description" content="{html.escape(meta_desc)}">
  <link rel="canonical" href="{canonical_url}">
  <link rel="alternate" hreflang="en" href="{en_url}">
  <link rel="alternate" hreflang="es" href="{es_url}">
  <link rel="alternate" hreflang="x-default" href="{en_url}">
  <meta name="robots" content="index, follow">

  <!-- OpenGraph -->
  <meta property="og:type" content="article">
  <meta property="og:site_name" content="Shadows of War">
  <meta property="og:title" content="{html.escape(page_title)}">
  <meta property="og:description" content="{html.escape(meta_desc)}">
  <meta property="og:url" content="{canonical_url}">
  <meta property="og:image" content="https://shadowsofwar.io/assets/shell/leaders/{art_slug}_desktop.webp">
  <meta property="og:image:alt" content="{html.escape(name)} - {html.escape(civ)} leader portrait artwork">

  <!-- Twitter Card -->
  <meta name="twitter:card" content="summary_large_image">
  <meta name="twitter:title" content="{html.escape(page_title)}">
  <meta name="twitter:description" content="{html.escape(meta_desc)}">
  <meta name="twitter:image" content="https://shadowsofwar.io/assets/shell/leaders/{art_slug}_desktop.webp">
  <meta name="twitter:image:alt" content="{html.escape(name)} leader artwork">

  <link rel="icon" type="image/svg+xml" href="{root_rel}sow.svg">
  <link rel="stylesheet" href="{root_rel}fonts/fonts.css">
  <link rel="stylesheet" href="{root_rel}styles.css">

  <script type="application/ld+json">
{json.dumps(schema, indent=2)}
  </script>
</head>
<body class="guide-page">
  <!-- SOW_SITE_HEADER -->

  <main class="guide-main">
    <div class="container">
      <nav class="guide-breadcrumbs" aria-label="Breadcrumb">
        <ol>
          <li><a href="{root_rel}">{home_label}</a></li>
          <li><a href="{compendium_url}">{compendium_label}</a></li>
          <li aria-current="page">{html.escape(name)}</li>
        </ol>
      </nav>
    </div>

    <article class="container leader-profile">
      <header class="leader-profile-header">
        <p class="eyebrow">{html.escape(civ)} · {html.escape(region)}</p>
        <h1>{html.escape(name)}</h1>
        <p class="leader-subtitle">{html.escape(historical)}</p>
      </header>

      <div class="leader-profile-grid">
        <div class="leader-profile-media">
          <picture class="leader-portrait-frame">
            <source media="(max-width: 700px)" srcset="/assets/shell/leaders/{art_slug}_mobile.webp">
            <img src="/assets/shell/leaders/{art_slug}_desktop.webp"
                 alt="{html.escape(name)} - {html.escape(civ)} ruler leader portrait artwork in Shadows of War"
                 width="600" height="750" fetchpriority="high" decoding="async">
          </picture>
          <div class="leader-cta-card">
            <a class="button button-orange button-full" href="/play/">{html.escape(play_cta)} <span aria-hidden="true">↗</span></a>
            <small>{html.escape(play_sub)}</small>
          </div>
        </div>

        <div class="leader-profile-content">
          <section class="leader-card-panel">
            <h2>{html.escape(command_heading)}</h2>
            <div class="ability-block">
              <span class="ability-tag">{html.escape(ability_name)}</span>
              <p class="ability-effect">{html.escape(ability_desc)}</p>
            </div>
          </section>

          <section class="leader-card-panel">
            <h2>{html.escape(overview_heading)}</h2>
            <p>
              {"En Shadows of War, " + html.escape(name) + " comanda la facción de " + html.escape(civ) + ". Históricamente recordado por su liderazgo militar y territorial, en el juego sus ejércitos y distritos aplican la ventaja táctica de " + html.escape(ability_name) + " (" + html.escape(ability_desc) + ")." if is_es else "In Shadows of War, " + html.escape(name) + " commands the " + html.escape(civ) + " faction. Renowned for strategic territorial expansion, in live matches this commander wields the " + html.escape(ability_name) + " ability (" + html.escape(ability_desc) + ")."}
            </p>
          </section>

          <section class="leader-card-panel">
            <h2>{html.escape(stats_heading)}</h2>
            <dl class="leader-dossier">
              <dt>{html.escape(civ_label)}</dt>
              <dd>{html.escape(civ)}</dd>

              <dt>{html.escape(historical_name_label)}</dt>
              <dd>{html.escape(historical)}</dd>

              <dt>{html.escape(region_label)}</dt>
              <dd>{html.escape(region)}</dd>

              {f'<dt>{html.escape(wiki_label)}</dt><dd><a href="{wiki_url}" target="_blank" rel="noopener noreferrer" class="inline-link">{wiki_url} ↗</a></dd>' if wiki_url else ''}
            </dl>
          </section>
        </div>
      </div>
    </article>
  </main>

  <footer class="site-footer">
    <div class="container footer-top">
      <a class="brand" href="{root_rel}" aria-label="Shadows of War home"><img class="brand-wordmark" src="/sow-long.svg" alt="Shadows of War" height="18"></a>
      <div class="footer-links">
        <a href="{root_rel}">{home_label}</a>
        <a href="{compendium_url}">{compendium_label}</a>
        <a href="{root_rel}support/">Support</a>
        <a href="{root_rel}privacy/">Privacy</a>
        <a href="{root_rel}terms/">Terms</a>
        <a href="https://discord.gg/d6ZDeChSE" target="_blank" rel="noopener">Discord</a>
        <a href="https://github.com/worldofunreal/shadows-of-war" target="_blank" rel="noopener">GitHub</a>
      </div>
    </div>
    <div class="container footer-bottom">
      <span>© 2026 World of Unreal</span>
    </div>
  </footer>

  <script data-cfasync="false" src="{root_rel}sow-dropdown.js"></script>
  <script data-cfasync="false" src="{root_rel}site-chrome.js"></script>
  <script src="{root_rel}analytics.js"></script>
</body>
</html>
"""


def render_compendium_index(
    all_heroes: list,
    lang: str,
    strings: dict,
    root_rel: str,
) -> str:
    is_es = lang == "es"
    page_title = (
        "Compendio de 80 Líderes y Civilizaciones | Shadows of War MMORTS"
        if is_es
        else "All 80 Historical Leaders & Civilizations | Shadows of War MMORTS"
    )
    meta_desc = (
        "Catálogo completo de los 80 líderes históricos en Shadows of War: bonificaciones militares, rasgos de mando, regiones y biografías tácticas."
        if is_es
        else "Complete compendium of all 80 historical leaders in Shadows of War: military traits, command abilities, civilization bonuses, and tactical guides."
    )

    canonical_url = "https://shadowsofwar.io/leaders/" if lang == "en" else "https://shadowsofwar.io/es/leaders/"
    en_url = "https://shadowsofwar.io/leaders/"
    es_url = "https://shadowsofwar.io/es/leaders/"

    home_label = "Inicio" if is_es else "Home"
    compendium_label = "Líderes" if is_es else "Leaders"
    heading = "Líderes y Civilizaciones" if is_es else "Leaders & Civilizations"
    lede = (
        "Explora los 80 comandantes históricos disponibles en el juego. Cada héroe transforma el mapa con ventajas militares, económicas y de expansión únicas."
        if is_es
        else "Explore all 80 historical commanders playable in Shadows of War. Each hero reshapes the battlefield with unique military, economic, or territorial traits."
    )

    site = strings.get("site", {})
    heroes_dict = strings.get("heroes", {})

    cards_html = []
    for h in all_heroes:
        hid = h["id"]
        art_slug = get_hero_art_slug(h)
        name = heroes_dict.get(f"leader_{hid}_name") or site.get(f"leader_{hid}_name") or h["name"]
        civ = heroes_dict.get(f"leader_{hid}_civilization") or h.get("entity", "")
        ability_name = site.get(f"leader_{hid}_ability") or h.get("hero", {}).get("name", "Trait")
        ability_desc = site.get(f"leader_{hid}_description") or "Special command bonus."
        leader_link = f"{hid}/" if lang == "en" else f"{hid}/"

        cards_html.append(f"""
        <article class="compendium-card">
          <a href="{leader_link}" class="compendium-card-link">
            <picture class="compendium-thumb">
              <source media="(max-width: 700px)" srcset="/assets/shell/leaders/{art_slug}_mobile.webp">
              <img src="/assets/shell/leaders/{art_slug}_desktop.webp"
                   alt="{html.escape(name)} {html.escape(civ)} leader portrait artwork"
                   loading="lazy" width="300" height="375" decoding="async">
            </picture>
            <div class="compendium-card-body">
              <p class="compendium-civ">{html.escape(civ)}</p>
              <h3>{html.escape(name)}</h3>
              <p class="compendium-ability"><strong>{html.escape(ability_name)}:</strong> {html.escape(ability_desc)}</p>
            </div>
          </a>
        </article>
        """)

    cards_str = "\n".join(cards_html)

    return f"""<!doctype html>
<html lang="{lang}" data-theme="dark">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
  <meta name="theme-color" content="#0a0a0e">
  <title>{html.escape(page_title)}</title>
  <meta name="description" content="{html.escape(meta_desc)}">
  <link rel="canonical" href="{canonical_url}">
  <link rel="alternate" hreflang="en" href="{en_url}">
  <link rel="alternate" hreflang="es" href="{es_url}">
  <link rel="alternate" hreflang="x-default" href="{en_url}">
  <meta name="robots" content="index, follow">

  <meta property="og:type" content="website">
  <meta property="og:site_name" content="Shadows of War">
  <meta property="og:title" content="{html.escape(page_title)}">
  <meta property="og:description" content="{html.escape(meta_desc)}">
  <meta property="og:url" content="{canonical_url}">
  <meta property="og:image" content="https://shadowsofwar.io/assets/shell/loader/sow-splash-desktop.webp">

  <meta name="twitter:card" content="summary_large_image">
  <meta name="twitter:title" content="{html.escape(page_title)}">
  <meta name="twitter:description" content="{html.escape(meta_desc)}">

  <link rel="icon" type="image/svg+xml" href="{root_rel}sow.svg">
  <link rel="stylesheet" href="{root_rel}fonts/fonts.css">
  <link rel="stylesheet" href="{root_rel}styles.css">
</head>
<body class="guide-page">
  <!-- SOW_SITE_HEADER -->

  <main class="guide-main">
    <div class="container">
      <nav class="guide-breadcrumbs" aria-label="Breadcrumb">
        <ol>
          <li><a href="{root_rel}">{home_label}</a></li>
          <li aria-current="page">{compendium_label}</li>
        </ol>
      </nav>

      <section class="guide-hero section-rule">
        <p class="eyebrow"><span class="eyebrow-dot"></span> 80 Playable Commanders</p>
        <h1>{html.escape(heading)}</h1>
        <p class="hero-lede">{html.escape(lede)}</p>
      </section>

      <div class="compendium-grid">
        {cards_str}
      </div>
    </div>
  </main>

  <footer class="site-footer">
    <div class="container footer-top">
      <a class="brand" href="{root_rel}" aria-label="Shadows of War home"><img class="brand-wordmark" src="/sow-long.svg" alt="Shadows of War" height="18"></a>
      <div class="footer-links">
        <a href="{root_rel}">{home_label}</a>
        <a href="{root_rel}support/">Support</a>
        <a href="{root_rel}privacy/">Privacy</a>
        <a href="{root_rel}terms/">Terms</a>
        <a href="https://discord.gg/d6ZDeChSE" target="_blank" rel="noopener">Discord</a>
        <a href="https://github.com/worldofunreal/shadows-of-war" target="_blank" rel="noopener">GitHub</a>
      </div>
    </div>
    <div class="container footer-bottom">
      <span>© 2026 World of Unreal</span>
    </div>
  </footer>

  <script data-cfasync="false" src="{root_rel}sow-dropdown.js"></script>
  <script data-cfasync="false" src="{root_rel}site-chrome.js"></script>
  <script src="{root_rel}analytics.js"></script>
</body>
</html>
"""


def main():
    with open(CATALOG_PATH, "r", encoding="utf-8") as f:
        catalog = json.load(f)

    heroes = [e for e in catalog["entities"] if e.get("hero")]
    print(f"Building encyclopedic pages for {len(heroes)} heroes...")

    # Load strings
    en_strings = load_strings("en")
    es_strings = load_strings("es")

    # 1. English Canonical (/leaders/<id>/ and /leaders/index.html)
    en_base_dir = SITE_DIR / "leaders"
    en_base_dir.mkdir(parents=True, exist_ok=True)

    en_compendium_html = render_compendium_index(heroes, "en", en_strings, "../")
    (en_base_dir / "index.html").write_text(en_compendium_html, encoding="utf-8")

    for h in heroes:
        hid = h["id"]
        hdir = en_base_dir / hid
        hdir.mkdir(parents=True, exist_ok=True)
        page_html = render_leader_page(h, "en", en_strings, heroes, "../../")
        (hdir / "index.html").write_text(page_html, encoding="utf-8")

    # 2. Spanish Localized (/es/leaders/<id>/ and /es/leaders/index.html)
    es_base_dir = SITE_DIR / "es" / "leaders"
    es_base_dir.mkdir(parents=True, exist_ok=True)

    es_compendium_html = render_compendium_index(heroes, "es", es_strings, "../../../")
    (es_base_dir / "index.html").write_text(es_compendium_html, encoding="utf-8")

    for h in heroes:
        hid = h["id"]
        hdir = es_base_dir / hid
        hdir.mkdir(parents=True, exist_ok=True)
        page_html = render_leader_page(h, "es", es_strings, heroes, "../../../../")
        (hdir / "index.html").write_text(page_html, encoding="utf-8")

    print(f"Successfully generated {len(heroes)} English pages and {len(heroes)} Spanish pages.")


if __name__ == "__main__":
    main()
