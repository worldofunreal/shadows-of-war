# Poki testing plan

Estado: el build local y el pipeline oficial web/backend ya están listos para
entregar a Poki. Todavía no se debe declarar "100% aprobado" porque Poki debe
aprobar el contenido, las conexiones externas y los resultados de sus pruebas.

## Decisión de producto

La versión Poki será anónima y gratuita:

- sin Stripe, compras, cuentas externas, OAuth ni chat;
- anuncios únicamente mediante el SDK de Poki;
- datos de juego enviados a Poki mediante `measure()`;
- el sitio propio queda para presentar Android e iOS y enlazar a Poki;
- Android/iOS no forman parte del paquete Poki ni de este testing.

## Estado contra las reglas de Poki

| Área | Estado | Evidencia o siguiente acción |
| --- | --- | --- |
| Paquete separado | Hecho | `sow-dist` genera `dist/poki`; el verificador rechaza archivos de la web, CrazyGames, tienda y service worker. |
| SDK y ciclo de vida | Hecho en código | `init()`, `gameLoadingFinished()`, `gameplayStart()` después de la primera interacción, `gameplayStop()`, `commercialBreak()`, `measure()` y `openExternalLink()`. |
| Anuncios | Hecho en código | El juego no tiene temporizadores ni anuncios propios; las pausas pasan por `commercialBreak()`. |
| Compras y cuentas | Hecho en código | El menú Poki es anónimo y no incluye tienda, Stripe, Google, Discord, X, Meta, OTP ni WOU-ID. |
| Chat y nombres | Hecho en código | Chat desactivado; el servidor limita nombres a 16 caracteres y rechaza lenguaje bloqueado, controles e invisibles Unicode antes de guardar o difundirlo. Política versionada: v1. |
| Recursos locales | Hecho en código | Mapas y recursos usados por el juego se copian al paquete. Los mapas se publican comprimidos y sin los archivos fuente. |
| Eventos | Hecho en código | La medición usa `PokiSDK.measure()` para cola, lobby, carga, tutorial, pausas, finalización y abandono; los anuncios no se duplican con `measure()`. |
| Analytics propio | Hecho en código | Se desactiva para Poki; no se conserva ni envía la cola de analytics de la web propia. |
| Google Analytics | Verificado ausente | No hay scripts, IDs ni llamadas de Google Analytics en el código fuente ni en `dist/poki`; el mensaje observado proviene del contenedor/SDK de Poki. |
| Contenido | Pendiente de moderación | La terminología visible de ataque se presenta como `Strategic Strike`; Poki aún debe revisar violencia, efectos, texto y arte. |
| Idiomas | Hecho en código | El paquete exporta inglés, español, francés, alemán, italiano y turco; el juego detecta esos locales y carga sus traducciones. |
| Tamaño | Verificado localmente | El paquete queda en 14.47 MB sin comprimir, aproximadamente 3.95 MiB de carga inicial y 7.67 MiB total al comprimir los archivos transferibles; confirmar la descarga real en Poki Inspector. La guía recomienda aproximadamente 5 MB iniciales y 8 MB comprimidos totales. |
| Thumbnail | Hecho localmente | `dist/poki-thumbnail.png` se genera como imagen cuadrada full-bleed de 628×628 px, sin texto ni bordes; queda fuera del build del juego para no consumir el presupuesto de descarga. Falta cargarla en Poki y preparar la versión animada antes del lanzamiento global. |
| Runtime local | Verificado localmente | Chromium cargó el documento y el WASM con `SOW_PORTAL=poki`, creó el canvas y no produjo excepciones JavaScript en desktop, móvil y tablet, en orientación vertical y horizontal. La emulación móvil expuso `maxTouchPoints=5` y puntero táctil; assets, fuentes y mapas se sirvieron desde el paquete local. Las conexiones externas observadas fueron el SDK/red de anuncios de Poki y los endpoints propios que requieren aprobación CSP. |
| URLs externas | Pendiente de aprobación | El juego ya fue creado y cargado, pero Poki bloquea API y WebSocket hasta aprobarlos en la pestaña CSP del juego. |
| Pipeline web/backend | Hecho | `./sow p` terminó correctamente con release `0.1.2-1165dc3b2907`, healthcheck y verificación pública. |

## URLs que deben aprobarse

Solicitar solamente las conexiones que el juego necesita:

- SDK de Poki: `https://game-cdn.poki.com/scripts/v2/poki-sdk.js`.
- Orquestador: `wss://shadowsofwar.io/ws/`.
- API anónima: `https://shadowsofwar.io/api/profile/anonymous`, `https://shadowsofwar.io/api/profile/anonymous/name` y `https://shadowsofwar.io/api/profile/anonymous/tutorial-complete`.
- Perfil público, si se mantiene la pantalla de perfil: `https://shadowsofwar.io/api/profiles/*`, `https://shadowsofwar.io/api/profiles/*/matches`, `https://shadowsofwar.io/api/profiles/*/seasons`, `https://shadowsofwar.io/api/profiles/search` y `https://shadowsofwar.io/api/matches/*`.
- Partidas: el servidor puede entregar un `relay_host` y un puerto entre `25592` y `26500`; con la configuración actual el host público es `relay.shadowsofwar.io`, por lo que Poki debe aprobar `wss://relay.shadowsofwar.io:25592-26500/ws/` o la forma exacta que indique su panel.
- Privacidad: `https://shadowsofwar.io/privacy/`, siempre abierta mediante `PokiSDK.openExternalLink()`.

No se debe pedir permiso para analytics propio, Stripe, WOU-ID, redes sociales,
Discord, Telegram, GitHub, mapas externos ni assets externos: el build Poki no
los usa.

## Secuencia para entrar a testing

1. Construir el paquete y verificar `dist/poki`.
2. Ejecutar Poki Inspector en desktop, mobile y tablet; corregir errores de carga, touch, orientación, audio y consola.
3. Crear `poki.json` con `npx @poki/cli init --game <game_id> --build-dir dist/poki` y subir con `npx @poki/cli upload`. La primera subida abre el navegador para autenticar; las credenciales quedan fuera del repositorio.
4. Completar la moderación de contenido y la aprobación de las URLs externas.
5. Entregar thumbnail y ejecutar Playtesting: Poki solicita 10 grabaciones.
6. Ejecutar Player Fit Test: 500 jugadores. La guía usa como señal saludable un promedio de 3 minutos o más y al menos 25% de jugadores por encima de 3 minutos.
7. Pasar Web Fit Test, que normalmente dura alrededor de una semana.
8. Resolver el feedback y solicitar Final Review.

## Estado del juego en Poki

El juego ya fue creado y cargado en Poki. Su `game_id` es
`e279cef3-1ba7-458c-97d2-c23daa3e6de8`.

La pestaña CSP está en la configuración del juego, no en la configuración
general del equipo:

`https://app.poki.dev/world-of-unreal/games/e279cef3-1ba7-458c-97d2-c23daa3e6de8/settings/content-security-policy`

Si aparece bloqueada, primero se debe guardar la URL de privacidad en General:

`https://shadowsofwar.io/privacy/`

Después de que Poki apruebe las conexiones, se debe volver a subir la versión
para que se actualice la política aplicada al juego.

## Comandos del repositorio

```text
./sow l
```

Genera y verifica el paquete local `dist/poki` junto con el resto del build web.
El paso final de web/backend, cuando el paquete esté listo para la integración,
es el pipeline oficial `./sow p`; no se usa `./sow a` para Poki.
El empaquetado Poki usa `magick` para generar las variantes ligeras de retratos
y miniaturas; los archivos originales no se modifican.

Para el CLI de Poki, crear localmente un `poki.json` con esta forma, sin
commitear el token:

```json
{
  "game_id": "ENTREGADO_POR_POKI",
  "build_dir": "dist/poki"
}
```

Referencias oficiales: [Poki CLI](https://raw.githubusercontent.com/poki/poki-cli/main/README.md),
[Requirements](https://developers.poki.com/guide/requirements-quality),
[External resources policy](https://developers.poki.com/guide/external-resources-policy),
[Content & player safety](https://developers.poki.com/guide/content-player-safety),
[SDK events](https://developers.poki.com/guide/game-events),
[Web game engines](https://developers.poki.com/guide/web-engine),
[Game thumbnail](https://developers.poki.com/guide/game-thumbnail) y
[How testing works](https://developers.poki.com/guide/how-testing-works).
