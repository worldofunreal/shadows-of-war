# Héroes: roles y propuesta de los próximos 12

Estado: propuesta. Nada de código ni del catálogo cambió todavía.

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

Todos cambian la partida: la prueba `every_hero_ability_changes_the_game` (`sow-core/src/execution/hero_ability_tests.rs:182`) pasó en el despliegue. Esa prueba confirma que la habilidad hace algo. No confirma que el balance esté bien.

| # | Héroe | Origen (rol hoy) | Región | Género | Habilidad | Dónde se aplica | Nota |
|---|---|---|---|---|---|---|---|
| 1 | Julius Caesar | héroe | Europa | M | Tropas en combate +10 % | `sow-core/src/execution/combat_tiles.rs:90` | |
| 2 | Cleopatra | héroe | África | F | Oro de fábricas +50 % | `sow-core/src/execution/income_rates.rs:46` | Igual que Lady Six Sky |
| 3 | Ragnar (Danish Vikings) | tribu | Europa | M | Oro de puertos +50 % | `income_rates.rs:52` | Sale también como IA |
| 4 | Sun Tzu | héroe | Asia | M | Alcance de búnkeres +50 % | `sow-core/src/engine/mod.rs:718` | |
| 5 | Alexander | héroe | Europa | M | Expansión +15 % | `combat_tiles.rs:123` | Mismo tipo que Napoleon |
| 6 | Genghis Khan (Borjigin Mongols) | tribu | Asia | M | Oro al eliminar a un jugador +50 % | `sow-core/src/engine/mod.rs:851` | Sale también como IA |
| 7 | Richard the Lionheart | héroe | Europa | M | Tope de tropas de ciudades +50 % | `sow-core/src/execution/income.rs:66` | |
| 8 | Vercingetorix (Arverni) | tribu | Europa | M | Tropas de ciudades +50 % | `income_rates.rs:24` | Sale también como IA |
| 9 | Boudica (Iceni) | tribu | Europa | F | Oro de granjas +50 % | `income_rates.rs:49` | Sale también como IA. Gratis |
| 10 | Lady Six Sky | héroe | Américas | F | Oro de fábricas +50 % | `income_rates.rs:46` | Igual que Cleopatra |
| 11 | Leonidas I | héroe | Europa | M | Búnkeres: atacantes pierden más tropas +50 % | `combat_tiles.rs:241` | |
| 12 | Napoleon | héroe | Europa | M | Expansión +20 % | `combat_tiles.rs:123` | Mismo tipo que Alexander |

Totales hoy: 9 hombres, 3 mujeres. Europa 8, Asia 2, África 1, Américas 1, Oceanía 0.

Repetidas: Cleopatra y Lady Six Sky hacen lo mismo. Alexander y Napoleon usan el mismo tipo de efecto. No bloquean nada. Conviene cambiar una de cada par cuando se haga el balance.

## 3. Propuesta de los próximos 12

Género y región pensados para que el total quede en 12 hombres y 12 mujeres, y para cubrir Oceanía y Américas.

| # | Héroe | Región | Género | Habilidad propuesta | Qué falta en código | Ya está en catálogo |
|---|---|---|---|---|---|---|
| 1 | Rani Durgavati | Asia | F | Granjas dan tropas +50 % | Nada: el camino ya existe y nadie lo usa | No (se quitó al agregar Qin Liangyu) |
| 2 | Empress Gi | Asia | F | Ciudades dan oro +50 % | 1 línea | No |
| 3 | Ashoka | Asia | M | Fábricas dan tropas +50 % | 1 línea | No (se quitó al agregar Gajah Mada) |
| 4 | Amina de Zazzau | África | F | Ciudades más fuertes al ser atacadas (tipo nuevo) | Tipo nuevo | Sí, como nación |
| 5 | Yaa Asantewaa | África | F | Reclutamiento más barato | Tipo nuevo y **costo de tropas, que hoy no existe** | Sí, como nación |
| 6 | Makeda | África | F | Caravanas: más oro por territorio (tipo nuevo) | Tipo nuevo | Sí, como nación |
| 7 | Hannibal | África | M | Alpes: sus tropas ganan fuerza en territorio enemigo (tipo nuevo) | Tipo nuevo | No |
| 8 | Anacaona | Américas | F | Puertos dan tropas +50 % | 1 línea | Sí, como nación |
| 9 | Awashonks | Américas | F | Alianzas más baratas (tipo nuevo) | Tipo nuevo. Las alianzas ya existen | No |
| 10 | Lozen | Américas | F | Refuerzos enemigos tardan más | Tipo nuevo y **refuerzos, que hoy no existen** | Sí, como nación |
| 11 | Sālote Tupou III | Oceanía | F | Oro por cada alianza activa (tipo nuevo) | Tipo nuevo. "Tratado de paz" no existe; se cambió a alianzas | Sí, como nación |
| 12 | Hongi Hika | Oceanía | M | Ataques contra ciudades más fuertes (tipo nuevo) | Tipo nuevo | Sí, como nación |

Resultado con los 24: 12 hombres y 12 mujeres. Europa 8, Asia 5, África 5, Américas 4, Oceanía 2.

Ajustes frente a la lista anterior:
- **Hannibal:** "menos bajas en ataque" repetiría el tipo de César. Se cambió.
- **Sālote:** "tratado de paz" no existe en el juego. Las alianzas sí, así que el oro va por alianza activa.
- **Yaa Asantewaa y Lozen:** necesitan mecanismos que hoy no existen (costo de tropas, refuerzos). Es más trabajo que un tipo de efecto. Si lo prefieres, se cambian por efectos que el juego ya tiene.

Tipos de efecto nuevos: 8 (Amina, Yaa, Makeda, Hannibal, Awashonks, Lozen, Sālote, Hongi). Cada uno necesita código, enlace y prueba. Los casos de "1 línea" son combinaciones de ingreso que el código todavía no conecta (ciudades → oro, fábricas → tropas, puertos → tropas). Rani Durgavati sí tiene camino ya conectado.

Cada héroe nuevo: nombre corto único (no se renombra después), región, género, bloque `hero` con habilidad, color de territorio, y arte (sección 4).

## 4. Arte: dónde está

Un héroe jugable necesita estos archivos (ver `assets/art/README.md`, líneas 18 a 29):

- `assets/art/avatars/heroes/<id>.webp`: cara base, 256 px.
- `assets/art/avatars/heroes/<id>_1.webp` a `_4.webp`: cuatro caras (feliz, calmado, miedo, enojado).
- `assets/art/heroes/<id>_desktop.webp` y `<id>_mobile.webp`: escena del menú y de carga.

Los 12 actuales tienen los cinco archivos. Ningún héroe nuevo tiene caras en el repositorio.

Estado de los 12 nuevos:

| Héroe | Escena en el repositorio | Caras en el repositorio | Dónde está lo que falta |
|---|---|---|---|
| Rani Durgavati | Sí: `assets/art/heroes/rani_durgavati_desktop.webp` y `_mobile` | No | Fuente sin procesar: `~/Downloads/nations/rani_durgavati.jpg` |
| Empress Gi | Sí: `assets/art/heroes/empress_gi_desktop.webp` y `_mobile` | No | `~/Downloads/leaders/empress_gi_desktop.jpg` y `_mobile.jpg` (copias de la escena) |
| Ashoka | Sí: `assets/art/heroes/ashoka_desktop.webp` y `_mobile` | No | `~/Downloads/leaders/ashoka_heroic_portrait.jpg`, `ashoka_heroic_portrait (1).jpg`, `ashoka_sunset_empire.jpg`; `~/Downloads/nations/ashoka.jpg` |
| Amina de Zazzau | No | Cara de nación: `assets/art/avatars/nations/amina_de_zazzau.webp` | `~/Downloads/leaders/amina_sunset_portrait.jpg`, `amina_warrior_portrait.jpg`, `amina_zaria_sunset_charge.jpg` |
| Yaa Asantewaa | No | Cara de nación: `assets/art/avatars/nations/yaa_asantewaa.webp` | `~/Downloads/leaders/yaa_asantewaa_portrait.jpg` (y `(1)`, `(2)`), `yaa_asantewaa_rally.jpg`, `image_20260927_152307-yaa-asantewaa-...jpg` |
| Makeda | No | Cara de nación: `assets/art/avatars/nations/makeda.webp` | `~/Downloads/leaders/makeda_heroic_portrait.jpg`, `queen_makeda_portrait.jpg`, `queen_makeda_sunset.jpg`, `makeda_amanirenas_triumphant.jpg` |
| Hannibal | Sí: `assets/art/heroes/hannibal_desktop.webp` y `_mobile` | No | Sin caras en Downloads. Copias de la escena en `~/Downloads/old/hannibal_*.jpg` |
| Anacaona | No | Cara de nación: `assets/art/avatars/nations/anacaona.webp` | `~/Downloads/leaders/anacaona_heroic_portrait.jpg`, `anacaona_golden_portrait.jpg`, `anacaona_golden_sunset.jpg` |
| Awashonks | No | No | `~/Downloads/leaders/serene_awashonks_portrait.jpg`; `awashonks_coastal_cliff.jpg` es escena ancha |
| Lozen | No | Cara de nación: `assets/art/avatars/nations/lozen.webp` | `~/Downloads/leaders/heroic_lozen_portrait.jpg` |
| Sālote Tupou III | No | Cara de nación: `assets/art/avatars/nations/salote_tupou_iii.webp` | Nada en Downloads |
| Hongi Hika | No | Cara de nación: `assets/art/avatars/nations/hongi_hika.webp` | `~/Downloads/old/hongi_hika_desktop.jpg` y `_mobile.jpg` (copias viejas) |

Resumen:
- Escena lista en el repositorio: 4 (Rani Durgavati, Empress Gi, Ashoka, Hannibal).
- Caras de héroe listas: 0 de 12.
- Cara de nación ya existente, que sirve como base de la cara de héroe: 7 (Amina, Yaa, Makeda, Anacaona, Lozen, Sālote, Hongi). Igual faltan las 4 caras.
- Sin arte en ningún lado: Awashonks.
- Los 7 que hoy son naciones de IA pasan a héroes: se les agrega el bloque `hero` y salen de la lista de IA.

## 5. Capacidad del atlas de caras

Cada héroe usa 4 casillas del atlas de caras. El atlas tiene 128 casillas (16 filas).

| Héroes | Filas 16 (128 casillas): casillas libres para IA y campañas | Filas 24 (192 casillas): casillas libres |
|---|---|---|
| 12 (hoy) | 78 | 142 |
| 24 (propuesta) | 30 | 94 |

Con 16 filas, 24 héroes dejan 30 casillas para la IA. Eso es poco para 200 naciones y 400 tribus. Con 24 filas quedan 94. Subir a 24 filas suma unos 16 MB de memoria. Se decide según el celular viejo.

## 6. Decisiones

1. ¿Aprobás la regla de roles? Recomendado: sí. Un solo rol de origen (nación o tribu) más el bloque `hero`.
2. ¿Aprobás la lista de 12, o cambiamos alguno?
3. ¿Subimos el atlas de caras a 24 filas? Recomendado: sí.
4. Yaa Asantewaa y Lozen: ¿los dejamos con mecanismos nuevos o los cambiamos por efectos que el juego ya tiene?

No se implementa nada hasta que apruebes.
