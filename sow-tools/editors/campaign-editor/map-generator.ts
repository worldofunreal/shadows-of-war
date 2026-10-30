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
    image = new Image();

  let crop: Rect | null = null;
  let dragStart: { x: number; y: number } | null = null;
  let generatedName = "";
  let generating = false;
  let regeneratePending = false;
  let regenerateTimer = 0;

  function showView(game: boolean) {
    worldView.hidden = game;
    gameView.hidden = !game;
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

  function pointIn(event: PointerEvent) {
    const bounds = canvas.getBoundingClientRect();
    const scale = canvas.width / bounds.width;
    return {
      x: Math.min(canvas.width, Math.max(0, (event.clientX - bounds.left) * scale)),
      y: Math.min(canvas.height, Math.max(0, (event.clientY - bounds.top) * scale)),
    };
  }

  function draw() {
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(image, 0, 0);
    if (!crop) return;
    const [x, y, width, height] = crop;
    ctx.fillStyle = "rgba(233, 187, 100, 0.25)";
    ctx.strokeStyle = "#e9bb64";
    ctx.lineWidth = Math.max(2, canvas.width / 400);
    ctx.fillRect(x, y, width, height);
    ctx.strokeRect(x, y, width, height);
  }

  function selectCrop(next: Rect | null) {
    crop = next;
    generatedName = "";
    uploadButton.disabled = true;
    generateButton.disabled = crop === null;
    if (crop) {
      const [x, y, width, height] = crop;
      cropLabel.textContent = `Area: x ${Math.round(x)}, y ${Math.round(y)}, ${Math.round(width)} × ${Math.round(height)} source pixels`;
    } else {
      cropLabel.textContent = "No area selected.";
    }
    draw();
  }

  canvas.addEventListener("pointerdown", (event) => {
    canvas.setPointerCapture(event.pointerId);
    dragStart = pointIn(event);
    selectCrop([dragStart.x, dragStart.y, 0, 0]);
  });

  canvas.addEventListener("pointermove", (event) => {
    if (!dragStart) return;
    const point = pointIn(event);
    selectCrop([
      Math.min(dragStart.x, point.x),
      Math.min(dragStart.y, point.y),
      Math.abs(point.x - dragStart.x),
      Math.abs(point.y - dragStart.y),
    ]);
  });

  canvas.addEventListener("pointerup", () => {
    dragStart = null;
    if (crop && (crop[2] < 1 || crop[3] < 1)) selectCrop(null);
  });

  function requestBody(force: boolean, draft: boolean) {
    return JSON.stringify({
      draft,
      crop: (crop || []).map((value) => Math.round(value)),
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
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    draw();
  });
  image.addEventListener("error", () => setStatus("Could not load the world map reference.", true));
  image.src = "/assets/map_sources/giantworldmap.png";
})();
