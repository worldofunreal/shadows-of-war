/**
 * Sound Studio — Interactive Procedural Audio Audition & Pre-Design Tool (`./sow m`).
 *
 * Imports the canonical Web Audio procedural engine (`sow-web/shell/audio/`) directly,
 * bundled on the fly by `server.mts`. Any edit to a recipe in `sow-web/shell/audio/`
 * is live immediately after a browser reload (F5).
 */

import {
  AudioCue,
  audioEngine,
  type AudioCategory,
  type AudioCueId,
} from "../../../sow-web/shell/audio/index.ts";
import {
  buildSoundCatalog,
  CATEGORY_LABELS,
  CATEGORY_ORDER,
  type CueCatalogEntry,
  type CueVariantSpec,
} from "./sound-catalog.ts";

type Verdict = "none" | "ok" | "tweak" | "remake";
type CategoryFilter = "all" | "noted" | AudioCategory;

interface CueNoteRecord {
  verdict: Verdict;
  note: string;
  lastSubtype?: number;
  lastLabel?: string;
  lastIntensity?: number;
}

const STORAGE_KEY = "sow_sound_studio_notes_v1";

const catalog: CueCatalogEntry[] = buildSoundCatalog();
const byId = new Map<AudioCueId, CueCatalogEntry>(catalog.map((e) => [e.id, e]));

// ── State ─────────────────────────────────────────────────────────────
let activeFilter: CategoryFilter = "all";
let searchQuery = "";
let selectedCueId: AudioCueId = catalog.find((e) => e.id === AudioCue.AttackOrder)?.id ?? catalog[0].id;

const nextVariantIdx = new Map<AudioCueId, number>();
const activeVariantIdx = new Map<AudioCueId, number>();
const seedStepByCue = new Map<AudioCueId, number>();
const lastSeedByCue = new Map<AudioCueId, number>();
let globalSeedCounter = 0x243f6a88;

let intensityOverrideActive = false;
let burstTimers: number[] = [];
let runningBurstCueId: AudioCueId | null = null;
let nowPlayingPulseTimer = 0;

let notes: Record<number, CueNoteRecord> = loadNotes();

// ── DOM References ────────────────────────────────────────────────────
const searchInput = document.getElementById("searchInput") as HTMLInputElement;
const nowPlayingEl = document.getElementById("nowPlaying") as HTMLElement;
const npCueEl = document.getElementById("npCue") as HTMLElement;
const npTypeEl = document.getElementById("npType") as HTMLElement;
const npMetaEl = document.getElementById("npMeta") as HTMLElement;
const catTabsEl = document.getElementById("catTabs") as HTMLElement;
const catalogEl = document.getElementById("catalog") as HTMLElement;

const vizCanvas = document.getElementById("vizCanvas") as HTMLCanvasElement;
const vizCtx = vizCanvas.getContext("2d");
const peakReadoutEl = document.getElementById("peakReadout") as HTMLElement;

const autoCycleToggle = document.getElementById("autoCycleToggle") as HTMLInputElement;
const realCooldownToggle = document.getElementById("realCooldownToggle") as HTMLInputElement;
const lockSeedToggle = document.getElementById("lockSeedToggle") as HTMLInputElement;
const volSlider = document.getElementById("volSlider") as HTMLInputElement;
const volValEl = document.getElementById("volVal") as HTMLElement;
const intensitySlider = document.getElementById("intensitySlider") as HTMLInputElement;
const intensityValEl = document.getElementById("intensityVal") as HTMLElement;
const panSlider = document.getElementById("panSlider") as HTMLInputElement;
const panValEl = document.getElementById("panVal") as HTMLElement;
const distSlider = document.getElementById("distSlider") as HTMLInputElement;
const distValEl = document.getElementById("distVal") as HTMLElement;

const playSelectedBtn = document.getElementById("playSelectedBtn") as HTMLButtonElement;
const burstSelectedBtn = document.getElementById("burstSelectedBtn") as HTMLButtonElement;
const resetControlsBtn = document.getElementById("resetControlsBtn") as HTMLButtonElement;

const selTitleEl = document.getElementById("selTitle") as HTMLElement;
const selIdEl = document.getElementById("selId") as HTMLElement;
const selDescEl = document.getElementById("selDesc") as HTMLElement;
const metaRecipeEl = document.getElementById("metaRecipe") as HTMLElement;
const metaGainEl = document.getElementById("metaGain") as HTMLElement;
const metaCooldownEl = document.getElementById("metaCooldown") as HTMLElement;
const metaSpatialEl = document.getElementById("metaSpatial") as HTMLElement;

const verdictRowEl = document.getElementById("verdictRow") as HTMLElement;
const noteInput = document.getElementById("noteInput") as HTMLTextAreaElement;
const notesCountEl = document.getElementById("notesCount") as HTMLElement;
const copyBriefBtn = document.getElementById("copyBriefBtn") as HTMLButtonElement;
const clearNotesBtn = document.getElementById("clearNotesBtn") as HTMLButtonElement;
const copyStatusEl = document.getElementById("copyStatus") as HTMLElement;

// ── Persistence ───────────────────────────────────────────────────────
function loadNotes(): Record<number, CueNoteRecord> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, CueNoteRecord>;
    const out: Record<number, CueNoteRecord> = {};
    for (const [k, v] of Object.entries(parsed)) {
      const id = Number(k);
      if (Number.isInteger(id) && v && typeof v === "object") {
        const verdict: Verdict =
          v.verdict === "ok" || v.verdict === "tweak" || v.verdict === "remake" ? v.verdict : "none";
        out[id] = {
          verdict,
          note: typeof v.note === "string" ? v.note : "",
          lastSubtype: typeof v.lastSubtype === "number" ? v.lastSubtype : undefined,
          lastLabel: typeof v.lastLabel === "string" ? v.lastLabel : undefined,
          lastIntensity: typeof v.lastIntensity === "number" ? v.lastIntensity : undefined,
        };
      }
    }
    return out;
  } catch {
    return {};
  }
}

function saveNotes(): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(notes));
  } catch {
    // Ignore storage quota / privacy mode failures.
  }
}

function hasAnnotation(id: AudioCueId): boolean {
  const rec = notes[id];
  return Boolean(rec && (rec.verdict !== "none" || rec.note.trim().length > 0));
}

function annotatedCount(): number {
  let count = 0;
  for (const entry of catalog) {
    if (hasAnnotation(entry.id)) count++;
  }
  return count;
}

function ensureNoteRecord(id: AudioCueId): CueNoteRecord {
  if (!notes[id]) {
    notes[id] = { verdict: "none", note: "" };
  }
  return notes[id];
}

// ── Real-Time Analyser Visualizer ─────────────────────────────────────
let analyser: AnalyserNode | null = null;
let timeData: Float32Array<ArrayBuffer> | null = null;
let freqData: Uint8Array<ArrayBuffer> | null = null;
let vizRafId = 0;
let vizIdleFrames = 0;
let heldPeakDb = -96;
let peakHoldFrames = 0;

function ensureMonitor(): void {
  if (analyser) return;
  audioEngine.unlock();
  const ctx = audioEngine.getContext();
  if (!ctx) return;
  analyser = ctx.createAnalyser();
  analyser.fftSize = 1024;
  analyser.smoothingTimeConstant = 0.74;
  analyser.minDecibels = -84;
  analyser.maxDecibels = -4;
  if (audioEngine.connectMonitor(analyser)) {
    timeData = new Float32Array(new ArrayBuffer(analyser.fftSize * 4));
    freqData = new Uint8Array(new ArrayBuffer(analyser.frequencyBinCount));
  }
}

function wakeVisualizer(): void {
  ensureMonitor();
  vizIdleFrames = 0;
  if (!vizRafId) {
    vizRafId = requestAnimationFrame(drawVisualizerFrame);
  }
}

function drawVisualizerIdle(): void {
  if (!vizCtx) return;
  const w = vizCanvas.width;
  const h = vizCanvas.height;
  vizCtx.clearRect(0, 0, w, h);
  vizCtx.fillStyle = "#090f12";
  vizCtx.fillRect(0, 0, w, h);
  drawFreqGuides(w, h, 48000);
  vizCtx.strokeStyle = "#263537";
  vizCtx.lineWidth = 1;
  vizCtx.beginPath();
  vizCtx.moveTo(0, h * 0.5);
  vizCtx.lineTo(w, h * 0.5);
  vizCtx.stroke();
  peakReadoutEl.textContent = "-∞ dB";
  peakReadoutEl.style.color = "";
}

function freqToLogX(freqHz: number, width: number, minHz = 40, maxHz = 16000): number {
  const ratio = Math.log(Math.max(minHz, Math.min(maxHz, freqHz)) / minHz) / Math.log(maxHz / minHz);
  return ratio * width;
}

function drawFreqGuides(w: number, h: number, _sampleRate: number): void {
  if (!vizCtx) return;
  const marks = [100, 500, 2000, 8000];
  vizCtx.strokeStyle = "#192427";
  vizCtx.lineWidth = 1;
  for (const hz of marks) {
    const x = Math.round(freqToLogX(hz, w)) + 0.5;
    vizCtx.beginPath();
    vizCtx.moveTo(x, 0);
    vizCtx.lineTo(x, h);
    vizCtx.stroke();
  }
}

function drawVisualizerFrame(): void {
  vizRafId = 0;
  if (!vizCtx || !analyser || !timeData || !freqData) {
    drawVisualizerIdle();
    return;
  }

  analyser.getFloatTimeDomainData(timeData);
  analyser.getByteFrequencyData(freqData);

  const w = vizCanvas.width;
  const h = vizCanvas.height;
  const sampleRate = analyser.context.sampleRate || 48000;
  const binHz = sampleRate / analyser.fftSize;

  let peak = 0;
  for (let i = 0; i < timeData.length; i++) {
    const abs = Math.abs(timeData[i]);
    if (abs > peak) peak = abs;
  }
  const db = peak > 1e-5 ? 20 * Math.log10(peak) : -96;
  if (db >= heldPeakDb) {
    heldPeakDb = db;
    peakHoldFrames = 32;
  } else if (peakHoldFrames > 0) {
    peakHoldFrames--;
  } else {
    heldPeakDb = Math.max(-96, heldPeakDb - 1.4);
  }

  peakReadoutEl.textContent = heldPeakDb <= -80 ? "-∞ dB" : `${heldPeakDb.toFixed(1)} dB`;
  peakReadoutEl.style.color = heldPeakDb > -1.5 ? "#ff7770" : heldPeakDb > -12 ? "#e9bb64" : "#98a7a5";

  vizCtx.clearRect(0, 0, w, h);
  vizCtx.fillStyle = "#090f12";
  vizCtx.fillRect(0, 0, w, h);
  drawFreqGuides(w, h, sampleRate);

  // Logarithmic FFT spectrum bars
  const barCount = 48;
  const minHz = 40;
  const maxHz = 16000;
  const logRatio = Math.log(maxHz / minHz);
  for (let b = 0; b < barCount; b++) {
    const f0 = minHz * Math.exp((b / barCount) * logRatio);
    const f1 = minHz * Math.exp(((b + 1) / barCount) * logRatio);
    const i0 = Math.max(1, Math.floor(f0 / binHz));
    const i1 = Math.min(freqData.length - 1, Math.max(i0, Math.ceil(f1 / binHz)));
    let maxVal = 0;
    for (let i = i0; i <= i1; i++) {
      if (freqData[i] > maxVal) maxVal = freqData[i];
    }
    const norm = maxVal / 255;
    if (norm > 0.01) {
      const x = (b / barCount) * w;
      const bw = Math.max(2, w / barCount - 1.5);
      const bh = norm * (h - 10);
      vizCtx.fillStyle = norm > 0.78 ? "rgba(233, 187, 100, 0.72)" : "rgba(233, 187, 100, 0.36)";
      vizCtx.fillRect(x, h - bh, bw, bh);
    }
  }

  // Center line
  vizCtx.strokeStyle = "#223033";
  vizCtx.lineWidth = 1;
  vizCtx.beginPath();
  vizCtx.moveTo(0, h * 0.5);
  vizCtx.lineTo(w, h * 0.5);
  vizCtx.stroke();

  // Oscilloscope waveform
  vizCtx.strokeStyle = "#e8eeea";
  vizCtx.lineWidth = 1.5;
  vizCtx.beginPath();
  const stride = Math.max(1, Math.floor(timeData.length / w));
  for (let x = 0; x < w; x++) {
    const idx = Math.min(timeData.length - 1, x * stride);
    const y = h * 0.5 - timeData[idx] * (h * 0.44);
    if (x === 0) vizCtx.moveTo(x, y);
    else vizCtx.lineTo(x, y);
  }
  vizCtx.stroke();

  if (peak < 0.0004 && heldPeakDb <= -78) {
    vizIdleFrames++;
    if (vizIdleFrames > 90) {
      drawVisualizerIdle();
      return;
    }
  } else {
    vizIdleFrames = 0;
  }
  vizRafId = requestAnimationFrame(drawVisualizerFrame);
}

// ── Playback & Cycling ────────────────────────────────────────────────
function stopBurst(): void {
  for (const t of burstTimers) window.clearTimeout(t);
  burstTimers = [];
  if (runningBurstCueId != null) {
    const prevBtn = catalogEl.querySelector<HTMLButtonElement>(
      `[data-burst-cue="${runningBurstCueId}"]`,
    );
    prevBtn?.classList.remove("is-running");
    runningBurstCueId = null;
    burstSelectedBtn.classList.remove("is-running");
    burstSelectedBtn.textContent = "× Ráfaga / Escala";
  }
}

function nextSeedForCue(id: AudioCueId): { seed: number; step: number } {
  const locked = lockSeedToggle.checked;
  if (locked && lastSeedByCue.has(id)) {
    return {
      seed: lastSeedByCue.get(id)!,
      step: seedStepByCue.get(id) ?? 1,
    };
  }
  globalSeedCounter = (globalSeedCounter + 0x9e3779b9) >>> 0;
  const step = ((seedStepByCue.get(id) ?? 0) % 8) + 1;
  seedStepByCue.set(id, step);
  lastSeedByCue.set(id, globalSeedCounter);
  return { seed: globalSeedCounter, step };
}

function updateSliderReadouts(): void {
  const vol = Number(volSlider.value);
  volValEl.textContent = `${Math.round(vol * 100)}%`;
  audioEngine.syncSettings({ sfx_volume: vol, mute_all: vol <= 0.001 });

  const intensity = Number(intensitySlider.value);
  intensityValEl.textContent = intensity.toFixed(2);

  const pan = Number(panSlider.value);
  panValEl.textContent =
    Math.abs(pan) < 0.03 ? "C" : pan < 0 ? `L${Math.round(-pan * 100)}` : `R${Math.round(pan * 100)}`;

  const dist = Number(distSlider.value);
  distValEl.textContent = dist >= 0.92 ? "Cerca" : dist <= 0.4 ? "Lejos" : `${Math.round(dist * 100)}%`;
}

function triggerVariant(
  entry: CueCatalogEntry,
  variantIndex: number,
  options?: { advanceNext?: boolean; respectVariantIntensity?: boolean },
): void {
  const clampedIdx = ((variantIndex % entry.variants.length) + entry.variants.length) % entry.variants.length;
  const variant: CueVariantSpec = entry.variants[clampedIdx];
  activeVariantIdx.set(entry.id, clampedIdx);
  if (options?.advanceNext) {
    nextVariantIdx.set(entry.id, (clampedIdx + 1) % entry.variants.length);
  }

  let intensity = Number(intensitySlider.value);
  if (options?.respectVariantIntensity && variant.intensity != null && !intensityOverrideActive) {
    intensity = variant.intensity;
    intensitySlider.value = String(intensity);
    intensityValEl.textContent = intensity.toFixed(2);
  }

  const pan = Number(panSlider.value);
  const gain = Number(distSlider.value);
  const { seed, step } = nextSeedForCue(entry.id);

  wakeVisualizer();
  const durSec = audioEngine.audition(
    {
      cue: entry.id,
      subtype: variant.subtype,
      pan,
      gain,
      intensity,
      seed,
    },
    { bypassCooldown: !realCooldownToggle.checked },
  );

  // Record last auditioned subtype/intensity on existing notes (without creating empty notes)
  if (notes[entry.id]) {
    notes[entry.id].lastSubtype = variant.subtype;
    notes[entry.id].lastLabel = variant.label;
    notes[entry.id].lastIntensity = intensity;
    saveNotes();
  }

  // Highlight active variant chip in catalog DOM without full re-render
  const card = catalogEl.querySelector<HTMLElement>(`[data-cue-card="${entry.id}"]`);
  if (card) {
    card.querySelectorAll<HTMLButtonElement>("[data-variant-idx]").forEach((chip) => {
      chip.classList.toggle("is-active", Number(chip.dataset.variantIdx) === clampedIdx);
    });
  }

  // Prominent Now Playing banner update
  const blocked = durSec === 0;
  npCueEl.textContent = `#${entry.id} ${entry.name}`;
  npTypeEl.hidden = false;
  if (entry.variants.length > 1) {
    npTypeEl.textContent = `${variant.label} (${clampedIdx + 1}/${entry.variants.length} · subtype=${variant.subtype})`;
  } else {
    npTypeEl.textContent = `Variación #${step} · subtype=${variant.subtype}`;
  }
  if (blocked) {
    npMetaEl.textContent =
      Number(volSlider.value) <= 0.001
        ? "Silenciado (volumen en 0%)"
        : `Bloqueado por cooldown real (${entry.def.cooldownMs} ms)`;
  } else {
    const panText = panValEl.textContent;
    npMetaEl.textContent = `intensity=${intensity.toFixed(2)} · pan=${panText} · dist=${Math.round(gain * 100)}% · ~${Math.round(durSec * 1000)}ms`;
  }

  nowPlayingEl.classList.remove("is-firing");
  void nowPlayingEl.offsetWidth;
  if (!blocked) {
    nowPlayingEl.classList.add("is-firing");
    window.clearTimeout(nowPlayingPulseTimer);
    nowPlayingPulseTimer = window.setTimeout(() => {
      nowPlayingEl.classList.remove("is-firing");
    }, Math.min(750, Math.max(180, Math.round(durSec * 1000))));
  }
}

function playCueWithCycle(id: AudioCueId): void {
  const entry = byId.get(id);
  if (!entry) return;
  if (selectedCueId !== id) {
    selectCue(id, false);
  }
  const hasMultiple = entry.variants.length > 1;
  const idx = autoCycleToggle.checked && hasMultiple
    ? (nextVariantIdx.get(id) ?? 0)
    : (activeVariantIdx.get(id) ?? 0);
  triggerVariant(entry, idx, {
    advanceNext: autoCycleToggle.checked && hasMultiple,
    respectVariantIntensity: true,
  });
}

function playCueBurst(id: AudioCueId): void {
  const entry = byId.get(id);
  if (!entry) return;
  if (runningBurstCueId === id) {
    stopBurst();
    return;
  }
  stopBurst();
  if (selectedCueId !== id) {
    selectCue(id, false);
  }

  runningBurstCueId = id;
  const cardBtn = catalogEl.querySelector<HTMLButtonElement>(`[data-burst-cue="${id}"]`);
  cardBtn?.classList.add("is-running");
  burstSelectedBtn.classList.add("is-running");
  burstSelectedBtn.textContent = "■ Detener ráfaga";

  const totalSteps = entry.variants.length > 1 ? entry.variants.length : entry.burstCount;
  for (let i = 0; i < totalSteps; i++) {
    const timer = window.setTimeout(() => {
      const vIdx = entry.variants.length > 1 ? i : 0;
      triggerVariant(entry, vIdx, {
        advanceNext: false,
        respectVariantIntensity: true,
      });
      if (i === totalSteps - 1) {
        stopBurst();
      }
    }, i * entry.burstIntervalMs);
    burstTimers.push(timer);
  }
}

// ── Selection & Inspector Sync ────────────────────────────────────────
function selectCue(id: AudioCueId, scrollIntoView = false): void {
  const entry = byId.get(id);
  if (!entry) return;
  selectedCueId = id;
  intensityOverrideActive = false;
  intensitySlider.value = String(entry.defaultIntensity);
  intensityValEl.textContent = entry.defaultIntensity.toFixed(2);

  catalogEl.querySelectorAll<HTMLElement>("[data-cue-card]").forEach((card) => {
    const match = Number(card.dataset.cueCard) === id;
    card.classList.toggle("is-selected", match);
    if (match && scrollIntoView) {
      card.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  });

  syncInspector();
}

function syncInspector(): void {
  const entry = byId.get(selectedCueId);
  if (!entry) return;
  selTitleEl.textContent = entry.name;
  selIdEl.textContent = `#${entry.id}`;
  selDescEl.textContent = entry.description;
  metaRecipeEl.textContent = entry.recipeFile;
  metaGainEl.textContent = entry.def.gain.toFixed(2);
  metaCooldownEl.textContent = `${entry.def.cooldownMs} ms`;
  metaSpatialEl.textContent = `${entry.def.priority} · ${entry.def.spatial ? "3D espacial" : "UI centro"}`;

  const rec = notes[entry.id];
  const verdict: Verdict = rec?.verdict ?? "none";
  verdictRowEl.querySelectorAll<HTMLButtonElement>("[data-verdict]").forEach((btn) => {
    btn.classList.toggle("is-active", btn.dataset.verdict === verdict);
  });
  if (document.activeElement !== noteInput) {
    noteInput.value = rec?.note ?? "";
  }
  syncNotesSummary();
}

function syncNotesSummary(): void {
  const count = annotatedCount();
  notesCountEl.textContent = count === 1 ? "1 nota" : `${count} notas`;
  copyBriefBtn.textContent = count > 0 ? `Copiar brief (${count})` : "Copiar brief para chat";
  copyBriefBtn.disabled = count === 0;
  clearNotesBtn.disabled = count === 0;
}

// ── Filtering & Rendering ─────────────────────────────────────────────
function filteredEntries(): CueCatalogEntry[] {
  const q = searchQuery.trim().toLowerCase();
  return catalog.filter((entry) => {
    if (activeFilter === "noted") {
      if (!hasAnnotation(entry.id)) return false;
    } else if (activeFilter !== "all" && entry.category !== activeFilter) {
      return false;
    }
    if (!q) return true;
    const hay = [
      String(entry.id),
      `#${entry.id}`,
      entry.name,
      entry.category,
      CATEGORY_LABELS[entry.category],
      entry.description,
      entry.recipeFile,
      ...entry.variants.map((v) => v.label),
      notes[entry.id]?.note ?? "",
    ]
      .join(" ")
      .toLowerCase();
    return hay.includes(q);
  });
}

function renderCategoryTabs(): void {
  catTabsEl.innerHTML = "";
  const tabs: { key: CategoryFilter; label: string; count: number }[] = [
    { key: "all", label: "Todos", count: catalog.length },
    ...CATEGORY_ORDER.map((cat) => ({
      key: cat as CategoryFilter,
      label: CATEGORY_LABELS[cat],
      count: catalog.filter((e) => e.category === cat).length,
    })),
    { key: "noted", label: "Con notas", count: annotatedCount() },
  ];

  for (const tab of tabs) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "cat-tab" + (activeFilter === tab.key ? " is-active" : "");
    btn.setAttribute("role", "tab");
    btn.setAttribute("aria-selected", String(activeFilter === tab.key));
    btn.innerHTML = `${escapeHtml(tab.label)}<span class="count">${tab.count}</span>`;
    btn.addEventListener("click", () => {
      activeFilter = tab.key;
      renderCategoryTabs();
      renderCatalog();
    });
    catTabsEl.appendChild(btn);
  }
}

function verdictBadgeMarkup(verdict: Verdict): string {
  if (verdict === "ok") return `<span class="badge badge--ok">✓ OK</span>`;
  if (verdict === "tweak") return `<span class="badge badge--tweak">✎ Ajustar</span>`;
  if (verdict === "remake") return `<span class="badge badge--remake">✕ Rehacer</span>`;
  return "";
}

function renderCatalog(): void {
  const entries = filteredEntries();
  catalogEl.innerHTML = "";
  if (entries.length === 0) {
    catalogEl.innerHTML = `<div class="empty-state">No hay sonidos que coincidan con el filtro actual.</div>`;
    return;
  }

  const grouped = new Map<AudioCategory, CueCatalogEntry[]>();
  for (const entry of entries) {
    let list = grouped.get(entry.category);
    if (!list) {
      list = [];
      grouped.set(entry.category, list);
    }
    list.push(entry);
  }

  for (const cat of CATEGORY_ORDER) {
    const list = grouped.get(cat);
    if (!list || list.length === 0) continue;

    const section = document.createElement("section");
    section.className = "cat-section";

    const heading = document.createElement("h3");
    heading.className = "cat-section__title";
    heading.textContent = `${CATEGORY_LABELS[cat]} (${list.length})`;
    section.appendChild(heading);

    const grid = document.createElement("div");
    grid.className = "cue-grid";

    for (const entry of list) {
      const rec = notes[entry.id];
      const activeIdx = activeVariantIdx.get(entry.id) ?? -1;
      const card = document.createElement("article");
      card.className =
        "cue-card" +
        (entry.id === selectedCueId ? " is-selected" : "") +
        (entry.disabledInGame ? " is-disabled" : "");
      card.dataset.cueCard = String(entry.id);

      const burstLabel =
        entry.variants.length > 1 ? `×${entry.variants.length}` : `×${entry.burstCount}`;

      const badges = [
        entry.def.spatial ? `<span class="badge badge--spatial">3D</span>` : "",
        entry.disabledInGame ? `<span class="badge badge--disabled">Desactivado</span>` : "",
        rec ? verdictBadgeMarkup(rec.verdict) : "",
      ]
        .filter(Boolean)
        .join("");

      const variantsHtml =
        entry.variants.length > 1
          ? `<div class="variant-row">${entry.variants
              .map(
                (v, idx) =>
                  `<button type="button" class="variant-chip${idx === activeIdx ? " is-active" : ""}" data-variant-cue="${entry.id}" data-variant-idx="${idx}" title="subtype=${v.subtype}${v.intensity != null ? `, intensity=${v.intensity}` : ""}">${escapeHtml(v.label)}</button>`,
              )
              .join("")}</div>`
          : "";

      const noteHtml =
        rec && rec.note.trim().length > 0
          ? `<div class="cue-note-preview" title="${escapeHtml(rec.note.trim())}">${escapeHtml(rec.note.trim())}</div>`
          : "";

      card.innerHTML = `
        <div class="cue-card__head">
          <button type="button" class="btn-play" data-play-cue="${entry.id}" title="Reproducir${entry.variants.length > 1 ? " (cicla al siguiente tono)" : " (rota variación)"}">▶</button>
          <button type="button" class="btn-burst${runningBurstCueId === entry.id ? " is-running" : ""}" data-burst-cue="${entry.id}" title="Reproducir secuencia / ráfaga">${burstLabel}</button>
          <div class="cue-card__title">
            <span class="cue-id">#${entry.id}</span>
            <span class="cue-name">${escapeHtml(entry.name)}</span>
          </div>
          <div class="cue-badges">${badges}</div>
        </div>
        <p class="cue-desc">${escapeHtml(entry.description)}</p>
        ${variantsHtml}
        ${noteHtml}
      `;
      grid.appendChild(card);
    }

    section.appendChild(grid);
    catalogEl.appendChild(section);
  }
}

function escapeHtml(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

// ── Feedback Brief Export ─────────────────────────────────────────────
function buildFeedbackBrief(): string {
  const verdictRank: Record<Verdict, number> = { remake: 0, tweak: 1, ok: 2, none: 3 };
  const verdictLabel: Record<Verdict, string> = {
    remake: "Rehacer",
    tweak: "Ajustar",
    ok: "OK",
    none: "Nota",
  };

  const items = catalog
    .filter((e) => hasAnnotation(e.id))
    .sort((a, b) => {
      const va = notes[a.id]?.verdict ?? "none";
      const vb = notes[b.id]?.verdict ?? "none";
      return verdictRank[va] - verdictRank[vb] || a.id - b.id;
    });

  const lines: string[] = ["### Feedback de Audio (Sound Studio)", ""];
  for (const entry of items) {
    const rec = notes[entry.id]!;
    const vIdx = activeVariantIdx.get(entry.id);
    const activeVariant = vIdx != null ? entry.variants[vIdx] : undefined;
    const subText =
      rec.lastLabel != null
        ? `subtype=${rec.lastSubtype ?? 0} ${rec.lastLabel}`
        : activeVariant && entry.variants.length > 1
          ? `subtype=${activeVariant.subtype} ${activeVariant.label}`
          : null;
    const intText =
      rec.lastIntensity != null ? `intensity=${rec.lastIntensity.toFixed(2)}` : null;
    const ctxParts = [subText, intText].filter(Boolean);
    const ctxTag = ctxParts.length > 0 ? ` [${ctxParts.join(" | ")}]` : "";
    const noteText = rec.note.trim();
    lines.push(
      `- **#${entry.id} \`${entry.name}\`${ctxTag}** — \`${verdictLabel[rec.verdict]}\`${noteText ? `: ${noteText}` : ""}`,
    );
  }
  return lines.join("\n");
}

async function copyFeedbackBrief(): Promise<void> {
  const text = buildFeedbackBrief();
  try {
    await navigator.clipboard.writeText(text);
    copyStatusEl.textContent = "Brief copiado al portapapeles. Pégalo directo en el chat.";
  } catch {
    const tmp = document.createElement("textarea");
    tmp.value = text;
    tmp.style.position = "fixed";
    tmp.style.opacity = "0";
    document.body.appendChild(tmp);
    tmp.select();
    document.execCommand("copy");
    tmp.remove();
    copyStatusEl.textContent = "Brief copiado al portapapeles.";
  }
  window.setTimeout(() => {
    copyStatusEl.textContent = "";
  }, 3500);
}

// ── Event Wiring ──────────────────────────────────────────────────────
catalogEl.addEventListener("click", (ev) => {
  const target = ev.target instanceof Element ? ev.target : null;
  if (!target) return;

  const playBtn = target.closest<HTMLButtonElement>("[data-play-cue]");
  if (playBtn) {
    ev.stopPropagation();
    playCueWithCycle(Number(playBtn.dataset.playCue) as AudioCueId);
    return;
  }

  const burstBtn = target.closest<HTMLButtonElement>("[data-burst-cue]");
  if (burstBtn) {
    ev.stopPropagation();
    playCueBurst(Number(burstBtn.dataset.burstCue) as AudioCueId);
    return;
  }

  const variantBtn = target.closest<HTMLButtonElement>("[data-variant-cue]");
  if (variantBtn) {
    ev.stopPropagation();
    const cueId = Number(variantBtn.dataset.variantCue) as AudioCueId;
    const vIdx = Number(variantBtn.dataset.variantIdx);
    const entry = byId.get(cueId);
    if (!entry) return;
    if (selectedCueId !== cueId) selectCue(cueId, false);
    triggerVariant(entry, vIdx, {
      advanceNext: true,
      respectVariantIntensity: true,
    });
    return;
  }

  const card = target.closest<HTMLElement>("[data-cue-card]");
  if (card) {
    selectCue(Number(card.dataset.cueCard) as AudioCueId, false);
  }
});

searchInput.addEventListener("input", () => {
  searchQuery = searchInput.value;
  renderCatalog();
});

volSlider.addEventListener("input", updateSliderReadouts);
intensitySlider.addEventListener("input", () => {
  intensityOverrideActive = true;
  updateSliderReadouts();
});
panSlider.addEventListener("input", updateSliderReadouts);
distSlider.addEventListener("input", updateSliderReadouts);

playSelectedBtn.addEventListener("click", () => playCueWithCycle(selectedCueId));
burstSelectedBtn.addEventListener("click", () => playCueBurst(selectedCueId));
resetControlsBtn.addEventListener("click", () => {
  const entry = byId.get(selectedCueId);
  intensityOverrideActive = false;
  intensitySlider.value = String(entry?.defaultIntensity ?? 0.7);
  panSlider.value = "0";
  distSlider.value = "1";
  lockSeedToggle.checked = false;
  realCooldownToggle.checked = false;
  updateSliderReadouts();
});

verdictRowEl.addEventListener("click", (ev) => {
  const btn = ev.target instanceof Element ? ev.target.closest<HTMLButtonElement>("[data-verdict]") : null;
  if (!btn) return;
  const verdict = (btn.dataset.verdict as Verdict) || "none";
  const rec = ensureNoteRecord(selectedCueId);
  rec.verdict = verdict;
  if (rec.verdict === "none" && rec.note.trim().length === 0) {
    delete notes[selectedCueId];
  }
  saveNotes();
  syncInspector();
  renderCategoryTabs();
  renderCatalog();
});

noteInput.addEventListener("input", () => {
  const text = noteInput.value;
  const rec = ensureNoteRecord(selectedCueId);
  rec.note = text;
  if (rec.verdict === "none" && rec.note.trim().length === 0) {
    delete notes[selectedCueId];
  }
  saveNotes();
  syncNotesSummary();
  renderCategoryTabs();
  // Lightweight inline card preview update
  const card = catalogEl.querySelector<HTMLElement>(`[data-cue-card="${selectedCueId}"]`);
  if (card) {
    let preview = card.querySelector<HTMLElement>(".cue-note-preview");
    const trimmed = text.trim();
    if (trimmed) {
      if (!preview) {
        preview = document.createElement("div");
        preview.className = "cue-note-preview";
        card.appendChild(preview);
      }
      preview.textContent = trimmed;
      preview.title = trimmed;
    } else if (preview) {
      preview.remove();
    }
  }
});

copyBriefBtn.addEventListener("click", () => {
  void copyFeedbackBrief();
});

clearNotesBtn.addEventListener("click", () => {
  if (!window.confirm("¿Borrar todas las notas y veredictos guardados en Sound Studio?")) return;
  notes = {};
  saveNotes();
  syncInspector();
  renderCategoryTabs();
  renderCatalog();
});

// Keyboard shortcuts (ignored while typing in search or notes)
window.addEventListener("keydown", (ev) => {
  const tag = (ev.target as HTMLElement | null)?.tagName;
  const editingText = tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";

  if (ev.key === "/" && !editingText) {
    ev.preventDefault();
    searchInput.focus();
    searchInput.select();
    return;
  }
  if (ev.key === "Escape" && editingText) {
    (ev.target as HTMLElement).blur();
    return;
  }
  if (editingText || ev.metaKey || ev.ctrlKey || ev.altKey) return;

  const visible = filteredEntries();
  if (visible.length === 0) return;

  if (ev.code === "Space") {
    ev.preventDefault();
    playCueWithCycle(selectedCueId);
    return;
  }
  if (ev.key === "r" || ev.key === "R") {
    ev.preventDefault();
    playCueBurst(selectedCueId);
    return;
  }
  if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
    ev.preventDefault();
    const cur = visible.findIndex((e) => e.id === selectedCueId);
    const delta = ev.key === "ArrowDown" ? 1 : -1;
    const next = cur < 0 ? 0 : (cur + delta + visible.length) % visible.length;
    selectCue(visible[next].id, true);
    return;
  }
  if (ev.key === "ArrowLeft" || ev.key === "ArrowRight") {
    const entry = byId.get(selectedCueId);
    if (!entry || entry.variants.length <= 1) return;
    ev.preventDefault();
    const cur = activeVariantIdx.get(entry.id) ?? 0;
    const delta = ev.key === "ArrowRight" ? 1 : -1;
    const next = (cur + delta + entry.variants.length) % entry.variants.length;
    triggerVariant(entry, next, {
      advanceNext: true,
      respectVariantIntensity: true,
    });
  }
});

// ── Boot ──────────────────────────────────────────────────────────────
updateSliderReadouts();
renderCategoryTabs();
renderCatalog();
selectCue(selectedCueId, false);
drawVisualizerIdle();
