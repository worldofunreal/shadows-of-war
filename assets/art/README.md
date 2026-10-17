# Arte del juego: el único lugar para agregar o reemplazar

Para reemplazar algo, sobrescribe el archivo con el mismo nombre.

- Formato: PNG o WebP, del tamaño que quieras. Lo ideal son unos 1200 px.
- Fondo: transparente para íconos, unidades y edificios.
- El siguiente `./sow p` lo optimiza, lo convierte y lo publica.
- No toques `assets/generated/`: lo arma la herramienta.

| Carpeta | Archivo | Dónde se ve | Se publica a |
|---|---|---|---|
| `leaders/<nombre>/` | `portrait` | HUD, placas de nombre, perfil, diálogos | 256 px |
| `leaders/<nombre>/` | `expressions` | caras del héroe sobre el mapa (solo héroes jugables) | igual |
| `leaders/<nombre>/` | `scene-desktop`, `scene-mobile` | menú y pantalla de carga | comprimido (máx. 400 KB) |
| `leaders/_fallback/` | `portrait` | jugador sin retrato | 256 px |
| `buildings/` | `city-1` … `city-4`, `factory-1` … `factory-4`, `port-1` … `port-4`, `bunker-1` … `bunker-4`, `farm-1` … `farm-4` | edificio en el mapa y botón del HUD | atlas, celda de 64 px |
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

Hoy, los íconos, las unidades y los edificios que no tienen arte propio usan la imagen del emoji que se veía antes; están marcados en `assets/SOURCES.toml`.

Un archivo con un nombre que no está en esta tabla detiene el build. Así no queda arte olvidado sin publicar. Para agregar un espacio nuevo hay que darlo de alta en `sow-dist` (`ART_ICONS`) o en `sow-tools` (`BADGES`).
