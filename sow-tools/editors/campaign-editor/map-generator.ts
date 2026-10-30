(function () {
  "use strict";
  type Rect = [number, number, number, number];

  const namePattern = /^[a-z][a-z0-9_]{0,63}$/;

  const $ = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T,
    canvas = $<HTMLCanvasElement>("map"),
    ctx = canvas.getContext("2d") as CanvasRenderingContext2D,
    nameInput = $<HTMLInputElement>("name"),
    sizeInput = $<HTMLSelectElement>("size"),
    frequencyInput = $<HTMLInputElement>("frequency"),
    cropLabel = $("crop-label"),
    generateButton = $<HTMLButtonElement>("generate"),
    uploadButton = $<HTMLButtonElement>("upload"),
    statusLine = $("status"),
    smoothCoastsInput = $<HTMLInputElement>("smooth-coasts"),
    waterLevelInput = $<HTMLInputElement>("water-level"),
    waterLevelValue = $("water-level-value"),
    connectDiagonalsInput = $<HTMLInputElement>("connect-diagonals"),
    minLakeInput = $<HTMLInputElement>("min-lake"),
    minIslandInput = $<HTMLInputElement>("min-island"),
    worldTab = $<HTMLButtonElement>("tab-world"),
    gameTab = $<HTMLButtonElement>("tab-game"),
    worldView = $("view-world"),
    gameView = $("view-game"),
    gameFrame = $<HTMLIFrameElement>("game"),
    // The editor serves the local game build on the next port (see server.mts handlePreview).
    previewOrigin = `${location.protocol}//${location.hostname}:${Number(location.port || 80) + 1}`,
    mapTools = $("map-tools"),
    cursorLabel = $("cursor"),
    image = new Image();

  let crop: Rect | null = null;
  let pointerMode: "draw" | "pan" | null = null;
  let pointerStart: number[] = [0, 0];
  let lastPointer: number[] = [0, 0];
  let drawStarted = false;
  let view = { scale: 1, x: 0, y: 0 };
  let fitScale = 0;
  let minScale = 0;
  let viewWidth = 0;
  let viewHeight = 0;
  let dpr = 1;
  let generatedName = "";
  let generating = false;
  let regeneratePending = false;
  let regenerateTimer = 0;

  function showView(game: boolean) {
    worldView.hidden = game;
    gameView.hidden = !game;
    mapTools.hidden = game;
    worldTab.setAttribute("aria-selected", String(!game));
    gameTab.setAttribute("aria-selected", String(game));
  }

  function reloadGameView(name: string) {
    gameFrame.src = `${previewOrigin}/play/?sow_preview_map=${encodeURIComponent(name)}&v=${Date.now()}`;
    gameTab.disabled = false;
    showView(true);
  }

  worldTab.addEventListener("click", () => showView(false));
  gameTab.addEventListener("click", () => showView(true));

  function setStatus(text: string, error = false) {
    statusLine.textContent = text;
    statusLine.className = error ? "status error" : "status";
  }

  function canvasPoint(event: MouseEvent) {
    const bounds = canvas.getBoundingClientRect();
    return [event.clientX - bounds.left, event.clientY - bounds.top];
  }

  function draw() {
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, viewWidth, viewHeight);
    if (!image.naturalWidth) return;
    // Sharp source pixels when zoomed in; thin rivers stay visible when zoomed out.
    ctx.imageSmoothingEnabled = view.scale * dpr < 1;
    ctx.drawImage(image, view.x, view.y, image.naturalWidth * view.scale, image.naturalHeight * view.scale);
    if (!crop) return;
    const [x, y] = SOWCampaignMapPreview.mapToScreen(crop, view);
    ctx.fillStyle = "rgba(233, 187, 100, 0.25)";
    ctx.strokeStyle = "#e9bb64";
    ctx.lineWidth = 2;
    ctx.fillRect(x, y, crop[2] * view.scale, crop[3] * view.scale);
    ctx.strokeRect(x, y, crop[2] * view.scale, crop[3] * view.scale);
  }

  function applyView(next: { scale: number; x: number; y: number }) {
    view = SOWCampaignMapPreview.clampMapView(
      next,
      image.naturalWidth,
      image.naturalHeight,
      viewWidth,
      viewHeight,
      60,
    );
    draw();
  }

  function fit() {
    const fitted = SOWCampaignMapPreview.fitMapView(
      image.naturalWidth,
      image.naturalHeight,
      viewWidth,
      viewHeight,
    );
    if (!fitted) return;
    fitScale = fitted.fitScale;
    minScale = fitted.minScale;
    applyView(fitted);
  }

  // The canvas has no size while the in-game tab is open; coming back keeps the zoom.
  function resize() {
    const bounds = canvas.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const size = SOWCampaignMapPreview.resizeCanvas(canvas, Math.min(window.devicePixelRatio || 1, 2));
    viewWidth = size.width;
    viewHeight = size.height;
    dpr = size.dpr;
    if (fitScale) applyView(view);
    else fit();
  }

  function zoomAt(point: number[], factor: number) {
    if (!fitScale) return;
    applyView(SOWCampaignMapPreview.zoomMapView(view, point, factor, minScale, fitScale * 64));
  }

  function selectCrop(next: Rect | null) {
    crop = next;
    generatedName = "";
    uploadButton.disabled = true;
    generateButton.disabled = crop === null;
    if (crop) {
      const [x, y, width, height] = crop;
      cropLabel.textContent = `Area: x ${x}, y ${y}, ${width} × ${height} source pixels`;
    } else {
      cropLabel.textContent = "No area selected.";
    }
    draw();
  }

  // Whole source pixels inside the image, whatever the zoom.
  function cropBetween(from: number[], to: number[]): Rect | null {
    const [fromX, fromY] = SOWCampaignMapPreview.screenToMap(from, view);
    const [toX, toY] = SOWCampaignMapPreview.screenToMap(to, view);
    const column = (value: number) => Math.round(Math.max(0, Math.min(image.naturalWidth, value)));
    const row = (value: number) => Math.round(Math.max(0, Math.min(image.naturalHeight, value)));
    const left = Math.min(column(fromX), column(toX));
    const top = Math.min(row(fromY), row(toY));
    const width = Math.abs(column(toX) - column(fromX));
    const height = Math.abs(row(toY) - row(fromY));
    return width > 0 && height > 0 ? [left, top, width, height] : null;
  }

  function showCursor(point: number[]) {
    if (!fitScale) return;
    const [x, y] = SOWCampaignMapPreview.screenToMap(point, view);
    const inside = x >= 0 && y >= 0 && x < image.naturalWidth && y < image.naturalHeight;
    const position = inside ? `x ${Math.floor(x)}  y ${Math.floor(y)}  ·  ` : "";
    cursorLabel.textContent = `${position}${Math.round((view.scale / fitScale) * 100)}%`;
  }

  canvas.addEventListener("pointerdown", (event) => {
    if (pointerMode || !fitScale) return;
    canvas.setPointerCapture(event.pointerId);
    pointerStart = canvasPoint(event);
    lastPointer = pointerStart;
    drawStarted = false;
    pointerMode = event.button === 0 ? "draw" : "pan";
    canvas.classList.toggle("panning", pointerMode === "pan");
  });

  canvas.addEventListener("pointermove", (event) => {
    const point = canvasPoint(event);
    if (pointerMode === "pan") {
      applyView(SOWCampaignMapPreview.panMapView(view, point[0] - lastPointer[0], point[1] - lastPointer[1]));
    } else if (pointerMode === "draw") {
      // A plain click keeps the current area; only a real drag replaces it.
      drawStarted ||= Math.hypot(point[0] - pointerStart[0], point[1] - pointerStart[1]) > 3;
      if (drawStarted) selectCrop(cropBetween(pointerStart, point));
    }
    lastPointer = point;
    showCursor(point);
  });

  function endPointer() {
    pointerMode = null;
    canvas.classList.remove("panning");
  }
  canvas.addEventListener("pointerup", endPointer);
  canvas.addEventListener("pointercancel", endPointer);
  canvas.addEventListener("contextmenu", (event) => event.preventDefault());
  canvas.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      const point = canvasPoint(event);
      zoomAt(point, Math.exp(-event.deltaY * 0.0015));
      showCursor(point);
    },
    { passive: false },
  );
  $("zoom-in").addEventListener("click", () => zoomAt([viewWidth / 2, viewHeight / 2], 1.3));
  $("zoom-out").addEventListener("click", () => zoomAt([viewWidth / 2, viewHeight / 2], 1 / 1.3));
  $("fit").addEventListener("click", fit);

  function requestBody(force: boolean, draft: boolean) {
    return JSON.stringify({
      draft,
      crop: crop || [],
      size: sizeInput.value,
      frequency: Number(frequencyInput.value),
      water: {
        min_lake_tiles: Number(minLakeInput.value),
        min_island_pixels: Number(minIslandInput.value),
        smooth_coasts: smoothCoastsInput.checked,
        water_level: Number(waterLevelInput.value),
        connect_diagonals: connectDiagonalsInput.checked,
      },
      force,
    });
  }

  function showWaterLevel() {
    waterLevelValue.textContent = Number(waterLevelInput.value).toFixed(2);
    waterLevelInput.disabled = !smoothCoastsInput.checked;
  }

  async function post(path: string, body: string) {
    return fetch(path, { method: "POST", headers: { "Content-Type": "application/json" }, body });
  }

  function validMapName(): string | null {
    const name = nameInput.value.trim();
    if (!namePattern.test(name)) {
      setStatus("Map name must start with a letter and use only lowercase letters, digits and _.", true);
      return null;
    }
    return name;
  }

  // One generation at a time; a change made meanwhile regenerates once more with the latest settings.
  // A draft (live change) skips the final compression; Generate and Upload always finish the map.
  async function generate(name: string, force: boolean, draft: boolean): Promise<boolean> {
    generating = true;
    generateButton.disabled = true;
    setStatus(draft ? "Updating the in-game view…" : "Generating…");
    try {
      let response = await post(`/__map-generate?map=${encodeURIComponent(name)}`, requestBody(force, draft));
      if (response.status === 409 && !force && confirm(`${await response.text()}\nReplace it?`)) {
        response = await post(`/__map-generate?map=${encodeURIComponent(name)}`, requestBody(true, draft));
      }
      const text = await response.text();
      if (!response.ok) {
        setStatus(text || "Could not generate the map.", true);
        return false;
      }
      const { message } = JSON.parse(text);
      generatedName = name;
      uploadButton.disabled = false;
      setStatus(`Saved assets/maps/${name} on this computer${draft ? " (draft)" : ""}.\n${message}`);
      reloadGameView(name);
      return true;
    } catch (error) {
      setStatus(String(error), true);
      return false;
    } finally {
      generating = false;
      generateButton.disabled = crop === null;
      if (regeneratePending) {
        regeneratePending = false;
        if (generatedName) generate(generatedName, true, true);
      }
    }
  }

  function scheduleRegenerate() {
    if (!generatedName || !crop) return;
    clearTimeout(regenerateTimer);
    regenerateTimer = window.setTimeout(() => {
      if (generating) regeneratePending = true;
      else generate(generatedName, true, true);
    }, 300);
  }

  generateButton.addEventListener("click", () => {
    const name = validMapName();
    if (!name || !crop || generating) return;
    generate(name, false, false);
  });

  for (const input of [smoothCoastsInput, waterLevelInput, connectDiagonalsInput, minLakeInput, minIslandInput, sizeInput]) {
    input.addEventListener("change", () => {
      showWaterLevel();
      scheduleRegenerate();
    });
  }
  waterLevelInput.addEventListener("input", showWaterLevel);
  showWaterLevel();

  uploadButton.addEventListener("click", async () => {
    if (!generatedName || generating) return;
    uploadButton.disabled = true;
    if (!(await generate(generatedName, true, false))) {
      uploadButton.disabled = false;
      return;
    }
    setStatus("Uploading to the server…");
    try {
      const response = await post(`/__map-upload?map=${encodeURIComponent(generatedName)}`, "");
      const text = await response.text();
      if (!response.ok) {
        setStatus(text || "Upload failed.", true);
        uploadButton.disabled = false;
        return;
      }
      setStatus(`Uploaded ${generatedName}. The server reloaded its map list.\n${text}`);
    } catch (error) {
      setStatus(String(error), true);
      uploadButton.disabled = false;
    }
  });

  image.addEventListener("load", () => {
    new ResizeObserver(resize).observe(canvas);
    resize();
  });
  image.addEventListener("error", () => setStatus("Could not load the world map reference.", true));
  image.src = "/assets/map_sources/giantworldmap.png";
})();
