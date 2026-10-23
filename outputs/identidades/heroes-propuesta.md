# Héroes: roles y lista de 36

Estado: los 36 héroes están en el catálogo con sus textos en 15 idiomas y su efecto implementado. La regla de roles está aplicada. Las caras y escenas están en el repositorio; las marcadas como provisionales en la sección 4 hay que revisarlas.

## 1. Roles: qué confunde

Hoy cada personaje del catálogo tiene un rol con tres valores:

- **nación**: sale como IA de nación. Hay 220.
- **tribu**: sale como IA de tribu. Hay 400.
- **héroe**: jugable. Hay 8.

Lo que está mal, verificado en el código y el catálogo:

1. **El valor "héroe" no lo usa ninguna línea de código.** Los 8 héroes salen de la IA porque el código solo toma los de rol "nación" o "tribu". El dato que de verdad dice "es jugable" es el bloque `hero` que ya tienen.
2. **Los 4 héroes con rol "tribu" sí salen como IA.** Son Ragnar, Genghis Khan, Vercingetorix y Boudica. Por eso la lista de tribus de IA tiene 404 entradas, 400 tribus más esos 4 héroes. Un héroe puede ser jugable y también aparecer como IA.
3. **Un héroe de nación o de tribu obliga a cambiarle el rol**, y así el rol deja de decir de dónde viene.

Qué hace hoy la etiqueta de rol: decide en qué lista de IA entra el personaje (nación o tribu) y qué tipo de jugador se crea cuando sale como IA. Nada más. El conteo de naciones y tribus de la partida sale de ahí (el tope de naciones es 20 % de la IA en emparejamiento).

### Propuesta: una sola regla

- **Origen**: nación o tribu. Solo dice en qué lista de IA cae el personaje cuando no es héroe.
- **Héroe**: no es un rol. Es tener el bloque `hero`. Si lo tiene, es jugable y nunca sale como IA, sin importar su origen.
- Se quita el valor "héroe". Los 8 héroes quedan con su origen real (nación). No cambia ninguna partida, porque ya estaban fuera de la IA.
- Los 4 héroes-tribu dejan de salir como IA, porque el filtro pasa a mirar el bloque `hero`. La lista de tribus de IA baja de 404 a 400.
- **Para agregar un héroe de la lista de naciones o de tribus:** solo se le agrega el bloque `hero`. No se toca el origen. Sale de la IA sola.

Cambio de código (cuando se apruebe):
- `sow-data/src/geo_entities/mod.rs`: el filtro de IA excluye las entradas con bloque `hero` (línea 188). Se quita `Hero` del enum `Role` (líneas 8 a 13).
- `assets/geo_entities.json`: los 8 héroes con rol "héroe" pasan a "nación".
- La lista de naciones de IA sigue en 220. La de tribus baja a 400.

Los nombres cortos de los héroes se guardan como clave del progreso de cada jugador. No se renombran.

## 2. Los 12 actuales

Todos cambian la partida: la prueba `every_hero_ability_changes_the_game` (`sow-core/src/execution/hero_ability_tests.rs`) pasó en el despliegue. Esa prueba confirma que la habilidad hace algo. No confirma que el balance esté bien.

| # | Héroe | Origen (rol hoy) | Región | Género | Habilidad | Dónde se aplica | Nota |
|---|---|---|---|---|---|---|---|
| 1 | Julius Caesar | héroe | Europa | M | Tropas en combate +10 % | `sow-core/src/execution/combat_tiles.rs` | |
| 2 | Cleopatra | héroe | África | F | Oro de fábricas +50 % | `sow-core/src/execution/income_rates.rs` | Igual que Lady Six Sky |
| 3 | Ragnar (Danish Vikings) | tribu | Europa | M | Oro de puertos +50 % | `income_rates.rs:52` | Sale también como IA |
| 4 | Sun Tzu | héroe | Asia | M | Alcance de búnkeres +50 % | `sow-core/src/engine/mod.rs` | |
| 5 | Alexander | héroe | Europa | M | Expansión +15 % | `combat_tiles.rs:123` | Mismo tipo que Napoleon |
| 6 | Genghis Khan (Borjigin Mongols) | tribu | Asia | M | Oro al eliminar a un jugador +50 % | `sow-core/src/engine/mod.rs` | Sale también como IA |
| 7 | Richard the Lionheart | héroe | Europa | M | Tope de tropas de ciudades +50 % | `sow-core/src/execution/income.rs` | |
| 8 | Vercingetorix (Arverni) | tribu | Europa | M | Tropas de ciudades +50 % | `income_rates.rs:24` | Sale también como IA |
| 9 | Boudica (Iceni) | tribu | Europa | F | Oro de granjas +50 % | `income_rates.rs:49` | Sale también como IA. Gratis |
| 10 | Lady Six Sky | héroe | Américas | F | Oro de fábricas +50 % | `income_rates.rs:46` | Igual que Cleopatra |
| 11 | Leonidas I | héroe | Europa | M | Búnkeres: atacantes pierden más tropas +50 % | `combat_tiles.rs:241` | |
| 12 | Napoleon | héroe | Europa | M | Expansión +20 % | `combat_tiles.rs:123` | Mismo tipo que Alexander |

Totales hoy: 9 hombres, 3 mujeres. Europa 8, Asia 2, África 1, Américas 1, Oceanía 0.

Repetidas: Cleopatra y Lady Six Sky hacen lo mismo. Alexander y Napoleon usan el mismo tipo de efecto. No bloquean nada. Conviene cambiar una de cada par cuando se haga el balance.

## 3. Los 24 héroes: efectos implementados

Los 24 nuevos quedaron así. Los números de los efectos nuevos (25 %, 30 % y 50 %) y las bases de ingreso son propuestos y se ajustan en el balance.

| # | Héroe | Región | Género | Habilidad | Efecto | Cómo está programado |
|---|---|---|---|---|---|---|
| 1 | Rani Durgavati | Asia | F | Cosecha gond | Granjas dan 50 % más tropas | Ya existía |
| 2 | Empress Gi | Asia | F | Tesoro imperial | Ciudades dan 50 % más oro | Base de oro por ciudad solo para Gi (`hero_city_gold_income`) |
| 3 | Ashoka | Asia | M | Arsenal del Dharma | Fábricas dan 50 % más tropas | Base de tropas por fábrica solo para Ashoka (`hero_factory_troop_income`) |
| 4 | Amina de Zazzau | África | F | Murallas de Zazzau | Enemigos necesitan 50 % más esfuerzo para tomar tus ciudades | Tipo nuevo `city_defense` |
| 5 | Yaa Asantewaa | África | F | Unir al pueblo | Ciudades aguantan 50 % más tropas | Ya existía |
| 6 | Makeda | África | F | Comercio de caravanas | Territorio da 50 % más oro | Tipo nuevo `territory_gold` |
| 7 | Hannibal | África | M | Cruce de los Alpes | Tropas 25 % más fuertes contra otros jugadores | Tipo nuevo `enemy_troop_strength` |
| 8 | Anacaona | Américas | F | Puertos de comercio | Puertos dan 50 % más tropas | Base de tropas por puerto solo para Anacaona (`hero_port_troop_income`) |
| 9 | Simón Bolívar | Américas | M | Pactos del Libertador | Alianzas de campaña cuestan 30 % menos oro (entra en lugar de Awashonks) | Tipo `alliance_cost`, el mismo de Awashonks |
| 10 | Lozen | Américas | F | Asaltante veloz | Expansión 15 % más rápida | Ya existía |
| 11 | Sālote Tupou III | Oceanía | F | Pacto del Pacífico | Cada alianza activa da 50 % más oro | Tipo nuevo `alliance_gold`, base `alliance_gold_income` (0,25 por alianza y segundo) solo para ella |
| 12 | Hongi Hika | Oceanía | M | Rompe-fortalezas | Ataques contra ciudades 50 % más fuertes | Tipo nuevo `city_assault` |
| 13 | Juana de Arco | Europa | F | Baluarte de la fe | Búnkeres: atacantes pierden 50 % más tropas | Tipo `bunker_defense`, igual que Leonidas |
| 14 | Catalina la Grande | Europa | F | Patronazgo imperial | Cada alianza activa da 50 % más oro | Tipo `alliance_gold`, igual que Sālote |
| 15 | Teodora | Europa | F | Justicia imperial | Ataques contra traidores hacen 100 % más daño | Tipo `damage_to_traitors`, igual que Boudica |
| 16 | Ciro el Grande | Asia | M | Guarniciones persas | Ciudades aguantan 50 % más tropas | Tipo `city_troop_capacity`, igual que Yaa Asantewaa |
| 17 | Saladino | Asia | M | Rompemuros | Ataques contra ciudades 50 % más fuertes | Tipo `city_assault`, igual que Hongi Hika |
| 18 | Solimán el Magnífico | Asia | M | Legiones del sultán | Tropas 25 % más fuertes contra otros jugadores | Tipo `enemy_troop_strength`, igual que Hannibal. La propuesta decía −25 %; el efecto correcto es +25 % |
| 19 | Tomoe Gozen | Asia | F | Carga a caballo | Expansión 15 % más rápida | Tipo `expansion_speed`, igual que Lozen |
| 20 | Lady Triệu | Asia | F | Fortalezas de montaña | Enemigos necesitan 50 % más esfuerzo para tomar tus ciudades | Tipo `city_defense`, igual que Amina |
| 21 | Mansa Musa | África | M | Peregrinación dorada | Territorio da 50 % más oro | Tipo `territory_gold`, igual que Makeda |
| 22 | Shaka | África | M | Impi de hierro | Ejércitos 25 % más fuertes | Tipo `combat_strength`, igual que Caesar (con 25 % en vez de 10 %) |
| 23 | Nzinga | África | F | Recompensa de la reina | Eliminar a un jugador da 50 % más oro | Tipo `elimination_gold`, igual que Genghis Khan |
| 24 | Kamehameha I | Oceanía | M | Fortalezas de la isla | Búnkeres defienden 50 % más área | Tipo `bunker_range`, igual que Sun Tzu |

Los 12 anteriores (sección 2) siguen igual. Con los 36, la lista tiene 19 hombres y 17 mujeres. Europa 11, Asia 10, África 8, Américas 4, Oceanía 3.

Cada ingreso base nuevo (`hero_city_gold_income`, `hero_factory_troop_income`, `hero_port_troop_income`, `alliance_gold_income`) solo lo recibe el héroe que lo usa. Los demás jugadores no cambian su economía.

La prueba `every_hero_ability_changes_the_game` mide cada habilidad en el juego real, incluidos los 6 tipos nuevos.

## 4. Arte

Cada héroe necesita `assets/art/avatars/heroes/<id>.webp` (cara, 256 px en el juego) y `assets/art/heroes/<id>_desktop.webp` y `<id>_mobile.webp` (escenas). Las caras `_1` a `_4` (expresiones) son opcionales: solo las tienen los 12 héroes de la sección 2. Los demás usan la cara base.

| Héroe | Cara | Escena | Estado |
|---|---|---|---|
| Rani Durgavati | Real | Real | Lista |
| Empress Gi | **Provisional**: recorte de su escena | Real | Revisar la cara |
| Ashoka | Real (`leaders/ashoka_heroic_portrait.jpg`) | Real | Lista |
| Amina de Zazzau | Real (antes cara de nación) | De Downloads (`leaders/amina_zaria_sunset_charge.jpg` y `amina_warrior_portrait.jpg`) | Revisar la escena |
| Yaa Asantewaa | Real (antes cara de nación) | **Provisional**: recorte de la cara | Revisar la escena |
| Makeda | Real (antes cara de nación) | De Downloads (`leaders/queen_makeda_sunset.jpg` y `queen_makeda_portrait.jpg`) | Revisar la escena |
| Hannibal | **Provisional**: recorte de su escena | Real | Revisar la cara |
| Anacaona | Real (antes cara de nación) | De Downloads (`leaders/anacaona_golden_sunset.jpg` y `anacaona_heroic_portrait.jpg`) | Revisar la escena |
| Simón Bolívar | Real (`leaders/simon_bolivar_hero_1.jpg`) | **Provisional**: recortes de su retrato; en la vertical se quitó el letrero de abajo | Revisar la escena |
| Lozen | Real (antes cara de nación) | **Provisional**: recorte de la cara | Revisar la escena |
| Sālote Tupou III | Real (antes cara de nación) | **Provisional**: ampliación de la cara; no hay escena | Falta escena real |
| Hongi Hika | Real (antes cara de nación) | Real (`old/hongi_hika_desktop.jpg` y `_mobile.jpg`) | Lista |
| Juana de Arco | Real (`old/juan_de_arco_mobile.jpg`) | De Downloads (`old/juan_de_arco_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Catalina la Grande | Real (`leaders/catherine_imperial_portrait.jpg`) | **Provisional**: recortes de su retrato (`leaders/catherine_great_pixar.jpg` y `catherine_imperial_portrait.jpg`) | Revisar la escena |
| Teodora | Real (`leaders/theodora_portrait.jpg`) | **Provisional**: recortes de su retrato (`leaders/theodora_hagia_sophia.jpg` y `theodora_portrait.jpg`) | Revisar la escena |
| Ciro el Grande | Real (`old/Cyrus the Great_mobile.jpg`) | De Downloads (`old/Cyrus the Great _desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Saladino | Real (`old/saladin_mobile.jpg`) | De Downloads (`old/saladin_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Solimán el Magnífico | **Provisional**: copia del retrato de nación (`avatars/nations/suleiman_el_magnifico.webp`) | **Falta**: no hay escena | Falta arte |
| Tomoe Gozen | Real (`old/tomoe_gozen_mobile.jpg`) | De Downloads (`old/tomoe_gozen_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Lady Triệu | Real (`old/lady_trieu_mobile.jpg`) | De Downloads (`old/lady_trieu_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Mansa Musa | Real (`old/mansa_musa_mobile.jpg`) | De Downloads (`old/mansa_musa_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Shaka | Real (`old/shaka_zulu_mobile.jpg`) | De Downloads (`old/shaka_zulu_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Nzinga | Real (`old/nzinga_mbande_mobile.jpg`) | De Downloads (`old/nzinga_mbande_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Kamehameha I | Real (`old/kamehameha_i_mobile.jpg`) | De Downloads (`old/kamehameha_i_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |

Las escenas nuevas vienen de `~/Downloads` sin revisión visual. El dueño las revisa antes de la publicación final.

Las cinco caras que eran de nación (Amina, Makeda, Anacaona, Sālote y Hongi) se movieron a `avatars/heroes/`. La lista de naciones bajó a 213 (`naciones-encargo.txt`).

## 5. Capacidad del atlas de caras

Cada héroe usa 4 casillas del atlas. Con 36 héroes son 146 casillas, y las 46 restantes quedan para retratos de bots y campañas.

El atlas pasó de 8 × 16 celdas (128 casillas, 4096 × 2048 px) a 16 × 12 celdas (192 casillas, 4096 × 3072 px). Con 36 héroes quedan 46 casillas libres para bots y campañas. Las 24 filas de 8 columnas habrían dado 6144 px de alto, y muchos celulares viejos no soportan más de 4096 px por lado. El costo es ~17 MB más de memoria.

## 6. Decisiones

1. Regla de roles: aplicada.
2. Lista de 12 con Yaa Asantewaa y Lozen con efectos existentes: aplicada. Luego se agregaron los 9 restantes (sección 3).
3. Atlas: 16 × 12 celdas (no 24 filas × 8 columnas, por el límite de textura de celulares viejos).
4. Ingresos y costos nuevos: solo para su héroe, con bases en la configuración de juego.
5. Lista de 13 aprobada: entra Simón Bolívar en lugar de Awashonks, y se suman 12 más. Solimán recibe el efecto que ya usa Hannibal (tropas 25 % más fuertes contra otros jugadores). La propuesta decía −25 %; era un error.

## 7. Pendientes conocidos

- El precio que muestra el diálogo de alianza de campaña sigue siendo el precio base. El descuento de Simón Bolívar se aplica en el servidor al cobrar, así que el jugador paga menos de lo que se muestra. Hay que mostrar el precio con descuento (lo hace el agente de habilidades y efectos).
- Los porcentajes y bases nuevas son propuestos y los ajusta el agente de habilidades y efectos.
- Escenas y caras provisionales: sección 4.
- Solimán no tiene arte: cara provisional y sin escena.
- Doce de los nuevos ya son personajes de la IA de nación (todos menos Tomoe Gozen). Hoy salen en las dos listas. Pendiente: decidir si salen de la IA de nación.
