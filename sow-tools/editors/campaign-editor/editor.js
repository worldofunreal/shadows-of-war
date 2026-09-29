(function () {
    "use strict";

    var $ = function (selector) { return document.querySelector(selector); };
    var query = new URLSearchParams(location.search);
    var episodeId = /^[a-z][a-z0-9_]{0,63}$/.test(query.get("episode") || "") ? query.get("episode") : "boudica";
    var rtlLanguages = new Set(["ar", "arc", "ckb", "dv", "fa", "he", "iw", "nqo", "pnb", "ps", "sd", "syr", "ug", "ur", "yi"]);
    var state = { roster: null, definition: null, etag: null, rosterEtag: null, externalChangeTag: null, dirty: false, saving: false, demoBackup: null, selected: null, previewBranch: null, previewFocusActive: false, previewActionStep: null, previewActionRatio: null, pickingFaction: false, flow: "episode", language: "en", previewLanguage: "en", catalogs: {}, localeFailures: [], localeRegistryFailed: false, avatars: [], machine: null, renderer: null, facts: {}, ui: {}, mapPreview: null, mapResizeObserver: null, mapResizeFallback: false, mapPreviewLoadingId: null, zoom: 1, pan: { x: 36, y: 44 }, drag: null, wire: null, keyboardWire: null, validation: { errors: [], warnings: [] } };
    var types = ["scene", "choice", "objective", "guide", "end"];
    var triggerTypes = [
        { value: "territory", label: "Gain territory" }, { value: "kills", label: "Defeat troops" },
        { value: "attack", label: "Launch an attack" }, { value: "troops", label: "Reach a troop minimum" },
        { value: "building", label: "Complete a building" }, { value: "city", label: "Complete a city" },
        { value: "farm", label: "Complete a farm" }, { value: "factory", label: "Complete a factory" },
        { value: "port", label: "Complete a port" }, { value: "bunker", label: "Complete a bunker" },
        { value: "structure_upgrade", label: "Upgrade a structure" }, { value: "city_upgrade", label: "Upgrade a city" },
        { value: "city_level", label: "Reach a city level" },
        { value: "port_upgrade", label: "Upgrade a port" }, { value: "port_level", label: "Reach port level" }, { value: "tile_upgrade", label: "Upgrade territory" },
        { value: "resource_transfer", label: "Send resources" }, { value: "alliance", label: "Form an alliance" },
        { value: "support", label: "Receive allied support" }, { value: "fleet", label: "Launch a fleet" },
        { value: "nuke", label: "Launch a nuke" }, { value: "elapsed", label: "Wait for game time" },
        { value: "contact", label: "Reach a faction" }, { value: "defeated", label: "Defeat a faction" },
        { value: "ui", label: "Use a control" }
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
        if (Array.isArray(definition.strings)) {
            var converted = {};
            definition.strings.forEach(function (locale) { converted[locale] = {}; });
            definition.strings = converted;
        }
        definition.strings = definition.strings || {};
        definition.default_locale = definition.default_locale || "en";
        definition.strings[definition.default_locale] = definition.strings[definition.default_locale] || {};
        Object.keys(state.catalogs).forEach(function (locale) { definition.strings[locale] = definition.strings[locale] || {}; });
    }
    function catalogValue(locale, key) {
        return String(key || "").split(".").reduce(function (value, part) { return value && value[part]; }, state.catalogs[locale] || {});
    }
    function textValue(key, locale) {
        var dictionaries = state.definition.strings || {};
        return dictionaries[locale] && dictionaries[locale][key] || catalogValue(locale, key) || dictionaries[state.definition.default_locale] && dictionaries[state.definition.default_locale][key] || catalogValue(state.definition.default_locale, key) || "";
    }
    function translated(key, locale) {
        return textValue(key, locale || state.previewLanguage) || (key ? "[" + key + "]" : "");
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
    function chooseFactionTarget(name) {
        var step = state.definition && state.definition.steps.find(function (item) { return item.id === state.selected; });
        if (!step || !["objective", "guide"].includes(step.type)) return;
        if (step.trigger && ["contact", "defeated"].includes(step.trigger.type) && Array.isArray(step.trigger.targets)) {
            if (!step.trigger.targets.includes(name)) step.trigger.targets.push(name);
            if (step.trigger.type === "defeated") step.trigger.value = step.trigger.targets.length;
        } else if (["contact", "defeated", "attack"].includes(step.trigger && step.trigger.type)) step.trigger.target = name;
        else step.marker = { target: name };
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
                var marker = el("span", { class: "sample-map-marker sample-faction", title: faction.name, "aria-label": faction.name, dataset: { factionName: faction.name, role: faction.role } });
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
            hasText: function (key) { return Boolean(textValue(key, state.definition.default_locale)); },
            hasAvatar: function (avatar) { return state.avatars.includes(avatar); }
        });
    }
    function markDirty() {
        if (state.demoBackup) return;
        state.dirty = true;
        var status = $("#saveStatus");
        status.textContent = "Unsaved changes";
        status.className = "status dirty";
        if (state.renderer) { refresh(true); syncPreview(); }
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
            key = "tutorial." + step.id + suffix;
            if (valuePath) setPath(step, valuePath, key); else step[keyPath] = key;
        }
        var box = el("div", { class: "text-key" });
        box.appendChild(el("span", {}, label));
        box.appendChild(el("code", { class: "key-field" }, key));
        var dictionary = state.definition.strings[state.language] || (state.definition.strings[state.language] = {});
        var field = el("textarea", { "aria-label": label, placeholder: translated(key, state.language) });
        field.rows = ["Title", "Choice label"].includes(label) ? 3 : 6;
        field.value = textValue(key, state.language);
        var hasLocaleText = Boolean(dictionary[key] || catalogValue(state.language, key));
        var hasDefaultText = Boolean(textValue(key, state.definition.default_locale));
        var fallbackNote = el("small", { class: "translation-fallback", role: "status", hidden: state.language === state.definition.default_locale || hasLocaleText || !hasDefaultText }, "Showing episode default-language text. Editing adds this language.");
        field.addEventListener("input", function () {
            dictionary[key] = field.value;
            fallbackNote.hidden = state.language === state.definition.default_locale || Boolean(dictionary[key] || catalogValue(state.language, key)) || !hasDefaultText;
            markDirty();
        });
        box.appendChild(field);
        box.appendChild(fallbackNote);
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
            var discarded = [];
            if (step.lines && value !== "scene") discarded.push("dialogue lines");
            if (step.choices && value !== "choice") discarded.push("decision answers");
            if (step.trigger && (!keepsMechanics || (value === "guide" && step.trigger.type === "elapsed"))) discarded.push("mechanic condition");
            if (step.guide && (!keepsMechanics || (value === "guide" && step.trigger && step.trigger.type === "elapsed"))) discarded.push("hand guide");
            if (step.marker && !keepsMechanics) discarded.push("map marker");
            if (step.routes && !keepsFlow) discarded.push("conditional routes");
            if (step.hint_key && !keepsMechanics) discarded.push("player hint");
            if (step.presentation && !["scene", "end"].includes(value)) discarded.push("presentation style");
            if (step.next && !keepsFlow && value !== "choice") discarded.push("outgoing connection");
            if (divergentChoiceTargets && value !== "choice") discarded.push("branch destinations");
            if (discarded.length && !confirm("Changing this step to “" + value + "” removes its " + discarded.join(", ") + ". Continue?")) { renderInspector(); return; }
            var ending = state.definition.steps.find(function (item) { return item !== step && item.type === "end"; });
            if (!next && !divergentChoiceTargets && value !== "end" && value !== "choice" && ending) next = ending.id;
            var menuAction = inGameUiTargets()[0] || "menu_campaign";
            var firstLine = step.lines && step.lines[0];
            var replacement = { id: id, type: value, title_key: step.title_key || "tutorial." + id + "_title" };
            var bodyKey = step.body_key || firstLine && firstLine.body_key || (value === "scene" && step.hint_key);
            if (bodyKey) replacement.body_key = bodyKey;
            if (step.speaker || firstLine && firstLine.speaker) replacement.speaker = step.speaker || firstLine.speaker;
            if (Number.isFinite(step.attack_ratio_on_enter)) replacement.attack_ratio_on_enter = step.attack_ratio_on_enter;
            if (["scene", "end"].includes(value)) replacement.presentation = step.presentation || "dialogue";
            if (value === "scene") {
                if (Array.isArray(step.lines)) replacement.lines = step.lines;
                if (next) replacement.next = next;
                if (Array.isArray(step.routes)) replacement.routes = step.routes;
            }
            if (value === "choice") replacement.choices = step.type === "choice" && Array.isArray(step.choices)
                ? step.choices
                : [{ id: "first_answer", label_key: "tutorial." + id + "_first", next: step.next || "" }, { id: "second_answer", label_key: "tutorial." + id + "_second", next: step.next || "" }];
            if (value === "objective" || value === "guide") {
                if (step.hint_key) replacement.hint_key = step.hint_key;
                replacement.trigger = step.trigger && !(value === "guide" && step.trigger.type === "elapsed")
                    ? step.trigger
                    : state.flow === "menu" ? { type: "ui", action: menuAction, scope: "step" } : { type: "territory", value: 1, scope: "step" };
                if (step.marker) replacement.marker = step.marker;
                if (Array.isArray(step.routes)) replacement.routes = step.routes;
                if (step.guide && !(value === "guide" && step.trigger && step.trigger.type === "elapsed")) replacement.guide = step.guide;
                else if (value === "guide") replacement.guide = replacement.trigger.type === "ui"
                    ? { kind: "ui", target: replacement.trigger.action, gesture: "tap" }
                    : { kind: "world", target: "expand", gesture: "tap" };
                if (next) replacement.next = next;
            }
            Object.keys(step).forEach(function (key) { delete step[key]; });
            Object.assign(step, replacement);
            if (xy) state.definition.layout[id] = xy;
            notice(""); markDirty(); renderInspector();
        }));
        basics.appendChild(selectField("Speaker", step.speaker || "", speakerOptions(), function (value) { step.speaker = value || undefined; if (!value) delete step.speaker; markDirty(); }));
        if (step.type === "scene" || step.type === "end") basics.appendChild(selectField("Presentation", step.presentation || "dialogue", ["dialogue", "chapter"], function (value) { step.presentation = value; markDirty(); }));
        basics.appendChild(inputField("Set send percentage on entry (%)", step.attack_ratio_on_enter == null ? "" : Math.round(step.attack_ratio_on_enter * 100), function (value) {
            if (value.trim() === "") delete step.attack_ratio_on_enter;
            else step.attack_ratio_on_enter = Number(value) / 100;
            markDirty();
        }, { type: "number", min: 5, max: 100, step: 1, placeholder: "No change" }));
        textEditor(basics, "Title", "title_key");
        if (step.body_key || step.type === "choice" || (step.type === "scene" && !Array.isArray(step.lines))) textEditor(basics, step.type === "end" ? "Closing text" : "Story text", "body_key");
        else if (step.type === "end") {
            var addEnding = el("button", { type: "button" }, "Add closing text");
            addEnding.addEventListener("click", function () { step.body_key = "tutorial." + step.id + "_body"; markDirty(); renderInspector(); });
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
                event.preventDefault(); step.lines.push({ speaker: step.speaker, body_key: "tutorial." + step.id + "_line_" + (step.lines.length + 1) }); markDirty(); renderInspector();
            });
        } else if (step.type === "scene") {
            var addConversation = el("button", { type: "button" }, "Split into dialogue lines");
            addConversation.addEventListener("click", function () { step.lines = [{ speaker: step.speaker, body_key: step.body_key || "tutorial." + step.id + "_line_1" }]; delete step.body_key; markDirty(); renderInspector(); });
            basics.appendChild(addConversation);
        }
        host.appendChild(basics);

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
                    addDetail.addEventListener("click", function () { choice.body_key = "tutorial." + step.id + "_" + choice.id + "_detail"; markDirty(); renderInspector(); });
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
            if (step.choices.length < 4) { var add = el("button", { type: "button" }, "Add answer"); add.addEventListener("click", function () { var answerId = uniqueId("answer", step.choices); step.choices.push({ id: answerId, label_key: "tutorial." + step.id + "_" + answerId, next: state.definition.steps.find(function (item) { return item.type === "end"; })?.id || "" }); markDirty(); renderInspector(); }); answers.appendChild(add); }
            host.appendChild(answers);
        }
        if (step.type === "objective" || step.type === "guide") {
            var objective = section("Mechanic and guide");
            step.trigger = step.trigger || { type: "territory", value: 1, scope: "step" };
            if (step.type === "guide" && !step.guide) step.guide = { kind: "world", target: "expand", gesture: "tap" };
            var availableTriggers = state.flow === "menu"
                ? triggerTypes.filter(function (trigger) { return trigger.value === "ui"; })
                : step.type === "guide" ? triggerTypes.filter(function (trigger) { return trigger.value !== "elapsed"; }) : triggerTypes;
            objective.appendChild(selectField("Complete when", step.trigger.type, availableTriggers, function (value) {
                step.trigger = { type: value, scope: value === "troops" ? "total" : step.trigger.scope || "step" };
                if (["contact", "defeated"].includes(value)) step.trigger.target = "";
                else if (value === "ui") {
                    step.trigger.action = state.flow === "menu" ? inGameUiTargets()[0] || "menu_campaign" : "map_attack";
                    if (step.guide) Object.assign(step.guide, { kind: "ui", target: step.trigger.action });
                }
                else step.trigger.value = 1;
                if (step.guide && value !== "ui" && step.guide.kind === "ui") Object.assign(step.guide, { kind: "world", target: worldGuideTarget(value) });
                if (value === "elapsed" && step.type === "objective") delete step.guide;
                markDirty(); renderInspector();
            }));
            if (step.trigger.type === "contact") {
                if (Array.isArray(step.trigger.targets)) {
                    objective.appendChild(multiFactionField("Contact any selected faction", step.trigger.targets, function (targets) { step.trigger.targets = targets; markDirty(); }));
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
            else if (step.trigger.type === "attack") objective.appendChild(selectField("Attack target", step.trigger.target || "", [{ value: "", label: "Any faction" }].concat(factionOptions()), function (value) { if (value) step.trigger.target = value; else delete step.trigger.target; markDirty(); }));
            else if (step.trigger.type === "ui") objective.appendChild(selectField("Control action", step.trigger.action, inGameUiTargets(), function (value) { step.trigger.action = value; markDirty(); }));
            else objective.appendChild(inputField(step.trigger.type === "troops" ? "Minimum troops" : step.trigger.type === "elapsed" ? "Wait (seconds)" : "Required amount", step.trigger.value, function (value) { step.trigger.value = Number(value); markDirty(); }, { type: "number", min: 1, step: 1 }));
            if (step.trigger.type !== "troops") objective.appendChild(selectField("Count from", step.trigger.scope || "step", ["step", "episode", "total"], function (value) { step.trigger.scope = value; markDirty(); }));
            if (step.guide) {
                objective.appendChild(selectField("Hand points at", step.guide.kind + ":" + step.guide.target, [
                    { value: "world:expand", label: "Map · expansion" }, { value: "world:assault", label: "Map · attack" }, { value: "world:target_action", label: "Map · target action" }, { value: "world:player", label: "Map · player base" }
                ].concat(inGameUiTargets().map(function (key) { return { value: "ui:" + key, label: "Interface · " + key.replace(/_/g, " ") }; })), function (value) { var pair = value.split(":"); if (pair[0] !== step.guide.kind) delete step.guide.to; step.guide.kind = pair[0]; step.guide.target = pair[1]; markDirty(); renderInspector(); }));
                objective.appendChild(selectField("Gesture", step.guide.gesture || "tap", ["tap", "hold", "drag"], function (value) { step.guide.gesture = value; if (value !== "drag") delete step.guide.to; markDirty(); renderInspector(); }));
                if (step.guide.gesture === "drag") {
                    var dragTargets = step.guide.kind === "world"
                        ? [{ value: "expand", label: "Map · expansion" }, { value: "assault", label: "Map · attack" }, { value: "target_action", label: "Map · target action" }, { value: "player", label: "Map · player base" }]
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
                    step.guide = step.trigger.type === "ui"
                        ? { kind: "ui", target: step.trigger.action, gesture: "tap" }
                        : { kind: "world", target: worldGuideTarget(step.trigger.type), gesture: "tap" };
                    markDirty(); renderInspector();
                });
                objective.appendChild(addGuide);
            }
            objective.appendChild(selectField("Next step", step.next, destinationOptions(step.id), function (value) { step.next = value; markDirty(); }));
            if (step.marker) objective.appendChild(selectField("Map marker", step.marker.target, [{ value: "player", label: "Player" }].concat(factionOptions()), function (value) { step.marker.target = value; markDirty(); }));
            else { var addMarker = el("button", { type: "button" }, "Add map marker"); addMarker.addEventListener("click", function () { step.marker = { target: "player" }; markDirty(); renderInspector(); }); objective.appendChild(addMarker); }
            host.appendChild(objective);
        } else if (step.type !== "choice" && step.type !== "end") {
            var next = section("Flow");
            next.appendChild(selectField("Next step", step.next, destinationOptions(step.id), function (value) { step.next = value; markDirty(); }));
            host.appendChild(next);
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
                    var gameFacts = Object.values(window.SOWCampaign.METRICS).concat(["tiles"]).filter(function (value, index, all) { return all.indexOf(value) === index; });
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
        var title = textValue(step.title_key, state.language);
        return [title && title !== step.id ? title : "", step.type, step.id].filter(Boolean).join(" · ");
    }
    function stepOption(step) { return { value: step.id, label: stepLabel(step) }; }
    function destinationOptions(except) {
        return state.definition.steps.filter(function (step) { return step.id !== except; }).map(stepOption);
    }
    function factionOptions() { return state.roster.factions.map(function (faction) { return { value: faction.name, label: faction.name }; }); }
    function speakerOptions() { return [{ value: "", label: "Narrator" }].concat(Object.keys(state.definition.speakers || {}).map(function (id) { var speaker = state.definition.speakers[id]; return { value: id, label: speaker.faction || speaker.name || id }; })); }
    function inGameUiTargets() { return Object.keys(window.SOWCampaign.UI_TARGETS).filter(function (key) { return state.flow === "menu" ? key.startsWith("menu_") || key === "campaign_replay" : !key.startsWith("menu_") && key !== "campaign_replay"; }); }
    function worldGuideTarget(type) { return ["attack", "kills"].includes(type) ? "assault" : ["contact", "defeated"].includes(type) ? "target_action" : "expand"; }
    function flowEntry() { return state.flow === "menu" ? state.definition.menu_guide && state.definition.menu_guide.entry : state.definition.entry; }
    function choiceAnswerOptions() {
        var result = [];
        state.definition.steps.filter(function (step) { return step.type === "choice"; }).forEach(function (step) {
            (step.choices || []).forEach(function (answer) { result.push({ value: step.id + "@" + answer.id, label: stepLabel(step) + " · " + (textValue(answer.label_key, state.language) || answer.id) }); });
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
                var defaultMilestone = settings.buildings_unlock_after_defeated || (state.roster.factions.find(function (faction) { return faction.name === "The Iceni Despoilers"; }) || {}).name || "";
                settings.campaign_support = settings.campaign_support || { after_defeated: defaultMilestone, share_percent: 50 };
            }
            markDirty(); renderSettings();
        }));
        if (settings.campaign_support) {
            var support = settings.campaign_support;
            host.appendChild(selectField("Support begins after", support.after_defeated || "", factionOptions(), function (value) { support.after_defeated = value; markDirty(); }));
            host.appendChild(inputField("Current reserves given (%)", support.share_percent, function (value) { support.share_percent = Number(value); markDirty(); }, { type: "number", min: 1, max: 100, step: 1 }));
        }
        host.appendChild(selectField("Default story language", state.definition.default_locale, localeOptions(), function (value) { state.definition.default_locale = value; state.definition.strings[value] = state.definition.strings[value] || {}; markDirty(); renderSettings(); }));
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
                var boundFaction = speaker.faction && state.roster.factions.find(function (faction) { return faction.name === speaker.faction; });
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
            var displayName = speaker.name_key ? textValue(speaker.name_key, state.language) : speaker.name || "";
            var portrait = el("img", { class: "speaker-avatar", alt: displayName || speakerId, loading: "lazy", hidden: !speaker.avatar });
            portrait.addEventListener("error", function () { portrait.hidden = true; });
            if (speaker.name_key) {
                card.appendChild(inputField("Character name · " + state.language, displayName, function (value) {
                    var dictionary = state.definition.strings[state.language] || (state.definition.strings[state.language] = {});
                    dictionary[speaker.name_key] = value; portrait.alt = value || speakerId; markDirty();
                }, { input: true }));
                var fixedName = el("button", { type: "button" }, "Use one name for every language");
                fixedName.addEventListener("click", function () {
                    speaker.name = textValue(speaker.name_key, state.definition.default_locale) || speaker.name || speakerId;
                    delete speaker.name_key; markDirty(); renderSettings();
                });
                card.appendChild(fixedName);
            } else {
                card.appendChild(inputField("Character name", speaker.name || "", function (value) { speaker.name = value; portrait.alt = value || speakerId; markDirty(); }));
                var localizeName = el("button", { type: "button" }, "Localize name");
                localizeName.addEventListener("click", function () {
                    var keyBase = "tutorial.speaker_" + speakerId + "_name", key = keyBase, suffix = 2;
                    while (Object.keys(state.definition.strings).some(function (locale) { var dictionary = state.definition.strings[locale]; return dictionary && Object.prototype.hasOwnProperty.call(dictionary, key); })) key = keyBase + "_" + suffix++;
                    speaker.name_key = key;
                    var dictionary = state.definition.strings[state.definition.default_locale] || (state.definition.strings[state.definition.default_locale] = {});
                    dictionary[speaker.name_key] = speaker.name || speakerId;
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
        if (step.type === "choice") return (step.choices || []).map(function (choice) { return { field: "choices." + choice.id + ".next", label: textValue(choice.label_key, state.language) || choice.id, target: choice.next, className: "choice-wire" }; });
        var outputs = [{ field: "next", label: "Next", target: step.next }];
        (step.routes || []).forEach(function (route, index) {
            var when = route.when || {}, label = "If condition";
            if (when.choice) {
                var decision = state.definition.steps.find(function (item) { return item.id === when.choice; });
                var answer = decision && (decision.choices || []).find(function (item) { return item.id === when.equals; });
                label = "If " + (answer && textValue(answer.label_key, state.language) || when.equals || when.choice);
            } else if (when.fact) label = "If " + when.fact.replace(/_/g, " ") + " ≥ " + String(when.gte);
            outputs.push({ field: "routes." + index + ".next", label: label, target: route.next, className: "route-wire" });
        });
        return outputs;
    }
    function nodeSummary(step) {
        var firstLine = step.lines && step.lines[0];
        var storyText = textValue(firstLine && firstLine.body_key || step.hint_key || step.body_key, state.language);
        if (step.type === "scene") return [step.lines && step.lines.length > 1 ? step.lines.length + " dialogue lines" : "Story scene", storyText].filter(Boolean).join(" · ");
        if (step.type === "choice") return [(step.choices || []).length + " decision paths", textValue(step.body_key, state.language)].filter(Boolean).join(" · ");
        if (step.type === "end") return ["Episode ending", textValue(step.body_key, state.language)].filter(Boolean).join(" · ");
        var trigger = step.trigger || {};
        var definition = triggerTypes.find(function (item) { return item.value === trigger.type; });
        var parts = [definition ? definition.label : "Set a condition"];
        if (trigger.type === "contact" && Array.isArray(trigger.targets)) parts[0] += " · any of " + trigger.targets.join(", ");
        else if (["contact", "defeated", "attack"].includes(trigger.type) && trigger.target) parts[0] += " · " + trigger.target;
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
                if (answer) result.push({ value: String(index), label: (translated(decision.title_key, state.language) || decision.id) + " · " + (translated(answer.label_key, state.language) || answer.id) + " → " + (translated(destination && destination.title_key, state.language) || route.next) });
            } else if (condition.fact && Number.isFinite(condition.gte)) {
                result.push({ value: String(index), label: condition.fact.replace(/_/g, " ") + " ≥ " + condition.gte + " → " + (translated(destination && destination.title_key, state.language) || route.next) });
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
                return speaker.name_key ? textValue(speaker.name_key, state.language) : speaker.name || speakerId;
            });
            var summary = nodeSummary(step);
            var card = el("article", { class: "node" + (step.id === oldSelection ? " selected" : "") + (step.id === state.playingStep ? " playing" : "") + (completedSteps.has(step.id) ? " visited" : "") + (step.id === state.definition.entry ? " is-entry" : "") + (state.definition.menu_guide && step.id === state.definition.menu_guide.entry ? " is-menu-entry" : "") + (stepErrors ? " has-error" : "") + (stepWarnings ? " has-warning" : ""), dataset: { stepId: step.id, stepType: step.type }, tabindex: "0", role: "group", "aria-label": step.type + ": " + (translated(step.title_key, state.language) || step.id) + " · " + summary + (castNames.length ? " · " + castNames.join(", ") : "") });
            card.style.left = pos.x + "px"; card.style.top = pos.y + "px";
            var head = el("header", { class: "node-hd" }); head.append(el("span", {}, step.type), el("small", { title: step.id, "aria-label": step.id }, rootLabel)); card.appendChild(head);
            card.appendChild(el("div", { class: "node-title" }, translated(step.title_key, state.language) || step.title_key || "Untitled"));
            card.appendChild(el("div", { class: "node-summary", title: summary }, summary));
            if (castIds.length) {
                var cast = el("div", { class: "node-cast", "aria-hidden": "true" });
                castIds.slice(0, 3).forEach(function (speakerId) {
                    var speaker = speakers[speakerId] || {};
                    var name = speaker.name_key ? textValue(speaker.name_key, state.language) : speaker.name || speakerId;
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
            var input = el("button", { type: "button", class: "pin in", title: "Connect here", "aria-label": "Connect a path to " + (translated(step.title_key, state.language) || step.id), dataset: { in: step.id } });
            input.addEventListener("keydown", connectPinByKeyboard); card.appendChild(input);
            portDefinitions(step).forEach(function (port, index) {
                var row = el("div", { class: "port-row" + (port.className ? " " + port.className : "") });
                row.appendChild(el("span", { class: "port-label", title: port.label }, port.label));
                var destination = byId[port.target];
                row.appendChild(el("small", { title: destination ? stepLabel(destination) : "Unconnected" }, destination ? stepLabel(destination) : "Unconnected"));
                var output = el("button", { type: "button", class: "pin out", title: "Drag to connect", "aria-label": "Connect " + port.label + " from " + (translated(step.title_key, state.language) || step.id), dataset: { out: step.id, field: port.field, outputIndex: index } });
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
        var step = { id: id, type: type, title_key: "tutorial." + id + "_title", body_key: "tutorial." + id + "_body", speaker: Object.keys(state.definition.speakers)[0] };
        if (type === "end") { delete step.body_key; }
        else if (type === "choice") { step.choices = [{ id: "first", label_key: "tutorial." + id + "_first", next: next }, { id: "second", label_key: "tutorial." + id + "_second", next: next }]; }
        else if (type === "objective" || type === "guide") {
            delete step.body_key; step.hint_key = "tutorial." + id + "_hint";
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
        var model;
        try { model = updateMachine === false ? state.machine.view() : state.machine.update(state.facts, state.ui, performance.now()); }
        catch (error) { $("#previewStatus").textContent = "Preview unavailable: " + error.message; return; }
        state.playingStep = model.step.id;
        var actionRatio = Number.isFinite(model.step.attack_ratio_on_enter) ? model.step.attack_ratio_on_enter : null;
        if (model.step.id !== state.previewActionStep || actionRatio !== state.previewActionRatio) {
            state.previewActionStep = model.step.id;
            state.previewActionRatio = actionRatio;
            if (actionRatio != null) $("#sow-hud-slider").value = String(Math.round(actionRatio * 100));
        }
        var anchor = previewAnchor(model.step);
        try { state.renderer.render(model, { anchor: anchor, reducedMotion: $("#reducedMotion").checked, direction: rtlLanguages.has(state.previewLanguage.toLowerCase().split("-")[0]) ? "rtl" : "ltr", localeScript: previewLocaleScript(state.previewLanguage) }); }
        catch (error) { $("#previewStatus").textContent = "Preview unavailable: " + error.message; return; }
        $("#engineState").textContent = JSON.stringify({ step: model.step.id, type: model.step.type, progress: model.progress, choices: model.state.choices }, null, 2);
        var guideTarget = model.step.guide && model.step.guide.kind === "ui" ? model.step.guide.target : "";
        var requiredMenu = /^map_(?:build|upgrade)_/.test(guideTarget) ? "build" : "";
        var menuHint = requiredMenu && $("#sow-hud").dataset.previewMapMenu !== requiredMenu ? " · open the " + requiredMenu + " submenu in the preview to reveal this guide" : "";
        if (!menuHint && guideTarget === "campaign_replay") {
            var replayTarget = window.SOWCampaign.resolveUiTarget("campaign_replay", document, episodeId);
            if (!replayTarget || !replayTarget.getClientRects().length) menuHint = " · open Campaign in the preview to reveal Replay";
        }
        var previewTitle = translated(model.step.title_key, state.previewLanguage) || model.step.id.replace(/_/g, " ");
        $("#previewStatus").textContent = (state.demoBackup ? "Sample preview · not saved — " : "Previewing · ") + previewTitle + (model.ready ? " · objective complete" : "") + menuHint + (state.validation.errors.length ? " · draft needs fixes before export" : "");
        renderGraph();
        renderFactControls(model);
        if (model.ready && updateMachine !== false) window.setTimeout(function () { if (state.machine) paintPreview(); }, 840);
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
        if (target === "player") {
            var playerTarget = step.marker && step.marker.target;
            if (!playerTarget || playerTarget === "player") return frame.querySelector(".player-base");
            var playerFaction = factions.find(function (item) { return item.name === playerTarget; });
            return playerFaction && Array.from(frame.querySelectorAll(".sample-faction")).find(function (marker) { return marker.dataset.factionName === playerFaction.name; }) || null;
        }
        var observed = step.trigger && step.trigger.type === "contact" ? state.facts && state.facts.contact_names || [] : state.facts && state.facts.defeated_names || [];
        var requested = target === "target_action" && (Array.isArray(step.trigger && step.trigger.targets)
            ? step.trigger.targets.find(function (name) { return !observed.includes(name); })
            : step.trigger && step.trigger.target || step.marker && step.marker.target !== "player" && step.marker.target);
        var faction = requested && factions.find(function (item) { return item.name === requested; });
        if (!faction && state.roster) {
            var spawn = state.roster.player_spawn;
            faction = factions.filter(function (item) { return item.role !== "kin"; }).sort(function (a, b) {
                return Math.hypot(a.x - spawn[0], a.y - spawn[1]) - Math.hypot(b.x - spawn[0], b.y - spawn[1]);
            })[0];
        }
        return faction && Array.from(frame.querySelectorAll(".sample-faction")).find(function (marker) { return marker.dataset.factionName === faction.name; }) || null;
    }
    function focusPreviewTarget() {
        if (state.flow !== "episode" || !state.machine) return;
        state.previewFocusActive = true;
        applyPreviewFocus();
        paintPreview(false);
    }
    function applyPreviewFocus() {
        if (!state.previewFocusActive || !state.machine) return;
        var step = state.machine.view().step, observed = step.trigger && step.trigger.type === "contact" ? state.facts && state.facts.contact_names || [] : state.facts && state.facts.defeated_names || [];
        var target = step.guide && step.guide.kind === "world" && step.guide.target === "target_action"
            ? Array.isArray(step.trigger && step.trigger.targets) ? step.trigger.targets.find(function (name) { return !observed.includes(name); }) : step.trigger && step.trigger.target || step.marker && step.marker.target
            : step.marker && step.marker.target;
        var marker = target === "player" ? $("#campaignPreviewMarkers .player-base")
            : target ? Array.from(document.querySelectorAll("#campaignPreviewMarkers .sample-faction")).find(function (item) { return item.dataset.factionName === target; })
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
        try { state.machine = window.SOWCampaign.create(state.definition, requestedEntry); }
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
        var canTick = Boolean((trigger && trigger.type === "elapsed") || timedRoute);
        var simulateButton = $("#simulateBtn"), tickButton = $("#tickBtn");
        simulateButton.hidden = !trigger;
        simulateButton.disabled = !trigger || model.ready;
        tickButton.hidden = !canTick;
        tickButton.disabled = !canTick || model.ready;
        if (!trigger) {
            host.appendChild(el("small", {}, timedRoute ? "Advance game time to test this route." : "Select an objective or guide to simulate game facts."));
            return;
        }
        var progress = model.progress;
        host.appendChild(el("small", {}, trigger.type + " · " + progress.current + " / " + progress.target));
        if (["troops", "elapsed"].includes(trigger.type)) return;
        var button = el("button", { type: "button" }, ["contact", "defeated"].includes(trigger.type) ? "+1 faction" : "+1 " + trigger.type);
        button.disabled = model.ready;
        button.addEventListener("click", function () { simulateObjective(1); });
        host.appendChild(button);
    }
    function simulateObjective(amount) {
        if (!state.machine) return;
        var step = state.machine.view().step, trigger = step.trigger;
        if (!trigger) return;
        if (amount == null) { var progress = state.machine.view().progress; amount = Math.max(1, progress.target - progress.current); }
        amount = Number(amount || 1);
        if (trigger.type === "contact") {
            var contacts = state.facts.contact_names || [];
            var candidates = Array.isArray(trigger.targets) ? trigger.targets : [trigger.target];
            var contacted = candidates.find(function (name) { return name && !contacts.includes(name); }) || candidates[0];
            if (contacted) state.facts.contact_names = Array.from(new Set(contacts.concat(contacted)));
        }
        else if (trigger.type === "defeated") {
            var targets = Array.isArray(trigger.targets) ? trigger.targets : [trigger.target];
            var defeated = state.facts.defeated_names || [];
            state.facts.defeated_names = Array.from(new Set(defeated.concat(targets.filter(function (name) { return name && !defeated.includes(name); }).slice(0, amount))));
        }
        else if (trigger.type === "ui") state.ui[trigger.action] = Number(state.ui[trigger.action] || 0) + amount;
        else {
            var metric = window.SOWCampaign.METRICS[trigger.type];
            if (trigger.type === "attack" && trigger.target) { state.facts.attacks_by_target = state.facts.attacks_by_target || {}; state.facts.attacks_by_target[trigger.target] = Number(state.facts.attacks_by_target[trigger.target] || 0) + amount; }
            else state.facts[metric] = Number(state.facts[metric] || 0) + amount;
            if (trigger.type === "territory") state.facts.tiles = Number(state.facts.tiles || 0) + amount;
        }
        paintPreview();
    }
    function freshFacts() { return { tiles: 0, tiles_gained: 0, kills: 0, troops: Number(state.definition.settings && state.definition.settings.starting_troops) || 0, buildings: 0, cities: 0, ally_support_deliveries: 0, fleets: 0, nukes: 0, attacks: 0, attacks_by_target: {}, contact_names: [], defeated_names: [], elapsed_seconds: 0 }; }
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
            box.appendChild(el("strong", {}, errors.length ? "Fix this step before export" : "Review this step"));
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
        state.definition = {
            version: 2, episode_id: episodeId, default_locale: "en", settings: state.definition.settings,
            entry: id + "_opening", speakers: speakers, strings: { en: {}, es: {} }, layout: {},
            steps: [
                { id: id + "_opening", type: "scene", title_key: "tutorial." + id + "_title", lines: [{ speaker: leaderId, body_key: "tutorial." + id + "_body" }, { speaker: advisorId, body_key: "tutorial." + id + "_advisor_line" }], presentation: "chapter", next: id + "_decision" },
                { id: id + "_decision", type: "choice", title_key: "tutorial." + id + "_decision_title", body_key: "tutorial." + id + "_decision_body", choices: [
                    { id: "gather", label_key: "tutorial." + id + "_gather", next: id + "_grow" }, { id: "strike", label_key: "tutorial." + id + "_strike", next: id + "_attack" }
                ] },
                { id: id + "_grow", type: "objective", title_key: "tutorial." + id + "_grow_title", hint_key: "tutorial." + id + "_grow_hint", trigger: { type: "territory", value: 300, scope: "step" }, guide: { kind: "world", target: "expand", gesture: "tap" }, next: id + "_recall" },
                { id: id + "_attack", type: "objective", title_key: "tutorial." + id + "_attack_title", hint_key: "tutorial." + id + "_attack_hint", trigger: { type: "kills", value: 1, scope: "total" }, guide: { kind: "world", target: "assault", gesture: "tap" }, next: id + "_recall" },
                { id: id + "_recall", type: "scene", title_key: "tutorial." + id + "_recall_title", body_key: "tutorial." + id + "_recall_body", next: id + "_turn", routes: [
                    { when: { choice: id + "_decision", equals: "gather" }, next: id + "_gather_echo" },
                    { when: { choice: id + "_decision", equals: "strike" }, next: id + "_strike_echo" }
                ] },
                { id: id + "_gather_echo", type: "scene", title_key: "tutorial." + id + "_gather_echo_title", body_key: "tutorial." + id + "_gather_echo_body", next: id + "_turn" },
                { id: id + "_strike_echo", type: "scene", title_key: "tutorial." + id + "_strike_echo_title", body_key: "tutorial." + id + "_strike_echo_body", next: id + "_turn" },
                { id: id + "_turn", type: "choice", title_key: "tutorial." + id + "_turn_title", body_key: "tutorial." + id + "_turn_body", choices: [
                    { id: "press", label_key: "tutorial." + id + "_press", next: id + "_advance" }, { id: "secure", label_key: "tutorial." + id + "_secure", next: id + "_hold" }
                ] },
                { id: id + "_advance", type: "objective", title_key: "tutorial." + id + "_advance_title", hint_key: "tutorial." + id + "_advance_hint", trigger: { type: "territory", value: 100, scope: "step" }, guide: { kind: "world", target: "expand", gesture: "tap" }, next: id + "_ending" },
                { id: id + "_hold", type: "objective", title_key: "tutorial." + id + "_hold_title", hint_key: "tutorial." + id + "_hold_hint", trigger: { type: "ui", action: "attack_ratio", scope: "step" }, guide: { kind: "ui", target: "attack_ratio", gesture: "drag" }, next: id + "_ending" },
                { id: id + "_ending", type: "end", title_key: "tutorial." + id + "_end_title", body_key: "tutorial." + id + "_end_body", presentation: "chapter" }
            ]
        };
        var texts = {
            en: { _title: "A story shaped by your choices", _body: "We have waited long enough. The first move is ours.", _advisor_line: "Then choose: gather strength, or strike the frontier.", _decision_title: "How should the campaign begin?", _decision_body: "Both paths teach a different mechanic, then meet again.", _gather_title: "Rally your forces", _grow_hint: "Claim territory and bring more people under your banner.", _strike_title: "Attack the frontier", _attack_hint: "Win a fight to open the road ahead.", _gather: "Gather strength", _strike: "Attack now", _recall_title: "Your choice is remembered", _recall_body: "The story can react to the path you chose.", _gather_echo_title: "The people answer", _gather_echo_body: "You chose to build strength before risking a fight.", _strike_echo_title: "The frontier is breached", _strike_echo_body: "You chose to take the fight to your enemy.", _turn_title: "The next decision is yours", _turn_body: "Keep pressing forward or give your forces time to regroup.", _press: "Keep moving", _secure: "Regroup first", _advance_title: "Press forward", _advance_hint: "Claim another stretch of ground.", _hold_title: "Set the troop balance", _hold_hint: "Drag the slider to choose how many troops to send.", _end_title: "Different paths, one campaign", _end_body: "The branches meet here. They could also continue separately." },
            es: { _title: "Una historia que cambia con tus decisiones", _body: "Ya hemos esperado suficiente. El primer movimiento es nuestro.", _advisor_line: "Entonces elige: reunir fuerzas o atacar la frontera.", _decision_title: "¿Cómo empieza la campaña?", _decision_body: "Cada camino enseña una mecánica distinta y después se reúnen.", _gather_title: "Reúne tus fuerzas", _grow_hint: "Conquista territorio y suma gente a tu estandarte.", _strike_title: "Ataca la frontera", _attack_hint: "Gana un combate para abrir el camino.", _gather: "Reunir fuerzas", _strike: "Atacar ahora", _recall_title: "La historia recuerda tu decisión", _recall_body: "El relato puede reaccionar al camino que elegiste.", _gather_echo_title: "La gente responde", _gather_echo_body: "Elegiste reunir fuerzas antes de arriesgarte a luchar.", _strike_echo_title: "La frontera cede", _strike_echo_body: "Elegiste llevar la lucha hasta tu enemigo.", _turn_title: "Tu siguiente decisión", _turn_body: "Sigue avanzando o dale tiempo a tus fuerzas para reagruparse.", _press: "Seguir avanzando", _secure: "Reagruparse primero", _advance_title: "Presiona la frontera", _advance_hint: "Conquista otro tramo de terreno.", _hold_title: "Ajusta el reparto de tropas", _hold_hint: "Arrastra el deslizador para elegir cuántas tropas enviar.", _end_title: "Distintos caminos, una campaña", _end_body: "Las ramas se reúnen aquí; también podrían continuar separadas." }
        };
        Object.keys(texts).forEach(function (locale) {
            Object.keys(texts[locale]).forEach(function (suffix) { state.definition.strings[locale]["tutorial." + id + suffix] = texts[locale][suffix]; });
        });
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
        var base = episodeId + "_menu_guide";
        function unique(baseId) { return uniqueId(baseId, state.definition.steps); }
        var opening = unique(base + "_opening"), campaign = unique(base + "_campaign"), replay = unique(base + "_replay"), ending = unique(base + "_end");
        var keys = { opening: "tutorial." + opening, campaign: "tutorial." + campaign, replay: "tutorial." + replay, ending: "tutorial." + ending };
        state.definition.steps.push(
            { id: opening, type: "scene", title_key: keys.opening + "_title", body_key: keys.opening + "_body", presentation: "chapter", next: campaign },
            { id: campaign, type: "guide", title_key: keys.campaign + "_title", hint_key: keys.campaign + "_hint", trigger: { type: "ui", action: "menu_campaign", scope: "step" }, guide: { kind: "ui", target: "menu_campaign", gesture: "tap" }, next: replay },
            { id: replay, type: "guide", title_key: keys.replay + "_title", hint_key: keys.replay + "_hint", trigger: { type: "ui", action: "campaign_replay", scope: "step" }, guide: { kind: "ui", target: "campaign_replay", gesture: "tap" }, next: ending },
            { id: ending, type: "end", title_key: keys.ending + "_title", body_key: keys.ending + "_body", presentation: "chapter" }
        );
        state.definition.menu_guide = { entry: opening, dismissible: true };
        state.definition.strings.en = Object.assign(state.definition.strings.en || {}, {
            [keys.opening + "_title"]: "Choose your next battle",
            [keys.opening + "_body"]: "Continue a campaign, replay this episode, or head into multiplayer.",
            [keys.campaign + "_title"]: "Campaigns",
            [keys.campaign + "_hint"]: "Open Campaigns to continue a story.",
            [keys.replay + "_title"]: "Replay this episode",
            [keys.replay + "_hint"]: "Select this episode to revisit its opening lesson.",
            [keys.ending + "_title"]: "Ready",
            [keys.ending + "_body"]: "Choose what to play next."
        });
        state.definition.strings.es = Object.assign(state.definition.strings.es || {}, {
            [keys.opening + "_title"]: "Elige tu siguiente partida",
            [keys.opening + "_body"]: "Continúa una campaña, repite este episodio o entra al multijugador.",
            [keys.campaign + "_title"]: "Campañas",
            [keys.campaign + "_hint"]: "Abre Campañas para continuar una historia.",
            [keys.replay + "_title"]: "Repetir este episodio",
            [keys.replay + "_hint"]: "Elige este episodio para volver a su lección inicial.",
            [keys.ending + "_title"]: "Listo",
            [keys.ending + "_body"]: "Elige qué jugar ahora."
        });
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
            event.preventDefault(); event.stopPropagation(); chooseFactionTarget(marker.dataset.factionName);
            $("#pickFactionTarget").focus({ preventScroll: true });
        });
        $("#campaignPreviewMarkers").addEventListener("keydown", function (event) {
            if (event.key !== "Enter" && event.key !== " ") return;
            var marker = state.pickingFaction && event.target.closest(".sample-faction.is-faction-pickable");
            if (!marker) return;
            event.preventDefault(); event.stopPropagation(); chooseFactionTarget(marker.dataset.factionName);
            $("#pickFactionTarget").focus({ preventScroll: true });
        });
        $("#previewLocale").addEventListener("change", function () {
            var locale = this.value; state.previewLanguage = locale; paintPreview();
            loadCatalog(locale).then(function () { if (state.previewLanguage === locale) paintPreview(); });
        });
        $("#editLocale").addEventListener("change", function () {
            var locale = this.value; state.language = locale; renderSettings(); renderInspector(); renderGraph();
            loadCatalog(locale).then(function () {
                if (state.language === locale) { renderSettings(); renderInspector(); renderGraph(); }
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
            optionList($("#editLocale"), localeOptions(), state.definition.strings.en ? "en" : state.definition.default_locale);
            optionList($("#previewLocale"), localeOptions(), state.definition.default_locale);
            state.language = $("#editLocale").value; state.previewLanguage = $("#previewLocale").value;
            return Promise.all([loadCatalog(state.language), loadCatalog(state.previewLanguage)]).then(function () {
                if (!results[2]) notice("Could not load the episode list; only the built-in episodes are available.");
                else if (state.localeRegistryFailed) notice("Language list unavailable; showing the episode's available text.");
                else if (state.localeFailures.length) notice("Catalog unavailable for " + state.localeFailures.join(", ") + "; episode text remains editable.");
                refresh(); renderSettings(); renderInspector(); renderPreview();
            });
        }).catch(function (error) { notice(error.message); status("Load failed", "error"); });
    }
    init();
})();
