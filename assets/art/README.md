# Arte fuente: dónde reemplazar y cómo se distribuye

Las fuentes editables viven en `assets/art/`; las tomas propias del landing y las capturas viven en `assets/site/media/`. El arte raster se conserva en WebP: fotos a calidad 90 y arte de píxel, logotipos y datos de imagen en WebP lossless. Reemplaza el archivo fuente en su carpeta y conserva el nombre esperado por el juego.

- Conserva las dimensiones de la fuente; `sow-dist` genera las variantes optimizadas para distribución sin reemplazar la fuente.
- Usa transparencia para íconos, unidades y edificios. Los barcos pixel art conservan sus bordes con un perfil de remuestreo puntual.
- `sow-dist/art-optimization.toml` define dimensiones, calidad y peso máximo por tipo de salida. `./sow l` y `./sow p` aplican esos perfiles al material de distribución y fallan si falta una receta o se excede un límite.
- Ejecuta `./sow art-preview` para muestras de perfiles, o `./sow art-preview ability_icon` para revisar todos los iconos de habilidades. El reporte y sus derivados se escriben en `dist/.sow-state/art-preview/`; los originales no se modifican.
- No toques `assets/generated/`: lo arma la herramienta.
- Si falta un archivo que el juego necesita, el build se detiene y dice cuál.

## Splash compartido e icono de aplicación

| Arte que reemplazas | Fuente maestra | Salida optimizada |
|---|---|---|
| Splash de escritorio | `assets/art/loader/sow-splash-desktop.webp` | `assets/shell/loader/sow-splash-desktop.webp`; lo comparten el loader y la portada del landing. |
| Splash móvil | `assets/art/loader/sow-splash-mobile.webp` | `assets/shell/loader/sow-splash-mobile.webp`; cada dispositivo usa su propio hash para la caché. |
| Icono de aplicación | `assets/art/brand/app-icon.webp` | Iconos PWA y densidades Android PNG generados por `sow-dist`; no mantengas copias fuente en `assets/site/icons/` ni en `sow-dist/deploy/android/.../res/mipmap-*/`. |

Las escenas de sección (`step-*`, `closing-*`) siguen siendo ilustraciones independientes de `assets/site/media/`. La apertura del landing usa los dos splash maestros de arriba; no hay copias `opening-*` en `assets/site/media/`.

## Retratos de personajes

| Carpeta | Archivo | Quién lo usa | Se publica a |
|---|---|---|---|
| `avatars/nations/` | `<avatar>.webp` | naciones bot (213) | 256 px |
| `avatars/tribes/` | `<avatar>.webp` | tribus bot (400) y tribus del tutorial | igual |
| `avatars/campaign/` | `<avatar>.webp` | figuras de campaña que no son tribus (romanos del tutorial) | 256 px |
| `avatars/heroes/` | `<id>.webp` | hoja de héroe; Calm también se publica como retrato | Calm a 256 px |
| `avatars/` | `default.webp` | cara de un jugador sin retrato | 256 px |
| `avatars/` | `random.webp` | opción de héroe aleatorio | 256 px |

`<id>` es el `avatar` del personaje en `assets/geo_entities.json`, por ejemplo `hatshepsut` o `xiongnu`. Cada personaje tiene su archivo con ese nombre. El valor de `avatar` es el nombre del archivo, sin prefijo: `xiongnu` usa `xiongnu.webp`. El ID interno lleva prefijo: `tribe_xiongnu` o `nation_hatshepsut`. Los que todavía no tienen retrato propio son copias de `default.webp`: sobrescribe el archivo con el retrato real.

## Héroes: hoja de caras y escenas

| Carpeta | Archivo | Dónde se ve |
|---|---|---|
| `avatars/heroes/` | `<id>.webp` | una hoja 2 × 2: Feliz, Calmado, Miedo y Enojado |
| `heroes/` | `<id>_desktop.webp` | escena del menú y pantalla de carga, escritorio |
| `heroes/` | `<id>_mobile.webp` | escena del menú y pantalla de carga, celular |

El orden de la hoja es de izquierda a derecha y de arriba abajo: Happy, Calm, Fear, Angry. El juego recorta las cuatro caras automáticamente; Calm también se publica como retrato normal y se usa en reposo. No hay una imagen o stance adicional por héroe. Los emotes usan Calm para saludar, Fear para pedir ayuda, Angry para amenazar y Happy para celebrar. `default.webp` y `random.webp` siguen siendo imágenes generales, no hojas de héroe.

`heroes/` también guarda escenas de personajes que todavía no son héroes; se publican igual.
Las escenas son independientes de la hoja y se publican si el archivo existe. Si falta una variante, el menú o la pantalla de carga no tienen una imagen sustituta para esa variante; agrega ambos archivos para mostrar las dos versiones.

## Íconos, unidades, edificios y tienda

| Carpeta | Archivo | Dónde se ve | Se publica a |
|---|---|---|---|
| `buildings/` | `city-1` … `city-4`, `factory-1` … `factory-4`, `port-1` … `port-4`, `tower-1` … `tower-4`, `farm-1` … `farm-4` | edificio en el mapa y botón del HUD | atlas, celda de 64 px |
| `icons/` | `troops` | tropas en el HUD, en la barra del menú (Batalla) y junto a las tropas en el mapa | 128 px y atlas |
| `icons/` | `gold`, `gem`, `crown`, `laurel` | monedas en tienda, HUD y perfil | 128 px |
| `icons/` | `nav-heroes`, `nav-profile`, `nav-store` | barra de navegación del menú | 128 px |
| `icons/` | `tutorial-hand` | mano del tutorial | 256 px |
| `icons/` | `surrender` | rendirse | 256 px |
| `icons/abilities/` | `<hero-id>.webp` | habilidad de cada héroe | 256 px máximo, WebP, límite 64 KiB; CrazyGames arma su atlas desde estos derivados |
| `icons/` | `request`, `allied`, `traitor` | solicitud, aliado y traidor en la placa de nombre y el HUD | atlas |
| `icons/` | `rank-1`, `rank-2`, `rank-3` | 1.º, 2.º y 3.er lugar en la placa de nombre | atlas |
| `icons/` | `you` | marca "eres tú" en tu placa | atlas |
| `icons/` | `disconnected` | jugador sin conexión | atlas |
| `icons/` | `upgrade-sparkle` | destellos al mejorar un edificio | atlas |
| `icons/` | `wilderness` | terreno sin dueño en el panel de ataques | atlas |
| `icons/` | `death`, `radiation` | marca al morir un jugador (normal y por bomba) | atlas |
| `icons/` | `construction`, `hammer` | tiempo de construcción y edificio en obra sobre el mapa | atlas |
| `icons/` | `arrow-up`, `arrow-right`, `arrow-down`, `arrow-left`, `cancel`, `confirm` | flechas y botones al colocar un edificio | atlas |
| `units/` | `transport-ship`, `trade-ship`, `warship`, `atom-bomb`, `sam-missile` | barcos y misiles que se mueven por el mapa | atlas, celda de 64 px |
| `store/` | `sow_gems_500`, `sow_gems_1200`, `sow_gems_2600` | paquetes de gemas | 128 px |
| `brand/` | `sow.svg`, `sow-long.svg`, `tower.svg`, `app-icon.webp` | marca e iconos de la aplicación | SVG minificado; iconos PWA/Android generados del maestro |
| `loader/` | `sow-splash-*`, `loader_empty`, `loader_full` | apertura compartida y barras de carga | WebP con límite por perfil |
| `skins/` | mismos SVG | skins | SVG minificado |
| `music/` | mismo nombre | música del juego | optimizador de audio existente |

Los iconos WebP fuente pueden conservarse a alta resolución. `sow-dist` los reduce según su perfil; los atlas se generan desde el arte fuente y conservan el tratamiento de bordes configurado. Tú solo reemplazas el archivo maestro.

Los tres sprites de barcos vienen de OpenFront; las bombas y misiles todavía usan imágenes provisionales. Las demás imágenes provisionales están marcadas en `assets/SOURCES.toml`.

## Arte y tomas del landing (`assets/site/media/`)

Las escenas propias del landing viven aquí; la portada usa los splash maestros de `assets/art/loader/`. `./sow l` y `./sow p` optimizan las imágenes registradas con perfiles para escenas anchas o altas. Los nombres permitidos se registran en `SITE_STILLS` y `SITE_VIDEOS` de `sow-dist`.

| Archivo | Uso |
|---|---|
| `step-expand`, `step-expand-mobile` | Expansión: ejército y estandartes avanzando por el valle. |
| `step-build`, `step-build-mobile` | Economía: puerto egipcio y barcos de comercio. |
| `step-war`, `step-war-mobile` | Combate: asalto a una frontera fortificada. |
| `closing-desktop`, `closing-mobile` | Panorama final y llamada a jugar. |
| `game-expand`, `game-build`, `game-war` | Capturas reales de partidas junto a las ilustraciones; no son imágenes generadas. |
| `hero-loop` | Video opcional de portada: bucle de 12 s, sin sonido. Se pausa fuera de pantalla y respeta la preferencia de reducir movimiento. Mientras no exista, se muestra la portada fija correspondiente al dispositivo. |
| `gameplay` | Video opcional de una partida, de 30 a 45 s. La sección aparece solo cuando existe. |

Los prompts exactos, sus referencias originales en orden y el encargo de animación viven en [el manifiesto de arte del landing](../site/landing-art.json). Las fuentes raster de imágenes se guardan en WebP; los videos admiten MP4, MOV, WebM o MKV. El publicador produce WebP para las imágenes, WebM y MP4 para los videos.

## Agregar un héroe jugable

El flujo completo de investigación, catálogo, habilidad, tienda, progreso, idiomas, biografías y pruebas está en la [guía para crear e integrar héroes](../../docs/guia-creacion-heroes.md). Esta sección conserva el contrato de los archivos de arte:

- Retrato: `avatars/heroes/<art_slug>.webp`, una hoja 2 × 2 en orden Happy, Calm, Fear, Angry. El selector, el perfil y los emotes usan los recortes de esta hoja.
- Icono de habilidad: `icons/abilities/<art_slug>.webp`, cuadrado y reconocible a tamaño pequeño.
- Escenas: `heroes/<art_slug>_desktop.webp` y `heroes/<art_slug>_mobile.webp`. Se publican si existen; las variantes faltantes no tienen imagen sustituta.
- El publicador procesa las hojas y recortes; no edites `assets/generated/` ni `dist/`. Si agregas una carpeta o categoría de arte distinta, registra su dueño en `sow-dist` (`ART_ICONS`/`ART_FOLDERS`) o `sow-tools` (`BADGES`).

El test de assets exige una hoja y un icono por héroe. Para mostrar escenas en selector, menú y página pública, entrega ambas variantes. Registra procedencia y licencia del arte en `assets/SOURCES.toml` cuando la fuente no corresponda a la clasificación existente.
