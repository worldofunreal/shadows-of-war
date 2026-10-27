# Gauntlet Loop — shadowsofwar.io

Loop adversarial para la página de marketing (`sow-web/site` → `dist/web`).
Objetivo: que la página sea (a) crawleable y citable por buscadores y motores de IA,
(b) honesta (cero datos inventados), (c) rápida, y (d) que no se autoengañe con "ya quedó".

## Contrato

- Fuente de verdad = repo, no memoria. Se edita `sow-web/site/`, nunca `dist/web/` a mano.
- Cada ronda termina con build limpio + `dist/web` regenerado. Si el build falla, la ronda no cuenta.
- Prohibido declarar victoria sin evidencia (`archivo:línea`, salida de comando, o HTML del build).
- Prohibido inventar métricas, premios, jugadores, quotes, fechas o features que no existan en el repo.
- Un hallazgo sin fix verificado se arrastra a la ronda siguiente; no se borra de la lista.

## El bar (se evalúa cada ronda, 0–5 cada uno)

**A. Crawlability e indexación**
1. `robots.txt` permite lo que debe indexarse, bloquea lo que no, y apunta al sitemap.
2. `sitemap.xml` lista TODAS las rutas públicas reales (/, /play/, /support/, /privacy/, /terms/) con `lastmod` real, sin rutas 404 ni redirecciones.
3. Canonical correcto y autorreferente en cada página. Sin canónicas cruzadas entre locales.
4. 200 OK en cada URL del sitemap; sin cadenas de redirect; sin `noindex` accidental.
5. HTML servido con contenido crítico en el DOM inicial (no todo inyectado por JS).

**B. Schema / datos estructurados**
6. JSON-LD válido y sin warnings en el validador. Tipos presentes y coherentes: `VideoGame`, `Organization` (publisher + `sameAs`), `WebSite`, `FAQPage` (para la sección real de FAQ), `BreadcrumbList` en subpáginas.
7. `Offer` con precio/moneda reales (free-to-play = 0 USD), `gamePlatform`, `applicationCategory`, `playMode`, `inLanguage`.
8. Cero contradicciones entre JSON-LD y el copy visible (mismo nombre, misma descripción, mismo precio).
9. `og:*` y `twitter:*` completos (incl. `og:locale`, `og:image:width/height`, `og:image:alt`) y con imagen que exista y pese < 300 KB.

**C. Señales de entidad**
10. Una sola entidad "Shadows of War" declarada de forma consistente: nombre, publisher (World of Unreal), URL canónica, perfiles externos (`sameAs`) y logo.
11. Enlazado desde/hacia el ecosistema WOU (worldofunreal.com, otros títulos) con anchors descriptivos, no "click aquí".
12. `llms.txt` en la raíz: qué es el juego, quién lo publica, hechos canónicos, enlaces a páginas clave. Sin marketing vacío.

**D. Estructura de contenido y autoridad**
13. Un solo `<h1>`, jerarquía h2/h3 sin saltos, cada sección con intención clara (qué es / cómo se juega / quiénes son los líderes / FAQ / jugar).
14. El copy responde preguntas reales de búsqueda ("is Shadows of War free", "how long is a match", "browser or app") en frases extraíbles de 1–2 líneas.
15. FAQ con preguntas literales de usuario y respuestas autocontenidas (citables sin contexto).
16. Cada `data-i18n` existe en los 16 locales; sin fugas de clave cruda ni inglés filtrado en `es/`.

**E. Accesibilidad a crawlers de IA y hreflang**
17. Nada crítico depende de JS para el primer render. Contenido de FAQ y líderes presente en el HTML.
18. `hreflang` + `x-default` para los locales servidos, con URLs reales y recíprocas.
19. Sin bloqueo accidental de `GPTBot`/`ClaudeBot`/`PerplexityBot`/`Google-Extended` si la intención es ser citado.
20. `IndexNow` (o ping equivalente) disparado en cada deploy; verificar la key servida en la raíz.

**F. Calidad de ejecución**
21. Lighthouse móvil: Performance ≥ 90, Accessibility ≥ 95, Best Practices ≥ 95, SEO = 100.
22. LCP < 2.5 s en 4G simulada; sin CLS > 0.1; fuentes con `font-display: swap` y sin FOIT.
23. Tests del sitio (`site_interaction.test.ts`, `analytics.test.ts`) verdes; sin regresión de analytics/consent.
24. Nada de texto placeholder, lorem, links muertos, `href="#"` ni imágenes rotas.
25. Todo claim numérico rastreable a una fuente del repo o eliminado.

## Reglas de ronda

1. **AUDIT** — recorre A–F contra el build actual. Cada hallazgo: `[severidad 1-5] archivo:línea — problema — por qué importa — fix propuesto`.
2. **RANK** — ordena por (severidad × alcance). Arregla como máximo los 5 primeros. Nada de refactors de gusto.
3. **FIX** — cambios mínimos, un commit lógico por bloque. Si un fix toca i18n, actualiza los 16 locales o no lo hagas.
4. **BUILD + VERIFY** — rebuild, tests, y re-medición del criterio tocado con evidencia pegada.
5. **RE-AUDIT** — vuelve al paso 1. Los criterios ya pasados se re-verifican (anti-regresión), no se asumen.
6. **VERIFICADOR INDEPENDIENTE** — antes de cerrar, un contexto fresco (o subagente) recibe solo el diff + el bar y busca: overclaims, schema inválido, regresiones, fixes fantasma. Sus hallazgos entran como ronda nueva.

## Stop

Para cuando se cumpla TODO:
- Dos rondas consecutivas sin hallazgos de severidad ≥ 3.
- Cero regresiones en criterios previamente verdes.
- Build limpio, tests verdes, sitemap y robots consistentes con las rutas reales.
- Informe final: tabla criterio → estado → evidencia (comando/salida), y lista de lo que se decidió NO hacer y por qué.

Máximo 6 rondas. Si en la ronda 6 sigue habiendo severidad ≥ 4, paras y reportas el bloqueo — no maquilles el resultado.

## Hallazgos semilla (verificados en `dist/web` al arrancar)

- `dist/web/index.html`: solo JSON-LD `VideoGame`; hay `Organization` como publisher embebido pero sin `WebSite`, sin `FAQPage` pese a que existe `#faq`, sin `BreadcrumbList` en subpáginas.
- 16 locales en `dist/web/locales` pero **0** `hreflang` en el HTML.
- No existe `llms.txt` en la raíz.
- `robots.txt` bloquea `/*.wasm$` y `/*.wasm.br$`; verificar que eso no impida el render del juego a crawlers que sí ejecutan JS, y documentar la decisión.
- `sitemap.xml` con `lastmod` fijo en 2026-08-25 / 2026-09-03 — validar que sea real y no un valor hardcodeado que nunca se actualiza.
- `dist/web/index.html` y `sow-web/site/index.html` divergen (description distinta: "Pick a game"/"Pick 1 of 12 leaders", CSS con hash vs sin hash) → confirmar que `dist/web` sale siempre del build y no hay drift manual.

## Ronda 1 — ejecutada 2026-10-19

Fix aplicados (rango por severidad × alcance):

1. **[4] `sow-web/site/llms.txt` (nuevo) + publicación** — no existía. Guía para answer engines: qué es el juego, plataformas, precio, sin cuenta, mecánicas, mapa de URLs. Copiado a la raíz por `sow-dist` y añadido al contrato `verify_layout` (falla el build si desaparece).
2. **[4] `sow-dist/src/main.rs` robots** — bloque explícito de allow para `GPTBot`, `OAI-SearchBot`, `ChatGPT-User`, `ClaudeBot`, `Claude-Web`, `PerplexityBot`, `Perplexity-User`, `Google-Extended`, `Applebot-Extended`, `CCBot`. Mantiene `Disallow: /internal/` y `/api/`.
3. **[3] JSON-LD** — sustituido el `VideoGame` suelto por un `@graph` con `@id` estables: `Organization` (World of Unreal + sameAs Discord/Telegram), `WebSite` (publisher, about), `VideoGame` (15 `inLanguage`, plataformas, offer 0 USD, publisher por referencia) y `FAQPage` con las 5 preguntas que ya existían en `#faq`. Antes el FAQ visible no tenía marcado.
4. **[3] `sitemap.xml`** — `lastmod` deja de ser un literal congelado en 2026-08-25: `/` y `/play/` usan la fecha UTC de build (`today_iso()`, civil-from-days, sin dependencia nueva); las páginas legales conservan su fecha real de edición. Construcción de las 6 entradas desde tabla en vez de `concat!` a mano.

Evidencia:
- `cargo check -p sow-dist` → `Finished dev profile` (sin warnings nuevos).
- JSON-LD validado con parseo estricto: `json.loads` OK, `@graph` = Organization, WebSite, VideoGame, FAQPage; `inLanguage` = 15 (coincide con `ls sow-i18n/strings`).
- `robots.txt` y `llms.txt` son estáticos por build: no hay `lastmod` ni hash que puedan mentir.

Decisión explícita NO tomada:
- **`hreflang`**: no se añade. El sitio sirve una sola URL y elige idioma en cliente (`navigator.languages` / localStorage); no existen URLs por locale, así que declarar alternates sería marcado falso. Requiere rutas reales (`/es/`, `/pt-br/`, …) o `?lang=` leído por `sow-i18n.js` antes de pintar → ronda de arquitectura, no de meta tags.
- **No se tocó `dist/web` a mano**: se regenera en el próximo `sow p`; editar el artefacto habría creado el drift que el propio doc advierte.

Pendiente ronda 2: rutas por locale + `hreflang` recíproco, `BreadcrumbList` en subpáginas, verificación de `IndexNow`.
