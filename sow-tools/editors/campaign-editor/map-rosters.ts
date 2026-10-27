(function () {
  "use strict";
  type Axis = "x" | "y";
  type RosterMap = Awaited<ReturnType<typeof SOWCampaignMapPreview.load>>;
  type RosterPreset = RosterMap["rosters"][number];
  type RosterEntry = RosterPreset["entries"][number];
  type WarningKind = "unknown" | "duplicate" | "water";
  type EntityPicker = ReturnType<AtlasEntityPickerApi["attach"]>;
  interface RosterCatalog {
    entities: AtlasEntity[];
  }
  interface RosterWarning {
    presetId: string;
    kind: WarningKind;
    text: string;
  }
  interface ViewState {
    scale: number;
    x: number;
    y: number;
  }
  interface TilePoint {
    x: number;
    y: number;
  }
  const $ = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T,
    canvas = $<HTMLCanvasElement>("map"),
    ctx = canvas.getContext("2d");
  let mapIds: string[] = [],
    catalog: RosterCatalog | null = null,
    map: RosterMap | null = null,
    mapId = "",
    etag: string | null = null,
    presetId: string | null = null,
    selectedId: string | null = null,
    selectedAnchor: number | null = null,
    pendingId: string | null = null,
    dirty = false,
    saving = false,
    entityPicker: EntityPicker | null = null;
  let scale = 1,
    fitScale = 1,
    minScale = 0.5,
    maxScale = 128,
    ox = 0,
    oy = 0,
    viewWidth = 0,
    viewHeight = 0,
    dpr = 1,
    pointer: number[] | null = null,
    pointerStart: number[] | null = null,
    pointerMode = "",
    pointerMoved = false;
  function preset(): RosterPreset | null {
    return (map && map.rosters.find((item) => item.id === presetId)) || null;
  }
  function entity(id: string): AtlasEntity | null {
    return (catalog && catalog.entities.find((item) => item.id === id)) || null;
  }
  function entry(id: string | null): RosterEntry | null {
    return preset()?.entries.find((item) => item.entity_id === id) || null;
  }
  function markDirty(): void {
    dirty = true;
    $("save-status").textContent = "Unsaved";
  }
  function options<T>(
    select: HTMLSelectElement,
    items: T[],
    value: (item: T) => string,
    label: (item: T) => string,
  ): void {
    select.replaceChildren();
    for (const item of items) {
      const option = document.createElement("option");
      option.value = value(item);
      option.textContent = label(item);
      select.append(option);
    }
  }
  function viewState(): ViewState {
    return { scale, x: ox, y: oy };
  }
  function applyView(view: ViewState): void {
    scale = view.scale;
    ox = view.x;
    oy = view.y;
  }
  function fit(): void {
    if (!map) return;
    const view = SOWCampaignMapPreview.fitMapView(map.width, map.height, viewWidth, viewHeight);
    if (!view) return;
    fitScale = view.fitScale;
    minScale = view.minScale;
    maxScale = fitScale * 128;
    applyView(view);
    draw();
  }
  function resize(): void {
    const size = SOWCampaignMapPreview.resizeCanvas(canvas, Math.min(window.devicePixelRatio || 1, 2));
    viewWidth = size.width;
    viewHeight = size.height;
    dpr = size.dpr;
    if (map) fit();
  }
  function screenPoint(event: MouseEvent): number[] {
    const rect = canvas.getBoundingClientRect();
    return [event.clientX - rect.left, event.clientY - rect.top];
  }
  function tileAt(point: number[]): TilePoint | null {
    if (!map) return null;
    const [x, y] = SOWCampaignMapPreview.screenToMap(point, viewState());
    return x >= 0 && y >= 0 && x < map.width && y < map.height
      ? { x: Math.floor(x), y: Math.floor(y) }
      : null;
  }
  function focusEntry(item: TilePoint | null): void {
    if (!item || !map) return;
    if (scale < fitScale * 2) scale = fitScale * 2;
    const offset = SOWCampaignMapPreview.centerMapOffsets(
      { x: item.x + 0.5, y: item.y + 0.5 },
      scale,
      viewWidth,
      viewHeight,
    );
    ox = offset.x;
    oy = offset.y;
    draw();
  }
  function zoomAt(point: number[], factor: number): void {
    if (!map) return;
    applyView(SOWCampaignMapPreview.zoomMapView(viewState(), point, factor, minScale, maxScale));
    draw();
  }
  function warnSummary(): RosterWarning[] {
    if (!map) return [];
    const warnings: RosterWarning[] = [];
    for (const current of map.rosters || []) {
      const tiles = new Set<string>();
      for (const item of current.entries) {
        if (!entity(item.entity_id))
          warnings.push({
            presetId: current.id,
            kind: "unknown",
            text: `Unknown Atlas ID: ${item.entity_id}`,
          });
        const key = item.x + "," + item.y;
        if (tiles.has(key))
          warnings.push({ presetId: current.id, kind: "duplicate", text: `Duplicate tile: ${key}` });
        tiles.add(key);
        if (!SOWCampaignMapPreview.isLand(map.terrain, map.width, map.height, item.x, item.y))
          warnings.push({
            presetId: current.id,
            kind: "water",
            text: `Water tile: ${item.entity_id} (${item.x}, ${item.y})`,
          });
      }
    }
    return warnings;
  }
  function draw(): void {
    if (!map || !map.image) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, canvas.width / dpr, canvas.height / dpr);
    ctx.fillStyle = "#142326";
    ctx.fillRect(0, 0, canvas.width / dpr, canvas.height / dpr);
    ctx.imageSmoothingEnabled = false;
    const [mapX, mapY] = SOWCampaignMapPreview.mapToScreen([0, 0], viewState());
    ctx.drawImage(map.image, mapX, mapY, map.width * scale, map.height * scale);
    const current = preset(),
      linked = new Set(
        (current?.entries || []).map((item) => item.legacy_anchor).filter((index) => index != null),
      );
    for (let i = 0; i < (map.spawns || []).length; i++) {
      if (linked.has(i)) continue;
      const a = map.spawns[i],
        [x, y] = SOWCampaignMapPreview.mapToScreen([a.x + 0.5, a.y + 0.5], viewState());
      if (x < -30 || y < -30 || x > viewWidth + 30 || y > viewHeight + 30) continue;
      ctx.beginPath();
      ctx.arc(x, y, Math.max(3, Math.min(7, scale * 0.28)), 0, Math.PI * 2);
      ctx.strokeStyle = i === selectedAnchor ? "#fff" : "#d9e2dc88";
      ctx.lineWidth = i === selectedAnchor ? 2 : 1.5;
      ctx.stroke();
      if ($<HTMLInputElement>("labels").checked)
        SOWCampaignMapPreview.drawMapLabel(ctx, a.name, x, y - 7, i === selectedAnchor);
    }
    for (const item of current?.entries || []) {
      const [x, y] = SOWCampaignMapPreview.mapToScreen([item.x + 0.5, item.y + 0.5], viewState());
      if (x < -50 || y < -50 || x > viewWidth + 50 || y > viewHeight + 50) continue;
      const selected = item.entity_id === selectedId,
        r = Math.max(3, Math.min(selected ? 8 : 5, scale * 0.34)),
        e = entity(item.entity_id);
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fillStyle = e?.territory_color || (item.role === "tribe" ? "#e9bb64" : "#70b6cf");
      ctx.fill();
      if (selected || !SOWCampaignMapPreview.isLand(map.terrain, map.width, map.height, item.x, item.y)) {
        ctx.beginPath();
        ctx.arc(x, y, r + 3, 0, Math.PI * 2);
        ctx.strokeStyle = selected ? "#fff" : "#ff4d4d";
        ctx.lineWidth = 2;
        ctx.stroke();
      }
      if ($<HTMLInputElement>("labels").checked || selected)
        SOWCampaignMapPreview.drawMapLabel(ctx, e?.name || item.entity_id, x, y - r - 4, selected);
    }
    if (pendingId) {
      ctx.fillStyle = "#f1cc7b";
      ctx.font = "12px system-ui";
      ctx.textAlign = "left";
      ctx.fillText(`Click a tile to place ${entity(pendingId)?.name || pendingId}`, 12, viewHeight - 20);
    }
    const warnings = warnSummary().filter((value) => value.presetId === presetId),
      water = warnings.filter((value) => value.kind === "water").length,
      dupes = warnings.filter((value) => value.kind === "duplicate").length;
    $("map-status").textContent =
      `${map.displayName} · ${map.width}×${map.height} · ${current?.entries.length || 0} roster entities · ${water} on water · ${dupes} duplicate tiles`;
  }
  function updateEntityControls(): void {
    const selected = entityPicker?.getSelected();
    $<HTMLButtonElement>("add-entity").disabled = !preset() || !selected || !!pendingId;
    if (!preset())
      $("entity-flow-hint").textContent =
        "Create a roster preset first; your selected Atlas entity will stay selected.";
    else if (pendingId)
      $("entity-flow-hint").textContent =
        `Click a map tile to place ${entity(pendingId)?.name || pendingId}.`;
    else if (selected)
      $("entity-flow-hint").textContent = `Selected ${selected.name}. Add it, then click its map tile.`;
    else $("entity-flow-hint").textContent = "Search by name or ID and choose a result.";
  }
  function renderPresets(): void {
    if (!map) return;
    const rosters = map.rosters || [];
    options(
      $<HTMLSelectElement>("preset-select"),
      rosters,
      (p) => p.id,
      (p) => p.name || p.id,
    );
    if (!rosters.some((p) => p.id === presetId)) presetId = rosters[0]?.id || null;
    $<HTMLSelectElement>("preset-select").value = presetId || "";
    $<HTMLSelectElement>("preset-select").disabled = !rosters.length;
    $<HTMLButtonElement>("delete-preset").disabled = !rosters.length;
    const defaults = rosters.map((p) => ({ id: p.id, name: p.name || p.id }));
    options(
      $<HTMLSelectElement>("default-select"),
      defaults,
      (p) => p.id,
      (p) => p.name,
    );
    $<HTMLSelectElement>("default-select").disabled = !rosters.length;
    $<HTMLSelectElement>("default-select").value = map.defaultRoster || presetId || "";
    $<HTMLInputElement>("preset-name").value = preset()?.name || "";
    renderList();
    renderDetail();
    renderAnchors();
    draw();
    updateEntityControls();
  }
  function renderList(): void {
    const host = $("entity-list"),
      current = preset();
    host.replaceChildren();
    for (const item of current?.entries || []) {
      const e = entity(item.entity_id),
        button = document.createElement("button");
      button.className = "entity" + (item.entity_id === selectedId ? " selected" : "");
      button.setAttribute("role", "option");
      button.setAttribute("aria-selected", String(item.entity_id === selectedId));
      if (e?.avatar) {
        const img = document.createElement("img");
        img.src = "/assets/gameplay/avatars/" + encodeURIComponent(e.avatar) + ".webp";
        img.alt = "";
        button.append(img);
      }
      const name = document.createElement("span");
      name.textContent = e?.name || item.entity_id;
      button.append(name);
      const meta = document.createElement("span");
      const land = SOWCampaignMapPreview.isLand(map.terrain, map.width, map.height, item.x, item.y);
      meta.className = "meta" + (!land ? " warn" : "");
      meta.textContent = `${item.role} · ${item.x}, ${item.y}${land ? "" : " · water"}`;
      button.append(meta);
      button.onclick = () => {
        selectedId = item.entity_id;
        pendingId = null;
        renderPresets();
        focusEntry(item);
      };
      host.append(button);
    }
    if (!current?.entries.length) {
      const empty = document.createElement("div");
      empty.className = "empty";
      empty.textContent = "No entities in this preset yet.";
      host.append(empty);
    }
  }
  function renderDetail(): void {
    const host = $("detail"),
      item = entry(selectedId),
      e = entity(selectedId);
    host.replaceChildren();
    const title = document.createElement("h2");
    title.textContent = "Selected entity";
    host.append(title);
    if (!item) {
      const hint = document.createElement("div");
      hint.className = "hint";
      hint.textContent = "Select an entity from this roster or its map marker.";
      host.append(hint);
      return;
    }
    if (e?.avatar) {
      const img = document.createElement("img");
      img.className = "avatar";
      img.src = "/assets/gameplay/avatars/" + encodeURIComponent(e.avatar) + ".webp";
      img.alt = "";
      host.append(img);
    }
    const name = document.createElement("strong");
    name.textContent = e?.name || item.entity_id;
    host.append(name);
    const id = document.createElement("div");
    id.className = "hint";
    id.textContent = item.entity_id;
    host.append(id);
    const row = document.createElement("div");
    row.className = "row";
    const role = document.createElement("select");
    role.setAttribute("aria-label", "Game role");
    for (const [v, label] of [
      ["nation", "Nation"],
      ["tribe", "Tribe"],
    ]) {
      const o = document.createElement("option");
      o.value = v;
      o.textContent = label;
      role.append(o);
    }
    role.value = item.role;
    role.onchange = () => {
      item.role = role.value as RosterEntry["role"];
      markDirty();
      renderPresets();
    };
    const coords = document.createElement("div");
    coords.className = "row";
    for (const [axis, max] of [
      ["x", map.width - 1],
      ["y", map.height - 1],
    ] as [Axis, number][]) {
      const input = document.createElement("input");
      input.type = "number";
      input.min = "0";
      input.max = String(max);
      input.step = "1";
      input.value = String(item[axis]);
      input.setAttribute("aria-label", axis + " tile");
      input.onchange = () => {
        const v = Number(input.value);
        if (!Number.isInteger(v) || v < 0 || v > max) {
          input.value = String(item[axis]);
          return;
        }
        item[axis] = v;
        markDirty();
        renderPresets();
      };
      coords.append(input);
    }
    row.append(role, coords);
    host.append(row);
    const actions = document.createElement("div");
    actions.className = "tools";
    const move = document.createElement("button");
    move.textContent = "Place on map";
    move.onclick = () => {
      pendingId = item.entity_id;
      draw();
    };
    const remove = document.createElement("button");
    remove.textContent = "Remove";
    remove.onclick = () => {
      const current = preset();
      current.entries = current.entries.filter((candidate) => candidate.entity_id !== item.entity_id);
      selectedId = null;
      pendingId = null;
      markDirty();
      renderPresets();
    };
    actions.append(move, remove);
    host.append(actions);
    if (item.legacy_anchor != null) {
      const link = document.createElement("div");
      link.className = "hint";
      link.textContent = `Linked to unchanged map anchor #${item.legacy_anchor}: ${map.spawns[item.legacy_anchor]?.name || ""}`;
      host.append(link);
    }
  }
  function renderAnchors(): void {
    const host = $("anchor-list"),
      current = preset(),
      linked = new Set((current?.entries || []).map((item) => item.legacy_anchor).filter((i) => i != null)),
      anchors = (map.spawns || [])
        .map((a, index) => ({ a, index }))
        .filter((item) => !linked.has(item.index));
    host.replaceChildren();
    $("anchor-count").textContent =
      `${anchors.length} of ${(map.spawns || []).length} anchors are not linked in this preset.`;
    for (const { a, index } of anchors) {
      const row = document.createElement("div");
      row.className = "anchor";
      row.style.background = selectedAnchor === index ? "#e9bb6417" : "";
      const text = document.createElement("span");
      text.textContent = `#${index} · ${a.name} · ${a.x}, ${a.y}`;
      row.append(text);
      const button = document.createElement("button");
      button.textContent = "Link selected Nation";
      button.disabled = !entry(selectedId);
      button.onclick = (event) => {
        event.stopPropagation();
        const item = entry(selectedId);
        if (!item) return;
        item.role = "nation";
        item.x = a.x;
        item.y = a.y;
        item.legacy_anchor = index;
        selectedAnchor = index;
        markDirty();
        pendingId = null;
        renderPresets();
        focusEntry(item);
      };
      row.append(button);
      row.onclick = () => {
        selectedAnchor = index;
        const p = { x: a.x, y: a.y };
        focusEntry(p);
        renderAnchors();
      };
      host.append(row);
    }
    if (!anchors.length) {
      const empty = document.createElement("div");
      empty.className = "hint";
      empty.textContent = "All map anchors are linked in this preset.";
      host.append(empty);
    }
  }
  function selectEntity(id: string): void {
    selectedId = id;
    pendingId = null;
    entityPicker?.clear();
    renderPresets();
    focusEntry(entry(id));
  }
  function markerAt(point: number[]): RosterEntry | null {
    const current = preset();
    let best: RosterEntry | null = null,
      distance = 16;
    for (const item of current?.entries || []) {
      const [x, y] = SOWCampaignMapPreview.mapToScreen([item.x + 0.5, item.y + 0.5], viewState()),
        d = Math.hypot(x - point[0], y - point[1]);
      if (d < distance) {
        best = item;
        distance = d;
      }
    }
    return best;
  }
  function placeAt(id: string | null, point: number[], refresh = true): boolean {
    const tile = tileAt(point);
    if (!tile) return false;
    const item = entry(id);
    if (!item) return false;
    item.x = tile.x;
    item.y = tile.y;
    selectedId = id;
    pendingId = null;
    markDirty();
    if (refresh) renderPresets();
    else draw();
    return true;
  }
  function newPreset(): void {
    if (!map) return;
    const name = prompt("Preset name (for example Historical or Modern)");
    if (!name || !name.trim()) return;
    const idBase =
      name
        .normalize("NFD")
        .replace(/[\u0300-\u036f]/g, "")
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "_")
        .replace(/^_+|_+$/g, "") || "roster";
    let id = idBase,
      n = 2;
    while (map.rosters.some((item) => item.id === id)) id = idBase + "_" + n++;
    const item: RosterPreset = { id, name: name.trim(), entries: [] };
    map.rosters.push(item);
    presetId = id;
    if (!map.defaultRoster) map.defaultRoster = id;
    selectedId = null;
    markDirty();
    renderPresets();
    $("preset-name").focus();
  }
  function deletePreset(): void {
    if (!preset() || !confirm(`Remove preset “${preset().name}”?`)) return;
    map.rosters = map.rosters.filter((item) => item.id !== presetId);
    if (map.defaultRoster === presetId) map.defaultRoster = map.rosters[0]?.id || null;
    presetId = map.rosters[0]?.id || null;
    selectedId = null;
    pendingId = null;
    markDirty();
    renderPresets();
  }
  async function loadMap(id: string, force = false): Promise<void> {
    if (dirty && !force && !confirm("Discard unsaved roster edits?")) {
      $<HTMLSelectElement>("map-select").value = mapId;
      return;
    }
    $("save-status").textContent = "Loading…";
    try {
      map = await SOWCampaignMapPreview.load(id);
      mapId = id;
      etag = map.etag;
      dirty = false;
      selectedId = null;
      selectedAnchor = null;
      pendingId = null;
      presetId = map.rosters[0]?.id || null;
      $("save-status").textContent = "Saved";
      renderPresets();
      fit();
    } catch (error) {
      $("save-status").textContent = "Load failed";
      $("map-status").textContent = error.message;
    }
  }
  async function save(): Promise<void> {
    if (!map || saving) return;
    if (pendingId) {
      alert("Place the pending entity on the map before saving.");
      return;
    }
    const warnings = warnSummary(),
      invalid = warnings.find((value) => value.kind === "duplicate" || value.kind === "unknown");
    if (invalid) {
      if (presetId !== invalid.presetId) {
        presetId = invalid.presetId;
        renderPresets();
      }
      alert(`${preset()?.name || invalid.presetId}: ${invalid.text}`);
      return;
    }
    saving = true;
    $("save-status").textContent = "Saving…";
    try {
      const body = { default_roster: map.defaultRoster || null, rosters: map.rosters || [] };
      const response = await fetch("/__map-rosters?map=" + encodeURIComponent(mapId), {
        method: "POST",
        headers: { "Content-Type": "application/json", "If-Match": etag || "" },
        body: JSON.stringify(body),
      });
      if (!response.ok) throw new Error(await response.text());
      etag = response.headers.get("ETag");
      dirty = false;
      $("save-status").textContent = "Saved";
      if (warnings.some((value) => value.kind === "water"))
        alert(
          "Saved. Water positions remain visible as warnings; place those entities on land if that was unintended.",
        );
    } catch (error) {
      $("save-status").textContent = "Save failed";
      alert(error.message);
    } finally {
      saving = false;
    }
  }
  async function load(): Promise<void> {
    try {
      const [mapsResponse, catalogResponse] = await Promise.all([
        fetch("/__entity_maps", { cache: "no-store" }),
        fetch("/__entities", { cache: "no-store" }),
      ]);
      if (!mapsResponse.ok || !catalogResponse.ok) throw new Error("Map or Atlas catalog unavailable");
      mapIds = await mapsResponse.json();
      catalog = await catalogResponse.json();
      entityPicker = SOWAtlasEntityPicker.attach({
        input: $<HTMLInputElement>("entity-search"),
        results: $("entity-results"),
        status: $("entity-picker-status"),
        getEntities: () => catalog.entities,
        onChange: updateEntityControls,
      });
      const selected = new URLSearchParams(location.search).get("map");
      options(
        $<HTMLSelectElement>("map-select"),
        mapIds,
        (id) => id,
        (id) => id,
      );
      const start = mapIds.includes(selected) ? selected : mapIds.includes("world") ? "world" : mapIds[0];
      if (!start) throw new Error("No map files found");
      $<HTMLSelectElement>("map-select").value = start;
      await loadMap(start, true);
      const selectedEntity = new URLSearchParams(location.search).get("entity");
      if (selectedEntity && !(await entityPicker.select(selectedEntity)))
        $("entity-picker-status").textContent = `Atlas entity not found: ${selectedEntity}`;
    } catch (error) {
      $("save-status").textContent = "Load failed";
      $("map-status").textContent = error.message;
    }
  }
  $("preset-select").onchange = () => {
    presetId = $<HTMLSelectElement>("preset-select").value || null;
    selectedId = null;
    selectedAnchor = null;
    pendingId = null;
    renderPresets();
  };
  $("new-preset").onclick = newPreset;
  $("delete-preset").onclick = deletePreset;
  $("preset-name").onchange = () => {
    if (preset() && $<HTMLInputElement>("preset-name").value.trim()) {
      preset().name = $<HTMLInputElement>("preset-name").value.trim();
      markDirty();
      renderPresets();
    }
  };
  $("default-select").onchange = () => {
    map.defaultRoster = $<HTMLSelectElement>("default-select").value || null;
    markDirty();
    renderPresets();
  };
  $("entity-list").addEventListener("click", (event) => {
    if ((event.target as Element).closest?.(".entity")) entityPicker?.clear();
  });
  $("map-select").onchange = () => loadMap($<HTMLSelectElement>("map-select").value);
  $("reload").onclick = () => loadMap(mapId, true);
  $("save").onclick = save;
  $("add-entity").onclick = () => {
    const found = entityPicker?.getSelected();
    if (!preset() || !found) return;
    const existing = entry(found.id);
    if (existing) {
      selectEntity(found.id);
      return;
    }
    preset().entries.push({
      entity_id: found.id,
      role: $<HTMLSelectElement>("role-select").value as RosterEntry["role"],
      x: 0,
      y: 0,
      legacy_anchor: null,
    });
    selectedId = found.id;
    pendingId = found.id;
    markDirty();
    renderPresets();
    draw();
  };
  $("labels").onchange = draw;
  $("zoom-in").onclick = () => zoomAt([viewWidth / 2, viewHeight / 2], 1.4);
  $("zoom-out").onclick = () => zoomAt([viewWidth / 2, viewHeight / 2], 1 / 1.4);
  $("fit").onclick = fit;
  canvas.addEventListener("pointerdown", (event) => {
    canvas.setPointerCapture(event.pointerId);
    pointer = screenPoint(event);
    pointerStart = pointer.slice();
    pointerMoved = false;
    const hit = markerAt(pointer);
    if (pendingId) pointerMode = "place";
    else if (hit) {
      selectedId = hit.entity_id;
      entityPicker?.clear();
      renderPresets();
      pointerMode = "move";
    } else {
      pointerMode = "pan";
      canvas.classList.add("dragging");
    }
  });
  canvas.addEventListener("pointermove", (event) => {
    if (!pointer) return;
    const point = screenPoint(event);
    if (Math.hypot(point[0] - pointerStart[0], point[1] - pointerStart[1]) > 2) pointerMoved = true;
    if (pointerMode === "pan" && pointerMoved) {
      applyView(SOWCampaignMapPreview.panMapView(viewState(), point[0] - pointer[0], point[1] - pointer[1]));
      draw();
    } else if (pointerMode === "move" && pointerMoved) {
      placeAt(selectedId, point, false);
    }
    pointer = point;
  });
  canvas.addEventListener("pointerup", (event) => {
    const point = screenPoint(event);
    if (pointerMode === "place") {
      if (placeAt(pendingId, point)) entityPicker?.clear();
    } else if (pointerMode === "move" && pointerMoved) renderPresets();
    else if (pointerMode === "move") {
      const hit = markerAt(point);
      if (hit) {
        selectedId = hit.entity_id;
        renderPresets();
      }
    }
    pointer = null;
    pointerStart = null;
    pointerMode = "";
    canvas.classList.remove("dragging");
  });
  canvas.addEventListener("pointercancel", () => {
    pointer = null;
    pointerStart = null;
    pointerMode = "";
  });
  canvas.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      zoomAt(screenPoint(event), Math.exp(-event.deltaY * 0.0015));
    },
    { passive: false },
  );
  window.addEventListener("resize", resize);
  resize();
  document.addEventListener("keydown", (event) => {
    if (
      (event.key !== "Delete" && event.key !== "Backspace") ||
      !selectedId ||
      (event.target as Element).closest?.("input,textarea,select,[contenteditable='true']")
    )
      return;
    event.preventDefault();
    const current = preset();
    current.entries = current.entries.filter((item) => item.entity_id !== selectedId);
    selectedId = null;
    pendingId = null;
    markDirty();
    renderPresets();
  });
  load();
})();
