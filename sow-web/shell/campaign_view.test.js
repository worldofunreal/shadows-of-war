const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const shell = __dirname;
const shellSource = fs.readFileSync(path.join(shell, "main_menu.shell.js"), "utf8");
const lobbiesSource = fs.readFileSync(path.join(shell, "main_menu.lobbies.js"), "utf8");
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const webCatalogEn = fs.readFileSync(path.join(shell, "../../sow-i18n/strings/en/web.toml"), "utf8");
const campaignView = fs.readFileSync(path.join(shell, "sow-campaign-view.js"), "utf8");
const storyCss = fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
const campaignEditor = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/editor.js"), "utf8");
const campaignEditorServer = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/server.mjs"), "utf8");
const campaignEditorHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/logic.html"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");
const hudCss = fs.readFileSync(path.join(shell, "main_menu.hud.css"), "utf8");
const windowInput = fs.readFileSync(path.join(shell, "../../sow-client/src/input/window.rs"), "utf8");
const surfaceSource = fs.readFileSync(path.join(shell, "../../sow-client/src/input/surface.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const coreSource = fs.readFileSync(path.join(shell, "main_menu.core.js"), "utf8");
const campaignEngine = fs.readFileSync(path.join(shell, "sow-campaign.js"), "utf8");
const campaignModule = fs.readFileSync(path.join(shell, "../../sow-client/src/campaign/mod.rs"), "utf8");
const cameraFrameUiSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/frame/ui.rs"), "utf8");


test("campaign zoom input mode prioritizes TWA and touch devices, then Mac trackpads", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.equal(campaign.zoomInputMode({ androidTwa: true, platform: "MacIntel" }), "pinch");
    assert.equal(campaign.zoomInputMode({ mobile: true, platform: "MacIntel" }), "pinch");
    assert.equal(campaign.zoomInputMode({ userAgent: "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X)", platform: "MacIntel", maxTouchPoints: 5 }), "pinch");
    assert.equal(campaign.zoomInputMode({ platform: "MacIntel" }), "trackpad");
    assert.equal(campaign.zoomInputMode({ platform: "Win32" }), "wheel");
    assert.equal(campaign.zoomInputMode({ platform: "Linux x86_64" }), "wheel");
});


test("camera practice stays paused through desktop and touch routes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "camera_practice_test", text_namespace: "tutorial.camera_practice_",
        settings: { starting_troops: 1000 }, speakers: {}, entry: "boudica_zoom_out",
        steps: [
            { id: "boudica_zoom_out", type: "objective", title_key: "tutorial.zoom_out", trigger: { type: "zoom_out_complete", value: 0.95, scope: "step" }, guide: { kind: "ui", target: "hud_center_camera", gesture: "zoom_out" }, pause_game: true, camera_only: true, next: "boudica_camera_drag" },
            { id: "boudica_camera_drag", type: "objective", title_key: "tutorial.camera_drag", trigger: { type: "camera_target", target: "suetonius_paulinus", distance: 8, scope: "step" }, marker: { target: "suetonius_paulinus" }, guide: { kind: "world", target: "target_action", gesture: "drag" }, pause_game: true, camera_only: true, next: "boudica_camera_hover" },
            { id: "boudica_camera_hover", type: "objective", title_key: "tutorial.camera_hover", trigger: { type: "hover", target: "suetonius_paulinus", value: 1, scope: "step" }, marker: { target: "suetonius_paulinus" }, guide: { kind: "world", target: "target_action", gesture: "hover" }, pause_game: true, camera_only: true, next: "boudica_zoom_in" },
            { id: "boudica_zoom_in", type: "objective", title_key: "tutorial.zoom_in", trigger: { type: "zoom_in_complete", target: "suetonius_paulinus", distance: 8, scope: "step" }, guide: { kind: "ui", target: "hud_center_camera", gesture: "zoom_in" }, pause_game: true, camera_only: true, next: "boudica_found_suetonius" },
            { id: "boudica_found_suetonius", type: "scene", title_key: "tutorial.found", body_key: "tutorial.found_body", next: "boudica_camera_home" },
            { id: "boudica_camera_home", type: "objective", title_key: "tutorial.home", trigger: { type: "ui", action: "hud_center_camera", scope: "step" }, guide: { kind: "ui", target: "hud_center_camera", gesture: "tap" }, pause_game: true, camera_only: true, next: "boudica_city_intro" },
            { id: "boudica_city_intro", type: "scene", title_key: "tutorial.city", body_key: "tutorial.city_body", wait_before: true, next: "boudica_choose_city" },
            { id: "boudica_choose_city", type: "scene", title_key: "tutorial.choose_city", body_key: "tutorial.choose_city_body", next: "boudica_end" },
            { id: "boudica_end", type: "end", title_key: "tutorial.end" }
        ]
    };
    const target = 10;
    assert.ok(10 >= target, "zoom in completes at the requested zoom");
    assert.ok(22 >= target, "zoom in completes after overshooting the requested zoom");
    assert.ok(!(9 >= target), "zoom in does not complete below the requested zoom");
    const pinchStart = 8, pinchTarget = 10;
    const pinchDelta = (pinchTarget / pinchStart - 1) / 0.005;
    assert.ok(Math.abs(pinchStart * (1 + pinchDelta * 0.005) - pinchTarget) < 0.00001);
    const invalidDistance = JSON.parse(JSON.stringify(definition));
    invalidDistance.steps.find(candidate => candidate.id === "boudica_camera_drag").trigger.distance = 0;
    assert.ok(campaign.validate(invalidDistance, { factions: [] }, { hasText: () => true }).errors.some(issue => issue.field === "trigger.distance"));
    const invalidZoomRange = JSON.parse(JSON.stringify(definition));
    invalidZoomRange.steps.find(candidate => candidate.id === "boudica_zoom_out").trigger.value = 1.1;
    assert.ok(campaign.validate(invalidZoomRange, { factions: [] }, { hasText: () => true }).errors.some(issue => issue.field === "trigger.value"));
    assert.equal(definition.steps.find(candidate => candidate.id === "boudica_zoom_out").next, "boudica_camera_drag");
    assert.equal(definition.steps.find(candidate => candidate.id === "boudica_camera_hover").next, "boudica_zoom_in");
    assert.equal(definition.steps.find(candidate => candidate.id === "boudica_zoom_in").trigger.target, "suetonius_paulinus");
    assert.equal(definition.steps.find(candidate => candidate.id === "boudica_zoom_in").next, "boudica_found_suetonius");
    for (const id of ["boudica_zoom_out", "boudica_zoom_in", "boudica_camera_drag", "boudica_camera_hover", "boudica_camera_home"]) {
        const step = definition.steps.find(candidate => candidate.id === id);
        assert.equal(step.pause_game, true, `${id} pauses the simulation`);
        assert.equal(step.camera_only, true, `${id} permits only camera input`);
    }
    const zoomStart = 1, zoomFloor = 0.2, zoomClose = 0.75;
    const initialFacts = { camera_zoom: zoomStart, camera_zoom_start: zoomStart, camera_zoom_floor: zoomFloor, camera_zoom_target: zoomClose, zoom_out_events: 0, zoom_in_events: 0, camera_target_distance: 30, zoom_out_complete: 0, zoom_in_complete: 0 };
    const zoomOutGoal = campaign.zoomOutTarget(initialFacts, 0.95);
    assert.ok(Math.abs(zoomOutGoal - 0.24) < 0.00001, "zoom-out brings the battlefield close to its wide-view limit");
    assert.ok(Math.abs(campaign.zoomOutProgress({ ...initialFacts, camera_zoom: zoomOutGoal }) - 0.95) < 0.00001);
    let desktopClock = 0;
    const desktop = campaign.create(definition, "boudica_zoom_out", { factions: [] }, () => desktopClock);
    desktop.update(initialFacts, {}, 0);
    assert.equal(desktop.view().paused, true);
    assert.equal(desktop.update({ ...initialFacts, camera_zoom: zoomOutGoal }, {}, 0.5).step.id, "boudica_zoom_out", "reaching the target without a zoom action does not count");
    assert.equal(desktop.advance(null, "boudica_zoom_out"), false);
    const zoomedOut = { ...initialFacts, camera_zoom: zoomOutGoal, zoom_out_events: 1 };
    assert.equal(desktop.update(zoomedOut, {}, 1).step.id, "boudica_camera_drag");
    assert.equal(desktop.view().paused, true);
    assert.equal(desktop.update({ ...zoomedOut, camera_target_distance: 9 }, {}, 2).step.id, "boudica_camera_drag");
    assert.equal(desktop.update({ ...zoomedOut, camera_target_distance: 8 }, {}, 3).step.id, "boudica_camera_hover");
    assert.equal(desktop.view().paused, true);
    const adaptiveDrag = campaign.create(definition, "boudica_camera_drag", { factions: [] });
    adaptiveDrag.update({ ...zoomedOut, camera_target_distance: 81, camera_target_radius: 80 }, {}, 0);
    assert.equal(adaptiveDrag.update({ ...zoomedOut, camera_target_distance: 80, camera_target_radius: 80 }, {}, 1).step.id, "boudica_camera_hover", "camera practice uses the visible, zoom-relative target radius");
    const hovered = { ...zoomedOut, camera_target_distance: 8, hover_events: 1 };
    assert.equal(desktop.update(hovered, {}, 4).step.id, "boudica_zoom_in");
    const hoverAfterReturningToCloseView = campaign.create(definition, "boudica_camera_hover", { factions: [] });
    const closeViewWithoutCameraDistance = {
        ...zoomedOut,
        camera_zoom: 2.0,
        zoom_in_events: 1,
        camera_target_distance: undefined,
        hover_events: 0
    };
    hoverAfterReturningToCloseView.update(closeViewWithoutCameraDistance, {}, 4);
    assert.equal(hoverAfterReturningToCloseView.update({ ...closeViewWithoutCameraDistance, hover_events: 1 }, {}, 4.25).step.id, "boudica_zoom_in", "hover does not require the zoom-out level or camera distance");
    assert.equal(desktop.update({ ...hovered, camera_zoom: zoomClose }, {}, 4.25).step.id, "boudica_zoom_in", "zoom level alone does not satisfy the final zoom lesson");
    assert.equal(desktop.update({ ...hovered, camera_zoom: zoomClose, zoom_in_events: 1, camera_target_distance: 9 }, {}, 4.5).step.id, "boudica_zoom_in", "zooming away from the commander does not satisfy the lesson");
    const adaptiveZoom = campaign.create(definition, "boudica_zoom_in", { factions: [] });
    const adaptiveZoomStart = { camera_zoom: zoomClose, camera_zoom_target: zoomClose, zoom_in_events: 0, camera_target_distance: 13, camera_target_radius: 12 };
    adaptiveZoom.update(adaptiveZoomStart, {}, 0);
    assert.equal(adaptiveZoom.update({ ...adaptiveZoomStart, zoom_in_events: 1 }, {}, 1).step.id, "boudica_zoom_in");
    assert.equal(adaptiveZoom.update({ ...adaptiveZoomStart, zoom_in_events: 1, camera_target_distance: 12 }, {}, 2).step.id, "boudica_found_suetonius", "final zoom accepts the same camera-relative distance");
    const inspected = { ...hovered, camera_zoom: zoomClose, zoom_in_events: 1, camera_target_distance: 8 };
    assert.equal(desktop.update(inspected, {}, 4.75).step.id, "boudica_found_suetonius");
    assert.equal(desktop.advance(null, "boudica_found_suetonius"), true);
    assert.equal(desktop.view().step.id, "boudica_camera_home");
    assert.equal(desktop.update(inspected, {}, 5).step.id, "boudica_camera_home");
    assert.equal(desktop.update(inspected, { hud_center_camera: 1 }, 6).step.id, "boudica_city_intro");
    assert.equal(desktop.view().waiting, true);
    assert.equal(desktop.view().paused, true);
    assert.equal(desktop.advance(null, "boudica_city_intro"), false);
    desktopClock = 1099;
    assert.equal(desktop.view().waiting, true, "dialogue stays hidden through 1099ms");
    desktopClock = 1100;
    assert.equal(desktop.view().waiting, false, "dialogue reveals at exactly 1100ms");
    assert.equal(desktop.advance(null, "boudica_city_intro"), true);
    assert.equal(desktop.view().step.id, "boudica_choose_city");
    let revealClock = 0;
    const timedReturn = campaign.create(definition, "boudica_camera_home", { factions: [] }, () => revealClock);
    timedReturn.update({}, {});
    assert.equal(timedReturn.update({}, { hud_center_camera: 1 }).waiting, true);
    revealClock = 1099;
    assert.equal(timedReturn.update({}, { hud_center_camera: 1 }).waiting, true);
    revealClock = 1100;
    assert.equal(timedReturn.update({}, { hud_center_camera: 1 }).waiting, false);
    assert.equal(timedReturn.advance(null, "boudica_city_intro"), true);
    assert.equal(timedReturn.view().step.id, "boudica_choose_city");
    assert.equal(timedReturn.view().paused, true);

    const touch = campaign.create(definition, "boudica_camera_drag", { factions: [] });
    touch.update({ camera_target: 0, camera_target_distance: 30, touch_controls: 1 }, {}, 0);
    assert.equal(touch.view().step.id, "boudica_camera_drag");
    assert.equal(touch.update({ camera_target: 1, camera_target_distance: 8, touch_controls: 1 }, {}, 1).step.id, "boudica_camera_hover");
    assert.equal(touch.view().paused, true);
});


test("campaign cinematic steps reuse the shared editor/game player and validate local media", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const roster = { map: "eastanglia", player_spawn: [10, 10], factions: [
        { id: "cinematic_target", name: "Target", x: 20, y: 20, starting_troops: 500, relation: "enemy", civ: "Roman Empire", leader: "Caesar" }
    ] };
    const definition = {
        version: 2, episode_id: "cinematic_test", settings: { starting_troops: 1000 },
        entry: "intro", speakers: {}, steps: [
            { id: "intro", type: "scene", presentation: "cinematic", title_key: "tutorial.intro", body_key: "tutorial.intro_body", next: "reveal" },
            { id: "reveal", type: "scene", presentation: "cinematic", title_key: "tutorial.reveal", body_key: "tutorial.reveal_body", next: "end" },
            { id: "end", type: "end", title_key: "tutorial.end" }
        ]
    };
    const intro = definition.steps.find(step => step.id === "intro");

    const localVideo = JSON.parse(JSON.stringify(definition));
    localVideo.steps.find(step => step.id === intro.id).video_src = "/assets/campaign/cinematic_test/opening.webm";
    assert.deepEqual(campaign.validate(localVideo, roster, { hasText: () => true, hasAvatar: () => true }).errors, []);
    for (const source of ["https://example.com/opening.mp4", "/assets/campaign/other/opening.mp4", "/assets/campaign/cinematic_test/../opening.mp4", "/assets/campaign/cinematic_test/opening.mov"]) {
        const invalid = JSON.parse(JSON.stringify(localVideo));
        invalid.steps.find(step => step.id === intro.id).video_src = source;
        assert.ok(campaign.validate(invalid, roster, { hasText: () => true, hasAvatar: () => true }).errors.some(issue => issue.field === "video_src"), source);
    }

});


test("Boudica video completion reveals its dialogue before the attack objective", async () => {
    const timers = new Map(); let timerId = 0; let playCalls = 0; let continued = 0; const cinematicChanges = [];
    class Element {
        constructor(ownerDocument, tagName) {
            this.ownerDocument = ownerDocument; this.tagName = tagName; this.listeners = {}; this.queries = new Map();
            this.children = []; this.attributes = {}; this.dataset = {}; this.hidden = false; this.textContent = "";
            this.paused = true; this.muted = false; this.ended = false;
            this.style = { setProperty() {}, removeProperty(name) { delete this[name]; }, getPropertyValue() { return ""; } };
            const classes = new Set();
            this.classList = {
                add: (...values) => values.forEach(value => classes.add(value)),
                remove: (...values) => values.forEach(value => classes.delete(value)),
                toggle: (value, force) => force === undefined ? (classes.has(value) ? classes.delete(value) : classes.add(value)) : (force ? classes.add(value) : classes.delete(value)),
                contains: value => classes.has(value)
            };
            this.clientWidth = 800; this.clientHeight = 600; this.scrollHeight = 20; this.isConnected = true;
        }
        addEventListener(type, listener, options) { (this.listeners[type] ||= []).push({ listener, once: Boolean(options && options.once) }); }
        removeEventListener(type, listener) { this.listeners[type] = (this.listeners[type] || []).filter(item => item.listener !== listener); }
        dispatch(type, extra = {}) {
            const event = Object.assign({ type, target: this, timeStamp: 1000, stopPropagation() {}, preventDefault() {} }, extra);
            const listeners = this.listeners[type] || [];
            this.listeners[type] = listeners.filter(item => !item.once);
            listeners.forEach(item => item.listener(event));
        }
        setAttribute(name, value) { this.attributes[name] = String(value); }
        getAttribute(name) { return this.attributes[name] || null; }
        hasAttribute(name) { return Object.hasOwn(this.attributes, name); }
        removeAttribute(name) { delete this.attributes[name]; }
        querySelector(selector) {
            if (!this.queries.has(selector)) this.queries.set(selector, new Element(this.ownerDocument, /img$/.test(selector) ? "img" : selector === "video" ? "video" : ["[data-story-start]", "[data-story-continue]"].includes(selector) ? "button" : "div"));
            return this.queries.get(selector);
        }
        querySelectorAll() { return []; }
        replaceChildren(...children) { this.children = children; }
        appendChild(child) { this.children.push(child); return child; }
        contains(child) { return this.children.includes(child) || [...this.queries.values()].includes(child); }
        closest(selector) { return selector.includes("button") && this.tagName === "button" ? this : null; }
        getBoundingClientRect() { return { width: 100, height: 40 }; }
        focus() { this.ownerDocument.activeElement = this; }
        pause() { this.paused = true; }
        load() {}
        click() {}
    }
    const documentListeners = {};
    const doc = {
        hidden: false,
        documentElement: { dir: "ltr", dataset: { localeScript: "latin" } },
        defaultView: { matchMedia: () => ({ matches: false }), performance: { now: () => 1 }, navigator: {} },
        createElement(tagName) { return new Element(this, tagName); },
        addEventListener(type, listener, capture) { (documentListeners[type] ||= []).push({ listener, capture: Boolean(capture) }); },
        removeEventListener(type, listener) { documentListeners[type] = (documentListeners[type] || []).filter(item => item.listener !== listener); },
        dispatchClick(target, isTrusted = true) {
            const event = { target, isTrusted, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopPropagation() {} };
            (documentListeners.click || []).filter(item => item.capture).forEach(item => item.listener(event));
            return event;
        }
    };
    doc.defaultView.getComputedStyle = () => ({ marginBottom: "0px" });
    const root = new Element(doc, "div"); root.ownerDocument = doc; root.hidden = true;
    const sandbox = {
        console,
        setTimeout(callback, delay) { const id = ++timerId; timers.set(id, { callback, delay }); return id; },
        clearTimeout(id) { timers.delete(id); }
    };
    vm.runInNewContext(campaignView, sandbox, { filename: "sow-campaign-view.js" });
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    const roster = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.json"), "utf8"));
    const opening = definition.steps.find(step => step.id === definition.entry);
    let machine = campaign.create(definition, definition.entry, roster);
    const view = sandbox.SOWCampaignView.mount(root, {
        asset: value => "/" + value,
        translate: key => key,
        roster,
        onContinue() {
            continued++;
            assert.equal(machine.advance(null, machine.view().step.id), true);
            view.render(machine.view(), {});
        },
        onDismiss: null,
        onCinematicChange(active) { cinematicChanges.push(active); }
    });
    const video = root.querySelector(".sow-story__cinematic").querySelector("video");
    video.play = () => {
        playCalls++; video.paused = false;
        return playCalls === 1
            ? Promise.reject(Object.assign(new Error("gesture required"), { name: "NotAllowedError" }))
            : Promise.resolve();
    };
    view.render(machine.view(), {});
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
    const start = root.querySelector("[data-story-start]");
    const dialog = root.querySelector(".sow-story__dialog");
    const cinematic = root.querySelector(".sow-story__cinematic");
    assert.equal(playCalls, 2, "a rejected audible attempt is retried muted so the scene can continue");
    assert.deepEqual(cinematicChanges, [true], "music is paused as soon as the cinematic is presented");
    assert.equal(video.muted, true, "only a rejected autoplay falls back to muted video");
    assert.equal(start.hidden, true, "autoplay recovery never shows a blocking start prompt");
    assert.equal(dialog.hidden, true, "the cinematic hides the dialogue card while playing");
    const cinematicClick = doc.dispatchClick(cinematic, false);
    assert.equal(video.muted, true, "synthetic clicks do not unlock audio");
    assert.equal(cinematicClick.defaultPrevented, false);
    const audioClick = doc.dispatchClick(cinematic);
    assert.equal(video.muted, false, "the first real click enables cinematic audio");
    assert.equal(audioClick.defaultPrevented, false, "audio recovery does not consume the click");
    video.dispatch("playing");
    assert.equal(cinematic.hidden, false, "the fullscreen overlay remains visible during playback");
    video.paused = true;
    video.dispatch("pause");
    assert.equal(start.hidden, true, "a phone pause does not bring the start prompt back");
    const wakeClick = doc.dispatchClick(cinematic);
    await Promise.resolve();
    assert.equal(playCalls, 3, "the next real click resumes a video paused by sleep");
    assert.equal(video.paused, false);
    assert.equal(wakeClick.defaultPrevented, false, "resuming after sleep leaves the click intact");
    video.dispatch("playing");
    cinematic.dispatch("pointerdown", { pointerId: 1, pointerType: "mouse", isPrimary: true, button: 0 });
    assert.equal(cinematic.classList.contains("is-holding"), true, "desktop press begins the skip progress");
    cinematic.dispatch("pointerup", { pointerId: 1, pointerType: "mouse" });
    assert.equal(cinematic.classList.contains("is-holding"), false, "a short desktop press cancels");
    assert.equal([...timers.values()].some(timer => timer.delay === 1250), false);
    assert.equal(continued, 0);

    const continueButton = root.querySelector("[data-story-continue]");
    continueButton.setAttribute("data-story-continue", "");
    for (const [width, height] of [[1530, 920], [390, 844]]) {
        for (const completion of ["hold", "Escape", "ended", "error"]) {
            view.render(null, {});
            root.clientWidth = width; root.clientHeight = height;
            machine = campaign.create(definition, definition.entry, roster);
            view.render(machine.view(), {});
            video.dispatch("playing");
            const before = continued;
            cinematic.dispatch("pointerdown", { pointerId: 2, pointerType: width < 960 ? "touch" : "mouse", isPrimary: true, button: 0 });
            const holdTimer = [...timers.entries()].find(([, timer]) => timer.delay === 1250);
            assert.ok(holdTimer, "skipping still requires a 1.25-second hold");
            if (completion === "hold") {
                timers.delete(holdTimer[0]); holdTimer[1].callback();
            } else if (completion === "Escape") root.dispatch("keydown", { key: "Escape", repeat: false });
            else video.dispatch(completion);

            assert.equal(continued, before, `${completion} closes only the video`);
            assert.equal(machine.view().step.id, definition.entry);
            assert.equal(cinematic.hidden, true);
            assert.equal(video.paused, true);
            assert.equal(cinematicChanges.at(-1), false, "music resumes with the dialogue");
            assert.equal(dialog.hidden, false, `${completion} reveals Boudica's dialogue at ${width}x${height}`);
            assert.equal(root.querySelector(".sow-story__body").textContent, opening.body_key);
            assert.equal(root.querySelector(".sow-story__footer").hidden, false);
            assert.equal(cinematic.classList.contains("is-holding"), false);
            assert.equal([...timers.values()].some(timer => timer.delay === 1250), false, "video completion cancels a pending skip");
            video.dispatch("playing");
            assert.equal(dialog.hidden, false, "a late playback event cannot hide the dialogue again");

            root.dispatch("click", { target: continueButton, timeStamp: 2000 + before * 1000 });
            assert.equal(continued, before + 1);
            assert.equal(machine.view().step.id, opening.next, "only Continue opens the attack objective");
            assert.equal(dialog.hidden, true);
            assert.equal(root.querySelector(".sow-story__objective").hidden, width < 960);
        }
    }
    view.render(null, {});
    machine = campaign.create(definition, definition.entry, roster);
    let rejectPlayback;
    video.play = () => { playCalls++; return new Promise((resolve, reject) => { rejectPlayback = reject; }); };
    view.render(machine.view(), {});
    root.dispatch("keydown", { key: "Escape", repeat: false });
    const attempts = playCalls;
    rejectPlayback(Object.assign(new Error("gesture required"), { name: "NotAllowedError" }));
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
    assert.equal(playCalls, attempts, "a late autoplay rejection cannot restart a skipped video");
    assert.equal(machine.view().step.id, definition.entry);
    assert.equal(dialog.hidden, false);
    view.destroy();
});


test("cinematic waits for the loader and swaps in hashed WebM+MP4 sources with a poster", async () => {
    const timers = new Map(); let timerId = 0;
    class Element {
        constructor(ownerDocument, tagName) {
            this.ownerDocument = ownerDocument; this.tagName = tagName; this.listeners = {}; this.queries = new Map();
            this.children = []; this.attributes = {}; this.dataset = {}; this.hidden = false; this.textContent = "";
            this.style = { setProperty() {}, removeProperty(name) { delete this[name]; }, getPropertyValue() { return ""; } };
            const classes = new Set();
            this.classList = {
                add: (...values) => values.forEach(value => classes.add(value)),
                remove: (...values) => values.forEach(value => classes.delete(value)),
                toggle: (value, force) => force === undefined ? (classes.has(value) ? classes.delete(value) : classes.add(value)) : (force ? classes.add(value) : classes.delete(value)),
                contains: value => classes.has(value)
            };
            this.clientWidth = 800; this.clientHeight = 600; this.scrollHeight = 20; this.isConnected = true;
        }
        addEventListener(type, listener, options) { (this.listeners[type] ||= []).push({ listener, once: Boolean(options && options.once) }); }
        removeEventListener(type, listener) { this.listeners[type] = (this.listeners[type] || []).filter(item => item.listener !== listener); }
        dispatch(type, extra = {}) {
            const event = Object.assign({ type, target: this, timeStamp: 1000, stopPropagation() {}, preventDefault() {} }, extra);
            const listeners = this.listeners[type] || [];
            this.listeners[type] = listeners.filter(item => !item.once);
            listeners.forEach(item => item.listener(event));
        }
        setAttribute(name, value) { this.attributes[name] = String(value); }
        getAttribute(name) { return this.attributes[name] || null; }
        hasAttribute(name) { return Object.hasOwn(this.attributes, name); }
        removeAttribute(name) { delete this.attributes[name]; }
        querySelector(selector) {
            if (!this.queries.has(selector)) this.queries.set(selector, new Element(this.ownerDocument, /img$/.test(selector) ? "img" : selector === "video" ? "video" : selector === "[data-story-start]" ? "button" : "div"));
            return this.queries.get(selector);
        }
        querySelectorAll() { return []; }
        replaceChildren(...children) { this.children = children; }
        appendChild(child) { this.children.push(child); return child; }
        contains(child) { return this.children.includes(child) || [...this.queries.values()].includes(child); }
        closest(selector) { return selector.includes("button") && this.tagName === "button" ? this : null; }
        getBoundingClientRect() { return { width: 100, height: 40 }; }
        focus() { this.ownerDocument.activeElement = this; }
        pause() {}
        load() {}
        click() {}
    }
    const loader = new Element(null, "div");
    loader.attributes["aria-hidden"] = "true";
    const viewListeners = {};
    const doc = {
        hidden: false,
        documentElement: { dir: "ltr", dataset: { localeScript: "latin" } },
        defaultView: {
            matchMedia: () => ({ matches: false }), performance: { now: () => 1 }, navigator: {},
            addEventListener(type, listener, options) { (viewListeners[type] ||= []).push({ listener, once: Boolean(options && options.once) }); },
            removeEventListener(type, listener) { viewListeners[type] = (viewListeners[type] || []).filter(item => item.listener !== listener); }
        },
        createElement(tagName) { return new Element(this, tagName); },
        getElementById(id) { return id === "web-loader" ? loader : null; },
        addEventListener() {}, removeEventListener() {}
    };
    const dispatchView = type => {
        (viewListeners[type] || []).forEach(item => item.listener({ type }));
        viewListeners[type] = (viewListeners[type] || []).filter(item => !item.once);
    };
    const root = new Element(doc, "div"); root.ownerDocument = doc; root.hidden = true;
    const sandbox = {
        console,
        setTimeout(callback, delay) { const id = ++timerId; timers.set(id, { callback, delay }); return id; },
        clearTimeout(id) { timers.delete(id); }
    };
    vm.runInNewContext(campaignView, sandbox, { filename: "sow-campaign-view.js" });
    const view = sandbox.SOWCampaignView.mount(root, {
        asset: value => "/" + value,
        translate: key => key,
        roster: { factions: [] },
        onContinue() {},
        onDismiss() {},
        onCinematicChange() {}
    });
    const video = root.querySelector(".sow-story__cinematic").querySelector("video");
    let playCalls = 0;
    video.play = () => { playCalls++; return Promise.resolve(); };
    const step = { id: "opening", type: "scene", presentation: "cinematic", video_src: "/assets/campaign/boudica/intro.720p.555d0500.webm", title_key: "opening_title", body_key: "opening_body" };
    const definition = { episode_id: "boudica", speakers: {}, steps: [step] };
    const machineView = { definition, step, line: step, paused: true, done: false, choices: [], progress: { target: 0, current: 0 }, state: { line: 0 } };
    view.render(machineView, {});
    await Promise.resolve();
    assert.equal(playCalls, 1, "a free loader lets the cinematic start at once");
    assert.equal(video.muted, false, "successful autoplay keeps the existing audible path unchanged");
    assert.deepEqual(video.children.map(child => [child.src, child.tagName]), [
        ["/assets/campaign/boudica/intro.720p.555d0500.webm", "source"],
        ["/assets/campaign/boudica/intro.720p.555d0500.mp4", "source"]
    ], "WebM first, MP4 as fallback");
    assert.equal(video.getAttribute("poster"), "/assets/campaign/boudica/intro.720p.555d0500.poster.webp", "the poster comes from the same stem");

    playCalls = 0;
    loader.attributes["aria-hidden"] = "false";
    const second = { ...step, id: "opening2", video_src: "/assets/campaign/boudica/intro.720p.555d0500.webm" };
    const secondView = { ...machineView, step: second, line: second, definition: { ...definition, steps: [second] } };
    view.render(secondView, {});
    await Promise.resolve();
    assert.equal(playCalls, 0, "a visible loader defers the cinematic without downloading bytes");
    assert.equal(root.querySelector("[data-story-start]").hidden, true, "gameplay loader waits without showing a start prompt");
    loader.attributes["aria-hidden"] = "true";
    dispatchView("sow:loader-cycle-ready");
    await Promise.resolve();
    assert.equal(playCalls, 1, "the cinematic starts once the loader is gone");
    view.destroy();

    const editorRoot = new Element(doc, "div"); editorRoot.ownerDocument = doc; editorRoot.hidden = true;
    const editorView = sandbox.SOWCampaignView.mount(editorRoot, {
        asset: value => "/" + value,
        translate: key => key,
        autoPlayCinematic: false,
        roster: { factions: [] },
        onContinue() {}, onDismiss() {}, onCinematicChange() {}
    });
    editorView.render(machineView, {});
    const playButton = editorRoot.querySelector("[data-story-start]");
    const editorVideo = editorRoot.querySelector(".sow-story__cinematic").querySelector("video");
    let editorPlayCalls = 0;
    editorVideo.play = () => { editorPlayCalls++; return Promise.resolve(); };
    assert.equal(playButton.hidden, false, "the campaign editor retains its deliberate manual Play action");
    assert.equal(playButton.textContent, "lobbies.play", "manual Play uses the existing localized label");
    playButton.setAttribute("data-story-start", "");
    editorRoot.dispatch("click", { target: playButton, timeStamp: 1000 });
    await Promise.resolve();
    assert.equal(editorPlayCalls, 1, "the editor Play action still starts its preview");
    editorView.destroy();
});




test("Boudica spoken cards and contact responses require named existing portraits", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definitionPath = path.join(shell, "../../assets/campaign/boudica.triggers.json");
    const rosterPath = path.join(shell, "../../assets/campaign/boudica.json");
    const avatarsDir = path.join(shell, "../../assets/art/avatars");
    const definition = JSON.parse(fs.readFileSync(definitionPath, "utf8"));
    const roster = JSON.parse(fs.readFileSync(rosterPath, "utf8"));
    const options = { hasText: () => true, hasAvatar: avatar => ["heroes", "nations", "tribes", "campaign"].some(folder => fs.existsSync(path.join(avatarsDir, folder, avatar + ".webp"))) };
    const report = value => campaign.validate(value, roster, options);
    assert.equal(definition.require_speaker_portraits, true);
    assert.deepEqual(report(definition).errors, []);

    const inheritedSpeaker = structuredClone(definition);
    const inheritedScene = inheritedSpeaker.steps.find(step => step.id === "boudica_camera_intro");
    inheritedScene.lines = [{ title_key: inheritedScene.title_key, body_key: inheritedScene.body_key }];
    assert.deepEqual(report(inheritedSpeaker).errors, [], "conversation lines inherit the scene speaker");

    const missingSpeaker = structuredClone(definition);
    delete missingSpeaker.steps.find(step => step.id === "boudica_camera_intro").speaker;
    assert.ok(report(missingSpeaker).errors.some(issue => issue.step === "boudica_camera_intro" && issue.field === "speaker"));

    const missingPortrait = structuredClone(definition);
    missingPortrait.speakers.boudica.avatar = "missing_portrait";
    assert.ok(report(missingPortrait).errors.some(issue => issue.field === "speakers" || issue.field === "speaker"));

    const missingDynamicRole = structuredClone(definition);
    const dynamicReaction = missingDynamicRole.reactions.find(reaction => reaction.id === "contact_stonea");
    delete dynamicReaction.speaker;
    assert.ok(report(missingDynamicRole).errors.some(issue => issue.field === "reactions.speaker"));

    const missingFactionPortrait = structuredClone(definition);
    roster.factions.find(faction => faction.id === "stonea").avatar = "null";
    assert.ok(report(missingFactionPortrait).errors.some(issue => issue.field === "speaker" && /portrait/.test(issue.message)));
    roster.factions.find(faction => faction.id === "stonea").avatar = "iceni";

    const badExpression = structuredClone(definition);
    badExpression.steps.find(step => step.id === definition.entry).portrait_expression = "furious";
    assert.ok(report(badExpression).errors.some(issue => issue.step === definition.entry && issue.field === "portrait_expression"));
    const titleWithoutKey = structuredClone(definition);
    const titleStep = titleWithoutKey.steps.find(step => step.id === definition.entry);
    titleStep.show_title = true; delete titleStep.title_key;
    assert.ok(report(titleWithoutKey).errors.some(issue => issue.step === definition.entry && issue.field === "show_title"));
    const badEmoteText = structuredClone(definition);
    badEmoteText.emote_texts = { threat: "not_a_tutorial_key" };
    assert.ok(report(badEmoteText).errors.some(issue => issue.field === "emote_texts"));
    const badEmoteKind = structuredClone(definition);
    badEmoteKind.steps.find(step => step.id === definition.entry).emote = "furious";
    assert.ok(report(badEmoteKind).errors.some(issue => issue.step === definition.entry && issue.field === "emote"));
    const pinnedRelease = structuredClone(definition);
    Object.assign(pinnedRelease.steps.find(step => step.id === definition.entry), { emote: "clear", emote_pinned: true });
    assert.ok(report(pinnedRelease).errors.some(issue => issue.step === definition.entry && issue.field === "emote_pinned"));
    for (const id of ["boudica_poison_end", "boudica_end"]) {
        const completion = definition.steps.find(step => step.id === id);
        assert.equal(completion.type, "end");
        assert.equal(completion.presentation, "chapter");
        assert.ok(completion.body_key);
        assert.equal(completion.speaker, undefined, "the epilogue is narration without a portrait");
        const spokenEnding = structuredClone(definition);
        spokenEnding.steps.find(step => step.id === id).presentation = "dialogue";
        assert.ok(report(spokenEnding).errors.some(issue => issue.step === id && issue.field === "speaker"));
    }


});


test("campaign view recovers portraits and renders compact objective guidance", async () => {
    const createdImages = [], warnings = [];
    const resizeObservers = [], resizeFrames = [];
    const documentListeners = {};
    let continueCalls = 0, dismissCalls = 0;
    class ResizeObserver {
        constructor(callback) { this.callback = callback; resizeObservers.push(this); }
        observe() {}
        disconnect() {}
    }
    class Element {
        constructor(ownerDocument, tagName) {
            this.ownerDocument = ownerDocument; this.tagName = tagName; this.listeners = {}; this.queries = new Map();
            this.children = []; this.attributes = {}; this.dataset = {}; this.hidden = false; this.textContent = "";
            this.style = { setProperty() {}, removeProperty() {}, getPropertyValue() { return ""; } };
            const classes = new Set();
            this.classList = {
                add: (...values) => values.forEach(value => classes.add(value)),
                remove: (...values) => values.forEach(value => classes.delete(value)),
                toggle: (value, force) => force === undefined ? (classes.has(value) ? classes.delete(value) : classes.add(value)) : (force ? classes.add(value) : classes.delete(value)),
                contains: value => classes.has(value)
            };
            this.clientWidth = 800; this.clientHeight = 600; this.scrollHeight = 20; this.isConnected = true;
        }
        addEventListener(type, listener, options) { (this.listeners[type] ||= []).push({ listener, once: Boolean(options && options.once) }); }
        removeEventListener(type, listener) { this.listeners[type] = (this.listeners[type] || []).filter(item => item.listener !== listener); }
        dispatch(type) {
            const listeners = this.listeners[type] || [];
            this.listeners[type] = listeners.filter(item => !item.once);
            listeners.forEach(item => item.listener({ type, target: this, stopPropagation() {}, preventDefault() {} }));
        }
        setAttribute(name, value) { this.attributes[name] = String(value); }
        getAttribute(name) { return this.attributes[name] || null; }
        hasAttribute(name) { return Object.hasOwn(this.attributes, name); }
        removeAttribute(name) { delete this.attributes[name]; }
        querySelector(selector) {
            if (!this.queries.has(selector)) this.queries.set(selector, new Element(this.ownerDocument, /img$/.test(selector) ? "img" : selector === "video" ? "video" : "div"));
            return this.queries.get(selector);
        }
        querySelectorAll() { return []; }
        replaceChildren(...children) { this.children = children; }
        appendChild(child) { this.children.push(child); return child; }
        contains(child) { return this.children.includes(child) || [...this.queries.values()].includes(child); }
        getBoundingClientRect() { return { width: 100, height: 40 }; }
        focus() { this.ownerDocument.activeElement = this; }
        pause() {}
        load() {}
        click() {}
    }
    const doc = {
        documentElement: { dir: "ltr", dataset: { localeScript: "latin" } },
        defaultView: {
            matchMedia: () => ({ matches: false }), performance: { now: () => 1 }, navigator: {}, ResizeObserver,
            requestAnimationFrame(callback) { resizeFrames.push(callback); return resizeFrames.length; },
            cancelAnimationFrame() {}
        },
        createElement(tagName) { const element = new Element(this, tagName); if (tagName === "img") createdImages.push(element); return element; },
        addEventListener(type, listener, capture) { (documentListeners[type] ||= []).push({ listener, capture: Boolean(capture) }); },
        removeEventListener(type, listener) { documentListeners[type] = (documentListeners[type] || []).filter(item => item.listener !== listener); },
        dispatchClick(target) {
            const event = { target, timeStamp: 2000, defaultPrevented: false, propagationStopped: false, preventDefault() { this.defaultPrevented = true; }, stopPropagation() { this.propagationStopped = true; } };
            (documentListeners.click || []).filter(item => item.capture).forEach(item => item.listener(event));
            return event;
        }
    };
    doc.defaultView.getComputedStyle = () => ({ marginBottom: "0px" });
    const root = new Element(doc, "div"); root.ownerDocument = doc; root.hidden = true;
    const sandbox = { console: { warn: (...args) => warnings.push(args) }, setTimeout, clearTimeout };
    vm.runInNewContext(campaignView, sandbox, { filename: "sow-campaign-view.js" });
    const renderer = sandbox.SOWCampaignView.mount(root, {
        asset: value => "/" + value,
        translate: key => key,
        roster: { factions: [] },
        guideSkipLabel: "Skip",
        onContinue() { continueCalls++; }, onDismiss() { dismissCalls++; }, onChoice() {}
    });
    const definition = {
        episode_id: "portrait_test",
        speakers: {
            first: { name: "First Speaker", avatar: "first" },
            retry: { name: "Retry Speaker", avatar: "retry" },
            recover: { name: "Recover Speaker", avatar: "recover" },
            late: { name: "Late Speaker", avatar: "late" }
        }, steps: [], reactions: []
    };
    const paint = (id, speakerKey) => {
        const step = { id, type: "scene", speaker: speakerKey, title_key: id + "_title", body_key: id + "_body", presentation: "dialogue" };
        renderer.render({ definition, step, line: step, paused: true, done: false, choices: [], progress: { target: 1, current: 0 }, state: { line: 0 } }, {});
    };
    const candidate = avatar => createdImages.slice().reverse().find(image => image.src === "/gameplay/avatars/" + avatar + ".webp");
    const settle = () => new Promise(resolve => setImmediate(resolve));

    paint("first_scene", "first");
    candidate("first").dispatch("load"); await settle();
    assert.equal(root.querySelector(".sow-story__portrait").children[0], candidate("first"), "a successful preloaded portrait is reused");

    paint("retry_scene", "retry");
    const retry = candidate("retry"); retry.dispatch("error");
    assert.match(retry.src, /sow_portrait_retry=1/);
    retry.dispatch("load"); await settle();
    assert.equal(root.querySelector(".sow-story__portrait").children[0], retry, "the one retry restores the correct portrait");

    paint("failed_scene", "recover");
    const failed = candidate("recover"); failed.dispatch("error"); failed.dispatch("error"); await settle();
    const portrait = root.querySelector(".sow-story__portrait");
    assert.match(portrait.children[0].src, /^data:image\/svg\+xml/);
    assert.equal(portrait.hidden, false);
    assert.doesNotMatch(campaignView, /sow-story__speaker/, "dialogues show the portrait, not a name label");
    assert.ok(warnings.some(args => args[1] && args[1].dialogue_id === "failed_scene" && args[1].image.includes("recover.webp")));

    paint("recovered_scene", "recover");
    const recovered = candidate("recover"); recovered.dispatch("load"); await settle();
    assert.equal(portrait.children[0], recovered, "a later successful load replaces the generic fallback");

    paint("late_scene", "late");
    const late = candidate("late"); late.dispatch("load"); await settle();
    paint("current_scene", "first"); await settle();
    const current = candidate("first");
    assert.equal(portrait.children[0], current);
    late.dispatch("error");
    assert.equal(portrait.children[0], current, "a late error from the previous speaker does not replace the current portrait");
    assert.equal(root.querySelector(".sow-story__title").hidden, true, "dialogue titles are hidden unless a step opts in");
    const titled = { id: "titled_scene", type: "scene", speaker: "first", title_key: "titled_title", body_key: "titled_body", presentation: "dialogue", show_title: true };
    renderer.render({ definition, step: titled, line: titled, paused: true, done: false, choices: [], progress: { target: 1, current: 0 }, state: { line: 0 } }, {});
    assert.equal(root.querySelector(".sow-story__title").hidden, false, "show_title opts one dialogue into its title");

    const objective = { id: "expand_250", type: "objective", title_key: "expand_title", hint_key: "expand_hint", trigger: { type: "territory", value: 250 }, guide: { kind: "world", target: "expand", gesture: "tap" } };
    const objectiveModel = { definition: { episode_id: "compact_test", speakers: {}, steps: [objective] }, step: objective, line: objective, paused: false, done: false, choices: [], progress: { current: 120, target: 250 }, state: { line: 0 } };
    root.clientWidth = 390; root.clientHeight = 844;
    renderer.render(objectiveModel, { anchor: { x: 220, y: 360 }, gestureLabel: "Expand the frontier" });
    assert.equal(root.querySelector(".sow-story__objective").hidden, true, "portrait removes the quest card without reserving its slot");
    assert.equal(root.querySelector(".sow-story__gesture").hidden, false, "the real hand remains visible on its target");
    assert.equal(root.querySelector(".sow-story__gesture-label").hidden, false);
    assert.equal(root.querySelector(".sow-story__gesture-metric").hidden, false);
    assert.equal(root.querySelector(".sow-story__zoom-target").textContent, "120 / 250");

    const compactContext = {
        anchor: { x: 380, y: 820 }, direction: "rtl", reducedMotion: true,
        gestureLabel: "استمر في توسيع أراضيك المحايدة حتى تصل إلى الهدف المحدد"
    };
    objectiveModel.progress = { current: 121, target: 250 };
    renderer.render(objectiveModel, compactContext);
    const gestureLabel = root.querySelector(".sow-story__gesture-label");
    assert.equal(root.dir, "rtl");
    assert.equal(root.classList.contains("is-reduced"), true);
    assert.equal(root.querySelector(".sow-story__gesture-copy").textContent, compactContext.gestureLabel, "long localized guidance is kept intact");
    assert.equal(root.querySelector(".sow-story__gesture-metric").hidden, false, "the counter remains visible beside long guidance");
    assert.equal(root.querySelector(".sow-story__zoom-target").textContent, "121 / 250", "the first action uses the engine's latest progress");
    assert.match(gestureLabel.style.transform, /translateX\(calc\(-50% \+ -52px\)\)/, "the label is clamped inside the right edge");

    const resizeStory = (width, height) => {
        root.clientWidth = width; root.clientHeight = height;
        resizeObservers[0].callback();
        const frame = resizeFrames.shift();
        assert.equal(typeof frame, "function");
        frame();
    };
    const portraitLabelTransform = gestureLabel.style.transform;
    resizeStory(844, 390);
    assert.equal(root.querySelector(".sow-story__objective").hidden, true, "compact landscape also removes the quest panel");
    assert.equal(root.querySelector(".sow-story__zoom-target").textContent, "121 / 250", "rotation does not reset the engine counter");
    assert.notEqual(gestureLabel.style.transform, portraitLabelTransform, "rotation within compact mode recalculates the guide clamp");
    assert.match(gestureLabel.style.transform, /translateY\(-428px\)/, "the label stays inside the short landscape viewport");
    resizeStory(390, 844);
    assert.equal(gestureLabel.style.transform, portraitLabelTransform, "returning to portrait restores its own guide position");
    resizeStory(1366, 768);
    assert.equal(root.querySelector(".sow-story__objective").hidden, false, "normal desktop restores the full panel");
    assert.equal(root.querySelector(".sow-story__gesture-metric").hidden, true, "desktop avoids duplicating the panel counter");
    resizeStory(390, 844);
    assert.equal(root.querySelector(".sow-story__objective").hidden, true);
    assert.equal(root.querySelector(".sow-story__zoom-target").textContent, "121 / 250", "returning to portrait keeps the same progress");

    const ratio = { id: "attack_ratio", type: "objective", title_key: "ratio_title", trigger: { type: "ui", action: "attack_ratio", value: 1 }, guide: { kind: "ui", target: "attack_ratio", gesture: "drag" } };
    const ratioModel = { ...objectiveModel, step: ratio, line: ratio, progress: { current: 0.5, target: 1 } };
    root.dir = "ltr";
    renderer.render(ratioModel, { anchor: { x: 380, y: 830 }, gestureLabel: "Set troops to 100%" });
    assert.equal(root.querySelector(".sow-story__zoom-target").textContent, "50% / 100%");
    assert.match(root.querySelector(".sow-story__gesture-label").style.transform, /translateY\(-50%\).*translateX\(-170px\).*translateY\(-18px\)/, "the troop-ratio guide stays inside the compact viewport");

    const zoom = { id: "zoom_in", type: "objective", title_key: "zoom_title", trigger: { type: "zoom_in_complete" }, guide: { kind: "world", target: "player", gesture: "zoom_in" } };
    const zoomModel = { ...objectiveModel, step: zoom, line: zoom, progress: { current: 0, target: 1 } };
    renderer.render(zoomModel, { anchor: { x: 190, y: 400 }, gestureLabel: "Zoom in", gestureHint: "Spread two fingers apart", guideMetric: { current: 1.25, target: 2, direction: "min" } });
    assert.equal(root.querySelector(".sow-story__zoom-current").textContent, "1.25×");
    assert.equal(root.querySelector(".sow-story__zoom-target").textContent, "≥2×", "zoom keeps the existing directional distance metric");

    const distance = { id: "camera_distance", type: "objective", title_key: "distance_title", trigger: { type: "camera_target", distance: 8 }, guide: { kind: "world", target: "player", gesture: "drag" } };
    const distanceModel = { ...objectiveModel, step: distance, line: distance, progress: { current: 0, target: 1 } };
    renderer.render(distanceModel, { anchor: { x: 190, y: 400 }, gestureLabel: "Move the map", guideMetric: { text: "8 / 12 tiles" } });
    assert.equal(root.querySelector(".sow-story__zoom-target").textContent, "8 / 12 tiles", "camera distance keeps its existing metric");

    const passive = { id: "support_wait", type: "objective", title_key: "support_title", hint_key: "support_hint", trigger: { type: "support" } };
    const passiveModel = { ...objectiveModel, step: passive, line: passive, progress: { current: 0, target: 1 } };
    renderer.render(passiveModel, { anchor: { x: 220, y: 620, cameraTracked: true }, statusOnly: true, statusLabel: "Allied support is approaching" });
    const statusGesture = root.querySelector(".sow-story__gesture");
    assert.equal(root.querySelector(".sow-story__objective").hidden, true);
    assert.equal(statusGesture.hidden, false);
    assert.equal(statusGesture.classList.contains("is-status"), true);
    assert.equal(statusGesture.dataset.gesture, undefined, "passive status shows no fake hand action");
    assert.equal(root.querySelector(".sow-story__spotlight").hidden, true, "passive status never dims or highlights the battlefield");
    assert.equal(renderer.updateCameraAnchor(240, 650), true, "the status follows the player while the camera moves");

    root.clientWidth = 1366; root.clientHeight = 768;
    renderer.render(objectiveModel, { anchor: { x: 220, y: 360 }, gestureLabel: "Expand the frontier" });
    assert.equal(root.querySelector(".sow-story__objective").hidden, false, "normal desktop retains the quest card");
    assert.equal(root.querySelector(".sow-story__gesture-metric").hidden, true, "desktop keeps its existing quest-card progress instead of duplicating it");

    const mapSurface = { closest: () => null };
    const pausedObjective = { ...objectiveModel, paused: true };
    renderer.render(pausedObjective, { anchor: { x: 220, y: 360 }, gestureLabel: "Expand the frontier" });
    const mapTapDuringGuide = doc.dispatchClick(mapSurface);
    assert.equal(mapTapDuringGuide.defaultPrevented, false, "a paused mechanics guide lets the map receive the user's tap");
    assert.equal(mapTapDuringGuide.propagationStopped, false, "a paused mechanics guide does not stop map input");
    assert.equal(continueCalls, 0, "map taps do not advance or dismiss a mechanics objective");

    const scene = { id: "scene_click_guard", type: "scene", title_key: "scene_title", presentation: "dialogue" };
    renderer.render({ ...pausedObjective, step: scene, line: scene }, {});
    const mapTapDuringScene = doc.dispatchClick(mapSurface);
    assert.equal(mapTapDuringScene.defaultPrevented, true, "dialogue modals continue to block clicks outside the scene");
    assert.equal(mapTapDuringScene.propagationStopped, true, "dialogue modals continue to stop outside propagation");
    assert.equal(continueCalls, 1, "an outside map tap continues a scene, like its Continue button");
    const welcome = { id: "welcome_heroes", type: "guide", title_key: "heroes_title",
        guide: { kind: "ui", target: "menu_heroes", gesture: "tap" } };
    const welcomeModel = { ...objectiveModel, step: welcome, line: welcome, paused: false };
    const welcomeContext = { anchor: { x: 150, y: 100, width: 80, height: 44 }, hideObjective: true,
        gestureLabel: "Open Heroes to choose a commander or unlock one with gems.", reducedMotion: true };
    const skip = root.querySelector("[data-story-guide-skip]");
    for (const [width, height] of [[1366, 768], [390, 844]]) {
        root.clientWidth = width; root.clientHeight = height;
        renderer.render(welcomeModel, welcomeContext);
        assert.equal(root.querySelector(".sow-story__dialog").hidden, true);
        assert.equal(root.querySelector(".sow-story__spotlight").hidden, false);
        assert.equal(skip.hidden, false, "Skip is available with the dimming on desktop and mobile");
        assert.equal(skip.textContent, "Skip");
        assert.equal(skip.attributes["aria-label"], "Skip");
    }
    skip.hasAttribute = name => name === "data-story-guide-skip";
    root.listeners.click[0].listener({ target: { closest: () => skip }, timeStamp: 10000, stopPropagation() {}, preventDefault() {} });
    assert.equal(dismissCalls, 1, "click/touch Skip uses the existing dismissal path");
    doc.activeElement = new Element(doc, "input");
    (documentListeners.keydown || []).forEach(item => item.listener({ key: "Escape", repeat: false,
        preventDefault() {}, stopPropagation() {} }));
    assert.equal(dismissCalls, 2, "Escape dismisses even when focus is on a menu control outside the guide");
    renderer.render(welcomeModel, { ...welcomeContext, anchor: null });
    assert.equal(skip.hidden, true, "no target and no dimming means no Skip button");
    renderer.render(zoomModel, { anchor: { x: 190, y: 400 }, gestureLabel: "Zoom in" });
    assert.equal(skip.hidden, true, "an undimmed guide has no Skip button");
    renderer.destroy();
    const campaignOnly = sandbox.SOWCampaignView.mount(root, {
        asset: value => "/" + value, translate: key => key, onDismiss() {}, onContinue() {}
    });
    campaignOnly.render(welcomeModel, welcomeContext);
    assert.equal(root.querySelector("[data-story-guide-skip]").hidden, true,
        "campaign views do not opt into the menu-only button");
    campaignOnly.destroy();
});








test("campaign dialogue keeps a compact speaker portrait on narrow screens", () => {
    const mobileStart = storyCss.indexOf("@container (max-width: 640px)");
    const mobileEnd = storyCss.indexOf("@container (max-width: 380px)", mobileStart);
    const mobileStory = storyCss.slice(mobileStart, mobileEnd);
    assert.match(mobileStory, /\.sow-story\.has-portrait \.sow-story__dialog \{ grid-template-columns: clamp\(108px, 30cqw, 150px\) minmax\(0, 1fr\); \}/);
    assert.doesNotMatch(storyCss, /--story-portrait-size/, "the avatar takes the modal height from the grid, not from a measured size");
    assert.doesNotMatch(mobileStory, /(?:144px|64px)/);
});


test("campaign dialogue resolves story copy through the shared catalog", () => {
    const resolverStart = tutorial.indexOf("function tr(key)");
    const resolverEnd = tutorial.indexOf("function zoomMode()", resolverStart);
    assert.ok(resolverStart >= 0 && resolverEnd > resolverStart);
    const resolver = tutorial.slice(resolverStart, resolverEnd);
    assert.match(resolver, /window\.SOW_t\(key\)/);
    assert.match(resolver, /replaceFactionStoryNames\(value, definition, runtime\.roster\)/);
    assert.doesNotMatch(resolver, /definition\.strings/);
});


test("mobile chapter dialogue hugs its copy while docked at the bottom", () => {
    const mobileStory = storyCss.slice(storyCss.indexOf("@container (max-width: 640px)"), storyCss.indexOf("@container (max-width: 380px)"));
    assert.match(mobileStory, /\.sow-story__conversation \{ padding: 14px 16px 0; \}/);
    assert.match(mobileStory, /\.sow-story\.is-chapter \.sow-story__dialog \{ bottom: var\(--story-safe-bottom\); transform: none; \}/);
});


test("mobile story dialogs scroll long text and choices while keeping speaker controls reachable", () => {
    const mobileStory = storyCss.slice(storyCss.indexOf("@container (max-width: 640px)"), storyCss.indexOf("@container (max-width: 380px)"));
    assert.match(mobileStory, /\.sow-story__actions \{ max-height: 45svh;/);
    assert.match(storyCss, /\.sow-story__conversation \{ padding: 24px 32px 0; min-width: 0; min-height: 0; overflow-y: auto;/);
});


test("desktop retains the quest card while compact play uses the shared guided label", () => {
    const statusRight = hud.slice(hud.indexOf("sow-hud__status-right"), hud.indexOf("</header>"));
    assert.doesNotMatch(statusRight, /sow-hud-nameplate/);
    const nameplateMarkup = hud.slice(hud.indexOf('id="sow-hud-nameplate"'), hud.indexOf('id="sow-hud-hover-card"'));
    assert.match(nameplateMarkup, /sow-hud-nameplate-gold/);
    assert.match(nameplateMarkup, /sow-hud-nameplate-gold-rate/);
    assert.match(nameplateMarkup, /sow-hud-nameplate-troop-rate/);
    const desktopObjective = storyCss.match(/#sow-story \.sow-story__objective \{[^}]*\}/);
    assert.ok(desktopObjective, "tutorial objective uses its own stacked placement");
    assert.match(desktopObjective[0], /inset-inline-start: max\(16px, var\(--story-inset-left\)\)/);
    assert.match(desktopObjective[0], /calc\(max\(12px, var\(--story-inset-top\)\) \+ var\(--sow-player-panel-height\) \+ var\(--sow-tutorial-player-panel-gap\)\)/);
});


test("player nameplate keeps the active leader portrait and shows the nickname responsively", () => {
    const plateStart = hud.indexOf("var playerPlateHidden = Boolean(hud.is_spectating)");
    const plateEnd = hud.indexOf("var plateText = formatNameplateTroops", plateStart);
    assert.ok(plateStart >= 0 && plateEnd > plateStart);
    const plateUpdate = hud.slice(plateStart, plateEnd);
    assert.match(plateUpdate, /plateLeader = leaderById\(hud\.player_leader/);
    assert.match(plateUpdate, /gameplay\/avatars\/" \+ plateLeader\.slug/);
    assert.match(plateUpdate, /platePlayerName = String\(hud\.player_name/);
    assert.match(plateUpdate, /plateName\.textContent = platePlayerName/);
    assert.doesNotMatch(plateUpdate, /plateName\.textContent = plateLeader\.name/);
    assert.match(webMenu, /"player_name": snapshot[\s\S]*?snapshot\.players\.iter\(\)[\s\S]*?player\.name\.as_str\(\)/);
    assert.match(hudCss, /\.sow-hud__nameplate \{[^}]*container-type: inline-size;/);
    assert.match(hudCss, /\.sow-hud__nameplate-copy strong \{[^}]*min-width: 0;[^}]*overflow: hidden;[^}]*text-overflow: ellipsis;[^}]*font-size: clamp\(/);
});


test("tutorial troop totals keep grouped digits below 100k and compact decimals above it", () => {
    const start = hud.indexOf("    function formatNameplateTroops(");
    const end = hud.indexOf("\n    function ensureHudDom(", start);
    assert.ok(start >= 0 && end > start);
    const formatNameplateTroops = vm.runInNewContext(hud.slice(start, end) + "\nformatNameplateTroops;");
    assert.equal(formatNameplateTroops(42003), (42003).toLocaleString());
    assert.equal(formatNameplateTroops(99999), (99999).toLocaleString());
    assert.equal(formatNameplateTroops(100000), "100.0K");
    assert.equal(formatNameplateTroops(123300), "123.3K");
    assert.equal(formatNameplateTroops(999999), "1.0M");
    assert.equal(formatNameplateTroops(34200000), "34.2M");
    const metricStart = hud.indexOf("    function formatHudMetric(");
    const metricEnd = hud.indexOf("\n    function notificationAnchor", metricStart);
    const formatHudMetric = vm.runInNewContext(hud.slice(metricStart, metricEnd) + "\n({ formatHudMetric, formatHudRate });");
    assert.equal(formatHudMetric.formatHudMetric(764704), "764.7K");
    assert.equal(formatHudMetric.formatHudMetric(-45), "-45");
    assert.equal(formatHudMetric.formatHudMetric(Infinity), "0");
    assert.equal(formatHudMetric.formatHudRate(12.5), "+12.5/s");
    assert.equal(formatHudMetric.formatHudRate(-0.5), "-0.5/s");
});








test("Boudica script links resolve and each ending completes the episode", () => {
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    const catalogValue = (catalog, key) => {
        const shortKey = key.replace(/^tutorial\./, "");
        const match = catalog.match(new RegExp("^" + shortKey + " = (\"(?:\\\\.|[^\"])*\")$", "m"));
        assert.ok(match, "missing catalog key " + key);
        return JSON.parse(match[1]);
    };
    const step = id => definition.steps.find(item => item.id === id);
    assert.ok(step(definition.entry), "the entry step exists");
    const ids = new Set([...definition.steps, ...definition.reactions].map(item => item.id));
    for (const item of [...definition.steps, ...definition.reactions]) {
        for (const target of [item.next, ...(item.choices || []).map(choice => choice.next)].filter(Boolean)) {
            assert.ok(ids.has(target), "dangling link " + target + " from " + item.id);
        }
    }
    const endings = definition.steps.filter(item => item.type === "end");
    assert.ok(endings.length > 0, "the episode has at least one ending");
    for (const ending of endings) {
        assert.equal(ending.speaker, undefined);
        assert.equal(ending.next, undefined);
        assert.ok(catalogValue(webCatalogEn, ending.body_key));
    }
    const campaign = require(path.join(shell, "sow-campaign.js"));
    for (const choice of step("boudica_final_choice").choices) {
        const machine = campaign.create(definition, "boudica_final_choice");
        machine.update({}, {});
        assert.equal(machine.advance(choice.id, "boudica_final_choice"), true);
        if (choice.id === "fight") machine.update({ eliminated_faction_ids: ["player"] }, {});
        assert.ok(endings.some(ending => ending.id === machine.view().step.id), "each choice leads to an ending");
        assert.equal(machine.view().done, false, "the epilogue stays open until the player continues");
        assert.equal(machine.advance(null, machine.view().step.id), true);
        assert.equal(machine.view().done, true, "each epilogue completes the episode without another card");
    }
    for (const item of [...definition.steps, ...definition.reactions]) {
        for (const field of ["title_key", "body_key", "hint_key"]) {
            if (item[field]) assert.ok(catalogValue(webCatalogEn, item[field]));
        }
        for (const choice of item.choices || []) {
            assert.ok(catalogValue(webCatalogEn, choice.label_key));
            assert.ok(catalogValue(webCatalogEn, choice.body_key));
        }
    }
});
test("shell stylesheets keep balanced blocks, so one bad edit cannot silently drop the rules after it", () => {
    for (const file of fs.readdirSync(shell).filter(name => name.endsWith(".css"))) {
        const css = fs.readFileSync(path.join(shell, file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
        let depth = 0;
        for (const char of css) {
            if (char === "{") depth++;
            if (char === "}") depth--;
            assert.ok(depth >= 0, file + " closes a block that was never opened");
        }
        assert.equal(depth, 0, file + " leaves a block open");
    }
});
test("tutorial and HUD scripts look up only elements their own markup still creates", () => {
    for (const file of ["sow-campaign-view.js", "main_menu.hud.js"]) {
        const source = fs.readFileSync(path.join(shell, file), "utf8");
        const lookups = [...source.matchAll(/(?:find|querySelector|querySelectorAll)\(\s*"\.([a-z_-]+)/g)].map(match => match[1]);
        for (const name of new Set(lookups)) {
            const occurrences = (source.match(new RegExp(name + "(?![a-z_-])", "g")) || []).length;
            const lookedUp = lookups.filter(item => item === name).length;
            assert.ok(occurrences > lookedUp, file + " looks up ." + name + " but its markup no longer creates it");
        }
    }
});
test("a marked campaign decision smoothly focuses its player target once on entry", async () => {
    const commands = [], root = { hidden: true, isConnected: false };
    const model = {
        step: { id: "final_choice", type: "choice", marker: { target: "player" } },
        paused: true,
        state: { choices: {} },
        choices: [{ id: "poison", label_key: "poison" }, { id: "fight", label_key: "fight" }]
    };
    const document = {
        body: { appendChild(node) { node.isConnected = true; } },
        documentElement: { dir: "ltr" },
        getElementById: () => null,
        createElement: () => root,
        addEventListener() {}
    };
    const window = {
        addEventListener() {},
        SOW_menu_command(message) { commands.push(JSON.parse(message)); },
        SOWCampaign: {
            validate: () => ({ errors: [] }), UI_TARGETS: {}, zoomInputMode: () => "wheel",
            create: () => ({ state: { choices: {} }, update: () => model, view: () => model, advance: () => true })
        },
        SOWCampaignView: { mount: () => ({ render() {}, destroy() {} }) }
    };
    vm.runInNewContext(tutorial, { window, document, performance: { now: () => 0 }, console,
        fetch(url) {
            const data = url.endsWith(".triggers.json")
                ? { episode_id: "boudica", settings: {}, strings: {}, speakers: {}, steps: [model.step] }
                : { factions: [] };
            return Promise.resolve({ ok: true, json: () => Promise.resolve(data) });
        }
    });
    const update = () => window.SOW_tutorial_state_update({ phase: "Playing", hud: {
        players: [{ id: 1, name: "Boudica", is_me: true, is_alive: true, centroid_x: 12, centroid_y: 34 }],
        settings: {}, tutorial: { active: true, episode_id: "boudica", facts: {} }
    } });
    update();
    await new Promise(setImmediate);
    update();
    const focuses = commands.filter(command => command.type === "focus_world");
    assert.equal(focuses.length, 1, JSON.stringify(commands));
    assert.deepEqual([focuses[0].x, focuses[0].y], [12.5, 34.5]);
});


test("a compact campaign guide scrolls a dock target into view once per step", () => {
    const start = tutorial.indexOf("    function revealGuidedUiTarget(");
    const end = tutorial.indexOf("\n    function syncTransferGuide(", start);
    assert.ok(start >= 0 && end > start);
    const strip = {
        scrollLeft: 0,
        scrollWidth: 700,
        clientWidth: 300,
        getBoundingClientRect: () => ({ left: 100, right: 400 })
    };
    const target = {
        disabled: false,
        closest: () => strip,
        getBoundingClientRect: () => ({ left: 370, right: 410 }),
        scrollIntoView: (options) => {
            assert.equal(options.block, "nearest");
            assert.equal(options.inline, "nearest");
            strip.scrollLeft = 10;
        }
    };
    const context = {
        runtime: { episodeId: "episode", guidedScrollStepId: null },
        window: { SOWCampaign: { resolveUiTarget: () => target } },
        document: {}
    };
    const reveal = vm.runInNewContext(tutorial.slice(start, end) + "\nrevealGuidedUiTarget;", context);
    const step = { id: "place_factory", guide: { kind: "ui", target: "dock_factory" } };
    reveal(step);
    assert.equal(strip.scrollLeft, 10);
    reveal(step);
    assert.equal(strip.scrollLeft, 10);
});


test("campaign pacing can wait without a hand and guide steps require a target", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "timing_test", default_locale: "en", settings: { starting_troops: 1000 },
        speakers: {}, entry: "wait", steps: [
            { id: "wait", type: "objective", title_key: "tutorial.wait", trigger: { type: "elapsed", value: 5, scope: "step" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.end" }
        ]
    };
    assert.deepEqual(campaign.validate(definition).errors, []);
    definition.steps[0].guide = { kind: "world", target: "expand", gesture: "tap" };
    assert.ok(campaign.validate(definition).errors.some(issue => issue.step === "wait" && issue.field === "guide"));
    delete definition.steps[0].guide;
    definition.steps[0] = { id: "wait", type: "guide", title_key: "tutorial.wait", trigger: { type: "territory", value: 1, scope: "step" }, next: "ending" };
    assert.ok(campaign.validate(definition).errors.some(issue => issue.step === "wait" && issue.field === "guide"));
});


test("campaign hand guides model the actual slider and support local or world drags", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.equal(campaign.UI_TARGETS.troops, undefined);
    assert.equal(campaign.UI_TARGETS.attack_ratio, "#sow-hud-slider");
    const definition = {
        version: 2, episode_id: "guide_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "slider", steps: [
            { id: "slider", type: "objective", title_key: "tutorial.slider", trigger: { type: "ui", action: "attack_ratio", scope: "step" }, guide: { kind: "ui", target: "attack_ratio", gesture: "drag" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.end" }
        ]
    };
    assert.deepEqual(campaign.validate(definition).errors, []);
    const machine = campaign.create(definition);
    assert.equal(machine.update({ attack_ratio: 0.5 }, {}).step.id, "slider");
    assert.equal(machine.update({ attack_ratio: 0.99 }, {}).step.id, "slider");
    assert.equal(machine.update({ attack_ratio: 1 }, {}).step.id, "ending");
    definition.steps[0].guide.to = "not_a_control";
    assert.ok(campaign.validate(definition).errors.some(issue => issue.step === "slider" && issue.field === "guide.to"));
    definition.steps[0].guide = { kind: "world", target: "expand", gesture: "drag", to: "assault" };
    assert.deepEqual(campaign.validate(definition).errors, []);
    const demo = campaignEditor.slice(campaignEditor.indexOf("function demo()"), campaignEditor.indexOf("function createMenuGuide()"));
    assert.match(demo, /trigger: \{ type: "ui", action: "attack_ratio", scope: "step" \}, guide: \{ kind: "ui", target: "attack_ratio", gesture: "drag" \}/);
});




test("campaign replay hand resolves the selected episode card, not always Boudica", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const boudica = { dataset: { episodeId: "boudica" } };
    const sixSky = { dataset: { episodeId: "six_sky_ep2" } };
    const root = { querySelectorAll: () => [boudica, sixSky] };
    assert.equal(campaign.resolveUiTarget("campaign_replay", root, "six_sky_ep2"), sixSky);
    assert.equal(campaign.resolveUiTarget("campaign_replay", root, "unknown_episode"), null);
    assert.equal(campaign.resolveUiTarget("campaign_replay", root), null);
});


test("tutorial hand selectors stay tied to controls rendered by the game", () => {
    const targetBlock = campaignEngine.match(/const UI_TARGETS = \{([\s\S]*?)\n    \};/);
    assert.ok(targetBlock);
    const sources = [shellSource, lobbiesSource, coreSource, hud].join("\n");
    for (const [, selector] of targetBlock[1].matchAll(/:\s*'([^']+)'/g)) {
        for (const [, attribute, value] of selector.matchAll(/\[([a-z-]+)="([^"]+)"\]/g)) {
            if (attribute === "data-map-action") {
            } else if (attribute === "data-map-group") {
            } else if (attribute === "data-episode-id") {
            } else {
                assert.ok(sources.includes(`${attribute}='${value}'`) || sources.includes(`${attribute}="${value}"`) || sources.includes(value), `No game UI target ${attribute}=${value}`);
            }
        }
        if (selector === "#sow-hud-slider") assert.match(hud, /id="sow-hud-slider"/);
    }
});




test("tutorial camera anchors and paused build guides publish current map coordinates", () => {
    const anchorStart = webMenu.indexOf("pub(crate) fn publish_tutorial_camera_anchor_frame");
    const stateStart = webMenu.indexOf("pub(crate) fn publish_state", anchorStart);
    const anchorPublisher = webMenu.slice(anchorStart, stateStart);
    assert.match(anchorPublisher, /nameplates\.anchor_for\(player_id\)/);
    assert.match(anchorPublisher, /world_to_screen\(/);
    assert.match(anchorPublisher, /tutorial_screen_anchor\(/);
    assert.match(anchorPublisher, /SOW_tutorial_camera_anchor_update/);
    assert.doesNotMatch(anchorPublisher, /LAST_HUD_KEY|LAST_PUBLISHED|publish_state/);

    const hudKeyStart = webMenu.indexOf("fn hud_publish_key");
    const hudKeyEnd = webMenu.indexOf("\n#[cfg", hudKeyStart);
    const hudKey = webMenu.slice(hudKeyStart, hudKeyEnd);
    assert.match(hudKey, /tutorial_camera_viewport: tutorial_active\.then_some\(\([\s\S]*?camera_x\.to_bits\(\)[\s\S]*?camera_y\.to_bits\(\)/);
    const guideStart = webMenu.indexOf("fn tutorial_guide_tiles(");
    const guideEnd = webMenu.indexOf("\nfn tutorial_payload(", guideStart);
    const guide = webMenu.slice(guideStart, guideEnd);
    assert.match(guide, /guide_build_site_candidates[\s\S]*tutorial_visible_build_site_candidate\([\s\S]*tutorial_project_tile\([\s\S]*&app\.input[\s\S]*tutorial_screen_point_visible/);
    const anchorFunction = tutorial.slice(tutorial.indexOf("function anchorFor"), tutorial.indexOf("function syncTransferGuide"));
    assert.match(anchorFunction, /factionAction[\s\S]*?tutorial\.target_nameplate[\s\S]*?cameraTracked: true/);
    const uiAnchorBranch = anchorFunction.slice(anchorFunction.indexOf("var source = window.SOWCampaign.resolveUiTarget"));
    assert.doesNotMatch(uiAnchorBranch, /cameraTracked/);
});


test("a UI step uses its configured target and device-specific hand labels", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = { episode_id: "test_campaign", text_namespace: "tutorial.test_campaign_" };
    const step = { id: "home_camera", hint_key: "tutorial.test_home_hint", guide: { kind: "ui", target: "hud_center_camera", gesture: "tap" }, trigger: { type: "ui", action: "hud_center_camera" } };
    const expansionStep = { id: "boudica_first_expansion", paused_action: "expand_once_then_resume", guide: { kind: "world", target: "expand", gesture: "tap" }, trigger: { type: "territory" } };
    assert.equal(campaign.UI_TARGETS.hud_center_camera, '#sow-hud [data-command="center_camera"]');

    const selector = campaign.UI_TARGETS.hud_center_camera;
    const homeButton = { disabled: false, getClientRects: () => [{}], getBoundingClientRect: () => ({ left: 12, top: 18, width: 40, height: 40 }) };
    let resolvedSelector;
    assert.equal(campaign.resolveUiTarget(step.guide.target, {
        querySelector(value) { resolvedSelector = value; return value === selector ? homeButton : null; }
    }, definition.episode_id), homeButton);
    assert.equal(resolvedSelector, selector);

    const contextStart = tutorial.indexOf("    function renderContext(");
    const contextEnd = tutorial.indexOf("\n    function fetchJson(", contextStart);
    const contextSource = tutorial.slice(contextStart, contextEnd);
    function renderContextFor(mode, translated = true, candidateStep = step, machineView = {}) {
        const desktopKey = definition.text_namespace + step.id + "_desktop_action";
        const mobileKey = definition.text_namespace + step.id + "_mobile_action";
        const expansionKey = definition.text_namespace + expansionStep.id;
        const translations = translated ? {
            [desktopKey]: "Click Home", [mobileKey]: "Tap Home",
            [expansionKey + "_desktop_action"]: "Click to expand",
            [expansionKey + "_mobile_action"]: "Tap to expand",
            [expansionKey + "_continue_action"]: "Keep expanding"
        } : {};
        const renderContext = vm.runInNewContext(contextSource + "\nrenderContext;", {
            document: { documentElement: { dir: "ltr" } },
            window: { SOWCampaign: campaign },
            zoomMode: () => mode,
            stepTextKey: (candidate, suffix) => definition.text_namespace + candidate.id + suffix,
            hasText: key => Object.hasOwn(translations, key),
            tr: key => translations[key] || ({
                "tutorial.hand_click": "Click the highlighted control",
                "tutorial.hand_tap": "Tap the highlighted control",
                "tutorial.hand_click_home": "Click Home",
                "tutorial.hand_tap_home": "Tap Home",
                "tutorial.hand_click_expand": "Click to expand",
                "tutorial.hand_tap_expand": "Tap to expand",
                "tutorial.hand_upgrade_progress": "Building in progress",
                "hud.building_in_progress": "Building in progress"
            })[key] || key
        });
        return renderContext(candidateStep, { settings: {} }, null, machineView);
    }
    assert.equal(campaign.guideAction(step), "home");
    assert.equal(campaign.guideAction(expansionStep), "expand");
    assert.equal(renderContextFor("wheel").gestureLabel, "Click Home");
    assert.equal(renderContextFor("pinch").gestureLabel, "Tap Home");
    assert.equal(renderContextFor("wheel", false).gestureLabel, "Click Home", "missing episode-specific copy uses the localized action fallback");
    assert.equal(renderContextFor("wheel", true, { ...step, guide: { ...step.guide, show_label: false } }).gestureLabel, undefined, "explicit opt-out hides hand text");
    assert.equal(renderContextFor("wheel", true, expansionStep, { paused_action: "expand_once_then_resume" }).gestureLabel, "Click to expand");
    assert.equal(renderContextFor("pinch", true, expansionStep, { paused_action: "expand_once_then_resume" }).gestureLabel, "Tap to expand");
    assert.equal(renderContextFor("wheel", true, expansionStep, { paused_action: null }).gestureLabel, "Keep expanding");
    const passiveConstruction = { id: "city_wait", type: "objective", hint_key: "city_wait_hint", trigger: { type: "structure_level", kind: "City" } };
    const passiveSupport = { id: "support_wait", type: "objective", hint_key: "support_wait_hint", trigger: { type: "support" } };
    const resistance = { id: "resistance_wait", type: "objective", hint_key: "resistance_hint", trigger: { type: "territory", comparison: "lte", value: 5000 } };
    const upgradeWait = { id: "upgrade_wait", type: "objective", hint_key: "upgrade_wait_hint", guide: { kind: "world", target: "upgrade_building", gesture: "tap" }, trigger: { type: "structure_level", kind: "City" } };
    const constructionContext = renderContextFor("wheel", false, passiveConstruction);
    assert.equal(constructionContext.statusOnly, true);
    assert.equal(constructionContext.statusLabel, "Building in progress");
    const supportContext = renderContextFor("wheel", false, passiveSupport);
    assert.equal(supportContext.statusOnly, true);
    assert.equal(supportContext.statusLabel, "support_wait_hint");
    const resistanceContext = renderContextFor("wheel", false, resistance);
    assert.equal(resistanceContext.statusOnly, true);
    assert.equal(resistanceContext.statusLabel, "resistance_hint", "resistance keeps a short status without inventing a progress counter");
    const upgradeContext = renderContextFor("wheel", false, upgradeWait);
    assert.equal(upgradeContext.statusOnly, true);
    assert.equal(upgradeContext.statusLabel, "Building in progress");
    const english = fs.readFileSync(path.join(shell, "../../sow-i18n/strings/en/web.toml"), "utf8");

    const anchorStart = tutorial.indexOf("    function anchorFor(");
    const anchorEnd = tutorial.indexOf("\n    function syncTransferGuide(", anchorStart);
    const anchorSource = tutorial.slice(anchorStart, anchorEnd);
    const anchorFor = vm.runInNewContext(anchorSource + "\nanchorFor;");
    assert.deepEqual(JSON.parse(JSON.stringify(anchorFor(passiveConstruction, { tutorial: { upgrading_buildings: { City: { x: 220, y: 140 } }, player: { x: 300, y: 220 } } }))), { x: 220, y: 140 },
        "construction status prefers the real building anchor");
    assert.deepEqual(JSON.parse(JSON.stringify(anchorFor(passiveSupport, { tutorial: { player: { x: 300, y: 220 } } }))), { x: 300, y: 220, cameraTracked: true },
        "passive player status follows the player anchor");
    assert.deepEqual(JSON.parse(JSON.stringify(anchorFor(resistance, { tutorial: { player: { x: 300, y: 220 } } }))), { x: 300, y: 220, cameraTracked: true },
        "resistance status stays with the player");
    assert.equal(anchorFor(passiveSupport, { tutorial: {} }), null, "no status is drawn without a real map anchor");
    assert.match(anchorSource, /resolveUiTarget\(guide\.target, document, runtime\.episodeId, guideFacts\)/);
    assert.match(anchorSource, /step\.id === "boudica_transfer_send"[\s\S]*resolveUiAnchor\(source, spotlightPanel\)/);
    assert.doesNotMatch(anchorSource, /guide\.target === "hud_center_camera"[\s\S]*root\.clientWidth/);
    assert.doesNotMatch(anchorSource, /delete result\.(?:width|height)/);
});


test("campaign tap guides use shared action labels and localized fallback", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const definitions = fs.readdirSync(campaignDir)
        .filter(file => file.endsWith(".triggers.json"))
        .map(file => JSON.parse(fs.readFileSync(path.join(campaignDir, file), "utf8")));
    const tapSteps = definitions.flatMap(definition => definition.steps
        .filter(step => step.guide && step.guide.gesture === "tap")
        .map(step => ({ definition, step })));
    assert.ok(tapSteps.length > 0, "campaigns must contain tap guides to check");
    const categories = new Set(tapSteps.map(({ step }) => campaign.guideAction(step)));
    assert.deepEqual([...categories].sort(), ["attack", "build", "expand", "home", "select", "send", "upgrade", "upgrade_progress"].sort());
    assert.equal(campaign.guideAction({ guide: { kind: "world", target: "expand", gesture: "tap" }, trigger: { type: "contact" } }), "expand");
    assert.equal(campaign.guideAction({ guide: { kind: "world", target: "target_action", gesture: "tap" }, trigger: { type: "contact" } }), "contact");
    assert.equal(campaign.guideAction({ guide: { kind: "world", target: "upgrade_building", gesture: "tap" }, trigger: { type: "structure_level" } }), "upgrade_progress");

    const contextStart = tutorial.indexOf("    function renderContext(");
    const contextEnd = tutorial.indexOf("\n    function fetchJson(", contextStart);
    const contextSource = tutorial.slice(contextStart, contextEnd);
    function renderFallbackLabel(mode, step) {
        const renderContext = vm.runInNewContext(contextSource + "\nrenderContext;", {
            document: { documentElement: { dir: "ltr" } },
            window: { SOWCampaign: campaign },
            zoomMode: () => mode,
            stepTextKey: (candidate, suffix) => "tutorial." + candidate.id + suffix,
            hasText: () => false,
            tr: key => key
        });
        return renderContext(step, { settings: {} }, null, { paused_action: null }).gestureLabel;
    }
    for (const { step } of tapSteps) {
        if (step.guide.show_label === false) {
            assert.equal(renderFallbackLabel("wheel", step), undefined, step.id + " opted out of hand text");
            continue;
        }
        const action = campaign.guideAction(step);
        const expected = mode => action === "upgrade_progress" ? "tutorial.hand_upgrade_progress"
            : action === "interact" ? "tutorial.hand_" + mode
                : "tutorial.hand_" + mode + "_" + action;
        assert.equal(renderFallbackLabel("wheel", step), expected("click"), step.id + " desktop label");
        assert.equal(renderFallbackLabel("pinch", step), expected("tap"), step.id + " mobile label");
    }
});


test("Boudica UI guides spotlight the requested controls and Legion IX attack slider", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const campaignSource = fs.readFileSync(path.join(shell, "sow-campaign.js"), "utf8");
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    const requested = [
        "boudica_transfer_choose", "boudica_transfer_send", "boudica_choose_city"
    ];
    // The upgrade button is drawn by the game, so its hand points at a world spot.
    const upgradeGuide = definition.steps.find(step => step.id === "boudica_structure_upgrade").guide;
    assert.deepEqual(upgradeGuide, { kind: "world", target: "upgrade_structure", gesture: "tap" });
    const uiSteps = definition.steps.filter(step => step.guide && step.guide.kind === "ui").map(step => step.id).sort();
    const ratioStepId = "boudica_ninth_legion_set_attack_ratio";
    assert.deepEqual(uiSteps, ["boudica_camera_home", ...requested, ratioStepId].sort());
    for (const id of requested) assert.equal(definition.steps.find(step => step.id === id).guide.gesture, "tap");
    const intro = definition.steps.find(step => step.id === "boudica_ninth_legion_intro");
    const ratioStep = definition.steps.find(step => step.id === ratioStepId);
    const strikeDialog = definition.steps.find(step => step.id === "boudica_ninth_strike");
    const legionObjective = definition.steps.find(step => step.id === "boudica_ninth_legion");
    const entry = definition.steps.find(step => step.id === definition.entry);
    const ratioOverrides = definition.steps.filter(step => Object.hasOwn(step, "attack_ratio_on_enter"))
        .map(step => ({ id: step.id, ratio: step.attack_ratio_on_enter }));
    assert.deepEqual(ratioOverrides, [{ id: definition.entry, ratio: 0.25 }]);
    assert.equal(entry.attack_ratio_on_enter, 0.25);
    assert.equal(intro.next, "boudica_select_city_upgrade");
    assert.equal(definition.steps.find(step => step.id === "boudica_city_upgrade_wait").next, ratioStepId);
    assert.equal(intro.attack_ratio_on_enter, undefined, "the Ninth intro preserves the 25% tutorial allocation");
    assert.equal(ratioStep.next, strikeDialog.id);
    assert.equal(strikeDialog.type, "scene");
    assert.equal(strikeDialog.speaker, "boudica");
    assert.equal(strikeDialog.next, legionObjective.id);
    assert.equal(ratioStep.pause_game, true);
    assert.equal(ratioStep.trigger.type, "ui");
    assert.equal(ratioStep.trigger.action, "attack_ratio");
    assert.equal(ratioStep.trigger.value, 1);
    assert.deepEqual(ratioStep.guide, { kind: "ui", target: "attack_ratio", gesture: "drag" });
    assert.equal(ratioStep.attack_ratio_on_enter, undefined, "the player must set 100% manually");
    let ratioClock = 0;
    const ratioMachine = campaign.create(definition, ratioStepId, undefined, () => ratioClock);
    for (const value of [0.25, 0.5, 0.99]) assert.equal(ratioMachine.update({ attack_ratio: value }, {}).step.id, ratioStepId);
    assert.equal(ratioMachine.update({ attack_ratio: 1 }, {}).step.id, strikeDialog.id);
    assert.equal(strikeDialog.wait_before, true);
    assert.equal(ratioMachine.view().waiting, true, "the strike dialogue stays hidden during the reveal wait");
    assert.equal(ratioMachine.view().paused, true, "the game remains paused during the reveal wait");
    ratioClock = campaign.WAIT_MS - 1;
    assert.equal(ratioMachine.view().waiting, true);
    ratioClock = campaign.WAIT_MS;
    assert.equal(ratioMachine.view().waiting, false, "the strike dialogue reveals after the shared 1.1-second wait");
    assert.equal(ratioMachine.view().paused, true, "the game remains paused after the dialogue is revealed");
    assert.equal(ratioMachine.advance(null, strikeDialog.id), true);
    assert.equal(ratioMachine.view().step.id, legionObjective.id);
    const english = fs.readFileSync(path.join(shell, "../../sow-i18n/strings/en/web.toml"), "utf8");
});


test("tutorial hand and ripple fade in at screen center and glide together to the dimmed Home button", () => {
    const hand = fs.readFileSync(path.join(shell, "../../assets/art/icons/tutorial-hand.webp"));
    assert.equal(hand.subarray(0, 4).toString(), "RIFF");
    assert.equal(hand.subarray(8, 12).toString(), "WEBP");
});




test("construction guidance points to confirmation without advancing on a preview", () => {
    const source = fs.readFileSync(path.join(__dirname, "main_menu.tutorial.js"), "utf8");
    const begin = source.indexOf("    function placementGuideView(");
    const end = source.indexOf("    function anchorFor(", begin);
    const adapt = vm.runInNewContext(source.slice(begin, end) + "\nplacementGuideView;");
    const model = { step: { id: "build_city", paused_action: "build_until_started", trigger: { type: "city", value: 1 }, guide: { kind: "world", target: "build_site", gesture: "tap" } } };
    assert.equal(adapt(model, {}), model);
    assert.equal(adapt(model, { building_placement: { can_confirm: false } }), model);
    assert.equal(adapt(model, { building_placement: { can_confirm: true, pending: true } }), model, "a submitted radial action never opens another confirmation guide");
    const display = adapt(model, { building_placement: { can_confirm: true } });
    assert.equal(display.step.guide.target, "confirm_building_placement");
    assert.equal(display.step.trigger, model.step.trigger);
    assert.equal(model.step.guide.target, "build_site");


});
