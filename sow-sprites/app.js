// app.js — Visor de spritesheet animado (estilo Unity).
//
// Carga una spritesheet (picker de archivos, drag & drop o abrir desde el menú
// de desarrollador), detecta su cuadrícula (grid.js) y reproduce los marcos
// marco a marco: reproduce a `fps`, reproduce/pausa, salta un marco a la vez y
// salta celdas con el clic en la vista previa de la hoja entera.

'use strict';

const state = {
  sheet: null,       // HTMLImageElement de la spritesheet cargada
  name: null,        // nombre del archivo (para el título)
  objectUrl: null,   // blob URL de la hoja cargada (para revocarla)
  cols: 1,
  rows: 1,
  fps: 12,
  frameIndex: 0,
  isPlaying: false,
  raf: null,
  nextDue: 0,
  detectedNote: null,
};

const MAX_INPUT = 999;
const FPS_MAX = 120;

const preview = document.getElementById('preview');
const sheetCanvas = document.getElementById('sheet-canvas');
const mainEl = document.getElementById('workspace');
const btnOpen = document.getElementById('btn-open');
const fileInput = document.getElementById('file-input');
const fileName = document.getElementById('file-name');
const colsInput = document.getElementById('cols');
const rowsInput = document.getElementById('rows');
const tileSize = document.getElementById('tile-size');
const fpsSlider = document.getElementById('fps-slider');
const fpsValue = document.getElementById('fps-value');
const btnPlay = document.getElementById('btn-play');
const btnStepBack = document.getElementById('btn-step-back');
const btnStepFwd = document.getElementById('btn-step-fwd');
const btnApply = document.getElementById('btn-apply');
const btnAuto = document.getElementById('btn-auto');
const btnSwap = document.getElementById('btn-swap');
const frameInfo = document.getElementById('frame-info');
const infoLine = document.getElementById('info-line');

const preCtx = preview.getContext('2d');
const sheetCtx = sheetCanvas.getContext('2d');
preCtx.imageSmoothingEnabled = false; // sprites de pixel art: escala verdadera (nearest)
sheetCtx.imageSmoothingEnabled = false;

// ---------------------------------------------------------------- helpers

function totalFrames() {
  return Math.max(1, state.cols * state.rows);
}

function currentIndex() {
  return Math.max(0, Math.min(state.frameIndex, totalFrames() - 1));
}

function framePos(index) {
  const idx = Math.max(0, Math.min(index, totalFrames() - 1));
  return { col: idx % state.cols, row: Math.floor(idx / state.cols) };
}

function clampInt(value, min, max) {
  const n = Number.isFinite(value) ? value : min;
  return Math.max(min, Math.min(max, Math.round(n)));
}

// Canvas pequeño usado para muestrear colores de celdas (promedio RGBA).
const sampleCanvas = document.createElement('canvas');
const sampleCtx = sampleCanvas.getContext('2d');

function cellColor(col, row, cols, rows) {
  const sheet = state.sheet;
  if (!sheet) return null;
  const r = tileRect(sheet.naturalWidth, sheet.naturalHeight, cols, rows, col, row);
  if (!r || r.w < 0.5 || r.h < 0.5) return null;
  const tw = Math.max(1, Math.ceil(r.w));
  const th = Math.max(1, Math.ceil(r.h));
  sampleCanvas.width = tw;
  sampleCanvas.height = th;
  sampleCtx.clearRect(0, 0, tw, th);
  sampleCtx.drawImage(sheet, r.x, r.y, r.w, r.h, 0, 0, tw, th);
  const data = sampleCtx.getImageData(0, 0, tw, th).data;
  let sr = 0, sg = 0, sb = 0, sa = 0;
  for (let i = 0; i < data.length; i += 4) {
    sr += data[i];
    sg += data[i + 1];
    sb += data[i + 2];
    sa += data[i + 3];
  }
  const n = data.length / 4;
  return { r: sr / n, g: sg / n, b: sb / n, a: sa / n };
}

function runDetection() {
  if (!state.sheet) return { cols: 1, rows: 1 };
  const detected = detectGrid(
    state.sheet.naturalWidth,
    state.sheet.naturalHeight,
    cellColor
  );
  if (!detected || !Number.isFinite(detected.cols) || detected.cols < 1 ||
      !Number.isFinite(detected.rows) || detected.rows < 1) {
    return { cols: 1, rows: 1 };
  }
  return {
    cols: Math.floor(detected.cols),
    rows: Math.floor(detected.rows),
    note: detected.note,
  };
}

// ---------------------------------------------------------------- UI update

function setGridInputs() {
  colsInput.value = String(state.cols);
  rowsInput.value = String(state.rows);
}

function updateInfo() {
  if (!state.sheet) {
    infoLine.textContent = 'Sin spritesheet cargada.';
    tileSize.textContent = '—';
    frameInfo.textContent = '0 / 0';
    fileName.textContent = 'Sin spritesheet cargada';
    document.title = 'Sow Spritesheet Viewer';
    return;
  }
  const w = state.sheet.naturalWidth;
  const h = state.sheet.naturalHeight;
  const pos = framePos(state.frameIndex);
  const r = tileRect(w, h, state.cols, state.rows, pos.col, pos.row);
  fileName.textContent = state.name || 'spritesheet';
  const parts = [`${w}×${h}`, `cuadrícula ${state.cols}×${state.rows}`];
  if (r && r.w > 0 && r.h > 0) {
    tileSize.textContent = `${Math.max(1, Math.round(r.w))}×${Math.max(1, Math.round(r.h))} px`;
    parts.push(`celda ${tileSize.textContent}`);
  } else {
    tileSize.textContent = '—';
  }
  parts.push(`${totalFrames()} marcos`);
  let text = parts.join(' · ');
  if (state.detectedNote) text += ` · ${state.detectedNote}`;
  infoLine.textContent = text;
  document.title = `Sow Spritesheet Viewer — ${fileName.textContent}`;
}

function updateFrameInfo() {
  if (!state.sheet) {
    frameInfo.textContent = '0 / 0';
    return;
  }
  const idx = currentIndex();
  frameInfo.textContent = `${idx + 1} / ${totalFrames()}`;
  frameInfo.setAttribute('title', `Marco ${idx + 1} de ${totalFrames()}`);
}

// ---------------------------------------------------------------- drawing

function drawPreview() {
  if (!state.sheet) {
    preview.width = 1;
    preview.height = 1;
    preCtx.clearRect(0, 0, 1, 1);
    return;
  }
  const pos = framePos(state.frameIndex);
  const r = tileRect(state.sheet.naturalWidth, state.sheet.naturalHeight, state.cols, state.rows, pos.col, pos.row);
  const tw = Math.max(1, Math.round(r.w));
  const th = Math.max(1, Math.round(r.h));
  preview.width = tw;
  preview.height = th;
  preCtx.clearRect(0, 0, tw, th);
  preCtx.drawImage(state.sheet, r.x, r.y, r.w, r.h, 0, 0, tw, th);
}

function drawSheetCanvas() {
  if (!state.sheet) {
    sheetCanvas.width = 1;
    sheetCanvas.height = 1;
    sheetCtx.clearRect(0, 0, 1, 1);
    return;
  }
  const w = state.sheet.naturalWidth;
  const h = state.sheet.naturalHeight;
  sheetCanvas.width = w;
  sheetCanvas.height = h;
  sheetCtx.clearRect(0, 0, w, h);
  sheetCtx.drawImage(state.sheet, 0, 0);
  const pos = framePos(state.frameIndex);
  const r = tileRect(w, h, state.cols, state.rows, pos.col, pos.row);
  sheetCtx.lineWidth = 2;
  sheetCtx.strokeStyle = '#5b8cff';
  sheetCtx.strokeRect(
    Math.round(r.x), Math.round(r.y),
    Math.max(1, Math.round(r.w)), Math.max(1, Math.round(r.h))
  );
}

// ---------------------------------------------------------------- loading

function onSheetLoaded(img, name) {
  stopTimer();
  state.sheet = img;
  state.name = name;
  state.frameIndex = 0;

  const detected = runDetection();
  state.cols = detected.cols;
  state.rows = detected.rows;
  state.detectedNote = detected.note;

  setGridInputs();
  drawPreview();
  drawSheetCanvas();
  updateInfo();
  updateFrameInfo();
  btnPlay.textContent = state.isPlaying ? '⏸' : '▶';
  btnPlay.setAttribute('aria-label', state.isPlaying ? 'Pausar' : 'Reproducir');
  mainEl.focus();
}

function showError(msg) {
  infoLine.textContent = `⚠ ${msg}`;
}

function loadSheet(src, name) {
  if (state.objectUrl) URL.revokeObjectURL(state.objectUrl);
  state.objectUrl = src;

  const img = new Image();
  img.src = src;
  if (typeof img.decode === 'function') {
    img.decode()
      .then(() => {
        if (!img.naturalWidth || !img.naturalHeight) throw new Error('sin dimensiones');
        onSheetLoaded(img, name);
      })
      .catch(() => showError(`No se pudo cargar la spritesheet: ${name}`));
  } else {
    img.onload = () => {
      if (img.naturalWidth && img.naturalHeight) onSheetLoaded(img, name);
      else showError(`No se pudo cargar la spritesheet: ${name}`);
    };
    img.onerror = () => showError(`No se pudo cargar la spritesheet: ${name}`);
  }
}

// Abierto desde el menú de desarrollador (main.js pasa una URL de archivo y el nombre).
function loadSpritesheetFromMain(src, name) {
  loadSheet(src, name);
}
window.loadSpritesheetFromMain = loadSpritesheetFromMain;

btnOpen.addEventListener('click', () => fileInput.click());
fileInput.addEventListener('change', () => {
  const file = fileInput.files ? fileInput.files[0] : null;
  fileInput.value = '';
  if (!file) return;
  if (file.type && !file.type.startsWith('image/')) {
    showError('Elige un archivo de imagen.');
    return;
  }
  loadSheet(URL.createObjectURL(file), file.name);
});

function onDrop(evt) {
  evt.preventDefault();
  mainEl.classList.remove('drag-over');
  const files = evt.dataTransfer ? evt.dataTransfer.files : null;
  if (!files || files.length === 0) return;
  const file = files[0];
  if (file.type && !file.type.startsWith('image/')) {
    showError('Elige un archivo de imagen.');
    return;
  }
  loadSheet(URL.createObjectURL(file), file.name);
}
mainEl.addEventListener('dragover', (evt) => { evt.preventDefault(); mainEl.classList.add('drag-over'); });
mainEl.addEventListener('dragleave', () => mainEl.classList.remove('drag-over'));
mainEl.addEventListener('drop', onDrop);

// ---------------------------------------------------------------- animation

function startTimer() {
  if (state.isPlaying || !state.sheet) return;
  state.isPlaying = true;
  state.nextDue = performance.now() + 1000 / state.fps;
  state.raf = requestAnimationFrame(loop);
}

function stopTimer() {
  if (state.raf !== null) cancelAnimationFrame(state.raf);
  state.raf = null;
  state.isPlaying = false;
}

function loop() {
  if (!state.sheet || !state.isPlaying) {
    stopTimer();
    return;
  }
  const now = performance.now();
  if (now >= state.nextDue) {
    advanceFrame();
    state.nextDue = now + 1000 / state.fps;
  }
  state.raf = requestAnimationFrame(loop);
}

function advanceFrame() {
  if (!state.sheet) return;
  state.frameIndex = (state.frameIndex + 1) % totalFrames();
  drawPreview();
  drawSheetCanvas();
  updateFrameInfo();
}

function togglePlay() {
  if (!state.sheet) return;
  if (state.isPlaying) stopTimer();
  else startTimer();
  btnPlay.textContent = state.isPlaying ? '⏸' : '▶';
  btnPlay.setAttribute('aria-label', state.isPlaying ? 'Pausar' : 'Reproducir');
}

function stepBack() {
  if (!state.sheet) return;
  state.frameIndex = (state.frameIndex - 1 + totalFrames()) % totalFrames();
  drawPreview();
  drawSheetCanvas();
  updateFrameInfo();
  if (state.isPlaying) state.nextDue = performance.now();
}

function stepForward() {
  if (!state.sheet) return;
  advanceFrame();
  if (state.isPlaying) state.nextDue = performance.now();
}

// ---------------------------------------------------------------- controls

function setFps(value) {
  state.fps = clampInt(value, 1, FPS_MAX);
  fpsValue.value = String(state.fps);
  fpsSlider.value = String(state.fps);
  if (state.isPlaying) state.nextDue = performance.now();
}
fpsSlider.addEventListener('input', () => setFps(+fpsSlider.value));
fpsValue.addEventListener('change', () => setFps(+fpsValue.value));

btnPlay.addEventListener('click', togglePlay);
btnStepBack.addEventListener('click', stepBack);
btnStepFwd.addEventListener('click', stepForward);

function applyGrid() {
  const cols = clampInt(Number(colsInput.value), 1, MAX_INPUT);
  const rows = clampInt(Number(rowsInput.value), 1, MAX_INPUT);
  if (!state.sheet) {
    state.cols = 1;
    state.rows = 1;
    setGridInputs();
    return;
  }
  state.cols = cols;
  state.rows = rows;
  state.detectedNote = null;
  state.frameIndex = 0;
  setGridInputs();
  drawPreview();
  drawSheetCanvas();
  updateInfo();
  updateFrameInfo();
}
btnApply.addEventListener('click', applyGrid);
colsInput.addEventListener('change', applyGrid);
rowsInput.addEventListener('change', applyGrid);

function autoDetect() {
  if (!state.sheet) return;
  const detected = runDetection();
  state.cols = detected.cols;
  state.rows = detected.rows;
  state.detectedNote = detected.note;
  setGridInputs();
  drawPreview();
  drawSheetCanvas();
  updateInfo();
  updateFrameInfo();
}
btnAuto.addEventListener('click', autoDetect);

function swapGrid() {
  const t = state.cols;
  state.cols = state.rows;
  state.rows = t;
  state.detectedNote = null;
  state.frameIndex = 0;
  setGridInputs();
  drawPreview();
  drawSheetCanvas();
  updateInfo();
  updateFrameInfo();
}
btnSwap.addEventListener('click', swapGrid);

// Salta a un marco de la vista de la hoja entera.
sheetCanvas.addEventListener('click', (evt) => {
  if (!state.sheet) return;
  const tileW = state.sheet.naturalWidth / state.cols;
  const tileH = state.sheet.naturalHeight / state.rows;
  const col = Math.floor(evt.offsetX / tileW);
  const row = Math.floor(evt.offsetY / tileH);
  const idx = row * state.cols + col;
  if (idx >= 0 && idx < totalFrames()) {
    state.frameIndex = idx;
    drawPreview();
    drawSheetCanvas();
    updateFrameInfo();
  }
});

// ---------------------------------------------------------------- keyboard

document.addEventListener('keydown', (evt) => {
  const t = evt.target;
  if (!t) return;
  if (t !== document.body && t !== mainEl && t.tagName !== 'CANVAS') return;
  switch (evt.key) {
    case ' ':
    case 'Spacebar':
      evt.preventDefault();
      togglePlay();
      break;
    case 'ArrowRight':
      evt.preventDefault();
      stepForward();
      break;
    case 'ArrowLeft':
      evt.preventDefault();
      stepBack();
      break;
    default:
      break;
  }
});

// ---------------------------------------------------------------- init

setFps(state.fps);
updateInfo();
updateFrameInfo();