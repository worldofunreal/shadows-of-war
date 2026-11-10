# Guía para crear e integrar héroes

Esta guía cubre el flujo canónico de Shadows of War desde la investigación hasta la incorporación al juego, el sitio y sus recursos. El catálogo es la fuente de verdad para la identidad y la habilidad; los registros de idioma, biografía y arte completan esa entrada.

## 1. Investigar antes de diseñar

- Usa fuentes primarias cuando existan y compleméntalas con museos, archivos, universidades o estudios académicos.
- Separa lo documentado de lo discutido: registra variantes de nombres, fechas inciertas, testimonios coloniales y tradiciones orales.
- No conviertas una relación familiar o un título ceremonial en autoridad ejecutiva sin evidencia. Identifica qué decisiones, mandos o acciones se atribuyen a la persona.
- Elige una acción del juego que represente una faceta documentada. Describe la mecánica como una representación de esa trayectoria, no como una táctica que las fuentes no registran.

## 2. Catálogo y roster

`assets/geo_entities.json` es el dueño canónico. `sow-data/src/leaders.rs` deriva automáticamente el roster jugable de las entradas con un bloque `hero`; esa misma entrada alimenta la tienda y los menús. No crees una lista paralela.

Una entrada necesita identidad y origen (`id`, `name`, `entity`, `role`, `region`, coordenadas, género, referencia histórica, `avatar` y color territorial), y un bloque `hero` con su nombre jugable y habilidad. `art_slug`, token comercial e identificadores antiguos solo se añaden cuando difieren o existen en los datos del producto.

- Usa un `id` estable en `snake_case`. El ID se guarda en perfiles y mensajes; no lo cambies al corregir un nombre visible.
- `role` conserva el origen de nación o tribu. La presencia del bloque `hero`, no un rol especial, determina que sea jugable y lo excluye de las listas de bots.
- Agrega héroes al final de `entities` para conservar los índices existentes de `Leader` y el orden del roster.
- Comprueba que `entity` describa el mismo pueblo o reino que se usará al decidir cuándo un objetivo es `kin` o `foreign`.

### Identidad, progreso y disponibilidad

- `id` también determina el `wire_id` serializado. Cambiarlo puede afectar perfiles, mensajes, compras y referencias fijas como `Leader::of(...)`; conserva el ID incluso si cambias el nombre o la cultura visible.
- `hero.name` es la clave histórica de XP y estadísticas por héroe. Si se renombra, agrega el nombre anterior a `legacy_ids` para que los perfiles puedan resolverlo y combinar el progreso previo. Usa `legacy_ids` también para IDs antiguos realmente guardados.
- `art_slug` determina nombres de archivos. Úsalo cuando los recursos deban conservar un nombre distinto del ID; cambiarlo no migra ni cambia la identidad del héroe.
- `free` omitido equivale a `false`: el héroe se ofrece como desbloqueable y puede entrar en la rotación gratuita semanal. Usa `free: true` solo si debe ser gratis permanentemente.
- `rarity` es obligatorio y fija la banda de precio: `Common`, `Rare`, `Epic` o `Legendary`. Decídela en este orden: ¿su figura se volvió símbolo o arquetipo que excede su biografía? → `Legendary`. Si no, ¿tiene una trayectoria excepcional o un punto de quiebre histórico con consecuencias duraderas, y puedes señalar el arco o el cambio concreto? → `Epic`. Si no, ¿es reconocible para el público general en varios mercados (usa las vistas de Wikipedia en varios idiomas como referencia)? → `Common`. Si no, `Rare`, el nivel por defecto. Ser poco conocido no basta para `Legendary`, y el arte no decide la rareza.
- `price_crowns` es obligatorio. Cada héroe no gratis cae en la banda de su rareza: `Common` 250–530, `Rare` 600–1 000, `Epic` 1 200–2 000 y `Legendary` 2 500–4 000. Ningún precio se repite, y dentro de su rareza el héroe con más vistas de Wikipedia en varios idiomas paga más. Un héroe con `free: true` lleva `price_crowns: 0`. Las gemas cuestan el 90 % de las coronas (`price_crowns * 9 / 10`): se calculan, no se guardan.
- El roster mantiene `Common` ≤ 45 % y `Legendary` ≤ 8 %. `cargo test -p sow-data` falla si un héroe nuevo rompe su banda, repite un precio o cambia esas proporciones.
- `card` es opcional y se reserva para héroes registrados con su token NFTROPOLY/WOU-ID; no inventes un token para un héroe nuevo.
- `tower_range_bonus_percent` es un efecto secundario adicional que puede coexistir con la habilidad principal; déjalo en cero salvo que el diseño lo requiera.
- `character.name` y las claves localizadas deben concordar con el nombre público; `hero.name` mantiene continuidad del progreso y debe cambiarse con cuidado.

## 3. Habilidad, efecto y señal visual

Las habilidades viven en `sow-data/src/geo_entities/mod.rs` y se ejecutan en `sow-core`. Reutiliza una situación ya soportada siempre que represente bien la evidencia: tipo de objetivo, terreno, edificio, alianza, retirada o momento del combate.

Cada héroe debe tener una situación propia. `Ability::situation()` elimina su magnitud y permite comprobar que dos héroes no compartan el mismo caso. Las acciones fijas, como fundar un puerto, conservan su tipo. Una habilidad situacional hace que el efecto tenga una condición visible y comprobable; evita bonos globales que no expresen la historia o que se activen en cualquier momento.

- Revisa todos los héroes actuales antes de escoger: `every_hero_has_a_unique_situation` comprueba la unicidad.
- No agregues un tipo de habilidad para darle un nombre exclusivo a un héroe. Si el motor no puede expresar una acción respaldada por fuentes, extiende el modelo y el flujo real de combate, ingreso o campaña, no solo el texto.
- El efecto real debe pasar por la fórmula existente y emitir `GameEvent::AbilityTriggered` cuando se aplica. `Ability::cue()` selecciona su familia visual para el mapa o HUD.
- Mantén nombre y descripción del efecto acordes con la regla, el porcentaje y la condición exactos. La tabla `docs/habilidades-heroes.md` es generada y no se edita a mano.
- Los efectos de construcción, captura, ingresos situacionales y presas navales se integran mediante `sow-core/src/execution/hero_effects.rs`, llamado desde sus dueños de ejecución. Los límites por ciudad o partida se guardan en el checkpoint. El cálculo de ingreso adicional es compartido con el HUD; los avisos muestran el icono, el nombre y la regla localizada.
- El mismo dueño deriva `hero_ability_feedback`: lista para su disparador automático, esperando condición, activa, agotada o no disponible, con segundos restantes cuando existe una ventana real. El HUD añade «Se activó» a partir del aviso real de activación. El indicador junto a los recursos permite abrir la regla con ratón, teclado o toque; una habilidad automática no se presenta como botón para lanzarla. Cada héroe debe tener estado comprobable, además de su señal de activación.

## 4. Textos en los idiomas publicados

Agrega estas cinco claves para el mismo ID en los 15 archivos `sow-i18n/strings/<locale>/web.toml`:

- En `[heroes]`: `leader_<id>_name`, `leader_<id>_historical` y `leader_<id>_civilization`.
- En `[site]`: `leader_<id>_ability` y `leader_<id>_description`.

Idiomas publicados: `ar`, `de`, `en`, `es`, `fil`, `fr`, `id`, `it`, `ja`, `ko`, `pt-br`, `ru`, `tr`, `vi` y `zh-cn`. Traduce también el nombre de la habilidad y explica la misma condición y porcentaje; no dejes claves en inglés por accidente ni marcadores de traducción.

## 5. Biografía pública

Las biografías viven en `sow-web/site/leader-bios/<id_del_héroe>.toml`, un archivo por héroe. Declara cada artículo bajo su idioma de origen, por ejemplo `[articles.es]` o `[articles.en]`, y conserva íntegros sus campos: idioma, URL HTTPS, título, introducción, secciones y fuentes. Cada idioma publicado debe resolver un artículo. Cuando una página reutilice un texto escrito en otro idioma, decláralo explícitamente en `[fallbacks]`, por ejemplo `fr = "en"`; la clave es el idioma publicado y el valor debe coincidir con una clave de `[articles]`. Conserva los códigos históricos tal como están escritos. El cargador no normaliza idiomas ni infiere sustituciones. Los artículos breves deben cumplir el mínimo de 700 caracteres combinados entre `highlight`, `intro` y párrafos; los demás requieren 1.200. El generador verifica longitud, referencias ausentes, fuentes y artículos sin uso.

Usa fuentes que sostengan las afirmaciones, y atribuye explícitamente la incertidumbre cuando cambien la fecha, la autoría o el sentido del evento. Una referencia wiki no reemplaza una búsqueda de fuentes mejores cuando estas existen.

`sow-web/site/leader-sagas.toml` es contenido narrativo del sitio, no una campaña jugable. Es opcional: cada idioma publicado aporta una síntesis de 70–140 palabras y exactamente cinco capítulos; el texto conjunto de los capítulos suma 1.000–2.200 palabras. Cada capítulo requiere al menos una fuente y cada referencia debe apuntar a un héroe y a un idioma publicados.

Una campaña jugable es otro sistema. Si el héroe será asesor o recompensa de una campaña, sus IDs deben concordar con `CampaignEpisode`/`CampaignSeries` en `sow-data/src/campaign.rs` y los archivos de episodio/triggers en `assets/campaign/`. Comprueba también sus traducciones y publicación; no agregues estos datos para un héroe que no tendrá campaña.

## 6. Arte y rutas

`assets/art/README.md` es la referencia canónica para la preparación del arte. Los originales se guardan en `assets/art/`; registra la procedencia/licencia correcta en `assets/SOURCES.toml` cuando corresponda y no edites `assets/generated/` ni los artefactos publicados.

- Retrato obligatorio: `avatars/heroes/<art_slug>.webp`, una lámina 2×2 en orden Happy, Calm, Fear y Angry. El publicador recorta las cuatro expresiones; Calm también es el retrato estático. La lámina no se publica.
- Icono obligatorio: `icons/abilities/<art_slug>.webp`, cuadrado y reconocible a tamaño pequeño.
- Escenas recomendadas para menú, carga y página pública: `heroes/<art_slug>_desktop.webp` y `heroes/<art_slug>_mobile.webp`. Se publican en `assets/shell/leaders/`; si falta una variante, su consumidor no tiene imagen de sustitución.
- Rutas runtime generadas: avatar en `assets/gameplay/avatars/<art_slug>.webp`, expresiones en `assets/gameplay/avatars/expressions/` e icono en `assets/gameplay/icons/abilities/`. Placas, anillo de emotes y radial cargan esos archivos; ver `docs/nameplates-and-emotes.md`. El inventario de assets comprueba la lámina, el icono y los cinco textos en cada idioma.

Los prompts deben fijar época, ropa, composición, relación de aspecto, espacio de interfaz y elementos que no deben aparecer. Si el arte lo produce otra persona, entrega los prompts y espera sus archivos finales; no rellenes las rutas con una imagen de otra identidad.

## 7. Verificación

Desde la raíz del repositorio:

```sh
python3 scripts/export-hero-abilities.py
cargo test -p sow-core every_hero_ability_changes_the_game_and_shows_its_mark
cargo test -p sow-core every_hero_has_a_unique_situation
cargo test -p sow-core <filtro_del_test_especifico_del_heroe>
cargo test -p sow-dist published_biographies_cover_every_catalog_hero
cargo test -p sow-dist landing_roster_lists_every_catalog_hero
bun test sow-web/shell/assets_layout.test.ts
cargo check -p sow-dist
```

La primera prueba mide a cada héroe mediante la fórmula real y comprueba su señal visual; la segunda protege la unicidad; el test de assets comprueba roster, retratos, iconos y textos localizados. Si el cambio altera escenarios deterministas de bots, revisa las referencias S16 de `sow-core/src/intent/nation/bot_lab.rs` y actualiza solo las que realmente cambien. Para comprobar el sitio completo usa `./sow l` y espera a que termine de generar y servir la salida; la revisión visual final corresponde a quien acepta el arte. `./sow p` es el pipeline de publicación y solo se usa cuando la publicación esté autorizada.
