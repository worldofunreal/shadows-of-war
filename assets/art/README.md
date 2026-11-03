# Arte del juego: el único lugar para agregar o reemplazar

Para reemplazar algo, sobrescribe el archivo con el mismo nombre.

- Formato: PNG o WebP, del tamaño que quieras. Lo ideal son unos 1200 px; los barcos pixel art pueden ser diminutos y se amplían conservando los píxeles.
- Fondo: transparente para íconos, unidades y edificios.
- `./sow p` y `./sow l` lo optimizan, lo convierten y lo publican.
- No toques `assets/generated/`: lo arma la herramienta.
- Si falta un archivo que el juego necesita, el build se detiene y dice cuál.

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
| `brand/`, `loader/`, `skins/`, `music/` | mismo nombre | logo, pantalla de carga, skins, música | sin cambios |

"atlas" quiere decir que `./sow p` los junta solo en una imagen (celdas de 128 px; unidades de 64 px). Tú solo cambias el archivo.

Los tres sprites de barcos vienen de OpenFront; las bombas y misiles todavía usan imágenes provisionales. Las demás imágenes provisionales están marcadas en `assets/SOURCES.toml`.

## Arte y tomas del landing (`assets/site/media/`)

Las escenas propias del landing viven aquí; el selector sigue usando los splashes originales de `heroes/`. `./sow l` y `./sow p` convierten y publican estos archivos con el mismo flujo del resto del arte. Los nombres permitidos se registran en `SITE_STILLS` y `SITE_VIDEOS` de `sow-dist`.

| Archivo | Uso |
|---|---|
| `opening-desktop`, `opening-mobile` | Portada: Boudica y Lady Six Sky, con composiciones separadas para el texto en escritorio y móvil. |
| `step-expand`, `step-expand-mobile` | Expansión: ejército y estandartes avanzando por el valle. |
| `step-build`, `step-build-mobile` | Economía: puerto egipcio y barcos de comercio. |
| `step-war`, `step-war-mobile` | Combate: asalto a una frontera fortificada. |
| `closing-desktop`, `closing-mobile` | Panorama final y llamada a jugar. |
| `game-expand`, `game-build`, `game-war` | Capturas reales de partidas junto a las ilustraciones; no son imágenes generadas. |
| `hero-loop` | Video opcional de portada: bucle de 12 s, sin sonido. Se pausa fuera de pantalla y respeta la preferencia de reducir movimiento. Mientras no exista, se muestra la portada fija correspondiente al dispositivo. |
| `gameplay` | Video opcional de una partida, de 30 a 45 s. La sección aparece solo cuando existe. |

Los prompts exactos, sus referencias originales en orden y el encargo de animación viven en [el manifiesto de arte del landing](../site/landing-art.json). Los originales de las imágenes son WebP, PNG o JPG; los videos admiten MP4, MOV, WebM o MKV. El publicador produce WebP para las imágenes, WebM y MP4 para los videos.

## Agregar un héroe jugable

El flujo completo de investigación, catálogo, habilidad, tienda, progreso, idiomas, biografías y pruebas está en la [guía para crear e integrar héroes](../../docs/guia-creacion-heroes.md). Esta sección conserva el contrato de los archivos de arte:

- Retrato: `avatars/heroes/<art_slug>.webp`, una hoja 2 × 2 en orden Happy, Calm, Fear, Angry. El selector, el perfil y los emotes usan los recortes de esta hoja.
- Icono de habilidad: `icons/abilities/<art_slug>.webp`, cuadrado y reconocible a tamaño pequeño.
- Escenas: `heroes/<art_slug>_desktop.webp` y `heroes/<art_slug>_mobile.webp`. Se publican si existen; las variantes faltantes no tienen imagen sustituta.
- El publicador procesa las hojas y recortes; no edites `assets/generated/` ni `dist/`. Si agregas una carpeta o categoría de arte distinta, registra su dueño en `sow-dist` (`ART_ICONS`/`ART_FOLDERS`) o `sow-tools` (`BADGES`).

El test de assets exige una hoja y un icono por héroe. Para mostrar escenas en selector, menú y página pública, entrega ambas variantes. Registra procedencia y licencia del arte en `assets/SOURCES.toml` cuando la fuente no corresponda a la clasificación existente.
