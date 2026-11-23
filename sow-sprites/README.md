# Sow Spritesheet Viewer

Visor de spritesheets animadas en Electron: carga una spritesheet, detecta su
cuadrícula (2×2, 3×3, tiras, …) y la anima marco a marco al estilo Unity.

## Correr

```
cd sow-sprites
electron .
```

(Electron está instalado como `electron` en el PATH; también vale
`npx electron .` desde aquí.)

## Uso

- **Abrir spritesheet**: botón superior, `Ctrl+O` (Archivo → Abrir spritesheet…)
  o arrastrar el archivo sobre la vista principal.
- **Cuadrícula**: la detección automática propone los tiles; los campos
  `cols × rows` se pueden editar, **Aplicar** los usa y **Auto** vuelve a
  detectarlos. **Swap** intercambia columnas y filas.
- **Animación**: slider/campo de **FPS**, y los botones
  `◀` / `▶` (Reproducir) / `▶` (Frame siguiente): cada marco cambia el sprite.
  Space también reproduce/pausa; ← / → avanzan/retroceden un marco.
- La vista de la hoja entera (debajo del marco actual) muestra el destello
  actual; hacer clic en una celda salta a ese marco.

## Notas de la detección

Heurística en `grid.js`:

1. Tira horizontal/vertical (dimensión múltiplo de la otra, k ≥ 2).
2. Imagen cuadrada: prueba 2×2, 3×3, … 8×8 — la primera que "varies" (colores
   medios de las celdas distintos), si el tamaño es divisible.
3. Resto: tiles cuadrados iguales por el GCD de las dimensiones.

Los valores detectados se muestran en la UI y siempre se pueden corregir a
mano, así que el detección es una sugerencia, no una verdad absoluta.
