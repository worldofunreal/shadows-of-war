# Nameplates y emotes

Contrato del sistema. Léelo antes de cambiar comunicación, retratos, placas,
recortes de héroes o compatibilidad de grabaciones. El código de placas,
retratos y emotes enlaza esta guía.

## Recorrido y responsables

```text
Clic derecho / mantener pulsado sobre territorio propio
  -> capacidades del menú calculadas por Rust
  -> anillo web: cuatro gestos, volver a calma y fijar
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
| Anillo, teclado y fijado | `sow-web/shell/main_menu.hud.ts`, `main_menu.hud.css` |
| Cargador de imágenes de retratos (único) | `sow-web/shell/avatar-presentation.ts`: `loadAvatarImage`, `loadAvatarBitmap` |
| Celda de cada retrato durante la partida | `sow-client/src/render/avatars.rs`: `AvatarSlots` |
| Textura de retratos | `sow-render/src/text/texture.rs`: `AvatarAtlasTexture`; límites en `text/types.rs` |
| Posición sobre tierra y prioridad de placas | `sow-render/src/nameplate.rs` |
| Vida del efecto y preparación del avatar | `sow-client/src/render/world/nameplates.rs`: `NameplateSystem`, `avatar_plan`, `paint_prepared_avatar` |
| Curvas de movimiento compartidas con el oro | `sow-client/src/render/motion.rs`, `render/world/feedback.rs` |
| Avisos | `sow-client/src/sim/snapshot_fx.rs`, `ui/hud/state.rs` |
| Recortes publicados de las caras | `sow-dist/src/main.rs`, con las rutas de `sow_core::emote::hero_portrait_path` |
| Lectura de grabaciones | `sow-server/src/replay.rs`; registro del formato en `register_match_start` |

No añadir otro selector, renderizador de placas, textura por jugador, catálogo
de mensajes ni ruta de red para comunicar estas cuatro acciones.

## Contrato de producto

| Código | Expresión | Posición en el anillo | Mensaje español |
| --- | --- | --- | --- |
| `greet` | `Calm` | 0 (arriba) | ¡Saludos! |
| `celebrate` | `Happy` | 1 (60°) | ¡Victoria! |
| `threat` | `Angry` | 2 (120°) | ¡Prepárate! |
| `help` | `Fear` | 3 (abajo) | ¡Ayuda! |

- El anillo tiene seis posiciones, cada 60° desde arriba en sentido horario.
  Emote ocupa la posición 5 del anillo principal. Dentro de Emote, «Volver a
  calma» aparece en la 4 mientras hay un gesto activo y fijar está en la 5.
  Las caras del anillo son los mismos archivos de expresión que usa el mapa.
- El motor permite emitir a humanos vivos, aparecidos y jugando; los humanos
  controlados automáticamente no emiten.
- Cada emisión aceptada recibe un número nuevo, aunque repita el código, y
  reemplaza el gesto anterior del jugador; no hay cola.
- Sin fijar, cara y burbuja terminan a los tres segundos. Fijado conserva la
  cara y la burbuja hasta que el jugador lo libera o vuelve a calma.
- Las líneas de campaña las ve cualquiera; los gestos de jugadores, el propio
  jugador, sus aliados y su equipo. Los avisos usan la misma audiencia.
- Solo viaja el código. Los catálogos de idioma traducen menú, frase y aviso;
  `SetEmoteTexts` entrega las cuatro frases locales al renderizador.

## Retratos

Reglas. Romper una vuelve a dejar placas con el retrato por defecto mientras el
radial sí muestra el correcto:

1. **Un solo cargador.** El navegador decodifica cada archivo con
   `loadAvatarImage`, el mismo que usa el radial. Para el mapa,
   `loadAvatarBitmap` recorta el cuadrado central y lo escala a 128 px con
   `createImageBitmap`, fuera del hilo principal. Rust lo copia directo a la
   textura (`copy_image_bitmap_to_texture` de blade). No hay decodificador en
   Rust, copia en CPU, ni precarga desde el menú.
2. **Cada retrato tiene su celda.** Cuando un jugador aparece en el snapshot,
   `AvatarSlots` le reserva celdas, primero al jugador local: cuatro caras
   para un héroe (Calm primero) y una para un bot o retrato de campaña. Un bot
   cuyo avatar coincide con el id de un héroe usa las cuatro caras del héroe.
   La reserva empieza en el loader de partida, con su primer snapshot, y el
   loader no se libera hasta que la primera cara de cada jugador (Calm o su
   única cara) cargó o falló una vez (`first_faces_settled`). Esas caras se
   piden antes que las demás. Una falla sigue reintentando ya en partida.
   Así la fase de despliegue empieza con los retratos puestos; en línea, el
   tiempo de esta espera sale de los 15 s de despliegue, porque el relay
   cuenta desde que registra la partida.
3. **Las celdas no se liberan durante la partida.** Quien muere o sale conserva
   las suyas, para el retrato de muerte y los ataques. Al salir de `Playing`
   (o del loader de partida sin entrar),
   `end_match` libera todo, la textura vuelve a una fila y las imágenes que
   lleguen después se descartan.
4. **El dibujo solo lee.** `AvatarSlots::uv` devuelve la celda si su imagen ya
   llegó. Un retrato con slot reservado permanece oculto hasta que cargue; un
   héroe usa Calm si esa cara ya está lista. Solo las identidades de respaldo o
   sin slot usan `null.webp`. Dibujar u omitir no pide, encola ni libera imágenes.
5. **Una falla se reintenta.** El registro anota la ruta y se reintenta a los
   2 s, duplicando hasta 60 s. El retrato reservado sigue oculto durante los
   reintentos. Hay como máximo seis cargas a la vez.

| Imagen | Ruta publicada (en CrazyGames, celda de una hoja de `gameplay/atlas/`) |
| --- | --- |
| Héroe, Calm (también retrato estático) | `gameplay/avatars/<art_slug>.webp` |
| Héroe, Happy / Fear / Angry | `gameplay/avatars/expressions/<art_slug>_<happy\|fear\|angry>.webp` |
| Bot o retrato de campaña | `gameplay/avatars/<slug>.webp` |
| Respaldo (celda 0) | `gameplay/avatars/null.webp` |
| Oro (celda 1) | `gameplay/currency/gold.webp` |

- La lámina fuente de cada héroe es `assets/art/avatars/heroes/<art_slug>.webp`,
  2 × 2 en orden Happy, Calm, Fear, Angry. El publicador recorta las cuatro
  caras y falla si falta la lámina; la lámina no se publica.
- La textura usa celdas de 128 px en 32 columnas (4096 px de ancho) y crece por
  filas hasta 32 (1,024 celdas, 4096 × 4096). Cada fila ocupa 2 MiB. Ejemplo:
  8 héroes y 120 bots usan 154 celdas, 5 filas, 10 MiB. Una prueba falla si
  todo el catálogo no cabe.
- Si el renderizador se recrea con una textura vacía, todos los retratos se
  vuelven a cargar.
- El retrato del mapa, el de muerte y el de los ataques comparten la misma
  máscara circular. Humanos usan ocho veces el tamaño de texto como diámetro y
  los bots seis; cada insignia mide el 43 % del diámetro del avatar.
- Muerte muestra Fear en un héroe y Calm en los demás; los ataques muestran
  Calm del atacante.

## Movimiento y placa

- 0–70 ms: anticipación. 70–180 ms: impulso. Hasta 500 ms: aterrizaje.
- Saludar salta el 10 % del diámetro; Celebrar salta el 20 % y añade un
  segundo rebote hasta 650 ms. El retrato se mueve; nombre y tropas no.
- Al quedar listo por primera vez, un retrato propio entra con el movimiento de
  Celebrar. Si coincide con un emote, ambos movimientos se componen y la burbuja
  sigue anclada al retrato.
- La burbuja aparece entre 80 y 240 ms. Sin fijar, se desvanece durante los
  últimos 300 ms de los tres segundos.
- Movimiento reducido conserva la transparencia y quita saltos y deformaciones.
- Las placas reciben primero el presupuesto de dibujo; la segunda imagen de una
  transición y las burbujas solo se dibujan si caben.

La muerte dura 1.5 s. El oro empieza a los 0.5 s, dura 3 s y solo lo ve quien
recibe el pago. Una eliminación se muestra si la víctima o quien elimina es el
jugador, un aliado o un compañero de equipo, o si el jugador cobra oro por ella.

## Traición y grabaciones

La traición usa `betrayal_retaliation_ticks` mediante `can_retaliate_against`:
100 turnos para bots y 300 para humanos, aparte de los 300 turnos del estado de
traidor. Ningún emote ni fijado acorta, extiende, activa o borra esa regla.

`GAMEPLAY_FORMAT_VERSION` (`sow-core/src/protocol.rs`) identifica el formato de
turnos guardados y `LIVE_PROTOCOL_VERSION` los mensajes de conexión;
`gameplay_build_identity` incluye ambos. Cada partida registra
`replay_format = GAMEPLAY_FORMAT_VERSION` al empezar. `sow-server/src/replay.rs`
solo lee ese formato y rechaza versiones anteriores o sin el campo.

## Comprobaciones

- `sow-core/src/emote.rs`: códigos, condiciones, repetición, fijado, traición y
  nombres de archivo de las caras.
- `cargo test -p sow-client --test nameplate_emotes`: curvas de movimiento y
  reglas de `render/avatars.rs` (cada retrato conserva su celda toda la
  partida, respaldo, bot con cara de héroe, catálogo completo, fin de partida y
  reintentos).
- `sow-client/src/render/world/nameplates.rs`: eventos, repetición, liberación,
  muerte, reconexión y burbujas. Corre en WASM:
  `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="$(command -v wasm-bindgen-test-runner)" cargo test -p sow-client --target wasm32-unknown-unknown --lib nameplates`.
- `sow-render/src/text/types.rs`: el catálogo completo cabe en la textura.
- `sow-render/src/nameplate.rs`: tierra, escala, prioridad y proporción de
  insignias.
- `sow-render/src/text/renderer.rs`: `gameplay_atlas_covers_every_localized_emote_phrase`;
  una frase con caracteres nuevos obliga a regenerar `assets/generated/msdf-atlas.webp`
  con `sow-tools/src/bin/generate_font_atlas.rs`.
- `sow-web/shell/map_interaction.test.ts` y `avatar_presentation.test.ts`
  (`bun test sow-web/shell`).

La calidad visual la acepta el dueño del juego en escritorio y móvil. Una
prueba de código no la demuestra.
