# assets/: qué hay en cada carpeta y por qué

Regla general:
- El **arte** se agrega o se reemplaza solo en `art/`.
- Lo **generado** lo escriben las herramientas.
- Lo demás son **datos** del juego: mapas, campañas y guías.
- `./sow p` arma la copia que se publica en `/assets/...` (en `dist/.sow-state/runtime-assets/`, fuera de git). Esa copia nunca se edita a mano.

| Carpeta | Qué es | Por qué está aquí | Quién la usa |
|---|---|---|---|
| `art/` | Todo el arte: líderes, edificios, íconos (incluye las insignias), unidades, tienda, marca, pantalla de carga, skins y música. Los espacios están en `art/README.md`. | Es el único lugar donde el owner deja o reemplaza arte. | `sow-dist` lo optimiza y lo publica; `sow-tools` arma los atlas de edificios, íconos y unidades. |
| `generated/` | Archivos que arman las herramientas: atlas de edificios, de íconos, de unidades y de letras del mapa (`msdf-atlas`). | El juego los usa al compilar o los descarga; están separados para que nadie los edite a mano. | Edificios, íconos y unidades: `pack-building-atlas`, en cada `./sow p`. Letras: `generate_font_atlas`, solo cuando una traducción trae caracteres nuevos. |
| `fonts/` | Work Sans en TTF. | Es la fuente del atlas de letras del mapa. | `generate_font_atlas`. |
| `maps/` | Los mapas jugables (`map.bin`, miniaturas y `catalog.bin`), con su `NOTICE` y `SOURCES.toml` de licencias. | Es la biblioteca única de mapas que usan el servidor y el cliente. | Servidor, cliente y `sow-dist`. Antes de tocarla, lee las reglas de mapas en `AGENTS.md`. |
| `map_sources/` | Las fuentes con las que se hacen los mapas, que no se juegan tal cual: `eastanglia.png` (imagen origen del mapa de Boudica), `giantworldmap.png` (relieve del mundo para el editor de campañas), `thumbnail_frames.json` (cuadros de recorte de las miniaturas) y `world_island_guides.json` (guías de islas del editor). | Están separadas de `maps/` porque son la materia prima y no lo que se publica para jugar. | Herramientas de mapas, `sow-dist` (miniaturas) y el editor de campañas. |
| `campaign/` | Cada episodio: `<episodio>.json` (personajes y mapa), `<episodio>.triggers.json` (guion paso a paso) y los videos de la intro (`boudica/`). | Son datos de juego que el cliente compila y carga por nombre de episodio. | Cliente, servidor, `sow-dist` y el editor de campañas. |
| `guides/` | `menu.triggers.json`: el guion de las guías del menú principal (qué botón señalar y cuándo). | No es una campaña, pero es el mismo tipo de guion. Lo leen el menú y el servidor. | `main_menu.tutorial.ts`, `sow-data` (perfil) y el editor de campañas. |
| `geo_entities.json` | Catálogo de los bots: cada nación (con su líder) y cada tribu (con su nombre), con pueblo, hogar en el mapa, género, Wikipedia y retrato. Los héroes jugables son parte de él. | Una sola lista para bots y héroes; los bots aparecen repartidos por toda la tierra y cada uno toma al personaje libre con hogar más cercano. Las facciones de campaña no van aquí: viven en `campaign/`. | `sow-data`, `sow-dist` y el editor de campañas. |
| `site/media/` | Imágenes y videos de la página web (`shadowsofwar.io`) y del README de GitHub. | Son del sitio de marketing, no del juego. | `sow-web/site/index.html` y `README.md`. |
| `site/icons/` | Íconos de la app web (192, 512 y la versión adaptable para Android). | Son los que salen al instalar el sitio o el juego en la pantalla de inicio. | `sow-dist` los copia a la raíz de la web; el `manifest.webmanifest` los usa. |
| `SOURCES.toml` | De dónde salió cada cosa y con qué licencia. | Registro legal de los assets. | Lo revisa una prueba (`assets_layout.test.ts`). |

## Fuera de `assets/` (y por qué no se mueve)

- `sow-web/site/fonts/`: las fuentes web que sirve el sitio (Work Sans y Noto). El sitio las pide por una dirección relativa y el renderizador del mapa también usa las Noto.
- `sow-native/src-tauri/icons/`: íconos de la app de escritorio e iOS. Tauri los exige en esa carpeta exacta.
- `sow-dist/deploy/android/app/src/main/res/mipmap-*/`: ícono de la app de Android. Android los exige en esa carpeta exacta.
- `sow-server/tests/fixtures/`: datos de prueba del servidor; no es arte.
