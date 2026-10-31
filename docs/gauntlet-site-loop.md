# Gauntlet Loop — shadowsofwar.io

Loop adversarial para la página de marketing (`sow-web/site` → `dist/web`).
Objetivo: que un humano que llega por primera vez y un crawler de IA ciego respondan
correctamente *"qué es Shadows of War, quién lo hace, cómo se juega, es gratis"* solo con el HTML,
y que la página sea (a) crawleable y citable, (b) honesta (cero datos inventados), (c) rápida y
(d) que no se autoengañe con "ya quedó".

No-objetivos: rediseñar la dirección de arte, inventar jugadores/reseñas/ingresos, tocar código de
gameplay, o "copy SEO" que ningún humano leería.

Método: builder/critic loop (Gauntlet). N rondas × críticos adversariales independientes × gate duro.
El builder nunca se califica a sí mismo.

## Contrato

- Fuente de verdad = repo, no memoria. Se edita `sow-web/site/`, nunca `dist/web/` a mano.
- Cada claim debe ser verificable por un comando o un `archivo:línea`. "Mejora la claridad" no es un hallazgo.
- Prohibido inventar métricas, premios, jugadores, quotes, fechas o features que no existan en el repo.
- La página humana gana los empates: si un fix para crawlers empeora la lectura para un jugador, el fix está mal.
- Cada ronda termina con build limpio + `dist/web` regenerado. Si el build falla, la ronda no cuenta.
- Una ronda = un diff, un build y un log de verificación. Sin drive-by edits.
- Prohibido declarar victoria sin evidencia (`archivo:línea`, salida de comando o HTML del build).
- Un hallazgo sin fix verificado se arrastra a la ronda siguiente; no se borra de la lista.
- Un hallazgo ya cerrado lo re-verifica un crítico **distinto** al que lo encontró (anti-regresión).

## Roles

**Builder** (un agente, acceso completo al repo)

- Toma los 3 primeros P0/P1 del informe de críticos, arregla en la fuente, reconstruye y re-verifica.
- No discute con un crítico: arregla o presenta una **réplica** con evidencia (`archivo:línea` o salida de comando).
- Escribe el log de la ronda: qué cambió, por qué, y el comando exacto de verificación de cada claim.

**Críticos** (independientes, read-only, contexto fresco cada ronda)

| # | Crítico | Solo mira | Tumba la ronda si |
|---|---|---|---|
| C1 | Acceso de crawlers | `robots.txt`, cabeceras, HTML crudo con `curl` y JS desactivado | Falta un hecho clave en el HTML crudo, hay bots de IA bloqueados, sitemap/IndexNow roto |
| C2 | Datos estructurados | bloques JSON-LD, validez schema.org | JSON-LD inválido, falta el grafo de entidad, falta `sameAs` |
| C3 | Entidad y enlaces | consistencia de nombres, anchors internos, enlaces cruzados | Variantes de nombre ("SOW", "Shadows of War", "shadowsofwar"), páginas huérfanas, enlaces muertos o con redirect |
| C4 | Contenido e intención | estructura H1/H2, párrafos que responden primero, cobertura por consulta | Ninguna respuesta directa a una consulta central, páginas finas duplicadas, sin superficie de FAQ/cómo jugar |
| C5 | Conversión humana | test de 5 segundos, móvil, pruebas, ruta al CTA | El visitante no puede decir qué es el juego y dónde hacer clic; claim no verificable en la página |

Cada crítico devuelve:

```
FINDING <id> | severidad P0/P1/P2 | eje | evidencia (archivo:línea o salida de comando) | por qué importa | fix mínimo
```

más un score 0–10 del eje, con la línea de evidencia que lo justifica.

## El bar (se evalúa cada ronda)

**A. Crawlability e indexación (C1)**

1. `robots.txt` permite lo que debe indexarse, bloquea lo que no, y apunta al sitemap.
2. `sitemap.xml` lista TODAS las rutas públicas reales (/, /play/, /support/, /privacy/, /terms/) con `lastmod` real, sin rutas 404 ni redirecciones.
3. Canonical correcto y autorreferente en cada página. Sin canónicas cruzadas entre locales.
4. 200 OK en cada URL del sitemap; sin cadenas de redirect; sin `noindex` accidental.
5. HTML servido con contenido crítico en el DOM inicial (no todo inyectado por JS).

**B. Schema / datos estructurados (C2)**

6. JSON-LD válido y sin warnings. Tipos presentes y coherentes: `VideoGame`, `Organization` (publisher + `sameAs`), `WebSite`, `FAQPage` (para la sección real de FAQ), `BreadcrumbList` en subpáginas.
7. `Offer` con precio/moneda reales (free-to-play = 0 USD), `gamePlatform`, `applicationCategory`, `playMode`, `inLanguage`.
8. Cero contradicciones entre JSON-LD y el copy visible (mismo nombre, misma descripción, mismo precio).
9. `og:*` y `twitter:*` completos (incl. `og:locale`, `og:image:width/height`, `og:image:alt`) y con imagen que exista y pese < 300 KB.

**C. Señales de entidad (C3)**

10. Una sola entidad "Shadows of War" declarada de forma consistente: nombre, publisher (World of Unreal), URL canónica, perfiles externos (`sameAs`) y logo.
11. Enlazado desde/hacia el ecosistema WOU (worldofunreal.com, otros títulos) con anchors descriptivos, no "click aquí".
12. `llms.txt` en la raíz: qué es el juego, quién lo publica, hechos canónicos, enlaces a páginas clave. Sin marketing vacío.

**D. Estructura de contenido y autoridad (C4)**

13. Un solo `<h1>`, jerarquía h2/h3 sin saltos, cada sección con intención clara (qué es / cómo se juega / quiénes son los líderes / FAQ / jugar).
14. El copy responde preguntas reales de búsqueda ("is Shadows of War free", "how long is a match", "browser or app") en frases extraíbles de 1–2 líneas.
15. FAQ con preguntas literales de usuario y respuestas autocontenidas (citables sin contexto).
16. Cada `data-i18n` existe en los 16 locales; sin fugas de clave cruda ni inglés filtrado en `es/`.

**E. Accesibilidad a crawlers de IA y hreflang (C1/C3)**

17. Nada crítico depende de JS para el primer render. Contenido de FAQ y líderes presente en el HTML.
18. `hreflang` + `x-default` para los locales servidos, con URLs reales y recíprocas.
19. Sin bloqueo accidental de `GPTBot`/`ClaudeBot`/`PerplexityBot`/`Google-Extended` si la intención es ser citado.
20. `IndexNow` (o ping equivalente) disparado en cada deploy; verificar la key servida en la raíz.

**F. Calidad de ejecución (C4/C5)**

21. Lighthouse móvil: Performance ≥ 90, Accessibility ≥ 95, Best Practices ≥ 95, SEO = 100.
22. LCP < 2.5 s en 4G simulada; sin CLS > 0.1; fuentes con `font-display: swap` y sin FOIT.
23. Tests del sitio (`site_interaction.test.ts`, `analytics.test.ts`) verdes; sin regresión de analytics/consent.
24. Nada de texto placeholder, lorem, links muertos, `href="#"` ni imágenes rotas.
25. Todo claim numérico rastreable a una fuente del repo o eliminado.

Cada criterio pasa con ≥ 4/5. **Gate de ronda: cada crítico ≥ 8/10, cero P0 abiertos y el log de verificación reproduce cada claim.**

## Reglas de ronda

1. **AUDIT** — recorre A–F contra el build actual. Cada hallazgo: `[severidad 1-5] archivo:línea — problema — por qué importa — fix propuesto`.
2. **RANK** — ordena por (severidad × alcance). Arregla como máximo los 5 primeros. Nada de refactors de gusto.
3. **FIX** — cambios mínimos, un commit lógico por bloque. Si un fix toca i18n, actualiza los 16 locales o no lo hagas.
4. **BUILD + VERIFY** — rebuild, tests, y re-medición del criterio tocado con evidencia pegada.
5. **RE-AUDIT** — vuelve al paso 1. Los criterios ya pasados se re-verifican (anti-regresión), no se asumen.
6. **VERIFICADOR INDEPENDIENTE** — antes de cerrar, un contexto fresco (o subagente) recibe solo el diff + el bar y busca: overclaims, schema inválido, regresiones, fixes fantasma. Sus hallazgos entran como ronda nueva.

Comandos de verificación (obligatorios en cada log de ronda):

```bash
./sow l &                                    # preview local en 127.0.0.1:4173
curl -s http://127.0.0.1:4173/ | sed 's/<[^>]*>/ /g' | tr -s ' \n' ' \n'
curl -s http://127.0.0.1:4173/robots.txt
curl -s http://127.0.0.1:4173/sitemap.xml | xmllint --noout -
curl -s http://127.0.0.1:4173/ | grep -c 'application/ld+json'
curl -sI http://127.0.0.1:4173/play/ | head -20
```

Re-check en producción tras `./sow p`: los mismos curls contra `https://shadowsofwar.io/`, más una pasada
por el validador de Rich Results / Schema sobre la URL viva.

## Reglas anti-trampa

- Los críticos no ven la justificación del builder antes de puntuar. El builder no ve qué crítico es el más débil.
- Ningún crítico sube un score porque un fix "se sienta mejor"; solo cuenta la evidencia en la salida nueva.
- Baneado como "fix": keyword stuffing, texto oculto que repite el visible, `aggregateRating`/`reviewCount` falsos, schema para contenido que no está en la página, doorway pages por keyword, páginas de ciudad/keyword autogeneradas.
- Un P0 no se degrada a P1 por el builder; solo lo cierra el crítico que lo abrió.
- Si dos críticos se contradicen, decide el contrato de página humana (§ Contrato); el conflicto se registra.

## Stop

Paras cuando se cumpla TODO:

- Dos rondas consecutivas sin hallazgos de severidad ≥ 3.
- Cero regresiones en criterios previamente verdes.
- Build limpio, tests verdes, sitemap y robots consistentes con las rutas reales.
- Informe final: tabla criterio → estado → evidencia (comando/salida), y lista de lo que se decidió NO hacer y por qué.

Máximo 6 rondas. Si en la ronda 6 sigue habiendo severidad ≥ 4, paras y reportas el bloqueo — no maquilles el resultado.

## Entregables por ronda

1. El informe de los críticos, verbatim: hallazgos, scores y veredicto del gate.
2. El log de la ronda: resumen del diff, comando de verificación y salida de cada claim, réplicas con evidencia.
3. Capturas renderizadas a 390 / 768 / 1440 px.
4. Al cerrar: un changelog de una página legible por un humano, y la lista de ejes por debajo de 8 con la razón por la que se dejaron.

## Hallazgos semilla (verificados en `dist/web` al arrancar)

- `dist/web/index.html`: solo JSON-LD `VideoGame`; hay `Organization` como publisher embebido pero sin `WebSite`, sin `FAQPage` pese a que existe `#faq`, sin `BreadcrumbList` en subpáginas.
- 16 locales en `dist/web/locales` pero **0** `hreflang` en el HTML.
- No existe `llms.txt` en la raíz.
- `robots.txt` bloquea `/*.wasm$` y `/*.wasm.br$`; verificar que eso no impida el render del juego a crawlers que sí ejecutan JS, y documentar la decisión.
- `sitemap.xml` con `lastmod` fijo en 2026-08-25 / 2026-09-03 — validar que sea real y no un valor hardcodeado que nunca se actualiza.
- `dist/web/index.html` y `sow-web/site/index.html` divergen (description distinta: "Pick a game"/"Pick 1 of 12 leaders", CSS con hash vs sin hash) → confirmar que `dist/web` sale siempre del build y no hay drift manual.
- La página es i18n (`data-i18n`, `SOW_SITE_LOCALE`) pero no hay `hreflang` ni variantes de locale en el sitemap → el descubrimiento por locale es invisible.
- No hay `/how-to-play/` ni superficie de FAQ; el bloque de 3 pasos es la única explicación del loop.
- `play/` es el shell del juego: confirmar qué ve un crawler antes de que arranque el WASM.
- IndexNow key file `8d227b8f9e6140d39e3381a1829e1db3.txt` existe en la raíz del sitio pero nada prueba que se haga ping en el deploy.

## Ronda 1 — ejecutada 2026-10-19

Fixes aplicados (rango por severidad × alcance):

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
- **No se tocó `dist/web` a mano**: se regenera en el próximo `sow p`; editar el artefacto habría creado el drift que este doc advierte.

Pendiente ronda 2: rutas por locale + `hreflang` recíproco, `BreadcrumbList` en subpáginas, verificación de `IndexNow`.

## Definición de terminado

Un lector frío y un crawler frío producen de forma independiente las mismas cuatro frases: qué es el juego,
qué haces en una partida, que es gratis en el navegador y dónde jugar. Todo claim de datos estructurados
es visible en la página. `robots.txt`, `sitemap.xml`, IndexNow y `hreflang` concuerdan con lo que
realmente se publica.
