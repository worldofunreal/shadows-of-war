# GDD — Evolución de edificios y ritmo de partida

**Estado:** propuesta para revisión, sin cambios de juego aprobados.  
**Fecha:** 2026-09-28.  
**Alcance:** partidas normales; las campañas con edificios desactivados conservan esa regla.

## 1. Fantasía y regla principal

El jugador funda sitios pequeños y decide cuáles convertir en centros importantes. Una mejora cambia el nombre, la silueta y una ventaja reconocible. Construir un edificio nuevo y mejorar uno existente son decisiones distintas: el jugador selecciona el edificio y ve el precio, el tiempo y el beneficio antes de pulsar **Mejorar**.

Las cinco familias de edificios tienen trabajos distintos: asentamientos sostienen el ejército y desbloquean etapas; granjas generan tropas; fábricas generan oro y aceleran obras; puertos gobiernan los barcos; defensas protegen territorio. No se introduce una moneda de comida o materiales.

Cada nivel conserva los beneficios anteriores. Durante una obra siguen activos los beneficios del último nivel terminado; el nuevo beneficio empieza al completarse. Un edificio solo puede tener una mejora en curso y no se encadenan niveles en una cola.

## 2. Punto de partida comprobado

- Hay cinco tipos de edificios y un nivel por edificio. Construir cerca de otro del mismo tipo crea otro sitio; mejorar requiere seleccionar el edificio y enviar su identificador. [Tipos](../sow-core/src/game.rs), [construcción](../sow-core/src/intent/buildings.rs), [costos](../sow-core/src/building/cost.rs).
- El nivel 0 significa que la primera construcción aún no termina; el primer edificio terminado es nivel 1. Existen módulos de ciudad que duplican funciones del puerto, la fábrica y la defensa. [Edificio y módulos](../sow-core/src/building/core.rs).
- `UpgradeTile` cobra oro y guarda un nivel en el mapa, pero ese nivel no modifica ingresos, combate ni producción en la simulación actual. No equivale a una granja. [Mejora de terreno](../sow-core/src/intent/buildings.rs).
- El mapa ya guarda un nivel por índice de casilla y solo envía las casillas que cambiaron; el costo de comunicar una mejora no depende de recorrer todo el mapa. La memoria sí crece con el tamaño completo del mapa, así que no se añade otra capa densa para granjas. [Mapa](../sow-core/src/map.rs), [snapshot](../sow-core/src/engine/snapshot.rs).
- Para la implementación agrícola, el nivel interno solo necesita 0–3, así que debe caber en `u8`; el protocolo puede seguir enviando el nivel como entero mientras se conserva compatibilidad. No se usa un mapa hash de millones de casillas: la lectura directa por índice es más rápida y los cultivos activos quedan limitados por las ranuras del jugador.
- Los barcos recorren una casilla por turno; un transporte puede zarpar desde una costa propia aunque no salga físicamente del puerto. El puerto solo habilita el lanzamiento. [Movimiento](../sow-core/src/execution/combat_fleets.rs), [ruta](../sow-core/src/warp_fleet.rs).
- Cualquier ciudad terminada puede lanzar una bomba nuclear humana, incluso sin Arsenal. La IA exige Arsenal, pero no lo construye mediante sus decisiones normales y deja de mejorar estructuras al nivel 5. [Lanzamiento](../sow-core/src/intent/nukes.rs), [IA](../sow-core/src/intent/nation/structures.rs).
- El inicio normal tiene 5 000 tropas, 100 de oro, 250 tropas por segundo antes del multiplicador global y turnos de 100 ms. Estos valores iniciales quedan fuera de la reducción de los extras por edificios. [Configuración](../sow-core/src/game_config.rs).

## 3. Niveles y ventajas

Los nombres son ingleses en el juego; las explicaciones aquí están en español. Los niveles de una misma rama se compran en orden. Los porcentajes son objetivos iniciales de balance, no valores cerrados.

### Asentamiento: Camp → Metropolis

| Nivel | Nombre | Beneficio que aparece al completar la mejora |
| --- | --- | --- |
| 1 | Camp | Aumenta el límite de tropas. Es el primer centro poblado. |
| 2 | Hamlet | Aumenta el oro recibido por territorio propio. |
| 3 | Village | Autoriza la construcción de Workshop y sus mejoras. |
| 4 | Town | Autoriza barcos mercantes si existe un Harbor; amplía el límite de tropas. |
| 5 | City | Autoriza buques de guerra si existe un Port; vuelve a ampliar el límite de tropas. |
| 6 | Metropolis | Puede lanzar bombas nucleares desde este asentamiento. |

**Megalopolis** queda fuera del primer diseño: requiere una ventaja final distinta de más ingresos, capacidad o bombas. No se añade un nivel solo por tener otro nombre.

### Puerto: Dock → Megaport

Un barco sale de un puerto concreto. La velocidad que recibe depende del nivel terminado de **ese puerto al zarpar**; mejorar o perder el puerto no modifica barcos que ya están en el mar.

| Nivel | Nombre | Beneficio que aparece al completar la mejora |
| --- | --- | --- |
| 1 | Dock | Permite lanzar transportes desde su costa. |
| 2 | Wharf | Los barcos que zarpan de aquí viajan aproximadamente 15 % más rápido. |
| 3 | Harbor | Permite construir barcos mercantes cuando también se posee un Town. |
| 4 | Port | Permite construir buques de guerra cuando también se posee una City. |
| 5 | Megaport | Los barcos que zarpan de aquí viajan aproximadamente 30 % más rápido que la base y su construcción tarda menos. |

La bonificación de Megaport **sustituye** la velocidad de Wharf: 30 % total, no 15 % más 30 %. El lanzamiento elige su puerto de salida antes de confirmar la ruta; ya no puede usar cualquier costa propia solo por poseer un puerto lejano.

### Fábrica: Workshop → Industrial Complex

| Nivel | Nombre | Beneficio que aparece al completar la mejora |
| --- | --- | --- |
| 1 | Workshop | Produce oro. |
| 2 | Manufactory | Reduce el tiempo de construcción de edificios propios. |
| 3 | Factory | Reduce moderadamente el precio de mejorar otros edificios. |
| 4 | Industrial Complex | Aumenta el oro que generan los barcos mercantes: la industria ya puede exportar. |

La producción de oro de varios talleres se suma. Las reducciones porcentuales usan solamente la mejor fábrica terminada del jugador; varias fábricas no producen descuentos infinitos. Los descuentos no afectan la propia rama de fábricas. El primer Workshop requiere Village.

### Defensa: Watchpost → Citadel

| Nivel | Nombre | Beneficio que aparece al completar la mejora |
| --- | --- | --- |
| 1 | Watchpost | Atacar territorio dentro de su zona cuesta más tropas. |
| 2 | Watchtower | Amplía la zona protegida. |
| 3 | Bastion | Reduce las bajas de las tropas defensoras dentro de esa zona. |
| 4 | Citadel | Puede interceptar un misil nuclear dentro de su alcance. |

La defensa sigue siendo local: varios fuertes no crean un bono para todo el imperio. La intercepción nuclear pasa del módulo **Shield** de la ciudad a Citadel.

### Granja: edificio visible

La granja se funda en una casilla propia de terreno bajo, aparece en la barra de edificios y ocupa un sitio de construcción. Cada granja consume una ranura limitada por los asentamientos del jugador: como primera curva, Camp 1, Hamlet 2, Village 4, Town 8, City 16 y Metropolis 32. Así el tamaño del mapa no crea millones de decisiones ni millones de fuentes de ingreso.

| Nivel | Nombre | Beneficio que aparece al completar la mejora |
| --- | --- | --- |
| 1 | Cultivated Plot | Genera tropas. |
| 2 | Farm | Genera más tropas. |
| 3 | Irrigated Fields | Genera el máximo de tropas de la rama. |

La producción de tropas de varias granjas se suma. Las granjas no generan oro, no aumentan el límite máximo y no cambian el ingreso territorial. Al perder la casilla, dejan de aportar hasta recuperarla. En la primera versión solo se usa terreno bajo; highland y mountain no. `UpgradeTile` deja de ser la ruta de granjas.

## 4. Economía y ritmo

El objetivo es conservar el ritmo base de la apertura y hacer que el crecimiento producido por edificios sea más controlado. No se reduce la frecuencia de los turnos ni la respuesta de los controles.

| Tramo objetivo | Qué debería ocurrir |
| --- | --- |
| Apertura, primeros 2 minutos | Se puede fundar Camp y cultivar una primera casilla pronto; la conquista neutral consume una parte apreciable de las tropas iniciales. Las primeras derrotas son posibles, pero no dominan el arranque. |
| Desarrollo, aproximadamente minutos 2–6 | El jugador elige entre economía, defensa y acceso al mar. Las mejoras de nivel medio requieren conservar territorio y oro. |
| Conflicto avanzado, aproximadamente minutos 6–10 | Town/City, comercio y buques de guerra se vuelven alcanzables mediante inversión sostenida. |
| Final de partida | Metropolis y bombas nucleares aparecen como recompensa excepcional, no como opción de una ciudad recién construida. |

Primera regla de balance: reducir a la mitad únicamente los extras producidos por edificios. Los valores iniciales, la regeneración base, el oro base, el territorio y la velocidad general de la partida permanecen iguales. No se reducen los costos de fundación por esta razón; las mejoras nuevas escalan por nivel.

La tabla de referencia para la primera pasada queda así:

| Valor actual | Objetivo inicial | Uso |
| ---: | ---: | --- |
| 5 000 | 5 000 | Tropas al aparecer |
| 100 | 100 | Oro al aparecer |
| 250/s | 250/s | Regeneración base de tropas |
| 4/s | 4/s | Oro base |
| 1 por 8 tiles | 1 por 8 tiles | Oro territorial |
| 5 000 por City | 2 500 por nivel de asentamiento | Capacidad de tropas |
| 25 por nivel de City | 12.5 por nivel de asentamiento | Tropas de asentamiento |
| 4 por City | 2 por nivel de asentamiento | Oro de asentamiento |
| 8 por Factory | 4 por nivel de fábrica | Oro de fábrica |
| 50 por Port | 25 por nivel de puerto | Tropas de puerto |
| 4 por Port | 2 por nivel de puerto | Oro de puerto |
| 100 por Foundry | 50 por nivel de fábrica | Oro de fundición |
| 80 por Armory | 40 por nivel de asentamiento | Tropas de armamento |

Los valores de la tabla son anclas de balance, no un contrato final. La simulación puede mover los extras de edificios, pero no debe alterar los valores iniciales en esta fase.

La fórmula territorial queda fuera de esta revisión. Si después de reducir los extras de edificios el oro todavía escala demasiado en mapas grandes, se balanceará por separado para no mezclar dos causas distintas.

**Precio de fundación:** conserva la relación actual entre ramas y crece con la cantidad de sitios ya poseídos de la misma rama. **Precio de mejora:** depende del nivel de destino de ese edificio y aumenta claramente en cada escalón, sin el límite actual de 10 veces el precio base. Como primera curva para simular, cada escalón cuesta alrededor de 1.8 veces el anterior. El precio mostrado en la ficha es el que valida la simulación. Las granjas usan una curva pequeña propia y su costo aumenta por nivel, no por cada casilla del mapa.

**Tiempo de obra:** primeras construcciones breves; mejoras de nivel medio toman decenas de segundos y Metropolis alrededor de un minuto. Un descuento de fábrica acorta la obra sin saltarse niveles. El objetivo es que una mejora se note durante la partida sin convertirla en una espera de juego móvil de horas.

## 5. Selección y presentación

1. La barra de edificios selecciona una rama para **fundar** un sitio nuevo. Mantener pulsado puede seguir colocando sitios separados donde haya espacio y oro; acercarse a un edificio existente nunca lo mejora por accidente.
2. Pulsar un edificio propio abre el menú radial y una ficha inferior en JavaScript: nombre actual, nivel, ventaja activa, siguiente nombre y ventaja, precio, tiempo y botón **Mejorar**. Si faltan oro o requisitos, la ficha explica cuál falta. Las órdenes del jugador viajan a la simulación Rust usando el identificador del edificio.
3. Una obra muestra su progreso y el siguiente aspecto. Los edificios enemigos muestran nombre y nivel sin permitir acciones propias.
4. Cada nivel terminado tiene una silueta reconocible al acercar la cámara. Se reutilizan el atlas y el dibujo GPU de edificios; a zoom lejano se mantienen el agrupamiento y la visibilidad actuales. Subir de nivel no aumenta la huella física ni exige revisar cada casilla del mapa en cada turno.

## 6. Límites estratégicos

- La bomba requiere una **Metropolis terminada** del dueño. El lanzamiento sale de ella, cuesta una cantidad relevante de oro —como objetivo inicial, varias veces el precio de mejorar City a Metropolis— y tiene un enfriamiento compartido por jugador para evitar salvas desde varias metrópolis. Se elimina la doble condición de Arsenal; humanos y bots obedecen la misma regla. El costo actual de 5 de oro no sirve para este rol.
- Los buques mercantes requieren Harbor y Town; los de guerra requieren Port y City. El menú enseña ambos requisitos antes de comprar.
- Los bonos globales de impuestos, velocidad de obra, descuentos, exportación y eficiencia agrícola usan el nivel más alto disponible, no la suma de edificios. La producción de oro y tropas sí puede crecer con varios sitios.
- Un edificio perdido deja de aportar sus bonos. Los barcos ya zarpados conservan la velocidad recibida al salir.
- Los módulos **Port**, **Foundry**, **Armory**, **Arsenal** y **Shield** se absorben en las cinco familias de edificios. **Intel**, sin efecto de juego actual, se retira del diseño. Los campos antiguos se conservan temporalmente solo para compatibilidad de mensajes; las partidas nuevas no los escriben.

## 7. Condiciones para aprobar la implementación

- Un jugador puede identificar qué gana antes y después de cada mejora y distinguir un sitio nuevo de una mejora.
- Cada tipo tiene un papel propio; sus ventajas no se duplican mediante módulos de ciudad.
- La apertura permite actuar de inmediato, pero la primera expansión y las primeras eliminaciones son sensiblemente más lentas que hoy.
- Transporte, comercio, guerra naval y bombas aparecen en el orden previsto; los bots pueden alcanzar los mismos escalones sin reglas especiales.
- La velocidad naval procede del puerto de salida, las bombas solo salen de Metropolis y Citadel puede interceptarlas.
- El costo y la duración de cada escalón se afinan con partidas simuladas: tiempo hasta Camp, primera casilla cultivada, Town, primer barco y Metropolis; oro sobrante; cantidad de eliminaciones durante los dos primeros minutos. No se fijan valores definitivos a partir de intuición solamente.
