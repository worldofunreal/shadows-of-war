# Recorridos del jugador: regreso, recompensas y guía

## Estado y alcance

Contrato de conexión de los sistemas, revisado el 12 de octubre de 2026.
La evidencia de aceptación se registra al final; la comprobación de las
respuestas y transiciones se distingue de la revisión visual del jugador.

Objetivo: ampliar la guía sin alterar cómo se conceden y muestran las
recompensas. Primero se completa el regreso; después se enseña el menú.

## Orden obligatorio

```
Salir de partida o terminar tutorial
  -> loader: confirmar cuenta, partida y resultado con el servidor
  -> menú: mostrar las recompensas confirmadas
  -> celebrar cada nivel ganado
  -> atender un destino elegido expresamente por el jugador, si existe
  -> iniciar o continuar el recorrido pendiente
```

La manita nunca ocupa el turno de las recompensas. Estar en el menú, esperar
unos milisegundos o no encontrar recompensas en una lista todavía incompleta
no autoriza la guía. Cuando no corresponden premios, el regreso debe indicarlo
expresamente: no se deduce de una lista vacía.

## Responsables: dónde buscar y dónde cambiar

Las rutas siguientes son relativas a la raíz del proyecto.

| Sistema | Dueño que se reutiliza | Responsabilidad |
| --- | --- | --- |
| Resultado y concesión | `sow-relay/src/main.rs`, `sow-server/src/main.rs`, `sow-data/src/db.rs` y `sow-data/src/main.rs` | Registrar participación, comprobar resultado y guardar premios una sola vez. |
| Datos compartidos | `sow-data/src/profile.rs` | Definir comprobantes de premios y progreso de recorridos. |
| Regreso y espera | `sow-client/src/app/account.rs`, `sow-client/src/net/session.rs`, `sow-client/src/loader/engine.rs` | Conservar el regreso pendiente, esperar la respuesta correcta y recuperarse de errores. |
| Conexión con el menú | `sow-client/src/web_menu.rs`, `sow-client/src/asset.rs` | Publicar datos confirmados y resultados de acciones; recibir comandos. |
| Orden del menú | `sow-web/shell/main_menu.flow.js` | Único responsable de autorizar recompensas, destino y guía, en ese orden. |
| Presentación de premios | `sow-web/shell/main_menu.shell.js` | Reutilizar la animación existente y celebrar todos los niveles ganados. No calcular premios. |
| Motor de pasos | `sow-web/shell/sow-campaign.js` | Reutilizar condiciones, transiciones y validación del motor existente. |
| Conexión de la guía al menú | `sow-web/shell/main_menu.tutorial.js` | Entregar hechos del menú al motor y guardar su avance; obedecer al responsable del orden. |
| Manita y presentación | `sow-web/shell/sow-campaign-view.js` | Reutilizar el aspecto, posiciones y animaciones compartidas. |
| Contenido de los recorridos | `assets/guides/menu.triggers.json` | Definir pasos, textos, destinos y condiciones. Separado de la historia de Boudica. |
| Edición y vista previa | `sow-tools/editors/campaign-editor/` | Usar el mismo motor, presentación y definición que el juego. |
| Empaquetado y aceptación | `sow-dist/src/main.rs`, `sow-dist/src/prod.rs` | Incluir los archivos y comprobar la conexión completa antes de publicar. |

Los archivos de conexión no son motores nuevos: adaptan los sistemas existentes.
No se permite otra ruta para iniciar la guía o saltarse el orden.

### Datos y llamadas que conectan los responsables

- El cliente publica `menu_return` con `account_id`, `cycle_id`, `source`,
  `match_id`, `receipt_ids`, `status` y `requested_screen`. Los estados son
  `waiting`, `confirmed`, `not_applicable` y `error`.
- Publica `reward_receipts` con identidad del recibo y partida, estado de
  concesión, estado de verificación y `level_before`/`level_after`.
- `SOWMenuFlow.update` decide la fase. Solo `canPresent()` autoriza al presentador
  y solo `canGuide()` autoriza `SOW_menu_guide.update`; `pause()` oculta la manita.
- El menú publica resultados de nombre, selección, apertura de compra, compra
  confirmada, cancelación y cierre de su presentación. Publica también pantalla,
  héroes poseídos, oferta seleccionada y partidas cargadas en el historial.
- `MENU_GUIDE_OUTCOMES` y `UI_TARGETS`, en el motor compartido, registran una sola
  vez los resultados y controles permitidos. `validateMenuGuides` valida la
  misma definición para juego, editor y aceptación.
- `save_menu_guide_progress` cruza el puente existente hacia
  `/profile/menu-guide-progress`, con la autenticación del perfil. Su respuesta
  actualiza solo el avance: no reemplaza saldos con una foto anterior del perfil.
- `source_match_id` viene del servidor. La petición de guardado de la guía no
  puede asignarlo ni cambiarlo.

## Contrato del regreso

El contexto `menu_return` identifica cuenta, ciclo de regreso, origen
(`tutorial`, `multiplayer` u otro), partida, comprobantes esperados y destino
solicitado. Sus estados deben distinguir espera, confirmado, sin premios
aplicables y error.

- Una respuesta de otra cuenta, partida o regreso no libera el loader.
- La numeración de partidas se recupera de los registros duraderos antes de
  abrir salas. `PlayerDb::next_match_id_seed` y `/internal/match-id-seed` impiden
  reiniciar en 1 y chocar con comprobantes o historiales anteriores. La
  asignación en memoria pertenece al único servidor que organiza partidas;
  añadir otro requiere trasladar también la asignación a la base de datos.
- Tutorial terminado antes de recibir la cuenta: conservar el pendiente y
  solicitar su premio cuando exista la identidad necesaria.
- La primera salida de Boudica, incluido «Salir del tutorial» antes del último
  paso, usa la misma concesión única y bienvenida. `begin_exit_to_main_menu`
  llama a `complete_boudica_tutorial` antes de desmontar la partida; terminar
  el último paso converge allí. Un indicador antiguo de tutorial terminado
  no sustituye el comprobante del premio ni impide recuperarlo.
- Error o demora: mantener abierto el loader y reintentar ahí. No volver al menú
  con una estimación local.
- Recargar conserva el regreso pendiente. Cambiar de cuenta separa todos sus
  pendientes, comprobantes y recorridos.
- Salir voluntariamente de multijugador confirma lo ganado hasta la salida
  registrada. Una desconexión temporal no equivale a abandonar. El cierre
  posterior de la partida no vuelve a pagar lo ya cerrado.
- Cada comprobante nuevo conserva el nivel anterior y posterior, para celebrar
  cada nivel cruzado sin reconstruirlo con una suposición del cliente.
- Guardar un premio y confirmar que se mostró son operaciones distintas.
  Repetir la confirmación de presentación nunca concede dinero o experiencia.

## Contrato de la guía

Cada recorrido tiene un identificador estable y pasos identificados por nombre.
El servidor conserva por cuenta `menu_guides`: pasos completados, paso pendiente (`current_step`), estado
`active`, `completed` o `skipped`, y la selección necesaria para continuar.
En `first_multiplayer`, `source_match_id` identifica la primera participación;
solo el servidor lo asigna. Recordar la elegibilidad no permite mostrar la guía:
su comprobante debe estar aplicado y su verificación debe haber terminado.

La guía solo propone una acción y observa su resultado. No concede premios,
no modifica saldos y no confirma compras o nombres por su cuenta.

| Paso | Evidencia para avanzar |
| --- | --- |
| Guardar nombre | Nombre aceptado por el servidor para la cuenta actual. |
| Cambiar héroe | Selección permitida y aplicada por el sistema de héroes. |
| Abrir oferta | Ventana de confirmación de esa oferta realmente abierta. |
| Comprar héroe | Compra confirmada y propiedad actualizada por el servidor. |
| Equipar compra | Selección aplicada del héroe comprado, no de cualquier héroe. |
| Mostrar perfil | Perfil propio cargado y visible. |
| Mostrar historial | Historial cargado que contiene la partida que acaba de terminar. |
| Mostrar tienda | Catálogo cargado correctamente y tienda visible. |
| Iniciar partida | Solicitud aceptada por el sistema de partidas. |

Un clic, una ventana cerrada o una petición enviada no sustituyen esas
confirmaciones. Una respuesta rechazada mantiene el paso y su explicación.

Reglas para continuar:

- Interrupción o recarga: retomar `current_step`, conservando la decisión tomada; no reconstruirla con el saldo nuevo.
- `menuGuideControl`, en el motor compartido, recupera el acceso al control pendiente mediante la navegación existente. Si Héroes o Perfil están cerrados, la manita enseña primero a volver allí. Juego y Studio usan el mismo responsable.
- Nuevos premios: pausar la manita, presentar premios y niveles, y retomar.
- Omitir voluntariamente: guardar `skipped`; no confundirlo con interrupción.
- Progreso guardado: los pasos completados no retroceden por una respuesta vieja.
- Destino explícito del jugador: atenderlo después de los premios; conservar la
  guía pendiente hasta que vuelva al lugar apropiado.
- Control ausente o pantalla equivocada: no apuntar al vacío ni completar el
  paso. La definición debe incluir la navegación necesaria.
- Plataforma sin tienda: tomar una ruta definida sin compra ni tienda.
- Selección de oferta: usar catálogo, propiedad y saldo actuales; no fijar un
  héroe o precio que pueda dejar de ser válido.
- Cuentas existentes: no reiniciar recorridos ya terminados ni inventar que su
  siguiente partida es la primera.

## Recorrido de bienvenida (`welcome`)

1. Terminar o salir del tutorial y confirmar sus 100 XP y 100 coronas en el loader.
2. Mostrar recompensas; una cuenta nueva debe alcanzar nivel 2 y celebrarlo.
   Los siguientes aumentos de nivel usan la misma presentación.
3. Enseñar a guardar el nombre; esperar aceptación.
4. Abrir Héroes, elegir y confirmar un héroe disponible.
5. Volver a Héroes si la confirmación cerró esa pantalla. Elegir un héroe no
   poseído que pueda comprarse con las gemas gratuitas confirmadas.
6. Abrir la oferta, enseñar la confirmación y esperar la compra aceptada.
7. Dejar terminar la presentación de compra y señalar su botón Continuar;
   después equipar específicamente el héroe adquirido.
8. Volver a Inicio si es necesario; enseñar Partida rápida y terminar cuando
   la solicitud sea aceptada.

Las 475 gemas iniciales y el premio del tutorial tienen concesiones separadas:
la guía usa saldos confirmados, sin sumarlos dos veces. Si no existe una oferta
asequible o la plataforma no permite comprar, el recorrido continúa por la
ruta sin compra. No se fuerza gastar dinero real.

## Recorrido tras primera partida multijugador (`first_multiplayer`)

1. Registrar la primera participación real desde el servidor. Tutorial,
   espectador, sala sin jugar y reconexión no cuentan como primera partida.
2. Al salir, confirmar resultado en el loader; mostrar premios y todos los
   niveles ganados antes de activar la manita.
3. Abrir el perfil propio y mostrar su resumen cargado.
4. Abrir el historial y mostrar esa partida concreta.
5. Abrir la tienda y mostrar su catálogo; no exigir una compra.
6. Volver a Inicio: Partida rápida no está en la pantalla de tienda.
7. Enseñar otra partida y terminar cuando la solicitud sea aceptada.

Si la bienvenida sigue pendiente, no ejecutar dos manitas a la vez. Conservar
ambos avances y terminar u omitir la bienvenida antes del siguiente recorrido.
El recorrido de primera partida se inicia una sola vez por cuenta.

## Orden de implementación

### 1. Asegurar el regreso y los premios

Completar resultado personal confirmado, identidad tardía, recuperación de
pendientes, estados del loader y comprobantes de nivel. Conectar el responsable
del orden a todas las salidas: tutorial, menú de partida, final de partida y
botón de tienda. Retirar los disparadores anteriores cuando estén sustituidos.

### 2. Completar la conexión de recorridos

Conectar hechos confirmados, guardado por cuenta y reanudación. Validar que los
identificadores enviados existen en la definición publicada. Ninguna definición
de guía puede autorizar por sí misma el paso del loader o la presentación.

### 3. Completar ambos contenidos y el editor

Cerrar los huecos de navegación de bienvenida y primera partida. Cargar,
editar, guardar y previsualizar ambos en Campaign Studio usando los mismos
pasos y resultados que el juego. Rechazar pasos sin salida, destinos
desconocidos y condiciones que ningún sistema publica.

### 4. Comprobar el recorrido completo

Usar comprobaciones que ejecuten las transiciones, no solo busquen palabras
en archivos. Cubrir cuenta tardía, resultado demorado/rechazado, respuesta de
otra partida, recarga, varios niveles, nombre rechazado, compra rechazada,
destino Tienda, salida voluntaria, reconexión y primera partida una sola vez.
Comprobar además que una guía nueva no puede adelantarse a los premios.

La aceptación de cambios del juego pasa por `./sow p`. Registrar por cuenta,
partida y regreso qué se confirmó, presentó y continuó, sin secretos. Separar
la evidencia de esas comprobaciones de la revisión visual del jugador.

## Cómo ampliar después

1. Leer este contrato y localizar al responsable de la acción solicitada.
2. Reutilizar su resultado confirmado. Si falta ese resultado, publicarlo
   desde ese sistema, no deducirlo desde la manita.
3. Añadir el paso y su navegación a la definición existente; conservar los
   identificadores publicados y el progreso de las cuentas.
4. Validar y previsualizar con el motor compartido.
5. Comprobar que premios y niveles siguen ocurriendo antes de la guía.

La documentación ayuda a encontrar el sistema; el responsable único del orden,
la validación y las comprobaciones impiden que una ampliación lo omita sin
detectarlo. Ningún comentario por sí solo ofrece esa protección.

## Evidencia de aceptación — 12 de octubre de 2026

- `./sow p` terminó con comprobación de servicios y verificación pública:
  `dist/logs/production-20261012-200759.log`.
- Pasaron las 190 comprobaciones del menú: espera por cuenta y partida,
  recompensas antes de cada nivel y guía, nombre rechazado, compra fallida,
  cancelación, presentación de compra, héroe exacto, reanudación, destino
  Tienda y recorridos de bienvenida y primera partida.
- Pasaron las comprobaciones del cliente, servidor y relay, incluida la
  recuperación de numeración después de reiniciar y el cierre personal sin
  sobrescribirlo al finalizar toda la partida.
- Prueba real con cuentas de auditoría y salas privadas: tutorial con 100 XP,
  100 coronas, nivel 2 y 475 gemas separadas. Repetir su confirmación no pagó otra vez.
- Partidas `15362` y `15364`: XP 100 → 120 → 140; dos participaciones,
  comprobantes verificados e historial disponible. El origen del segundo
  recorrido siguió siendo `15362`.
- Una desconexión y reconexión no cerró el premio. El otro jugador recibió
  turnos posteriores a la salida del primero. La finalización de ambas
  partidas conservó el saldo ya pagado.
- El servidor rechazó un nombre vacío y aceptó su guardado válido. Rechazó
  un paso desconocido; un guardado atrasado conservó el paso pendiente y
  no reactivó un recorrido omitido.
- Campaign Studio: carga de ambos recorridos y guardado válido; rechazo de
  control inexistente y versión desactualizada. Su vista previa y el juego
  comparten motor, presentación y recuperación de controles.

Estas comprobaciones ejecutaron respuestas y transiciones reales. La revisión
visual de la manita, su colocación y las animaciones en el navegador corresponde
al jugador; no se presenta como realizada.

### Corrección de la salida del tutorial — 12 de octubre, revisión posterior

La evidencia anterior no comprobaba que «Salir del tutorial» solicitara el
premio: las pruebas del menú empezaban con el comprobante y bienvenida ya
preparados. Esa salida se clasificaba como sin premios si no se había completado
el último paso de Boudica. Se corrigió en el responsable común del regreso,
sin añadir otro presentador ni otra guía.

- `hud_leave_tutorial_requests_reward_before_releasing_return` ejecuta el
  controlador real del botón del HUD, el comando del puente, `SowApp`, la
  solicitud HTTP y la publicación de estado. Parte de 0 XP y sin bienvenida;
  retrasa la respuesta y comprueba que el regreso no quede confirmado antes.
- La misma prueba se ejecutó con una cuenta de auditoría contra el servidor
  público, reemplazando solamente la respuesta simulada por HTTP real. El
  cliente solicitó el premio una vez, recibió 100 XP, 100 coronas y nivel 2,
  y publicó la bienvenida activa. Repetir la petición conservó los saldos.
- El estado publicado por ese cliente se entregó al presentador y motor de
  guía existentes: ejecutaron XP → coronas → nivel 2 → `welcome_name`. La guía
  permaneció pausada durante los premios. Evidencia local sin credenciales:
  `/tmp/sow-tutorial-live.json`.
- Esta comprobación ejecuta el recorrido sin abrir un navegador. La
  presentación visual y posición de la manita siguen siendo revisión del jugador.
- Aceptación final: `./sow p` pasó con las ocho comprobaciones del regreso,
  salud de servicios y verificación pública en
  `dist/logs/production-20261012-224817.log`. La repetición contra el servidor
  público con la cuenta de auditoría `ca438eae…` conservó el mismo resultado.
