# Nameplates y emotes

Contrato del sistema. Leerlo antes de cambiar comunicación, retratos, placas o
compatibilidad de grabaciones. Las instrucciones del proyecto enlazan esta guía.

## Recorrido y responsables

```text
Clic derecho / mantener pulsado sobre territorio propio
  -> capacidades del menú calculadas por Rust
  -> submenú web: cuatro opciones y fijado
  -> ExpressEmote(kind, pinned), dentro de los turnos existentes
  -> motor: validar jugador y guardar ActiveEmote
  -> snapshot: evento, inicio y capacidad de emitir
  -> NameplateSystem: expresión, movimiento y burbuja
  -> snapshot_fx: aviso para el emisor y sus aliados/compañeros
```

| Responsabilidad | Propietario |
| --- | --- |
| Cuatro códigos, expresiones, claves y audiencia | `sow-core/src/emote.rs`: `EMOTES`, `EmoteKind`, `ActiveEmote`, `can_express`, `is_recipient` |
| Aceptar, repetir, liberar y expirar | `sow-core/src/intent/apply.rs`, `engine/tick.rs`, `engine/snapshot.rs` |
| Capacidad contextual y gestos | `sow-client/src/input/map_click.rs`, `input/window.rs` |
| Comandos locales y datos del HUD | `sow-client/src/web_menu.rs` |
| Submenú, teclado, fijado y conexión de idioma | `sow-web/shell/main_menu.hud.js`, `main_menu.hud.css` |
| Descargar y recortar caras compartidas | `sow-client/src/ui/asset_loader/mod.rs`, `asset.rs` |
| Espacios de imágenes y dibujo | `sow-render/src/text/types.rs`, `text/renderer.rs` |
| Posición sobre tierra y prioridad de placas | `sow-render/src/nameplate.rs` |
| Vida del efecto y preparación compartida del avatar | `sow-client/src/render/world/nameplates.rs`: `NameplateSystem`, `avatar_plan`, `paint_prepared_avatar` |
| Curvas de movimiento compartidas con oro | `sow-client/src/render/motion.rs`, `render/world/feedback.rs` |
| Avisos agrupados | `sow-client/src/sim/snapshot_fx.rs`, `ui/hud/state.rs` |
| Lectura de grabaciones actuales | `sow-server/src/replay.rs`; registro de versión en `register_match_start` |

No añadir otro selector, renderizador de placas, textura por jugador, catálogo
de mensajes ni ruta de red para comunicar estas cuatro acciones.

## Contrato de producto

| Código | Expresión | Cuadrante | Mensaje español |
| --- | --- | --- | --- |
| `greet` | `Calm` | Superior derecho | ¡Saludos! |
| `help` | `Fear` | Inferior izquierdo | ¡Ayuda! |
| `threat` | `Angry` | Inferior derecho | ¡Prepárate! |
| `celebrate` | `Happy` | Superior izquierdo | ¡Victoria! |

- El motor permite emitir a humanos vivos, aparecidos y jugando; los humanos
  controlados automáticamente no emiten. El menú respeta además observación
  y las restricciones de entrada del tutorial.
- Cada emisión aceptada recibe un número nuevo, incluso si repite el código.
  Reemplaza el evento anterior del jugador; no forma una cola de efectos.
- La expresión vuelve a calma a los tres segundos. Fijar conserva la cara,
  mientras la burbuja siempre termina. El control de fijado se aplica a la
  siguiente selección; desactivarlo o «Volver a calma» libera la expresión.
- El radial tiene seis posiciones fijas; Emote vive siempre en la de 300° y
  abre los cuatro gestos (con las caras del héroe), volver a calma y fijar
  dentro del mismo anillo. Las demás acciones proceden de las capacidades de
  Rust. Una acción que no corresponde deja su posición vacía; una acción válida
  pero temporalmente indisponible sigue visible y atenuada.
- El avatar conserva las reglas de visibilidad de su placa. La burbuja requiere
  una placa completa visible y audiencia propia/aliada/de equipo. Los avisos
  comparten audiencia sin depender de la cámara; se agrupan por emisor y tienen
  prioridad baja, sin reemplazar alertas importantes.
- Solo viaja el código. Los quince catálogos existentes traducen menú, frase y
  aviso. `SetEmoteTexts` entrega cuatro frases locales y los márgenes seguros
  al renderizador; no es una acción multijugador. Se sincroniza al cargar,
  cambiar idioma, recrear el cliente y volver a una partida. Rust conserva el
  moldeado de árabe y los caracteres asiáticos del sistema de texto existente.

## Imágenes y capacidad

Cada líder tiene su carpeta `assets/art/leaders/<slug>/`: `portrait.*` sirve
perfiles, diálogos y avisos, y `expressions.*` sirve las expresiones del mapa.
El pipeline las publica en `/assets/gameplay/avatars/<slug>.webp` y
`/assets/gameplay/avatars/spritesheets/<slug>.webp`. El `<slug>` es el `id` del héroe en
`assets/geo_entities.json` (`Leader::id`); no se construye el nombre de archivo a partir del
nombre visible del héroe.

- Una hoja por cada héroe del catálogo (`Leader::all`); orden de lectura: Happy, Calm,
  Fear, Angry. Se recorta un margen del 1 % para retirar separadores, después
  un cuadrado centrado por cuadrante y finalmente se ajusta a 256 × 256.
  Las hojas no cuadradas o con dimensiones impares son válidas.
- La textura compartida tiene 8 × 16 espacios de 256 px (32 MiB RGBA). Primero van
  el respaldo y la moneda; después cada héroe reserva cuatro, en el orden del
  catálogo, y al final los retratos estáticos de campaña. Una prueba de
  `sow-render` falla si los héroes no dejan lugar para las campañas.
- Los consumidores utilizan `leader_avatar_slot`, `AVATAR_FALLBACK_SLOT` y
  `avatar_campaign_first_slot()`. Un identificador de campaña que coincide con un
  héroe comparte su bloque; no ocupa otro retrato.
- El héroe seleccionado y los presentes se cargan primero. Se conserva el
  máximo de seis descargas simultáneas y sus reintentos. Si la hoja no existe,
  no se puede leer o agota los reintentos, se solicita su retrato fijo. El
  respaldo general permanece disponible y los errores se registran.
- Decodificar y recortar ocurre al recibir el archivo. Las caras se comparten
  entre jugadores; el dibujo no procesa imágenes en cada cuadro.
- El retrato del mapa y el de muerte comparten una sola máscara circular, sin un
  fondo cuadrado adicional ni un control para cambiar sus esquinas. Humanos y
  ghosts usan ocho veces el tamaño de texto como diámetro; los avatares de bots
  visibles usan seis veces. Cada insignia mide el 43 % del diámetro real del
  avatar y el ajuste a tierra incluye el círculo y todas las insignias.
- Los iconos de espadas pasan por `TextRenderer::push_troop_icon`, que solo
  permite opacidad y contorno y conserva los colores del atlas. El texto de
  tropas en la placa es blanco; los conteos de ataques salientes son rojos y
  los entrantes blancos. Los barcos de guerra y proyectiles conservan el color
  original de sus sprites.
- La comprobación de capacidad incluye los retratos de entidades geográficas
  y de campañas. Añadir contenido que exceda la textura debe fallar ahí, antes
  de publicar; el límite también se comprueba al asignar durante el juego.

La fuente del mapa se genera con `sow-tools/src/bin/generate_font_atlas.rs`.
Su entrada combina el corpus de nombres existente y las cuatro frases de cada
idioma publicado, tomadas de los catálogos compartidos. Incluye las formas
árabes obtenidas de las frases completas. El test
`gameplay_atlas_covers_every_localized_emote_phrase` comprueba cada carácter y
cada forma árabe; cambiar una traducción que necesite caracteres nuevos obliga
a regenerar `assets/generated/msdf-atlas.png` y su JSON antes de publicar.
La cabecera del generador documenta su comando completo.

## Movimiento y placa

`NameplateSystem` observa eventos cada cuadro, antes de su muestreo espacial
cada cuatro turnos. Su reloj combina el inicio aceptado con interpolación entre
estados. Un evento anterior fijado no vuelve a lanzar su burbuja al reconectar.

- 0–70 ms: anticipación. 70–180 ms: impulso. 180–500 ms: aterrizaje amortiguado.
- La expresión cambia en unos 120 ms. Saludar usa la cara en calma.
- Saludar salta el 10 % del diámetro; Ayuda retrocede y tiembla; Amenazar
  aterriza con dos sacudidas; Celebrar salta el 20 % y añade otro rebote, hasta
  650 ms. El retrato se mueve; nombre, tropas y posición no se mueven.
- Burbuja: entra entre 80–240 ms, se mantiene hasta 2700 ms y sale hasta 3000 ms.
  El texto, relleno y punta escalan con el avatar; el texto conserva un mínimo
  legible en pantallas pequeñas y se reduce si una frase larga no cabe. El cuerpo
  se desplaza al cuadrante elegido y la punta ancha queda unida al retrato. La
  posición preferida es arriba a la derecha; si no cabe, prueba arriba a la
  izquierda, abajo a la derecha y abajo a la izquierda. Caja, punta, sombra y
  texto respetan el espacio visible y los márgenes seguros.
- Repetir parte de la posición actual. Liberar realiza una transición breve
  hacia calma. Morir elimina el efecto; reiniciar o salir limpia sus estados.
- Movimiento reducido conserva las transiciones de transparencia y evita
  saltos, temblores y deformaciones.
- Las placas principales reciben primero el presupuesto de dibujo. La segunda
  imagen de una transición y las burbujas solo se dibujan si caben completas.
  Una burbuja no altera el ajuste de la placa a tierra ni sus zonas del tutorial.

La curva `spring_overshoot` es la misma utilizada por el oro. Muerte conserva
sus tres espacios independientes: indicador, avatar y nombre limpio. Dura
1.5 segundos; el oro empieza al segundo y permanece tres segundos. Durante el
medio segundo compartido ambas filas quedan separadas y parten del mismo ancla.
El oro muestra la cantidad con la moneda; solo lo ve quien recibe el pago. Las
muertes locales, de aliados o del equipo se muestran; las demás no se encolan.

## Traición y grabaciones

La traición utiliza `betrayal_retaliation_ticks`, mediante
`can_retaliate_against`. Conserva 100 turnos para bots y 300 para humanos,
separados de los 300 turnos del estado de traidor. Ningún emote ni fijado puede
acortar, extender, activar o borrar esa regla.

`GAMEPLAY_FORMAT_VERSION`, en `sow-core/src/protocol.rs`, identifica el formato
de turnos guardados. `LIVE_PROTOCOL_VERSION` identifica por separado los mensajes
de conexión. `gameplay_build_identity` incluye ambos; el servidor rechaza una
conexión anterior sin cambiar cómo interpreta una grabación histórica.

Las partidas registran `replay_format = GAMEPLAY_FORMAT_VERSION` en sus datos de inicio confiables.
`sow-server/src/replay.rs` solo lee ese formato. La ausencia del campo y las
versiones anteriores se rechazan; el archivo entregado no decide su versión.
El botín de eliminación se duplica antes de repartirlo entre el atacante y
sus asistentes.
Las nuevas partidas usan bases 4x4 sin separación adicional y aceptan solo la
posición confirmada. Respeta el índice de cada acción porque también interviene
en identificadores de flotas. Cubre
grabaciones completas y cierres parciales; los reintentos conservan los bytes
originales. Este lector es una dependencia activa de recompensas pendientes,
no una segunda ruta de comunicación. No eliminarlo mientras haya registros
anteriores que deban verificarse.

## Comprobaciones y aceptación

`./sow p` ejecuta las comprobaciones del motor, lector de grabaciones, menú web,
renderizador, muerte, imágenes y movimiento; después construye y verifica la
publicación. `./sow l` sirve para la revisión visual local y no sustituye esa
aceptación.

- `sow-core/src/emote.rs`: códigos, condiciones, repetición, fijado y traición.
- `sow-server/src/replay.rs`: solo lee el formato de grabación actual; las
  grabaciones de versiones anteriores se rechazan con un mensaje claro.
- `sow-client/tests/nameplate_emotes.rs`: curvas, cuadrantes, los doce héroes,
  respaldo, reutilización y capacidad de todos los retratos referenciados.
- `sow-client/src/render/world/nameplates.rs`: eventos inmediatos, repetición,
  liberación, muerte, reconexión y expiración a distintas velocidades de dibujo.
  Estas pruebas se ejecutan en WASM mediante el control oficial.
- `sow-render/src/nameplate.rs`: tierra, escala y prioridad con presupuesto
  limitado; sus comprobaciones cubren proporción de insignias y ajuste del grupo.
- `sow-client/src/render/world/nameplates.rs`: proporción de la burbuja, frases
  largas, punta conectada, cuatro posiciones de borde y viewport estrecho.
- `sow-web/shell/map_interaction.test.js`: capacidades del radial,
  traducciones, sincronización local y ausencia del selector retirado.

La calidad visual final requiere revisión del usuario en escritorio y móvil:
cuatro movimientos, transición de cara, fijado y liberación, zoom, bordes de
pantalla y movimiento reducido. Una comprobación de código no demuestra esa
calidad visual.
