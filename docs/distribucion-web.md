# Distribución web: inventario, peso y optimización

Auditoría del 2 de noviembre de 2026; paquete Poki reconstruido a las 21:12. Los bytes de paquetes se miden sobre los archivos, no sobre `du` redondeado. MiB usa 1,048,576 bytes; MB usa 1,000,000.

## Resumen para subir

- **Poki:** los assets estáticos de runtime van dentro del paquete. Poki bloquea las solicitudes externas por defecto y pide incluir assets, fuentes y librerías en el build. No encontré un máximo numérico público para el peso total del build. Sí exige un paquete pequeño, recomienda carga progresiva y advierte que muchos jugadores abandonan si la carga tarda más de 10 segundos. El [Poki Inspector](https://developers.poki.com/guide/inspector) reporta peso/tiempo de carga y avisos de optimización; ese es el control práctico, no una cifra fija de “legal”. Fuentes: [requisitos](https://developers.poki.com/guide/requirements-quality), [recursos externos](https://developers.poki.com/guide/external-resources-policy), [carga progresiva](https://developers.poki.com/guide/easy-access).
- **Paquete Poki actual:** `dist/poki` contiene **64,959,867 B (64.96 MB / 61.95 MiB), 1,532 archivos**. Incluye **49,821,547 B (49.82 MB / 47.51 MiB) de assets**, incluidos los avatares; también 2,600,395 B de mapas, 8,597,275 B de WASM, 1,510,997 B de locales y el shell/HTML. Un ZIP DEFLATE 9 de esa carpeta mide **58,024,838 B (58.02 MB / 55.34 MiB)**. El ZIP es una medición comprimida local; aún no se ha medido la transferencia del portal. La cifra anterior de **54.60 MiB** era una proyección de ZIP al sustituir avatares dentro de un candidato viejo, no el tamaño del paquete actual.
- **Fuentes vs. paquete:** el árbol fuente completo `assets/` pesa **477,579,633 B (477.58 MB / 455.46 MiB)**. No se sube tal cual: el paquete selecciona y transforma lo que necesita el runtime Poki. Los assets publicados ocupan 49.82 MB; los avatares, 22.11 MB.
- **Build fresco:** `./sow poki` terminó correctamente el 2 nov. a las 21:12. Regeneró el staging, recompiló el WASM con los shaders actuales, exportó los 15 idiomas, copió y verificó inventarios de assets/mapas y pasó `verify_poki_layout`. Revisé además que el catálogo embebido enumera los 1,160 WebP de avatares del paquete sin faltantes ni sobrantes y que `index.html` usa `./assets` y `./maps`, sin rutas estáticas a SOW. El intento anterior sí había fallado por el icono de Abd; ya se añadió un recorte provisional de su propia expresión Calm, byte idéntico al recorte generado del sheet. El arte final de habilidad sigue pendiente.
- **Paquete anterior en caché:** **90,337,078 B (90.34 MB / 86.15 MiB), 1,441 archivos**; su ZIP DEFLATE 9 pesó **83,486,770 B (83.49 MB / 79.62 MiB)**. Era una salida vieja: contenía 75,310,577 B de assets y 48,250,178 B de avatares antiguos. No representa el paquete actual.
- **Auditoría del empaquetador:** `copy_poki_assets()` copia el árbol runtime y valida inventario y hashes. `package_poki()` arma y valida en un directorio temporal, y solo sustituye `dist/poki` si el bundle pasa todos los verificadores; una falla no deja un paquete parcial con nombre de subida. La ejecución actual ejercitó ese flujo completo.
- **Poki no publica un máximo numérico total** en sus requisitos revisados. El tamaño por sí solo no demuestra aprobación ni rechazo; falta revisar la subida con Poki Inspector y medir la descarga que presenta esa herramienta. La carpeta, el ZIP y los bytes que el jugador descarga al inicio son mediciones distintas.
- **CrazyGames:** su tope de paquete es 250 MB y 1,500 archivos; el máximo inicial es 50 MB hasta `Gameplay start` (20 MB para optar a portada móvil). Eso mide transferencia hasta que el juego ya es jugable, no el ZIP que se sube. CrazyGames sirve por su CDN los archivos del Developer Portal. Fuentes: [límites técnicos](https://docs.crazygames.com/requirements/technical/) y [CDN del portal](https://docs.crazygames.com/faq/). El paquete Poki de 1,532 archivos no sirve para inferir si el paquete CrazyGames cumple su límite de 1,500.
- **Paquete CrazyGames:** el `./sow p` del 10 nov. (16:03) dejó `dist/crazygames` en **101,116,147 B, 401 archivos**: líderes 49.0 MB (1600×900 / 900×1600), avatares 17.3 MB en 19 hojas q90, campaña 11.1 MB (WebM + MP4), atlas de habilidades 3.2 MB sin pérdida y WASM plano 9.1 MB. Desde entonces el empaquetador publica líderes a 1280×720 / 720×1280 (`cg_leader_*`), hojas de avatar a q85, atlas de habilidades lossy q75 (`ability_atlas`) y campaña solo MP4; el estimado es ~74 MB. `verify_cg_layout` falla si el paquete pasa de `CG_BUNDLE_BYTE_BUDGET` (85 MB). `package_cg()` solo corre dentro de `./sow p`; falta medir el siguiente.
- **Subida:** usa `dist/poki`, que contiene `index.html` y pasó el verificador de layout. La configuración de `@poki/cli` está en `sow-dist/poki.json`; ejecuta el uploader desde `sow-dist`, donde `../dist/poki` resuelve al paquete. No subas `dist/`, que además incluye cachés y artefactos de compilación.

`./sow poki` genera el paquete local, sin desplegar ni subir. El catálogo de héroes exige un icono de habilidad por héroe. Para completar el build se registró el icono provisional de Abd como recorte de su propia hoja Calm; está marcado como provisional en `assets/SOURCES.toml` y debe reemplazarse cuando llegue el arte final. CrazyGames también necesita el icono para construir su atlas.

## De qué se compone el paquete Poki

Las filas son categorías disjuntas y suman los **64,959,867 B** del paquete; el porcentaje es sobre el paquete entero.

| Contenido | Archivos | Bytes | % | Formato / lectura |
| --- | ---: | ---: | ---: | --- |
| Avatares | 1,160 | 22,107,362 | 34.03% | WebP; retratos, sheets y expresiones. |
| Escenas de héroes | 162 | 13,855,762 | 21.33% | WebP reescalado y recomprimido para el portal. |
| WASM | 1 | 8,597,275 | 13.23% | WASM plano dentro del paquete. El staging tiene `.wasm.br` de 3,760,724 B, pero no se incluye. |
| Campaña | 10 | 5,564,015 | 8.57% | Incluye un MP4 de 5,412,982 B y sus poster/triggers. |
| Sprites de unidades | 5 | 3,589,031 | 5.52% | PNG; `sam-missile.png` pesa 2,649,061 B y `atom-bomb.png` 939,608 B. |
| Música | 1 | 3,215,191 | 4.95% | WebM/Opus. |
| Mapas | 46 | 2,600,395 | 4.00% | Incluye 22 mapas `.br` que suman 2,536,902 B; lo demás son miniaturas y catálogos. |
| `index.html` | 1 | 1,774,548 | 2.73% | Incrusta unos 861 KB del bundle JS del menú/manifiesto del atlas, 397 KB de CSS y 125 KB de loader/catálogos/hash de assets. |
| Locales | 16 | 1,510,997 | 2.33% | JSON de idioma, almacenado sin Brotli en la carpeta. |
| Resto | 130 | 2,145,291 | 3.30% | Iconos/atlas, fuentes, runtime JS/CSS, manifiestos y avisos. |

Los **1,442 WebP suman 37,531,548 B**; junto con el MP4, el WebM y los mapas `.br`, son formatos que ya llevan compresión y por eso apenas se reducen al comprimir el paquete otra vez. El ZIP local pasa de 64,959,867 a 58,024,838 B: ahorra 6,935,029 B (10.68%), no la mitad. Por eso el peso se concentra en medios ya comprimidos y el WASM, no en copias crudas de todo `assets/`.

El WASM sí es una oportunidad de **transferencia**: el `.wasm.br` que ya genera el staging es 4,836,551 B menor que el WASM plano. Ese archivo no está en `dist/poki`; aprovecharlo depende de que el host del portal lo sirva como `Content-Encoding: br`. Tampoco medimos aún los headers del portal, así que el ZIP y el tamaño de carpeta no equivalen a los bytes que recibe el jugador.

## Quién publica qué

`assets/art/` es la fuente editable. `stage_runtime_assets()` en `sow-dist/src/main.rs` crea el árbol de runtime `assets/gameplay` y `assets/shell`; luego cada empaquetador elige y transforma lo que se sirve:

| Destino | Flujo actual |
| --- | --- |
| Web propia | `./sow p` publica los assets en el CDN de SOW y la web los pide desde `/assets` y `/maps`. |
| Poki | `package_poki()` copia los assets runtime y mapas a `dist/poki`; el HTML usa `./assets` y `./maps`. Poki exige assets en el build, así que no se apuntan al CDN de SOW. |
| CrazyGames | `package_cg()` empaqueta assets runtime, fuentes y mapas; las rutas relativas `./assets` y `./maps` hacen que CrazyGames los sirva por su CDN. Agrupa iconos de habilidad y avatares en hojas para respetar el límite de archivos; publica líderes a 1280×720 y campaña solo en MP4. |
| Jest | `package_jest()` empaqueta assets runtime y mapas en `dist/jest.zip`; sirve como paquete autocontenido de prueba. |

**Estrategia de CDN:** para CrazyGames, sí: subir los assets al portal y dejar que su CDN los entregue ahorra ancho de banda estático de SOW; no reduce los bytes que descarga el jugador. Para Poki, incluirlos en el build es requisito y evita pedirlos a nuestro dominio; la documentación pública revisada no especifica qué CDN interno usa Poki. En ambos portales, los endpoints de API y multiplayer no son assets: siguen apuntando a SOW y requieren la aprobación/integración aplicable del portal. No se debe usar el CDN de SOW como atajo para saltarse la política de assets externos de Poki.

El paquete CrazyGames incluye assets runtime, fuentes y mapas. Esto reduce el egress estático de SOW, no el volumen total que descarga cada jugador. El evento `Gameplay start` debe coincidir con el primer estado jugable. Falta medir el tráfico real hasta ese evento.

Las caras de la lista de héroes se piden al acercarse a la vista (`watchAvatarMarkup`, `IntersectionObserver`), así que la lista baja solo las hojas visibles. El preload del loader usa `./assets` servido por CrazyGames; el verificador exige imágenes locales y rechaza URLs `shadowsofwar.io/assets/`. Falta medir el tráfico de red real antes de `Gameplay start`.

## Inventario medido

| Árbol | Archivos | Tamaño | Estado y alcance |
| --- | ---: | ---: | --- |
| `assets/` completo | 1,228 | 477,579,633 B / 455.46 MiB | Archivos de fuente del juego; no es el contenido que se sube tal cual. El paquete Poki publicado tiene 49,821,547 B de assets runtime. |
| `assets/art` | 1,083 | 406,863,694 B / 388.02 MiB | Arte fuente completo de trabajo; Poki no sube esta carpeta tal cual. |
| `assets/art/avatars` | 755 | 271,001,774 B / 258.45 MiB | Originales de trabajo; no se suben tal cual. |
| `assets/art/buildings` | 20 | 51,465,992 B / 49.08 MiB | PNG fuente para 5 tipos × 4 niveles. |
| Atlas de edificios runtime | 2 | 498,178 B / 0.48 MiB | WebP lossless de 493,522 B más manifiesto de 4,656 B; 128 px por celda. Los SHA-256 del generado, staging y último paquete Poki coinciden; sustituye los 20 PNG fuente. |
| `assets/art/heroes` | 162 | 72,256,450 B / 68.91 MiB | Originales de escenas; Poki reescala y recomprime al empaquetar. |
| `assets/art/icons/abilities` | 81 | 146,404 B / 0.14 MiB | Iconos fuente actuales. El de Abd es un recorte provisional de su hoja Calm y está pendiente de arte final. |
| `assets/maps` | 91 | 47,685,933 B / 45.48 MiB | Árbol de trabajo con `.bin` y `.br`; no equivale al mapa que se transfiere. |
| Mapas binarios por portal | 22 | 2,536,902 B / 2.42 MiB | Los mismos 22 mapas bajan de 42,788,971 B `.bin` a 2,536,902 B `.br`: 40,252,069 B (94.1%) menos. Se copian los `.br`; los `.bin` de trabajo no entran. |
| `sow-web/site/fonts` | 9 | 20,493,774 B / 19.54 MiB | Fuentes y materiales de generación; no se copian completos al paquete web. |
| Fuentes web del paquete anterior | 6 | 152,570 B / 0.15 MiB | Cuatro WOFF2, `fonts.css` y `OFL.txt`. Noto TTF/TTC no se publican como ficheros: el atlas MSDF y la fuente árabe se incorporan al WASM. |
| `dist/.sow-state/poki-runtime-assets` | 1,466 | 109,714,916 B / 104.63 MiB | Staging regenerado por el build Poki del 2 nov.; no es el paquete de subida. |
| └ `gameplay` en ese staging | 1,263 | 30,061,878 B / 28.67 MiB | Assets runtime de gameplay, antes de las exclusiones y transformaciones del paquete. |
| └ `shell` en ese staging | 183 | 75,750,573 B / 72.24 MiB | Incluye las escenas originales grandes; Poki las reduce a 960×540 o 540×960. |
| └ `site` en ese staging | 20 | 3,902,465 B / 3.72 MiB | Medios de sitio que el target Poki no usa y elimina. |
| Avatares en ese staging | 1,160 | 22,107,362 B / 21.08 MiB | Retratos, sheets y expresiones incluidos en el paquete actual. |
| Retratos planos | 755 | 13,556,618 B / 12.93 MiB | Retratos de naciones, tribus, héroes, fallback y random. |
| Sheets 2×2 | 81 | 3,649,900 B / 3.48 MiB | Un sheet por héroe del catálogo actual, máximo 384×384 px, WebP q85. |
| Recortes de expresiones | 324 | 4,900,844 B / 4.67 MiB | Cuatro expresiones por cada uno de esos 81 héroes. |
| `gameplay/icons` en staging | 85 | 610,974 B / 0.58 MiB | 81 iconos de habilidad y 4 iconos/atlas de UI. |
| Assets en el paquete Poki actual | 1,446 | 49,821,547 B / 47.51 MiB | Medición exacta de `dist/poki/assets`, incluyendo campaña MP4 y guías. |
| └ `assets/gameplay` en el paquete | 1,257 | 30,043,478 B / 28.65 MiB | Assets consumidos por el runtime, con las exclusiones Poki aplicadas. |
| └ `assets/shell` en el paquete | 178 | 14,212,810 B / 13.55 MiB | Incluye 162 escenas de héroes optimizadas: 13,855,762 B / 13.21 MiB. |
| └ `assets/campaign` y `assets/guides` | 11 | 5,565,259 B / 5.31 MiB | Campaña MP4 y guía de menú. |
| `dist/.sow-state/prod-web/poki` | 1,441 | 90,337,078 B / 86.15 MiB | Último paquete completo encontrado; usa 46.01 MiB de avatares antiguos y omite `gameplay/icons`. Incluye 8.21 MiB de WASM, 2.48 MiB de mapas, 1.43 MiB de locales y 13.17 MiB de arte de líderes. |
| ZIP de ese paquete en caché, DEFLATE 9 | — | 83,486,770 B / 79.62 MiB | Medido localmente; exacto para la caché vieja, no para el build actual. |
| `dist/poki` | 1,532 | 64,959,867 B / 61.95 MiB | Paquete completo fresco; contiene `index.html` y pasó `verify_poki_layout`. |
| ZIP local DEFLATE 9 de `dist/poki` | 1,532 | 58,024,838 B / 55.34 MiB | Archivo de comparación local; el tamaño exacto que acepte el portal depende del método de subida. |
| `dist/crazygames` (10 nov., 16:03) | 401 | 101,116,147 B / 96.43 MiB | Antes de los perfiles CG de líderes, atlas lossy y campaña solo MP4. |
| Candidato Poki (assets transformados + bundle anterior) | 1,531 | 65,327,622 B / 62.30 MiB | Proyección histórica; el build actual la reemplaza. |
| ZIP Poki DEFLATE 9 del candidato | — | 58,478,962 B / 55.77 MiB | Medición histórica, reemplazada por el ZIP fresco de arriba. |

El paquete Poki actual conserva 1,446 archivos de assets: `gameplay` aporta 30,043,478 B, `shell` 14,212,810 B después de recomprimir las escenas de líderes, campaña 5,564,015 B y guías 1,244 B. Suma 49,821,547 B de assets. La carpeta completa incluye 8,597,275 B de WASM, 1,774,548 B de HTML, 2,600,395 B de mapas y los demás archivos, para 64,959,867 B y 1,532 archivos. Su ZIP DEFLATE 9 mide 58,024,838 B. La proyección anterior quedó 367,755 B por encima del build real; queda reemplazada por esta medición.

En el paquete actual, los avatares ocupan 21.08 MiB, las escenas de líderes optimizadas 13.21 MiB y la campaña 5.31 MiB. El paquete viejo de caché tenía 13.17 MiB de escenas y un WASM de 8.21 MiB; no mezclar esos builds al sumar tamaños. El WASM del paquete fresco ocupa 8,597,275 B y se publica plano; no se asume que Poki lo sirva luego con `Content-Encoding: br`.

## Qué mete Poki y qué se queda fuera

El empaquetador parte del árbol completo de assets del build web y copia los archivos por defecto. Solo elimina para Poki la tienda/skins, los medios del sitio, los SVG duplicados de marca, el icono de navegación a tienda y las versiones WebM de campaña; además optimiza las escenas de líderes y reescribe sus triggers a MP4. Compara el inventario completo y los hashes de los archivos byte-idénticos, valida las dimensiones de escenas transformadas y exige el icono de habilidad de cada héroe jugable. Así una nueva carpeta de assets no se omite silenciosamente.

Los archivos estáticos locales son distintos de identidad, API y multiplayer. El target Poki inyecta `shadowsofwar.io/api` y `wss://shadowsofwar.io/ws/`, pero este audit de assets no demuestra que Poki haya aprobado cada flujo. Su política permite revisar servidores de multiplayer externos caso por caso y prohíbe sistemas externos de login/cuenta; confirmar la aprobación de API y multiplayer es un gate separado del empaquetado. Fuente: [política de recursos externos de Poki](https://developers.poki.com/guide/external-resources-policy).

La superficie `main_menu.poki.ts` oculta tienda y skins; por eso esas carpetas y el icono de navegación de tienda se excluyen. Se conserva el inventario completo de avatares, los iconos de HUD y habilidades, mapas, fuentes, campaña y todo el resto del árbol de assets que el runtime Poki consume. También se excluye `site/media`: el build sustituye `main_menu.surfaces.ts` por `main_menu.poki.ts`, cuyo panel de ajustes no renderiza los enlaces sociales con SVG. Los 20 archivos de `site/media` suman 3,902,465 B en staging y no son solicitudes del runtime Poki. CrazyGames conserva la superficie compartida y empaqueta solo `discord.svg`, `telegram.svg` y `github.svg` de esa carpeta.

## Descarga y carga de avatares

El menú no precarga avatares. En partida, el cliente pide solo los retratos de los jugadores presentes (cuatro caras por héroe, una por bot), seis a la vez, con el mismo cargador de imágenes del radial; las reglas están en `docs/nameplates-and-emotes.md`. Las URLs de avatar llevan un SHA-256 en `?v=` y el CDN propio las responde con `max-age=31536000, immutable`, así que la caché HTTP del navegador las reutiliza. CrazyGames confirma que administra su caché CDN para todo lo que se sube; en ese portal hay que medir cuántos bytes cruzan la red antes de `Gameplay start`, pues el paquete bajo el límite total no demuestra por sí solo que pase el límite inicial.

Ese tamaño de red no es memoria de textura. La textura de retratos usa celdas de 128 px en filas de 32 (2 MiB por fila) y crece solo con el roster de la partida: 8 héroes y 120 bots ocupan 5 filas, 10 MiB. El máximo es 32 filas (64 MiB) y al terminar la partida vuelve a una fila. No hay copia de las imágenes en CPU.

CrazyGames permite 250 MB totales y 1,500 archivos. Con el SDK, mide el máximo de 50 MB iniciales (20 MB para portada móvil) desde el comienzo de carga hasta el primer `Gameplay start`; sin SDK usa el tamaño total. El evento debe representar que el juego ya es jugable. No moverlo para ocultar descargas; medir el flujo real del paquete actualizado y mantener el inicio jugable dentro del umbral.

## Optimizaciones ordenadas por ganancia

### Ya aplicadas en el empaquetado

| Transformación aplicada | Última medición registrada |
| --- | ---: |
| Generar el catálogo de avatares de runtime en WebP q85 | 258.45 MiB de fuente → 21.08 MiB en el staging/package actual; ahorro medido de 237.36 MiB (91.8%). |
| Reescalar y recomprimir 162 escenas de héroes para Poki | 72,256,450 B → 13,855,762 B; ahorro de 55.70 MiB en el paquete actual. |
| Empacar 20 imágenes de edificios en un atlas WebP lossless | 51,465,992 B → 498,178 B entre atlas y manifiesto; ahorro de 48.61 MiB (99.0%). Las fuentes PNG se reducen a celdas de 128 px antes del atlas. |
| Publicar mapas binarios con Brotli | 22 mapas: 42,788,971 B `.bin` → 2,536,902 B `.br`; ahorro de 40,252,069 B (38.39 MiB, 94.1%). |
| Reducir miniaturas de mapas a 256×144 q70 para portal y tarjetas web; el landing usa `preview.webp` a 512×288 | Portal, 22 WebP: 2,340,666 B → 61,990 B; ahorro de 2,278,676 B (2.17 MiB, 97.4%). Web: falta medir el paquete. |
| No entregar fuentes Noto como ficheros web | 20,493,774 B de materiales fuente → 152,570 B de ficheros de fuente/estilo en el webroot. El MSDF y el TTF árabe van dentro del WASM; la diferencia no debe restarse otra vez del paquete total. |
| Omitir el WebM duplicado de campaña en Poki y CrazyGames | ~5.6 MB menos por portal; quedan MP4, poster y triggers. |
| Subir assets, fuentes y mapas a CrazyGames para que los sirva desde su CDN | Reduce egreso estático de SOW; no reduce lo que descarga el jugador. |
| Dejar de publicar las 81 láminas 2×2 de héroes; el runtime usa los recortes | 3,649,900 B (3.48 MiB) y 81 archivos menos según el staging del 2 nov.; falta volver a medir el paquete. |
| Atlas de habilidades para CrazyGames | Un archivo en lugar de uno por héroe. Sin pérdida pesaba 3.2 MB; ahora es lossy q75, como los iconos sueltos de Poki. |

### Ahorro incremental de tamaño del paquete

#### Pruebas de resolución en copias temporales

Recomprimí copias de los WebP actuales a q85; no cambié los originales ni el paquete. La prueba aisló los 647 retratos de naciones/tribus para dejar intactos los retratos de héroes y campaña. Son medidas de bytes, sin revisión visual:

| Grupo | Actual | Variante | Resultado | Ahorro |
| --- | ---: | ---: | ---: | ---: |
| 647 retratos de naciones/tribus (256×256) | 11,857,548 B | 192×192 q85 | 7,227,010 B | 4,630,538 B / 4.42 MiB |
| Los mismos 647 retratos | 11,857,548 B | 128×128 q85 | 3,879,132 B | 7,978,416 B / 7.61 MiB |
| 324 crops de expresiones (256×256) | 4,900,844 B | 192×192 q85 | 3,282,188 B | 1,618,656 B / 1.54 MiB |
| Los mismos 324 crops | 4,900,844 B | 128×128 q85 | 1,879,390 B | 3,021,454 B / 2.88 MiB |

192 px es el primer candidato razonable para los retratos pequeños; 128 px es agresivo. Los retratos de campaña se conservan en 256 px porque algunos ocupan casi todo el panel de diálogo. Las expresiones se muestran también en el panel grande de campaña, así que no bajaría sus 256 px sin revisar ahí la nitidez. Al aplicar una receta a los originales, hay que regenerar y volver a medir: estos números salen de una segunda codificación temporal del paquete WebP actual.

| Prioridad | Cambio | Ahorro medido | Costo / límite |
| --- | --- | ---: | --- |
| 1 | Reducir los 647 retratos de naciones/tribus de 256 a 128 px; la alternativa 192 px es menos agresiva. | 7.61 MiB a 128 px; 4.42 MiB a 192 px, ambas q85. | Prueba temporal de bytes, sin revisión visual; conservar los 25 retratos de campaña y los retratos de héroes a 256 px. |
| 2 | Reencodear avatares a WebP q80 en la salida, conservando intactos los originales. | Ensayo sobre 1,160 archivos: 22,107,362 → 18,463,210 B; ahorro de 3,644,152 B (3.475 MiB, 16.5%). | Revisar calidad visual; el ahorro se solapa con los cambios de resolución y deduplicación. |
| 3 | Reducir los crops de expresiones de 256 a 128 px; 192 px es la opción moderada. | 2.88 MiB a 128 px; 1.54 MiB a 192 px, q85. | Prueba temporal sin revisión visual; afecta retratos grandes de campaña. |
| 4 | Cambiar los cinco PNG de unidades a WebP lossless en la salida. | Medición temporal: 3,589,031 B → 2,236,556 B; ahorro 1.29 MiB. | Requiere actualizar `main_menu.hud.ts`, que forma la ruta `.png`, y el verificador de paquete; validar decodificación y conservar los originales. |
| 5 | Dejar de publicar los 81 crops Calm duplicados y usar el retrato plano, que contiene exactamente los mismos bytes. | 1,204,158 B / 1.15 MiB y 81 solicitudes menos. | Los renderers ya resuelven Calm al retrato plano. El preloader de campaña debe resolver Calm igual antes de quitar los archivos; verificar catálogo y todas las expresiones. |
| 6 | Bajar bitrate del WebM de música (Opus, 48 kHz estéreo, 249.2 s). | Experimentos temporales desde 3,215,191 B: 96 kb/s → 3,179,405 B (−1.1%); 80 kb/s → 2,649,652 B (−565,539 B, 17.6%); 64 kb/s → 2,118,537 B (−1,096,654 B, 34.1%). | No se tocaron los originales. Falta escuchar/aceptar calidad; los dos bitrates más bajos pueden degradar música. Ninguna variante está aplicada. |
| 7 | Reducir la resolución del MP4 de campaña. | Transcodificación temporal: 720p CRF28, 5,412,982 → 5,371,425 B (0.8% menos); 540p CRF26, 5,412,982 → 4,855,061 B (10.3%, 557,921 B menos). | El original sigue intacto. 540p pierde resolución y ahorra solo 0.53 MiB; falta revisión visual antes de considerar el cambio. |
| 8 | Seguir reduciendo escenas de héroes. | El paquete Poki ya las reduce a 13.21 MiB. | Ganancia adicional no medida; el loader depende de su apariencia y puede perder nitidez. |

El orden compara el ahorro incremental de cada técnica por separado contra el catálogo actual. Los escenarios combinados se indican aparte y no deben sumarse con los ahorros individuales. Los escenarios combinados medidos el 2 nov. incluían bajar las láminas a 256 px; como las láminas ya no se publican, hay que regenerarlos desde los originales antes de decidir. El ahorro medido no implica que el cambio visual o auditivo esté aprobado; las variantes de calidad requieren aceptación antes de incorporarse a la receta de publicación.

### Reducción de bytes antes de empezar a jugar

- El WASM del paquete actual mide 8,597,275 B (8.60 MB / 8.20 MiB) y se incluye plano. El staging genera también un `.wasm.br` de 3,760,724 B: reducción potencial de 4,836,551 B (4.61 MiB, 56.3%) si el host sirve el `.wasm` con `Content-Encoding: br`. Ese archivo comprimido no está en `dist/poki`; no conviene sustituirlo ni añadirlo hasta confirmar en Poki Inspector qué headers entrega su host. Brotli de transferencia no equivale al tamaño de la carpeta o del ZIP.
- El menú ya no precarga el catálogo de avatares (21.08 MiB en el staging del 2 nov.); cada partida pide solo los retratos de su roster.
- El umbral de descarga inicial de CrazyGames se mide hasta el primer `Gameplay start`; en Poki hay que leer la cifra que presenta su Inspector. No tenemos una medición de red en un navegador/portal y no debe inferirse del peso de la carpeta o del ZIP.

WebP y MP4/WebM ya llevan compresión; los mapas se publican en Brotli. El WASM del paquete Poki sigue plano; Brotli es solo una opción de transferencia si la plataforma admite `Content-Encoding: br`. El ZIP local mide 6.94 MB menos que la carpeta, pero no representa por sí mismo lo descargado por el jugador. Los PNG de unidades sí mostraron ahorro lossless medible. `dist/` no es una medida de subida porque acumula compilaciones, cachés y paquetes de más de un destino.

## Anuncios y economía por portal

Estado del 9 nov. 2026. Ningún portal tiene aún el juego publicado; esto es lo que el código ya hace en cada build.

| Portal | Anuncios en el build | Recompensado |
| --- | --- | --- |
| Poki | `commercialBreak` solo antes de entrar a la siguiente partida, y solo si ya se jugó una (`SOW_portalBeforeMatch` en `sdk/poki_portals.ts`). Nunca en el primer input ni a mitad de partida. | `rewardedBreak`; la recompensa solo se paga si la promesa devuelve `true`. |
| CrazyGames | Midgame entre partidas, solo después de juego real; los pares `gameplayStart`/`gameplayStop` no se duplican. Si el SDK responde `adsDisabled` (Basic Launch) o hay adblock, se dejan de pedir anuncios. | Se enciende solo cuando CrazyGames ya sirvió un anuncio real (Full Launch) y lo recuerda en `localStorage`; en Basic Launch todo pedido responde `adsDisabled` y la oferta nunca aparece. Si `adsDisabled` vuelve, se apaga otra vez. Sin flag ni paso manual. |
| Jest | Ninguno. El verificador del paquete rechaza `commercialBreak`, `rewardedBreak`, `SOW_ENABLE_PORTAL_ADS` y el SDK de Poki. | No hay oferta; el shell no la muestra sin el hook del portal. |

El audio del juego (WASM) se silencia durante cada anuncio y cuando el portal lo pide (`SOW_setPortalAudioMuted`).

**Doblar recompensas.** Al volver de una partida pública verificada, el menú ofrece una sola vez “Continuar” o “Ver video”, con botones del mismo tamaño y un icono de video. Si el anuncio termina, el servidor (`POST /profile/reward-receipts/double`) vuelve a pagar las coronas y gemas de esa partida. Reglas en `sow-data/src/db.rs`: solo partidas con bono verificado, dentro de las 6 h siguientes, una vez por partida y como máximo 5 por día UTC.

**Gemas por partida** (`match_gems` en `sow-data/src/rewards.rs`, solo en Matchmaking y Ranked 1v1 verificados): 1 por jugar al menos 5 min o ganar, 2 si además derrotaste jugadores o imperios. Ganar suma 17 en FFA público y 8 en el resto (Teams, HvN, Ranked 1v1). Coronas por partida: 40 por participar, +100 por ganar, más bonos por bajas, imperios y asistencias.

**Revisión de Poki.** Poki revisa el uso de anuncios en su QA antes de publicar; esta oferta sigue sus reglas publicadas de recompensado (opcional, botón de continuar del mismo tamaño, icono de video, sin pago si el video falla).

## Verificación local

Los tests siguientes son resultados registrados del trabajo previo; no se reejecutaron en esta revisión. Sí se volvió a ejecutar el empaquetado oficial local de Poki; no despliega ni sube.

- `cargo test -p sow-client --test nameplate_emotes` (3 nov.): pasaron las 6 pruebas de `render/avatars.rs`. Cubren que cada retrato conserva su celda toda la partida, el respaldo mientras carga, el catálogo completo, el fin de partida y los reintentos. La precarga del menú ya no existe.
- `cargo test -p sow-dist --bin sow hero_sheet` y `avatar_output_recipe` (3 nov.): 2 tests pasaron; cubren recortes 2×2 y que el cache invalide una salida copiada/vieja con la receta nueva.
- `cargo test -p sow-dist --bin sow poki_package_keeps_runtime_assets_and_only_drops_explicit_exclusions`: pasó; verifica que el empaquetador conserva avatar, atlas, icono, video de campaña MP4 y navegación usados, y elimina únicamente las exclusiones declaradas.
- `cargo test -p sow-render full_avatar_atlas_holds_every_catalog_portrait_within_gpu_limits` (3 nov.): pasó; verifica celdas de 128 px, que todo el catálogo cabe en 32 filas de 32 y UV dentro de la textura.
- `./sow poki`: pasó el 2 nov. a las 21:12. Recompiló WASM y shell con los shaders actuales, validó los 15 catálogos, regeneró el staging, comprobó inventarios de assets/mapas y publicó `dist/poki` tras pasar `verify_poki_layout`. La auditoría posterior encontró 1,160 rutas de avatar del catálogo y 1,160 archivos coincidentes; las rutas de assets/mapas son locales. Salida: 1,532 archivos / 64,959,867 B. Log: `dist/logs/poki-20261102-210802.log`.
- `./sow poki` y `./sow jest` (9 nov.): pasaron con el flujo de anuncios nuevo. `dist/poki`: 1,461 archivos / 56,586,026 B; el verificador Poki exige `SOW_portalRewardedBreak` y `double_match_reward` antes de aceptar `rewardedBreak`. El verificador Jest confirmó que su paquete no lleva anuncios. `menu_flow.test.ts` cubre la oferta (botones iguales, pago solo con video terminado, una vez por partida) y el encendido automático en CrazyGames.
- La revisión visual de nameplates, expresiones y calidad de compresión sigue pendiente del dueño del juego.

## Siguiente verificación

1. Revisar `dist/poki` con Poki Inspector y medir peso/tiempo de carga en el portal. El build local no prueba aprobación de API/multiplayer.
2. Aceptar visualmente los nameplates, las cuatro expresiones, los avatares del menú radial y la calidad de compresión. El icono de Abd aún es provisional.
3. Medir `dist/crazygames` en el siguiente `./sow p` y ajustar `CG_BUNDLE_BYTE_BUDGET` a esa medida más un 10%.
4. Medir la descarga de red antes de `Gameplay start` en escritorio y móvil sin adelantar ese evento mientras haya pantalla de carga.
