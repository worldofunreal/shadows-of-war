(function () {
    "use strict";

    var $ = function (selector) { return document.querySelector(selector); };
    var query = new URLSearchParams(location.search);
    var episodeId = /^[a-z][a-z0-9_]{0,63}$/.test(query.get("episode") || "") ? query.get("episode") : "boudica";
    var rtlLanguages = new Set(["ar", "arc", "ckb", "dv", "fa", "he", "iw", "nqo", "pnb", "ps", "sd", "syr", "ug", "ur", "yi"]);
    var state = { roster: null, definition: null, etag: null, rosterEtag: null, externalChangeTag: null, dirty: false, saving: false, demoBackup: null, selected: null, previewBranch: null, previewFocusActive: false, previewActionStep: null, previewActionRatio: null, pickingFaction: false, flow: "episode", previewLanguage: "en", catalogs: {}, localeFailures: [], localeRegistryFailed: false, avatars: [], machine: null, renderer: null, facts: {}, ui: {}, mapPreview: null, mapResizeObserver: null, mapResizeFallback: false, mapPreviewLoadingId: null, zoom: 1, pan: { x: 36, y: 44 }, drag: null, wire: null, keyboardWire: null, validation: { errors: [], warnings: [] } };
    var types = ["scene", "choice", "objective", "guide", "end"];
    var triggerTypes = [
        { value: "territory", label: "Gain territory" }, { value: "kills", label: "Defeat troops" },
        { value: "attack", label: "Launch an attack" }, { value: "troops", label: "Reach a troop minimum" },
        { value: "building", label: "Complete a building" }, { value: "city", label: "Complete a city" },
        { value: "farm", label: "Complete a farm" }, { value: "factory", label: "Complete a factory" },
        { value: "port", label: "Complete a port" }, { value: "bunker", label: "Complete a bunker" },
        { value: "structure_upgrade", label: "Upgrade a structure" }, { value: "structure_level", label: "Reach a building level" }, { value: "city_upgrade", label: "Upgrade a city" },
        { value: "city_level", label: "Reach a city level" }, { value: "foundry_level", label: "Reach a foundry level" },
        { value: "port_upgrade", label: "Upgrade a port" }, { value: "port_level", label: "Reach port level" }, { value: "tile_upgrade", label: "Upgrade territory" },
        { value: "resource_transfer", label: "Send resources" }, { value: "alliance", label: "Form an alliance" },
        { value: "support", label: "Receive allied support" }, { value: "fleet", label: "Launch a fleet" },
        { value: "nuke", label: "Launch a nuke" }, { value: "elapsed", label: "Wait for game time" },
        { value: "contact", label: "Reach a faction" }, { value: "defeated", label: "Defeat a faction" }, { value: "eliminated", label: "Faction eliminated" },
        { value: "ui", label: "Use a control" },
        { value: "zoom_in", label: "Zoom in" }, { value: "zoom_out", label: "Zoom out" },
        { value: "camera_drag", label: "Pan camera by dragging" }, { value: "camera_key_pan", label: "Pan camera with keys" },
        { value: "hover", label: "Hover an entity" },
        { value: "zoom_out_complete", label: "Reach zoom-out distance" },
        { value: "zoom_in_complete", label: "Zoom in on a faction" },
        { value: "camera_target", label: "Center the camera on a target" }
    ];
    var locales = ["en", "es"], localeNames = { en: "English", es: "Español" }, catalogLoads = Object.create(null);

    function el(tag, attrs, text) {
        var node = document.createElement(tag);
        Object.keys(attrs || {}).forEach(function (key) {
            if (key === "class") node.className = attrs[key];
            else if (key === "dataset") Object.assign(node.dataset, attrs[key]);
            else if (key === "checked" || key === "disabled" || key === "hidden") node[key] = attrs[key];
            else node.setAttribute(key, attrs[key]);
        });
        if (text != null) node.textContent = text;
        return node;
    }
    function optionList(select, values, current, label) {
        select.replaceChildren();
        values.forEach(function (value) {
            var item = el("option", { value: typeof value === "string" ? value : value.value }, typeof value === "string" ? (label ? label(value) : value) : value.label);
            select.appendChild(item);
        });
        select.value = current;
    }
    function localeOptions() { return locales.map(function (code) { return { value: code, label: localeNames[code] || code.toUpperCase() }; }); }
    function defaultLayout(index) { return { x: 70 + (index % 3) * 280, y: 70 + Math.floor(index / 3) * 190 }; }
    function ensureLayout() {
        state.definition.layout = state.definition.layout || {};
        state.definition.steps.forEach(function (step, index) { state.definition.layout[step.id] = state.definition.layout[step.id] || defaultLayout(index); });
    }
    function normalizeStrings(definition) {
        var legacy = definition.strings;
        if (Array.isArray(legacy) || legacy && typeof legacy === "object" && Object.values(legacy).every(function (catalog) {
            return catalog && typeof catalog === "object" && !Array.isArray(catalog) && Object.keys(catalog).length === 0;
        })) delete definition.strings;
        delete definition.default_locale;
    }
    function catalogValue(locale, key) {
        var parts = String(key || "").split("."), catalog = state.catalogs[locale] || {};
        if (parts[0] === "tutorial") { catalog = catalog.tutorial || {}; parts.shift(); }
        return parts.reduce(function (value, part) { return value && value[part]; }, catalog);
    }
    function textValue(key, locale) {
        var text = catalogValue(locale || state.previewLanguage, key) || catalogValue("en", key) || "";
        return window.SOWCampaign && typeof window.SOWCampaign.replaceFactionStoryNames === "function"
            ? window.SOWCampaign.replaceFactionStoryNames(text, state.definition, state.roster)
            : text;
    }
    function translated(key, locale) {
        return textValue(key, locale || state.previewLanguage) || (key ? "[" + key + "]" : "");
    }
    function previewZoom(value) {
        var number = Number(value);
        return Number.isFinite(number) ? number.toFixed(2).replace(/\.?0+$/, "") + "×" : "";
    }
    function previewMetric(key, values) {
        var text = translated(key, state.previewLanguage);
        Object.keys(values).forEach(function (name) { text = text.replace(new RegExp("\\{" + name + "\\}", "g"), String(values[name])); });
        return text;
    }
    function asset(path) { return "/assets/" + path.split("/").map(encodeURIComponent).join("/"); }
    function setFactionPicker(active) {
        var step = state.definition && state.definition.steps.find(function (item) { return item.id === state.selected; });
        var picker = $("#pickFactionTarget"), wasPicking = state.pickingFaction;
        state.pickingFaction = Boolean(active && step && ["objective", "guide"].includes(step.type));
        picker.disabled = !step || !["objective", "guide"].includes(step.type);
        picker.setAttribute("aria-pressed", String(state.pickingFaction));
        picker.textContent = state.pickingFaction ? "Cancel map pick" : "Pick target on map";
        if (wasPicking !== state.pickingFaction) renderRosterMapPreview();
    }
    function chooseFactionTarget(factionId) {
        var step = state.definition && state.definition.steps.find(function (item) { return item.id === state.selected; });
        if (!step || !["objective", "guide"].includes(step.type)) return;
        if (step.trigger && ["contact", "defeated"].includes(step.trigger.type) && Array.isArray(step.trigger.targets)) {
            if (!step.trigger.targets.includes(factionId)) step.trigger.targets.push(factionId);
            if (step.trigger.type === "defeated") step.trigger.value = step.trigger.targets.length;
        } else if (["contact", "defeated", "attack"].includes(step.trigger && step.trigger.type)) step.trigger.target = factionId;
        else step.marker = { target: factionId };
        setFactionPicker(false);
        markDirty(); renderInspector(); renderPreview(step.id);
    }
    function renderRosterMapPreview() {
        if (!state.roster) return;
        var mapId = state.roster.map, layer = $("#campaignPreviewLayer"), canvas = $("#campaignMapPreview"), markers = $("#campaignPreviewMarkers");
        var label = $("#previewMapLabel"), count = $("#previewFactionCount"), status = $("#previewMapStatus");
        if (state.mapPreview && state.mapPreview.id !== mapId) {
            state.mapPreview = null; canvas.width = 1; canvas.height = 1; markers.replaceChildren();
        }
        label.textContent = mapId.replace(/_/g, " ").toUpperCase();
        count.textContent = state.roster.factions.length + " FACTIONS";
        function draw(map) {
            var width = layer.parentElement.clientWidth, height = layer.parentElement.clientHeight, scale = Math.min(width / map.width, height / map.height);
            layer.style.width = Math.max(1, map.width * scale) + "px";
            layer.style.height = Math.max(1, map.height * scale) + "px";
            canvas.width = map.width; canvas.height = map.height;
            canvas.getContext("2d").drawImage(map.image, 0, 0);
            markers.replaceChildren();
            state.roster.factions.forEach(function (faction) {
                var marker = el("span", { class: "sample-map-marker sample-faction", title: faction.name, "aria-label": faction.name, dataset: { factionId: faction.id } });
                marker.style.left = (Number(faction.x) / map.width * 100) + "%";
                marker.style.top = (Number(faction.y) / map.height * 100) + "%";
                if ($("#factionLabels").checked || state.pickingFaction) {
                    marker.dataset.labelSide = Number(faction.x) / map.width > 0.78 ? "left" : "right";
                    marker.appendChild(el("span", { class: "sample-faction-name" }, faction.name));
                }
                if (state.pickingFaction) {
                    marker.classList.add("is-faction-pickable");
                    marker.tabIndex = 0;
                    marker.setAttribute("role", "button");
                    marker.setAttribute("aria-label", "Set selected step target to " + faction.name);
                }
                markers.appendChild(marker);
            });
            var spawn = el("span", { class: "sample-map-marker player-base", title: "Player spawn", "aria-label": "Player spawn" });
            spawn.style.left = (Number(state.roster.player_spawn[0]) / map.width * 100) + "%";
            spawn.style.top = (Number(state.roster.player_spawn[1]) / map.height * 100) + "%";
            markers.appendChild(spawn);
            status.hidden = true;
            applyPreviewFocus();
        }
        function resize() { if (state.mapPreview && state.mapPreview.id === mapId) draw(state.mapPreview); }
        if (typeof ResizeObserver !== "undefined") {
            if (!state.mapResizeObserver) state.mapResizeObserver = new ResizeObserver(resize);
            state.mapResizeObserver.disconnect(); state.mapResizeObserver.observe(layer.parentElement);
        } else if (!state.mapResizeFallback) { window.addEventListener("resize", resize, { passive: true }); state.mapResizeFallback = true; }
        if (state.mapPreview && state.mapPreview.id === mapId) { draw(state.mapPreview); return; }
        if (state.mapPreviewLoadingId === mapId) return;
        state.mapPreviewLoadingId = mapId;
        status.textContent = "Loading campaign map…"; status.hidden = false;
        window.SOWCampaignMapPreview.load(mapId).then(function (map) {
            if (!state.roster || state.roster.map !== mapId) return;
            state.mapPreview = map; draw(map);
        }).catch(function () {
            if (state.roster && state.roster.map === mapId) status.textContent = "Map preview unavailable";
        }).then(function () {
            if (state.mapPreviewLoadingId === mapId) state.mapPreviewLoadingId = null;
        });
    }
    function valid() {
        if (!state.definition || !state.roster) return { errors: [], warnings: [] };
        return window.SOWCampaign.validate(state.definition, state.roster, {
            hasText: function (key) { return Boolean(catalogValue("en", key)); },
            hasAvatar: function (avatar) { return state.avatars.includes(avatar); }
        });
    }
    function markDirty() {
        if (state.demoBackup) return;
        state.dirty = true;
        var status = $("#saveStatus");
        status.textContent = "Unsaved changes";
        status.className = "status dirty";
        if (state.renderer) { refresh(true); syncPreview(); renderGraph(); }
        else refresh();
    }
    function status(message, kind) {
        var node = $("#saveStatus");
        node.textContent = message;
        node.className = "status" + (kind ? " " + kind : "");
    }
    function notice(message) {
        var node = $("#notice");
        node.textContent = message;
        node.hidden = !message;
    }
    function setPath(object, path, value) {
        var parts = path.split("."), target = object;
        for (var i = 0; i < parts.length - 1; i++) target = target[parts[i]];
        if (value == null) delete target[parts[parts.length - 1]];
        else target[parts[parts.length - 1]] = value;
    }
    function uniqueId(base, values) {
        var id = base, suffix = 2;
        while (values.some(function (value) { return value.id === id; })) id = base + "_" + suffix++;
        return id;
    }
    function reorderControls(items, index, label) {
        var controls = el("div", { class: "order-actions" });
        [{ offset: -1, glyph: "↑", direction: "up" }, { offset: 1, glyph: "↓", direction: "down" }].forEach(function (move) {
            var button = el("button", { type: "button", "aria-label": "Move " + label + " " + (index + 1) + " " + move.direction }, move.glyph);
            button.disabled = index + move.offset < 0 || index + move.offset >= items.length;
            button.addEventListener("click", function () {
                var item = items.splice(index, 1)[0];
                items.splice(index + move.offset, 0, item);
                markDirty(); renderInspector();
                var nextLabel = "Move " + label + " " + (index + move.offset + 1) + " " + move.direction;
                var nextButton = Array.from($("#inspector").querySelectorAll(".order-actions button")).find(function (candidate) { return candidate.getAttribute("aria-label") === nextLabel; });
                if (!nextButton || nextButton.disabled) {
                    var otherDirection = move.direction === "up" ? "down" : "up";
                    nextButton = Array.from($("#inspector").querySelectorAll(".order-actions button")).find(function (candidate) { return candidate.getAttribute("aria-label") === "Move " + label + " " + (index + move.offset + 1) + " " + otherDirection; });
                }
                if (nextButton && !nextButton.disabled) nextButton.focus();
            });
            controls.appendChild(button);
        });
        return controls;
    }
    function inputField(label, value, onChange, options) {
        options = options || {};
        var wrapper = el("label", { class: options.class || "" });
        wrapper.appendChild(el("span", {}, label));
        var input = el(options.multiline ? "textarea" : "input", { type: options.type || "text", value: value == null ? "" : value, placeholder: options.placeholder || "" });
        if (options.min != null) input.min = options.min;
        if (options.max != null) input.max = options.max;
        if (options.step != null) input.step = options.step;
        if (options.multiline) input.value = value || "";
        input.addEventListener(options.input ? "input" : "change", function () { onChange(input.value); });
        wrapper.appendChild(input);
        return wrapper;
    }
    function selectField(label, current, choices, onChange) {
        var wrapper = el("label", {}); wrapper.appendChild(el("span", {}, label));
        var select = el("select", {});
        choices.forEach(function (choice) {
            var value = typeof choice === "string" ? choice : choice.value;
            select.appendChild(el("option", { value: value }, typeof choice === "string" ? choice : choice.label));
        });
        select.value = current || "";
        select.addEventListener("change", function () { onChange(select.value); });
        wrapper.appendChild(select); return wrapper;
    }
    function multiFactionField(label, selected, onChange) {
        var wrapper = el("label", {}); wrapper.appendChild(el("span", {}, label));
        var select = el("select", { multiple: "multiple", size: Math.min(8, Math.max(3, state.roster.factions.length)) });
        factionOptions().forEach(function (option) {
            var item = el("option", { value: option.value }, option.label);
            item.selected = selected.includes(option.value); select.appendChild(item);
        });
        select.addEventListener("change", function () { onChange(Array.from(select.selectedOptions).map(function (item) { return item.value; })); });
        wrapper.appendChild(select); return wrapper;
    }
    function checkboxField(label, checked, onChange) {
        var wrapper = el("label", { class: "check" });
        var input = el("input", { type: "checkbox", checked: Boolean(checked) });
        input.addEventListener("change", function () { onChange(input.checked); });
        wrapper.append(input, el("span", {}, label)); return wrapper;
    }
    function section(title) { var node = el("section", { class: "form-section" }); node.appendChild(el("h3", {}, title)); return node; }
    function storyKey(suffix) { return (state.definition.text_namespace || "tutorial.") + suffix; }
    function stepTextKey(step, suffix) { return (state.definition.text_namespace || "tutorial.") + step.id + suffix; }
    function catalogTextField(container, label, key, rows) {
        var field = el("textarea", { "aria-label": label, readonly: "", placeholder: "Add this key to sow-i18n/strings/en/web.toml." });
        field.rows = rows || 4;
        field.value = textValue(key, state.previewLanguage);
        container.appendChild(el("label", {}, label + " · " + state.previewLanguage));
        container.appendChild(field);
        var note = field.value ? "Text is managed by the shared language catalogs." : "Missing shared English text. Add this key to sow-i18n/strings/en/web.toml.";
        container.appendChild(el("small", { class: "translation-fallback", role: "status" }, note));
    }
    function textEditor(container, label, keyPath, valuePath) {
        var step = state.definition.steps.find(function (item) { return item.id === state.selected; });
        var key = valuePath ? valuePath.split(".").reduce(function (value, part) { return value && value[part]; }, step) : step[keyPath];
        if (!key) {
            var suffix = keyPath === "title_key" ? "_title" : keyPath === "hint_key" ? "_hint" : "_body";
            if (valuePath) {
                var parts = valuePath.split("."), index = Number(parts[1]);
                if (parts[0] === "choices" && step.choices[index]) suffix = "_choice_" + step.choices[index].id + (keyPath === "label_key" ? "_label" : "_detail");
                else if (parts[0] === "lines" && step.lines[index]) suffix = "_line_" + (index + 1) + "_body";
            }
            key = storyKey(step.id + suffix);
            if (valuePath) setPath(step, valuePath, key); else step[keyPath] = key;
            markDirty();
        }
        var box = el("div", { class: "text-key" });
        box.appendChild(el("code", { class: "key-field" }, key));
        catalogTextField(box, label, key, ["Title", "Choice label"].includes(label) ? 3 : 6);
        container.appendChild(box);
    }
    function renderInspector() {
        var host = $("#inspector"); host.replaceChildren();
        var step = state.definition.steps.find(function (item) { return item.id === state.selected; });
        if (!step) { setFactionPicker(false); host.appendChild(el("p", { class: "empty-state" }, "Select a step in the mission flow.")); return; }
        setFactionPicker(state.pickingFaction);
        host.appendChild(el("div", { class: "step-validation", role: "status", hidden: true }));
        updateStepValidation();
        var basics = section("Step");
        basics.appendChild(inputField("Stable ID", step.id, function (value) {
            value = value.trim().toLowerCase().replace(/[^a-z0-9_-]+/g, "_");
            if (!value || state.definition.steps.some(function (item) { return item !== step && item.id === value; })) return;
            var old = step.id; step.id = value;
            if (state.definition.entry === old) state.definition.entry = value;
            if (state.definition.menu_guide && state.definition.menu_guide.entry === old) state.definition.menu_guide.entry = value;
            if (state.definition.layout[old]) { state.definition.layout[value] = state.definition.layout[old]; delete state.definition.layout[old]; }
            state.definition.steps.forEach(function (item) {
                if (item.next === old) item.next = value;
                (item.choices || []).forEach(function (choice) { if (choice.next === old) choice.next = value; });
                (item.routes || []).forEach(function (route) { if (route.next === old) route.next = value; });
                (item.routes || []).forEach(function (route) { if (route.when && route.when.choice === old) route.when.choice = value; });
            });
            state.selected = value; markDirty(); renderInspector();
        }));
        basics.appendChild(selectField("Step kind", step.type, types, function (value) {
            if (value === step.type) return;
            var id = step.id, xy = state.definition.layout[id];
            var referencedChoice = step.type === "choice" && state.definition.steps.some(function (item) {
                return item && Array.isArray(item.routes) && item.routes.some(function (route) { return route && route.when && route.when.choice === id; });
            });
            if (referencedChoice) { notice("Remove routes that depend on this decision before changing its type."); renderInspector(); return; }
            var sharedChoiceTarget = step.type === "choice" && step.choices && step.choices.length && step.choices.every(function (choice) { return choice.next === step.choices[0].next; }) ? step.choices[0].next : "";
            var divergentChoiceTargets = step.type === "choice" && step.choices && step.choices.some(function (choice) { return choice.next !== sharedChoiceTarget; });
            var next = step.type === "choice" ? sharedChoiceTarget : step.next || "";
            var keepsFlow = ["scene", "objective", "guide"].includes(value);
            var keepsMechanics = ["objective", "guide"].includes(value);
            var keepsMarker = ["scene", "objective", "guide"].includes(value);
            var discarded = [];
            if (step.lines && value !== "scene") discarded.push("dialogue lines");
            if (step.choices && value !== "choice") discarded.push("decision answers");
            if (step.trigger && (!keepsMechanics || (value === "guide" && step.trigger.type === "elapsed"))) discarded.push("mechanic condition");
            if (step.guide && (!keepsMechanics || (value === "guide" && step.trigger && step.trigger.type === "elapsed"))) discarded.push("hand guide");
            if (step.marker && !keepsMarker) discarded.push("map marker");
            if (step.advance_delay_seconds != null && !keepsMechanics) discarded.push("completion delay");
            if (step.campaign_assault_on_enter && value === "end") discarded.push("campaign assault action");
            if (step.routes && !keepsFlow) discarded.push("conditional routes");
            if (step.hint_key && !keepsMechanics) discarded.push("player hint");
            if (step.presentation && !["scene", "end"].includes(value)) discarded.push("presentation style");
            if (step.video_src && value !== "scene") discarded.push("cinematic video path");
            if (step.next && !keepsFlow && value !== "choice") discarded.push("outgoing connection");
            if (divergentChoiceTargets && value !== "choice") discarded.push("branch destinations");
            if (discarded.length && !confirm("Changing this step to “" + value + "” removes its " + discarded.join(", ") + ". Continue?")) { renderInspector(); return; }
            var ending = state.definition.steps.find(function (item) { return item !== step && item.type === "end"; });
            if (!next && !divergentChoiceTargets && value !== "end" && value !== "choice" && ending) next = ending.id;
            var menuAction = inGameUiTargets()[0] || "menu_campaign";
            var firstLine = step.lines && step.lines[0];
            var replacement = { id: id, type: value, title_key: step.title_key || storyKey(id + "_title") };
            var bodyKey = step.body_key || firstLine && firstLine.body_key || (value === "scene" && step.hint_key);
            if (bodyKey) replacement.body_key = bodyKey;
            if (step.speaker || firstLine && firstLine.speaker) replacement.speaker = step.speaker || firstLine.speaker;
            if (Number.isFinite(step.attack_ratio_on_enter)) replacement.attack_ratio_on_enter = step.attack_ratio_on_enter;
            if (step.campaign_assault_on_enter && value !== "end") replacement.campaign_assault_on_enter = step.campaign_assault_on_enter;
            if (keepsMechanics && Number.isFinite(step.advance_delay_seconds)) replacement.advance_delay_seconds = step.advance_delay_seconds;
            if (keepsMechanics && step.pause_game === true) replacement.pause_game = true;
            if (["scene", "end"].includes(value)) replacement.presentation = value === "end" && step.presentation === "cinematic" ? "chapter" : step.presentation || "dialogue";
            if (value === "scene") {
                if (step.video_src) replacement.video_src = step.video_src;
                if (Array.isArray(step.lines)) replacement.lines = step.lines;
                if (next) replacement.next = next;
                if (Array.isArray(step.routes)) replacement.routes = step.routes;
            }
            if (value === "choice") replacement.choices = step.type === "choice" && Array.isArray(step.choices)
                ? step.choices
                : [{ id: "first_answer", label_key: storyKey(id + "_first"), next: step.next || "" }, { id: "second_answer", label_key: storyKey(id + "_second"), next: step.next || "" }];
            if (value === "objective" || value === "guide") {
                if (step.hint_key) replacement.hint_key = step.hint_key;
                replacement.trigger = step.trigger && !(value === "guide" && step.trigger.type === "elapsed")
                    ? step.trigger
                    : state.flow === "menu" ? { type: "ui", action: menuAction, scope: "step" } : { type: "territory", value: 1, scope: "step" };
                if (Array.isArray(step.routes)) replacement.routes = step.routes;
                if (step.guide && !(value === "guide" && step.trigger && step.trigger.type === "elapsed")) replacement.guide = step.guide;
                else if (value === "guide") replacement.guide = replacement.trigger.type === "ui"
                    ? { kind: "ui", target: replacement.trigger.action, gesture: "tap" }
                    : { kind: "world", target: "expand", gesture: "tap" };
                if (next) replacement.next = next;
            }
            if (step.marker && keepsMarker) replacement.marker = step.marker;
            Object.keys(step).forEach(function (key) { delete step[key]; });
            Object.assign(step, replacement);
            if (xy) state.definition.layout[id] = xy;
            notice(""); markDirty(); renderInspector();
        }));
        basics.appendChild(selectField("Speaker", step.speaker || "", speakerOptions(), function (value) { step.speaker = value || undefined; if (!value) delete step.speaker; markDirty(); }));
        if (step.type === "scene" || step.type === "end") basics.appendChild(selectField("Presentation", step.presentation || "dialogue", step.type === "scene" ? ["dialogue", "chapter", "celebration", "cinematic"] : ["dialogue", "chapter", "celebration"], function (value) { step.presentation = value; markDirty(); }));
        if (step.type === "scene") {
            basics.appendChild(inputField("Local cinematic video", step.video_src || "", function (value) {
                var source = value.trim();
                if (source) { step.video_src = source; step.presentation = "cinematic"; }
                else delete step.video_src;
                markDirty();
            }, { placeholder: "/assets/campaign/" + state.definition.episode_id + "/opening.webm" }));
            basics.appendChild(el("small", { class: "translation-fallback" }, "Optional MP4/WebM; place the file in this episode's assets/campaign folder. The shared preview plays it without autoplay."));
        }
        basics.appendChild(inputField("Set send percentage on entry (%)", step.attack_ratio_on_enter == null ? "" : Math.round(step.attack_ratio_on_enter * 100), function (value) {
            if (value.trim() === "") delete step.attack_ratio_on_enter;
            else step.attack_ratio_on_enter = Number(value) / 100;
            markDirty();
        }, { type: "number", min: 5, max: 100, step: 1, placeholder: "No change" }));
        textEditor(basics, "Title", "title_key");
        if (step.body_key || step.type === "choice" || (step.type === "scene" && !Array.isArray(step.lines))) textEditor(basics, step.type === "end" ? "Closing text" : "Story text", "body_key");
        else if (step.type === "end") {
            var addEnding = el("button", { type: "button" }, "Add closing text");
            addEnding.addEventListener("click", function () { step.body_key = storyKey(step.id + "_body"); markDirty(); renderInspector(); });
            basics.appendChild(addEnding);
        }
        if (step.type === "objective" || step.type === "guide") textEditor(basics, "Player hint", "hint_key");
        if (step.type === "scene" && Array.isArray(step.lines)) {
            step.lines.forEach(function (line, index) {
                var lineBox = el("div", { class: "form-card" });
                var lineHeading = el("div", { class: "card-title" }); lineHeading.appendChild(el("strong", {}, "Dialogue line " + (index + 1)));
                var lineActions = reorderControls(step.lines, index, "dialogue line");
                var removeLine = el("button", { type: "button" }, step.lines.length > 1 ? "Remove" : "Use one paragraph");
                removeLine.addEventListener("click", function () {
                    if (step.lines.length > 1) step.lines.splice(index, 1);
                    else { step.body_key = line.body_key; delete step.lines; }
                    markDirty(); renderInspector();
                });
                lineActions.appendChild(removeLine);
                lineHeading.appendChild(lineActions); lineBox.appendChild(lineHeading);
                lineBox.appendChild(selectField("Speaker", line.speaker || step.speaker || "", speakerOptions(), function (value) { line.speaker = value || undefined; if (!value) delete line.speaker; markDirty(); }));
                textEditor(lineBox, "Line text", "body_key", "lines." + index + ".body_key");
                basics.appendChild(lineBox);
            });
            basics.appendChild(el("button", { type: "button" }, "Add dialogue line")).addEventListener("click", function (event) {
                event.preventDefault(); step.lines.push({ speaker: step.speaker, body_key: storyKey(step.id + "_line_" + (step.lines.length + 1)) }); markDirty(); renderInspector();
            });
        } else if (step.type === "scene") {
            var addConversation = el("button", { type: "button" }, "Split into dialogue lines");
            addConversation.addEventListener("click", function () { step.lines = [{ speaker: step.speaker, body_key: step.body_key || storyKey(step.id + "_line_1") }]; delete step.body_key; markDirty(); renderInspector(); });
            basics.appendChild(addConversation);
        }
        host.appendChild(basics);

        if (step.type !== "end") {
            var campaignActions = section("Campaign actions");
            campaignActions.appendChild(checkboxField("Order a team to attack when this step begins", Boolean(step.campaign_assault_on_enter), function (enabled) {
                if (enabled) step.campaign_assault_on_enter = step.campaign_assault_on_enter || { attacker_team: "Red", target: "player" };
                else delete step.campaign_assault_on_enter;
                markDirty(); renderInspector();
            }));
            if (step.campaign_assault_on_enter) {
                var assault = step.campaign_assault_on_enter;
                campaignActions.appendChild(selectField("Attacking team", assault.attacker_team || "Red", window.SOWCampaign.TEAMS, function (value) { assault.attacker_team = value; markDirty(); }));
                campaignActions.appendChild(selectField("Target", assault.target || "player", [{ value: "player", label: "Player" }].concat(factionOptions()), function (value) { assault.target = value; markDirty(); }));
            }
            host.appendChild(campaignActions);
        }

        if (step.type === "choice") {
            var answers = section("Player decisions");
            step.choices.forEach(function (choice, index) {
                var card = el("div", { class: "form-card" });
                var titleRow = el("div", { class: "card-title" }); titleRow.appendChild(el("strong", {}, "Answer " + (index + 1)));
                var answerActions = reorderControls(step.choices, index, "answer");
                if (step.choices.length > 2) {
                    var remove = el("button", { type: "button", class: "danger" }, "Remove");
                    remove.addEventListener("click", function () {
                        var isReferenced = state.definition.steps.some(function (item) {
                            return (item.routes || []).some(function (route) { return route.when && route.when.choice === step.id && route.when.equals === choice.id; });
                        });
                        if (isReferenced) { notice("Remove routes that depend on this answer first."); return; }
                        step.choices.splice(index, 1); notice(""); markDirty(); renderInspector();
                    });
                    answerActions.appendChild(remove);
                }
                titleRow.appendChild(answerActions);
                card.appendChild(titleRow);
                card.appendChild(inputField("Answer ID", choice.id, function (value) {
                    var old = choice.id, next = value.trim().toLowerCase().replace(/[^a-z0-9_-]+/g, "_");
                    if (!next || step.choices.some(function (item) { return item !== choice && item.id === next; })) return;
                    choice.id = next;
                    state.definition.steps.forEach(function (item) { (item.routes || []).forEach(function (route) { if (route.when && route.when.choice === step.id && route.when.equals === old) route.when.equals = next; }); });
                    markDirty(); renderGraph();
                }));
                textEditor(card, "Choice label", "label_key", "choices." + index + ".label_key");
                if (choice.body_key) {
                    textEditor(card, "Optional consequence", "body_key", "choices." + index + ".body_key");
                    var removeDetail = el("button", { type: "button", class: "danger" }, "Remove detail");
                    removeDetail.addEventListener("click", function () { delete choice.body_key; markDirty(); renderInspector(); });
                    card.appendChild(removeDetail);
                } else {
                    var addDetail = el("button", { type: "button" }, "Add consequence detail");
                    addDetail.addEventListener("click", function () { choice.body_key = storyKey(step.id + "_" + choice.id + "_detail"); markDirty(); renderInspector(); });
                    card.appendChild(addDetail);
                }
                card.appendChild(selectField("Next step", choice.next, destinationOptions(step.id), function (value) { choice.next = value; markDirty(); }));
                var branchType = "scene", branchTypeOptions = [
                    { value: "scene", label: "Scene" }, { value: "choice", label: "Decision" },
                    { value: "objective", label: "Objective" }, { value: "guide", label: "Guide" }
                ];
                card.appendChild(selectField("Create after this answer", branchType, branchTypeOptions, function (value) { branchType = value; }));
                var createBranch = el("button", { type: "button" }, "Create next step");
                createBranch.addEventListener("click", function () { addStep(branchType, { step: step, output: choice }); });
                card.appendChild(createBranch);
                answers.appendChild(card);
            });
            if (step.choices.length < 4) { var add = el("button", { type: "button" }, "Add answer"); add.addEventListener("click", function () { var answerId = uniqueId("answer", step.choices); step.choices.push({ id: answerId, label_key: storyKey(step.id + "_" + answerId), next: state.definition.steps.find(function (item) { return item.type === "end"; })?.id || "" }); markDirty(); renderInspector(); }); answers.appendChild(add); }
            host.appendChild(answers);
        }
        if (step.type === "objective" || step.type === "guide") {
            var objective = section("Mechanic and guide");
            objective.appendChild(inputField("Wait after completion (seconds)", step.advance_delay_seconds, function (value) {
                if (value.trim() === "") delete step.advance_delay_seconds;
                else step.advance_delay_seconds = Number(value);
                markDirty();
            }, { type: "number", min: 0.1, max: 10, step: 0.1, placeholder: "No wait" }));
            objective.appendChild(checkboxField("Pause game until this objective completes", step.pause_game === true, function (value) { if (value) step.pause_game = true; else { delete step.pause_game; delete step.camera_only; } markDirty(); renderInspector(); }));
            objective.appendChild(checkboxField("Allow camera controls only while paused", step.camera_only === true, function (value) { if (value) { step.pause_game = true; step.camera_only = true; } else delete step.camera_only; markDirty(); renderInspector(); }));
            step.trigger = step.trigger || { type: "territory", value: 1, scope: "step" };
            if (step.type === "guide" && !step.guide) step.guide = { kind: "world", target: "expand", gesture: "tap" };
            var availableTriggers = state.flow === "menu"
                ? triggerTypes.filter(function (trigger) { return trigger.value === "ui"; })
                : step.type === "guide" ? triggerTypes.filter(function (trigger) { return trigger.value !== "elapsed"; }) : triggerTypes;
            objective.appendChild(selectField("Complete when", step.trigger.type, availableTriggers, function (value) {
                step.trigger = { type: value, scope: value === "troops" ? "total" : step.trigger.scope || "step" };
                if (["contact", "defeated", "eliminated", "hover", "camera_target", "zoom_in_complete"].includes(value)) step.trigger.target = "";
                if (value === "camera_target") step.trigger.distance = 12;
                if (value === "zoom_in_complete") step.trigger.distance = 12;
                if (value === "zoom_out_complete") step.trigger.value = 0.55;
                if (value === "fleet") step.trigger.unit = "TransportShip";
                if (value === "resource_transfer") { step.trigger.recipient = ""; step.trigger.resources = ["gold", "troops"]; }
                if (value === "structure_level") step.trigger.kind = "City";
                else if (value === "ui") {
                    step.trigger.action = state.flow === "menu" ? inGameUiTargets()[0] || "menu_campaign" : "map_attack";
                    if (step.guide) Object.assign(step.guide, { kind: "ui", target: step.trigger.action });
                }
                else step.trigger.value = 1;
                if (["zoom_in", "zoom_out"].includes(value)) step.guide = { kind: "world", target: "player", gesture: value };
                if (step.guide && value !== "ui" && step.guide.kind === "ui") Object.assign(step.guide, { kind: "world", target: worldGuideTarget(value) });
                if (value === "elapsed" && step.type === "objective") delete step.guide;
                markDirty(); renderInspector();
            }));
            if (step.trigger.type === "contact") {
                if (Array.isArray(step.trigger.targets)) {
                    objective.appendChild(multiFactionField("Factions to contact", step.trigger.targets, function (targets) { step.trigger.targets = targets; if (step.trigger.value > targets.length) step.trigger.value = targets.length; markDirty(); renderInspector(); }));
                    objective.appendChild(inputField("How many must be contacted", step.trigger.value || 1, function (value) { step.trigger.value = Number(value); markDirty(); }, { type: "number", min: 1, max: step.trigger.targets.length, step: 1 }));
                } else {
                    objective.appendChild(selectField("Faction", step.trigger.target, factionOptions(), function (value) { step.trigger.target = value; markDirty(); }));
                    var anyContact = el("button", { type: "button" }, "Accept contact with multiple factions");
                    anyContact.addEventListener("click", function () { step.trigger.targets = step.trigger.target ? [step.trigger.target] : []; delete step.trigger.target; markDirty(); renderInspector(); });
                    objective.appendChild(anyContact);
                }
            }
            else if (step.trigger.type === "defeated") {
                if (Array.isArray(step.trigger.targets)) {
                    objective.appendChild(multiFactionField("Factions to defeat", step.trigger.targets, function (targets) { step.trigger.targets = targets; step.trigger.value = targets.length; markDirty(); }));
                } else {
                    objective.appendChild(selectField("Faction", step.trigger.target || "", factionOptions(), function (value) { step.trigger.target = value; markDirty(); }));
                    var groupTargets = el("button", { type: "button" }, "Track multiple factions (3/3)");
                    groupTargets.addEventListener("click", function () { step.trigger.targets = step.trigger.target ? [step.trigger.target] : []; delete step.trigger.target; step.trigger.value = step.trigger.targets.length; markDirty(); renderInspector(); });
                    objective.appendChild(groupTargets);
                }
            }
            else if (step.trigger.type === "eliminated") objective.appendChild(selectField("Faction", step.trigger.target || "player", [{ value: "player", label: "Player" }].concat(factionOptions()), function (value) { step.trigger.target = value; markDirty(); }));
            else if (step.trigger.type === "attack") objective.appendChild(selectField("Attack target", step.trigger.target || "", [{ value: "", label: "Any faction" }].concat(factionOptions()), function (value) { if (value) step.trigger.target = value; else delete step.trigger.target; markDirty(); }));
            else if (step.trigger.type === "fleet") {
                objective.appendChild(selectField("Ship type", step.trigger.unit || "", ["TransportShip", "TradeShip", "Warship"], function (value) { step.trigger.unit = value; if (value !== "TransportShip") delete step.trigger.target; markDirty(); renderInspector(); }));
                if (step.trigger.unit === "TransportShip") objective.appendChild(selectField("Landing target", step.trigger.target || "", [{ value: "", label: "Any target" }].concat(factionOptions()), function (value) { if (value) step.trigger.target = value; else delete step.trigger.target; markDirty(); }));
                objective.appendChild(inputField("Required amount", step.trigger.value || 1, function (value) { step.trigger.value = Number(value); markDirty(); }, { type: "number", min: 1, step: 1 }));
            }
            else if (step.trigger.type === "resource_transfer") {
                objective.appendChild(selectField("Recipient", step.trigger.recipient || "", [{ value: "", label: "Any faction" }].concat(factionOptions()), function (value) { if (value) step.trigger.recipient = value; else delete step.trigger.recipient; markDirty(); }));
                var requiredResources = step.trigger.resources || [];
                objective.appendChild(checkboxField("Must include gold", requiredResources.includes("gold"), function (enabled) { step.trigger.resources = requiredResources.filter(function (item) { return item !== "gold"; }); if (enabled) step.trigger.resources.push("gold"); if (!step.trigger.resources.length) delete step.trigger.resources; markDirty(); renderInspector(); }));
                objective.appendChild(checkboxField("Must include troops", requiredResources.includes("troops"), function (enabled) { step.trigger.resources = requiredResources.filter(function (item) { return item !== "troops"; }); if (enabled) step.trigger.resources.push("troops"); if (!step.trigger.resources.length) delete step.trigger.resources; markDirty(); renderInspector(); }));
                objective.appendChild(inputField("Required amount", step.trigger.value || 1, function (value) { step.trigger.value = Number(value); markDirty(); }, { type: "number", min: 1, step: 1 }));
            }
            else if (step.trigger.type === "structure_level") {
                var structureLevelLimits = { City: 6, Farm: 3, Factory: 4, Bunker: 4, Port: 5 };
                objective.appendChild(selectField("Building type", step.trigger.kind || "City", ["City", "Farm", "Factory", "Bunker", "Port"], function (value) {
                    step.trigger.kind = value;
                    step.trigger.value = Math.min(Number(step.trigger.value || 1), structureLevelLimits[value]);
                    markDirty(); renderInspector();
                }));
                objective.appendChild(inputField("Target level", step.trigger.value || 1, function (value) { step.trigger.value = Math.min(Number(value), structureLevelLimits[step.trigger.kind] || 1); markDirty(); }, { type: "number", min: 1, max: structureLevelLimits[step.trigger.kind] || 1, step: 1 }));
            }
            else if (step.trigger.type === "zoom_out_complete") objective.appendChild(inputField("Zoom-out distance (% of range)", Math.round(Number(step.trigger.value || 1) * 100), function (value) { step.trigger.value = Math.max(1, Math.min(100, Number(value))) / 100; markDirty(); }, { type: "number", min: 1, max: 100, step: 5 }));
            else if (step.trigger.type === "zoom_in_complete") {
                objective.appendChild(selectField("Faction to inspect", step.trigger.target || "", [{ value: "", label: "Any location" }].concat(factionOptions()), function (value) { if (value) { step.trigger.target = value; step.trigger.distance = Number(step.trigger.distance || 12); } else { delete step.trigger.target; delete step.trigger.distance; } markDirty(); renderInspector(); }));
                if (step.trigger.target) objective.appendChild(inputField("Max distance from camera (tiles)", step.trigger.distance || 12, function (value) { step.trigger.distance = Number(value); markDirty(); }, { type: "number", min: 1, max: 1000, step: 1 }));
            }
            else if (step.trigger.type === "camera_target") {
                objective.appendChild(selectField("Target to locate", step.trigger.target || "", [{ value: "", label: "Choose a target" }, { value: "player", label: "Player" }].concat(factionOptions()), function (value) { if (value) step.trigger.target = value; else delete step.trigger.target; markDirty(); renderInspector(); }));
                objective.appendChild(inputField("Max distance (tiles)", step.trigger.distance || 12, function (value) { step.trigger.distance = Number(value); markDirty(); }, { type: "number", min: 1, max: 1000, step: 1 }));
            }
            else if (step.trigger.type === "hover") objective.appendChild(selectField("Target to locate", step.trigger.target || "", [{ value: "", label: "Choose a target" }, { value: "player", label: "Player" }].concat(factionOptions()), function (value) { if (value) step.trigger.target = value; else delete step.trigger.target; markDirty(); renderInspector(); }));
            else if (step.trigger.type === "ui") objective.appendChild(selectField("Control action", step.trigger.action, inGameUiTargets(), function (value) { step.trigger.action = value; markDirty(); }));
            else objective.appendChild(inputField(step.trigger.type === "troops" ? "Minimum troops" : step.trigger.type === "elapsed" ? "Wait (seconds)" : "Required amount", step.trigger.value, function (value) { step.trigger.value = Number(value); markDirty(); }, { type: "number", min: 1, step: 1 }));
            if (step.trigger.type !== "troops") objective.appendChild(selectField("Count from", step.trigger.scope || "step", ["step", "episode", "total"], function (value) { step.trigger.scope = value; markDirty(); }));
            if (step.guide) {
                objective.appendChild(selectField("Hand points at", step.guide.kind + ":" + step.guide.target, [
                    { value: "world:expand", label: "Map · expansion" }, { value: "world:assault", label: "Map · attack" }, { value: "world:target_action", label: "Map · target action" }, { value: "world:player", label: "Map · player base" }, { value: "world:nameplate", label: "Map · faction nameplate" }
                ].concat(inGameUiTargets().map(function (key) { return { value: "ui:" + key, label: "Interface · " + key.replace(/_/g, " ") }; })), function (value) { var pair = value.split(":"); if (pair[0] !== step.guide.kind) delete step.guide.to; step.guide.kind = pair[0]; step.guide.target = pair[1]; markDirty(); renderInspector(); }));
                objective.appendChild(selectField("Gesture", step.guide.gesture || "tap", ["tap", "hold", "drag", "hover", "pan_keys", "zoom_in", "zoom_out"], function (value) {
                    step.guide.gesture = value;
                    if (value !== "drag") delete step.guide.to;
                    if (["zoom_in", "zoom_out"].includes(value)) { step.guide.kind = "world"; step.guide.target = "player"; step.trigger.type = step.trigger.type === value + "_complete" ? value + "_complete" : value; }
                    markDirty(); renderInspector();
                }));
                if (step.guide.gesture === "drag") {
                    var dragTargets = step.guide.kind === "world"
                        ? [{ value: "expand", label: "Map · expansion" }, { value: "assault", label: "Map · attack" }, { value: "target_action", label: "Map · target action" }, { value: "player", label: "Map · player base" }, { value: "nameplate", label: "Map · faction nameplate" }]
                        : [{ value: "", label: "Within this control" }].concat(inGameUiTargets().map(function (key) { return { value: key, label: "Interface · " + key.replace(/_/g, " ") }; }));
                    objective.appendChild(selectField("Drag destination", step.guide.to, dragTargets, function (value) { if (value) step.guide.to = value; else delete step.guide.to; markDirty(); }));
                }
                if (step.type === "objective") {
                    var removeGuide = el("button", { type: "button" }, "Remove hand guide");
                    removeGuide.addEventListener("click", function () { delete step.guide; markDirty(); renderInspector(); });
                    objective.appendChild(removeGuide);
                }
            } else if (step.type === "objective" && step.trigger.type !== "elapsed") {
                var addGuide = el("button", { type: "button" }, "Add hand guide");
                addGuide.addEventListener("click", function () {
                    step.guide = ["zoom_in", "zoom_out"].includes(step.trigger.type)
                        ? { kind: "world", target: "player", gesture: step.trigger.type }
                        : step.trigger.type === "ui"
                        ? { kind: "ui", target: step.trigger.action, gesture: "tap" }
                        : { kind: "world", target: worldGuideTarget(step.trigger.type), gesture: "tap" };
                    markDirty(); renderInspector();
                });
                objective.appendChild(addGuide);
            }
            objective.appendChild(selectField("Next step", step.next, destinationOptions(step.id), function (value) { step.next = value; markDirty(); }));
            host.appendChild(objective);
        } else if (step.type !== "choice" && step.type !== "end") {
            var next = section("Flow");
            next.appendChild(selectField("Next step", step.next, destinationOptions(step.id), function (value) { step.next = value; markDirty(); }));
            host.appendChild(next);
        }
        if (["scene", "objective", "guide"].includes(step.type)) {
            var markerPanel = section("Map marker");
            if (step.marker) markerPanel.appendChild(selectField("Highlight faction", step.marker.target, [{ value: "player", label: "Player" }].concat(factionOptions()), function (value) { step.marker.target = value; markDirty(); }));
            else {
                var addMarkerButton = el("button", { type: "button" }, "Add map marker");
                addMarkerButton.addEventListener("click", function () { step.marker = { target: "player" }; markDirty(); renderInspector(); });
                markerPanel.appendChild(addMarkerButton);
            }
            host.appendChild(markerPanel);
        }
        if (step.type !== "choice" && step.type !== "end") {
            var paths = section("Conditional paths");
            paths.appendChild(el("small", { class: "muted" }, "First matching condition wins; otherwise the default next link is used."));
            (step.routes || []).forEach(function (route, index) {
                var card = el("div", { class: "form-card" });
                var routeHeader = el("div", { class: "card-title" });
                routeHeader.appendChild(el("strong", {}, "Condition " + (index + 1)));
                routeHeader.appendChild(reorderControls(step.routes, index, "condition"));
                card.appendChild(routeHeader);
                var kind = route.when && Object.prototype.hasOwnProperty.call(route.when, "choice") ? "choice" : "fact";
                card.appendChild(selectField("Condition", kind, [
                    { value: "fact", label: "Game fact threshold" }, { value: "choice", label: "Earlier decision" }
                ], function (value) { route.when = value === "choice" ? { choice: "", equals: "" } : { fact: "tiles_gained", gte: 1 }; markDirty(); renderInspector(); }));
                if (kind === "choice") {
                    var selectedAnswer = route.when.choice && route.when.equals ? route.when.choice + "@" + route.when.equals : "";
                    card.appendChild(selectField("Answer", selectedAnswer, [{ value: "", label: "Choose a decision answer" }].concat(choiceAnswerOptions()), function (value) {
                        var pair = value.split("@"); route.when.choice = pair[0] || ""; route.when.equals = pair[1] || ""; markDirty();
                    }));
                } else {
                    var gameFacts = Object.values(window.SOWCampaign.METRICS).concat(["tiles", "touch_controls"]).filter(function (value, index, all) { return all.indexOf(value) === index; });
                    card.appendChild(selectField("Fact", route.when.fact, gameFacts, function (value) { route.when.fact = value; markDirty(); }));
                    card.appendChild(inputField("At least", route.when.gte, function (value) { route.when.gte = Number(value); markDirty(); }, { type: "number", min: 0, step: 1 }));
                }
                card.appendChild(selectField("Then go to", route.next, destinationOptions(step.id), function (value) { route.next = value; markDirty(); }));
                var removeRoute = el("button", { type: "button", class: "danger" }, "Remove condition");
                removeRoute.addEventListener("click", function () { step.routes.splice(index, 1); if (!step.routes.length) delete step.routes; markDirty(); renderInspector(); });
                card.appendChild(removeRoute); paths.appendChild(card);
            });
            var addRoute = el("button", { type: "button" }, "Add conditional path");
            addRoute.addEventListener("click", function () {
                var destination = destinationOptions(step.id)[0];
                step.routes = step.routes || [];
                step.routes.push({ when: { fact: "tiles_gained", gte: 1 }, next: destination && destination.value || "" });
                markDirty(); renderInspector();
            });
            paths.appendChild(addRoute); host.appendChild(paths);
        }
        var danger = el("section", { class: "form-section" });
        var removeStep = el("button", { type: "button", class: "danger" }, "Delete step");
        removeStep.addEventListener("click", function () {
            if (state.definition.steps.length <= 1) return;
            var next = step.next;
            var incoming = state.definition.steps.some(function (item) {
                return item !== step && (item.next === step.id || (item.choices || []).some(function (choice) { return choice.next === step.id; }) || (item.routes || []).some(function (route) { return route.next === step.id; }));
            });
            var referencedChoice = state.definition.steps.some(function (item) {
                return (item.routes || []).some(function (route) { return route.when && route.when.choice === step.id; });
            });
            var isEntry = state.definition.entry === step.id;
            var isMenuEntry = state.definition.menu_guide && state.definition.menu_guide.entry === step.id;
            var targetExists = state.definition.steps.some(function (item) { return item.id === next && item !== step; });
            var canBypass = step.type !== "choice" && !(step.routes || []).length && targetExists;
            if (referencedChoice) {
                notice("Remove conditions that depend on this decision before deleting it.");
                return;
            }
            if ((incoming || isEntry || isMenuEntry) && !canBypass) {
                notice("Reconnect incoming paths before deleting this step; it has no single next destination.");
                return;
            }
            if (canBypass) {
                state.definition.steps.forEach(function (item) {
                    if (item.next === step.id) item.next = next;
                    (item.choices || []).forEach(function (choice) { if (choice.next === step.id) choice.next = next; });
                    (item.routes || []).forEach(function (route) { if (route.next === step.id) route.next = next; });
                });
                if (isEntry) state.definition.entry = next;
                if (isMenuEntry) state.definition.menu_guide.entry = next;
            }
            state.definition.steps = state.definition.steps.filter(function (item) { return item !== step; });
            state.selected = canBypass ? next : state.definition.entry;
            delete state.definition.layout[step.id]; notice(""); markDirty(); renderInspector();
        });
        danger.appendChild(removeStep); host.appendChild(danger);
    }
    function stepLabel(step) {
        var title = textValue(step.title_key, state.previewLanguage);
        return [title && title !== step.id ? title : "", step.type, step.id].filter(Boolean).join(" · ");
    }
    function stepOption(step) { return { value: step.id, label: stepLabel(step) }; }
    function destinationOptions(except) {
        return state.definition.steps.filter(function (step) { return step.id !== except; }).map(stepOption);
    }
    function factionOptions() { return state.roster.factions.map(function (faction) { return { value: faction.id, label: faction.name }; }); }
    function supportFactionOptions() { return state.roster.factions.filter(function (faction) { return Number.isInteger(faction.support_interval_seconds); }).map(function (faction) { return { value: faction.id, label: faction.name }; }); }
    function factionName(factionId) { var faction = state.roster.factions.find(function (item) { return item.id === factionId; }); return faction ? faction.name : factionId; }
    function speakerOptions() { return [{ value: "", label: "Narrator" }].concat(Object.keys(state.definition.speakers || {}).map(function (id) { var speaker = state.definition.speakers[id], faction = state.roster.factions.find(function (item) { return item.id === speaker.faction; }); return { value: id, label: faction ? faction.name : speaker.name || id }; })); }
    function inGameUiTargets() { return Object.keys(window.SOWCampaign.UI_TARGETS).filter(function (key) { return state.flow === "menu" ? key.startsWith("menu_") || key === "campaign_replay" : !key.startsWith("menu_") && key !== "campaign_replay"; }); }
    function worldGuideTarget(type) { return ["attack", "kills"].includes(type) ? "assault" : ["contact", "defeated"].includes(type) ? "target_action" : ["hover", "camera_target"].includes(type) ? "player" : "expand"; }
    function flowEntry() { return state.flow === "menu" ? state.definition.menu_guide && state.definition.menu_guide.entry : state.definition.entry; }
    function choiceAnswerOptions() {
        var result = [];
        state.definition.steps.filter(function (step) { return step.type === "choice"; }).forEach(function (step) {
            (step.choices || []).forEach(function (answer) { result.push({ value: step.id + "@" + answer.id, label: stepLabel(step) + " · " + (textValue(answer.label_key, state.previewLanguage) || answer.id) }); });
        });
        return result;
    }

    function renderSettings() {
        var host = $("#settings"); host.replaceChildren();
        var settings = state.definition.settings;
        host.appendChild(checkboxField("Buildings available", settings.buildings_enabled, function (value) { settings.buildings_enabled = value; markDirty(); }));
        host.appendChild(inputField("Starting troops", settings.starting_troops, function (value) { settings.starting_troops = Number(value); $("#sampleTroops").textContent = Number(value).toLocaleString(); markDirty(); }, { type: "number", min: 1, max: 100000, step: 100 }));
        host.appendChild(selectField("Unlock buildings after", settings.buildings_unlock_after_defeated || "", [{ value: "", label: "No delayed unlock" }].concat(factionOptions()), function (value) {
            if (value) settings.buildings_unlock_after_defeated = value; else delete settings.buildings_unlock_after_defeated;
            markDirty();
        }));
        host.appendChild(checkboxField("Allied support after milestone", Boolean(settings.campaign_support), function (enabled) {
            if (!enabled) delete settings.campaign_support;
            else {
                var defaultMilestone = settings.buildings_unlock_after_defeated || (state.roster.factions.find(function (faction) { return faction.avatar === "the_iceni_despoilers"; }) || {}).id || "";
                settings.campaign_support = settings.campaign_support || { after_defeated: defaultMilestone, share_percent: 50 };
            }
            markDirty(); renderSettings();
        }));
        if (settings.campaign_support) {
            var support = settings.campaign_support;
            host.appendChild(selectField("Support begins after", support.after_defeated || "", factionOptions(), function (value) { support.after_defeated = value; markDirty(); }));
            host.appendChild(inputField("Current reserves given (%)", support.share_percent, function (value) { support.share_percent = Number(value); markDirty(); }, { type: "number", min: 1, max: 100, step: 1 }));
        }
        var reactions = section("Story responses");
        (state.definition.reactions || []).forEach(function (reaction) {
            var card = el("div", { class: "form-card" });
            var isContact = reaction.when.type === "contact";
            var isFirstContact = reaction.when.type === "first_contact";
            card.appendChild(el("strong", {}, (isFirstContact ? "First Iceni contact · " : isContact ? "Contact · " : "First support · ") + reaction.id));
            if (isContact) {
                var contactTargetOptions = [{ value: "any_neutral", label: "Any initially neutral faction" }].concat(factionOptions());
                card.appendChild(selectField("On contact with", reaction.when.relation === "neutral" ? "any_neutral" : reaction.when.target, contactTargetOptions, function (value) {
                    if (value === "any_neutral") { delete reaction.when.target; reaction.when.relation = "neutral"; }
                    else { delete reaction.when.relation; reaction.when.target = value; }
                    markDirty(); renderSettings();
                }));
                card.appendChild(selectField("Show after", reaction.after || "", [{ value: "", label: "Immediately" }].concat(state.definition.steps.filter(function (step) { return ["objective", "guide"].includes(step.type); }).map(stepOption)), function (value) { if (value) reaction.after = value; else delete reaction.after; markDirty(); }));
            } else if (isFirstContact) {
                card.appendChild(multiFactionField("Eligible factions (first matching contact)", reaction.when.targets || [], function (value) { reaction.when.targets = value; markDirty(); }));
            } else {
                card.appendChild(selectField("Show after", reaction.after, state.definition.steps.filter(function (step) { return ["objective", "guide"].includes(step.type); }).map(stepOption), function (value) { reaction.after = value; markDirty(); }));
                card.appendChild(selectField("First aid from", reaction.when.target, supportFactionOptions(), function (value) { reaction.when.target = value; markDirty(); }));
            }
            if (isContact || isFirstContact) {
                card.appendChild(selectField("Automatic relationship result", reaction.outcome || "", [
                    { value: "", label: reaction.choices ? "Negotiation choices" : "Dialogue only" },
                    { value: "allied", label: "Alliance" }, { value: "enemy", label: "Enemy" }, { value: "neutral", label: "Remain neutral" }
                ], function (value) { if (value) { reaction.outcome = value; delete reaction.choices; } else delete reaction.outcome; markDirty(); renderSettings(); }));
                if (reaction.outcome === "allied") card.appendChild(inputField("Gold paid", reaction.gold_cost || 0, function (value) { reaction.gold_cost = Number(value); markDirty(); }, { type: "number", min: 0, max: 1000000, step: 25 }));
                if (reaction.choices) {
                    reaction.choices.forEach(function (choice) {
                        var choiceCard = el("div", { class: "form-card" });
                        choiceCard.appendChild(el("strong", {}, "Negotiation · " + choice.id));
                        choiceCard.appendChild(selectField("Result", choice.relation, ["allied", "enemy", "neutral"], function (value) { choice.relation = value; markDirty(); renderSettings(); }));
                        if (choice.relation === "allied") choiceCard.appendChild(inputField("Gold demanded", choice.gold_cost || 0, function (value) { choice.gold_cost = Number(value); markDirty(); }, { type: "number", min: 0, max: 1000000, step: 25 }));
                        catalogTextField(choiceCard, "Choice", choice.label_key, 2);
                        catalogTextField(choiceCard, "Consequence", choice.body_key, 3);
                        var removeChoice = el("button", { type: "button", class: "danger" }, "Remove choice");
                        removeChoice.disabled = reaction.choices.length <= 2;
                        removeChoice.addEventListener("click", function () { reaction.choices = reaction.choices.filter(function (item) { return item !== choice; }); markDirty(); renderSettings(); });
                        choiceCard.appendChild(removeChoice); card.appendChild(choiceCard);
                    });
                    var addChoice = el("button", { type: "button" }, "Add negotiation option");
                    addChoice.disabled = reaction.choices.length >= 4;
                    addChoice.addEventListener("click", function () {
                        var choiceId = "offer_" + (reaction.choices.length + 1), key = storyKey(reaction.id + "_" + choiceId);
                        reaction.choices.push({ id: choiceId, label_key: key + "_label", body_key: key + "_detail", relation: "neutral" });
                        markDirty(); renderSettings();
                    });
                    card.appendChild(addChoice);
                } else if (!reaction.outcome) {
                    var addNegotiation = el("button", { type: "button" }, "Add negotiation choices");
                    addNegotiation.addEventListener("click", function () {
                        var prefix = storyKey(reaction.id);
                        reaction.choices = [
                            { id: "accept", label_key: prefix + "_accept_label", body_key: prefix + "_accept_detail", relation: "allied", gold_cost: 200 },
                            { id: "refuse", label_key: prefix + "_refuse_label", body_key: prefix + "_refuse_detail", relation: "enemy", gold_cost: 0 }
                        ];
                        markDirty(); renderSettings();
                    });
                    card.appendChild(addNegotiation);
                }
            }
            card.appendChild(selectField("Speaker", reaction.speaker || "", speakerOptions(), function (value) { if (value) reaction.speaker = value; else delete reaction.speaker; markDirty(); }));
            catalogTextField(card, "Title", reaction.title_key, 2);
            catalogTextField(card, "Dialogue", reaction.body_key, 4);
            var remove = el("button", { type: "button", class: "danger" }, "Remove response");
            remove.addEventListener("click", function () { state.definition.reactions = state.definition.reactions.filter(function (item) { return item !== reaction; }); markDirty(); renderSettings(); });
            card.appendChild(remove); reactions.appendChild(card);
        });
        function addReaction(type) {
            var isFirstContact = type === "first_contact", isNeutralContact = type === "neutral_contact";
            var eventType = isNeutralContact ? "contact" : type;
            var used = new Set((state.definition.reactions || []).filter(function (item) { return item.when.type === eventType; }).map(function (item) { return item.when.target || (item.when.relation === "neutral" ? "any_neutral" : null); }));
            var factions = eventType === "support" ? supportFactionOptions().map(function (option) { return state.roster.factions.find(function (item) { return item.id === option.value; }); }) : state.roster.factions;
            var faction = isFirstContact || isNeutralContact ? null : factions.find(function (item) { return item && !used.has(item.id); });
            if (type === "first_contact" && (state.definition.reactions || []).some(function (item) { return item.when.type === "first_contact"; })) return;
            if (isNeutralContact && used.has("any_neutral")) return;
            var after = eventType === "support" && state.definition.steps.find(function (step) { return step.type === "objective" || step.type === "guide"; });
            if (!isFirstContact && !isNeutralContact && !faction || eventType === "support" && !after) return;
            var responseId = isFirstContact ? "first_contact" : isNeutralContact ? "neutral_contact" : eventType + "_" + faction.id;
            while (state.definition.steps.some(function (step) { return step.id === responseId; }) || (state.definition.reactions || []).some(function (item) { return item.id === responseId; })) responseId += "_2";
            var speakerId = faction && Object.keys(state.definition.speakers || {}).find(function (key) { return state.definition.speakers[key].faction === faction.id; });
            var when = isFirstContact ? { type: type, targets: state.roster.factions.filter(function (item) { return item.relation === "neutral"; }).map(function (item) { return item.id; }) }
                : isNeutralContact ? { type: "contact", relation: "neutral" } : { type: eventType, target: faction.id };
            var reaction = { id: responseId, when: when, title_key: storyKey(responseId + "_title"), body_key: storyKey(responseId + "_body") };
            if (after) reaction.after = after.id;
            if (speakerId) reaction.speaker = speakerId;
            if (isFirstContact) reaction.outcome = "allied";
            state.definition.reactions = (state.definition.reactions || []).concat(reaction);
            markDirty(); renderSettings();
        }
        var addContactReaction = el("button", { type: "button" }, "Add contact response");
        addContactReaction.disabled = !state.roster.factions.some(function (item) { return !(state.definition.reactions || []).some(function (reaction) { return reaction.when.type === "contact" && reaction.when.target === item.id; }); });
        addContactReaction.addEventListener("click", function () { addReaction("contact"); });
        var addNeutralNegotiation = el("button", { type: "button" }, "Add any-neutral negotiation");
        addNeutralNegotiation.disabled = (state.definition.reactions || []).some(function (item) { return item.when.type === "contact" && item.when.relation === "neutral"; });
        addNeutralNegotiation.addEventListener("click", function () { addReaction("neutral_contact"); });
        var addFirstContact = el("button", { type: "button" }, "Add first-contact scene");
        addFirstContact.disabled = (state.definition.reactions || []).some(function (item) { return item.when.type === "first_contact"; }) || !state.roster.factions.some(function (item) { return item.relation === "neutral"; });
        addFirstContact.addEventListener("click", function () { addReaction("first_contact"); });
        var addSupportReaction = el("button", { type: "button" }, "Add support response");
        addSupportReaction.disabled = !state.definition.steps.some(function (step) { return ["objective", "guide"].includes(step.type); }) || !supportFactionOptions().some(function (option) { return !(state.definition.reactions || []).some(function (reaction) { return reaction.when.type === "support" && reaction.when.target === option.value; }); });
        addSupportReaction.addEventListener("click", function () { addReaction("support"); });
        reactions.append(addContactReaction, addNeutralNegotiation, addFirstContact, addSupportReaction); host.appendChild(reactions);
        host.appendChild(selectField("Opening step", state.definition.entry, state.definition.steps.map(stepOption), function (value) { state.definition.entry = value; markDirty(); }));
        host.appendChild(selectField("Menu guide opening", state.definition.menu_guide && state.definition.menu_guide.entry || "", [{ value: "", label: "Not set" }].concat(state.definition.steps.map(stepOption)), function (value) { if (!value) delete state.definition.menu_guide; else state.definition.menu_guide = Object.assign({}, state.definition.menu_guide, { entry: value, dismissible: true }); markDirty(); if (state.flow === "menu") resetPreview(); }));
        if (state.definition.menu_guide) host.appendChild(checkboxField("Player can dismiss this guide", state.definition.menu_guide.dismissible !== false, function (value) { state.definition.menu_guide.dismissible = value; markDirty(); }));
        Object.keys(state.definition.speakers || {}).forEach(function (speakerId) {
            var speaker = state.definition.speakers[speakerId], card = el("div", { class: "form-card" });
            card.appendChild(el("strong", {}, "Character · " + speakerId));
            var binding = speaker.faction ? "faction:" + speaker.faction : "";
            var bindingOptions = [{ value: "", label: "Custom character" }].concat(
                factionOptions().map(function (option) { return { value: "faction:" + option.value, label: "Roster · " + option.label }; })
            );
            card.appendChild(selectField("Name and portrait source", binding, bindingOptions, function (value) {
                delete speaker.faction;
                if (value.indexOf("faction:") === 0) { speaker.faction = value.slice(8); delete speaker.name_key; delete speaker.avatar; }
                else speaker.name = speaker.name || "New character";
                markDirty(); renderSettings();
            }));
            if (speaker.faction) {
                var boundFaction = speaker.faction && state.roster.factions.find(function (faction) { return faction.id === speaker.faction; });
                var boundName = boundFaction ? boundFaction.name : speaker.faction;
                var boundAvatar = boundFaction ? boundFaction.avatar || "null" : "null";
                var boundPortrait = el("img", { class: "speaker-avatar", alt: boundName, loading: "lazy" });
                boundPortrait.src = asset("gameplay/avatars/" + boundAvatar + ".webp");
                boundPortrait.addEventListener("error", function () { boundPortrait.hidden = true; });
                card.appendChild(boundPortrait);
                card.appendChild(el("small", {}, "Uses the selected faction's name and portrait."));
                var removeBoundSpeaker = el("button", { type: "button", class: "danger" }, "Remove character");
                removeBoundSpeaker.addEventListener("click", function () {
                    if (!confirm("Remove " + speakerId + "? Their dialogue will become narrator text.")) return;
                    delete state.definition.speakers[speakerId];
                    state.definition.steps.forEach(function (step) { if (step.speaker === speakerId) delete step.speaker; (step.lines || []).forEach(function (line) { if (line.speaker === speakerId) delete line.speaker; }); });
                    markDirty(); renderSettings(); renderInspector();
                });
                card.appendChild(removeBoundSpeaker); host.appendChild(card); return;
            }
            var displayName = speaker.name_key ? textValue(speaker.name_key, state.previewLanguage) : speaker.name || "";
            var portrait = el("img", { class: "speaker-avatar", alt: displayName || speakerId, loading: "lazy", hidden: !speaker.avatar });
            portrait.addEventListener("error", function () { portrait.hidden = true; });
            if (speaker.name_key) {
                catalogTextField(card, "Character name", speaker.name_key, 2);
                var fixedName = el("button", { type: "button" }, "Use one name for every language");
                fixedName.addEventListener("click", function () {
                    speaker.name = textValue(speaker.name_key, state.previewLanguage) || speaker.name || speakerId;
                    delete speaker.name_key; markDirty(); renderSettings();
                });
                card.appendChild(fixedName);
            } else {
                card.appendChild(inputField("Character name", speaker.name || "", function (value) { speaker.name = value; portrait.alt = value || speakerId; markDirty(); }, { input: true }));
                var localizeName = el("button", { type: "button" }, "Use a shared text key");
                localizeName.addEventListener("click", function () {
                    var keyBase = storyKey("speaker_" + speakerId + "_name"), key = keyBase, suffix = 2;
                    while (catalogValue("en", key)) key = keyBase + "_" + suffix++;
                    speaker.name_key = key;
                    delete speaker.name;
                    markDirty(); renderSettings();
                });
                card.appendChild(localizeName);
            }
            function updatePortrait(value) {
                if (value) speaker.avatar = value; else delete speaker.avatar;
                portrait.alt = speaker.name || speakerId; portrait.hidden = !speaker.avatar;
                if (speaker.avatar) portrait.src = asset("gameplay/avatars/" + speaker.avatar + ".webp");
                else portrait.removeAttribute("src");
            }
            if (speaker.avatar) portrait.src = asset("gameplay/avatars/" + speaker.avatar + ".webp");
            card.append(portrait);
            var avatarChoices = [{ value: "", label: "No portrait" }].concat(state.avatars.map(function (avatar) {
                return { value: avatar, label: avatar.split("_").map(function (part) { return part.charAt(0).toUpperCase() + part.slice(1); }).join(" ") };
            }));
            if (speaker.avatar && !state.avatars.includes(speaker.avatar)) avatarChoices.push({ value: speaker.avatar, label: "Missing asset · " + speaker.avatar });
            card.appendChild(selectField("Portrait", speaker.avatar || "", avatarChoices, function (value) { updatePortrait(value); markDirty(); }));
            var removeSpeaker = el("button", { type: "button", class: "danger" }, "Remove character");
            removeSpeaker.addEventListener("click", function () {
                if (!confirm("Remove " + (speaker.name || speakerId) + "? Their dialogue will become narrator text.")) return;
                delete state.definition.speakers[speakerId];
                state.definition.steps.forEach(function (step) {
                    if (step.speaker === speakerId) delete step.speaker;
                    (step.lines || []).forEach(function (line) { if (line.speaker === speakerId) delete line.speaker; });
                });
                markDirty(); renderSettings(); renderInspector();
            });
            card.appendChild(removeSpeaker);
            host.appendChild(card);
        });
        var addSpeaker = el("button", { type: "button" }, "Add character");
        addSpeaker.addEventListener("click", function () {
            var id = "character_" + (Object.keys(state.definition.speakers).length + 1);
            while (Object.prototype.hasOwnProperty.call(state.definition.speakers, id)) id = "character_" + (Number(id.slice("character_".length)) + 1);
            state.definition.speakers[id] = { name: "New character" }; markDirty(); renderSettings(); renderInspector();
        });
        host.appendChild(addSpeaker);
        var warning = el("small", {}, "Roster, map and faction positions are edited in Map."); host.appendChild(warning);
    }

    function portDefinitions(step) {
        if (step.type === "end") return [];
        if (step.type === "choice") return (step.choices || []).map(function (choice) { return { field: "choices." + choice.id + ".next", label: textValue(choice.label_key, state.previewLanguage) || choice.id, target: choice.next, className: "choice-wire" }; });
        var outputs = [{ field: "next", label: "Next", target: step.next }];
        (step.routes || []).forEach(function (route, index) {
            var when = route.when || {}, label = "If condition";
            if (when.choice) {
                var decision = state.definition.steps.find(function (item) { return item.id === when.choice; });
                var answer = decision && (decision.choices || []).find(function (item) { return item.id === when.equals; });
                label = "If " + (answer && textValue(answer.label_key, state.previewLanguage) || when.equals || when.choice);
            } else if (when.fact) label = "If " + when.fact.replace(/_/g, " ") + " ≥ " + String(when.gte);
            outputs.push({ field: "routes." + index + ".next", label: label, target: route.next, className: "route-wire" });
        });
        return outputs;
    }
    function nodeSummary(step) {
        var firstLine = step.lines && step.lines[0];
        var storyText = textValue(firstLine && firstLine.body_key || step.hint_key || step.body_key, state.previewLanguage);
        if (step.type === "scene") return [step.lines && step.lines.length > 1 ? step.lines.length + " dialogue lines" : "Story scene", storyText].filter(Boolean).join(" · ");
        if (step.type === "choice") return [(step.choices || []).length + " decision paths", textValue(step.body_key, state.previewLanguage)].filter(Boolean).join(" · ");
        if (step.type === "end") return ["Episode ending", textValue(step.body_key, state.previewLanguage)].filter(Boolean).join(" · ");
        var trigger = step.trigger || {};
        var definition = triggerTypes.find(function (item) { return item.value === trigger.type; });
        var parts = [definition ? definition.label : "Set a condition"];
        if (trigger.type === "contact" && Array.isArray(trigger.targets)) parts[0] += " · " + (trigger.value || 1) + " of " + trigger.targets.join(", ");
        else if (["contact", "defeated", "attack", "hover", "camera_target"].includes(trigger.type) && trigger.target) parts[0] += " · " + trigger.target;
        if (trigger.type === "structure_level" && trigger.kind) parts[0] += " · " + trigger.kind;
        if (trigger.type === "fleet" && trigger.unit) parts[0] += " · " + trigger.unit + (trigger.target ? " to " + trigger.target : "");
        if (trigger.type === "resource_transfer") parts[0] += (trigger.recipient ? " · to " + trigger.recipient : "") + (trigger.resources && trigger.resources.length ? " · " + trigger.resources.join(" + ") : "");
        else if (trigger.type === "ui" && typeof trigger.action === "string") parts[0] += " · " + trigger.action.replace(/_/g, " ");
        if (Number.isFinite(trigger.value)) parts.push((trigger.type === "elapsed" ? trigger.value + "s" : "≥ " + trigger.value.toLocaleString()));
        if (trigger.scope && trigger.type !== "troops") parts.push({ step: "this step", episode: "this episode", total: "all time" }[trigger.scope] || trigger.scope);
        if (step.guide && typeof step.guide.gesture === "string" && typeof step.guide.target === "string") parts.push("hand: " + step.guide.gesture + " " + step.guide.target.replace(/_/g, " "));
        if (storyText) parts.push(storyText);
        return parts.join(" · ");
    }
    function renderPreviewBranchOptions() {
        var wrapper = $("#previewBranchWrap"), select = $("#previewBranchSelect");
        if (!wrapper || !select || !state.definition) return;
        var step = state.definition.steps.find(function (item) { return item.id === state.selected; });
        var options = step ? (step.routes || []).reduce(function (result, route, index) {
            var condition = route.when || {};
            var destination = state.definition.steps.find(function (item) { return item.id === route.next; });
            if (condition.choice) {
                var decision = state.definition.steps.find(function (item) { return item.id === condition.choice && item.type === "choice"; });
                var answer = decision && (decision.choices || []).find(function (item) { return item.id === condition.equals; });
                if (answer) result.push({ value: String(index), label: (translated(decision.title_key, state.previewLanguage) || decision.id) + " · " + (translated(answer.label_key, state.previewLanguage) || answer.id) + " → " + (translated(destination && destination.title_key, state.previewLanguage) || route.next) });
            } else if (condition.fact && Number.isFinite(condition.gte)) {
                result.push({ value: String(index), label: condition.fact.replace(/_/g, " ") + " ≥ " + condition.gte + " → " + (translated(destination && destination.title_key, state.previewLanguage) || route.next) });
            }
            return result;
        }, []) : [];
        if (state.previewBranch && state.previewBranch.stepId !== state.selected) state.previewBranch = null;
        var selected = state.previewBranch && options.some(function (option) { return option.value === String(state.previewBranch.routeIndex); }) ? String(state.previewBranch.routeIndex) : "";
        if (state.previewBranch && !selected) state.previewBranch = null;
        wrapper.hidden = options.length === 0;
        optionList(select, [{ value: "", label: "Current preview state" }].concat(options), selected);
    }
    function previewContext(stepId) {
        var step = state.definition.steps.find(function (item) { return item.id === stepId; });
        var route = state.previewBranch && state.previewBranch.stepId === stepId && step && (step.routes || [])[state.previewBranch.routeIndex];
        var when = route && route.when || {}, facts = Object.assign({}, state.facts), choices = {};
        if (when.choice) choices[when.choice] = when.equals;
        if (when.fact) facts[when.fact] = Math.max(Number(facts[when.fact] || 0), Number(when.gte));
        return { facts: facts, choices: choices };
    }
    function renderGraph() {
        if (!state.definition) return;
        ensureLayout();
        var nodes = $("#nodes"), svg = $("#wires"), oldSelection = state.selected;
        var completedSteps = new Set(state.machine ? state.machine.state.completed : []);
        var focusedStep = document.activeElement && document.activeElement.dataset && document.activeElement.dataset.stepId;
        nodes.replaceChildren();
        var group = svg.querySelector("g");
        if (group) group.remove();
        group = document.createElementNS("http://www.w3.org/2000/svg", "g"); svg.appendChild(group);
        nodes.style.transform = "translate(" + state.pan.x + "px," + state.pan.y + "px) scale(" + state.zoom + ")";
        group.setAttribute("transform", "translate(" + state.pan.x + " " + state.pan.y + ") scale(" + state.zoom + ")");
        var byId = Object.create(null);
        var errorsByStep = Object.create(null);
        var warningsByStep = Object.create(null);
        (state.validation.errors || []).forEach(function (issue) {
            if (!issue.step) return;
            errorsByStep[issue.step] = (errorsByStep[issue.step] || 0) + 1;
        });
        (state.validation.warnings || []).forEach(function (issue) {
            if (!issue.step) return;
            warningsByStep[issue.step] = (warningsByStep[issue.step] || 0) + 1;
        });
        state.definition.steps.forEach(function (step) { byId[step.id] = step; });
        state.definition.steps.forEach(function (step) {
            var pos = state.definition.layout[step.id] || defaultLayout(0);
            var rootLabel = step.id === state.definition.entry ? "IN-GAME START" : state.definition.menu_guide && step.id === state.definition.menu_guide.entry ? "MENU START" : step.id;
            var stepErrors = errorsByStep[step.id] || 0;
            var stepWarnings = warningsByStep[step.id] || 0;
            var lines = Array.isArray(step.lines) ? step.lines : [];
            var speakers = state.definition.speakers || {};
            var castIds = Array.from(new Set(lines.map(function (line) { return line && line.speaker || step.speaker; }).concat(step.speaker || []).filter(Boolean)));
            var castNames = castIds.map(function (speakerId) {
                var speaker = speakers[speakerId] || {};
                return speaker.name_key ? textValue(speaker.name_key, state.previewLanguage) : speaker.name || speakerId;
            });
            var summary = nodeSummary(step);
            var card = el("article", { class: "node" + (step.id === oldSelection ? " selected" : "") + (step.id === state.playingStep ? " playing" : "") + (completedSteps.has(step.id) ? " visited" : "") + (step.id === state.definition.entry ? " is-entry" : "") + (state.definition.menu_guide && step.id === state.definition.menu_guide.entry ? " is-menu-entry" : "") + (stepErrors ? " has-error" : "") + (stepWarnings ? " has-warning" : ""), dataset: { stepId: step.id, stepType: step.type }, tabindex: "0", role: "group", "aria-label": step.type + ": " + (translated(step.title_key, state.previewLanguage) || step.id) + " · " + summary + (castNames.length ? " · " + castNames.join(", ") : "") });
            card.style.left = pos.x + "px"; card.style.top = pos.y + "px";
            var head = el("header", { class: "node-hd" }); head.append(el("span", {}, step.type), el("small", { title: step.id, "aria-label": step.id }, rootLabel)); card.appendChild(head);
            card.appendChild(el("div", { class: "node-title" }, translated(step.title_key, state.previewLanguage) || step.title_key || "Untitled"));
            card.appendChild(el("div", { class: "node-summary", title: summary }, summary));
            if (castIds.length) {
                var cast = el("div", { class: "node-cast", "aria-hidden": "true" });
                castIds.slice(0, 3).forEach(function (speakerId) {
                    var speaker = speakers[speakerId] || {};
                    var name = speaker.name_key ? textValue(speaker.name_key, state.previewLanguage) : speaker.name || speakerId;
                    var badge = el("span", { class: "node-cast__member", title: name });
                    if (speaker.avatar) {
                        var portrait = el("img", { src: asset("gameplay/avatars/" + speaker.avatar + ".webp"), alt: "", loading: "lazy" });
                        portrait.addEventListener("error", function () { badge.textContent = String(name || speakerId).trim().charAt(0).toUpperCase(); portrait.remove(); });
                        badge.appendChild(portrait);
                    } else badge.textContent = String(name || speakerId).trim().charAt(0).toUpperCase();
                    cast.appendChild(badge);
                });
                if (castIds.length > 3) cast.appendChild(el("span", { class: "node-cast__more" }, "+" + (castIds.length - 3)));
                card.appendChild(cast);
            }
            if (stepErrors) card.appendChild(el("div", { class: "node-error", role: "img", "aria-label": stepErrors + " validation errors" }, stepErrors + (stepErrors === 1 ? " issue" : " issues")));
            if (stepWarnings) card.appendChild(el("div", { class: "node-warning", role: "img", "aria-label": stepWarnings + " validation warnings" }, stepWarnings + (stepWarnings === 1 ? " note" : " notes")));
            var input = el("button", { type: "button", class: "pin in", title: "Connect here", "aria-label": "Connect a path to " + (translated(step.title_key, state.previewLanguage) || step.id), dataset: { in: step.id } });
            input.addEventListener("keydown", connectPinByKeyboard); card.appendChild(input);
            portDefinitions(step).forEach(function (port, index) {
                var row = el("div", { class: "port-row" + (port.className ? " " + port.className : "") });
                row.appendChild(el("span", { class: "port-label", title: port.label }, port.label));
                var destination = byId[port.target];
                row.appendChild(el("small", { title: destination ? stepLabel(destination) : "Unconnected" }, destination ? stepLabel(destination) : "Unconnected"));
                var output = el("button", { type: "button", class: "pin out", title: "Drag to connect", "aria-label": "Connect " + port.label + " from " + (translated(step.title_key, state.previewLanguage) || step.id), dataset: { out: step.id, field: port.field, outputIndex: index } });
                output.addEventListener("keydown", connectPinByKeyboard); row.appendChild(output);
                card.appendChild(row);
            });
            card.addEventListener("click", function (event) { if (event.target.closest(".pin")) return; selectStep(step.id); });
            card.addEventListener("keydown", function (event) {
                if (event.target !== card || (event.key !== "Enter" && event.key !== " ")) return;
                event.preventDefault(); selectStep(step.id);
            });
            head.addEventListener("pointerdown", function (event) { if (event.button !== 0) return; event.preventDefault(); state.drag = { kind: "node", id: step.id, x: event.clientX, y: event.clientY, origin: { x: pos.x, y: pos.y } }; head.setPointerCapture(event.pointerId); });
            card.addEventListener("pointerdown", function (event) {
                var output = event.target.closest(".pin.out");
                if (!output) return;
                event.preventDefault(); event.stopPropagation();
                state.keyboardWire = null; $("#stage").dataset.connecting = "false";
                $(".graph-hint").textContent = "Drag outputs to inputs · keyboard: Enter on output, then Enter on destination · Escape cancels";
                state.wire = { source: output.dataset.out, field: output.dataset.field, outputIndex: Number(output.dataset.outputIndex) };
                output.setPointerCapture(event.pointerId);
            });
            nodes.appendChild(card);
        });
        if (focusedStep) {
            var focusTarget = $("#nodes [data-step-id=" + CSS.escape(focusedStep) + "]");
            if (focusTarget) focusTarget.focus({ preventScroll: true });
        }
        drawWires();
        $("#stepCount").textContent = state.definition.steps.length + " steps";
        $("#episodeLabel").textContent = episodeId.replace(/_/g, " ").toUpperCase();
        renderPreviewBranchOptions();
    }
    function selectStep(id) {
        state.selected = id;
        renderInspector(); renderGraph(); renderPreview(id);
        revealStep(id);
    }
    function revealStep(id) {
        var stage = $("#stage"), node = $("#nodes [data-step-id=" + CSS.escape(id) + "]");
        if (!node) return;
        var frame = stage.getBoundingClientRect(), rect = node.getBoundingClientRect(), margin = 36, dx = 0, dy = 0;
        if (rect.left < frame.left + margin) dx = frame.left + margin - rect.left;
        else if (rect.right > frame.right - margin) dx = frame.right - margin - rect.right;
        if (rect.top < frame.top + margin) dy = frame.top + margin - rect.top;
        else if (rect.bottom > frame.bottom - margin) dy = frame.bottom - margin - rect.bottom;
        if (dx || dy) { state.pan.x += dx; state.pan.y += dy; renderGraph(); }
    }
    function setDestination(wire, destination) {
        var source = state.definition.steps.find(function (step) { return step.id === wire.source; });
        if (!source) return;
        var choiceMatch = wire.field.match(/^choices\.([^.]+)\.next$/);
        if (choiceMatch) {
            var choice = (source.choices || []).find(function (item) { return item.id === choiceMatch[1]; });
            if (choice) choice.next = destination;
        } else setPath(source, wire.field, destination);
    }
    function connectPinByKeyboard(event) {
        if (event.key !== "Enter" && event.key !== " ") return;
        event.preventDefault();
        var pin = event.currentTarget;
        if (pin.dataset.out) {
            state.keyboardWire = { source: pin.dataset.out, field: pin.dataset.field };
            $("#stage").dataset.connecting = "true";
            $(".graph-hint").textContent = "Choose a destination input · Escape cancels";
            return;
        }
        if (!state.keyboardWire) {
            $(".graph-hint").textContent = "Focus an output and press Enter to begin connecting";
            return;
        }
        var destination = pin.dataset.in;
        setDestination(state.keyboardWire, destination);
        state.keyboardWire = null;
        $("#stage").dataset.connecting = "false";
        $(".graph-hint").textContent = "Drag outputs to inputs · keyboard: Enter on output, then Enter on destination · Escape cancels";
        markDirty();
        var target = Array.from($("#nodes").querySelectorAll(".pin.in")).find(function (candidate) { return candidate.dataset.in === destination; });
        if (target) target.focus({ preventScroll: true });
    }
    function drawWires() {
        var svg = $("#wires"), group = svg.querySelector("g");
        if (!group) return;
        group.replaceChildren();
        var trace = state.machine && state.machine.state.completed || [];
        var currentStep = state.machine && state.machine.state.id;
        var stageRect = $("#stage").getBoundingClientRect();
        state.definition.steps.forEach(function (step) {
            var node = $("#nodes [data-step-id=" + CSS.escape(step.id) + "]");
            if (!node) return;
            portDefinitions(step).forEach(function (port, index) {
                var target = port.target && $("#nodes [data-step-id=" + CSS.escape(port.target) + "]");
                if (!target) return;
                var output = node.querySelectorAll(".pin.out")[index], input = target.querySelector(".pin.in");
                if (!output || !input) return;
                var a = output.getBoundingClientRect(), b = input.getBoundingClientRect();
                var x1 = (a.left + a.width / 2 - stageRect.left - state.pan.x) / state.zoom;
                var y1 = (a.top + a.height / 2 - stageRect.top - state.pan.y) / state.zoom;
                var x2 = (b.left + b.width / 2 - stageRect.left - state.pan.x) / state.zoom;
                var y2 = (b.top + b.height / 2 - stageRect.top - state.pan.y) / state.zoom;
                var path = document.createElementNS("http://www.w3.org/2000/svg", "path");
                path.setAttribute("d", "M" + x1 + " " + y1 + " C" + (x1 + 70) + " " + y1 + " " + (x2 - 70) + " " + y2 + " " + x2 + " " + y2);
                if (port.className) path.classList.add(port.className);
                var traceIndex = trace.indexOf(step.id);
                if (traceIndex >= 0 && (trace[traceIndex + 1] === port.target || (traceIndex === trace.length - 1 && currentStep === port.target))) path.classList.add("path-traversed");
                group.appendChild(path);
            });
        });
    }
    function addStep(type, branch) {
        var index = state.definition.steps.length, id = uniqueId(type + "_" + (index + 1), state.definition.steps), previous = branch ? branch.step : state.definition.steps.find(function (step) { return step.id === state.selected; });
        if (!branch && previous && previous.type === "choice") { notice("Choose the answer to branch from in the inspector."); return; }
        var ending = state.definition.steps.find(function (item) { return item.type === "end"; });
        var output = branch ? branch.output : previous && previous.type !== "end" ? previous : null;
        var previousDestination = output && output.next || "";
        var next = previousDestination || ending && ending.id || "";
        if (type === "end" && previousDestination && !confirm("This path already continues to " + previousDestination + ". Replace that route with an ending?")) return;
        var position = defaultLayout(index), origin = previous && state.definition.layout[previous.id];
        if (output && origin) {
            position = { x: origin.x + 320, y: origin.y };
            var newHeight = type === "choice" ? 230 : type === "objective" || type === "guide" ? 190 : 160;
            var occupiedNodes = state.definition.steps.map(function (item, itemIndex) {
                var point = state.definition.layout[item.id] || defaultLayout(itemIndex);
                var node = $("#nodes [data-step-id=" + CSS.escape(item.id) + "]");
                return { point: point, width: node && node.offsetWidth || 224, height: node && node.offsetHeight || 160 };
            });
            for (var attempt = 0; attempt < state.definition.steps.length; attempt++) {
                var collision = occupiedNodes.find(function (occupied) {
                    return position.x < occupied.point.x + occupied.width + 32 && position.x + 256 > occupied.point.x &&
                        position.y < occupied.point.y + occupied.height + 32 && position.y + newHeight + 32 > occupied.point.y;
                });
                if (!collision) break;
                position.y = collision.point.y + collision.height + 40;
            }
        }
        var menuAction = inGameUiTargets()[0] || "menu_campaign";
        var step = { id: id, type: type, title_key: storyKey(id + "_title"), body_key: storyKey(id + "_body"), speaker: Object.keys(state.definition.speakers)[0] };
        if (type === "end") { delete step.body_key; }
        else if (type === "choice") { step.choices = [{ id: "first", label_key: storyKey(id + "_first"), next: next }, { id: "second", label_key: storyKey(id + "_second"), next: next }]; }
        else if (type === "objective" || type === "guide") {
            delete step.body_key; step.hint_key = storyKey(id + "_hint");
            step.trigger = state.flow === "menu" ? { type: "ui", action: menuAction, scope: "step" } : { type: "territory", value: 1, scope: "step" };
            if (type === "guide") step.guide = state.flow === "menu" ? { kind: "ui", target: menuAction, gesture: "tap" } : { kind: "world", target: "expand", gesture: "tap" };
            if (state.flow !== "menu") step.marker = { target: "player" };
            step.next = next;
        }
        else step.next = next;
        if (output) output.next = id;
        state.definition.steps.push(step);
        state.definition.layout[id] = position;
        if (type === "end") state.definition.steps.forEach(function (item) { if (item.type !== "end" && !item.next && item.type !== "choice") item.next = id; });
        markDirty(); selectStep(id); revealStep(id);
    }
    function renderPalette() {
        var host = $("#palette"); host.replaceChildren();
        types.forEach(function (type) { var button = el("button", { type: "button" }, type === "scene" ? "Scene" : type === "choice" ? "Decision" : type === "objective" ? "Objective" : type === "guide" ? "Guide" : "Ending"); button.addEventListener("click", function () { addStep(type); }); host.appendChild(button); });
    }

    function paintPreview(updateMachine) {
        if (!state.machine || state.validation.errors.length) return;
        $("#previewFrame").dataset.device = $("#device").value;
        state.facts.touch_controls = $("#device").value === "mobile" ? 1 : 0;
        var model;
        var previousStep = state.playingStep;
        try { model = updateMachine === false ? state.machine.view() : state.machine.update(state.facts, state.ui); }
        catch (error) { $("#previewStatus").textContent = "Preview unavailable: " + error.message; return; }
        state.playingStep = model.step.id;
        var cancelBuildMode = $("#sow-hud [data-command='cancel_building_mode']");
        if (cancelBuildMode) cancelBuildMode.hidden = !model.step.trigger || model.step.trigger.action !== "cancel_building_mode";
        var actionRatio = Number.isFinite(model.step.attack_ratio_on_enter) ? model.step.attack_ratio_on_enter : null;
        if (model.step.id !== state.previewActionStep || actionRatio !== state.previewActionRatio) {
            state.previewActionStep = model.step.id;
            state.previewActionRatio = actionRatio;
            if (actionRatio != null) $("#sow-hud-slider").value = String(Math.round(actionRatio * 100));
        }
        var anchor = previewAnchor(model.step);
        var zoomMode = $("#device").value === "mobile" ? "pinch" : "wheel";
        var zoomGuide = model.step.guide && ["zoom_in", "zoom_out"].includes(model.step.guide.gesture);
        var deviceHint = model.step.guide && ["drag", "hover"].includes(model.step.guide.gesture)
            ? translated(stepTextKey(model.step, "_" + ($("#device").value === "mobile" ? "mobile" : "desktop") + "_hint"), state.previewLanguage) : null;
        var previewHint = deviceHint || "", guideMetric = null;
        if (zoomGuide) {
            var zoomTarget = model.step.trigger && model.step.trigger.type === "zoom_out_complete" ? window.SOWCampaign.zoomOutTarget(state.facts, model.step.trigger.value)
                : model.step.trigger && model.step.trigger.type === "zoom_in_complete" ? state.facts.camera_zoom_target : null;
            if (zoomTarget != null && Number.isFinite(Number(zoomTarget)) && Number.isFinite(Number(state.facts.camera_zoom))) {
                guideMetric = {
                    current: Number(state.facts.camera_zoom),
                    target: Number(zoomTarget),
                    direction: model.step.guide.gesture === "zoom_in" ? "min" : "max"
                };
            }
        } else if (model.step.trigger && model.step.trigger.type === "camera_target" && Number.isFinite(Number(model.step.trigger.distance)) && Number.isFinite(Number(state.facts.camera_target_distance))) {
            guideMetric = { text: previewMetric(stepTextKey(model.step, "_progress"), { current: Math.ceil(Number(state.facts.camera_target_distance)), target: model.step.trigger.distance }) };
        }
        try { state.renderer.render(model, { anchor: anchor, reducedMotion: $("#reducedMotion").checked, direction: rtlLanguages.has(state.previewLanguage.toLowerCase().split("-")[0]) ? "rtl" : "ltr", localeScript: previewLocaleScript(state.previewLanguage), zoomMode: zoomMode, hintOverride: previewHint, guideMetric: guideMetric }); }
        catch (error) { $("#previewStatus").textContent = "Preview unavailable: " + error.message; return; }
        $("#engineState").textContent = JSON.stringify({ step: model.step.id, type: model.step.type, campaign_assault_on_enter: model.step.campaign_assault_on_enter || null, reaction: model.reaction || null, progress: model.progress, choices: model.state.choices, reactionsShown: model.state.reactionsShown }, null, 2);
        var guideTarget = model.step.guide && model.step.guide.kind === "ui" ? model.step.guide.target : "";
        var requiredMenu = /^map_(?:build|upgrade)_/.test(guideTarget) ? "build" : "";
        var menuHint = requiredMenu && $("#sow-hud").dataset.previewMapMenu !== requiredMenu ? " · open the " + requiredMenu + " submenu in the preview to reveal this guide" : "";
        if (!menuHint && guideTarget === "campaign_replay") {
            var replayTarget = window.SOWCampaign.resolveUiTarget("campaign_replay", document, episodeId);
            if (!replayTarget || !replayTarget.getClientRects().length) menuHint = " · open Campaign in the preview to reveal Replay";
        }
        var previewTitle = translated(model.step.title_key, state.previewLanguage) || model.step.id.replace(/_/g, " ");
        var assaultHint = model.step.campaign_assault_on_enter ? " · on entry: " + model.step.campaign_assault_on_enter.attacker_team + " attacks " + (model.step.campaign_assault_on_enter.target === "player" ? "player" : factionName(model.step.campaign_assault_on_enter.target)) : "";
        $("#previewStatus").textContent = (state.demoBackup ? "Sample preview · not saved — " : "Previewing · ") + previewTitle + menuHint + assaultHint + (state.validation.errors.length ? " · draft needs fixes before saving" : "");
        if (previousStep !== model.step.id) renderGraph();
        renderFactControls(model);
    }
    function previewLocaleScript(locale) {
        var code = String(locale || "").toLowerCase();
        if (code === "ar") return "arabic";
        if (["zh-cn", "ja", "ko"].includes(code)) return "cjk";
        return code === "ru" ? "cyrillic" : "latin";
    }
    function syncPreview() {
        if (state.validation.errors.length) { $("#previewStatus").textContent = "Fix validation errors to preview this draft"; return; }
        if (state.machine && state.machine.replaceDefinition(state.definition)) paintPreview();
        else renderPreview();
    }
    function previewAnchor(step) {
        var frame = $("#previewFrame");
        frame.querySelectorAll(".is-guide-target").forEach(function (marker) { marker.classList.remove("is-guide-target"); });
        if (!step.guide) return null;
        var frameRect = frame.getBoundingClientRect(), guide = step.guide;
        if (["zoom_in", "zoom_out"].includes(guide.gesture)) return { x: frame.clientWidth * 0.5, y: frame.clientHeight * 0.5 };
        var target;
        if (guide.kind === "world") target = previewWorldMarker(frame, guide.target, step);
        else {
            target = window.SOWCampaign.resolveUiTarget(guide.target, document, episodeId);
            if (target && !frame.contains(target)) target = null;
        }
        if (!target || target.getClientRects().length === 0) return null;
        if (guide.kind === "world") target.classList.add("is-guide-target");
        var rect = target.getBoundingClientRect();
        var anchor = guide.kind === "ui" ? window.SOWCampaign.resolveUiAnchor(target) : { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
        anchor.x -= frameRect.left; anchor.y -= frameRect.top;
        if (anchor.cutout) { anchor.cutout.x -= frameRect.left; anchor.cutout.y -= frameRect.top; }
        if (guide.gesture === "drag" && guide.to) {
            var end;
            if (guide.kind === "world") end = previewWorldMarker(frame, guide.to, step);
            else {
                end = window.SOWCampaign.resolveUiTarget(guide.to, document, episodeId);
                if (end && !frame.contains(end)) end = null;
            }
            if (!end || end.disabled || !end.getClientRects().length) return null;
            var endAnchor = guide.kind === "ui" ? window.SOWCampaign.resolveUiAnchor(end) : end.getBoundingClientRect();
            anchor.toX = (guide.kind === "ui" ? endAnchor.x : endAnchor.left + endAnchor.width / 2) - frameRect.left;
            anchor.toY = (guide.kind === "ui" ? endAnchor.y : endAnchor.top + endAnchor.height / 2) - frameRect.top;
        }
        return anchor;
    }
    function previewWorldMarker(frame, target, step) {
        if (target === "expand") return frame.querySelector(".player-base");
        var factions = state.roster && state.roster.factions || [];
        if (target === "nameplate") {
            var nameplateTarget = step.marker && step.marker.target || step.trigger && step.trigger.target;
            var nameplateFaction = factions.find(function (item) { return item.id === nameplateTarget; });
            return nameplateFaction && Array.from(frame.querySelectorAll(".sample-faction")).find(function (marker) { return marker.dataset.factionId === nameplateFaction.id; }) || null;
        }
        if (target === "player") {
            var playerTarget = step.trigger && ["camera_target", "hover"].includes(step.trigger.type) && step.trigger.target
                ? step.trigger.target : step.marker && step.marker.target;
            if (!playerTarget || playerTarget === "player") return frame.querySelector(".player-base");
            var playerFaction = factions.find(function (item) { return item.id === playerTarget; });
            return playerFaction && Array.from(frame.querySelectorAll(".sample-faction")).find(function (marker) { return marker.dataset.factionId === playerFaction.id; }) || null;
        }
        var observed = step.trigger && step.trigger.type === "contact" ? state.facts && state.facts.contact_faction_ids || [] : state.facts && state.facts.defeated_faction_ids || [];
        var requested = target === "target_action" && (Array.isArray(step.trigger && step.trigger.targets)
            ? step.trigger.targets.find(function (name) { return !observed.includes(name); })
            : step.trigger && step.trigger.target || step.marker && step.marker.target !== "player" && step.marker.target);
        var faction = requested && factions.find(function (item) { return item.id === requested; });
        if (!faction && state.roster) {
            var spawn = state.roster.player_spawn;
            faction = factions.filter(function (item) { return item.relation !== "allied"; }).sort(function (a, b) {
                return Math.hypot(a.x - spawn[0], a.y - spawn[1]) - Math.hypot(b.x - spawn[0], b.y - spawn[1]);
            })[0];
        }
        return faction && Array.from(frame.querySelectorAll(".sample-faction")).find(function (marker) { return marker.dataset.factionId === faction.id; }) || null;
    }
    function focusPreviewTarget() {
        if (state.flow !== "episode" || !state.machine) return;
        state.previewFocusActive = true;
        applyPreviewFocus();
        paintPreview(false);
    }
    function applyPreviewFocus() {
        if (!state.previewFocusActive || !state.machine) return;
        var step = state.machine.view().step, observed = step.trigger && step.trigger.type === "contact" ? state.facts && state.facts.contact_faction_ids || [] : state.facts && state.facts.defeated_faction_ids || [];
        var target = step.guide && step.guide.kind === "world" && step.guide.target === "target_action"
            ? Array.isArray(step.trigger && step.trigger.targets) ? step.trigger.targets.find(function (name) { return !observed.includes(name); }) : step.trigger && step.trigger.target || step.marker && step.marker.target
            : step.marker && step.marker.target;
        var marker = target === "player" ? $("#campaignPreviewMarkers .player-base")
            : target ? Array.from(document.querySelectorAll("#campaignPreviewMarkers .sample-faction")).find(function (item) { return item.dataset.factionId === target; })
                : null;
        if (!marker) return;
        var layer = $("#campaignPreviewLayer"), scale = 1.8;
        layer.classList.toggle("has-map-guide", Boolean(step.guide));
        var x = layer.offsetWidth * parseFloat(marker.style.left) / 100 - layer.offsetWidth / 2;
        var y = layer.offsetHeight * parseFloat(marker.style.top) / 100 - layer.offsetHeight / 2;
        layer.style.setProperty("--map-focus-x", (-x * scale) + "px");
        layer.style.setProperty("--map-focus-y", (-y * scale) + "px");
        layer.style.setProperty("--map-focus-scale", scale);
        $("#campaignPreviewMarkers .is-camera-focus")?.classList.remove("is-camera-focus");
        marker.classList.add("is-camera-focus");
    }
    function renderPreview(forceStep) {
        if (!state.definition) return;
        if (state.validation.errors.length) { $("#previewStatus").textContent = "Fix validation errors to preview this draft"; return; }
        var frame = $("#previewFrame"); frame.dataset.device = $("#device").value;
        frame.dataset.flow = state.flow;
        $("#sow-menu").hidden = state.flow !== "menu";
        var steps = Array.isArray(state.definition.steps) ? state.definition.steps : [];
        var candidates = [forceStep, state.machine && state.machine.state.id, state.selected, flowEntry(), state.definition.entry];
        var start = candidates.find(function (id) { return id && steps.some(function (step) { return step && step.id === id; }); });
        if (!start) {
            $("#previewStatus").textContent = "Preview waiting for a valid opening step.";
            return;
        }
        var requestedEntry = flowEntry() || state.definition.entry;
        if (!steps.some(function (step) { return step && step.id === requestedEntry; })) requestedEntry = start;
        state.playingStep = null;
        try { state.machine = window.SOWCampaign.create(state.definition, requestedEntry, state.roster); }
        catch (error) { $("#previewStatus").textContent = "Preview unavailable: " + error.message; return; }
        state.previewActionStep = null; state.previewActionRatio = null;
        state.facts = state.facts || {};
        var context = previewContext(start);
        state.machine.jump(start, context.facts, context.choices);
        if (!state.renderer) state.renderer = window.SOWCampaignView.mount($("#previewRoot"), {
            translate: function (key) { return translated(key, state.previewLanguage); }, asset: asset,
            roster: function () { return state.roster; },
            onContinue: function () { var step = state.machine.view().step; state.machine.advance(null, step.id); paintPreview(); },
            onChoice: function (choice) { var step = state.machine.view().step; state.machine.advance(choice, step.id); paintPreview(); },
            onFocus: focusPreviewTarget, onDismiss: null
        });
        paintPreview();
    }
    function renderFactControls(model) {
        var host = $("#factControls"); host.replaceChildren();
        var trigger = model.step.trigger;
        var timedRoute = (model.step.routes || []).some(function (route) { return route.when && route.when.fact === "elapsed_seconds"; });
        var canTick = Boolean((trigger && trigger.type === "elapsed") || timedRoute || Number(model.step.advance_delay_seconds) > 0);
        var simulateButton = $("#simulateBtn"), tickButton = $("#tickBtn");
        simulateButton.hidden = !trigger;
        simulateButton.disabled = !trigger;
        tickButton.hidden = !canTick;
        tickButton.disabled = !canTick;
        if (!trigger) host.appendChild(el("small", {}, timedRoute ? "Advance game time to test this route." : "This beat waits for dialogue, a decision or an incoming story event."));
        else {
            var progress = model.progress, description = trigger.type;
            if (trigger.type === "contact" && trigger.targets) description += " · contact " + (trigger.value || 1) + " of " + trigger.targets.length + " factions";
            if (trigger.type === "fleet") description += " · " + (trigger.unit || "any ship") + (trigger.target ? " to " + factionName(trigger.target) : "");
            if (trigger.type === "resource_transfer") description += (trigger.recipient ? " · to " + factionName(trigger.recipient) : " · any recipient") + (trigger.resources ? " · " + trigger.resources.join(" + ") : " · any resources");
            host.appendChild(el("small", {}, "Waiting for " + description + " · " + progress.current + " / " + progress.target));
            if (Number(model.step.advance_delay_seconds) > 0) host.appendChild(el("small", {}, "Advance game time to test the pause before the next step."));
            if (!["troops", "elapsed", "contact", "eliminated", "fleet", "resource_transfer", "alliance"].includes(trigger.type)) {
                var button = el("button", { type: "button" }, "+1 " + trigger.type);
                button.addEventListener("click", function () { simulateObjective(1); });
                host.appendChild(button);
            }
            if (trigger.type === "contact") (trigger.targets || [trigger.target]).filter(Boolean).forEach(function (factionId) {
                var contact = el("button", { type: "button" }, "Contact · " + factionName(factionId));
                contact.disabled = (state.facts.contact_faction_ids || []).includes(factionId);
                contact.addEventListener("click", function () { simulateObjective(1, { factionId: factionId }); });
                host.appendChild(contact);
            });
            if (trigger.type === "eliminated") {
                var eliminatedTarget = trigger.target || "player";
                var eliminate = el("button", { type: "button" }, "Simulate elimination · " + (eliminatedTarget === "player" ? "Player" : factionName(eliminatedTarget)));
                eliminate.disabled = (state.facts.eliminated_faction_ids || []).includes(eliminatedTarget);
                eliminate.addEventListener("click", function () { simulateObjective(1); });
                host.appendChild(eliminate);
            }
            if (trigger.type === "alliance") factionOptions().forEach(function (option) {
                var alliance = el("button", { type: "button" }, "Alliance · " + option.label);
                alliance.disabled = (state.facts.alliance_faction_ids || []).includes(option.value);
                alliance.addEventListener("click", function () { simulateObjective(1, { factionId: option.value }); });
                host.appendChild(alliance);
            });
            if (trigger.type === "fleet") ["TransportShip", "TradeShip", "Warship"].forEach(function (unit) {
                if (trigger.target && unit === "TransportShip") {
                    [trigger.target, factionOptions().find(function (option) { return option.value !== trigger.target; })?.value].filter(Boolean).forEach(function (target) {
                        var ship = el("button", { type: "button" }, "Simulate · " + unit + " to " + factionName(target));
                        ship.addEventListener("click", function () { simulateObjective(1, { unit: unit, target: target }); });
                        host.appendChild(ship);
                    });
                } else {
                    var ship = el("button", { type: "button" }, "Simulate · " + unit);
                    ship.addEventListener("click", function () { simulateObjective(1, { unit: unit }); });
                    host.appendChild(ship);
                }
            });
            if (trigger.type === "resource_transfer") {
                var recipient = trigger.recipient || (state.roster.factions[0] && state.roster.factions[0].id);
                var wrongRecipient = state.roster.factions.find(function (faction) { return faction.id !== recipient; });
                if (wrongRecipient) {
                    var wrongTarget = el("button", { type: "button" }, "Simulate wrong recipient · " + wrongRecipient.name);
                    wrongTarget.addEventListener("click", function () { simulateObjective(1, { recipient: wrongRecipient.id, resources: ["gold", "troops"] }); });
                    host.appendChild(wrongTarget);
                }
                [["gold"], ["troops"], ["gold", "troops"]].forEach(function (resources) {
                    var transfer = el("button", { type: "button" }, "Simulate " + resources.join(" + ") + " to " + factionName(recipient));
                    transfer.addEventListener("click", function () { simulateObjective(1, { recipient: recipient, resources: resources }); });
                    host.appendChild(transfer);
                });
            }
        }
        (state.definition.reactions || []).forEach(function (reaction) {
            var isContact = reaction.when.type === "contact";
            var button = el("button", { type: "button" }, "Simulate first " + (isContact ? "contact" : "support") + " · " + factionName(reaction.when.target));
            button.disabled = isContact
                ? (state.facts.contact_faction_ids || []).includes(reaction.when.target)
                : Boolean((state.facts.support_deliveries_by_faction_id || {})[reaction.when.target]);
            button.addEventListener("click", function () {
                if (isContact) state.facts.contact_faction_ids = Array.from(new Set((state.facts.contact_faction_ids || []).concat(reaction.when.target)));
                else {
                    state.facts.support_deliveries_by_faction_id = state.facts.support_deliveries_by_faction_id || {};
                    state.facts.support_deliveries_by_faction_id[reaction.when.target] = { deliveries: 1, gold: 100, troops: 100, first_tick: ++state.facts.elapsed_ticks };
                    state.facts.ally_support_deliveries = Number(state.facts.ally_support_deliveries || 0) + 1;
                }
                paintPreview();
            });
            host.appendChild(button);
        });
    }
    function simulateObjective(amount, sample) {
        if (!state.machine) return;
        var step = state.machine.view().step, trigger = step.trigger;
        if (!trigger) return;
        sample = sample || {};
        if (amount == null) { var progress = state.machine.view().progress; amount = Math.max(1, progress.target - progress.current); }
        amount = Number(amount || 1);
        if (trigger.type === "contact") {
            var contacts = state.facts.contact_faction_ids || [];
            var candidates = Array.isArray(trigger.targets) ? trigger.targets : [trigger.target];
            var contacted = sample.factionId || candidates.find(function (factionId) { return factionId && !contacts.includes(factionId); }) || candidates[0];
            if (contacted) state.facts.contact_faction_ids = Array.from(new Set(contacts.concat(contacted)));
        }
        else if (trigger.type === "defeated") {
            var targets = Array.isArray(trigger.targets) ? trigger.targets : [trigger.target];
            var defeated = state.facts.defeated_faction_ids || [];
            state.facts.defeated_faction_ids = Array.from(new Set(defeated.concat(targets.filter(function (factionId) { return factionId && !defeated.includes(factionId); }).slice(0, amount))));
        }
        else if (trigger.type === "eliminated") {
            var eliminatedTarget = trigger.target || "player";
            state.facts.eliminated_faction_ids = Array.from(new Set((state.facts.eliminated_faction_ids || []).concat(eliminatedTarget)));
        }
        else if (trigger.type === "ui") state.ui[trigger.action] = Number(state.ui[trigger.action] || 0) + amount;
        else if (trigger.type === "alliance") {
            state.facts.alliance_faction_ids = Array.from(new Set((state.facts.alliance_faction_ids || []).concat(sample.factionId || trigger.target || "simulated_ally")));
            state.facts.alliances_formed = state.facts.alliance_faction_ids.length;
        }
        else if (trigger.type === "fleet") {
            var unit = sample.unit || trigger.unit || "TransportShip";
            state.facts.fleets_by_type[unit] = Number(state.facts.fleets_by_type[unit] || 0) + amount;
            var target = sample.target || trigger.target;
            if (unit === "TransportShip" && target) state.facts.transport_fleets_by_faction_id[target] = Number(state.facts.transport_fleets_by_faction_id[target] || 0) + amount;
            state.facts.fleets += amount;
        }
        else if (trigger.type === "resource_transfer") {
            var receiver = sample.recipient || trigger.recipient || "simulated_ally", received = sample.resources || trigger.resources || ["gold", "troops"];
            var sent = state.facts.resource_transfers_by_recipient_faction_id[receiver] || (state.facts.resource_transfers_by_recipient_faction_id[receiver] = { total: 0, gold: 0, troops: 0, gold_troops: 0 });
            sent.total += amount;
            if (received.includes("gold")) sent.gold += amount;
            if (received.includes("troops")) sent.troops += amount;
            if (received.includes("gold") && received.includes("troops")) sent.gold_troops += amount;
            state.facts.resource_transfers += amount;
        }
        else if (trigger.type === "camera_target") {
            state.facts.camera_target_distance = Number(trigger.distance || 12);
            state.facts.camera_target = 1;
        }
        else if (trigger.type === "zoom_out_complete") {
            state.facts.camera_zoom = window.SOWCampaign.zoomOutTarget(state.facts, trigger.value);
            state.facts.zoom_out_events = Number(state.facts.zoom_out_events || 0) + 1;
        }
        else if (trigger.type === "zoom_in_complete") {
            state.facts.camera_zoom = Number(state.facts.camera_zoom_target || 0.75);
            state.facts.zoom_in_complete = 1;
            state.facts.zoom_in_events = Number(state.facts.zoom_in_events || 0) + 1;
            if (trigger.target) state.facts.camera_target_distance = Number(trigger.distance || 12);
        }
        else if (trigger.type === "structure_level") {
            var kind = String(trigger.kind || "City").toLowerCase();
            state.facts.structure_levels[kind] = Math.max(Number(state.facts.structure_levels[kind] || 0), Number(trigger.value || 1));
        }
        else {
            var metric = window.SOWCampaign.METRICS[trigger.type];
            if (trigger.type === "attack" && trigger.target) { state.facts.attacks_by_faction_id = state.facts.attacks_by_faction_id || {}; state.facts.attacks_by_faction_id[trigger.target] = Number(state.facts.attacks_by_faction_id[trigger.target] || 0) + amount; }
            else state.facts[metric] = Number(state.facts[metric] || 0) + amount;
            if (trigger.type === "territory") state.facts.tiles = Number(state.facts.tiles || 0) + amount;
        }
        paintPreview();
    }
    function freshFacts() { return { tiles: 0, tiles_gained: 0, kills: 0, troops: Number(state.definition.settings && state.definition.settings.starting_troops) || 0, buildings: 0, cities: 0, ally_support_deliveries: 0, support_deliveries_by_faction_id: {}, structure_levels: {}, fleets: 0, fleets_by_type: {}, transport_fleets_by_faction_id: {}, nukes: 0, attacks: 0, attacks_by_faction_id: {}, camera_drag_events: 0, camera_key_pan_events: 0, hover_events: 0, zoom_out_events: 0, zoom_in_events: 0, zoom_out_complete: 0, zoom_in_complete: 0, camera_target: 0, camera_target_distance: 30, camera_zoom: 1, camera_zoom_start: 1, camera_zoom_floor: 0.25, camera_zoom_target: 0.8, touch_controls: 0, contact_faction_ids: [], defeated_faction_ids: [], eliminated_faction_ids: [], resource_transfers: 0, resource_transfers_by_recipient_faction_id: {}, alliance_faction_ids: [], elapsed_ticks: 0, elapsed_seconds: 0 }; }
    function setPreviewMenuScreen(screen) {
        var menu = $("#sow-menu");
        menu.dataset.previewScreen = screen;
        menu.querySelectorAll("[data-preview-menu-screen]").forEach(function (panel) { panel.hidden = panel.dataset.previewMenuScreen !== screen; });
    }
    function resetPreview() {
        state.facts = freshFacts(); state.ui = {}; state.previewFocusActive = false;
        var mapLayer = $("#campaignPreviewLayer");
        ["--map-focus-x", "--map-focus-y", "--map-focus-scale"].forEach(function (property) { mapLayer.style.removeProperty(property); });
        mapLayer.classList.remove("has-map-guide");
        $("#campaignPreviewMarkers .is-camera-focus")?.classList.remove("is-camera-focus");
        setPreviewMenuScreen("home"); renderPreview(flowEntry() || state.definition.entry);
    }
    function refresh(skipGraph) {
        if (!state.definition) return;
        normalizeStrings(state.definition); ensureLayout();
        var report = valid();
        state.validation = report;
        var blocked = report.errors.length > 0;
        $("#previewRoot").inert = blocked;
        ["playSelected", "restartBtn", "simulateBtn", "tickBtn", "previewBranchSelect"].forEach(function (id) { $("#" + id).disabled = blocked; });
        updateStepValidation();
        $("#validationStatus").textContent = report.errors.length ? "Needs fixes · " + report.errors.length : report.warnings.length ? "Ready · " + report.warnings.length + " notes" : "Ready to play";
        var errors = $("#errors"); errors.replaceChildren();
        report.errors.forEach(function (issue) { var button = el("button", { type: "button" }, [issue.step, issue.field, issue.message].filter(Boolean).join(" · ")); button.addEventListener("click", function () { if (issue.step) selectStep(issue.step); }); errors.appendChild(button); });
        report.warnings.forEach(function (issue) {
            var button = el("button", { type: "button", class: "warning" }, [issue.step, issue.message].filter(Boolean).join(" · "));
            if (issue.step) button.addEventListener("click", function () { selectStep(issue.step); });
            errors.appendChild(button);
        });
        $("#exportBtn").disabled = state.saving || report.errors.length > 0 || Boolean(state.demoBackup);
        if (!skipGraph) renderGraph();
    }
    function updateStepValidation() {
        var box = $("#inspector .step-validation");
        if (!box) return;
        var errors = (state.validation.errors || []).filter(function (issue) { return issue.step === state.selected; });
        var warnings = (state.validation.warnings || []).filter(function (issue) { return issue.step === state.selected; });
        var issues = errors.concat(warnings);
        box.replaceChildren();
        box.hidden = !issues.length;
        box.classList.toggle("is-warning", !errors.length && warnings.length > 0);
        if (issues.length) {
            box.appendChild(el("strong", {}, errors.length ? "Fix this step before saving" : "Review this step"));
            issues.forEach(function (issue) { box.appendChild(el("p", {}, [warnings.includes(issue) ? "Note" : "", issue.field, issue.message].filter(Boolean).join(" · "))); });
        }
    }

    async function load() {
        status("Loading…"); notice("");
        var rosterResponse = await fetch("/assets/campaign/" + episodeId + ".json", { cache: "no-store" });
        var logicResponse = await fetch("/assets/campaign/" + episodeId + ".triggers.json", { cache: "no-store" });
        if (!rosterResponse.ok || !logicResponse.ok) throw new Error("Could not load both map and story files for " + episodeId + ".");
        var files = await Promise.all([rosterResponse.json(), logicResponse.json()]);
        state.roster = files[0]; state.definition = files[1]; state.rosterEtag = rosterResponse.headers.get("ETag"); state.etag = logicResponse.headers.get("ETag"); state.externalChangeTag = null; state.dirty = false; state.flow = "episode"; state.selected = state.definition.entry;
        renderRosterMapPreview();
        normalizeStrings(state.definition); ensureLayout();
        var title = $("#episodeSelect"); title.value = episodeId;
        var previewEpisode = title.selectedOptions[0] && title.selectedOptions[0].textContent || episodeId;
        previewEpisode = previewEpisode.split("·")[0].trim();
        $("[data-preview-episode-title]").textContent = previewEpisode;
        var replayButton = $("#sow-menu [data-command='start_campaign_episode']");
        replayButton.dataset.episodeId = episodeId;
        replayButton.textContent = episodeId === "boudica" ? "REPLAY TUTORIAL" : "REPLAY EPISODE";
        $("#flowSelect").value = state.flow;
        $("#mapLink").href = "index.html?episode=" + encodeURIComponent(episodeId);
        $("#logicLink").href = "logic.html?episode=" + encodeURIComponent(episodeId);
        renderPalette(); renderSettings(); renderInspector(); refresh(); resetPreview(); status("Saved");
    }
    async function save() {
        if (state.demoBackup) return false;
        var report = valid();
        if (report.errors.length) { status("Fix story errors before saving", "error"); notice(report.errors[0].message); return false; }
        if (state.saving) return false;
        var savedDraft = JSON.stringify(state.definition, null, 2) + "\n";
        state.saving = true; $("#exportBtn").disabled = true; status("Saving…");
        try {
            var response = await fetch("/__save?file=" + encodeURIComponent(episodeId + ".triggers.json"), {
                method: "POST", headers: { "Content-Type": "application/json", "If-Match": state.etag || "" },
                body: savedDraft
            });
            if (response.status === 409) { status("Reload needed", "error"); notice(await response.text()); return false; }
            if (!response.ok) throw new Error(await response.text());
            state.etag = response.headers.get("ETag");
            state.dirty = JSON.stringify(state.definition, null, 2) + "\n" !== savedDraft;
            status(state.dirty ? "Unsaved changes" : "Saved", state.dirty ? "dirty" : ""); notice("");
            return !state.dirty;
        } catch (error) { status("Save failed", "error"); notice(error.message); return false; }
        finally { state.saving = false; refresh(); }
    }
    async function watchExternalFiles() {
        if (state.demoBackup || document.visibilityState !== "visible" || !state.definition) return;
        try {
            var responses = await Promise.all([
                fetch("/assets/campaign/" + episodeId + ".json", { method: "HEAD", cache: "no-store" }),
                fetch("/assets/campaign/" + episodeId + ".triggers.json", { method: "HEAD", cache: "no-store" })
            ]);
            if (responses.some(function (response) { return !response.ok; })) return;
            var externalTag = responses.map(function (response) { return response.headers.get("ETag"); }).join("\n");
            if (responses[0].headers.get("ETag") === state.rosterEtag && responses[1].headers.get("ETag") === state.etag) { state.externalChangeTag = null; return; }
            if (state.dirty) {
                if (externalTag === state.externalChangeTag) return;
                state.externalChangeTag = externalTag;
                status("Files changed elsewhere", "error");
                notice("Your draft is preserved. Reload the episode to see external changes before saving.");
                return;
            }
            state.externalChangeTag = null;
            await load();
            status("Updated from disk");
        } catch (_) { /* A short editor-server restart must not discard the open draft. */ }
    }
    function download() {
        if (state.demoBackup) return;
        var blob = new Blob([JSON.stringify(state.definition, null, 2) + "\n"], { type: "application/json" });
        var anchor = el("a", { href: URL.createObjectURL(blob), download: episodeId + ".triggers.json" }); anchor.click(); URL.revokeObjectURL(anchor.href);
    }
    function setDemoMode(active) {
        $("#graphPanel").inert = active;
        $("#inspectorPanel").inert = active;
        $("#exportBtn").disabled = active || state.saving || state.validation.errors.length > 0;
        $("#reloadBtn").disabled = active;
        $("#episodeSelect").disabled = active;
        $("#demoBtn").textContent = active ? "Return to episode" : "Preview branching sample";
        $("#demoBtn").setAttribute("aria-pressed", String(active));
        $(".simulation-badge").textContent = active ? "SAMPLE · PREVIEW ONLY" : "SIMULATION";
    }
    function demo() {
        if (state.demoBackup) {
            var previous = state.demoBackup;
            state.demoBackup = null;
            state.definition = previous.definition;
            state.selected = previous.selected;
            state.previewBranch = previous.previewBranch;
            state.flow = previous.flow;
            state.dirty = previous.dirty;
            $("#flowSelect").value = state.flow;
            renderSettings(); renderInspector(); refresh(); resetPreview();
            status(state.dirty ? "Unsaved changes" : "Saved", state.dirty ? "dirty" : "");
            setDemoMode(false);
            return;
        }
        state.demoBackup = {
            definition: JSON.parse(JSON.stringify(state.definition)),
            selected: state.selected, previewBranch: state.previewBranch,
            flow: state.flow, dirty: state.dirty
        };
        var id = episodeId + "_studio_demo";
        var oldSpeaker = Object.entries(state.definition.speakers || {})[0];
        var leaderId = oldSpeaker && oldSpeaker[0] || "commander";
        var speakers = {};
        speakers[leaderId] = oldSpeaker ? Object.assign({}, oldSpeaker[1]) : { name: "Commander" };
        var advisorId = "advisor";
        while (advisorId === leaderId) advisorId += "_2";
        speakers[advisorId] = { name: "Advisor" };
        var key = function (name) { return "tutorial.campaign_studio_demo_" + name; };
        state.definition = {
            version: 2, episode_id: episodeId, settings: state.definition.settings,
            entry: id + "_opening", speakers: speakers, layout: {},
            steps: [
                { id: id + "_opening", type: "scene", title_key: key("opening_title"), lines: [{ speaker: leaderId, body_key: key("opening_body") }, { speaker: advisorId, body_key: key("advisor_line") }], presentation: "chapter", next: id + "_decision" },
                { id: id + "_decision", type: "choice", title_key: key("decision_title"), body_key: key("decision_body"), choices: [
                    { id: "gather", label_key: key("gather"), next: id + "_grow" }, { id: "strike", label_key: key("strike"), next: id + "_attack" }
                ] },
                { id: id + "_grow", type: "objective", title_key: key("grow_title"), hint_key: key("grow_hint"), trigger: { type: "territory", value: 300, scope: "step" }, guide: { kind: "world", target: "expand", gesture: "tap" }, next: id + "_recall" },
                { id: id + "_attack", type: "objective", title_key: key("attack_title"), hint_key: key("attack_hint"), trigger: { type: "kills", value: 1, scope: "total" }, guide: { kind: "world", target: "assault", gesture: "tap" }, next: id + "_recall" },
                { id: id + "_recall", type: "scene", title_key: key("recall_title"), body_key: key("recall_body"), next: id + "_turn", routes: [
                    { when: { choice: id + "_decision", equals: "gather" }, next: id + "_gather_echo" },
                    { when: { choice: id + "_decision", equals: "strike" }, next: id + "_strike_echo" }
                ] },
                { id: id + "_gather_echo", type: "scene", title_key: key("gather_echo_title"), body_key: key("gather_echo_body"), next: id + "_turn" },
                { id: id + "_strike_echo", type: "scene", title_key: key("strike_echo_title"), body_key: key("strike_echo_body"), next: id + "_turn" },
                { id: id + "_turn", type: "choice", title_key: key("turn_title"), body_key: key("turn_body"), choices: [
                    { id: "press", label_key: key("press"), next: id + "_advance" }, { id: "secure", label_key: key("secure"), next: id + "_hold" }
                ] },
                { id: id + "_advance", type: "objective", title_key: key("advance_title"), hint_key: key("advance_hint"), trigger: { type: "territory", value: 100, scope: "step" }, guide: { kind: "world", target: "expand", gesture: "tap" }, next: id + "_ending" },
                { id: id + "_hold", type: "objective", title_key: key("hold_title"), hint_key: key("hold_hint"), trigger: { type: "ui", action: "attack_ratio", scope: "step" }, guide: { kind: "ui", target: "attack_ratio", gesture: "drag" }, next: id + "_ending" },
                { id: id + "_ending", type: "end", title_key: key("ending_title"), body_key: key("ending_body"), presentation: "chapter" }
            ]
        };
        state.definition.layout = {}; state.definition.steps.forEach(function (step, index) { state.definition.layout[step.id] = { x: 70 + (index % 2) * 320, y: 70 + Math.floor(index / 2) * 220 }; });
        state.flow = "episode"; $("#flowSelect").value = "episode";
        state.selected = state.definition.entry; renderSettings(); renderInspector(); refresh(); resetPreview();
        status("Previewing sample · episode draft preserved", "preview");
        setDemoMode(true);
    }
    function createMenuGuide() {
        if (state.definition.menu_guide) {
            state.flow = "menu"; $("#flowSelect").value = "menu"; state.selected = state.definition.menu_guide.entry;
            renderSettings(); renderInspector(); resetPreview(); return;
        }
        var opening = uniqueId(episodeId + "_menu_guide_opening", state.definition.steps);
        var campaign = uniqueId(episodeId + "_menu_guide_campaign", state.definition.steps);
        var replay = uniqueId(episodeId + "_menu_guide_replay", state.definition.steps);
        var ending = uniqueId(episodeId + "_menu_guide_end", state.definition.steps);
        var keys = {
            opening: "tutorial.campaign_menu_guide_opening",
            campaign: "tutorial.campaign_menu_guide_campaign",
            replay: "tutorial.campaign_menu_guide_replay",
            ending: "tutorial.campaign_menu_guide_ending"
        };
        state.definition.steps.push(
            { id: opening, type: "scene", title_key: keys.opening + "_title", body_key: keys.opening + "_body", presentation: "chapter", next: campaign },
            { id: campaign, type: "guide", title_key: keys.campaign + "_title", hint_key: keys.campaign + "_hint", trigger: { type: "ui", action: "menu_campaign", scope: "step" }, guide: { kind: "ui", target: "menu_campaign", gesture: "tap" }, next: replay },
            { id: replay, type: "guide", title_key: keys.replay + "_title", hint_key: keys.replay + "_hint", trigger: { type: "ui", action: "campaign_replay", scope: "step" }, guide: { kind: "ui", target: "campaign_replay", gesture: "tap" }, next: ending },
            { id: ending, type: "end", title_key: keys.ending + "_title", body_key: keys.ending + "_body", presentation: "chapter" }
        );
        state.definition.menu_guide = { entry: opening, dismissible: true };
        state.definition.layout[opening] = { x: 70, y: 80 }; state.definition.layout[campaign] = { x: 350, y: 80 };
        state.definition.layout[replay] = { x: 630, y: 80 }; state.definition.layout[ending] = { x: 910, y: 80 };
        state.flow = "menu"; $("#flowSelect").value = "menu"; state.selected = opening;
        markDirty(); renderSettings(); renderInspector(); refresh(); resetPreview();
    }
    function connectControls() {
        $("#exportBtn").addEventListener("click", save);
        $("#mapLink").addEventListener("click", async function (event) {
            if (!state.dirty && !state.demoBackup) return;
            event.preventDefault();
            var href = event.currentTarget.href;
            if (state.demoBackup) demo();
            if (!state.dirty) { location.href = href; return; }
            if (await save() && !state.dirty) location.href = href;
        });
        $("#reloadBtn").addEventListener("click", function () { if (!state.dirty || confirm("Discard unsaved changes and reload this episode?")) load().catch(function (error) { notice(error.message); status("Load failed", "error"); }); });
        $("#downloadBtn").addEventListener("click", download);
        $("#demoBtn").addEventListener("click", demo);
        $("#menuGuideBtn").addEventListener("click", createMenuGuide);
        $("#flowSelect").addEventListener("change", function () {
            state.flow = this.value;
            $("#previewFrame").dataset.flow = state.flow;
            $("#sow-menu").hidden = state.flow !== "menu";
            renderSettings(); renderInspector(); resetPreview();
        });
        $("#flowEntryBtn").addEventListener("click", function () {
            if (!state.selected) return;
            if (state.flow === "menu") state.definition.menu_guide = { entry: state.selected, dismissible: true };
            else state.definition.entry = state.selected;
            markDirty(); renderSettings(); resetPreview();
        });
        $("#playSelected").addEventListener("click", function () { state.facts = freshFacts(); state.ui = {}; renderPreview(state.selected); });
        $("#previewBranchSelect").addEventListener("change", function () {
            state.previewBranch = this.value ? { stepId: state.selected, routeIndex: Number(this.value) } : null;
            state.facts = freshFacts(); state.ui = {};
            renderPreview(state.selected);
        });
        $("#restartBtn").addEventListener("click", resetPreview);
        $("#simulateBtn").addEventListener("click", function () { simulateObjective(null); });
        $("#tickBtn").addEventListener("click", function () { state.facts.elapsed_seconds = Number(state.facts.elapsed_seconds || 0) + 1; paintPreview(); });
        $("#device").addEventListener("change", function () { renderRosterMapPreview(); paintPreview(); });
        $("#reducedMotion").addEventListener("change", paintPreview);
        $("#factionLabels").addEventListener("change", renderRosterMapPreview);
        $("#pickFactionTarget").addEventListener("click", function () { setFactionPicker(!state.pickingFaction); });
        $("#campaignPreviewMarkers").addEventListener("click", function (event) {
            var marker = state.pickingFaction && event.target.closest(".sample-faction.is-faction-pickable");
            if (!marker) return;
            event.preventDefault(); event.stopPropagation(); chooseFactionTarget(marker.dataset.factionId);
            $("#pickFactionTarget").focus({ preventScroll: true });
        });
        $("#campaignPreviewMarkers").addEventListener("keydown", function (event) {
            if (event.key !== "Enter" && event.key !== " ") return;
            var marker = state.pickingFaction && event.target.closest(".sample-faction.is-faction-pickable");
            if (!marker) return;
            event.preventDefault(); event.stopPropagation(); chooseFactionTarget(marker.dataset.factionId);
            $("#pickFactionTarget").focus({ preventScroll: true });
        });
        $("#previewLocale").addEventListener("change", function () {
            var locale = this.value; state.previewLanguage = locale; paintPreview();
            loadCatalog(locale).then(function () { if (state.previewLanguage === locale) paintPreview(); });
        });
        $("#previewLocale").addEventListener("change", function () {
            var locale = this.value; state.previewLanguage = locale;
            loadCatalog(locale).then(function () {
                if (state.previewLanguage !== locale) return;
                renderSettings(); renderInspector(); renderGraph(); paintPreview();
            });
        });
        document.addEventListener("visibilitychange", watchExternalFiles);
        window.setInterval(watchExternalFiles, 1500);
        function previewUiAction(event) {
            if (!state.machine) return;
            var trigger = state.machine.view().step.trigger;
            if (!trigger || trigger.type !== "ui") return;
            var control = window.SOWCampaign.resolveUiTarget(trigger.action, document, episodeId);
            var expectedEvent = control && control.matches("input") ? "change" : "click";
            if (event.type !== expectedEvent) return;
            if (control && (control === event.target || control.contains(event.target))) simulateObjective(1);
        }
        $("#previewFrame").addEventListener("click", function (event) {
            var menu = $("#sow-menu");
            if (menu && menu.contains(event.target)) {
                if (event.target.closest('[data-command="open_campaign"]')) setPreviewMenuScreen("campaign");
                else if (event.target.closest("[data-preview-menu-back]")) setPreviewMenuScreen("home");
                else return;
                paintPreview();
                return;
            }
            var back = event.target.closest("[data-map-back]");
            var group = event.target.closest("[data-map-group]");
            var action = event.target.closest("[data-map-action]");
            var hudRoot = $("#sow-hud");
            if (!hudRoot || !hudRoot.contains(event.target)) return;
            if (back) hudRoot.dataset.previewMapMenu = "";
            else if (group) hudRoot.dataset.previewMapMenu = group.dataset.mapGroup;
            else if (action) hudRoot.dataset.previewMapMenu = "";
            else return;
            var transferPanel = $("#sow-hud-transfer");
            if (transferPanel && action && action.dataset.mapAction === "transfer") transferPanel.hidden = false;
            var popup = hudRoot.querySelector(".sample-map-popup");
            if (popup) popup.hidden = !hudRoot.dataset.previewMapMenu;
            hudRoot.querySelectorAll("[data-preview-map-menu]").forEach(function (panel) {
                panel.hidden = panel.dataset.previewMapMenu !== hudRoot.dataset.previewMapMenu;
            });
            paintPreview();
        });
        ["click", "change"].forEach(function (type) { $("#previewFrame").addEventListener(type, previewUiAction); });
        $("#episodeSelect").addEventListener("change", function () { if (state.dirty && !confirm("Discard unsaved changes and switch episode?")) { this.value = episodeId; return; } location.href = "logic.html?episode=" + encodeURIComponent(this.value); });
        $("#fileIn").addEventListener("change", function () { var file = this.files[0]; if (!file) return; if (state.dirty && !confirm("Replace the unsaved draft with this file?")) { this.value = ""; return; } var reader = new FileReader(); reader.onload = function () { try { var next = JSON.parse(reader.result); if (next.episode_id !== episodeId) throw new Error("Episode ID does not match this editor tab."); state.definition = next; normalizeStrings(next); ensureLayout(); state.dirty = true; state.flow = "episode"; $("#flowSelect").value = "episode"; state.selected = next.entry; status("Imported draft", "dirty"); renderSettings(); renderInspector(); refresh(); resetPreview(); } catch (error) { notice(error.message); } }; reader.readAsText(file); });
        $("#stage").addEventListener("pointermove", function (event) {
            if (!state.drag) return;
            var scale = state.zoom;
            if (state.drag.kind === "node") {
                var pos = state.definition.layout[state.drag.id];
                pos.x = Math.max(0, state.drag.origin.x + (event.clientX - state.drag.x) / scale);
                pos.y = Math.max(0, state.drag.origin.y + (event.clientY - state.drag.y) / scale);
                var card = $("#nodes [data-step-id=" + CSS.escape(state.drag.id) + "]");
                if (card) { card.style.left = pos.x + "px"; card.style.top = pos.y + "px"; }
                drawWires();
            }
            else { state.pan.x += event.movementX; state.pan.y += event.movementY; renderGraph(); }
        });
        $("#stage").addEventListener("pointerup", function () { if (state.drag && state.drag.kind === "node") markDirty(); state.drag = null; });
        $("#stage").addEventListener("pointerdown", function (event) {
            if (state.keyboardWire) {
                state.keyboardWire = null; $("#stage").dataset.connecting = "false";
                $(".graph-hint").textContent = "Drag outputs to inputs · keyboard: Enter on output, then Enter on destination · Escape cancels";
            }
            if (event.target === $("#stage") || event.target.id === "wires") { state.drag = { kind: "pan" }; $("#stage").setPointerCapture(event.pointerId); }
        });
        $("#stage").addEventListener("keydown", function (event) {
            if (event.key !== "Escape" || !state.keyboardWire) return;
            event.preventDefault(); state.keyboardWire = null; $("#stage").dataset.connecting = "false";
            $(".graph-hint").textContent = "Drag outputs to inputs · keyboard: Enter on output, then Enter on destination · Escape cancels";
        });
        document.addEventListener("pointermove", function (event) {
            if (!state.wire) return;
            var source = $("#nodes [data-step-id=" + CSS.escape(state.wire.source) + "]");
            var output = source && source.querySelectorAll(".pin.out")[state.wire.outputIndex];
            var group = $("#wires g");
            if (!output || !group) return;
            if (!state.wire.path) { state.wire.path = document.createElementNS("http://www.w3.org/2000/svg", "path"); state.wire.path.classList.add("wire-pending"); group.appendChild(state.wire.path); }
            var stageRect = $("#stage").getBoundingClientRect(), rect = output.getBoundingClientRect();
            var x1 = (rect.left + rect.width / 2 - stageRect.left - state.pan.x) / state.zoom;
            var y1 = (rect.top + rect.height / 2 - stageRect.top - state.pan.y) / state.zoom;
            var x2 = (event.clientX - stageRect.left - state.pan.x) / state.zoom;
            var y2 = (event.clientY - stageRect.top - state.pan.y) / state.zoom;
            state.wire.path.setAttribute("d", "M" + x1 + " " + y1 + " C" + (x1 + 70) + " " + y1 + " " + (x2 - 70) + " " + y2 + " " + x2 + " " + y2);
        });
        document.addEventListener("pointerup", function (event) {
            if (!state.wire) return;
            var target = document.elementFromPoint(event.clientX, event.clientY);
            var port = target && target.closest("[data-in]");
            var wire = state.wire;
            state.wire = null;
            if (wire.path && wire.path.isConnected) wire.path.remove();
            if (port) { setDestination(wire, port.dataset.in); markDirty(); }
        });
        $("#stage").addEventListener("wheel", function (event) { event.preventDefault(); zoomAt(event.clientX, event.clientY, event.deltaY < 0 ? 1.1 : 1 / 1.1); }, { passive: false });
        document.querySelectorAll("[data-zoom]").forEach(function (button) { button.addEventListener("click", function () { if (button.dataset.zoom === "fit") fitGraph(); else zoomAt(innerWidth / 2, innerHeight / 2, button.dataset.zoom === "in" ? 1.2 : 1 / 1.2); }); });
        $("#arrangeGraphBtn").addEventListener("click", arrangeGraph);
        window.addEventListener("resize", function () { drawWires(); if (state.machine) paintPreview(false); }, { passive: true });
        window.addEventListener("beforeunload", function (event) { if (state.dirty) { event.preventDefault(); event.returnValue = ""; } });
    }
    function zoomAt(x, y, factor) {
        var rect = $("#stage").getBoundingClientRect(), sx = x - rect.left, sy = y - rect.top;
        var next = Math.max(.45, Math.min(1.8, state.zoom * factor));
        state.pan.x = sx - (sx - state.pan.x) * next / state.zoom;
        state.pan.y = sy - (sy - state.pan.y) * next / state.zoom;
        state.zoom = next; $("#zlabel").textContent = Math.round(next * 100) + "%"; renderGraph();
    }
    function fitGraph() {
        var stage = $("#stage"), nodes = Array.from($("#nodes").querySelectorAll(".node"));
        if (!nodes.length || !stage.clientWidth || !stage.clientHeight) return;
        var bounds = nodes.reduce(function (box, node) {
            var x = node.offsetLeft, y = node.offsetTop;
            box.left = Math.min(box.left, x); box.top = Math.min(box.top, y);
            box.right = Math.max(box.right, x + node.offsetWidth); box.bottom = Math.max(box.bottom, y + node.offsetHeight);
            return box;
        }, { left: Infinity, top: Infinity, right: -Infinity, bottom: -Infinity });
        var width = bounds.right - bounds.left, height = bounds.bottom - bounds.top;
        var zoom = Math.min(1.4, (stage.clientWidth - 64) / width, (stage.clientHeight - 64) / height);
        state.zoom = zoom;
        state.pan = { x: (stage.clientWidth - width * zoom) / 2 - bounds.left * zoom, y: (stage.clientHeight - height * zoom) / 2 - bounds.top * zoom };
        $("#zlabel").textContent = Math.round(zoom * 100) + "%";
        renderGraph();
    }
    function arrangeGraph() {
        var steps = state.definition.steps, byId = Object.create(null), depth = Object.create(null), queue = [];
        steps.forEach(function (step) { byId[step.id] = step; });
        [state.definition.entry, state.definition.menu_guide && state.definition.menu_guide.entry].forEach(function (entry) {
            if (byId[entry] && depth[entry] == null) { depth[entry] = 0; queue.push(entry); }
        });
        for (var index = 0; index < queue.length; index++) {
            var source = byId[queue[index]], nextDepth = depth[source.id] + 1;
            portDefinitions(source).forEach(function (port) {
                if (byId[port.target] && depth[port.target] == null) { depth[port.target] = nextDepth; queue.push(port.target); }
            });
        }
        var lastDepth = queue.reduce(function (max, id) { return Math.max(max, depth[id]); }, 0);
        steps.forEach(function (step) { if (depth[step.id] == null) depth[step.id] = lastDepth + 1; });
        var columns = [];
        steps.forEach(function (step) { (columns[depth[step.id]] || (columns[depth[step.id]] = [])).push(step); });
        state.definition.layout = {};
        columns.forEach(function (column, x) {
            var y = 48;
            column.forEach(function (step) {
                var node = $("#nodes [data-step-id=" + CSS.escape(step.id) + "]");
                state.definition.layout[step.id] = { x: 48 + x * 320, y: y };
                y += (node ? node.offsetHeight : 160) + 48;
            });
        });
        markDirty(); fitGraph();
    }
    function fillLocales() {
        return fetch("/locales/index.json", { cache: "no-store" }).then(function (response) { if (!response.ok) throw new Error("locale registry unavailable"); return response.json(); }).then(function (registry) {
            var version = Number(registry && registry.version);
            if (!registry || registry.schema !== 1 || !Number.isInteger(version) || version < 1 || !Array.isArray(registry.languages) || !registry.languages.length) throw new Error("invalid locale registry");
            localeNames = Object.create(null);
            locales = registry.languages.map(function (entry) {
                var code = String(typeof entry === "string" ? entry : entry.code).toLowerCase();
                if (typeof entry === "object" && typeof entry.name === "string" && entry.name.trim()) localeNames[code] = entry.name;
                return code;
            });
        }).catch(function () { locales = ["en", "es"]; state.localeRegistryFailed = true; localeNames = { en: "English", es: "Español" }; });
    }
    function loadCatalog(locale) {
        var code = String(locale || "").toLowerCase();
        if (state.catalogs[code]) return Promise.resolve(true);
        if (catalogLoads[code]) return catalogLoads[code];
        var request = fetch("/locales/" + encodeURIComponent(code), { cache: "no-store" }).then(function (response) {
            if (!response.ok) throw new Error("locale " + code + " unavailable");
            return response.json();
        }).then(function (payload) {
            if (!payload || !payload.strings || typeof payload.strings !== "object" || Array.isArray(payload.strings)) throw new Error("locale " + code + " has invalid strings");
            state.catalogs[code] = payload.strings;
            state.localeFailures = state.localeFailures.filter(function (failed) { return failed !== code; });
            return true;
        }).catch(function () {
            if (!state.localeFailures.includes(code)) state.localeFailures.push(code);
            return false;
        }).then(function (loaded) { delete catalogLoads[code]; return loaded; });
        catalogLoads[code] = request;
        return request;
    }
    function fillAvatars() {
        return fetch("/__avatars", { cache: "no-store" }).then(function (response) {
            if (!response.ok) throw new Error("avatar list unavailable");
            return response.json();
        }).then(function (avatars) {
            if (!Array.isArray(avatars) || avatars.some(function (avatar) { return typeof avatar !== "string" || !/^[a-z][a-z0-9_]*$/.test(avatar); })) throw new Error("invalid avatar list");
            state.avatars = avatars;
        });
    }
    function fillEpisodes() {
        var select = $("#episodeSelect"), labels = Object.create(null);
        Array.from(select.options).forEach(function (option) { labels[option.value] = option.textContent; });
        return fetch("/__episodes", { cache: "no-store" }).then(function (response) {
            if (!response.ok) throw new Error("episode list unavailable");
            return response.json();
        }).then(function (episodes) {
            if (!Array.isArray(episodes) || !episodes.length || episodes.some(function (id) { return typeof id !== "string" || !/^[a-z][a-z0-9_]{0,63}$/.test(id); })) throw new Error("invalid episode list");
            optionList(select, episodes.map(function (id) {
                return { value: id, label: labels[id] || id.replace(/_/g, " ").replace(/\b\w/g, function (letter) { return letter.toUpperCase(); }) };
            }), episodeId);
        }).then(function () { return true; }).catch(function () { return false; });
    }
    function init() {
        connectControls();
        Promise.all([fillLocales(), fillAvatars(), fillEpisodes(), load()]).then(function (results) {
            optionList($("#previewLocale"), localeOptions(), "en");
            state.previewLanguage = $("#previewLocale").value;
            return Promise.all([loadCatalog("en"), loadCatalog(state.previewLanguage)]).then(function () {
                if (!results[2]) notice("Could not load the episode list; only the built-in episodes are available.");
                else if (state.localeRegistryFailed) notice("Language list unavailable; showing English and Spanish previews.");
                else if (state.localeFailures.length) notice("Catalog unavailable for " + state.localeFailures.join(", ") + "; some shared-text previews may be incomplete.");
                refresh(); renderSettings(); renderInspector(); renderPreview();
            });
        }).catch(function (error) { notice(error.message); status("Load failed", "error"); });
    }
    init();
})();
