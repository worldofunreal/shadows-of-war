// grid.js — Detección de la cuadrícula de una spritesheet.
//
// Dado el tamaño de la hoja (ancho, alto) y un callback cellColor(col, row, cols, rows)
// que devuelve el color medio de una celda (o null si no existe), detectGrid()
// devuelve una sugerencia de cuadrícula: { cols, rows, tileW, tileH, note }.
//
// Heurística, en orden:
//   1. Tira horizontal: ancho = k·alto con k ≥ 2 → k columnas de tiles del alto completo.
//   2. Tira vertical: alto = k·ancho con k ≥ 2 → k filas de tiles del ancho completo.
//   3. Imagen cuadrada: prueba cuadrículas 2×2..8×8 (la más frecuente primero);
//      si el tamaño es divisible y los colores medios de las celdas varían,
//      asume esa cuadrícula. Si nada varía, devuelve 1×1.
//   4. Resto: tiles cuadrados iguales por el GCD de las dimensiones. Si el GCD
//      es 1 (dimensiones coprimas) devuelve 1×1.
//
// Todo es una heurística: los valores detectados se muestran en la UI y se pueden
// editar a mano.
(function (root) {
  'use strict';

  // Distancia cuadrática media (suma de cuadrados de las diferencias de los 4 canales).
  const VARIANCE_THRESHOLD = 25 * 25 * 3;

  function gcd(a, b) {
    while (b) {
      const t = b;
      b = a % b;
      a = t;
    }
    return a;
  }

  // true si alguna pareja de celdas de una cuadrícula de cols×rows tiene colores
  // medios lo suficientemente distintos.
  function colorsVary(cellColor, cols, rows) {
    const cells = [];
    for (let j = 0; j < rows; j++) {
      for (let i = 0; i < cols; i++) {
        const c = cellColor(i, j, cols, rows);
        if (!c) return false; // celda inexistente o ilegible: no podemos verificar
        cells.push(c);
      }
    }
    for (let a = 0; a < cells.length; a++) {
      for (let b = a + 1; b < cells.length; b++) {
        const A = cells[a];
        const B = cells[b];
        const d = (A.r - B.r) ** 2 + (A.g - B.g) ** 2 + (A.b - B.b) ** 2 + (A.a - B.a) ** 2;
        if (d > VARIANCE_THRESHOLD) return true;
      }
    }
    return false;
  }

  // Rectángulo de una celda de una cuadrícula: col ∈ [0, cols), row ∈ [0, rows).
  function tileRect(w, h, cols, rows, col, row) {
    const tileW = w / cols;
    const tileH = h / rows;
    const x = col * tileW;
    const y = row * tileH;
    const tw = Math.min(tileW, Math.max(0, w - x));
    const th = Math.min(tileH, Math.max(0, h - y));
    return { x, y, w: tw, h: th };
  }

  function detectGrid(width, height, cellColor) {
    if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
      return null;
    }

    // 1. Tira horizontal: ancho = k·alto (k ≥ 2) → k columnas de tiles del alto.
    if (width > height && width % height === 0) {
      const k = width / height;
      if (k >= 2) {
        return { cols: k, rows: 1, tileW: height, tileH: height, note: `tira horizontal: ${k} columnas` };
      }
    }

    // 2. Tira vertical: alto = k·ancho (k ≥ 2) → k filas de tiles del ancho.
    if (height > width && height % width === 0) {
      const k = height / width;
      if (k >= 2) {
        return { cols: 1, rows: k, tileW: width, tileH: width, note: `tira vertical: ${k} filas` };
      }
    }

    // 3. Imagen cuadrada: prueba cuadrículas comunes, en orden de frecuencia.
    if (width === height) {
      for (let n = 2; n <= 8; n++) {
        if (width % n === 0 && colorsVary(cellColor, n, n)) {
          return {
            cols: n,
            rows: n,
            tileW: width / n,
            tileH: width / n,
            note: `cuadrícula ${n}×${n} detectada`,
          };
        }
      }
      return { cols: 1, rows: 1, tileW: width, tileH: height, note: '1 frame (todo el sprite)' };
    }

    // 4. Tiles cuadrados iguales por el GCD de las dimensiones.
    const t = gcd(width, height);
    if (t >= 2) {
      return {
        cols: width / t,
        rows: height / t,
        tileW: t,
        tileH: t,
        note: `tiles cuadrados iguales: ${width / t}×${height / t}`,
      };
    }
    return { cols: 1, rows: 1, tileW: width, tileH: height, note: 'sin cuadrícula regular' };
  }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = { detectGrid, colorsVary, tileRect, VARIANCE_THRESHOLD };
  }
  root.detectGrid = detectGrid;
  root.colorsVary = colorsVary;
  root.tileRect = tileRect;
})(typeof window !== 'undefined' ? window : globalThis);
