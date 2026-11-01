# Experimentos de ritmo de conquista

Actualizado: 2026-11-01

## Objetivo

Buscar una curva que haga la conquista más lenta al inicio, conserve el ritmo de media partida y la acelere en late game. El criterio inicial es por tropas efectivas del ataque; todavía no se ha cambiado el ritmo del juego en producción.

Este documento es el registro de hipótesis, método, resultados y siguientes pasos. Cada corrida nueva debe añadir sus condiciones y resultados aquí para poder retomar el trabajo sin depender del contexto de una sesión.

## Estado actual del código

El dueño del ritmo de conquista es `SowEngine::execute_combat` en [`combat_tiles.rs`](../../sow-core/src/execution/combat_tiles.rs). Por ataque y tick:

1. Calcula `effective_troops` con los modificadores de ataque y defensa.
2. Para neutral, usa un presupuesto base de `2 × tamaño_de_frontera`. Para PvP, multiplica la frontera por un factor basado en la relación entre tropas atacantes y defensoras.
3. Escala por duración del tick y `global_speed_multiplier` (por defecto, 0.5).
4. Aplica el tope de tiles por ataque. Este tope ya depende de tropas: usa 256 tiles en el ancla de 25.000 tropas, se duplica por cada aumento de 10× y está limitado por un techo de 4.096. Por debajo de 25.000, el tope queda anclado en 256; por tanto, ese tope no distingue el early del mid.
5. En neutral, cada tile cuesta `clamp(2000 × velocidad_del_terreno / effective_troops, 5, 100)` unidades de presupuesto. La pérdida actual por tile neutral es 1 tropa.

El contador de frontera de producción es hoy `to_conquer.len()`. Esa es la longitud de la cola, que puede contener coordenadas repetidas o tiles que ya no son candidatos válidos. La prueba experimental permite comparar ese contador con la cantidad de coordenadas únicas que todavía pertenecen al defensor, son terreno válido y siguen junto al atacante. La ruta pública sigue usando cola y multiplicador 1.0: el experimento no altera el ritmo real.

## Referencia: OpenFront actual

Referencia revisada el 2026-11-01: `openfrontio/OpenFrontIO`, `main` en `030274df6e377a57e784a9f6f9865720bdfbbba` (2026-10-30).

- [`AttackImpl.ts`](https://github.com/openfrontio/OpenFrontIO/blob/030274df6e377a57e784a9f6f9865720bdfbbba/packages/engine/src/game/AttackImpl.ts) mantiene la frontera en un `Set`; `addBorderTile` aumenta el tamaño solo cuando la coordenada todavía no existe.
- [`AttackExecution.ts`](https://github.com/openfrontio/OpenFrontIO/blob/030274df6e377a57e784a9f6f9865720bdfbbba/packages/engine/src/execution/AttackExecution.ts) toma ese tamaño de frontera por tick y le agrega una variación aleatoria de 0 a 4. El presupuesto neutral es dos veces la frontera; el costo de un tile depende de tropas y terreno.
- [`EngineConfig.ts`](https://github.com/openfrontio/OpenFrontIO/blob/030274df6e377a57e784a9f6f9865720bdfbbba/packages/engine/src/configuration/EngineConfig.ts) usa en PvP una curva por relación de tropas y bonos logísticos por tamaño territorial. El bono de atacante llega a su punto medio alrededor de 300.000 tiles y hace que ataques de territorios grandes sean más rápidos. Es una referencia para late game, no una fórmula que debamos copiar sin medir.
- En terreno llano, la pérdida neutral de OpenFront es 16 tropas por tile humano y 8 por tile bot; SOW usa 1 por tile neutral por defecto. Así que la duración de una ofensiva neutral también difiere, aunque no es lo mismo que su presupuesto de capturas por tick.
- El envío humano predeterminado es una quinta parte de las tropas disponibles (`troops / 5`); bots envían una vigésima parte. Es otra palanca posible, independiente del ritmo por tile.

La diferencia de frontera es la comparación más directa para este problema: OpenFront cuenta coordenadas únicas; SOW actualmente cuenta elementos de la cola.

## Hipótesis que validaremos

1. **La cola sobredimensiona la frontera activa.** Si el promedio de entradas de cola supera ampliamente los tiles únicos válidos, el presupuesto actual puede ser mayor de lo que representa el frente real.
2. **Corregir la frontera afecta más a frentes angostos.** Con una frontera amplia, aun los tiles únicos podrían bastar para alcanzar el tope; contar únicos podría entonces cambiar poco la velocidad.
3. **Una curva por tropas efectivas puede producir la progresión deseada.** Hipótesis provisional: 0.70× a 2.500 tropas, 1.00× a 25.000 y 1.15× a 100.000, con interpolación lineal en `log10(tropas)` y extremos limitados. Se aplicaría después del tope actual.
4. **Tropas por sí solas quizá no representen la fase de partida.** Un jugador puede reunir 100.000 tropas temprano o conservar pocas en late game. Si la curva anterior no separa esas situaciones, comparar una curva por territorio relativo al mapa.
5. **Neutral y PvP pueden requerir escalas distintas.** El resultado neutral no tiene pérdidas defensivas por relación de tropas; en PvP, el ataque puede agotarse antes de que la velocidad de presupuesto sea el factor limitante.

Los multiplicadores propuestos son parámetros de experimento, no valores aprobados de balance.

## Experimento 1: cola frente a frontera única

**Estado: completado.** Prueba determinista con 10 semillas, 300 ticks por semilla (30 segundos nominales por semilla), dos tipos de objetivo (neutral y jugador), tres tamaños de ataque y tres fronteras iniciales. Se midieron capturas por segundo. La variante «única» calcula tiles válidos y únicos antes de determinar el presupuesto; ambas variantes conservan el resto de la resolución actual.

**Registro:** base SOW `e8fea7ed382c24b89eec0196bdc5c60b29f8da44` más harness experimental local; `GameConfig::default()` (100 ms por tick, velocidad global 0,5), mapa 256 × 256 llano, sin habilidades ni edificios; semillas 1–10.

`cola → única` muestra las capturas por segundo. El porcentaje es el cambio de la variante única respecto a la cola. «Tope» muestra ticks que alcanzaron el tope de presupuesto sobre los ticks activos de cada variante.

| Objetivo | Tropas iniciales | Frontera inicial | Capturas/s: cola → única | Cambio | Tope: cola; única |
| --- | ---: | ---: | ---: | ---: | --- |
| Neutral | 2.500 | 6 | 55,84 → 43,52 | −22,1% | 2.757/3.000; 332/3.000 |
| Neutral | 2.500 | 24 | 56,64 → 47,69 | −15,8% | 2.851/3.000; 687/3.000 |
| Neutral | 2.500 | 96 | 57,31 → 56,82 | −0,9% | 2.977/3.000; 2.480/3.000 |
| Neutral | 25.000 | 6 | 250,93 → 220,51 | −12,1% | 2.912/3.000; 2.100/3.000 |
| Neutral | 25.000 | 24 | 253,40 → 229,47 | −9,4% | 2.940/3.000; 2.206/3.000 |
| Neutral | 25.000 | 96 | 255,89 → 253,81 | −0,8% | 2.990/3.000; 2.773/3.000 |
| Neutral | 100.000 | 6 | 372,91 → 298,29 | −20,0% | 2.899/3.000; 1.597/3.000 |
| Neutral | 100.000 | 24 | 376,43 → 311,78 | −17,2% | 2.929/3.000; 1.716/3.000 |
| Neutral | 100.000 | 96 | 380,57 → 358,98 | −5,7% | 2.970/3.000; 2.254/3.000 |
| PvP | 2.500 | 6 | 41,67 → 40,75 | −2,2% | 1.301/1.943; 0/3.000 |
| PvP | 2.500 | 24 | 41,67 → 41,20 | −1,1% | 1.304/1.886; 0/3.000 |
| PvP | 2.500 | 96 | 41,67 → 41,67 | 0,0% | 1.364/1.788; 0/2.698 |
| PvP | 25.000 | 6 | 374,79 → 321,60 | −14,2% | 2.914/3.000; 1.915/3.000 |
| PvP | 25.000 | 24 | 376,83 → 333,56 | −11,5% | 2.940/3.000; 2.013/3.000 |
| PvP | 25.000 | 96 | 379,05 → 370,77 | −2,2% | 2.980/3.000; 2.494/3.000 |
| PvP | 100.000 | 6 | 543,53 → 414,68 | −23,7% | 2.901/3.000; 1.360/3.000 |
| PvP | 100.000 | 24 | 547,75 → 431,77 | −21,2% | 2.924/3.000; 1.463/3.000 |
| PvP | 100.000 | 96 | 553,28 → 496,96 | −10,2% | 2.960/3.000; 1.930/3.000 |

### Lectura del resultado

- Se valida la primera hipótesis: la cola representa una frontera mucho más grande. En el escenario neutral de 2.500 tropas y 6 tiles, el promedio fue 276,26 entradas de cola frente a 108,23 tiles únicos válidos por tick (2,55×).
- La diferencia de capturas es grande con fronteras angostas: para neutral, bajó 15,8–22,1% en las pruebas de 2.500 tropas con 6–24 tiles. Con 96 tiles, cambió menos de 1%. Esto valida la segunda hipótesis para este escenario controlado.
- El tope se alcanzó en 2.757/3.000 ticks con cola frente a 332/3.000 con frontera única en el caso neutral de 2.500/6. La cola no solo sobrecuenta; puede hacer que el límite oculte la diferencia entre presupuestos.
- PvP con 2.500 tropas no permite concluir sobre velocidad: el ataque se quedó sin tropas y el total capturado fue prácticamente igual. En ese rango, la pérdida de tropas limita el resultado antes que el contador de frontera.
- Son mapas y ataques sintéticos. Las tasas identifican el efecto del contador en esta prueba; todavía falta una partida representativa con múltiples ataques, terreno y defensas para evaluar el efecto de balance en el juego.

## Experimento 2: curva por tropas

**Estado: completado; no cumple todos los criterios en PvP.** Se comparó la base de frontera única con la curva después del tope actual, usando las mismas 10 semillas, mapa plano y 30 segundos nominales por semilla. La curva usó las tropas efectivas vivas en cada tick, no solo las tropas iniciales. Las tasas se normalizan por los 30 segundos completos; las tropas restantes son el promedio al final de cada semilla.

**Registro:** misma base SOW, harness, mapa, configuración, duración y semillas del experimento 1; una sola variable cambiada: multiplicador por tropas después del tope. La comparación usa frontera única en ambas ramas.

| Objetivo | Tropas iniciales | Frontera | Capturas/s: base → curva (Δ) | Tropas restantes: base → curva | Ticks al tope: base → curva |
| --- | ---: | ---: | --- | ---: | ---: |
| Neutral | 2.500 | 6 | 43,52 → 27,76 (−36,2%) | 1.199 → 1.671 | 332 → 0 |
| Neutral | 2.500 | 24 | 47,69 → 32,96 (−30,9%) | 1.073 → 1.515 | 687 → 0 |
| Neutral | 2.500 | 96 | 56,82 → 45,72 (−19,5%) | 799 → 1.131 | 2.480 → 2.314 |
| Neutral | 25.000 | 6 | 220,51 → 216,38 (−1,9%) | 18.411 → 18.533 | 2.100 → 2.099 |
| Neutral | 25.000 | 24 | 229,47 → 224,85 (−2,0%) | 18.141 → 18.279 | 2.206 → 2.209 |
| Neutral | 25.000 | 96 | 253,81 → 248,38 (−2,1%) | 17.411 → 17.573 | 2.773 → 2.771 |
| Neutral | 100.000 | 6 | 298,29 → 352,60 (+18,2%) | 91.089 → 89.465 | 1.597 → 1.775 |
| Neutral | 100.000 | 24 | 311,78 → 365,63 (+17,3%) | 90.684 → 89.074 | 1.716 → 1.876 |
| Neutral | 100.000 | 96 | 358,98 → 412,33 (+14,9%) | 89.268 → 87.672 | 2.254 → 2.346 |
| PvP | 2.500 | 6 | 40,75 → 32,33 (−20,7%) | 57 → 566 | 0 → 0 |
| PvP | 2.500 | 24 | 41,20 → 34,90 (−15,3%) | 29 → 411 | 0 → 0 |
| PvP | 2.500 | 96 | 41,67 → 41,29 (−0,9%) | 0 → 23 | 0 → 0 |
| PvP | 25.000 | 6 | 321,60 → 299,54 (−6,9%) | 5.781 → 7.092 | 1.915 → 1.902 |
| PvP | 25.000 | 24 | 333,56 → 309,42 (−7,2%) | 5.064 → 6.499 | 2.013 → 2.007 |
| PvP | 25.000 | 96 | 370,77 → 342,10 (−7,7%) | 2.810 → 4.534 | 2.494 → 2.493 |
| PvP | 100.000 | 6 | 414,68 → 486,25 (+17,3%) | 75.227 → 70.944 | 1.360 → 1.559 |
| PvP | 100.000 | 24 | 431,77 → 502,49 (+16,4%) | 74.201 → 69.969 | 1.463 → 1.649 |
| PvP | 100.000 | 96 | 496,96 → 564,06 (+13,5%) | 70.288 → 66.272 | 1.930 → 2.044 |

Los ticks al tope están medidos sobre 3.000 ticks activos, salvo PvP/2.500/96, cuya base duró 2.698 ticks y la curva 3.000.

### Lectura del resultado

- Neutral sigue claramente la intención: baja 19,5–36,2% en 2.500 tropas, cambia −1,9–2,1% en 25.000 y sube 14,9–18,2% en 100.000.
- PvP cumple early en frentes de 6 y 24, pero casi no baja con frontera 96 (−0,9%). El límite del test no es solo el presupuesto: a 2.500 tropas el ejército se agota.
- El criterio de punto medio falla en PvP: 25.000 tropas iniciales terminan 6,9–7,7% más lentas, por encima del máximo de 5 puntos porcentuales. La curva sigue las tropas vivas; las pérdidas PvP bajan el multiplicador por debajo de 1.00 durante buena parte del ataque.
- Late sube 13,5–18,2% en todos los frentes medidos. La prueba confirma la dirección, aunque la velocidad real depende también del límite y el desgaste.
- La curva por tropas sirve mejor para gobernar el tamaño de cada ataque que para identificar por sí sola la etapa de la partida. Por ese motivo se ejecutó el siguiente experimento por territorio.

## Experimento 3: curva por territorio propio

**Estado: completado; mejor señal de fase, requiere calibración por modo.** Se dejó fijo el ataque inicial en 25.000 tropas y se varió el territorio atacante inicial en un mapa llano de 256 × 256 tiles, todos de tierra. La señal es la proporción de tiles propios respecto a la tierra total del mapa. Puntos provisionales: 0,70× a 1%, 1,00× a 10% y 1,15× a 30%, con interpolación logarítmica y límites. Se comparó la curva con base sin multiplicador para cada mismo mapa, semilla y frontera.

**Registro:** misma base SOW y configuración estándar; semillas 1–10, 3.000 ticks por semilla; cambio aislado: multiplicador basado en `tile_count / tierra_total`, recalculado en cada tick. La comparación usa frontera única en ambas ramas.

| Objetivo | Territorio propio inicial | Frontera | Capturas/s: base → curva (Δ) | Tropas restantes: base → curva | Ticks al tope: base → curva |
| --- | ---: | ---: | --- | ---: | ---: |
| Neutral | 1% | 6 | 222,29 → 188,23 (−15,3%) | 18.357 → 19.379 | 2.188 → 1.951 |
| Neutral | 1% | 24 | 228,18 → 197,25 (−13,6%) | 18.180 → 19.108 | 2.247 → 2.047 |
| Neutral | 1% | 96 | 253,97 → 230,55 (−9,2%) | 17.407 → 18.110 | 2.772 → 2.717 |
| Neutral | 10% | 6 | 226,96 → 238,91 (+5,3%) | 18.216 → 17.861 | 2.285 → 2.280 |
| Neutral | 10% | 24 | 235,32 → 248,23 (+5,5%) | 17.966 → 17.581 | 2.385 → 2.374 |
| Neutral | 10% | 96 | 253,39 → 268,70 (+6,0%) | 17.424 → 16.967 | 2.762 → 2.756 |
| Neutral | 30% | 6 | 227,25 → 267,28 (+17,6%) | 18.208 → 17.011 | 2.301 → 2.436 |
| Neutral | 30% | 24 | 235,48 → 275,05 (+16,8%) | 17.961 → 16.778 | 2.416 → 2.530 |
| Neutral | 30% | 96 | 254,16 → 292,47 (+15,1%) | 17.401 → 16.256 | 2.836 → 2.855 |
| PvP | 1% | 6 | 327,22 → 288,15 (−11,9%) | 5.445 → 7.792 | 2.117 → 1.876 |
| PvP | 1% | 24 | 335,88 → 301,95 (−10,1%) | 4.925 → 6.964 | 2.195 → 1.987 |
| PvP | 1% | 96 | 370,84 → 353,53 (−4,7%) | 2.806 → 3.870 | 2.497 → 2.371 |
| PvP | 10% | 6 | 335,11 → 360,24 (+7,5%) | 4.971 → 3.466 | 2.092 → 2.103 |
| PvP | 10% | 24 | 346,11 → 371,02 (+7,2%) | 4.311 → 2.805 | 2.192 → 2.200 |
| PvP | 10% | 96 | 371,28 → 389,27 (+4,8%) | 2.781 → 1.685 | 2.518 → 2.530 |
| PvP | 30% | 6 | 340,69 → 388,97 (+14,2%) | 4.636 → 1.712 | 2.254 → 2.342 |
| PvP | 30% | 24 | 350,12 → 394,73 (+12,7%) | 4.070 → 1.356 | 2.347 → 2.443 |
| PvP | 30% | 96 | 376,63 → 404,08 (+7,3%) | 2.463 → 778 | 2.670 → 2.714 |

Cada variante tuvo 3.000 ticks activos (30 segundos nominales × 10 semillas). Las tasas son cambio contra la base de la misma distribución territorial; los ritmos absolutos entre distintos porcentajes no son una comparación aislada, porque cambia el mapa y, en PvP, el territorio del defensor.

### Lectura del resultado

- 1% de territorio sí ralentiza los dos modos en estos escenarios: −9,2 a −15,3% neutral y −4,7 a −11,9% PvP. La frontera PvP de 96 tiles apenas baja, así que sigue siendo el caso que más cuesta ralentizar.
- 10% queda cerca de la base, pero la curva se mueve mientras captura: en esos 30 segundos el territorio propio crece y el multiplicador deja 1.00. El efecto medido fue +5,3 a +6,0% neutral y +4,8 a +7,5% PvP.
- 30% acelera los dos modos, +15,1 a +17,6% neutral y +7,3 a +14,2% PvP. PvP ancho vuelve a quedar por debajo del aumento de 15%, lo que sugiere que pérdidas/tropas y presupuesto se deben modelar junto con la escala.
- Esta señal representa mejor la etapa global que tropas del ataque, pero los puntos 1/10/30% son provisionales. En PvP cambia también el tamaño del defensor; la comparación base/curva dentro de cada escenario aísla el multiplicador, pero no permite comparar directamente las tasas absolutas entre escenarios.
- La curva territorial es una alternativa viable para seguir afinando, no un balance final ya validado. Haría falta probar una distribución de mapa y varios ataques simultáneos antes de aceptar una implementación.

## Opciones si la curva territorial aún no cumple los objetivos

1. **Calibrar por modo.** Probar un ancla early más fuerte para PvP de frontera amplia, manteniendo una curva neutral separada. Evita que el desgaste y la relación de fuerzas oculten la señal del multiplicador.
2. **Ajustar los puntos de territorio.** Barrer el share early (por ejemplo, 0,5–2%), el punto medio (8–15%) y el late (25–40%) sin cambiar el método de interpolación; mantener cada corrida de una sola variable.
3. **Corregir la frontera única como cambio independiente.** La prueba demuestra el efecto; falta diseñar una representación eficiente que no obligue a escanear y desduplicar la cola en cada tick y que sobreviva checkpoint/restore.
4. **Ajustar el presupuesto neutral por separado.** Si el problema principal sigue siendo capturar neutrales, medir un costo/presupuesto neutral distinto sin ralentizar PvP.
5. **Cambiar pérdidas neutrales por tile.** SOW pierde 1 tropa por tile neutral por defecto; aumentarlo puede limitar cuánto dura un ataque, aunque no reduce necesariamente las capturas del primer segundo.
6. **Cambiar el tamaño del envío o el valor inicial del slider de ataque.** Es una palanca de cuánto ejército entra a cada ataque; no representa por sí sola la fase y puede crear varios ataques pequeños.
7. **Curva por tramos o combinación de señales.** Si una curva suave no da suficiente control, probar rangos o combinar share territorial con tropas solo después de aislar sus efectos. Mantener los mismos mapas y semillas para comparar.

No combinar alternativas en una misma corrida: cada prueba debe cambiar una sola variable para atribuir el resultado.

## Reproducción

Los experimentos y pruebas de anclas están en [`combat_tiles.rs`](../../sow-core/src/execution/combat_tiles.rs). El filtro del módulo que encuentra las pruebas es:

```sh
cargo test -p sow-core execution::combat::combat_tiles
```

El filtro original `execution::combat_tiles` no coincide con la ruta del módulo y devuelve cero pruebas. El filtro actual, verificado, es `execution::combat::combat_tiles`.

Última verificación del módulo: `cargo test -p sow-core execution::combat::combat_tiles` — **8 passed, 0 failed** (22,41 s). Incluye las dos comparaciones de frontera, los dos experimentos de curva, sus pruebas de anclas/interpolación logarítmica y las tres pruebas existentes del módulo.

## Registro para las siguientes corridas

Añadir fecha y revisión del código; variante y única variable cambiada; semillas, modo, tropas y frontera; capturas/s, tropas restantes, duración y ticks al tope; interpretación y siguiente hipótesis. Mantener por separado resultados sintéticos, partidas representativas y aceptación visual de juego.
