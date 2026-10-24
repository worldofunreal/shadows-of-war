# Héroes: roles y lista de 63

Estado: los 63 héroes están en el catálogo con sus textos en 15 idiomas y su habilidad implementada. La regla de roles está aplicada. Las caras, escenas e iconos están en el repositorio, salvo los de Isabel I de Castilla y las escenas de Solimán (sección 4). Las escenas y caras provisionales de la sección 4 hay que revisarlas en partida. Las naciones quedan en 204 con retrato; faltan 9 retratos (sección 7).

## 1. Roles: qué confunde

Antes del cambio de roles, cada personaje del catálogo tenía un rol con tres valores:

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

Las habilidades de estos 12 cambiaron con la revisión por historia. La lista vigente está en [docs/habilidades-heroes.md](../../docs/habilidades-heroes.md).

| # | Héroe | Origen (rol hoy) | Región | Género |
|---|---|---|---|---|
| 1 | Julius Caesar | héroe | Europa | M |
| 2 | Cleopatra | héroe | África | F |
| 3 | Ragnar (Danish Vikings) | tribu | Europa | M |
| 4 | Sun Tzu | héroe | Asia | M |
| 5 | Alexander | héroe | Europa | M |
| 6 | Genghis Khan (Borjigin Mongols) | tribu | Asia | M |
| 7 | Richard the Lionheart | héroe | Europa | M |
| 8 | Vercingetorix (Arverni) | tribu | Europa | M |
| 9 | Boudica (Iceni) | tribu | Europa | F |
| 10 | Lady Six Sky | héroe | Américas | F |
| 11 | Leonidas I | héroe | Europa | M |
| 12 | Napoleon | héroe | Europa | M |

Totales de estos 12: 9 hombres, 3 mujeres. Europa 8, Asia 2, África 1, Américas 1, Oceanía 0.

Repetidas: Cleopatra y Lady Six Sky hacen lo mismo. Alexander y Napoleon usan el mismo tipo de efecto. No bloquean nada. Conviene cambiar una de cada par cuando se haga el balance.

## 3. Habilidades de los 63

Cada héroe tiene una habilidad única y situacional, con su marca visual cuando la situación aplica. La tabla vigente (héroe, habilidad, descripción, tipo y número) está en [docs/habilidades-heroes.md](../../docs/habilidades-heroes.md), generada desde el catálogo. No se copia aquí.

La prueba `every_hero_ability_changes_the_game_and_shows_its_mark` (`sow-core/src/execution/hero_ability_tests.rs`) mide cada habilidad en el juego real: debe cambiar la partida, debe mostrar su marca en su situación y no debe mostrarla en la situación de un héroe sin ella.

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
| Isabel I de Castilla | Por encargar | — | Falta escena, retrato y icono de habilidad |
| Ciro el Grande | Real (`old/Cyrus the Great_mobile.jpg`) | De Downloads (`old/Cyrus the Great _desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Saladino | Real (`old/saladin_mobile.jpg`) | De Downloads (`old/saladin_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Solimán el Magnífico | **Provisional**: copia del retrato de nación (`avatars/nations/suleiman_el_magnifico.webp`) | **Falta**: no hay escena | Falta arte |
| Tomoe Gozen | Real (`old/tomoe_gozen_mobile.jpg`) | De Downloads (`old/tomoe_gozen_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Lady Triệu | Real (`old/lady_trieu_mobile.jpg`) | De Downloads (`old/lady_trieu_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Mansa Musa | Real (`old/mansa_musa_mobile.jpg`) | De Downloads (`old/mansa_musa_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Shaka | Real (`old/shaka_zulu_mobile.jpg`) | De Downloads (`old/shaka_zulu_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Nzinga | Real (`old/nzinga_mbande_mobile.jpg`) | De Downloads (`old/nzinga_mbande_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |
| Kamehameha I | Real (`old/kamehameha_i_mobile.jpg`) | De Downloads (`old/kamehameha_i_desktop.jpg` y `_mobile.jpg`) | Revisar la escena |

Los 27 héroes nuevos tienen cara en `avatars/heroes/`, escenas `_desktop` y `_mobile` en `heroes/` e icono en `icons/abilities/`; todos los archivos existen. Las escenas nuevas vienen de `~/Downloads` sin revisión visual; el dueño las revisa en partida antes de la publicación final.

Las siete caras que eran de nación (Yaa Asantewaa, Lozen, Amina, Makeda, Anacaona, Sālote y Hongi) están ahora en `avatars/heroes/`. La lista de naciones quedó en 204 con retrato; 9 esperan encargo (`naciones-encargo.txt`).

## 5. Capacidad del atlas de caras

Los 12 héroes con caras de expresión usan 4 casillas cada uno; los otros 51 usan 1. Con los 63 héroes son 99 casillas, y con las 2 fijas (relleno y dorada) los héroes llegan a la casilla 101. Quedan 91 casillas para retratos de campaña y bots.

El atlas pasó de 8 × 16 celdas (128 casillas, 4096 × 2048 px) a 16 × 12 celdas (192 casillas, 4096 × 3072 px). Con 4 casillas para cada héroe harían falta 254 casillas: en 4096 px solo quedarían 2 para retratos de campaña y bots, y subir a 17 filas (4352 px) rompe celulares viejos. Por eso solo los 12 con expresiones usan 4. Las 24 filas de 8 columnas habrían dado 6144 px. El costo es ~17 MB más de memoria.

## 6. Decisiones

1. Regla de roles: aplicada.
2. Lista de 12 con Yaa Asantewaa y Lozen con efectos existentes: aplicada. Luego se agregaron los 9 restantes (sección 3).
3. Atlas: 16 × 12 celdas (no 24 filas × 8 columnas, por el límite de textura de celulares viejos).
4. Ingresos y costos nuevos: solo para su héroe, con bases en la configuración de juego.
5. Lista de 13 aprobada: entra Simón Bolívar en lugar de Awashonks, y se suman 12 más. Solimán recibe el efecto que ya usa Hannibal (tropas 25 % más fuertes contra otros jugadores). La propuesta decía −25 %; era un error.
6. Lista de 27 nuevos aprobada. Tomyris se reemplaza por Mulan (Asia, mujer): la Reina de Saba del plan es Makeda, que ya es héroe.
7. Caras de expresión: solo los 12 héroes de la sección 2 las tienen. Los demás muestran su cara base; en el anillo de emociones muestran el icono genérico, como hoy.
8. Naciones: se publica con 204 bots de nación con retrato. Los 9 que faltan entran al juego cuando tengan retrato; no se publican sin cara.
9. Habilidades: cada una tiene una situación que ningún otro héroe usa. La versión vigente está en [docs/habilidades-heroes.md](../../docs/habilidades-heroes.md).

## 7. Pendientes conocidos

- Sin situación con respaldo histórico: Ragnar, Anacaona, Makeda, Empress Gi, Ashoka, Simón Bolívar, Tomoe Gozen, Gajah Mada, Sayyid Said, Mary of Burgundy, Semiramis, Shima Kalingga, Gorgo, Tenamaxtli, Tomochichi y Shajar al-Durr. Tienen la situación más cercana que el motor puede expresar con sus 76 reglas; para un encaje real hace falta una regla nueva.
- El precio que muestra el diálogo de alianza de campaña sigue siendo el precio base. El descuento de Simón Bolívar se aplica en el servidor al cobrar, así que el jugador paga menos de lo que se muestra. Hay que mostrar el precio con descuento (lo hace el agente de habilidades y efectos).
- Los porcentajes y bases nuevas son propuestos y los ajusta el agente de habilidades y efectos.
- Escenas y caras provisionales: sección 4.
- Solimán no tiene arte: cara provisional y sin escena.
- Doce de los nuevos ya son personajes de la IA de nación (todos menos Tomoe Gozen). Hoy salen en las dos listas. Pendiente: decidir si salen de la IA de nación.
- Nueve naciones sin retrato: Francisco de Miranda, Miguel Hidalgo, Túpac Katari, Osceola, Hatuey, Lempira, Tecún Umán, Ezana de Aksum y Khama III. Se encargan (`naciones-encargo.txt`) y entran al juego cuando llegue cada retrato.
- Tzintzicha: el enlace va al artículo del Imperio purépecha. No encontré un artículo en inglés del líder.
