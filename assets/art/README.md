# Arte del juego: el único lugar para agregar o reemplazar

Para reemplazar algo, sobrescribe el archivo con el mismo nombre.

- Formato: PNG o WebP, del tamaño que quieras. Lo ideal son unos 1200 px.
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

Hoy, los íconos, las unidades y los edificios que no tienen arte propio usan una imagen provisional; están marcados en `assets/SOURCES.toml`.

## Tomas del landing (`assets/site/media/`)

Graba en 16:9 y pon el archivo con este nombre, en cualquier formato (mp4, mov, webm o mkv para video; png, jpg o webp para imagen). `./sow p` lo convierte y lo publica.

| Archivo | Qué grabar |
|---|---|
| `hero-loop` | Video de fondo de la portada, de 10 a 15 s: el mapa del mundo a media partida, alejado y sin menús abiertos. Se reproduce sin sonido y en bucle. |
| `gameplay` | Video de la partida, de 30 a 45 s: arranque, expansión, construcción, una alianza rota y el mapa final. La sección aparece solo cuando existe. |
| `step-expand`, `step-build`, `step-war` | Imágenes de los tres pasos: territorio creciendo, ciudad, puerto y torre de cerca, y una frontera en guerra. Mientras no estén, se usa arte del juego. |

## Agregar un héroe jugable

1. En `assets/geo_entities.json`, al personaje se le agrega `hero` con `name` (nombre corto; queda guardado en la experiencia de cada jugador, no se cambia después), `ability` y, si tiene carta registrada, `card`. `art_slug` dentro de `hero` nombra sus archivos de arte y, si falta, se usa `id`; cambiarlo no altera la identidad ni la experiencia guardada. La habilidad sale de las clases de efecto que ya existen y debe tener una situación que ningún otro héroe use (misma clase y mismo objetivo o lado; `Ability::situation()`, y lo prueba `every_hero_has_a_unique_situation`). Regla de diseño: el efecto sale de un hecho histórico real del personaje y se ve en esa situación (anillo en el mapa o aviso con su icono, vía `GameEvent::AbilityTriggered`). `free: true` lo deja gratis para siempre. También necesita `territory_color`.
2. Para sus avatares, un solo archivo: `avatars/heroes/<art_slug>.webp`, con las cuatro caras en una hoja 2 × 2. El icono de habilidad va en `icons/abilities/<art_slug>.webp`. Las escenas del menú y la pantalla de carga van aparte en `heroes/<art_slug>_desktop.webp` y `heroes/<art_slug>_mobile.webp`; si faltan, no detienen la publicación, pero tampoco tienen imagen sustituta.
3. En los 15 idiomas (`sow-i18n/strings/*/web.toml`): `leader_<id>_name`, `leader_<id>_historical` y `leader_<id>_civilization` en `[heroes]`; `leader_<id>_ability` y `leader_<id>_description` en `[site]`.

El selector, el perfil y los emotes usan la hoja; las escenas se cargan por separado cuando existen. `./sow p` se detiene si falta un recurso obligatorio o si la habilidad no cambia el juego.

Un archivo con un nombre que no encaja en estas carpetas detiene el build, y también un nombre repetido en dos carpetas. Para agregar un espacio nuevo hay que darlo de alta en `sow-dist` (`ART_ICONS`) o en `sow-tools` (`BADGES`).
