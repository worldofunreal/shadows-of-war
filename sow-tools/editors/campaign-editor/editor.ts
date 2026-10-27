(function () {
  "use strict";

  const $ = function (selector) {
    return document.querySelector(selector);
  };
  const query = new URLSearchParams(location.search);
  const episodeId = /^[a-z][a-z0-9_]{0,63}$/.test(query.get("episode") || "")
    ? query.get("episode")
    : "boudica";
  const rtlLanguages = new Set([
    "ar",
    "arc",
    "ckb",
    "dv",
    "fa",
    "he",
    "iw",
    "nqo",
    "pnb",
    "ps",
    "sd",
    "syr",
    "ug",
    "ur",
    "yi",
  ]);
  const state = {
    roster: null,
    definition: null,
    etag: null,
    rosterEtag: null,
    externalChangeTag: null,
    dirty: false,
    saving: false,
    demoBackup: null,
    selected: null,
    previewBranch: null,
    previewFocusActive: false,
    previewActionStep: null,
    previewActionRatio: null,
    previewRevealTimer: null,
    previewRevealToken: 0,
    pickingFaction: false,
    flow: "episode",
    isMenuGuides: false,
    menuGuideId: "welcome",
    previewLanguage: "en",
    catalogs: {},
    localeFailures: [],
    localeRegistryFailed: false,
    avatars: [],
    machine: null,
    renderer: null,
    facts: {},
    ui: {},
    mapPreview: null,
    mapResizeObserver: null,
    mapResizeFallback: false,
    mapPreviewLoadingId: null,
    zoom: 1,
    pan: { x: 36, y: 44 },
    drag: null,
    wire: null,
    keyboardWire: null,
    validation: { errors: [], warnings: [] },
  };
  const types = ["scene", "choice", "objective", "guide", "end"];
  const triggerTypes = [
    { value: "territory", label: "Gain territory" },
    { value: "kills", label: "Defeat troops" },
    { value: "attack", label: "Launch an attack" },
    { value: "troops", label: "Reach a troop minimum" },
    { value: "building", label: "Complete a building" },
    { value: "city", label: "Complete a city" },
    { value: "farm", label: "Complete a farm" },
    { value: "factory", label: "Complete a factory" },
    { value: "port", label: "Complete a port" },
    { value: "tower", label: "Complete a tower" },
    { value: "structure_upgrade", label: "Upgrade a structure" },
    { value: "structure_level", label: "Reach a building level" },
    { value: "city_upgrade", label: "Upgrade a city" },
    { value: "city_level", label: "Reach a city level" },
    { value: "port_upgrade", label: "Upgrade a port" },
    { value: "port_level", label: "Reach port level" },
    { value: "resource_transfer", label: "Send resources" },
    { value: "alliance", label: "Form an alliance" },
    { value: "support", label: "Receive allied support" },
    { value: "fleet", label: "Launch a fleet" },
    { value: "nuke", label: "Launch a nuke" },
    { value: "elapsed", label: "Wait for game time" },
    { value: "contact", label: "Reach a faction" },
    { value: "defeated", label: "Defeat a faction" },
    { value: "eliminated", label: "Faction eliminated" },
    { value: "ui", label: "Use a control" },
    { value: "zoom_in", label: "Zoom in" },
    { value: "zoom_out", label: "Zoom out" },
    { value: "camera_drag", label: "Pan camera by dragging" },
    { value: "camera_key_pan", label: "Pan camera with keys" },
    { value: "hover", label: "Hover an entity" },
    { value: "zoom_out_complete", label: "Reach zoom-out distance" },
    { value: "zoom_in_complete", label: "Zoom in on a faction" },
    { value: "camera_target", label: "Center the camera on a target" },
  ];
  let locales = ["en", "es"],
    localeNames = { en: "English", es: "Español" };
  const catalogLoads = Object.create(null);

  function el(tag, attrs, text) {
    const node = document.createElement(tag);
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
      const item = el(
        "option",
        { value: typeof value === "string" ? value : value.value },
        typeof value === "string" ? (label ? label(value) : value) : value.label,
      );
      select.appendChild(item);
    });
    select.value = current;
  }
  function localeOptions() {
    return locales.map(function (code) {
      return { value: code, label: localeNames[code] || code.toUpperCase() };
    });
  }
  function defaultLayout(index) {
    return { x: 70 + (index % 3) * 280, y: 70 + Math.floor(index / 3) * 190 };
  }
  function ensureLayout() {
    state.definition.layout = state.definition.layout || {};
    state.definition.steps.forEach(function (step, index) {
      state.definition.layout[step.id] = state.definition.layout[step.id] || defaultLayout(index);
    });
  }
  function normalizeStrings(definition) {
    const legacy = definition.strings;
    if (
      Array.isArray(legacy) ||
      (legacy &&
        typeof legacy === "object" &&
        Object.values(legacy).every(function (catalog) {
          return (
            catalog &&
            typeof catalog === "object" &&
            !Array.isArray(catalog) &&
            Object.keys(catalog).length === 0
          );
        }))
    )
      delete definition.strings;
    delete definition.default_locale;
  }
  function catalogValue(locale, key) {
    const parts = String(key || "").split(".");
    let catalog = state.catalogs[locale] || {};
    if (parts[0] === "tutorial") {
      catalog = catalog.tutorial || {};
      parts.shift();
    }
    return parts.reduce(function (value, part) {
      return value && value[part];
    }, catalog);
  }
  function textValue(key, locale) {
    if (state.isMenuGuides && key) {
      const builtIn = (state.definition && state.definition.strings) || {},
        language = locale || state.previewLanguage;
      const menuText =
        (builtIn[language] && builtIn[language][key]) ||
        (builtIn[String(language).split("-")[0]] && builtIn[String(language).split("-")[0]][key]) ||
        (builtIn.en && builtIn.en[key]);
      if (menuText) return menuText;
    }
    const text = catalogValue(locale || state.previewLanguage, key) || catalogValue("en", key) || "";
    return window.SOWCampaign && typeof window.SOWCampaign.replaceFactionStoryNames === "function"
      ? window.SOWCampaign.replaceFactionStoryNames(text, state.definition, state.roster)
      : text;
  }
  function translated(key, locale) {
    return textValue(key, locale || state.previewLanguage) || (key ? "[" + key + "]" : "");
  }
  function previewMetric(key, values) {
    let text = translated(key, state.previewLanguage);
    Object.keys(values).forEach(function (name) {
      text = text.replace(new RegExp("\\{" + name + "\\}", "g"), String(values[name]));
    });
    return text;
  }
  function asset(path) {
    return "/assets/" + path.split("/").map(encodeURIComponent).join("/");
  }
  function setFactionPicker(active) {
    const step =
      state.definition &&
      state.definition.steps.find(function (item) {
        return item.id === state.selected;
      });
    const picker = $("#pickFactionTarget"),
      wasPicking = state.pickingFaction;
    state.pickingFaction = Boolean(active && step && ["objective", "guide"].includes(step.type));
    picker.disabled = !step || !["objective", "guide"].includes(step.type);
    picker.setAttribute("aria-pressed", String(state.pickingFaction));
    picker.textContent = state.pickingFaction ? "Cancel map pick" : "Pick target on map";
    if (wasPicking !== state.pickingFaction) renderRosterMapPreview();
  }
  function chooseFactionTarget(factionId) {
    const step =
      state.definition &&
      state.definition.steps.find(function (item) {
        return item.id === state.selected;
      });
    if (!step || !["objective", "guide"].includes(step.type)) return;
    if (
      step.trigger &&
      ["contact", "defeated"].includes(step.trigger.type) &&
      Array.isArray(step.trigger.targets)
    ) {
      if (!step.trigger.targets.includes(factionId)) step.trigger.targets.push(factionId);
      if (step.trigger.type === "defeated") step.trigger.value = step.trigger.targets.length;
    } else if (["contact", "defeated", "attack"].includes(step.trigger && step.trigger.type))
      step.trigger.target = factionId;
    else step.marker = { target: factionId };
    setFactionPicker(false);
    markDirty();
    renderInspector();
    renderPreview(step.id);
  }
  function renderRosterMapPreview() {
    if (!state.roster) return;
    const mapId = state.roster.map,
      layer = $("#campaignPreviewLayer"),
      canvas = $("#campaignMapPreview"),
      markers = $("#campaignPreviewMarkers");
    const label = $("#previewMapLabel"),
      count = $("#previewFactionCount"),
      status = $("#previewMapStatus");
    if (state.mapPreview && state.mapPreview.id !== mapId) {
      state.mapPreview = null;
      canvas.width = 1;
      canvas.height = 1;
      markers.replaceChildren();
    }
    label.textContent = mapId.replace(/_/g, " ").toUpperCase();
    count.textContent = state.roster.factions.length + " FACTIONS";
    function draw(map) {
      const width = layer.parentElement.clientWidth,
        height = layer.parentElement.clientHeight,
        scale = Math.min(width / map.width, height / map.height);
      layer.style.width = Math.max(1, map.width * scale) + "px";
      layer.style.height = Math.max(1, map.height * scale) + "px";
      canvas.width = map.width;
      canvas.height = map.height;
      canvas.getContext("2d").drawImage(map.image, 0, 0);
      markers.replaceChildren();
      state.roster.factions.forEach(function (faction) {
        const marker = el("span", {
          class: "sample-map-marker sample-faction",
          title: faction.name,
          "aria-label": faction.name,
          dataset: { factionId: faction.id },
        });
        marker.style.left = (Number(faction.x) / map.width) * 100 + "%";
        marker.style.top = (Number(faction.y) / map.height) * 100 + "%";
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
      const spawn = el("span", {
        class: "sample-map-marker player-base",
        title: "Player spawn",
        "aria-label": "Player spawn",
      });
      spawn.style.left = (Number(state.roster.player_spawn[0]) / map.width) * 100 + "%";
      spawn.style.top = (Number(state.roster.player_spawn[1]) / map.height) * 100 + "%";
      markers.appendChild(spawn);
      status.hidden = true;
      applyPreviewFocus();
    }
    function resize() {
      if (state.mapPreview && state.mapPreview.id === mapId) draw(state.mapPreview);
    }
    if (typeof ResizeObserver !== "undefined") {
      if (!state.mapResizeObserver) state.mapResizeObserver = new ResizeObserver(resize);
      state.mapResizeObserver.disconnect();
      state.mapResizeObserver.observe(layer.parentElement);
    } else if (!state.mapResizeFallback) {
      window.addEventListener("resize", resize, { passive: true });
      state.mapResizeFallback = true;
    }
    if (state.mapPreview && state.mapPreview.id === mapId) {
      draw(state.mapPreview);
      return;
    }
    if (state.mapPreviewLoadingId === mapId) return;
    state.mapPreviewLoadingId = mapId;
    status.textContent = "Loading campaign map…";
    status.hidden = false;
    window.SOWCampaignMapPreview.load(mapId)
      .then(function (map) {
        if (!state.roster || state.roster.map !== mapId) return;
        state.mapPreview = map;
        draw(map);
      })
      .catch(function () {
        if (state.roster && state.roster.map === mapId) status.textContent = "Map preview unavailable";
      })
      .then(function () {
        if (state.mapPreviewLoadingId === mapId) state.mapPreviewLoadingId = null;
      });
  }
  function valid() {
    if (!state.definition) return { errors: [], warnings: [] };
    if (state.isMenuGuides) {
      syncMenuGuideSteps();
      return window.SOWCampaign.validateMenuGuides(state.definition);
    }
    if (!state.roster) return { errors: [], warnings: [] };
    return window.SOWCampaign.validate(state.definition, state.roster, {
      hasText: function (key) {
        return Boolean(catalogValue("en", key));
      },
      getText: function (key) {
        return textValue(key, "en") || null;
      },
      hasAvatar: function (avatar) {
        return state.avatars.includes(avatar);
      },
    });
  }
  function markDirty() {
    if (state.demoBackup) return;
    state.dirty = true;
    const status = $("#saveStatus");
    status.textContent = "Unsaved changes";
    status.className = "status dirty";
    if (state.renderer) {
      refresh(true);
      syncPreview();
      renderGraph();
    } else refresh();
  }
  function clearPreviewRevealTimer() {
    state.previewRevealToken++;
    if (state.previewRevealTimer !== null) window.clearTimeout(state.previewRevealTimer);
    state.previewRevealTimer = null;
  }
  function status(message, kind) {
    const node = $("#saveStatus");
    node.textContent = message;
    node.className = "status" + (kind ? " " + kind : "");
  }
  function notice(message) {
    const node = $("#notice");
    node.textContent = message;
    node.hidden = !message;
  }
  function setPath(object, path, value) {
    const parts = path.split(".");
    let target = object;
    for (let i = 0; i < parts.length - 1; i++) target = target[parts[i]];
    if (value == null) delete target[parts[parts.length - 1]];
    else target[parts[parts.length - 1]] = value;
  }
  function uniqueId(base, values) {
    let id = base,
      suffix = 2;
    while (
      values.some(function (value) {
        return value.id === id;
      })
    )
      id = base + "_" + suffix++;
    return id;
  }
  function reorderControls(items, index, label) {
    const controls = el("div", { class: "order-actions" });
    [
      { offset: -1, glyph: "↑", direction: "up" },
      { offset: 1, glyph: "↓", direction: "down" },
    ].forEach(function (move) {
      const button = el(
        "button",
        { type: "button", "aria-label": "Move " + label + " " + (index + 1) + " " + move.direction },
        move.glyph,
      );
      button.disabled = index + move.offset < 0 || index + move.offset >= items.length;
      button.addEventListener("click", function () {
        const item = items.splice(index, 1)[0];
        items.splice(index + move.offset, 0, item);
        markDirty();
        renderInspector();
        const nextLabel = "Move " + label + " " + (index + move.offset + 1) + " " + move.direction;
        let nextButton = Array.from($("#inspector").querySelectorAll(".order-actions button")).find(
          function (candidate) {
            return candidate.getAttribute("aria-label") === nextLabel;
          },
        );
        if (!nextButton || nextButton.disabled) {
          const otherDirection = move.direction === "up" ? "down" : "up";
          nextButton = Array.from($("#inspector").querySelectorAll(".order-actions button")).find(
            function (candidate) {
              return (
                candidate.getAttribute("aria-label") ===
                "Move " + label + " " + (index + move.offset + 1) + " " + otherDirection
              );
            },
          );
        }
        if (nextButton && !nextButton.disabled) nextButton.focus();
      });
      controls.appendChild(button);
    });
    return controls;
  }
  function inputField(label, value, onChange, options) {
    options = options || {};
    const wrapper = el("label", { class: options.class || "" });
    wrapper.appendChild(el("span", {}, label));
    const input = el(options.multiline ? "textarea" : "input", {
      type: options.type || "text",
      value: value == null ? "" : value,
      placeholder: options.placeholder || "",
    });
    if (options.min != null) input.min = options.min;
    if (options.max != null) input.max = options.max;
    if (options.step != null) input.step = options.step;
    if (options.multiline) input.value = value || "";
    input.addEventListener(options.input ? "input" : "change", function () {
      onChange(input.value);
    });
    wrapper.appendChild(input);
    return wrapper;
  }
  function selectField(label, current, choices, onChange) {
    const wrapper = el("label", {});
    wrapper.appendChild(el("span", {}, label));
    const select = el("select", {});
    choices.forEach(function (choice) {
      const value = typeof choice === "string" ? choice : choice.value;
      select.appendChild(el("option", { value: value }, typeof choice === "string" ? choice : choice.label));
    });
    select.value = current || "";
    select.addEventListener("change", function () {
      onChange(select.value);
    });
    wrapper.appendChild(select);
    return wrapper;
  }
  function multiFactionField(label, selected, onChange) {
    const wrapper = el("label", {});
    wrapper.appendChild(el("span", {}, label));
    const select = el("select", {
      multiple: "multiple",
      size: Math.min(8, Math.max(3, state.roster.factions.length)),
    });
    factionOptions().forEach(function (option) {
      const item = el("option", { value: option.value }, option.label);
      item.selected = selected.includes(option.value);
      select.appendChild(item);
    });
    select.addEventListener("change", function () {
      onChange(
        Array.from(select.selectedOptions).map(function (item) {
          return item.value;
        }),
      );
    });
    wrapper.appendChild(select);
    return wrapper;
  }
  function checkboxField(label, checked, onChange) {
    const wrapper = el("label", { class: "check" });
    const input = el("input", { type: "checkbox", checked: Boolean(checked) });
    input.addEventListener("change", function () {
      onChange(input.checked);
    });
    wrapper.append(input, el("span", {}, label));
    return wrapper;
  }
  function section(title) {
    const node = el("section", { class: "form-section" });
    node.appendChild(el("h3", {}, title));
    return node;
  }
  function renderExchangeLines(host, lines, ownerId, label) {
    lines.forEach(function (line, index) {
      const card = el("div", { class: "form-card" });
      const presentation = line.presentation || "emote";
      const delay = Number.isInteger(line.delay_ms) ? line.delay_ms : 250;
      card.appendChild(el("strong", {}, label + " " + (index + 1)));
      card.appendChild(
        selectField("Speaker", line.speaker, speakerOptions(), function (value) {
          line.speaker = value;
          markDirty();
        }),
      );
      card.appendChild(
        selectField(
          "Presentation",
          presentation,
          [
            { value: "emote", label: "Short emote bubble" },
            { value: "dialogue", label: "Dialogue modal" },
          ],
          function (value) {
            if (value === "dialogue") {
              line.presentation = "dialogue";
              delete line.emote;
              delete line.emote_text_key;
              line.body_key = line.body_key || storyKey(ownerId + "_line_" + (index + 1) + "_body");
            } else {
              line.presentation = "emote";
              delete line.title_key;
              delete line.show_title;
              delete line.body_key;
              line.emote = line.emote || "threat";
              line.emote_text_key =
                line.emote_text_key || storyKey(ownerId + "_line_" + (index + 1) + "_emote");
            }
            markDirty();
            renderInspector();
          },
        ),
      );
      if (presentation === "dialogue") {
        line.body_key = line.body_key || storyKey(ownerId + "_line_" + (index + 1) + "_body");
        card.appendChild(
          inputField(
            "Title text key (optional)",
            line.title_key || "",
            function (value) {
              line.title_key = value.trim();
              if (!line.title_key) delete line.title_key;
              markDirty();
            },
            {},
          ),
        );
        card.appendChild(
          inputField(
            "Dialogue text key",
            line.body_key,
            function (value) {
              line.body_key = value.trim();
              markDirty();
            },
            {},
          ),
        );
        catalogTextField(card, "Dialogue", line.body_key, 4);
      } else {
        line.emote_text_key = line.emote_text_key || storyKey(ownerId + "_line_" + (index + 1) + "_emote");
        card.appendChild(
          selectField(
            "Emote",
            line.emote || "threat",
            ["greet", "help", "threat", "celebrate"],
            function (value) {
              line.emote = value;
              markDirty();
            },
          ),
        );
        card.appendChild(
          inputField(
            "Bubble text key",
            line.emote_text_key,
            function (value) {
              line.emote_text_key = value.trim();
              markDirty();
            },
            {},
          ),
        );
        catalogTextField(card, "Bubble text", line.emote_text_key, 2);
      }
      card.appendChild(
        inputField(
          "Pause after line (ms)",
          delay,
          function (value) {
            const milliseconds = Number(value);
            if (Number.isInteger(milliseconds) && milliseconds >= 0 && milliseconds <= 10000)
              line.delay_ms = milliseconds;
            markDirty();
          },
          { type: "number", min: 0, max: 10000, step: 50 },
        ),
      );
      card.appendChild(reorderControls(lines, index, label.toLowerCase()));
      if (lines.length > 1) {
        const remove = el("button", { type: "button" }, "Remove line");
        remove.addEventListener("click", function () {
          lines.splice(index, 1);
          markDirty();
          renderInspector();
        });
        card.appendChild(remove);
      }
      host.appendChild(card);
    });
  }
  function renderExchangeFields(host, step) {
    host.appendChild(
      checkboxField("Timed emote exchange", Boolean(step.exchange), function (enabled) {
        if (enabled) {
          step.exchange = [
            {
              speaker: step.speaker || Object.keys(state.definition.speakers || {})[0],
              presentation: "emote",
              emote: "threat",
              emote_text_key: storyKey(step.id + "_line_1_emote"),
              delay_ms: window.SOWCampaign.EXCHANGE_DEFAULT_REPLY_DELAY_MS,
            },
          ];
          if (step.type === "scene")
            [
              "title_key",
              "body_key",
              "lines",
              "presentation",
              "show_title",
              "wait_before",
              "video_src",
            ].forEach(function (key) {
              delete step[key];
            });
          delete step.emote;
          delete step.emote_text_key;
          delete step.emote_pinned;
        } else {
          delete step.exchange;
        }
        markDirty();
        renderInspector();
      }),
    );
    if (step.exchange) renderExchangeLines(host, step.exchange, step.id, "Line");
    if (step.exchange && step.exchange.length < 16) {
      const add = el("button", { type: "button" }, "Add exchange line");
      add.addEventListener("click", function () {
        step.exchange.push({
          speaker: step.speaker || Object.keys(state.definition.speakers || {})[0],
          presentation: "emote",
          emote: "threat",
          emote_text_key: storyKey(step.id + "_line_" + (step.exchange.length + 1) + "_emote"),
          delay_ms: window.SOWCampaign.EXCHANGE_DEFAULT_REPLY_DELAY_MS,
        });
        markDirty();
        renderInspector();
      });
      host.appendChild(add);
    }
    host.appendChild(
      selectField(
        "If conversation partner is unavailable",
        step.unavailable_next || "",
        [{ value: "", label: "Continue normally" }].concat(destinationOptions(step.id)),
        function (value) {
          if (value) step.unavailable_next = value;
          else delete step.unavailable_next;
          markDirty();
        },
      ),
    );
    if (step.campaign_accept_alliance_on_enter)
      host.appendChild(
        selectField(
          "Faction accepting a pending player request",
          step.campaign_accept_alliance_on_enter.faction_id,
          factionOptions(),
          function (value) {
            step.campaign_accept_alliance_on_enter.faction_id = value;
            markDirty();
          },
        ),
      );
  }
  function storyKey(suffix) {
    return (state.definition.text_namespace || "tutorial.") + suffix;
  }
  function stepTextKey(step, suffix) {
    return (state.definition.text_namespace || "tutorial.") + step.id + suffix;
  }
  function catalogTextField(container, label, key, rows) {
    const field = el("textarea", {
      "aria-label": label,
      readonly: "",
      placeholder: "Add this key to sow-i18n/strings/en/web.toml.",
    });
    field.rows = rows || 4;
    field.value = textValue(key, state.previewLanguage);
    container.appendChild(el("label", {}, label + " · " + state.previewLanguage));
    container.appendChild(field);
    const note = field.value
      ? "Text is managed by the shared language catalogs."
      : "Missing shared English text. Add this key to sow-i18n/strings/en/web.toml.";
    container.appendChild(el("small", { class: "translation-fallback", role: "status" }, note));
  }
  function textEditor(container, label, keyPath, valuePath) {
    const step = state.definition.steps.find(function (item) {
      return item.id === state.selected;
    });
    let key = valuePath
      ? valuePath.split(".").reduce(function (value, part) {
          return value && value[part];
        }, step)
      : step[keyPath];
    if (!key) {
      let suffix = keyPath === "title_key" ? "_title" : keyPath === "hint_key" ? "_hint" : "_body";
      if (valuePath) {
        const parts = valuePath.split("."),
          index = Number(parts[1]);
        if (parts[0] === "choices" && step.choices[index])
          suffix = "_choice_" + step.choices[index].id + (keyPath === "label_key" ? "_label" : "_detail");
        else if (parts[0] === "lines" && step.lines[index]) suffix = "_line_" + (index + 1) + "_body";
      }
      key = storyKey(step.id + suffix);
      if (valuePath) setPath(step, valuePath, key);
      else step[keyPath] = key;
      markDirty();
    }
    const box = el("div", { class: "text-key" });
    box.appendChild(el("code", { class: "key-field" }, key));
    catalogTextField(box, label, key, ["Title", "Choice label"].includes(label) ? 3 : 6);
    container.appendChild(box);
  }
  function menuTextEditor(container, label, field) {
    const step = state.definition.steps.find(function (item) {
      return item.id === state.selected;
    });
    let key = step[field];
    if (!key) {
      key =
        "menu.guide." +
        step.id +
        (field === "title_key" ? ".title" : field === "hint_key" ? ".hint" : ".body");
      step[field] = key;
      markDirty();
    }
    const sectionNode = section(label);
    sectionNode.appendChild(el("code", { class: "key-field" }, key));
    ["en", "es"].forEach(function (locale) {
      const input = el("textarea", { "aria-label": label + " · " + locale });
      input.rows = 3;
      const strings = state.definition.strings[locale] || (state.definition.strings[locale] = {});
      input.value = strings[key] || "";
      input.addEventListener("input", function () {
        strings[key] = input.value;
        markDirty();
      });
      sectionNode.appendChild(el("label", {}, label + " · " + locale));
      sectionNode.appendChild(input);
    });
    container.appendChild(sectionNode);
  }
  function menuGuideOutcomeOptions() {
    return Object.keys(window.SOWCampaign.MENU_GUIDE_OUTCOMES).map(function (outcome) {
      const control = window.SOWCampaign.MENU_GUIDE_OUTCOMES[outcome].control;
      return { value: outcome, label: outcome.replace(/_/g, " ") + " · " + control.replace(/_/g, " ") };
    });
  }
  function menuGuideTriggerOutcomes(trigger) {
    return Array.isArray(trigger && trigger.outcomes)
      ? trigger.outcomes.slice()
      : trigger && trigger.outcome
        ? [trigger.outcome]
        : [];
  }
  function setMenuGuideTrigger(step, primary, additional) {
    const registered = window.SOWCampaign.MENU_GUIDE_OUTCOMES[primary];
    if (!registered) return;
    const outcomes = [primary];
    if (
      additional &&
      additional !== primary &&
      window.SOWCampaign.MENU_GUIDE_OUTCOMES[additional] &&
      window.SOWCampaign.MENU_GUIDE_OUTCOMES[additional].kind === "counter"
    )
      outcomes.push(additional);
    const requirePurchased = Boolean(
      step.trigger && step.trigger.require_purchased_leader && primary === "leader_selected",
    );
    step.trigger = { type: "ui", action: registered.control, scope: "step" };
    if (outcomes.length > 1) step.trigger.outcomes = outcomes;
    else step.trigger.outcome = primary;
    if (requirePurchased) step.trigger.require_purchased_leader = true;
    step.guide = { kind: "ui", target: registered.control, gesture: "tap" };
  }
  function renderMenuGuideInspector(host, step) {
    host.appendChild(el("div", { class: "step-validation", role: "status", hidden: true }));
    updateStepValidation();
    const basics = section("Menu guide step");
    basics.appendChild(
      inputField("Stable ID", step.id, function (value) {
        value = value
          .trim()
          .toLowerCase()
          .replace(/[^a-z0-9_-]+/g, "_");
        if (
          !value ||
          state.definition.steps.some(function (item) {
            return item !== step && item.id === value;
          })
        )
          return;
        const old = step.id;
        step.id = value;
        Object.keys(state.definition.menu_guides).forEach(function (id) {
          if (state.definition.menu_guides[id].entry === old) state.definition.menu_guides[id].entry = value;
        });
        state.definition.steps.forEach(function (item) {
          if (item.next === old) item.next = value;
          if (item.unavailable_next === old) item.unavailable_next = value;
          (item.routes || []).forEach(function (route) {
            if (route.next === old) route.next = value;
          });
          (item.choices || []).forEach(function (choice) {
            if (choice.next === old) choice.next = value;
          });
        });
        if (state.definition.layout[old]) {
          state.definition.layout[value] = state.definition.layout[old];
          delete state.definition.layout[old];
        }
        state.selected = value;
        markDirty();
        renderInspector();
      }),
    );
    basics.appendChild(
      selectField("Step kind", step.type, ["guide", "scene", "choice", "end"], function (type) {
        if (type === step.type) return;
        const old = Object.assign({}, step),
          next =
            old.next ||
            state.definition.steps.find(function (item) {
              return item.type === "end";
            })?.id ||
            "";
        Object.keys(step).forEach(function (key) {
          delete step[key];
        });
        step.id = old.id;
        step.type = type;
        step.title_key = old.title_key || "menu.guide." + step.id + ".title";
        if (type === "guide") {
          const outcome = menuGuideTriggerOutcomes(old.trigger)[0] || "name_saved";
          const control = window.SOWCampaign.MENU_GUIDE_OUTCOMES[outcome].control;
          step.hint_key = old.hint_key || "menu.guide." + step.id + ".hint";
          step.trigger = { type: "ui", action: control, outcome: outcome, scope: "step" };
          step.guide = { kind: "ui", target: control, gesture: "tap" };
          step.next = next;
        } else if (type === "scene") {
          step.body_key = old.body_key || "menu.guide." + step.id + ".body";
          step.next = next;
        } else if (type === "choice") {
          step.choices = [
            { id: "first", label_key: "menu.guide." + step.id + ".first", next: next },
            { id: "second", label_key: "menu.guide." + step.id + ".second", next: next },
          ];
        } else if (old.body_key) step.body_key = old.body_key;
        markDirty();
        renderInspector();
      }),
    );
    menuTextEditor(basics, "Title", "title_key");
    if (step.type === "guide") {
      const triggerOutcomes = menuGuideTriggerOutcomes(step.trigger),
        primaryOutcome = triggerOutcomes[0] || "";
      basics.appendChild(
        selectField("Confirmed action", primaryOutcome, menuGuideOutcomeOptions(), function (outcome) {
          setMenuGuideTrigger(step, outcome, triggerOutcomes[1]);
          markDirty();
          renderInspector();
        }),
      );
      const additionalOptions = Object.keys(window.SOWCampaign.MENU_GUIDE_OUTCOMES)
        .filter(function (outcome) {
          return (
            outcome !== primaryOutcome && window.SOWCampaign.MENU_GUIDE_OUTCOMES[outcome].kind === "counter"
          );
        })
        .map(function (outcome) {
          return { value: outcome, label: outcome.replace(/_/g, " ") };
        });
      basics.appendChild(
        selectField(
          "Also accept action",
          triggerOutcomes[1] || "",
          [{ value: "", label: "No alternate action" }].concat(additionalOptions),
          function (outcome) {
            setMenuGuideTrigger(step, primaryOutcome, outcome);
            markDirty();
            renderInspector();
          },
        ),
      );
      if (primaryOutcome === "leader_selected")
        basics.appendChild(
          checkboxField(
            "Require the selected purchased hero to be equipped",
            Boolean(step.trigger.require_purchased_leader),
            function (enabled) {
              step.trigger.require_purchased_leader = enabled;
              if (!enabled) delete step.trigger.require_purchased_leader;
              markDirty();
              renderInspector();
            },
          ),
        );
      menuTextEditor(basics, "Hand label", "hint_key");
    } else if (step.type === "scene" || step.type === "end")
      menuTextEditor(basics, step.type === "end" ? "Closing text" : "Story text", "body_key");
    host.appendChild(basics);
    if (step.type === "choice") {
      const answers = section("Answers");
      (step.choices || []).forEach(function (choice, index) {
        answers.appendChild(el("strong", {}, "Answer " + (index + 1)));
        answers.appendChild(
          inputField("Answer ID", choice.id, function (value) {
            choice.id = value
              .trim()
              .toLowerCase()
              .replace(/[^a-z0-9_-]+/g, "_");
            markDirty();
            renderInspector();
          }),
        );
        const labelKey = choice.label_key || "menu.guide." + step.id + "." + choice.id;
        choice.label_key = labelKey;
        ["en", "es"].forEach(function (locale) {
          const input = el("textarea", { "aria-label": "Answer " + locale });
          input.rows = 2;
          const strings = state.definition.strings[locale] || (state.definition.strings[locale] = {});
          input.value = strings[labelKey] || "";
          input.addEventListener("input", function () {
            strings[labelKey] = input.value;
            markDirty();
          });
          answers.appendChild(el("label", {}, "Answer · " + locale));
          answers.appendChild(input);
        });
        answers.appendChild(
          selectField("Next step", choice.next, destinationOptions(step.id), function (value) {
            choice.next = value;
            markDirty();
          }),
        );
        if (step.choices.length > 2) {
          const remove = el("button", { type: "button", class: "danger" }, "Remove answer");
          remove.addEventListener("click", function () {
            step.choices.splice(index, 1);
            markDirty();
            renderInspector();
          });
          answers.appendChild(remove);
        }
      });
      if (step.choices.length < 4) {
        const addAnswer = el("button", { type: "button" }, "Add answer");
        addAnswer.addEventListener("click", function () {
          const id = uniqueId("answer", step.choices);
          step.choices.push({
            id: id,
            label_key: "menu.guide." + step.id + "." + id,
            next: step.choices[0].next || "",
          });
          markDirty();
          renderInspector();
        });
        answers.appendChild(addAnswer);
      }
      host.appendChild(answers);
    }
    if (step.type !== "end" && step.type !== "choice")
      host.appendChild(
        selectField("Next step", step.next, destinationOptions(step.id), function (value) {
          step.next = value;
          markDirty();
        }),
      );
    if (step.type === "guide" || step.type === "scene") {
      const routes = section("Conditional paths");
      (step.routes || []).forEach(function (route, index) {
        const routeLabels = {
          commerce_available: "The store is available on this platform",
          commerce_offer_available: "An affordable gem hero is available",
          leader_purchase_modal_open: "The gem purchase window is open",
          leader_purchase_confirmed: "The selected gem hero purchase is confirmed",
        };
        const routeOptions = window.SOWCampaign.MENU_GUIDE_ROUTE_FACTS.map(function (fact) {
          return { value: fact, label: routeLabels[fact] || fact.replace(/_/g, " ") };
        });
        routes.appendChild(
          selectField("When", (route.when && route.when.fact) || "", routeOptions, function (fact) {
            route.when.fact = fact;
            markDirty();
          }),
        );
        routes.appendChild(
          inputField(
            "At least",
            (route.when && route.when.gte) || 1,
            function (value) {
              route.when.gte = Number(value);
              markDirty();
            },
            { type: "number", min: 1, step: 1 },
          ),
        );
        routes.appendChild(
          selectField("Go to", route.next, destinationOptions(step.id), function (value) {
            route.next = value;
            markDirty();
          }),
        );
        const removeRoute = el("button", { type: "button", class: "danger" }, "Remove path");
        removeRoute.addEventListener("click", function () {
          step.routes.splice(index, 1);
          markDirty();
          renderInspector();
        });
        routes.appendChild(removeRoute);
      });
      const addRoute = el("button", { type: "button" }, "Add conditional path");
      addRoute.disabled = (step.routes || []).length >= window.SOWCampaign.MENU_GUIDE_ROUTE_FACTS.length;
      addRoute.addEventListener("click", function () {
        step.routes = step.routes || [];
        const used = new Set(
          step.routes.map(function (route) {
            return route.when && route.when.fact;
          }),
        );
        const fact = window.SOWCampaign.MENU_GUIDE_ROUTE_FACTS.find(function (candidate) {
          return !used.has(candidate);
        });
        if (!fact) return;
        step.routes.push({ when: { fact: fact, gte: 1 }, next: step.next || "" });
        markDirty();
        renderInspector();
      });
      routes.appendChild(addRoute);
      host.appendChild(routes);
    }
    const flow = section("Journey");
    if (state.definition.menu_guides[state.menuGuideId].entry !== step.id) {
      const setOpening = el("button", { type: "button" }, "Use as journey opening");
      setOpening.addEventListener("click", function () {
        state.definition.menu_guides[state.menuGuideId].entry = step.id;
        markDirty();
        renderInspector();
        renderGraph();
        resetPreview();
      });
      flow.appendChild(setOpening);
    } else flow.appendChild(el("small", {}, "This step opens the selected journey."));
    if (
      !Object.keys(state.definition.menu_guides).some(function (id) {
        return state.definition.menu_guides[id].entry === step.id;
      })
    ) {
      const removeStep = el("button", { type: "button", class: "danger" }, "Remove step");
      removeStep.addEventListener("click", function () {
        if (!confirm("Remove this step and reconnect its incoming paths?")) return;
        const fallback = step.type === "end" ? "" : step.next || "";
        state.definition.steps.forEach(function (item) {
          if (item.next === step.id) item.next = fallback;
          (item.routes || []).forEach(function (route) {
            if (route.next === step.id) route.next = fallback;
          });
          (item.choices || []).forEach(function (choice) {
            if (choice.next === step.id) choice.next = fallback;
          });
        });
        state.definition.steps = state.definition.steps.filter(function (item) {
          return item !== step;
        });
        delete state.definition.layout[step.id];
        state.selected = flowEntry();
        markDirty();
        renderPalette();
        renderInspector();
        renderGraph();
        resetPreview();
      });
      flow.appendChild(removeStep);
    }
    host.appendChild(flow);
  }
  function renderInspector() {
    const host = $("#inspector");
    host.replaceChildren();
    const step = state.definition.steps.find(function (item) {
      return item.id === state.selected;
    });
    if (!step) {
      setFactionPicker(false);
      host.appendChild(el("p", { class: "empty-state" }, "Select a step in the mission flow."));
      return;
    }
    if (state.isMenuGuides) {
      renderMenuGuideInspector(host, step);
      return;
    }
    setFactionPicker(state.pickingFaction);
    host.appendChild(el("div", { class: "step-validation", role: "status", hidden: true }));
    updateStepValidation();
    const basics = section("Step");
    basics.appendChild(
      inputField("Stable ID", step.id, function (value) {
        value = value
          .trim()
          .toLowerCase()
          .replace(/[^a-z0-9_-]+/g, "_");
        if (
          !value ||
          state.definition.steps.some(function (item) {
            return item !== step && item.id === value;
          })
        )
          return;
        const old = step.id;
        step.id = value;
        if (state.definition.entry === old) state.definition.entry = value;
        if (state.definition.menu_guide && state.definition.menu_guide.entry === old)
          state.definition.menu_guide.entry = value;
        if (state.definition.layout[old]) {
          state.definition.layout[value] = state.definition.layout[old];
          delete state.definition.layout[old];
        }
        state.definition.steps.forEach(function (item) {
          if (item.next === old) item.next = value;
          if (item.unavailable_next === old) item.unavailable_next = value;
          (item.choices || []).forEach(function (choice) {
            if (choice.next === old) choice.next = value;
          });
          (item.routes || []).forEach(function (route) {
            if (route.next === old) route.next = value;
          });
          (item.routes || []).forEach(function (route) {
            if (route.when && route.when.choice === old) route.when.choice = value;
          });
        });
        state.selected = value;
        markDirty();
        renderInspector();
      }),
    );
    basics.appendChild(
      selectField("Step kind", step.type, types, function (value) {
        if (value === step.type) return;
        const id = step.id,
          xy = state.definition.layout[id];
        const referencedChoice =
          step.type === "choice" &&
          state.definition.steps.some(function (item) {
            return (
              item &&
              Array.isArray(item.routes) &&
              item.routes.some(function (route) {
                return route && route.when && route.when.choice === id;
              })
            );
          });
        if (referencedChoice) {
          notice("Remove routes that depend on this decision before changing its type.");
          renderInspector();
          return;
        }
        const sharedChoiceTarget =
          step.type === "choice" &&
          step.choices &&
          step.choices.length &&
          step.choices.every(function (choice) {
            return choice.next === step.choices[0].next;
          })
            ? step.choices[0].next
            : "";
        const divergentChoiceTargets =
          step.type === "choice" &&
          step.choices &&
          step.choices.some(function (choice) {
            return choice.next !== sharedChoiceTarget;
          });
        let next = step.type === "choice" ? sharedChoiceTarget : step.next || "";
        const keepsFlow = ["scene", "objective", "guide"].includes(value);
        const keepsMechanics = ["objective", "guide"].includes(value);
        const keepsExchange = ["scene", "objective", "guide"].includes(value);
        const keepsMarker = ["scene", "choice", "objective", "guide"].includes(value);
        const discarded = [];
        if (step.lines && value !== "scene") discarded.push("dialogue lines");
        if (step.exchange && !keepsExchange) discarded.push("timed emote exchange");
        if (step.unavailable_next && !keepsFlow) discarded.push("partner-unavailable fallback");
        if (step.campaign_accept_alliance_on_enter && !keepsMechanics)
          discarded.push("alliance response action");
        if (step.choices && value !== "choice") discarded.push("decision answers");
        if (step.trigger && (!keepsMechanics || (value === "guide" && step.trigger.type === "elapsed")))
          discarded.push("mechanic condition");
        if (
          step.guide &&
          (!keepsMechanics || (value === "guide" && step.trigger && step.trigger.type === "elapsed"))
        )
          discarded.push("hand guide");
        if (step.marker && !keepsMarker) discarded.push("map marker");
        if (step.wait_before === true && !["scene", "choice"].includes(value))
          discarded.push("1.1-second reveal wait");
        if (step.paused_action && !keepsMechanics) discarded.push("paused action permission");
        if (step.campaign_assault_on_enter && value === "end") discarded.push("campaign assault action");
        if (step.routes && !keepsFlow) discarded.push("conditional routes");
        if (step.hint_key && !keepsMechanics) discarded.push("player hint");
        if (step.presentation && !["scene", "end"].includes(value)) discarded.push("presentation style");
        if (step.video_src && value !== "scene") discarded.push("cinematic video path");
        if (step.next && !keepsFlow && value !== "choice") discarded.push("outgoing connection");
        if (divergentChoiceTargets && value !== "choice") discarded.push("branch destinations");
        if (
          discarded.length &&
          !confirm(
            "Changing this step to “" + value + "” removes its " + discarded.join(", ") + ". Continue?",
          )
        ) {
          renderInspector();
          return;
        }
        const ending = state.definition.steps.find(function (item) {
          return item !== step && item.type === "end";
        });
        if (!next && !divergentChoiceTargets && value !== "end" && value !== "choice" && ending)
          next = ending.id;
        const menuAction = inGameUiTargets()[0] || "menu_campaign";
        const firstLine = step.lines && step.lines[0];
        const exchangeScene = value === "scene" && Boolean(step.exchange);
        const replacement = { id: id, type: value };
        if (!exchangeScene) replacement.title_key = step.title_key || storyKey(id + "_title");
        const bodyKey =
          step.body_key || (firstLine && firstLine.body_key) || (value === "scene" && step.hint_key);
        if (bodyKey && !exchangeScene) replacement.body_key = bodyKey;
        if (step.speaker || (firstLine && firstLine.speaker))
          replacement.speaker = step.speaker || firstLine.speaker;
        if (Number.isFinite(step.attack_ratio_on_enter))
          replacement.attack_ratio_on_enter = step.attack_ratio_on_enter;
        if (step.campaign_assault_on_enter && value !== "end")
          replacement.campaign_assault_on_enter = step.campaign_assault_on_enter;
        if (["scene", "choice"].includes(value) && step.wait_before === true) replacement.wait_before = true;
        if (keepsMechanics && step.pause_game === true) replacement.pause_game = true;
        if (keepsMechanics && step.paused_action) replacement.paused_action = step.paused_action;
        if (["scene", "end"].includes(value) && !exchangeScene)
          replacement.presentation =
            value === "end" && step.presentation === "cinematic"
              ? "chapter"
              : step.presentation || "dialogue";
        if (keepsExchange && step.exchange) replacement.exchange = step.exchange;
        if (keepsFlow && step.unavailable_next) replacement.unavailable_next = step.unavailable_next;
        if (keepsMechanics && step.campaign_accept_alliance_on_enter)
          replacement.campaign_accept_alliance_on_enter = step.campaign_accept_alliance_on_enter;
        if (value === "scene") {
          if (step.video_src) replacement.video_src = step.video_src;
          if (Array.isArray(step.lines)) replacement.lines = step.lines;
          if (next) replacement.next = next;
          if (Array.isArray(step.routes)) replacement.routes = step.routes;
        }
        if (value === "choice")
          replacement.choices =
            step.type === "choice" && Array.isArray(step.choices)
              ? step.choices
              : [
                  { id: "first_answer", label_key: storyKey(id + "_first"), next: step.next || "" },
                  { id: "second_answer", label_key: storyKey(id + "_second"), next: step.next || "" },
                ];
        if (value === "objective" || value === "guide") {
          if (step.hint_key) replacement.hint_key = step.hint_key;
          replacement.trigger =
            step.trigger && !(value === "guide" && step.trigger.type === "elapsed")
              ? step.trigger
              : state.flow === "menu"
                ? { type: "ui", action: menuAction, scope: "step" }
                : { type: "territory", value: 1, scope: "step" };
          if (Array.isArray(step.routes)) replacement.routes = step.routes;
          if (step.guide && !(value === "guide" && step.trigger && step.trigger.type === "elapsed"))
            replacement.guide = step.guide;
          else if (value === "guide")
            replacement.guide =
              replacement.trigger.type === "ui"
                ? { kind: "ui", target: replacement.trigger.action, gesture: "tap" }
                : { kind: "world", target: "expand", gesture: "tap" };
          if (next) replacement.next = next;
        }
        if (step.marker && keepsMarker) replacement.marker = step.marker;
        Object.keys(step).forEach(function (key) {
          delete step[key];
        });
        Object.assign(step, replacement);
        if (xy) state.definition.layout[id] = xy;
        notice("");
        markDirty();
        renderInspector();
      }),
    );
    basics.appendChild(
      selectField("Speaker", step.speaker || "", speakerOptions(), function (value) {
        step.speaker = value || undefined;
        if (!value) delete step.speaker;
        markDirty();
      }),
    );
    if (!step.exchange && (step.type === "scene" || step.type === "end"))
      basics.appendChild(
        selectField(
          "Presentation",
          step.presentation || "dialogue",
          step.type === "scene"
            ? ["dialogue", "chapter", "celebration", "cinematic"]
            : ["dialogue", "chapter", "celebration"],
          function (value) {
            step.presentation = value;
            markDirty();
          },
        ),
      );
    if (!step.exchange && (step.type === "scene" || step.type === "choice"))
      basics.appendChild(
        checkboxField("Wait 1.1 seconds before showing", step.wait_before === true, function (value) {
          if (value) step.wait_before = true;
          else delete step.wait_before;
          markDirty();
        }),
      );
    if (step.type === "scene" && !step.exchange) {
      basics.appendChild(
        inputField(
          "Local cinematic video",
          step.video_src || "",
          function (value) {
            const source = value.trim();
            if (source) {
              step.video_src = source;
              step.presentation = "cinematic";
            } else delete step.video_src;
            markDirty();
          },
          { placeholder: "/assets/campaign/" + state.definition.episode_id + "/opening.webm" },
        ),
      );
      basics.appendChild(
        el(
          "small",
          { class: "translation-fallback" },
          "Optional MP4/WebM; place the file in this episode's assets/campaign folder. The shared preview plays it without autoplay.",
        ),
      );
    }
    basics.appendChild(
      inputField(
        step.type === "objective" || step.type === "guide"
          ? "Strike troop percentage on entry (%)"
          : "Set send percentage on entry (%)",
        step.attack_ratio_on_enter == null ? "" : Math.round(step.attack_ratio_on_enter * 100),
        function (value) {
          if (value.trim() === "") delete step.attack_ratio_on_enter;
          else step.attack_ratio_on_enter = Number(value) / 100;
          markDirty();
        },
        { type: "number", min: 5, max: 100, step: 1, placeholder: "No change" },
      ),
    );
    if (!step.exchange || step.type !== "scene") textEditor(basics, "Title", "title_key");
    if (["scene", "objective", "guide"].includes(step.type)) renderExchangeFields(basics, step);
    if (
      !step.exchange &&
      (step.body_key || step.type === "choice" || (step.type === "scene" && !Array.isArray(step.lines)))
    )
      textEditor(basics, step.type === "end" ? "Closing text" : "Story text", "body_key");
    else if (step.type === "end") {
      const addEnding = el("button", { type: "button" }, "Add closing text");
      addEnding.addEventListener("click", function () {
        step.body_key = storyKey(step.id + "_body");
        markDirty();
        renderInspector();
      });
      basics.appendChild(addEnding);
    }
    if (step.type === "objective" || step.type === "guide") textEditor(basics, "Player hint", "hint_key");
    if (step.type === "scene" && Array.isArray(step.lines)) {
      step.lines.forEach(function (line, index) {
        const lineBox = el("div", { class: "form-card" });
        const lineHeading = el("div", { class: "card-title" });
        lineHeading.appendChild(el("strong", {}, "Dialogue line " + (index + 1)));
        const lineActions = reorderControls(step.lines, index, "dialogue line");
        const removeLine = el(
          "button",
          { type: "button" },
          step.lines.length > 1 ? "Remove" : "Use one paragraph",
        );
        removeLine.addEventListener("click", function () {
          if (step.lines.length > 1) step.lines.splice(index, 1);
          else {
            step.body_key = line.body_key;
            delete step.lines;
          }
          markDirty();
          renderInspector();
        });
        lineActions.appendChild(removeLine);
        lineHeading.appendChild(lineActions);
        lineBox.appendChild(lineHeading);
        lineBox.appendChild(
          selectField("Speaker", line.speaker || step.speaker || "", speakerOptions(), function (value) {
            line.speaker = value || undefined;
            if (!value) delete line.speaker;
            markDirty();
          }),
        );
        textEditor(lineBox, "Line text", "body_key", "lines." + index + ".body_key");
        basics.appendChild(lineBox);
      });
      basics
        .appendChild(el("button", { type: "button" }, "Add dialogue line"))
        .addEventListener("click", function (event) {
          event.preventDefault();
          step.lines.push({
            speaker: step.speaker,
            body_key: storyKey(step.id + "_line_" + (step.lines.length + 1)),
          });
          markDirty();
          renderInspector();
        });
    } else if (step.type === "scene" && !step.exchange) {
      const addConversation = el("button", { type: "button" }, "Split into dialogue lines");
      addConversation.addEventListener("click", function () {
        step.lines = [{ speaker: step.speaker, body_key: step.body_key || storyKey(step.id + "_line_1") }];
        delete step.body_key;
        markDirty();
        renderInspector();
      });
      basics.appendChild(addConversation);
    }
    host.appendChild(basics);

    if (state.flow !== "menu") {
      const unlockSection = section("Campaign availability");
      const grants = step.unlocks || { buildings: {}, actions: [] };
      const levelOptions = [{ value: "0", label: "Locked" }];
      Object.keys(window.SOWCampaign.STRUCTURE_LEVEL_LIMITS).forEach(function (kind) {
        const maxLevel = window.SOWCampaign.STRUCTURE_LEVEL_LIMITS[kind];
        const choices = levelOptions.concat(
          Array.from({ length: maxLevel }, function (_, index) {
            const level = index + 1;
            return { value: String(level), label: "Level " + level };
          }),
        );
        unlockSection.appendChild(
          selectField(
            kind + " maximum level",
            String(Number(grants.buildings && grants.buildings[kind]) || 0),
            choices,
            function (value) {
              step.unlocks = step.unlocks || { buildings: {}, actions: [] };
              step.unlocks.buildings = step.unlocks.buildings || {};
              if (Number(value)) step.unlocks.buildings[kind] = Number(value);
              else delete step.unlocks.buildings[kind];
              if (!Object.keys(step.unlocks.buildings).length && !(step.unlocks.actions || []).length)
                delete step.unlocks;
              markDirty();
              renderInspector();
            },
          ),
        );
      });
      const actionNames = {
        transport_fleet: "Transport fleets",
        trade_ship: "Trade ships",
        warship: "Warships",
        nuke: "Nukes",
      };
      window.SOWCampaign.CAMPAIGN_ACTIONS.forEach(function (action) {
        unlockSection.appendChild(
          checkboxField(actionNames[action], (grants.actions || []).includes(action), function (enabled) {
            step.unlocks = step.unlocks || { buildings: {}, actions: [] };
            const actions = new Set(step.unlocks.actions || []);
            if (enabled) actions.add(action);
            else actions.delete(action);
            step.unlocks.actions = window.SOWCampaign.CAMPAIGN_ACTIONS.filter(function (item) {
              return actions.has(item);
            });
            if (!Object.keys(step.unlocks.buildings || {}).length && !step.unlocks.actions.length)
              delete step.unlocks;
            markDirty();
            renderInspector();
          }),
        );
      });
      const policies = campaignStepPolicies(state.machine && state.machine.state.choices, state.facts);
      const effective = window.SOWCampaign.applyCampaignUnlocks(policies.get(step.id), step.unlocks);
      const available = campaignUnlockLabel(effective);
      unlockSection.appendChild(
        el(
          "small",
          { class: "muted" },
          available
            ? "Available when this step begins: " + available
            : "Nothing is available yet on this route.",
        ),
      );
      host.appendChild(unlockSection);
    }

    if (step.type !== "end") {
      const campaignActions = section("Campaign actions");
      campaignActions.appendChild(
        checkboxField(
          "Change an enemy's attack behavior when this step begins",
          Boolean(step.campaign_attack_on_enter),
          function (enabled) {
            if (enabled)
              step.campaign_attack_on_enter = step.campaign_attack_on_enter || {
                faction_id: (enemyFactionOptions()[0] && enemyFactionOptions()[0].value) || "",
                can_attack_player: true,
              };
            else delete step.campaign_attack_on_enter;
            markDirty();
            renderInspector();
          },
        ),
      );
      if (step.campaign_attack_on_enter) {
        const attackPermission = step.campaign_attack_on_enter;
        campaignActions.appendChild(
          selectField(
            "Enemy faction",
            attackPermission.faction_id || "",
            enemyFactionOptions(),
            function (value) {
              attackPermission.faction_id = value;
              markDirty();
            },
          ),
        );
        campaignActions.appendChild(
          checkboxField(
            "Allow autonomous attacks against player",
            attackPermission.can_attack_player === true,
            function (enabled) {
              attackPermission.can_attack_player = enabled;
              markDirty();
            },
          ),
        );
      }
      campaignActions.appendChild(
        checkboxField(
          "Order a team to attack when this step begins",
          Boolean(step.campaign_assault_on_enter),
          function (enabled) {
            if (enabled)
              step.campaign_assault_on_enter = step.campaign_assault_on_enter || {
                attacker_team: "Red",
                target: "player",
              };
            else delete step.campaign_assault_on_enter;
            markDirty();
            renderInspector();
          },
        ),
      );
      if (step.campaign_assault_on_enter) {
        const assault = step.campaign_assault_on_enter;
        campaignActions.appendChild(
          selectField(
            "Attacking team",
            assault.attacker_team || "Red",
            window.SOWCampaign.TEAMS,
            function (value) {
              assault.attacker_team = value;
              markDirty();
            },
          ),
        );
        campaignActions.appendChild(
          selectField(
            "Target",
            assault.target || "player",
            [
              { value: "player", label: "Player" },
              { value: "player_and_allies", label: "Boudica and current allies" },
            ].concat(factionOptions()),
            function (value) {
              assault.target = value;
              if (value === "player_and_allies" && !assault.reinforcement)
                assault.reinforcement = { capacity_ratio: 2, interval_seconds: 10 };
              markDirty();
              renderInspector();
            },
          ),
        );
        if (assault.reinforcement) {
          campaignActions.appendChild(
            inputField(
              "Strength vs allied capacity (×)",
              assault.reinforcement.capacity_ratio,
              function (value) {
                assault.reinforcement.capacity_ratio = Number(value);
                markDirty();
              },
              { type: "number", min: 1, max: 10, step: 0.25 },
            ),
          );
          campaignActions.appendChild(
            inputField(
              "Reinforcement interval (seconds)",
              assault.reinforcement.interval_seconds,
              function (value) {
                assault.reinforcement.interval_seconds = Number(value);
                markDirty();
              },
              { type: "number", min: 1, max: 600, step: 1 },
            ),
          );
        }
        campaignActions.appendChild(
          checkboxField(
            "Hold the player's final territory until the next decision",
            assault.hold_last_tile === true,
            function (enabled) {
              assault.hold_last_tile = enabled;
              markDirty();
            },
          ),
        );
        campaignActions.appendChild(
          checkboxField(
            "Preserve the attackers' current campaign relationship",
            assault.preserve_relation === true,
            function (enabled) {
              assault.preserve_relation = enabled;
              markDirty();
            },
          ),
        );
      }
      host.appendChild(campaignActions);
    }

    if (step.type === "choice") {
      const answers = section("Player decisions");
      step.choices.forEach(function (choice, index) {
        const card = el("div", { class: "form-card" });
        const titleRow = el("div", { class: "card-title" });
        titleRow.appendChild(el("strong", {}, "Answer " + (index + 1)));
        const answerActions = reorderControls(step.choices, index, "answer");
        if (step.choices.length > 2) {
          const remove = el("button", { type: "button", class: "danger" }, "Remove");
          remove.addEventListener("click", function () {
            const isReferenced = state.definition.steps.some(function (item) {
              return (item.routes || []).some(function (route) {
                return route.when && route.when.choice === step.id && route.when.equals === choice.id;
              });
            });
            if (isReferenced) {
              notice("Remove routes that depend on this answer first.");
              return;
            }
            step.choices.splice(index, 1);
            notice("");
            markDirty();
            renderInspector();
          });
          answerActions.appendChild(remove);
        }
        titleRow.appendChild(answerActions);
        card.appendChild(titleRow);
        card.appendChild(
          inputField("Answer ID", choice.id, function (value) {
            const old = choice.id,
              next = value
                .trim()
                .toLowerCase()
                .replace(/[^a-z0-9_-]+/g, "_");
            if (
              !next ||
              step.choices.some(function (item) {
                return item !== choice && item.id === next;
              })
            )
              return;
            choice.id = next;
            state.definition.steps.forEach(function (item) {
              (item.routes || []).forEach(function (route) {
                if (route.when && route.when.choice === step.id && route.when.equals === old)
                  route.when.equals = next;
              });
            });
            markDirty();
            renderGraph();
          }),
        );
        textEditor(card, "Choice label", "label_key", "choices." + index + ".label_key");
        if (choice.body_key) {
          textEditor(card, "Optional consequence", "body_key", "choices." + index + ".body_key");
          const removeDetail = el("button", { type: "button", class: "danger" }, "Remove detail");
          removeDetail.addEventListener("click", function () {
            delete choice.body_key;
            markDirty();
            renderInspector();
          });
          card.appendChild(removeDetail);
        } else {
          const addDetail = el("button", { type: "button" }, "Add consequence detail");
          addDetail.addEventListener("click", function () {
            choice.body_key = storyKey(step.id + "_" + choice.id + "_detail");
            markDirty();
            renderInspector();
          });
          card.appendChild(addDetail);
        }
        card.appendChild(
          selectField("Next step", choice.next, destinationOptions(step.id), function (value) {
            choice.next = value;
            markDirty();
          }),
        );
        let branchType = "scene";
        const branchTypeOptions = [
          { value: "scene", label: "Scene" },
          { value: "choice", label: "Decision" },
          { value: "objective", label: "Objective" },
          { value: "guide", label: "Guide" },
        ];
        card.appendChild(
          selectField("Create after this answer", branchType, branchTypeOptions, function (value) {
            branchType = value;
          }),
        );
        const createBranch = el("button", { type: "button" }, "Create next step");
        createBranch.addEventListener("click", function () {
          addStep(branchType, { step: step, output: choice });
        });
        card.appendChild(createBranch);
        answers.appendChild(card);
      });
      if (step.choices.length < 4) {
        const add = el("button", { type: "button" }, "Add answer");
        add.addEventListener("click", function () {
          const answerId = uniqueId("answer", step.choices);
          step.choices.push({
            id: answerId,
            label_key: storyKey(step.id + "_" + answerId),
            next:
              state.definition.steps.find(function (item) {
                return item.type === "end";
              })?.id || "",
          });
          markDirty();
          renderInspector();
        });
        answers.appendChild(add);
      }
      host.appendChild(answers);
    }
    if (step.type === "objective" || step.type === "guide") {
      const objective = section("Mechanic and guide");
      objective.appendChild(
        checkboxField(
          "Pause game until this objective completes",
          step.pause_game === true,
          function (value) {
            if (value) step.pause_game = true;
            else {
              delete step.pause_game;
              delete step.camera_only;
              delete step.paused_action;
            }
            markDirty();
            renderInspector();
          },
        ),
      );
      objective.appendChild(
        checkboxField("Allow camera controls only while paused", step.camera_only === true, function (value) {
          if (value) {
            step.pause_game = true;
            step.camera_only = true;
            delete step.paused_action;
          } else delete step.camera_only;
          markDirty();
          renderInspector();
        }),
      );
      objective.appendChild(
        selectField(
          "Action allowed while paused",
          step.paused_action || "",
          [
            { value: "", label: "No gameplay action" },
            { value: "expand_once_then_resume", label: "Expand once, then resume" },
            { value: "send_resources_stay_paused", label: "Send resources; stay paused" },
            { value: "build_until_started", label: "Build until accepted" },
            { value: "upgrade_until_started", label: "Upgrade until accepted" },
          ],
          function (value) {
            if (value) {
              step.pause_game = true;
              step.paused_action = value;
            } else delete step.paused_action;
            markDirty();
            renderInspector();
          },
        ),
      );
      step.trigger = step.trigger || { type: "territory", value: 1, scope: "step" };
      if (step.type === "guide" && !step.guide)
        step.guide = { kind: "world", target: "expand", gesture: "tap" };
      const availableTriggers =
        state.flow === "menu"
          ? triggerTypes.filter(function (trigger) {
              return trigger.value === "ui";
            })
          : step.type === "guide"
            ? triggerTypes.filter(function (trigger) {
                return trigger.value !== "elapsed";
              })
            : triggerTypes;
      objective.appendChild(
        selectField("Complete when", step.trigger.type, availableTriggers, function (value) {
          step.trigger = { type: value, scope: value === "troops" ? "total" : step.trigger.scope || "step" };
          if (
            ["contact", "defeated", "eliminated", "hover", "camera_target", "zoom_in_complete"].includes(
              value,
            )
          )
            step.trigger.target = "";
          if (value === "camera_target") step.trigger.distance = 12;
          if (value === "zoom_in_complete") step.trigger.distance = 12;
          if (value === "zoom_out_complete") step.trigger.value = 0.55;
          if (value === "fleet") step.trigger.unit = "TransportShip";
          if (value === "resource_transfer") {
            step.trigger.recipient = "";
            step.trigger.resources = ["gold", "troops"];
          }
          if (value === "structure_level" || value === "structure_upgrade") step.trigger.kind = "City";
          else if (value === "ui") {
            step.trigger.action =
              state.flow === "menu" ? inGameUiTargets()[0] || "menu_campaign" : "map_attack";
            if (step.guide) Object.assign(step.guide, { kind: "ui", target: step.trigger.action });
          } else step.trigger.value = 1;
          if (["zoom_in", "zoom_out"].includes(value))
            step.guide = { kind: "world", target: "player", gesture: value };
          if (step.guide && value !== "ui" && step.guide.kind === "ui")
            Object.assign(step.guide, { kind: "world", target: worldGuideTarget(value) });
          if (value === "elapsed" && step.type === "objective") delete step.guide;
          markDirty();
          renderInspector();
        }),
      );
      if (step.trigger.type === "contact") {
        if (Array.isArray(step.trigger.targets)) {
          objective.appendChild(
            multiFactionField("Factions to contact", step.trigger.targets, function (targets) {
              step.trigger.targets = targets;
              if (step.trigger.value > targets.length) step.trigger.value = targets.length;
              markDirty();
              renderInspector();
            }),
          );
          objective.appendChild(
            inputField(
              "How many must be contacted",
              step.trigger.value || 1,
              function (value) {
                step.trigger.value = Number(value);
                markDirty();
              },
              { type: "number", min: 1, max: step.trigger.targets.length, step: 1 },
            ),
          );
        } else {
          objective.appendChild(
            selectField("Faction", step.trigger.target, factionOptions(), function (value) {
              step.trigger.target = value;
              markDirty();
            }),
          );
          const anyContact = el("button", { type: "button" }, "Accept contact with multiple factions");
          anyContact.addEventListener("click", function () {
            step.trigger.targets = step.trigger.target ? [step.trigger.target] : [];
            delete step.trigger.target;
            markDirty();
            renderInspector();
          });
          objective.appendChild(anyContact);
        }
      } else if (step.trigger.type === "defeated") {
        if (Array.isArray(step.trigger.targets)) {
          objective.appendChild(
            multiFactionField("Factions to defeat", step.trigger.targets, function (targets) {
              step.trigger.targets = targets;
              step.trigger.value = targets.length;
              markDirty();
            }),
          );
        } else {
          objective.appendChild(
            selectField("Faction", step.trigger.target || "", factionOptions(), function (value) {
              step.trigger.target = value;
              markDirty();
            }),
          );
          const groupTargets = el("button", { type: "button" }, "Track multiple factions (3/3)");
          groupTargets.addEventListener("click", function () {
            step.trigger.targets = step.trigger.target ? [step.trigger.target] : [];
            delete step.trigger.target;
            step.trigger.value = step.trigger.targets.length;
            markDirty();
            renderInspector();
          });
          objective.appendChild(groupTargets);
        }
      } else if (step.trigger.type === "eliminated")
        objective.appendChild(
          selectField(
            "Faction",
            step.trigger.target || "player",
            [{ value: "player", label: "Player" }].concat(factionOptions()),
            function (value) {
              step.trigger.target = value;
              markDirty();
            },
          ),
        );
      else if (step.trigger.type === "attack")
        objective.appendChild(
          selectField(
            "Attack target",
            step.trigger.target || "",
            [{ value: "", label: "Any faction" }].concat(factionOptions()),
            function (value) {
              if (value) step.trigger.target = value;
              else delete step.trigger.target;
              markDirty();
            },
          ),
        );
      else if (step.trigger.type === "fleet") {
        objective.appendChild(
          selectField(
            "Ship type",
            step.trigger.unit || "",
            ["TransportShip", "TradeShip", "Warship"],
            function (value) {
              step.trigger.unit = value;
              if (value !== "TransportShip") delete step.trigger.target;
              markDirty();
              renderInspector();
            },
          ),
        );
        if (step.trigger.unit === "TransportShip")
          objective.appendChild(
            selectField(
              "Landing target",
              step.trigger.target || "",
              [{ value: "", label: "Any target" }].concat(factionOptions()),
              function (value) {
                if (value) step.trigger.target = value;
                else delete step.trigger.target;
                markDirty();
              },
            ),
          );
        objective.appendChild(
          inputField(
            "Required amount",
            step.trigger.value || 1,
            function (value) {
              step.trigger.value = Number(value);
              markDirty();
            },
            { type: "number", min: 1, step: 1 },
          ),
        );
      } else if (step.trigger.type === "resource_transfer") {
        objective.appendChild(
          selectField(
            "Recipient",
            step.trigger.recipient || "",
            [{ value: "", label: "Any faction" }].concat(factionOptions()),
            function (value) {
              if (value) step.trigger.recipient = value;
              else delete step.trigger.recipient;
              markDirty();
            },
          ),
        );
        const requiredResources = step.trigger.resources || [];
        objective.appendChild(
          checkboxField("Must include gold", requiredResources.includes("gold"), function (enabled) {
            step.trigger.resources = requiredResources.filter(function (item) {
              return item !== "gold";
            });
            if (enabled) step.trigger.resources.push("gold");
            if (!step.trigger.resources.length) delete step.trigger.resources;
            markDirty();
            renderInspector();
          }),
        );
        objective.appendChild(
          checkboxField("Must include troops", requiredResources.includes("troops"), function (enabled) {
            step.trigger.resources = requiredResources.filter(function (item) {
              return item !== "troops";
            });
            if (enabled) step.trigger.resources.push("troops");
            if (!step.trigger.resources.length) delete step.trigger.resources;
            markDirty();
            renderInspector();
          }),
        );
        objective.appendChild(
          inputField(
            "Required amount",
            step.trigger.value || 1,
            function (value) {
              step.trigger.value = Number(value);
              markDirty();
            },
            { type: "number", min: 1, step: 1 },
          ),
        );
      } else if (step.trigger.type === "structure_upgrade") {
        objective.appendChild(
          selectField(
            "Building type",
            step.trigger.kind || "City",
            ["City", "Farm", "Factory", "Tower", "Port"],
            function (value) {
              step.trigger.kind = value;
              markDirty();
              renderInspector();
            },
          ),
        );
        objective.appendChild(
          inputField(
            "Upgrades to complete",
            step.trigger.value || 1,
            function (value) {
              step.trigger.value = Number(value);
              markDirty();
            },
            { type: "number", min: 1, step: 1 },
          ),
        );
      } else if (step.trigger.type === "structure_level") {
        const structureLevelLimits = window.SOWCampaign.STRUCTURE_LEVEL_LIMITS;
        objective.appendChild(
          selectField(
            "Building type",
            step.trigger.kind || "City",
            ["City", "Farm", "Factory", "Tower", "Port"],
            function (value) {
              step.trigger.kind = value;
              step.trigger.value = Math.min(Number(step.trigger.value || 1), structureLevelLimits[value]);
              markDirty();
              renderInspector();
            },
          ),
        );
        objective.appendChild(
          inputField(
            "Target level",
            step.trigger.value || 1,
            function (value) {
              step.trigger.value = Math.min(Number(value), structureLevelLimits[step.trigger.kind] || 1);
              markDirty();
            },
            { type: "number", min: 1, max: structureLevelLimits[step.trigger.kind] || 1, step: 1 },
          ),
        );
      } else if (step.trigger.type === "zoom_out_complete")
        objective.appendChild(
          inputField(
            "Zoom-out distance (% of range)",
            Math.round(Number(step.trigger.value || 1) * 100),
            function (value) {
              step.trigger.value = Math.max(1, Math.min(100, Number(value))) / 100;
              markDirty();
            },
            { type: "number", min: 1, max: 100, step: 5 },
          ),
        );
      else if (step.trigger.type === "zoom_in_complete") {
        objective.appendChild(
          selectField(
            "Faction to inspect",
            step.trigger.target || "",
            [{ value: "", label: "Any location" }].concat(factionOptions()),
            function (value) {
              if (value) {
                step.trigger.target = value;
                step.trigger.distance = Number(step.trigger.distance || 12);
              } else {
                delete step.trigger.target;
                delete step.trigger.distance;
              }
              markDirty();
              renderInspector();
            },
          ),
        );
        if (step.trigger.target)
          objective.appendChild(
            inputField(
              "Max distance from camera (tiles)",
              step.trigger.distance || 12,
              function (value) {
                step.trigger.distance = Number(value);
                markDirty();
              },
              { type: "number", min: 1, max: 1000, step: 1 },
            ),
          );
      } else if (step.trigger.type === "camera_target") {
        objective.appendChild(
          selectField(
            "Target to locate",
            step.trigger.target || "",
            [
              { value: "", label: "Choose a target" },
              { value: "player", label: "Player" },
            ].concat(factionOptions()),
            function (value) {
              if (value) step.trigger.target = value;
              else delete step.trigger.target;
              markDirty();
              renderInspector();
            },
          ),
        );
        objective.appendChild(
          inputField(
            "Max distance (tiles)",
            step.trigger.distance || 12,
            function (value) {
              step.trigger.distance = Number(value);
              markDirty();
            },
            { type: "number", min: 1, max: 1000, step: 1 },
          ),
        );
      } else if (step.trigger.type === "hover")
        objective.appendChild(
          selectField(
            "Target to locate",
            step.trigger.target || "",
            [
              { value: "", label: "Choose a target" },
              { value: "player", label: "Player" },
            ].concat(factionOptions()),
            function (value) {
              if (value) step.trigger.target = value;
              else delete step.trigger.target;
              markDirty();
              renderInspector();
            },
          ),
        );
      else if (step.trigger.type === "ui")
        objective.appendChild(
          selectField("Control action", step.trigger.action, inGameUiTargets(), function (value) {
            step.trigger.action = value;
            markDirty();
          }),
        );
      else
        objective.appendChild(
          inputField(
            step.trigger.type === "troops"
              ? "Minimum troops"
              : step.trigger.type === "elapsed"
                ? "Wait (seconds)"
                : step.trigger.type === "territory" && step.trigger.scope === "total"
                  ? "Territory threshold"
                  : "Required amount",
            step.trigger.value,
            function (value) {
              step.trigger.value = Number(value);
              markDirty();
            },
            { type: "number", min: 1, step: 1 },
          ),
        );
      if (step.trigger.type !== "troops")
        objective.appendChild(
          selectField(
            "Count from",
            step.trigger.scope || "step",
            ["step", "episode", "total"],
            function (value) {
              step.trigger.scope = value;
              if (value !== "total") delete step.trigger.comparison;
              markDirty();
              renderInspector();
            },
          ),
        );
      if (step.trigger.type === "territory" && step.trigger.scope === "total")
        objective.appendChild(
          selectField(
            "Territory comparison",
            step.trigger.comparison || "gte",
            [
              { value: "gte", label: "At least" },
              { value: "lte", label: "At most" },
            ],
            function (value) {
              if (value === "lte") step.trigger.comparison = value;
              else delete step.trigger.comparison;
              markDirty();
              renderInspector();
            },
          ),
        );
      if (step.guide) {
        objective.appendChild(
          selectField(
            "Hand points at",
            step.guide.kind + ":" + step.guide.target,
            [
              { value: "world:expand", label: "Map · expansion" },
              { value: "world:assault", label: "Map · attack" },
              { value: "world:target_action", label: "Map · target action" },
              { value: "world:player", label: "Map · player base" },
              { value: "world:nameplate", label: "Map · faction nameplate" },
            ].concat(
              inGameUiTargets().map(function (key) {
                return { value: "ui:" + key, label: "Interface · " + key.replace(/_/g, " ") };
              }),
            ),
            function (value) {
              const pair = value.split(":");
              if (pair[0] !== step.guide.kind) delete step.guide.to;
              step.guide.kind = pair[0];
              step.guide.target = pair[1];
              markDirty();
              renderInspector();
            },
          ),
        );
        objective.appendChild(
          selectField(
            "Gesture",
            step.guide.gesture || "tap",
            ["tap", "hold", "drag", "hover", "pan_keys", "zoom_in", "zoom_out"],
            function (value) {
              step.guide.gesture = value;
              if (value !== "drag") delete step.guide.to;
              if (["zoom_in", "zoom_out"].includes(value)) {
                step.guide.kind = "world";
                step.guide.target = "player";
                step.trigger.type = step.trigger.type === value + "_complete" ? value + "_complete" : value;
              }
              markDirty();
              renderInspector();
            },
          ),
        );
        objective.appendChild(
          checkboxField("Show text beside hand", step.guide.show_label !== false, function (show) {
            if (show) delete step.guide.show_label;
            else step.guide.show_label = false;
            markDirty();
          }),
        );
        if (step.guide.gesture === "drag") {
          const dragTargets =
            step.guide.kind === "world"
              ? [
                  { value: "expand", label: "Map · expansion" },
                  { value: "assault", label: "Map · attack" },
                  { value: "target_action", label: "Map · target action" },
                  { value: "player", label: "Map · player base" },
                  { value: "nameplate", label: "Map · faction nameplate" },
                ]
              : [{ value: "", label: "Within this control" }].concat(
                  inGameUiTargets().map(function (key) {
                    return { value: key, label: "Interface · " + key.replace(/_/g, " ") };
                  }),
                );
          objective.appendChild(
            selectField("Drag destination", step.guide.to, dragTargets, function (value) {
              if (value) step.guide.to = value;
              else delete step.guide.to;
              markDirty();
            }),
          );
        }
        if (step.type === "objective") {
          const removeGuide = el("button", { type: "button" }, "Remove hand guide");
          removeGuide.addEventListener("click", function () {
            delete step.guide;
            markDirty();
            renderInspector();
          });
          objective.appendChild(removeGuide);
        }
      } else if (step.type === "objective" && step.trigger.type !== "elapsed") {
        const addGuide = el("button", { type: "button" }, "Add hand guide");
        addGuide.addEventListener("click", function () {
          step.guide = ["zoom_in", "zoom_out"].includes(step.trigger.type)
            ? { kind: "world", target: "player", gesture: step.trigger.type }
            : step.trigger.type === "ui"
              ? { kind: "ui", target: step.trigger.action, gesture: "tap" }
              : { kind: "world", target: worldGuideTarget(step.trigger.type), gesture: "tap" };
          markDirty();
          renderInspector();
        });
        objective.appendChild(addGuide);
      }
      objective.appendChild(
        selectField("Next step", step.next, destinationOptions(step.id), function (value) {
          step.next = value;
          markDirty();
        }),
      );
      host.appendChild(objective);
    } else if (step.type !== "choice" && step.type !== "end") {
      const next = section("Flow");
      next.appendChild(
        selectField("Next step", step.next, destinationOptions(step.id), function (value) {
          step.next = value;
          markDirty();
        }),
      );
      host.appendChild(next);
    }
    if (["scene", "choice", "objective", "guide"].includes(step.type)) {
      const markerPanel = section("Map marker");
      if (step.marker)
        markerPanel.appendChild(
          selectField(
            "Highlight faction",
            step.marker.target,
            [{ value: "player", label: "Player" }].concat(factionOptions()),
            function (value) {
              step.marker.target = value;
              markDirty();
            },
          ),
        );
      else {
        const addMarkerButton = el("button", { type: "button" }, "Add map marker");
        addMarkerButton.addEventListener("click", function () {
          step.marker = { target: "player" };
          markDirty();
          renderInspector();
        });
        markerPanel.appendChild(addMarkerButton);
      }
      host.appendChild(markerPanel);
    }
    if (step.type !== "choice" && step.type !== "end") {
      const paths = section("Conditional paths");
      paths.appendChild(
        el(
          "small",
          { class: "muted" },
          "First matching condition wins; otherwise the default next link is used.",
        ),
      );
      (step.routes || []).forEach(function (route, index) {
        const card = el("div", { class: "form-card" });
        const routeHeader = el("div", { class: "card-title" });
        routeHeader.appendChild(el("strong", {}, "Condition " + (index + 1)));
        routeHeader.appendChild(reorderControls(step.routes, index, "condition"));
        card.appendChild(routeHeader);
        const kind =
          route.when && Object.prototype.hasOwnProperty.call(route.when, "choice") ? "choice" : "fact";
        card.appendChild(
          selectField(
            "Condition",
            kind,
            [
              { value: "fact", label: "Game fact threshold" },
              { value: "choice", label: "Earlier decision" },
            ],
            function (value) {
              route.when = value === "choice" ? { choice: "", equals: "" } : { fact: "tiles_gained", gte: 1 };
              markDirty();
              renderInspector();
            },
          ),
        );
        if (kind === "choice") {
          const selectedAnswer =
            route.when.choice && route.when.equals ? route.when.choice + "@" + route.when.equals : "";
          card.appendChild(
            selectField(
              "Answer",
              selectedAnswer,
              [{ value: "", label: "Choose a decision answer" }].concat(choiceAnswerOptions()),
              function (value) {
                const pair = value.split("@");
                route.when.choice = pair[0] || "";
                route.when.equals = pair[1] || "";
                markDirty();
              },
            ),
          );
        } else {
          const gameFacts = Object.values(window.SOWCampaign.METRICS)
            .concat(["tiles", "touch_controls"])
            .filter(function (value, index, all) {
              return all.indexOf(value) === index;
            });
          card.appendChild(
            selectField("Fact", route.when.fact, gameFacts, function (value) {
              route.when.fact = value;
              markDirty();
            }),
          );
          card.appendChild(
            inputField(
              "At least",
              route.when.gte,
              function (value) {
                route.when.gte = Number(value);
                markDirty();
              },
              { type: "number", min: 0, step: 1 },
            ),
          );
        }
        card.appendChild(
          selectField("Then go to", route.next, destinationOptions(step.id), function (value) {
            route.next = value;
            markDirty();
          }),
        );
        const removeRoute = el("button", { type: "button", class: "danger" }, "Remove condition");
        removeRoute.addEventListener("click", function () {
          step.routes.splice(index, 1);
          if (!step.routes.length) delete step.routes;
          markDirty();
          renderInspector();
        });
        card.appendChild(removeRoute);
        paths.appendChild(card);
      });
      const addRoute = el("button", { type: "button" }, "Add conditional path");
      addRoute.addEventListener("click", function () {
        const destination = destinationOptions(step.id)[0];
        step.routes = step.routes || [];
        step.routes.push({
          when: { fact: "tiles_gained", gte: 1 },
          next: (destination && destination.value) || "",
        });
        markDirty();
        renderInspector();
      });
      paths.appendChild(addRoute);
      host.appendChild(paths);
    }
    const danger = el("section", { class: "form-section" });
    const removeStep = el("button", { type: "button", class: "danger" }, "Delete step");
    removeStep.addEventListener("click", function () {
      if (state.definition.steps.length <= 1) return;
      const next = step.next;
      const incoming = state.definition.steps.some(function (item) {
        return (
          item !== step &&
          (item.next === step.id ||
            (item.choices || []).some(function (choice) {
              return choice.next === step.id;
            }) ||
            (item.routes || []).some(function (route) {
              return route.next === step.id;
            }))
        );
      });
      const referencedChoice = state.definition.steps.some(function (item) {
        return (item.routes || []).some(function (route) {
          return route.when && route.when.choice === step.id;
        });
      });
      const isEntry = state.definition.entry === step.id;
      const isMenuEntry = state.definition.menu_guide && state.definition.menu_guide.entry === step.id;
      const targetExists = state.definition.steps.some(function (item) {
        return item.id === next && item !== step;
      });
      const canBypass = step.type !== "choice" && !(step.routes || []).length && targetExists;
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
          (item.choices || []).forEach(function (choice) {
            if (choice.next === step.id) choice.next = next;
          });
          (item.routes || []).forEach(function (route) {
            if (route.next === step.id) route.next = next;
          });
        });
        if (isEntry) state.definition.entry = next;
        if (isMenuEntry) state.definition.menu_guide.entry = next;
      }
      state.definition.steps = state.definition.steps.filter(function (item) {
        return item !== step;
      });
      state.selected = canBypass ? next : state.definition.entry;
      delete state.definition.layout[step.id];
      notice("");
      markDirty();
      renderInspector();
    });
    danger.appendChild(removeStep);
    host.appendChild(danger);
  }
  function stepLabel(step) {
    const title = textValue(step.title_key, state.previewLanguage);
    return [title && title !== step.id ? title : "", step.type, step.id].filter(Boolean).join(" · ");
  }
  function stepOption(step) {
    return { value: step.id, label: stepLabel(step) };
  }
  function destinationOptions(except) {
    return state.definition.steps
      .filter(function (step) {
        return step.id !== except;
      })
      .map(stepOption);
  }
  function factionOptions() {
    return state.roster.factions.map(function (faction) {
      return { value: faction.id, label: faction.name };
    });
  }
  function enemyFactionOptions() {
    return state.roster.factions
      .filter(function (faction) {
        return faction.relation === "enemy";
      })
      .map(function (faction) {
        return { value: faction.id, label: faction.name };
      });
  }
  function supportFactionOptions() {
    return state.roster.factions
      .filter(function (faction) {
        return Number.isInteger(faction.support_interval_seconds);
      })
      .map(function (faction) {
        return { value: faction.id, label: faction.name };
      });
  }
  function factionName(factionId) {
    const faction = state.roster.factions.find(function (item) {
      return item.id === factionId;
    });
    return faction ? faction.name : factionId;
  }
  function speakerOptions() {
    return [{ value: "", label: "Narrator" }].concat(
      Object.keys(state.definition.speakers || {}).map(function (id) {
        const speaker = state.definition.speakers[id],
          faction = state.roster.factions.find(function (item) {
            return item.id === speaker.faction;
          });
        return { value: id, label: faction ? faction.name : speaker.name || id };
      }),
    );
  }
  function inGameUiTargets() {
    return Object.keys(window.SOWCampaign.UI_TARGETS).filter(function (key) {
      return state.flow === "menu"
        ? key.startsWith("menu_") || key === "campaign_replay"
        : !key.startsWith("menu_") && key !== "campaign_replay";
    });
  }
  function worldGuideTarget(type) {
    return ["attack", "kills"].includes(type)
      ? "assault"
      : ["contact", "defeated"].includes(type)
        ? "target_action"
        : ["hover", "camera_target"].includes(type)
          ? "player"
          : "expand";
  }
  function syncMenuGuideSteps() {
    if (!state.isMenuGuides || !state.definition || !state.definition.menu_guides) return;
    const byId = new Map(
      state.definition.steps.map(function (step) {
        return [step.id, step];
      }),
    );
    Object.keys(state.definition.menu_guides).forEach(function (guideId) {
      const guide = state.definition.menu_guides[guideId],
        reached = new Set();
      function visit(id) {
        if (reached.has(id) || !byId.has(id)) return;
        reached.add(id);
        const step = byId.get(id);
        [step.next]
          .concat(
            (step.routes || []).map(function (route) {
              return route.next;
            }),
            (step.choices || []).map(function (choice) {
              return choice.next;
            }),
          )
          .forEach(visit);
      }
      visit(guide.entry);
      guide.step_ids = state.definition.steps
        .filter(function (step) {
          return reached.has(step.id);
        })
        .map(function (step) {
          return step.id;
        });
    });
  }
  function flowEntry() {
    return state.isMenuGuides
      ? state.definition.menu_guides[state.menuGuideId] &&
          state.definition.menu_guides[state.menuGuideId].entry
      : state.flow === "menu"
        ? state.definition.menu_guide && state.definition.menu_guide.entry
        : state.definition.entry;
  }
  function campaignStepPolicies(choices, facts) {
    const byId = new Map(
      state.definition.steps.map(function (step) {
        return [step.id, step];
      }),
    );
    return window.SOWCampaign.campaignUnlockStates(byId, state.definition.entry, choices, facts);
  }
  function campaignUnlockLabel(unlocks) {
    const labels = Object.keys((unlocks && unlocks.buildings) || {})
      .sort()
      .map(function (kind) {
        return kind + " L" + unlocks.buildings[kind];
      });
    const names = {
      transport_fleet: "Transport fleets",
      trade_ship: "Trade ships",
      warship: "Warships",
      nuke: "Nukes",
    };
    ((unlocks && unlocks.actions) || []).forEach(function (action) {
      if (!names[action]) return;
      if (window.SOWCampaign.campaignAllowsAction(unlocks, action)) {
        labels.push(names[action]);
        return;
      }
      const requirement = window.SOWCampaign.CAMPAIGN_ACTION_BUILDING_REQUIREMENTS[action];
      if (requirement)
        labels.push(names[action] + " (needs " + requirement.kind + " L" + requirement.level + ")");
    });
    return labels.join(" · ");
  }
  function choiceAnswerOptions() {
    const result = [];
    state.definition.steps
      .filter(function (step) {
        return step.type === "choice";
      })
      .forEach(function (step) {
        (step.choices || []).forEach(function (answer) {
          result.push({
            value: step.id + "@" + answer.id,
            label:
              stepLabel(step) + " · " + (textValue(answer.label_key, state.previewLanguage) || answer.id),
          });
        });
      });
    return result;
  }

  function renderSettings() {
    const host = $("#settings");
    host.replaceChildren();
    if (state.isMenuGuides) {
      host.appendChild(el("p", { class: "empty-state" }, "Menu guides have no map or speaker settings."));
      return;
    }
    const settings = state.definition.settings;
    host.appendChild(
      inputField(
        "Starting troops",
        settings.starting_troops,
        function (value) {
          settings.starting_troops = Number(value);
          $("#sampleTroops").textContent = Number(value).toLocaleString();
          markDirty();
        },
        { type: "number", min: 1, max: 100000, step: 100 },
      ),
    );
    host.appendChild(
      checkboxField("Allied support after milestone", Boolean(settings.campaign_support), function (enabled) {
        if (!enabled) delete settings.campaign_support;
        else {
          const defaultMilestone =
            (
              state.roster.factions.find(function (faction) {
                return faction.id === "roman_outpost";
              }) || {}
            ).id || "";
          settings.campaign_support = settings.campaign_support || {
            after_defeated: defaultMilestone,
            share_percent: 50,
          };
        }
        markDirty();
        renderSettings();
      }),
    );
    if (settings.campaign_support) {
      const support = settings.campaign_support;
      host.appendChild(
        selectField("Support begins after", support.after_defeated || "", factionOptions(), function (value) {
          support.after_defeated = value;
          markDirty();
        }),
      );
      host.appendChild(
        inputField(
          "Current reserves given (%)",
          support.share_percent,
          function (value) {
            support.share_percent = Number(value);
            markDirty();
          },
          { type: "number", min: 1, max: 100, step: 1 },
        ),
      );
    }
    const reactions = section("Story responses");
    (state.definition.reactions || []).forEach(function (reaction) {
      const card = el("div", { class: "form-card" });
      const isContact = reaction.when.type === "contact";
      const isFirstContact = reaction.when.type === "first_contact";
      const isPreDefeat = reaction.when.type === "pre_defeat";
      const isSupport = reaction.when.type === "support";
      card.appendChild(
        el(
          "strong",
          {},
          (isPreDefeat
            ? "Before defeat · "
            : isFirstContact
              ? "First Iceni contact · "
              : isContact
                ? "Contact · "
                : "First support · ") + reaction.id,
        ),
      );
      if (isContact) {
        const contactTargetOptions = [
          { value: "any_neutral", label: "Any initially neutral faction" },
        ].concat(factionOptions());
        card.appendChild(
          selectField(
            "On contact with",
            reaction.when.relation === "neutral" ? "any_neutral" : reaction.when.target,
            contactTargetOptions,
            function (value) {
              if (value === "any_neutral") {
                delete reaction.when.target;
                reaction.when.relation = "neutral";
              } else {
                delete reaction.when.relation;
                reaction.when.target = value;
              }
              markDirty();
              renderSettings();
            },
          ),
        );
        card.appendChild(
          selectField(
            "Show after",
            reaction.after || "",
            [{ value: "", label: "Immediately" }].concat(
              state.definition.steps
                .filter(function (step) {
                  return ["objective", "guide"].includes(step.type);
                })
                .map(stepOption),
            ),
            function (value) {
              if (value) {
                reaction.after = value;
                delete reaction.during;
              } else {
                delete reaction.after;
                delete reaction.during;
              }
              markDirty();
              renderSettings();
            },
          ),
        );
        if (reaction.when.target) {
          const contactGates = state.definition.steps.filter(function (step) {
            const trigger = step.trigger || {};
            return (
              ["objective", "guide"].includes(step.type) &&
              trigger.type === "defeated" &&
              (trigger.target === reaction.when.target ||
                (Array.isArray(trigger.targets) && trigger.targets.includes(reaction.when.target)))
            );
          });
          if (contactGates.length)
            card.appendChild(
              selectField(
                "Only after this defeat objective starts",
                reaction.during || "",
                [{ value: "", label: "No active-objective gate" }].concat(contactGates.map(stepOption)),
                function (value) {
                  if (value) {
                    reaction.during = value;
                    delete reaction.after;
                  } else delete reaction.during;
                  markDirty();
                  renderSettings();
                },
              ),
            );
        }
      } else if (isFirstContact) {
        card.appendChild(
          multiFactionField(
            "Eligible factions (first matching contact)",
            reaction.when.targets || [],
            function (value) {
              reaction.when.targets = value;
              markDirty();
            },
          ),
        );
      } else if (isSupport) {
        card.appendChild(
          selectField(
            "Show after",
            reaction.after,
            state.definition.steps
              .filter(function (step) {
                return ["objective", "guide"].includes(step.type);
              })
              .map(stepOption),
            function (value) {
              reaction.after = value;
              markDirty();
            },
          ),
        );
        card.appendChild(
          selectField("First aid from", reaction.when.target, supportFactionOptions(), function (value) {
            reaction.when.target = value;
            markDirty();
          }),
        );
      } else if (isPreDefeat) {
        const defeatGates = state.definition.steps.filter(function (step) {
          const trigger = step.trigger || {};
          return (
            ["objective", "guide"].includes(step.type) &&
            trigger.type === "defeated" &&
            (trigger.target === reaction.when.target ||
              (Array.isArray(trigger.targets) && trigger.targets.includes(reaction.when.target)))
          );
        });
        card.appendChild(
          selectField(
            "Active defeat objective",
            reaction.during || "",
            defeatGates.map(stepOption),
            function (value) {
              reaction.during = value;
              markDirty();
              renderSettings();
            },
          ),
        );
      }
      if ((isContact || isFirstContact) && reaction.presentation !== "exchange") {
        card.appendChild(
          selectField(
            "Automatic relationship result",
            reaction.outcome || "",
            [
              { value: "", label: reaction.choices ? "Negotiation choices" : "Dialogue only" },
              { value: "allied", label: "Alliance" },
              { value: "enemy", label: "Enemy" },
              { value: "neutral", label: "Remain neutral" },
            ],
            function (value) {
              if (value) {
                reaction.outcome = value;
                delete reaction.choices;
              } else delete reaction.outcome;
              markDirty();
              renderSettings();
            },
          ),
        );
        if (reaction.outcome === "allied")
          card.appendChild(
            inputField(
              "Gold paid",
              reaction.gold_cost || 0,
              function (value) {
                reaction.gold_cost = Number(value);
                markDirty();
              },
              { type: "number", min: 0, max: 1000000, step: 25 },
            ),
          );
        if (reaction.choices) {
          reaction.choices.forEach(function (choice) {
            const choiceCard = el("div", { class: "form-card" });
            choiceCard.appendChild(el("strong", {}, "Negotiation · " + choice.id));
            choiceCard.appendChild(
              selectField("Result", choice.relation, ["allied", "enemy", "neutral"], function (value) {
                choice.relation = value;
                markDirty();
                renderSettings();
              }),
            );
            if (choice.relation === "allied")
              choiceCard.appendChild(
                inputField(
                  "Gold demanded",
                  choice.gold_cost || 0,
                  function (value) {
                    choice.gold_cost = Number(value);
                    markDirty();
                  },
                  { type: "number", min: 0, max: 1000000, step: 25 },
                ),
              );
            catalogTextField(choiceCard, "Choice", choice.label_key, 2);
            catalogTextField(choiceCard, "Consequence", choice.body_key, 3);
            const removeChoice = el("button", { type: "button", class: "danger" }, "Remove choice");
            removeChoice.disabled = reaction.choices.length <= 2;
            removeChoice.addEventListener("click", function () {
              reaction.choices = reaction.choices.filter(function (item) {
                return item !== choice;
              });
              markDirty();
              renderSettings();
            });
            choiceCard.appendChild(removeChoice);
            card.appendChild(choiceCard);
          });
          const addChoice = el("button", { type: "button" }, "Add negotiation option");
          addChoice.disabled = reaction.choices.length >= 4;
          addChoice.addEventListener("click", function () {
            const choiceId = "offer_" + (reaction.choices.length + 1),
              key = storyKey(reaction.id + "_" + choiceId);
            reaction.choices.push({
              id: choiceId,
              label_key: key + "_label",
              body_key: key + "_detail",
              relation: "neutral",
            });
            markDirty();
            renderSettings();
          });
          card.appendChild(addChoice);
        } else if (!reaction.outcome) {
          const addNegotiation = el("button", { type: "button" }, "Add negotiation choices");
          addNegotiation.addEventListener("click", function () {
            const prefix = storyKey(reaction.id);
            reaction.choices = [
              {
                id: "accept",
                label_key: prefix + "_accept_label",
                body_key: prefix + "_accept_detail",
                relation: "allied",
                gold_cost: 200,
              },
              {
                id: "refuse",
                label_key: prefix + "_refuse_label",
                body_key: prefix + "_refuse_detail",
                relation: "enemy",
                gold_cost: 0,
              },
            ];
            markDirty();
            renderSettings();
          });
          card.appendChild(addNegotiation);
        }
      }
      const responsePresentation = reaction.presentation === "exchange" ? "exchange" : "dialogue";
      const presentationOptions = [
        { value: "dialogue", label: reaction.exchange ? "Dialogue, then exchange" : "Dialogue modal" },
      ];
      if (!reaction.choices && !reaction.outcome && reaction.gold_cost == null)
        presentationOptions.push({ value: "exchange", label: "Automatic exchange" });
      card.appendChild(
        selectField("Response", responsePresentation, presentationOptions, function (value) {
          if (value === "exchange") {
            const actor =
              reaction.speaker ||
              Object.keys(state.definition.speakers || {}).find(function (speakerId) {
                return state.definition.speakers[speakerId].faction === reaction.when.target;
              }) ||
              Object.keys(state.definition.speakers || {}).find(function (speakerId) {
                return !state.definition.speakers[speakerId].faction;
              });
            if (!reaction.exchange || !reaction.exchange.length) {
              reaction.exchange = reaction.body_key
                ? [
                    {
                      speaker: actor,
                      presentation: "dialogue",
                      title_key: reaction.title_key,
                      show_title: reaction.show_title,
                      body_key: reaction.body_key,
                      delay_ms: 0,
                    },
                  ]
                : [
                    {
                      speaker: actor,
                      presentation: "emote",
                      emote: "threat",
                      emote_text_key: storyKey(reaction.id + "_line_1_emote"),
                      delay_ms: window.SOWCampaign.EXCHANGE_DEFAULT_REPLY_DELAY_MS,
                    },
                  ];
            }
            ["title_key", "body_key", "show_title", "speaker_role_key", "emote", "emote_text_key"].forEach(
              function (key) {
                delete reaction[key];
              },
            );
            reaction.presentation = "exchange";
          } else {
            reaction.presentation = "dialogue";
            reaction.body_key = reaction.body_key || storyKey(reaction.id + "_body");
          }
          markDirty();
          renderSettings();
        }),
      );
      if (reaction.presentation !== "exchange") {
        card.appendChild(
          selectField("Speaker", reaction.speaker || "", speakerOptions(), function (value) {
            if (value) reaction.speaker = value;
            else delete reaction.speaker;
            markDirty();
            renderSettings();
          }),
        );
        if (reaction.speaker_role_key)
          catalogTextField(card, "Faction speaker role", reaction.speaker_role_key, 2);
        else if (!reaction.speaker) {
          const addSpeakerRole = el("button", { type: "button" }, "Identify the contacted faction's speaker");
          addSpeakerRole.addEventListener("click", function () {
            reaction.speaker_role_key = storyKey(reaction.id + "_speaker_role");
            markDirty();
            renderSettings();
          });
          card.appendChild(addSpeakerRole);
        }
        reaction.body_key = reaction.body_key || storyKey(reaction.id + "_body");
        catalogTextField(card, "Title", reaction.title_key, 2);
        catalogTextField(card, "Dialogue", reaction.body_key, 4);
      }
      if (reaction.exchange) {
        renderExchangeLines(card, reaction.exchange, reaction.id, "Exchange line");
        if (reaction.exchange.length < 16) {
          const addExchangeLine = el("button", { type: "button" }, "Add exchange line");
          addExchangeLine.addEventListener("click", function () {
            const targetSpeaker =
              Object.keys(state.definition.speakers || {}).find(function (speakerId) {
                return state.definition.speakers[speakerId].faction === reaction.when.target;
              }) ||
              Object.keys(state.definition.speakers || {}).find(function (speakerId) {
                return !state.definition.speakers[speakerId].faction;
              });
            reaction.exchange.push({
              speaker: targetSpeaker,
              presentation: "emote",
              emote: "threat",
              emote_text_key: storyKey(reaction.id + "_line_" + (reaction.exchange.length + 1) + "_emote"),
              delay_ms: window.SOWCampaign.EXCHANGE_DEFAULT_REPLY_DELAY_MS,
            });
            markDirty();
            renderSettings();
          });
          card.appendChild(addExchangeLine);
        }
        if (reaction.presentation !== "exchange") {
          const removeExchange = el("button", { type: "button", class: "danger" }, "Remove exchange");
          removeExchange.addEventListener("click", function () {
            delete reaction.exchange;
            markDirty();
            renderSettings();
          });
          card.appendChild(removeExchange);
        }
      }
      const remove = el("button", { type: "button", class: "danger" }, "Remove response");
      remove.addEventListener("click", function () {
        state.definition.reactions = state.definition.reactions.filter(function (item) {
          return item !== reaction;
        });
        markDirty();
        renderSettings();
      });
      card.appendChild(remove);
      reactions.appendChild(card);
    });
    function addReaction(type) {
      const isFirstContact = type === "first_contact",
        isNeutralContact = type === "neutral_contact";
      const eventType = isNeutralContact ? "contact" : type;
      const used = new Set(
        (state.definition.reactions || [])
          .filter(function (item) {
            return item.when.type === eventType;
          })
          .map(function (item) {
            return item.when.target || (item.when.relation === "neutral" ? "any_neutral" : null);
          }),
      );
      const factions =
        eventType === "support"
          ? supportFactionOptions().map(function (option) {
              return state.roster.factions.find(function (item) {
                return item.id === option.value;
              });
            })
          : state.roster.factions;
      const faction =
        isFirstContact || isNeutralContact
          ? null
          : factions.find(function (item) {
              return item && !used.has(item.id);
            });
      if (
        type === "first_contact" &&
        (state.definition.reactions || []).some(function (item) {
          return item.when.type === "first_contact";
        })
      )
        return;
      if (isNeutralContact && used.has("any_neutral")) return;
      const after =
        eventType === "support" &&
        state.definition.steps.find(function (step) {
          return step.type === "objective" || step.type === "guide";
        });
      if ((!isFirstContact && !isNeutralContact && !faction) || (eventType === "support" && !after)) return;
      let responseId = isFirstContact
        ? "first_contact"
        : isNeutralContact
          ? "neutral_contact"
          : eventType + "_" + faction.id;
      while (
        state.definition.steps.some(function (step) {
          return step.id === responseId;
        }) ||
        (state.definition.reactions || []).some(function (item) {
          return item.id === responseId;
        })
      )
        responseId += "_2";
      const speakerId =
        faction &&
        Object.keys(state.definition.speakers || {}).find(function (key) {
          return state.definition.speakers[key].faction === faction.id;
        });
      const when = isFirstContact
        ? {
            type: type,
            targets: state.roster.factions
              .filter(function (item) {
                return item.relation === "neutral";
              })
              .map(function (item) {
                return item.id;
              }),
          }
        : isNeutralContact
          ? { type: "contact", relation: "neutral" }
          : { type: eventType, target: faction.id };
      const reaction = {
        id: responseId,
        when: when,
        title_key: storyKey(responseId + "_title"),
        body_key: storyKey(responseId + "_body"),
      };
      if (after) reaction.after = after.id;
      if (speakerId) reaction.speaker = speakerId;
      if (isFirstContact) reaction.outcome = "allied";
      state.definition.reactions = (state.definition.reactions || []).concat(reaction);
      markDirty();
      renderSettings();
    }
    const addContactReaction = el("button", { type: "button" }, "Add contact response");
    addContactReaction.disabled = !state.roster.factions.some(function (item) {
      return !(state.definition.reactions || []).some(function (reaction) {
        return reaction.when.type === "contact" && reaction.when.target === item.id;
      });
    });
    addContactReaction.addEventListener("click", function () {
      addReaction("contact");
    });
    const addNeutralNegotiation = el("button", { type: "button" }, "Add any-neutral negotiation");
    addNeutralNegotiation.disabled = (state.definition.reactions || []).some(function (item) {
      return item.when.type === "contact" && item.when.relation === "neutral";
    });
    addNeutralNegotiation.addEventListener("click", function () {
      addReaction("neutral_contact");
    });
    const addFirstContact = el("button", { type: "button" }, "Add first-contact scene");
    addFirstContact.disabled =
      (state.definition.reactions || []).some(function (item) {
        return item.when.type === "first_contact";
      }) ||
      !state.roster.factions.some(function (item) {
        return item.relation === "neutral";
      });
    addFirstContact.addEventListener("click", function () {
      addReaction("first_contact");
    });
    const addSupportReaction = el("button", { type: "button" }, "Add support response");
    addSupportReaction.disabled =
      !state.definition.steps.some(function (step) {
        return ["objective", "guide"].includes(step.type);
      }) ||
      !supportFactionOptions().some(function (option) {
        return !(state.definition.reactions || []).some(function (reaction) {
          return reaction.when.type === "support" && reaction.when.target === option.value;
        });
      });
    addSupportReaction.addEventListener("click", function () {
      addReaction("support");
    });
    function encounterGates(type) {
      const speakers = state.definition.speakers || {};
      const boudica = speakers.boudica
        ? "boudica"
        : Object.keys(speakers).find(function (key) {
            return !speakers[key].faction;
          });
      return state.definition.steps
        .filter(function (step) {
          const trigger = step.trigger || {};
          return ["objective", "guide"].includes(step.type) && trigger.type === "defeated";
        })
        .flatMap(function (step) {
          const trigger = step.trigger || {},
            targets = Array.isArray(trigger.targets) ? trigger.targets : [trigger.target];
          return targets
            .filter(Boolean)
            .map(function (target) {
              let roman =
                Object.keys(speakers).find(function (key) {
                  return speakers[key].faction === target;
                }) || "faction_" + target;
              while (speakers[roman] && speakers[roman].faction !== target) roman += "_2";
              const alreadyUsed = (state.definition.reactions || []).some(function (reaction) {
                return (
                  reaction.when.type === type &&
                  reaction.when.target === target &&
                  reaction.during === step.id
                );
              });
              return !alreadyUsed && roman && (type !== "contact" || boudica)
                ? { step: step, target: target, roman: roman, boudica: boudica }
                : null;
            })
            .filter(Boolean);
        });
    }
    function addEncounterReaction(type) {
      const encounter = encounterGates(type)[0];
      if (!encounter) return;
      state.definition.speakers = state.definition.speakers || {};
      if (!state.definition.speakers[encounter.roman])
        state.definition.speakers[encounter.roman] = { faction: encounter.target };
      const responseId = uniqueId(
        "campaign_" + type + "_" + encounter.target,
        state.definition.reactions || [],
      );
      const speaker = encounter.roman;
      if (type === "contact") {
        state.definition.reactions.push({
          id: responseId,
          when: { type: "contact", target: encounter.target },
          during: encounter.step.id,
          presentation: "exchange",
          exchange: [
            {
              speaker: speaker,
              presentation: "emote",
              emote: "threat",
              emote_text_key: storyKey(responseId + "_roman_taunt"),
              delay_ms: window.SOWCampaign.EXCHANGE_DEFAULT_REPLY_DELAY_MS,
            },
            {
              speaker: encounter.boudica,
              presentation: "emote",
              emote: "threat",
              emote_text_key: storyKey(responseId + "_boudica_reply"),
              delay_ms: 0,
            },
          ],
        });
      } else {
        state.definition.reactions.push({
          id: responseId,
          when: { type: "pre_defeat", target: encounter.target },
          during: encounter.step.id,
          presentation: "exchange",
          exchange: [
            {
              speaker: speaker,
              presentation: "emote",
              emote: "threat",
              emote_text_key: storyKey(responseId + "_last_words"),
              delay_ms: 0,
            },
          ],
        });
      }
      markDirty();
      renderSettings();
    }
    const addCampaignProvocation = el("button", { type: "button" }, "Add next active-objective provocation");
    addCampaignProvocation.disabled = encounterGates("contact").length === 0;
    addCampaignProvocation.addEventListener("click", function () {
      addEncounterReaction("contact");
    });
    const addPreDefeatExchange = el("button", { type: "button" }, "Add next pre-defeat exchange");
    addPreDefeatExchange.disabled = encounterGates("pre_defeat").length === 0;
    addPreDefeatExchange.addEventListener("click", function () {
      addEncounterReaction("pre_defeat");
    });
    reactions.append(
      addContactReaction,
      addNeutralNegotiation,
      addFirstContact,
      addSupportReaction,
      addCampaignProvocation,
      addPreDefeatExchange,
    );
    host.appendChild(reactions);
    host.appendChild(
      selectField(
        "Opening step",
        state.definition.entry,
        state.definition.steps.map(stepOption),
        function (value) {
          state.definition.entry = value;
          markDirty();
        },
      ),
    );
    host.appendChild(
      selectField(
        "Menu guide opening",
        (state.definition.menu_guide && state.definition.menu_guide.entry) || "",
        [{ value: "", label: "Not set" }].concat(state.definition.steps.map(stepOption)),
        function (value) {
          if (!value) delete state.definition.menu_guide;
          else
            state.definition.menu_guide = Object.assign({}, state.definition.menu_guide, {
              entry: value,
              dismissible: true,
            });
          markDirty();
          if (state.flow === "menu") resetPreview();
        },
      ),
    );
    if (state.definition.menu_guide)
      host.appendChild(
        checkboxField(
          "Player can dismiss this guide",
          state.definition.menu_guide.dismissible !== false,
          function (value) {
            state.definition.menu_guide.dismissible = value;
            markDirty();
          },
        ),
      );
    Object.keys(state.definition.speakers || {}).forEach(function (speakerId) {
      const speaker = state.definition.speakers[speakerId],
        card = el("div", { class: "form-card" });
      card.appendChild(el("strong", {}, "Character · " + speakerId));
      const binding = speaker.faction ? "faction:" + speaker.faction : "";
      const bindingOptions = [{ value: "", label: "Custom character" }].concat(
        factionOptions().map(function (option) {
          return { value: "faction:" + option.value, label: "Roster · " + option.label };
        }),
      );
      card.appendChild(
        selectField("Name and portrait source", binding, bindingOptions, function (value) {
          delete speaker.faction;
          if (value.indexOf("faction:") === 0) {
            speaker.faction = value.slice(8);
            delete speaker.avatar;
          } else speaker.name = speaker.name || "New character";
          markDirty();
          renderSettings();
        }),
      );
      if (speaker.faction) {
        const boundFaction =
          speaker.faction &&
          state.roster.factions.find(function (faction) {
            return faction.id === speaker.faction;
          });
        const boundName = boundFaction ? boundFaction.name : speaker.faction;
        const boundAvatar = boundFaction ? boundFaction.avatar || "null" : "null";
        const boundPortrait = el("img", { class: "speaker-avatar", alt: boundName, loading: "lazy" });
        boundPortrait.src = asset("gameplay/avatars/" + boundAvatar + ".webp");
        boundPortrait.addEventListener("error", function () {
          boundPortrait.hidden = true;
        });
        card.appendChild(boundPortrait);
        card.appendChild(el("small", {}, "Uses the selected faction's name and portrait."));
        if (speaker.name_key) catalogTextField(card, "Role", speaker.name_key, 2);
        else {
          const addRole = el("button", { type: "button" }, "Identify this character's role");
          addRole.addEventListener("click", function () {
            speaker.name_key = storyKey("speaker_" + speakerId + "_role");
            markDirty();
            renderSettings();
          });
          card.appendChild(addRole);
        }
        const removeBoundSpeaker = el("button", { type: "button", class: "danger" }, "Remove character");
        removeBoundSpeaker.addEventListener("click", function () {
          if (!confirm("Remove " + speakerId + "? Their dialogue will become narrator text.")) return;
          delete state.definition.speakers[speakerId];
          state.definition.steps.forEach(function (step) {
            if (step.speaker === speakerId) delete step.speaker;
            (step.lines || []).forEach(function (line) {
              if (line.speaker === speakerId) delete line.speaker;
            });
          });
          markDirty();
          renderSettings();
          renderInspector();
        });
        card.appendChild(removeBoundSpeaker);
        host.appendChild(card);
        return;
      }
      const displayName = speaker.name_key
        ? textValue(speaker.name_key, state.previewLanguage)
        : speaker.name || "";
      const portrait = el("img", {
        class: "speaker-avatar",
        alt: displayName || speakerId,
        loading: "lazy",
        hidden: !speaker.avatar,
      });
      portrait.addEventListener("error", function () {
        portrait.hidden = true;
      });
      if (speaker.name_key) {
        catalogTextField(card, "Character name", speaker.name_key, 2);
        const fixedName = el("button", { type: "button" }, "Use one name for every language");
        fixedName.addEventListener("click", function () {
          speaker.name = textValue(speaker.name_key, state.previewLanguage) || speaker.name || speakerId;
          delete speaker.name_key;
          markDirty();
          renderSettings();
        });
        card.appendChild(fixedName);
      } else {
        card.appendChild(
          inputField(
            "Character name",
            speaker.name || "",
            function (value) {
              speaker.name = value;
              portrait.alt = value || speakerId;
              markDirty();
            },
            { input: true },
          ),
        );
        const localizeName = el("button", { type: "button" }, "Use a shared text key");
        localizeName.addEventListener("click", function () {
          const keyBase = storyKey("speaker_" + speakerId + "_name");
          let key = keyBase,
            suffix = 2;
          while (catalogValue("en", key)) key = keyBase + "_" + suffix++;
          speaker.name_key = key;
          delete speaker.name;
          markDirty();
          renderSettings();
        });
        card.appendChild(localizeName);
      }
      function updatePortrait(value) {
        if (value) speaker.avatar = value;
        else delete speaker.avatar;
        portrait.alt = speaker.name || speakerId;
        portrait.hidden = !speaker.avatar;
        if (speaker.avatar) portrait.src = asset("gameplay/avatars/" + speaker.avatar + ".webp");
        else portrait.removeAttribute("src");
      }
      if (speaker.avatar) portrait.src = asset("gameplay/avatars/" + speaker.avatar + ".webp");
      card.append(portrait);
      const avatarChoices = [{ value: "", label: "No portrait" }].concat(
        state.avatars.map(function (avatar) {
          return {
            value: avatar,
            label: avatar
              .split("_")
              .map(function (part) {
                return part.charAt(0).toUpperCase() + part.slice(1);
              })
              .join(" "),
          };
        }),
      );
      if (speaker.avatar && !state.avatars.includes(speaker.avatar))
        avatarChoices.push({ value: speaker.avatar, label: "Missing asset · " + speaker.avatar });
      card.appendChild(
        selectField("Portrait", speaker.avatar || "", avatarChoices, function (value) {
          updatePortrait(value);
          markDirty();
        }),
      );
      const removeSpeaker = el("button", { type: "button", class: "danger" }, "Remove character");
      removeSpeaker.addEventListener("click", function () {
        if (!confirm("Remove " + (speaker.name || speakerId) + "? Their dialogue will become narrator text."))
          return;
        delete state.definition.speakers[speakerId];
        state.definition.steps.forEach(function (step) {
          if (step.speaker === speakerId) delete step.speaker;
          (step.lines || []).forEach(function (line) {
            if (line.speaker === speakerId) delete line.speaker;
          });
        });
        markDirty();
        renderSettings();
        renderInspector();
      });
      card.appendChild(removeSpeaker);
      host.appendChild(card);
    });
    const addSpeaker = el("button", { type: "button" }, "Add character");
    addSpeaker.addEventListener("click", function () {
      let id = "character_" + (Object.keys(state.definition.speakers).length + 1);
      while (Object.prototype.hasOwnProperty.call(state.definition.speakers, id))
        id = "character_" + (Number(id.slice("character_".length)) + 1);
      state.definition.speakers[id] = { name: "New character" };
      markDirty();
      renderSettings();
      renderInspector();
    });
    host.appendChild(addSpeaker);
    const warning = el("small", {}, "Roster, map and faction positions are edited in Map.");
    host.appendChild(warning);
  }

  function portDefinitions(step) {
    if (step.type === "end") return [];
    if (step.type === "choice")
      return (step.choices || []).map(function (choice) {
        return {
          field: "choices." + choice.id + ".next",
          label: textValue(choice.label_key, state.previewLanguage) || choice.id,
          target: choice.next,
          className: "choice-wire",
        };
      });
    const outputs = [{ field: "next", label: "Next", target: step.next }];
    if (step.unavailable_next)
      outputs.push({
        field: "unavailable_next",
        label: "Partner unavailable",
        target: step.unavailable_next,
        className: "route-wire",
      });
    (step.routes || []).forEach(function (route, index) {
      const when = route.when || {};
      let label = "If condition";
      if (when.choice) {
        const decision = state.definition.steps.find(function (item) {
          return item.id === when.choice;
        });
        const answer =
          decision &&
          (decision.choices || []).find(function (item) {
            return item.id === when.equals;
          });
        label =
          "If " +
          ((answer && textValue(answer.label_key, state.previewLanguage)) || when.equals || when.choice);
      } else if (when.fact) label = "If " + when.fact.replace(/_/g, " ") + " ≥ " + String(when.gte);
      outputs.push({
        field: "routes." + index + ".next",
        label: label,
        target: route.next,
        className: "route-wire",
      });
    });
    return outputs;
  }
  function nodeSummary(step) {
    const firstLine = step.lines && step.lines[0];
    const storyText = textValue(
      (firstLine && firstLine.body_key) || step.hint_key || step.body_key,
      state.previewLanguage,
    );
    if (step.type === "scene")
      return [
        step.lines && step.lines.length > 1 ? step.lines.length + " dialogue lines" : "Story scene",
        storyText,
      ]
        .filter(Boolean)
        .join(" · ");
    if (step.type === "choice")
      return [
        (step.choices || []).length + " decision paths",
        textValue(step.body_key, state.previewLanguage),
      ]
        .filter(Boolean)
        .join(" · ");
    if (step.type === "end")
      return ["Episode ending", textValue(step.body_key, state.previewLanguage)].filter(Boolean).join(" · ");
    const trigger = step.trigger || {};
    const definition = triggerTypes.find(function (item) {
      return item.value === trigger.type;
    });
    const parts = [definition ? definition.label : "Set a condition"];
    if (trigger.type === "contact" && Array.isArray(trigger.targets))
      parts[0] += " · " + (trigger.value || 1) + " of " + trigger.targets.join(", ");
    else if (
      ["contact", "defeated", "attack", "hover", "camera_target"].includes(trigger.type) &&
      trigger.target
    )
      parts[0] += " · " + trigger.target;
    if (["structure_level", "structure_upgrade"].includes(trigger.type) && trigger.kind)
      parts[0] += " · " + trigger.kind;
    if (trigger.type === "fleet" && trigger.unit)
      parts[0] += " · " + trigger.unit + (trigger.target ? " to " + trigger.target : "");
    if (trigger.type === "resource_transfer")
      parts[0] +=
        (trigger.recipient ? " · to " + trigger.recipient : "") +
        (trigger.resources && trigger.resources.length ? " · " + trigger.resources.join(" + ") : "");
    else if (trigger.type === "ui" && typeof trigger.action === "string")
      parts[0] += " · " + trigger.action.replace(/_/g, " ");
    if (Number.isFinite(trigger.value))
      parts.push(
        trigger.type === "elapsed"
          ? trigger.value + "s"
          : (trigger.comparison === "lte" ? "≤ " : "≥ ") + trigger.value.toLocaleString(),
      );
    if (trigger.scope && trigger.type !== "troops")
      parts.push(
        { step: "this step", episode: "this episode", total: "all time" }[trigger.scope] || trigger.scope,
      );
    if (step.guide && typeof step.guide.gesture === "string" && typeof step.guide.target === "string")
      parts.push("hand: " + step.guide.gesture + " " + step.guide.target.replace(/_/g, " "));
    if (storyText) parts.push(storyText);
    return parts.join(" · ");
  }
  function renderPreviewBranchOptions() {
    const wrapper = $("#previewBranchWrap"),
      select = $("#previewBranchSelect");
    if (!wrapper || !select || !state.definition) return;
    const step = state.definition.steps.find(function (item) {
      return item.id === state.selected;
    });
    const options = step
      ? (step.routes || []).reduce(function (result, route, index) {
          const condition = route.when || {};
          const destination = state.definition.steps.find(function (item) {
            return item.id === route.next;
          });
          if (condition.choice) {
            const decision = state.definition.steps.find(function (item) {
              return item.id === condition.choice && item.type === "choice";
            });
            const answer =
              decision &&
              (decision.choices || []).find(function (item) {
                return item.id === condition.equals;
              });
            if (answer)
              result.push({
                value: String(index),
                label:
                  (translated(decision.title_key, state.previewLanguage) || decision.id) +
                  " · " +
                  (translated(answer.label_key, state.previewLanguage) || answer.id) +
                  " → " +
                  (translated(destination && destination.title_key, state.previewLanguage) || route.next),
              });
          } else if (condition.fact && Number.isFinite(condition.gte)) {
            result.push({
              value: String(index),
              label:
                condition.fact.replace(/_/g, " ") +
                " ≥ " +
                condition.gte +
                " → " +
                (translated(destination && destination.title_key, state.previewLanguage) || route.next),
            });
          }
          return result;
        }, [])
      : [];
    if (state.previewBranch && state.previewBranch.stepId !== state.selected) state.previewBranch = null;
    const selected =
      state.previewBranch &&
      options.some(function (option) {
        return option.value === String(state.previewBranch.routeIndex);
      })
        ? String(state.previewBranch.routeIndex)
        : "";
    if (state.previewBranch && !selected) state.previewBranch = null;
    wrapper.hidden = options.length === 0;
    optionList(select, [{ value: "", label: "Current preview state" }].concat(options), selected);
  }
  function previewContext(stepId) {
    const step = state.definition.steps.find(function (item) {
      return item.id === stepId;
    });
    const route =
      state.previewBranch &&
      state.previewBranch.stepId === stepId &&
      step &&
      (step.routes || [])[state.previewBranch.routeIndex];
    const when = (route && route.when) || {},
      facts = Object.assign({}, state.facts),
      choices = {};
    if (when.choice) choices[when.choice] = when.equals;
    if (when.fact) facts[when.fact] = Math.max(Number(facts[when.fact] || 0), Number(when.gte));
    return { facts: facts, choices: choices };
  }
  function renderGraph() {
    if (!state.definition) return;
    ensureLayout();
    const nodes = $("#nodes"),
      svg = $("#wires"),
      oldSelection = state.selected;
    const completedSteps = new Set(state.machine ? state.machine.state.completed : []);
    const focusedStep =
      document.activeElement && document.activeElement.dataset && document.activeElement.dataset.stepId;
    nodes.replaceChildren();
    let group = svg.querySelector("g");
    if (group) group.remove();
    group = document.createElementNS("http://www.w3.org/2000/svg", "g");
    svg.appendChild(group);
    nodes.style.transform =
      "translate(" + state.pan.x + "px," + state.pan.y + "px) scale(" + state.zoom + ")";
    group.setAttribute(
      "transform",
      "translate(" + state.pan.x + " " + state.pan.y + ") scale(" + state.zoom + ")",
    );
    const byId = Object.create(null);
    const errorsByStep = Object.create(null);
    const warningsByStep = Object.create(null);
    (state.validation.errors || []).forEach(function (issue) {
      if (!issue.step) return;
      errorsByStep[issue.step] = (errorsByStep[issue.step] || 0) + 1;
    });
    (state.validation.warnings || []).forEach(function (issue) {
      if (!issue.step) return;
      warningsByStep[issue.step] = (warningsByStep[issue.step] || 0) + 1;
    });
    state.definition.steps.forEach(function (step) {
      byId[step.id] = step;
    });
    const policyByStep = state.isMenuGuides
      ? new Map()
      : campaignStepPolicies(state.machine && state.machine.state.choices, state.facts);
    state.definition.steps.forEach(function (step) {
      const pos = state.definition.layout[step.id] || defaultLayout(0);
      const rootLabel = state.isMenuGuides
        ? Object.keys(state.definition.menu_guides).some(function (id) {
            return state.definition.menu_guides[id].entry === step.id;
          })
          ? "JOURNEY START"
          : step.id
        : step.id === state.definition.entry
          ? "IN-GAME START"
          : state.definition.menu_guide && step.id === state.definition.menu_guide.entry
            ? "MENU START"
            : step.id;
      const stepErrors = errorsByStep[step.id] || 0;
      const stepWarnings = warningsByStep[step.id] || 0;
      const lines = Array.isArray(step.exchange)
        ? step.exchange
        : Array.isArray(step.lines)
          ? step.lines
          : [];
      const speakers = state.definition.speakers || {};
      const castIds = Array.from(
        new Set(
          lines
            .map(function (line) {
              return (line && line.speaker) || step.speaker;
            })
            .concat(step.speaker || [])
            .filter(Boolean),
        ),
      );
      const castNames = castIds.map(function (speakerId) {
        const speaker = speakers[speakerId] || {};
        return speaker.name_key
          ? textValue(speaker.name_key, state.previewLanguage)
          : speaker.name || speakerId;
      });
      const summary = nodeSummary(step);
      const menuOpening =
        state.isMenuGuides &&
        Object.keys(state.definition.menu_guides).some(function (id) {
          return state.definition.menu_guides[id].entry === step.id;
        });
      const card = el("article", {
        class:
          "node" +
          (step.id === oldSelection ? " selected" : "") +
          (step.id === state.playingStep ? " playing" : "") +
          (completedSteps.has(step.id) ? " visited" : "") +
          (step.id === state.definition.entry ? " is-entry" : "") +
          (menuOpening || (state.definition.menu_guide && step.id === state.definition.menu_guide.entry)
            ? " is-menu-entry"
            : "") +
          (stepErrors ? " has-error" : "") +
          (stepWarnings ? " has-warning" : ""),
        dataset: { stepId: step.id, stepType: step.type },
        tabindex: "0",
        role: "group",
        "aria-label":
          step.type +
          ": " +
          (translated(step.title_key, state.previewLanguage) || step.id) +
          " · " +
          summary +
          (castNames.length ? " · " + castNames.join(", ") : ""),
      });
      card.style.left = pos.x + "px";
      card.style.top = pos.y + "px";
      const head = el("header", { class: "node-hd" });
      head.append(
        el("span", {}, step.type),
        el("small", { title: step.id, "aria-label": step.id }, rootLabel),
      );
      card.appendChild(head);
      card.appendChild(
        el(
          "div",
          { class: "node-title" },
          translated(step.title_key, state.previewLanguage) || step.title_key || "Untitled",
        ),
      );
      card.appendChild(el("div", { class: "node-summary", title: summary }, summary));
      const effectiveUnlocks = state.isMenuGuides
        ? null
        : window.SOWCampaign.applyCampaignUnlocks(policyByStep.get(step.id), step.unlocks);
      const availability = state.isMenuGuides ? "" : campaignUnlockLabel(effectiveUnlocks);
      if (availability) card.appendChild(el("div", { class: "node-summary" }, "Available: " + availability));
      if (castIds.length) {
        const cast = el("div", { class: "node-cast", "aria-hidden": "true" });
        castIds.slice(0, 3).forEach(function (speakerId) {
          const speaker = speakers[speakerId] || {};
          const name = speaker.name_key
            ? textValue(speaker.name_key, state.previewLanguage)
            : speaker.name || speakerId;
          const badge = el("span", { class: "node-cast__member", title: name });
          if (speaker.avatar) {
            const portrait = el("img", {
              src: asset("gameplay/avatars/" + speaker.avatar + ".webp"),
              alt: "",
              loading: "lazy",
            });
            portrait.addEventListener("error", function () {
              badge.textContent = String(name || speakerId)
                .trim()
                .charAt(0)
                .toUpperCase();
              portrait.remove();
            });
            badge.appendChild(portrait);
          } else
            badge.textContent = String(name || speakerId)
              .trim()
              .charAt(0)
              .toUpperCase();
          cast.appendChild(badge);
        });
        if (castIds.length > 3)
          cast.appendChild(el("span", { class: "node-cast__more" }, "+" + (castIds.length - 3)));
        card.appendChild(cast);
      }
      if (stepErrors)
        card.appendChild(
          el(
            "div",
            { class: "node-error", role: "img", "aria-label": stepErrors + " validation errors" },
            stepErrors + (stepErrors === 1 ? " issue" : " issues"),
          ),
        );
      if (stepWarnings)
        card.appendChild(
          el(
            "div",
            { class: "node-warning", role: "img", "aria-label": stepWarnings + " validation warnings" },
            stepWarnings + (stepWarnings === 1 ? " note" : " notes"),
          ),
        );
      const input = el("button", {
        type: "button",
        class: "pin in",
        title: "Connect here",
        "aria-label": "Connect a path to " + (translated(step.title_key, state.previewLanguage) || step.id),
        dataset: { in: step.id },
      });
      input.addEventListener("keydown", connectPinByKeyboard);
      card.appendChild(input);
      portDefinitions(step).forEach(function (port, index) {
        const row = el("div", { class: "port-row" + (port.className ? " " + port.className : "") });
        row.appendChild(el("span", { class: "port-label", title: port.label }, port.label));
        const destination = byId[port.target];
        row.appendChild(
          el(
            "small",
            { title: destination ? stepLabel(destination) : "Unconnected" },
            destination ? stepLabel(destination) : "Unconnected",
          ),
        );
        const output = el("button", {
          type: "button",
          class: "pin out",
          title: "Drag to connect",
          "aria-label":
            "Connect " +
            port.label +
            " from " +
            (translated(step.title_key, state.previewLanguage) || step.id),
          dataset: { out: step.id, field: port.field, outputIndex: index },
        });
        output.addEventListener("keydown", connectPinByKeyboard);
        row.appendChild(output);
        card.appendChild(row);
      });
      card.addEventListener("click", function (event) {
        if (event.target.closest(".pin")) return;
        selectStep(step.id);
      });
      card.addEventListener("keydown", function (event) {
        if (event.target !== card || (event.key !== "Enter" && event.key !== " ")) return;
        event.preventDefault();
        selectStep(step.id);
      });
      head.addEventListener("pointerdown", function (event) {
        if (event.button !== 0) return;
        event.preventDefault();
        state.drag = {
          kind: "node",
          id: step.id,
          x: event.clientX,
          y: event.clientY,
          origin: { x: pos.x, y: pos.y },
        };
        head.setPointerCapture(event.pointerId);
      });
      card.addEventListener("pointerdown", function (event) {
        const output = event.target.closest(".pin.out");
        if (!output) return;
        event.preventDefault();
        event.stopPropagation();
        state.keyboardWire = null;
        $("#stage").dataset.connecting = "false";
        $(".graph-hint").textContent =
          "Drag outputs to inputs · keyboard: Enter on output, then Enter on destination · Escape cancels";
        state.wire = {
          source: output.dataset.out,
          field: output.dataset.field,
          outputIndex: Number(output.dataset.outputIndex),
        };
        output.setPointerCapture(event.pointerId);
      });
      nodes.appendChild(card);
    });
    if (focusedStep) {
      const focusTarget = $("#nodes [data-step-id=" + CSS.escape(focusedStep) + "]");
      if (focusTarget) focusTarget.focus({ preventScroll: true });
    }
    drawWires();
    $("#stepCount").textContent = state.definition.steps.length + " steps";
    $("#episodeLabel").textContent = state.isMenuGuides
      ? "Menu guides"
      : episodeId.replace(/_/g, " ").toUpperCase();
    renderPreviewBranchOptions();
  }
  function selectStep(id) {
    state.selected = id;
    renderInspector();
    renderGraph();
    renderPreview(id);
    revealStep(id);
  }
  function revealStep(id) {
    const stage = $("#stage"),
      node = $("#nodes [data-step-id=" + CSS.escape(id) + "]");
    if (!node) return;
    const frame = stage.getBoundingClientRect(),
      rect = node.getBoundingClientRect(),
      margin = 36;
    let dx = 0,
      dy = 0;
    if (rect.left < frame.left + margin) dx = frame.left + margin - rect.left;
    else if (rect.right > frame.right - margin) dx = frame.right - margin - rect.right;
    if (rect.top < frame.top + margin) dy = frame.top + margin - rect.top;
    else if (rect.bottom > frame.bottom - margin) dy = frame.bottom - margin - rect.bottom;
    if (dx || dy) {
      state.pan.x += dx;
      state.pan.y += dy;
      renderGraph();
    }
  }
  function setDestination(wire, destination) {
    const source = state.definition.steps.find(function (step) {
      return step.id === wire.source;
    });
    if (!source) return;
    const choiceMatch = wire.field.match(/^choices\.([^.]+)\.next$/);
    if (choiceMatch) {
      const choice = (source.choices || []).find(function (item) {
        return item.id === choiceMatch[1];
      });
      if (choice) choice.next = destination;
    } else setPath(source, wire.field, destination);
  }
  function connectPinByKeyboard(event) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    const pin = event.currentTarget;
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
    const destination = pin.dataset.in;
    setDestination(state.keyboardWire, destination);
    state.keyboardWire = null;
    $("#stage").dataset.connecting = "false";
    $(".graph-hint").textContent =
      "Drag outputs to inputs · keyboard: Enter on output, then Enter on destination · Escape cancels";
    markDirty();
    const target = Array.from($("#nodes").querySelectorAll(".pin.in")).find(function (candidate) {
      return candidate.dataset.in === destination;
    });
    if (target) target.focus({ preventScroll: true });
  }
  function drawWires() {
    const svg = $("#wires"),
      group = svg.querySelector("g");
    if (!group) return;
    group.replaceChildren();
    const trace = (state.machine && state.machine.state.completed) || [];
    const currentStep = state.machine && state.machine.state.id;
    const stageRect = $("#stage").getBoundingClientRect();
    state.definition.steps.forEach(function (step) {
      const node = $("#nodes [data-step-id=" + CSS.escape(step.id) + "]");
      if (!node) return;
      portDefinitions(step).forEach(function (port, index) {
        const target = port.target && $("#nodes [data-step-id=" + CSS.escape(port.target) + "]");
        if (!target) return;
        const output = node.querySelectorAll(".pin.out")[index],
          input = target.querySelector(".pin.in");
        if (!output || !input) return;
        const a = output.getBoundingClientRect(),
          b = input.getBoundingClientRect();
        const x1 = (a.left + a.width / 2 - stageRect.left - state.pan.x) / state.zoom;
        const y1 = (a.top + a.height / 2 - stageRect.top - state.pan.y) / state.zoom;
        const x2 = (b.left + b.width / 2 - stageRect.left - state.pan.x) / state.zoom;
        const y2 = (b.top + b.height / 2 - stageRect.top - state.pan.y) / state.zoom;
        const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
        path.setAttribute(
          "d",
          "M" +
            x1 +
            " " +
            y1 +
            " C" +
            (x1 + 70) +
            " " +
            y1 +
            " " +
            (x2 - 70) +
            " " +
            y2 +
            " " +
            x2 +
            " " +
            y2,
        );
        if (port.className) path.classList.add(port.className);
        const traceIndex = trace.indexOf(step.id);
        if (
          traceIndex >= 0 &&
          (trace[traceIndex + 1] === port.target ||
            (traceIndex === trace.length - 1 && currentStep === port.target))
        )
          path.classList.add("path-traversed");
        group.appendChild(path);
      });
    });
  }
  function addStep(type, branch) {
    if (state.isMenuGuides) {
      addMenuGuideStep(type);
      return;
    }
    const index = state.definition.steps.length,
      id = uniqueId(type + "_" + (index + 1), state.definition.steps),
      previous = branch
        ? branch.step
        : state.definition.steps.find(function (step) {
            return step.id === state.selected;
          });
    if (!branch && previous && previous.type === "choice") {
      notice("Choose the answer to branch from in the inspector.");
      return;
    }
    const ending = state.definition.steps.find(function (item) {
      return item.type === "end";
    });
    const output = branch ? branch.output : previous && previous.type !== "end" ? previous : null;
    const previousDestination = (output && output.next) || "";
    const next = previousDestination || (ending && ending.id) || "";
    if (
      type === "end" &&
      previousDestination &&
      !confirm(
        "This path already continues to " + previousDestination + ". Replace that route with an ending?",
      )
    )
      return;
    let position = defaultLayout(index);
    const origin = previous && state.definition.layout[previous.id];
    if (output && origin) {
      position = { x: origin.x + 320, y: origin.y };
      const newHeight = type === "choice" ? 230 : type === "objective" || type === "guide" ? 190 : 160;
      const occupiedNodes = state.definition.steps.map(function (item, itemIndex) {
        const point = state.definition.layout[item.id] || defaultLayout(itemIndex);
        const node = $("#nodes [data-step-id=" + CSS.escape(item.id) + "]");
        return {
          point: point,
          width: (node && node.offsetWidth) || 224,
          height: (node && node.offsetHeight) || 160,
        };
      });
      for (let attempt = 0; attempt < state.definition.steps.length; attempt++) {
        const collision = occupiedNodes.find(function (occupied) {
          return (
            position.x < occupied.point.x + occupied.width + 32 &&
            position.x + 256 > occupied.point.x &&
            position.y < occupied.point.y + occupied.height + 32 &&
            position.y + newHeight + 32 > occupied.point.y
          );
        });
        if (!collision) break;
        position.y = collision.point.y + collision.height + 40;
      }
    }
    const menuAction = inGameUiTargets()[0] || "menu_campaign";
    const step = {
      id: id,
      type: type,
      title_key: storyKey(id + "_title"),
      body_key: storyKey(id + "_body"),
      speaker: Object.keys(state.definition.speakers)[0],
    };
    if (type === "end") {
      delete step.body_key;
    } else if (type === "choice") {
      step.choices = [
        { id: "first", label_key: storyKey(id + "_first"), next: next },
        { id: "second", label_key: storyKey(id + "_second"), next: next },
      ];
    } else if (type === "objective" || type === "guide") {
      delete step.body_key;
      step.hint_key = storyKey(id + "_hint");
      step.trigger =
        state.flow === "menu"
          ? { type: "ui", action: menuAction, scope: "step" }
          : { type: "territory", value: 1, scope: "step" };
      if (type === "guide")
        step.guide =
          state.flow === "menu"
            ? { kind: "ui", target: menuAction, gesture: "tap" }
            : { kind: "world", target: "expand", gesture: "tap" };
      if (state.flow !== "menu") step.marker = { target: "player" };
      step.next = next;
    } else step.next = next;
    if (output) output.next = id;
    state.definition.steps.push(step);
    state.definition.layout[id] = position;
    if (type === "end")
      state.definition.steps.forEach(function (item) {
        if (item.type !== "end" && !item.next && item.type !== "choice") item.next = id;
      });
    markDirty();
    selectStep(id);
    revealStep(id);
  }
  function addMenuGuideStep(type) {
    const index = state.definition.steps.length,
      id = uniqueId(type + "_" + (index + 1), state.definition.steps);
    const previous = state.definition.steps.find(function (step) {
      return step.id === state.selected;
    });
    if (previous && previous.type === "choice") {
      notice("Choose an answer path in the inspector first.");
      return;
    }
    const ending = state.definition.steps.find(function (step) {
      return step.type === "end";
    });
    const next =
      previous && previous.type !== "end"
        ? previous.next || (ending && ending.id) || ""
        : (ending && ending.id) || "";
    let position = defaultLayout(index);
    const origin = previous && state.definition.layout[previous.id];
    if (origin) position = { x: origin.x + 320, y: origin.y };
    const step = { id: id, type: type, title_key: "menu.guide." + id + ".title" };
    if (type === "end") step.body_key = "menu.guide." + id + ".body";
    else if (type === "scene") {
      step.body_key = "menu.guide." + id + ".body";
      step.next = next;
    } else if (type === "choice")
      step.choices = [
        { id: "first", label_key: "menu.guide." + id + ".first", next: next },
        { id: "second", label_key: "menu.guide." + id + ".second", next: next },
      ];
    else {
      const outcome = menuGuideOutcomeOptions()[0].value,
        control = window.SOWCampaign.MENU_GUIDE_OUTCOMES[outcome].control;
      step.hint_key = "menu.guide." + id + ".hint";
      step.trigger = { type: "ui", action: control, outcome: outcome, scope: "step" };
      step.guide = { kind: "ui", target: control, gesture: "tap" };
      step.next = next;
    }
    if (previous && previous.type !== "end" && previous.type !== "choice") previous.next = id;
    state.definition.steps.push(step);
    state.definition.layout[id] = position;
    ["en", "es"].forEach(function (locale) {
      state.definition.strings[locale] = state.definition.strings[locale] || {};
    });
    state.selected = id;
    markDirty();
    renderPalette();
    renderInspector();
    renderGraph();
    resetPreview();
    revealStep(id);
  }
  function renderPalette() {
    const host = $("#palette");
    host.replaceChildren();
    (state.isMenuGuides ? ["scene", "choice", "guide", "end"] : types).forEach(function (type) {
      const button = el(
        "button",
        { type: "button" },
        type === "scene"
          ? "Scene"
          : type === "choice"
            ? "Decision"
            : type === "objective"
              ? "Objective"
              : type === "guide"
                ? "Guide"
                : "Ending",
      );
      button.addEventListener("click", function () {
        addStep(type);
      });
      host.appendChild(button);
    });
  }

  function paintPreview(updateMachine) {
    if (!state.machine || state.validation.errors.length) return;
    clearPreviewRevealTimer();
    $("#previewFrame").dataset.device = $("#device").value;
    state.facts.touch_controls = $("#device").value === "mobile" ? 1 : 0;
    if (state.isMenuGuides) syncMenuPreviewFacts();
    let model;
    const previousStep = state.playingStep;
    try {
      model = updateMachine === false ? state.machine.view() : state.machine.update(state.facts, state.ui);
    } catch (error) {
      $("#previewStatus").textContent = "Preview unavailable: " + error.message;
      return;
    }
    const acceptance = model.step.campaign_accept_alliance_on_enter;
    if (acceptance && (state.facts.alliance_request_faction_ids || []).includes(acceptance.faction_id)) {
      state.facts.alliance_faction_ids = Array.from(
        new Set((state.facts.alliance_faction_ids || []).concat(acceptance.faction_id)),
      );
      state.facts.alliance_request_faction_ids = state.facts.alliance_request_faction_ids.filter(
        function (id) {
          return id !== acceptance.faction_id;
        },
      );
      model = state.machine.update(state.facts, state.ui);
    }
    paintExchangePreview(model);
    state.playingStep = model.step.id;
    const transferPanel = $("#sow-hud-transfer");
    if (transferPanel && model.step.id === "boudica_transfer_send") transferPanel.hidden = false;
    else if (transferPanel && previousStep === "boudica_transfer_send") transferPanel.hidden = true;
    const actionRatio = Number.isFinite(model.step.attack_ratio_on_enter)
      ? model.step.attack_ratio_on_enter
      : null;
    if (model.step.id !== state.previewActionStep || actionRatio !== state.previewActionRatio) {
      state.previewActionStep = model.step.id;
      state.previewActionRatio = actionRatio;
      if (actionRatio != null) $("#sow-hud-slider").value = String(Math.round(actionRatio * 100));
    }
    const menuControl = state.isMenuGuides
      ? window.SOWCampaign.menuGuideControl(model.step, state.facts)
      : null;
    const anchorStep = menuControl
      ? Object.assign({}, model.step, {
          guide: Object.assign({}, model.step.guide, { target: menuControl.target }),
        })
      : model.step;
    const anchor = previewAnchor(anchorStep, state.facts);
    const previewPlatform =
      (window.navigator && window.navigator.userAgentData && window.navigator.userAgentData.platform) ||
      (window.navigator && window.navigator.platform) ||
      "";
    const zoomMode =
      $("#device").value === "mobile"
        ? "pinch"
        : window.SOWCampaign.zoomInputMode({ platform: previewPlatform });
    const zoomGuide = model.step.guide && ["zoom_in", "zoom_out"].includes(model.step.guide.gesture);
    const cameraCue =
      model.step.camera_only === true &&
      model.step.guide &&
      ["zoom_in", "zoom_out", "drag", "hover"].includes(model.step.guide.gesture);
    const deviceHint =
      !cameraCue && model.step.guide && ["drag", "hover"].includes(model.step.guide.gesture)
        ? translated(
            stepTextKey(model.step, "_" + ($("#device").value === "mobile" ? "mobile" : "desktop") + "_hint"),
            state.previewLanguage,
          )
        : null;
    let previewHint = deviceHint || "",
      guideMetric = null,
      gestureLabel = null;
    const guideAction = model.step.guide ? window.SOWCampaign.guideAction(model.step) : "";
    const statusOnly =
      model.step.type === "objective" && (!model.step.guide || guideAction === "upgrade_progress");
    if (model.step.guide && model.step.guide.gesture === "tap" && model.step.guide.show_label !== false) {
      const expansionResumed =
        model.step.paused_action === "expand_once_then_resume" && model.paused_action == null;
      const actionSuffix = expansionResumed
        ? "_continue_action"
        : "_" + (zoomMode === "pinch" ? "mobile" : "desktop") + "_action";
      const actionKey = stepTextKey(model.step, actionSuffix),
        actionText = textValue(actionKey, state.previewLanguage);
      const fallbackKey =
        guideAction === "upgrade_progress"
          ? "tutorial.hand_upgrade_progress"
          : guideAction === "interact"
            ? "tutorial.hand_" + (zoomMode === "pinch" ? "tap" : "click")
            : "tutorial.hand_" + (zoomMode === "pinch" ? "tap" : "click") + "_" + guideAction;
      gestureLabel = actionText || translated(fallbackKey, state.previewLanguage);
    }
    if (zoomGuide && !cameraCue) {
      const zoomModeKey = stepTextKey(model.step, "_" + zoomMode);
      previewHint = textValue(zoomModeKey + "_hint", state.previewLanguage) || "";
      gestureLabel =
        textValue(zoomModeKey + "_label", state.previewLanguage) ||
        translated(
          model.step.guide.gesture === "zoom_in" ? "hud.zoom_in" : "hud.zoom_out",
          state.previewLanguage,
        );
      const zoomTarget =
        model.step.trigger && model.step.trigger.type === "zoom_out_complete"
          ? window.SOWCampaign.zoomOutTarget(state.facts, model.step.trigger.value)
          : model.step.trigger && model.step.trigger.type === "zoom_in_complete"
            ? state.facts.camera_zoom_target
            : null;
      if (
        zoomTarget != null &&
        Number.isFinite(Number(zoomTarget)) &&
        Number.isFinite(Number(state.facts.camera_zoom))
      ) {
        guideMetric = {
          current: Number(state.facts.camera_zoom),
          target: Number(zoomTarget),
          direction: model.step.guide.gesture === "zoom_in" ? "min" : "max",
        };
      }
    } else if (
      !cameraCue &&
      !zoomGuide &&
      model.step.trigger &&
      model.step.trigger.type === "camera_target" &&
      Number.isFinite(Number(model.step.trigger.distance)) &&
      Number.isFinite(Number(state.facts.camera_target_distance))
    ) {
      guideMetric = {
        text: previewMetric(stepTextKey(model.step, "_progress"), {
          current: Math.ceil(Number(state.facts.camera_target_distance)),
          target: model.step.trigger.distance,
        }),
      };
    }
    if (menuControl) gestureLabel = translated(menuControl.hint_key, state.previewLanguage);
    try {
      state.renderer.render(model.waiting ? null : model, {
        hideObjective: state.isMenuGuides,
        anchor: anchor,
        reducedMotion: $("#reducedMotion").checked,
        direction: rtlLanguages.has(state.previewLanguage.toLowerCase().split("-")[0]) ? "rtl" : "ltr",
        localeScript: previewLocaleScript(state.previewLanguage),
        zoomMode: zoomMode,
        hintOverride: previewHint,
        guideMetric: guideMetric,
        gestureLabel: gestureLabel,
        gestureOverride:
          model.step.trigger && model.step.trigger.action === "map_menu_open"
            ? zoomMode === "pinch"
              ? "hold"
              : "tap"
            : null,
        statusOnly: statusOnly,
        statusLabel: statusOnly ? gestureLabel : null,
      });
    } catch (error) {
      $("#previewStatus").textContent = "Preview unavailable: " + error.message;
      return;
    }
    $("#engineState").textContent = JSON.stringify(
      state.isMenuGuides
        ? {
            journey: state.menuGuideId,
            step: model.step.id,
            type: model.step.type,
            progress: model.progress,
            facts: state.facts,
          }
        : {
            step: model.step.id,
            type: model.step.type,
            waiting: model.waiting,
            wait_remaining_ms: model.wait_remaining_ms,
            unlocks: model.unlocks,
            campaign_assault_on_enter: model.step.campaign_assault_on_enter || null,
            reaction: model.reaction || null,
            exchange: model.exchange || null,
            exchange_phase: model.exchange_phase || null,
            progress: model.progress,
            choices: model.state.choices,
            reactionsShown: model.state.reactionsShown,
          },
      null,
      2,
    );
    const guideTarget = model.step.guide && model.step.guide.kind === "ui" ? model.step.guide.target : "";
    const requiredMenu = /^map_(?:build|upgrade)_/.test(guideTarget) ? "build" : "";
    let menuHint =
      requiredMenu && $("#sow-hud").dataset.previewMapMenu !== requiredMenu
        ? " · open the " + requiredMenu + " submenu in the preview to reveal this guide"
        : "";
    if (!menuHint && guideTarget === "campaign_replay") {
      const replayTarget = window.SOWCampaignView.resolveUiTarget("campaign_replay", document, episodeId);
      if (!replayTarget || !replayTarget.getClientRects().length)
        menuHint = " · open Campaign in the preview to reveal Replay";
    }
    const previewTitle =
      translated(model.step.title_key, state.previewLanguage) || model.step.id.replace(/_/g, " ");
    const assaultHint = model.step.campaign_assault_on_enter
      ? " · on entry: " +
        model.step.campaign_assault_on_enter.attacker_team +
        " attacks " +
        (model.step.campaign_assault_on_enter.target === "player"
          ? "player"
          : model.step.campaign_assault_on_enter.target === "player_and_allies"
            ? "player and current allies"
            : factionName(model.step.campaign_assault_on_enter.target)) +
        (model.step.campaign_assault_on_enter.hold_last_tile ? " · hold at one tile" : "")
      : "";
    const exchangeText =
      model.exchange &&
      ((model.exchange.presentation || "emote") === "dialogue"
        ? translated(model.exchange.body_key, state.previewLanguage)
        : translated(model.exchange.emote_text_key, state.previewLanguage));
    $("#previewStatus").textContent =
      (state.demoBackup ? "Sample preview · not saved — " : "Previewing · ") +
      previewTitle +
      (model.exchange
        ? " · " +
          model.exchange.speaker +
          ": " +
          exchangeText +
          " · " +
          model.exchange_phase +
          " · " +
          (model.wait_remaining_ms / 1000).toFixed(1) +
          "s"
        : model.waiting
          ? " · appears in " + (model.wait_remaining_ms / 1000).toFixed(1) + "s"
          : "") +
      menuHint +
      assaultHint +
      (state.validation.errors.length ? " · draft needs fixes before saving" : "");
    if (model.waiting) {
      const waitingMachine = state.machine,
        waitingStepId = model.step.id,
        waitingToken = state.previewRevealToken;
      const timerId = window.setTimeout(
        function () {
          if (state.previewRevealToken !== waitingToken || state.previewRevealTimer !== timerId) return;
          state.previewRevealTimer = null;
          if (state.machine === waitingMachine && waitingMachine.view().step.id === waitingStepId)
            paintPreview();
        },
        Math.max(1, model.wait_remaining_ms),
      );
      state.previewRevealTimer = timerId;
    }
    if (previousStep !== model.step.id) renderGraph();
    renderFactControls(model);
  }
  function paintExchangePreview(model) {
    document.querySelectorAll(".campaign-preview-emote").forEach(function (node) {
      node.remove();
    });
    document.querySelectorAll(".sample-map-marker.is-speaking").forEach(function (node) {
      node.classList.remove("is-speaking");
    });
    const line = model.exchange;
    if (!line || model.exchange_phase !== "speaking" || (line.presentation || "emote") !== "emote") return;
    const speaker = state.definition.speakers && state.definition.speakers[line.speaker];
    const marker =
      speaker && speaker.faction
        ? Array.from(document.querySelectorAll("#campaignPreviewMarkers .sample-faction")).find(
            function (item) {
              return item.dataset.factionId === speaker.faction;
            },
          )
        : document.querySelector("#campaignPreviewMarkers .player-base");
    if (!marker) return;
    marker.appendChild(
      el(
        "span",
        { class: "campaign-preview-emote", role: "status" },
        translated(line.emote_text_key, state.previewLanguage),
      ),
    );
    marker.classList.add("is-speaking");
  }
  function previewLocaleScript(locale) {
    const code = String(locale || "").toLowerCase();
    if (code === "ar") return "arabic";
    if (["zh-cn", "ja", "ko"].includes(code)) return "cjk";
    return code === "ru" ? "cyrillic" : "latin";
  }
  function syncPreview() {
    if (state.validation.errors.length) {
      $("#previewStatus").textContent = "Fix validation errors to preview this draft";
      return;
    }
    if (state.machine && state.machine.replaceDefinition(state.definition)) paintPreview();
    else renderPreview();
  }
  function previewAnchor(step, facts) {
    const frame = $("#previewFrame");
    frame.querySelectorAll(".is-guide-target").forEach(function (marker) {
      marker.classList.remove("is-guide-target");
    });
    const passiveStatus =
      step.type === "objective" &&
      (!step.guide || window.SOWCampaign.guideAction(step) === "upgrade_progress");
    if (!step.guide || passiveStatus) {
      if (!passiveStatus) return null;
      const playerMarker = previewWorldMarker(frame, "player", step) || frame.querySelector(".player-base");
      if (!playerMarker || playerMarker.getClientRects().length === 0) return null;
      const playerRect = playerMarker.getBoundingClientRect(),
        frameRect = frame.getBoundingClientRect();
      return {
        x: playerRect.left + playerRect.width / 2 - frameRect.left,
        y: playerRect.top + playerRect.height / 2 - frameRect.top,
      };
    }
    const frameRect = frame.getBoundingClientRect(),
      guide = step.guide;
    if (["zoom_in", "zoom_out"].includes(guide.gesture))
      return { x: frame.clientWidth * 0.5, y: frame.clientHeight * 0.5 };
    let target;
    if (guide.kind === "world") target = previewWorldMarker(frame, guide.target, step);
    else {
      target = window.SOWCampaignView.resolveUiTarget(guide.target, document, episodeId, facts);
      if (target && !frame.contains(target)) target = null;
    }
    if (!target || target.getClientRects().length === 0) return null;
    if (guide.kind === "world") target.classList.add("is-guide-target");
    const rect = target.getBoundingClientRect();
    const spotlightPanel = step.id === "boudica_transfer_send" ? target.closest("#sow-hud-transfer") : null;
    if (
      step.id === "boudica_transfer_send" &&
      (!spotlightPanel || spotlightPanel.getClientRects().length === 0)
    )
      return null;
    let anchor =
      guide.kind === "ui"
        ? window.SOWCampaignView.resolveUiAnchor(target, spotlightPanel)
        : { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
    const anchorRect = guide.kind === "ui" ? $("#previewRoot").getBoundingClientRect() : frameRect;
    if (
      guide.kind === "ui" &&
      guide.target === "attack_ratio" &&
      step.trigger &&
      step.trigger.action === "attack_ratio" &&
      target.type === "range"
    ) {
      const rangeMin = Number(target.min),
        rangeMax = Number(target.max);
      const rangeTarget = rangeMin + (rangeMax - rangeMin) * Number(step.trigger.value || 1);
      anchor = window.SOWCampaignView.resolveUiRangeGuideAnchor(target, Number(target.value), rangeTarget);
      if (!anchor) return null;
      anchor.toX -= anchorRect.left;
      anchor.toY -= anchorRect.top;
    }
    anchor.x -= anchorRect.left;
    anchor.y -= anchorRect.top;
    if (Number.isFinite(anchor.spotlightX)) anchor.spotlightX -= anchorRect.left;
    if (Number.isFinite(anchor.spotlightY)) anchor.spotlightY -= anchorRect.top;
    if (guide.gesture === "drag" && guide.to) {
      let end;
      if (guide.kind === "world") end = previewWorldMarker(frame, guide.to, step);
      else {
        end = window.SOWCampaignView.resolveUiTarget(guide.to, document, episodeId);
        if (end && !frame.contains(end)) end = null;
      }
      if (!end || end.disabled || !end.getClientRects().length) return null;
      const endAnchor =
        guide.kind === "ui" ? window.SOWCampaignView.resolveUiAnchor(end) : end.getBoundingClientRect();
      anchor.toX =
        (guide.kind === "ui" ? endAnchor.x : endAnchor.left + endAnchor.width / 2) - anchorRect.left;
      anchor.toY =
        (guide.kind === "ui" ? endAnchor.y : endAnchor.top + endAnchor.height / 2) - anchorRect.top;
    }
    return anchor;
  }
  function previewWorldMarker(frame, target, step) {
    if (target === "expand") return frame.querySelector(".player-base");
    const factions = (state.roster && state.roster.factions) || [];
    if (target === "nameplate") {
      const nameplateTarget = (step.marker && step.marker.target) || (step.trigger && step.trigger.target);
      const nameplateFaction = factions.find(function (item) {
        return item.id === nameplateTarget;
      });
      return (
        (nameplateFaction &&
          Array.from(frame.querySelectorAll(".sample-faction")).find(function (marker) {
            return marker.dataset.factionId === nameplateFaction.id;
          })) ||
        null
      );
    }
    if (target === "player") {
      const playerTarget =
        step.trigger && ["camera_target", "hover"].includes(step.trigger.type) && step.trigger.target
          ? step.trigger.target
          : step.marker && step.marker.target;
      if (!playerTarget || playerTarget === "player") return frame.querySelector(".player-base");
      const playerFaction = factions.find(function (item) {
        return item.id === playerTarget;
      });
      return (
        (playerFaction &&
          Array.from(frame.querySelectorAll(".sample-faction")).find(function (marker) {
            return marker.dataset.factionId === playerFaction.id;
          })) ||
        null
      );
    }
    const observed =
      step.trigger && step.trigger.type === "contact"
        ? (state.facts && state.facts.contact_faction_ids) || []
        : (state.facts && state.facts.defeated_faction_ids) || [];
    const requested =
      target === "target_action" &&
      (Array.isArray(step.trigger && step.trigger.targets)
        ? step.trigger.targets.find(function (name) {
            return !observed.includes(name);
          })
        : (step.trigger && step.trigger.target) ||
          (step.marker && step.marker.target !== "player" && step.marker.target));
    let faction =
      requested &&
      factions.find(function (item) {
        return item.id === requested;
      });
    if (!faction && state.roster) {
      const spawn = state.roster.player_spawn;
      faction = factions
        .filter(function (item) {
          return item.relation !== "allied";
        })
        .sort(function (a, b) {
          return Math.hypot(a.x - spawn[0], a.y - spawn[1]) - Math.hypot(b.x - spawn[0], b.y - spawn[1]);
        })[0];
    }
    return (
      (faction &&
        Array.from(frame.querySelectorAll(".sample-faction")).find(function (marker) {
          return marker.dataset.factionId === faction.id;
        })) ||
      null
    );
  }
  function focusPreviewTarget() {
    if (state.flow !== "episode" || !state.machine) return;
    state.previewFocusActive = true;
    applyPreviewFocus();
    paintPreview(false);
  }
  function applyPreviewFocus() {
    if (!state.previewFocusActive || !state.machine) return;
    const step = state.machine.view().step,
      observed =
        step.trigger && step.trigger.type === "contact"
          ? (state.facts && state.facts.contact_faction_ids) || []
          : (state.facts && state.facts.defeated_faction_ids) || [];
    const target =
      step.guide && step.guide.kind === "world" && step.guide.target === "target_action"
        ? Array.isArray(step.trigger && step.trigger.targets)
          ? step.trigger.targets.find(function (name) {
              return !observed.includes(name);
            })
          : (step.trigger && step.trigger.target) || (step.marker && step.marker.target)
        : step.marker && step.marker.target;
    const marker =
      target === "player"
        ? $("#campaignPreviewMarkers .player-base")
        : target
          ? Array.from(document.querySelectorAll("#campaignPreviewMarkers .sample-faction")).find(
              function (item) {
                return item.dataset.factionId === target;
              },
            )
          : null;
    if (!marker) return;
    const layer = $("#campaignPreviewLayer"),
      scale = 1.8;
    layer.classList.toggle("has-map-guide", Boolean(step.guide));
    const x = (layer.offsetWidth * parseFloat(marker.style.left)) / 100 - layer.offsetWidth / 2;
    const y = (layer.offsetHeight * parseFloat(marker.style.top)) / 100 - layer.offsetHeight / 2;
    layer.style.setProperty("--map-focus-x", -x * scale + "px");
    layer.style.setProperty("--map-focus-y", -y * scale + "px");
    layer.style.setProperty("--map-focus-scale", scale);
    $("#campaignPreviewMarkers .is-camera-focus")?.classList.remove("is-camera-focus");
    marker.classList.add("is-camera-focus");
  }
  function renderPreview(forceStep) {
    if (!state.definition) return;
    if (state.validation.errors.length) {
      $("#previewStatus").textContent = "Fix validation errors to preview this draft";
      return;
    }
    const frame = $("#previewFrame");
    frame.dataset.device = $("#device").value;
    frame.dataset.flow = state.flow;
    $("#sow-menu").hidden = state.flow !== "menu";
    $("#previewFrame .sample-world").hidden = state.isMenuGuides;
    $("#sow-hud").hidden = state.isMenuGuides;
    const steps = Array.isArray(state.definition.steps) ? state.definition.steps : [];
    const candidates = [
      forceStep,
      state.machine && state.machine.state.id,
      state.selected,
      flowEntry(),
      state.definition.entry,
    ];
    const start = candidates.find(function (id) {
      return (
        id &&
        steps.some(function (step) {
          return step && step.id === id;
        })
      );
    });
    if (!start) {
      $("#previewStatus").textContent = "Preview waiting for a valid opening step.";
      return;
    }
    let requestedEntry = flowEntry() || state.definition.entry;
    if (
      !steps.some(function (step) {
        return step && step.id === requestedEntry;
      })
    )
      requestedEntry = start;
    state.playingStep = null;
    try {
      state.machine = window.SOWCampaign.create(state.definition, requestedEntry, state.roster);
    } catch (error) {
      $("#previewStatus").textContent = "Preview unavailable: " + error.message;
      return;
    }
    state.previewActionStep = null;
    state.previewActionRatio = null;
    state.facts = state.facts || {};
    const context = previewContext(start);
    const startTrigger = steps.find(function (item) {
      return item.id === start;
    })?.trigger;
    if (
      startTrigger &&
      startTrigger.type === "territory" &&
      startTrigger.scope === "total" &&
      startTrigger.comparison === "lte" &&
      Number(context.facts.tiles || 0) <= Number(startTrigger.value || 1)
    ) {
      context.facts.tiles = Number(startTrigger.value || 1) + 1;
      state.facts.tiles = context.facts.tiles;
    }
    state.machine.jump(start, context.facts, context.choices);
    if (!state.renderer)
      state.renderer = window.SOWCampaignView.mount($("#previewRoot"), {
        translate: function (key) {
          return translated(key, state.previewLanguage);
        },
        asset: asset,
        autoPlayCinematic: false,
        roster: function () {
          return state.roster;
        },
        onContinue: function () {
          const step = state.machine.view().step;
          state.machine.advance(null, step.id);
          paintPreview();
        },
        onChoice: function (choice) {
          const step = state.machine.view().step;
          state.machine.advance(choice, step.id);
          paintPreview();
        },
        onFocus: focusPreviewTarget,
        onDismiss: null,
      });
    paintPreview();
  }
  function syncMenuPreviewFacts() {
    const facts = state.facts;
    facts.menu_outcomes = facts.menu_outcomes || {};
    const available = Array.isArray(facts.affordable_unowned_leader_ids)
      ? facts.affordable_unowned_leader_ids.map(String)
      : [];
    const owned = Array.isArray(facts.owned_leader_ids) ? facts.owned_leader_ids.map(String) : [];
    const selectedOffer = String(facts.menu_guide_selected_leader || "");
    facts.commerce_offer_available = facts.commerce_available && available.length ? 1 : 0;
    facts.leader_purchase_modal_open = facts.purchase_modal_phase ? 1 : 0;
    facts.alternate_leader_available = (facts.available_leader_ids || []).some(function (id) {
      return String(id) !== String(facts.selected_leader || "");
    })
      ? 1
      : 0;
    facts.leader_purchase_confirmed =
      selectedOffer &&
      (String(facts.purchased_leader_id || "") === selectedOffer || owned.includes(selectedOffer))
        ? 1
        : 0;
    facts.menu_guide_selected_offer_equipped = Boolean(
      selectedOffer && String(facts.selected_leader || "") === selectedOffer && owned.includes(selectedOffer),
    );
    const storeNav = document.querySelector("#sow-menu [data-nav-screen='store']");
    if (storeNav) storeNav.disabled = !facts.commerce_available;
    const purchaseConfirm = document.querySelector("#sow-menu [data-command='confirm_purchase']");
    const purchaseClose = document.querySelector("#sow-menu [data-command='cancel_purchase']");
    document.querySelectorAll("#sow-menu [data-nav-screen]").forEach(function (button) {
      button.disabled =
        Boolean(facts.purchase_modal_phase) ||
        (button.dataset.navScreen === "store" && !facts.commerce_available);
    });
    if (purchaseConfirm) purchaseConfirm.hidden = facts.purchase_modal_phase === "success";
    if (purchaseClose)
      purchaseClose.textContent = facts.purchase_modal_phase === "success" ? "CONTINUE" : "CANCEL";
    document.querySelectorAll("#sow-menu [data-command='preview_leader']").forEach(function (button) {
      const leaderId = String(button.dataset.leaderId || "");
      button.disabled =
        !(facts.available_leader_ids || []).map(String).includes(leaderId) && !available.includes(leaderId);
      button.classList.toggle(
        "is-locked",
        !(facts.available_leader_ids || []).map(String).includes(leaderId),
      );
    });
    document.querySelectorAll("#sow-menu [data-command='unlock_leader']").forEach(function (button) {
      button.disabled = !available.includes(String(button.dataset.leaderId || ""));
    });
  }
  function incrementMenuOutcome(outcome) {
    const counts = state.facts.menu_outcomes || (state.facts.menu_outcomes = {});
    counts[outcome] = Number(counts[outcome] || 0) + 1;
  }
  function handleMenuPreviewClick(event) {
    if (!state.isMenuGuides) return false;
    const target = event.target.closest("button, [data-lobby-card]");
    if (!target || target.disabled) return true;
    const nav = target.closest("[data-nav-screen]");
    if (nav) {
      const nextScreen = nav.dataset.navScreen === "battle" ? "home" : nav.dataset.navScreen;
      setPreviewMenuScreen(nextScreen);
      if (nextScreen === "profile") {
        state.facts.profile_ready = true;
        state.facts.own_profile_ready = true;
        incrementMenuOutcome("profile_opened");
      } else if (nextScreen === "store") {
        state.facts.store_ready = true;
        incrementMenuOutcome("store_ready");
      }
      syncMenuPreviewFacts();
      paintPreview();
      return true;
    }
    const command = target.dataset.command;
    if (command === "open_campaign") {
      setPreviewMenuScreen("campaign");
      paintPreview();
      return true;
    }
    if (target.matches("[data-preview-menu-back]")) {
      setPreviewMenuScreen("home");
      paintPreview();
      return true;
    }
    if (command === "preview_leader") {
      const leaderId = String(target.dataset.leaderId || "");
      state.facts.preview_leader_id = leaderId;
      if (
        (state.facts.available_leader_ids || []).map(String).includes(leaderId) &&
        leaderId !== state.facts.selected_leader
      )
        incrementMenuOutcome("leader_preview_selected");
      if (
        Array.isArray(state.facts.affordable_unowned_leader_ids) &&
        state.facts.affordable_unowned_leader_ids.map(String).includes(leaderId)
      ) {
        state.facts.menu_guide_selected_leader = leaderId;
        incrementMenuOutcome("leader_offer_selected");
      }
      syncMenuPreviewFacts();
      paintPreview();
      return true;
    }
    if (command === "confirm_leader") {
      let selected = String(state.facts.preview_leader_id || "");
      const currentStep = state.machine && state.machine.view().step;
      const trigger = (currentStep && currentStep.trigger) || {};
      if (trigger.require_purchased_leader) selected = String(state.facts.menu_guide_selected_leader || "");
      if (
        selected &&
        (state.facts.available_leader_ids || []).map(String).includes(selected) &&
        (!trigger.require_purchased_leader || selected === state.facts.menu_guide_selected_leader)
      ) {
        state.facts.selected_leader = selected;
        incrementMenuOutcome("leader_selected");
        setPreviewMenuScreen("home");
        syncMenuPreviewFacts();
        paintPreview();
      }
      return true;
    }
    if (command === "unlock_leader") {
      const offerId = String(target.dataset.leaderId || "");
      if (!(state.facts.affordable_unowned_leader_ids || []).map(String).includes(offerId)) return true;
      state.facts.purchase_prompt_leader_id = offerId;
      state.facts.purchase_modal_phase = "confirm";
      incrementMenuOutcome("leader_purchase_prompt_opened");
      setPreviewMenuScreen("purchase");
      paintPreview();
      return true;
    }
    if (command === "confirm_purchase") {
      const purchaseId = String(state.facts.purchase_prompt_leader_id || "");
      if (!purchaseId || !(state.facts.affordable_unowned_leader_ids || []).map(String).includes(purchaseId))
        return true;
      state.facts.purchased_leader_id = purchaseId;
      state.facts.available_leader_ids = Array.from(
        new Set((state.facts.available_leader_ids || []).map(String).concat(purchaseId)),
      );
      state.facts.owned_leader_ids = Array.from(
        new Set((state.facts.owned_leader_ids || []).map(String).concat(purchaseId)),
      );
      state.facts.affordable_unowned_leader_ids = (state.facts.affordable_unowned_leader_ids || []).filter(
        function (id) {
          return String(id) !== purchaseId;
        },
      );
      incrementMenuOutcome("leader_purchased");
      state.facts.purchase_modal_phase = "success";
      setPreviewMenuScreen("purchase");
      syncMenuPreviewFacts();
      paintPreview();
      return true;
    }
    if (command === "cancel_purchase") {
      if (state.facts.purchase_modal_phase === "success") incrementMenuOutcome("leader_purchase_closed");
      else if (state.facts.purchase_modal_phase === "confirm" && state.facts.purchase_prompt_leader_id)
        incrementMenuOutcome("leader_purchase_cancelled");
      state.facts.purchase_modal_phase = null;
      state.facts.purchase_prompt_leader_id = null;
      setPreviewMenuScreen("heroes");
      syncMenuPreviewFacts();
      paintPreview();
      return true;
    }
    if (command === "profile_tab" && target.dataset.profileTab === "history") {
      state.facts.profile_tab = "history";
      state.facts.history_ready = true;
      state.facts.history_match_ids = [state.facts.current_match_id];
      incrementMenuOutcome("history_current_match_ready");
      paintPreview();
      return true;
    }
    if (command === "quick_match") {
      incrementMenuOutcome("quick_match_accepted");
      setPreviewMenuScreen("matchmaking");
      paintPreview();
      return true;
    }
    if (command === "open_campaign" || command === "start_campaign_episode") return true;
    return true;
  }
  function renderMenuGuideFactControls(model, host) {
    const trigger = model.step.trigger;
    const outcome = menuGuideTriggerOutcomes(trigger)[0];
    const label = outcome ? outcome.replace(/_/g, " ") : model.step.type;
    host.appendChild(
      el("small", {}, "Preview only · the guide waits for the simulated action: " + label + "."),
    );
    if (model.step.id === "welcome_reopen_heroes") {
      const noOffer = el("button", { type: "button" }, "Simulate no affordable gem offer");
      noOffer.addEventListener("click", function () {
        state.facts.affordable_unowned_leader_ids = [];
        syncMenuPreviewFacts();
        paintPreview();
      });
      host.appendChild(noOffer);
    }
    if (model.step.id === "welcome_purchase_confirm") {
      host.appendChild(
        el("small", {}, "A failed purchase keeps this step pending. Use Cancel to continue without buying."),
      );
      const failed = el("button", { type: "button" }, "Simulate failed purchase · no progress");
      failed.addEventListener("click", function () {
        $("#previewStatus").textContent = "Simulated purchase failure · the guide remains on this step.";
      });
      host.appendChild(failed);
    }
    if (model.step.id === "first_history") {
      const noCommerce = el("button", { type: "button" }, "Simulate a platform without the store");
      noCommerce.addEventListener("click", function () {
        state.facts.commerce_available = false;
        syncMenuPreviewFacts();
        paintPreview();
      });
      host.appendChild(noCommerce);
    }
    if (model.step.id === "welcome_name") {
      const rejectedName = el("button", { type: "button" }, "Simulate rejected name · no progress");
      rejectedName.addEventListener("click", function () {
        $("#previewStatus").textContent = "Simulated name rejection · the guide remains on this step.";
      });
      host.appendChild(rejectedName);
    }
  }
  function renderFactControls(model) {
    const host = $("#factControls");
    host.replaceChildren();
    if (state.isMenuGuides) {
      $("#simulateBtn").hidden = true;
      $("#tickBtn").hidden = true;
      renderMenuGuideFactControls(model, host);
      return;
    }
    if (model.waiting) {
      host.appendChild(
        el("small", {}, "Scene appears in " + (model.wait_remaining_ms / 1000).toFixed(1) + " seconds."),
      );
      return;
    }
    const trigger = model.step.trigger;
    const timedRoute = (model.step.routes || []).some(function (route) {
      return route.when && route.when.fact === "elapsed_seconds";
    });
    const canTick = Boolean((trigger && trigger.type === "elapsed") || timedRoute);
    const simulateButton = $("#simulateBtn"),
      tickButton = $("#tickBtn");
    simulateButton.hidden = !trigger;
    simulateButton.disabled = !trigger;
    tickButton.hidden = !canTick;
    tickButton.disabled = !canTick;
    if (!trigger)
      host.appendChild(
        el(
          "small",
          {},
          timedRoute
            ? "Advance game time to test this route."
            : "This beat waits for dialogue, a decision or an incoming story event.",
        ),
      );
    else {
      const progress = model.progress;
      let description = trigger.type;
      if (trigger.type === "contact" && trigger.targets)
        description += " · contact " + (trigger.value || 1) + " of " + trigger.targets.length + " factions";
      if (trigger.type === "fleet")
        description +=
          " · " + (trigger.unit || "any ship") + (trigger.target ? " to " + factionName(trigger.target) : "");
      if (trigger.type === "resource_transfer")
        description +=
          (trigger.recipient ? " · to " + factionName(trigger.recipient) : " · any recipient") +
          (trigger.resources ? " · " + trigger.resources.join(" + ") : " · any resources");
      host.appendChild(
        el("small", {}, "Waiting for " + description + " · " + progress.current + " / " + progress.target),
      );
      const territoryMaximum =
        trigger.type === "territory" && trigger.scope === "total" && trigger.comparison === "lte";
      if (
        !territoryMaximum &&
        !["troops", "elapsed", "contact", "eliminated", "fleet", "resource_transfer", "alliance"].includes(
          trigger.type,
        )
      ) {
        const button = el("button", { type: "button" }, "+1 " + trigger.type);
        button.addEventListener("click", function () {
          simulateObjective(1);
        });
        host.appendChild(button);
      }
      if (territoryMaximum)
        host.appendChild(
          inputField(
            "Current territories",
            Math.max(0, Number(state.facts.tiles || 0)),
            function (value) {
              state.facts.tiles = Math.max(0, Math.floor(Number(value) || 0));
              paintPreview();
            },
            { type: "number", min: 0, step: 1 },
          ),
        );
      if (trigger.type === "contact")
        (trigger.targets || [trigger.target]).filter(Boolean).forEach(function (factionId) {
          const contact = el("button", { type: "button" }, "Contact · " + factionName(factionId));
          contact.disabled = (state.facts.contact_faction_ids || []).includes(factionId);
          contact.addEventListener("click", function () {
            simulateObjective(1, { factionId: factionId });
          });
          host.appendChild(contact);
        });
      if (trigger.type === "eliminated") {
        const eliminatedTarget = trigger.target || "player";
        const eliminate = el(
          "button",
          { type: "button" },
          "Simulate elimination · " +
            (eliminatedTarget === "player" ? "Player" : factionName(eliminatedTarget)),
        );
        eliminate.disabled = (state.facts.eliminated_faction_ids || []).includes(eliminatedTarget);
        eliminate.addEventListener("click", function () {
          simulateObjective(1);
        });
        host.appendChild(eliminate);
      }
      if (trigger.type === "alliance" || trigger.type === "alliance_request")
        factionOptions().forEach(function (option) {
          const alliance = el("button", { type: "button" }, "Alliance · " + option.label);
          alliance.disabled = (state.facts.alliance_faction_ids || []).includes(option.value);
          alliance.addEventListener("click", function () {
            simulateObjective(1, { factionId: option.value });
          });
          host.appendChild(alliance);
        });
      if (trigger.type === "fleet")
        ["TransportShip", "TradeShip", "Warship"].forEach(function (unit) {
          if (trigger.target && unit === "TransportShip") {
            [
              trigger.target,
              factionOptions().find(function (option) {
                return option.value !== trigger.target;
              })?.value,
            ]
              .filter(Boolean)
              .forEach(function (target) {
                const ship = el(
                  "button",
                  { type: "button" },
                  "Simulate · " + unit + " to " + factionName(target),
                );
                ship.addEventListener("click", function () {
                  simulateObjective(1, { unit: unit, target: target });
                });
                host.appendChild(ship);
              });
          } else {
            const ship = el("button", { type: "button" }, "Simulate · " + unit);
            ship.addEventListener("click", function () {
              simulateObjective(1, { unit: unit });
            });
            host.appendChild(ship);
          }
        });
      if (trigger.type === "resource_transfer") {
        const recipient = trigger.recipient || (state.roster.factions[0] && state.roster.factions[0].id);
        const wrongRecipient = state.roster.factions.find(function (faction) {
          return faction.id !== recipient;
        });
        if (wrongRecipient) {
          const wrongTarget = el(
            "button",
            { type: "button" },
            "Simulate wrong recipient · " + wrongRecipient.name,
          );
          wrongTarget.addEventListener("click", function () {
            simulateObjective(1, { recipient: wrongRecipient.id, resources: ["gold", "troops"] });
          });
          host.appendChild(wrongTarget);
        }
        [["gold"], ["troops"], ["gold", "troops"]].forEach(function (resources) {
          const transfer = el(
            "button",
            { type: "button" },
            "Simulate " + resources.join(" + ") + " to " + factionName(recipient),
          );
          transfer.addEventListener("click", function () {
            simulateObjective(1, { recipient: recipient, resources: resources });
          });
          host.appendChild(transfer);
        });
      }
    }
    (state.definition.reactions || []).forEach(function (reaction) {
      const type = reaction.when.type;
      const isContact = type === "contact" || type === "first_contact";
      let target =
        reaction.when.target ||
        (Array.isArray(reaction.when.targets)
          ? reaction.when.targets.find(function (id) {
              return !(state.facts.contact_faction_ids || []).includes(id);
            })
          : null);
      if (type === "contact" && reaction.when.relation)
        target = (state.roster.factions || []).find(function (faction) {
          return (
            faction.relation === reaction.when.relation &&
            !(state.facts.contact_faction_ids || []).includes(faction.id)
          );
        })?.id;
      const buttonLabel =
        type === "pre_defeat"
          ? "Simulate final capture before defeat"
          : isContact
            ? "Simulate first contact"
            : "Simulate first support";
      const button = el("button", { type: "button" }, buttonLabel + " · " + factionName(target));
      button.disabled = isContact
        ? !target || (state.facts.contact_faction_ids || []).includes(target)
        : type === "pre_defeat"
          ? (state.facts.pre_defeat_faction_ids || []).includes(reaction.when.target)
          : Boolean((state.facts.support_deliveries_by_faction_id || {})[reaction.when.target]);
      button.addEventListener("click", function () {
        if (reaction.during && state.machine) state.machine.jump(reaction.during, state.facts, state.ui);
        if (isContact)
          state.facts.contact_faction_ids = Array.from(
            new Set((state.facts.contact_faction_ids || []).concat(target)),
          );
        else if (type === "pre_defeat")
          state.facts.pre_defeat_faction_ids = Array.from(
            new Set((state.facts.pre_defeat_faction_ids || []).concat(reaction.when.target)),
          );
        else {
          state.facts.support_deliveries_by_faction_id = state.facts.support_deliveries_by_faction_id || {};
          state.facts.support_deliveries_by_faction_id[reaction.when.target] = {
            deliveries: 1,
            gold: 100,
            troops: 100,
            first_tick: ++state.facts.elapsed_ticks,
          };
          state.facts.ally_support_deliveries = Number(state.facts.ally_support_deliveries || 0) + 1;
        }
        paintPreview();
      });
      host.appendChild(button);
    });
  }
  function previewBuildingAction(target) {
    const command = target.closest("[data-command]");
    const action = target.closest("[data-map-action]");
    const type = command && command.dataset.command;
    const direct = action && /^build_(city|factory|port|tower|farm)$/.exec(action.dataset.mapAction);
    if (
      ![
        "select_building",
        "nudge_building_placement",
        "cancel_building_mode",
        "confirm_building_placement",
      ].includes(type) &&
      !direct
    )
      return false;
    const root = $("#sow-hud");
    let placement = $("#sow-hud-placement");
    if (type === "nudge_building_placement") {
      if (!placement) return true;
      const offset = { up: [0, -8], right: [8, 0], down: [0, 8], left: [-8, 0] }[command.dataset.direction];
      const rect = placement.getBoundingClientRect(),
        bounds = root.getBoundingClientRect();
      if (
        offset &&
        rect.left + offset[0] >= bounds.left &&
        rect.right + offset[0] <= bounds.right &&
        rect.top + offset[1] >= bounds.top &&
        rect.bottom + offset[1] <= bounds.bottom
      ) {
        placement.style.left = rect.left - bounds.left + 76 + offset[0] + "px";
        placement.style.top = rect.top - bounds.top + 76 + offset[1] + "px";
      }
      return true;
    }
    const kind = direct
      ? direct[1][0].toUpperCase() + direct[1].slice(1)
      : type === "select_building"
        ? command.dataset.kind
        : placement && placement.dataset.kind;
    if (placement) placement.remove();
    if (type === "select_building") {
      placement = el("div", {
        id: "sow-hud-placement",
        class: "sample-placement",
        role: "group",
        "aria-label": "Place " + kind,
        dataset: { kind: kind },
      });
      placement.appendChild(el("span", { class: "sample-placement-building", "aria-label": kind }, "▥"));
      ["up", "right", "down", "left"].forEach(function (direction, index) {
        placement.appendChild(
          el(
            "button",
            {
              type: "button",
              "aria-label": "Move " + direction,
              dataset: { command: "nudge_building_placement", direction: direction },
            },
            ["↑", "→", "↓", "←"][index],
          ),
        );
      });
      placement.appendChild(
        el(
          "button",
          { type: "button", "aria-label": "Cancel", dataset: { command: "cancel_building_mode" } },
          "×",
        ),
      );
      placement.appendChild(
        el(
          "button",
          { type: "button", "aria-label": "Build", dataset: { command: "confirm_building_placement" } },
          "✓",
        ),
      );
      root.appendChild(placement);
    } else if (state.machine) {
      const trigger = state.machine.view().step.trigger;
      if (trigger && trigger.type === "ui" && trigger.action === type) simulateObjective(1);
      else if (
        (direct || type === "confirm_building_placement") &&
        kind &&
        trigger &&
        [kind.toLowerCase(), "building"].includes(trigger.type)
      )
        simulateObjective(1);
    }
    if (direct) {
      root.dataset.previewMapMenu = "";
      root.querySelector(".sample-map-popup").hidden = true;
    }
    paintPreview();
    return true;
  }

  function simulateObjective(amount, sample) {
    if (!state.machine) return;
    const step = state.machine.view().step,
      trigger = step.trigger;
    if (!trigger) return;
    sample = sample || {};
    if (amount == null) {
      const progress = state.machine.view().progress;
      amount = Math.max(1, progress.target - progress.current);
    }
    amount = Number(amount || 1);
    if (trigger.type === "contact") {
      const contacts = state.facts.contact_faction_ids || [];
      const candidates = Array.isArray(trigger.targets) ? trigger.targets : [trigger.target];
      const contacted =
        sample.factionId ||
        candidates.find(function (factionId) {
          return factionId && !contacts.includes(factionId);
        }) ||
        candidates[0];
      if (contacted) state.facts.contact_faction_ids = Array.from(new Set(contacts.concat(contacted)));
    } else if (trigger.type === "defeated") {
      const targets = Array.isArray(trigger.targets) ? trigger.targets : [trigger.target];
      const defeated = state.facts.defeated_faction_ids || [];
      state.facts.defeated_faction_ids = Array.from(
        new Set(
          defeated.concat(
            targets
              .filter(function (factionId) {
                return factionId && !defeated.includes(factionId);
              })
              .slice(0, amount),
          ),
        ),
      );
    } else if (trigger.type === "eliminated") {
      const eliminatedTarget = trigger.target || "player";
      state.facts.eliminated_faction_ids = Array.from(
        new Set((state.facts.eliminated_faction_ids || []).concat(eliminatedTarget)),
      );
    } else if (trigger.type === "ui")
      state.ui[trigger.action] = Number(state.ui[trigger.action] || 0) + amount;
    else if (trigger.type === "alliance_request") {
      state.facts.alliance_request_faction_ids = Array.from(
        new Set((state.facts.alliance_request_faction_ids || []).concat(sample.factionId || trigger.target)),
      );
    } else if (trigger.type === "alliance") {
      state.facts.alliance_faction_ids = Array.from(
        new Set(
          (state.facts.alliance_faction_ids || []).concat(
            sample.factionId || trigger.target || "simulated_ally",
          ),
        ),
      );
      state.facts.alliances_formed = state.facts.alliance_faction_ids.length;
    } else if (trigger.type === "fleet") {
      const unit = sample.unit || trigger.unit || "TransportShip";
      state.facts.fleets_by_type[unit] = Number(state.facts.fleets_by_type[unit] || 0) + amount;
      const target = sample.target || trigger.target;
      if (unit === "TransportShip" && target)
        state.facts.transport_fleets_by_faction_id[target] =
          Number(state.facts.transport_fleets_by_faction_id[target] || 0) + amount;
      state.facts.fleets += amount;
    } else if (trigger.type === "resource_transfer") {
      const receiver = sample.recipient || trigger.recipient || "simulated_ally",
        received = sample.resources || trigger.resources || ["gold", "troops"];
      const sent =
        state.facts.resource_transfers_by_recipient_faction_id[receiver] ||
        (state.facts.resource_transfers_by_recipient_faction_id[receiver] = {
          total: 0,
          gold: 0,
          troops: 0,
          gold_troops: 0,
        });
      sent.total += amount;
      if (received.includes("gold")) sent.gold += amount;
      if (received.includes("troops")) sent.troops += amount;
      if (received.includes("gold") && received.includes("troops")) sent.gold_troops += amount;
      state.facts.resource_transfers += amount;
    } else if (trigger.type === "camera_target") {
      state.facts.camera_target_distance = Number(trigger.distance || 12);
      state.facts.camera_target = 1;
    } else if (trigger.type === "zoom_out_complete") {
      state.facts.camera_zoom = window.SOWCampaign.zoomOutTarget(state.facts, trigger.value);
      state.facts.zoom_out_events = Number(state.facts.zoom_out_events || 0) + 1;
    } else if (trigger.type === "zoom_in_complete") {
      state.facts.camera_zoom = Number(state.facts.camera_zoom_target || 0.75);
      state.facts.zoom_in_complete = 1;
      state.facts.zoom_in_events = Number(state.facts.zoom_in_events || 0) + 1;
      if (trigger.target) state.facts.camera_target_distance = Number(trigger.distance || 12);
    } else if (trigger.type === "structure_level") {
      const kind = String(trigger.kind || "City").toLowerCase();
      state.facts.structure_levels[kind] = Math.max(
        Number(state.facts.structure_levels[kind] || 0),
        Number(trigger.value || 1),
      );
    } else {
      const metric = window.SOWCampaign.METRICS[trigger.type];
      if (trigger.type === "attack" && trigger.target) {
        state.facts.attacks_by_faction_id = state.facts.attacks_by_faction_id || {};
        state.facts.attacks_by_faction_id[trigger.target] =
          Number(state.facts.attacks_by_faction_id[trigger.target] || 0) + amount;
      } else state.facts[metric] = Number(state.facts[metric] || 0) + amount;
      if (trigger.type === "territory") state.facts.tiles = Number(state.facts.tiles || 0) + amount;
    }
    paintPreview();
  }
  function freshFacts() {
    if (state.isMenuGuides)
      return {
        screen: "home",
        profile_tab: "overview",
        profile_ready: false,
        history_ready: false,
        own_profile_ready: false,
        history_match_ids: [],
        current_match_id: "studio-match",
        commerce_available: true,
        affordable_unowned_leader_ids: ["gem_offer"],
        owned_leader_ids: ["owned"],
        available_leader_ids: ["owned", "free"],
        selected_leader: "owned",
        preview_leader_id: "owned",
        purchased_leader_id: null,
        purchase_prompt_leader_id: null,
        purchase_modal_phase: null,
        store_ready: false,
        menu_guide_selected_leader: null,
        commerce_offer_available: 1,
        leader_purchase_modal_open: 0,
        leader_purchase_confirmed: 0,
        menu_guide_selected_offer_equipped: false,
        menu_outcomes: {
          name_saved: 0,
          leader_selected: 0,
          leader_offer_selected: 0,
          leader_purchase_prompt_opened: 0,
          leader_purchased: 0,
          leader_purchase_cancelled: 0,
          leader_purchase_closed: 0,
          profile_opened: 0,
          history_current_match_ready: 0,
          store_ready: 0,
          quick_match_accepted: 0,
        },
      };
    return {
      tiles: 0,
      tiles_gained: 0,
      kills: 0,
      troops: Number(state.definition.settings && state.definition.settings.starting_troops) || 0,
      buildings: 0,
      cities: 0,
      ally_support_deliveries: 0,
      support_deliveries_by_faction_id: {},
      structure_levels: {},
      fleets: 0,
      fleets_by_type: {},
      transport_fleets_by_faction_id: {},
      nukes: 0,
      attacks: 0,
      attacks_by_faction_id: {},
      camera_drag_events: 0,
      camera_key_pan_events: 0,
      hover_events: 0,
      zoom_out_events: 0,
      zoom_in_events: 0,
      zoom_out_complete: 0,
      zoom_in_complete: 0,
      camera_target: 0,
      camera_target_distance: 30,
      camera_zoom: 1,
      camera_zoom_start: 1,
      camera_zoom_floor: 0.25,
      camera_zoom_target: 0.8,
      touch_controls: 0,
      contact_faction_ids: [],
      defeated_faction_ids: [],
      eliminated_faction_ids: [],
      resource_transfers: 0,
      resource_transfers_by_recipient_faction_id: {},
      alliance_faction_ids: [],
      alliance_request_faction_ids: [],
      hostile_faction_ids: [],
      resources_available: 1,
      elapsed_ticks: 0,
      elapsed_seconds: 0,
    };
  }
  function setPreviewMenuScreen(screen) {
    const menu = $("#sow-menu");
    menu.dataset.previewScreen = screen;
    menu.querySelectorAll("[data-preview-menu-screen]").forEach(function (panel) {
      panel.hidden = panel.dataset.previewMenuScreen !== screen;
    });
    if (state.isMenuGuides && state.facts) state.facts.screen = screen;
  }
  function resetPreview() {
    clearPreviewRevealTimer();
    $("#sow-hud-placement")?.remove();
    state.facts = freshFacts();
    state.ui = {};
    state.previewFocusActive = false;
    const mapLayer = $("#campaignPreviewLayer");
    ["--map-focus-x", "--map-focus-y", "--map-focus-scale"].forEach(function (property) {
      mapLayer.style.removeProperty(property);
    });
    mapLayer.classList.remove("has-map-guide");
    $("#campaignPreviewMarkers .is-camera-focus")?.classList.remove("is-camera-focus");
    setPreviewMenuScreen("home");
    renderPreview(flowEntry() || state.definition.entry);
  }
  function refresh(skipGraph) {
    if (!state.definition) return;
    normalizeStrings(state.definition);
    ensureLayout();
    const report = valid();
    state.validation = report;
    const blocked = report.errors.length > 0;
    $("#previewRoot").inert = blocked;
    ["playSelected", "restartBtn", "simulateBtn", "tickBtn", "previewBranchSelect"].forEach(function (id) {
      $("#" + id).disabled = blocked;
    });
    updateStepValidation();
    $("#validationStatus").textContent = report.errors.length
      ? "Needs fixes · " + report.errors.length
      : report.warnings.length
        ? "Ready · " + report.warnings.length + " notes"
        : "Ready to play";
    const errors = $("#errors");
    errors.replaceChildren();
    report.errors.forEach(function (issue) {
      const button = el(
        "button",
        { type: "button" },
        [issue.step, issue.field, issue.message].filter(Boolean).join(" · "),
      );
      button.addEventListener("click", function () {
        if (issue.step) selectStep(issue.step);
      });
      errors.appendChild(button);
    });
    report.warnings.forEach(function (issue) {
      const button = el(
        "button",
        { type: "button", class: "warning" },
        [issue.step, issue.message].filter(Boolean).join(" · "),
      );
      if (issue.step)
        button.addEventListener("click", function () {
          selectStep(issue.step);
        });
      errors.appendChild(button);
    });
    $("#exportBtn").disabled = state.saving || report.errors.length > 0 || Boolean(state.demoBackup);
    if (!skipGraph) renderGraph();
  }
  function updateStepValidation() {
    const box = $("#inspector .step-validation");
    if (!box) return;
    const errors = (state.validation.errors || []).filter(function (issue) {
      return issue.step === state.selected;
    });
    const warnings = (state.validation.warnings || []).filter(function (issue) {
      return issue.step === state.selected;
    });
    const issues = errors.concat(warnings);
    box.replaceChildren();
    box.hidden = !issues.length;
    box.classList.toggle("is-warning", !errors.length && warnings.length > 0);
    if (issues.length) {
      box.appendChild(el("strong", {}, errors.length ? "Fix this step before saving" : "Review this step"));
      issues.forEach(function (issue) {
        box.appendChild(
          el(
            "p",
            {},
            [warnings.includes(issue) ? "Note" : "", issue.field, issue.message].filter(Boolean).join(" · "),
          ),
        );
      });
    }
  }

  function syncStudioMode() {
    $("#flowControl").hidden = state.isMenuGuides;
    $("#menuGuideControl").hidden = !state.isMenuGuides;
    $("#menuGuideBtn").hidden = state.isMenuGuides;
    $("#mapLink").hidden = state.isMenuGuides;
    $(".episode-settings").hidden = state.isMenuGuides;
    $("#flowEntryBtn").textContent = state.isMenuGuides
      ? "Use selected as journey opening"
      : "Use selected as opening";
    $("#menuGuideSelect").value = state.menuGuideId;
    $("#episodeLabel").textContent = state.isMenuGuides
      ? "Menu guides"
      : episodeId.replace(/_/g, " ").toUpperCase();
  }

  async function load() {
    status("Loading…");
    notice("");
    if (episodeId === "menu") {
      const guideResponse = await fetch("/assets/guides/menu.triggers.json", { cache: "no-store" });
      if (!guideResponse.ok) throw new Error("Could not load the published menu guide definition.");
      const guideDefinition = await guideResponse.json();
      const guideReport = window.SOWCampaign.validateMenuGuides(guideDefinition);
      if (guideReport.errors.length)
        throw new Error("Menu guide definition is invalid: " + guideReport.errors[0].message);
      state.roster = null;
      state.definition = guideDefinition;
      state.rosterEtag = null;
      state.etag = guideResponse.headers.get("ETag");
      state.externalChangeTag = null;
      state.dirty = false;
      state.isMenuGuides = true;
      state.flow = "menu";
      state.menuGuideId = state.definition.menu_guides[state.menuGuideId] ? state.menuGuideId : "welcome";
      state.selected = flowEntry();
      normalizeStrings(state.definition);
      ensureLayout();
      syncMenuGuideSteps();
      $("#episodeSelect").value = "menu";
      $("#flowSelect").value = "menu";
      $("#previewFrame").dataset.flow = "menu";
      $("#logicLink").href = "logic.html?episode=menu";
      syncStudioMode();
      renderPalette();
      renderSettings();
      renderInspector();
      refresh();
      resetPreview();
      status("Saved");
      return;
    }
    const rosterResponse = await fetch("/assets/campaign/" + episodeId + ".json", { cache: "no-store" });
    const logicResponse = await fetch("/assets/campaign/" + episodeId + ".triggers.json", {
      cache: "no-store",
    });
    if (!rosterResponse.ok || !logicResponse.ok)
      throw new Error("Could not load both map and story files for " + episodeId + ".");
    const files = await Promise.all([rosterResponse.json(), logicResponse.json()]);
    state.roster = files[0];
    state.definition = files[1];
    state.rosterEtag = rosterResponse.headers.get("ETag");
    state.etag = logicResponse.headers.get("ETag");
    state.externalChangeTag = null;
    state.dirty = false;
    state.flow = "episode";
    state.isMenuGuides = false;
    state.selected = state.definition.entry;
    renderRosterMapPreview();
    normalizeStrings(state.definition);
    ensureLayout();
    const title = $("#episodeSelect");
    title.value = episodeId;
    let previewEpisode = (title.selectedOptions[0] && title.selectedOptions[0].textContent) || episodeId;
    previewEpisode = previewEpisode.split("·")[0].trim();
    $("[data-preview-episode-title]").textContent = previewEpisode;
    const replayButton = $("#sow-menu [data-command='start_campaign_episode']");
    replayButton.dataset.episodeId = episodeId;
    replayButton.textContent = episodeId === "boudica" ? "REPLAY TUTORIAL" : "REPLAY EPISODE";
    $("#flowSelect").value = state.flow;
    $("#mapLink").href = "index.html?episode=" + encodeURIComponent(episodeId);
    $("#logicLink").href = "logic.html?episode=" + encodeURIComponent(episodeId);
    syncStudioMode();
    renderPalette();
    renderSettings();
    renderInspector();
    refresh();
    resetPreview();
    status("Saved");
  }
  async function save() {
    if (state.demoBackup) return false;
    const report = valid();
    if (report.errors.length) {
      status("Fix story errors before saving", "error");
      notice(report.errors[0].message);
      return false;
    }
    if (state.saving) return false;
    const savedDraft = JSON.stringify(state.definition, null, 2) + "\n";
    state.saving = true;
    $("#exportBtn").disabled = true;
    status("Saving…");
    try {
      const targetFile = state.isMenuGuides ? "menu.triggers.json" : episodeId + ".triggers.json";
      const response = await fetch("/__save?file=" + encodeURIComponent(targetFile), {
        method: "POST",
        headers: { "Content-Type": "application/json", "If-Match": state.etag || "" },
        body: savedDraft,
      });
      if (response.status === 409) {
        status("Reload needed", "error");
        notice(await response.text());
        return false;
      }
      if (!response.ok) throw new Error(await response.text());
      state.etag = response.headers.get("ETag");
      state.dirty = JSON.stringify(state.definition, null, 2) + "\n" !== savedDraft;
      status(state.dirty ? "Unsaved changes" : "Saved", state.dirty ? "dirty" : "");
      notice("");
      return !state.dirty;
    } catch (error) {
      status("Save failed", "error");
      notice(error.message);
      return false;
    } finally {
      state.saving = false;
      refresh();
    }
  }
  async function watchExternalFiles() {
    if (state.demoBackup || document.visibilityState !== "visible" || !state.definition) return;
    try {
      const responses = state.isMenuGuides
        ? [await fetch("/assets/guides/menu.triggers.json", { method: "HEAD", cache: "no-store" })]
        : await Promise.all([
            fetch("/assets/campaign/" + episodeId + ".json", { method: "HEAD", cache: "no-store" }),
            fetch("/assets/campaign/" + episodeId + ".triggers.json", { method: "HEAD", cache: "no-store" }),
          ]);
      if (
        responses.some(function (response) {
          return !response.ok;
        })
      )
        return;
      const externalTag = responses
        .map(function (response) {
          return response.headers.get("ETag");
        })
        .join("\n");
      const unchanged = state.isMenuGuides
        ? responses[0].headers.get("ETag") === state.etag
        : responses[0].headers.get("ETag") === state.rosterEtag &&
          responses[1].headers.get("ETag") === state.etag;
      if (unchanged) {
        state.externalChangeTag = null;
        return;
      }
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
    } catch {
      /* A short editor-server restart must not discard the open draft. */
    }
  }
  function download() {
    if (state.demoBackup) return;
    const blob = new Blob([JSON.stringify(state.definition, null, 2) + "\n"], { type: "application/json" });
    const filename = state.isMenuGuides ? "menu.triggers.json" : episodeId + ".triggers.json";
    const anchor = el("a", { href: URL.createObjectURL(blob), download: filename });
    anchor.click();
    URL.revokeObjectURL(anchor.href);
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
      const previous = state.demoBackup;
      state.demoBackup = null;
      state.definition = previous.definition;
      state.selected = previous.selected;
      state.previewBranch = previous.previewBranch;
      state.flow = previous.flow;
      state.dirty = previous.dirty;
      $("#flowSelect").value = state.flow;
      renderSettings();
      renderInspector();
      refresh();
      resetPreview();
      status(state.dirty ? "Unsaved changes" : "Saved", state.dirty ? "dirty" : "");
      setDemoMode(false);
      return;
    }
    state.demoBackup = {
      definition: JSON.parse(JSON.stringify(state.definition)),
      selected: state.selected,
      previewBranch: state.previewBranch,
      flow: state.flow,
      dirty: state.dirty,
    };
    const id = episodeId + "_studio_demo";
    const oldSpeaker = Object.entries(state.definition.speakers || {})[0];
    const leaderId = (oldSpeaker && oldSpeaker[0]) || "commander";
    const speakers = {};
    speakers[leaderId] = oldSpeaker ? Object.assign({}, oldSpeaker[1]) : { name: "Commander" };
    let advisorId = "advisor";
    while (advisorId === leaderId) advisorId += "_2";
    speakers[advisorId] = { name: "Advisor" };
    const key = function (name) {
      return "tutorial.campaign_studio_demo_" + name;
    };
    state.definition = {
      version: 2,
      episode_id: episodeId,
      settings: state.definition.settings,
      entry: id + "_opening",
      speakers: speakers,
      layout: {},
      steps: [
        {
          id: id + "_opening",
          type: "scene",
          title_key: key("opening_title"),
          lines: [
            { speaker: leaderId, body_key: key("opening_body") },
            { speaker: advisorId, body_key: key("advisor_line") },
          ],
          presentation: "chapter",
          next: id + "_decision",
        },
        {
          id: id + "_decision",
          type: "choice",
          title_key: key("decision_title"),
          body_key: key("decision_body"),
          choices: [
            { id: "gather", label_key: key("gather"), next: id + "_grow" },
            { id: "strike", label_key: key("strike"), next: id + "_attack" },
          ],
        },
        {
          id: id + "_grow",
          type: "objective",
          title_key: key("grow_title"),
          hint_key: key("grow_hint"),
          trigger: { type: "territory", value: 300, scope: "step" },
          guide: { kind: "world", target: "expand", gesture: "tap" },
          next: id + "_recall",
        },
        {
          id: id + "_attack",
          type: "objective",
          title_key: key("attack_title"),
          hint_key: key("attack_hint"),
          trigger: { type: "kills", value: 1, scope: "total" },
          guide: { kind: "world", target: "assault", gesture: "tap" },
          next: id + "_recall",
        },
        {
          id: id + "_recall",
          type: "scene",
          title_key: key("recall_title"),
          body_key: key("recall_body"),
          next: id + "_turn",
          routes: [
            { when: { choice: id + "_decision", equals: "gather" }, next: id + "_gather_echo" },
            { when: { choice: id + "_decision", equals: "strike" }, next: id + "_strike_echo" },
          ],
        },
        {
          id: id + "_gather_echo",
          type: "scene",
          title_key: key("gather_echo_title"),
          body_key: key("gather_echo_body"),
          next: id + "_turn",
        },
        {
          id: id + "_strike_echo",
          type: "scene",
          title_key: key("strike_echo_title"),
          body_key: key("strike_echo_body"),
          next: id + "_turn",
        },
        {
          id: id + "_turn",
          type: "choice",
          title_key: key("turn_title"),
          body_key: key("turn_body"),
          choices: [
            { id: "press", label_key: key("press"), next: id + "_advance" },
            { id: "secure", label_key: key("secure"), next: id + "_hold" },
          ],
        },
        {
          id: id + "_advance",
          type: "objective",
          title_key: key("advance_title"),
          hint_key: key("advance_hint"),
          trigger: { type: "territory", value: 100, scope: "step" },
          guide: { kind: "world", target: "expand", gesture: "tap" },
          next: id + "_ending",
        },
        {
          id: id + "_hold",
          type: "objective",
          title_key: key("hold_title"),
          hint_key: key("hold_hint"),
          trigger: { type: "ui", action: "attack_ratio", scope: "step" },
          guide: { kind: "ui", target: "attack_ratio", gesture: "drag" },
          next: id + "_ending",
        },
        {
          id: id + "_ending",
          type: "end",
          title_key: key("ending_title"),
          body_key: key("ending_body"),
          presentation: "chapter",
        },
      ],
    };
    state.definition.layout = {};
    state.definition.steps.forEach(function (step, index) {
      state.definition.layout[step.id] = { x: 70 + (index % 2) * 320, y: 70 + Math.floor(index / 2) * 220 };
    });
    state.flow = "episode";
    $("#flowSelect").value = "episode";
    state.selected = state.definition.entry;
    renderSettings();
    renderInspector();
    refresh();
    resetPreview();
    status("Previewing sample · episode draft preserved", "preview");
    setDemoMode(true);
  }
  function createMenuGuide() {
    if (state.definition.menu_guide) {
      state.flow = "menu";
      $("#flowSelect").value = "menu";
      state.selected = state.definition.menu_guide.entry;
      renderSettings();
      renderInspector();
      resetPreview();
      return;
    }
    const opening = uniqueId(episodeId + "_menu_guide_opening", state.definition.steps);
    const campaign = uniqueId(episodeId + "_menu_guide_campaign", state.definition.steps);
    const replay = uniqueId(episodeId + "_menu_guide_replay", state.definition.steps);
    const ending = uniqueId(episodeId + "_menu_guide_end", state.definition.steps);
    const keys = {
      opening: "tutorial.campaign_menu_guide_opening",
      campaign: "tutorial.campaign_menu_guide_campaign",
      replay: "tutorial.campaign_menu_guide_replay",
      ending: "tutorial.campaign_menu_guide_ending",
    };
    state.definition.steps.push(
      {
        id: opening,
        type: "scene",
        title_key: keys.opening + "_title",
        body_key: keys.opening + "_body",
        presentation: "chapter",
        next: campaign,
      },
      {
        id: campaign,
        type: "guide",
        title_key: keys.campaign + "_title",
        hint_key: keys.campaign + "_hint",
        trigger: { type: "ui", action: "menu_campaign", scope: "step" },
        guide: { kind: "ui", target: "menu_campaign", gesture: "tap" },
        next: replay,
      },
      {
        id: replay,
        type: "guide",
        title_key: keys.replay + "_title",
        hint_key: keys.replay + "_hint",
        trigger: { type: "ui", action: "campaign_replay", scope: "step" },
        guide: { kind: "ui", target: "campaign_replay", gesture: "tap" },
        next: ending,
      },
      {
        id: ending,
        type: "end",
        title_key: keys.ending + "_title",
        body_key: keys.ending + "_body",
        presentation: "chapter",
      },
    );
    state.definition.menu_guide = { entry: opening, dismissible: true };
    state.definition.layout[opening] = { x: 70, y: 80 };
    state.definition.layout[campaign] = { x: 350, y: 80 };
    state.definition.layout[replay] = { x: 630, y: 80 };
    state.definition.layout[ending] = { x: 910, y: 80 };
    state.flow = "menu";
    $("#flowSelect").value = "menu";
    state.selected = opening;
    markDirty();
    renderSettings();
    renderInspector();
    refresh();
    resetPreview();
  }
  function connectControls() {
    $("#exportBtn").addEventListener("click", save);
    $("#mapLink").addEventListener("click", async function (event) {
      if (!state.dirty && !state.demoBackup) return;
      event.preventDefault();
      const href = event.currentTarget.href;
      if (state.demoBackup) demo();
      if (!state.dirty) {
        location.href = href;
        return;
      }
      if ((await save()) && !state.dirty) location.href = href;
    });
    $("#reloadBtn").addEventListener("click", function () {
      if (!state.dirty || confirm("Discard unsaved changes and reload this episode?"))
        load().catch(function (error) {
          notice(error.message);
          status("Load failed", "error");
        });
    });
    $("#downloadBtn").addEventListener("click", download);
    $("#demoBtn").addEventListener("click", demo);
    $("#menuGuideBtn").addEventListener("click", createMenuGuide);
    $("#flowSelect").addEventListener("change", function () {
      state.flow = this.value;
      $("#previewFrame").dataset.flow = state.flow;
      $("#sow-menu").hidden = state.flow !== "menu";
      renderSettings();
      renderInspector();
      resetPreview();
    });
    $("#menuGuideSelect").addEventListener("change", function () {
      if (!state.isMenuGuides || !state.definition.menu_guides[this.value]) return;
      state.menuGuideId = this.value;
      state.selected = flowEntry();
      state.previewBranch = null;
      renderInspector();
      renderGraph();
      resetPreview();
    });
    $("#flowEntryBtn").addEventListener("click", function () {
      if (!state.selected) return;
      if (state.isMenuGuides) state.definition.menu_guides[state.menuGuideId].entry = state.selected;
      else if (state.flow === "menu")
        state.definition.menu_guide = { entry: state.selected, dismissible: true };
      else state.definition.entry = state.selected;
      markDirty();
      renderSettings();
      renderInspector();
      renderGraph();
      resetPreview();
    });
    $("#playSelected").addEventListener("click", function () {
      state.facts = freshFacts();
      state.ui = {};
      renderPreview(state.selected);
    });
    $("#previewBranchSelect").addEventListener("change", function () {
      state.previewBranch = this.value ? { stepId: state.selected, routeIndex: Number(this.value) } : null;
      state.facts = freshFacts();
      state.ui = {};
      renderPreview(state.selected);
    });
    $("#restartBtn").addEventListener("click", resetPreview);
    $("#simulateBtn").addEventListener("click", function () {
      simulateObjective(null);
    });
    $("#tickBtn").addEventListener("click", function () {
      state.facts.elapsed_seconds = Number(state.facts.elapsed_seconds || 0) + 1;
      paintPreview();
    });
    $("#device").addEventListener("change", function () {
      renderRosterMapPreview();
      paintPreview();
    });
    $("#reducedMotion").addEventListener("change", paintPreview);
    $("#factionLabels").addEventListener("change", renderRosterMapPreview);
    $("#pickFactionTarget").addEventListener("click", function () {
      setFactionPicker(!state.pickingFaction);
    });
    $("#campaignPreviewMarkers").addEventListener("click", function (event) {
      const marker = state.pickingFaction && event.target.closest(".sample-faction.is-faction-pickable");
      if (!marker) return;
      event.preventDefault();
      event.stopPropagation();
      chooseFactionTarget(marker.dataset.factionId);
      $("#pickFactionTarget").focus({ preventScroll: true });
    });
    $("#campaignPreviewMarkers").addEventListener("keydown", function (event) {
      if (event.key !== "Enter" && event.key !== " ") return;
      const marker = state.pickingFaction && event.target.closest(".sample-faction.is-faction-pickable");
      if (!marker) return;
      event.preventDefault();
      event.stopPropagation();
      chooseFactionTarget(marker.dataset.factionId);
      $("#pickFactionTarget").focus({ preventScroll: true });
    });
    $("#previewLocale").addEventListener("change", function () {
      const locale = this.value;
      state.previewLanguage = locale;
      paintPreview();
      loadCatalog(locale).then(function () {
        if (state.previewLanguage === locale) paintPreview();
      });
    });
    $("#previewLocale").addEventListener("change", function () {
      const locale = this.value;
      state.previewLanguage = locale;
      loadCatalog(locale).then(function () {
        if (state.previewLanguage !== locale) return;
        renderSettings();
        renderInspector();
        renderGraph();
        paintPreview();
      });
    });
    document.addEventListener("visibilitychange", watchExternalFiles);
    window.setInterval(watchExternalFiles, 1500);
    function previewUiAction(event) {
      if (state.isMenuGuides) {
        if (
          event.type === "change" &&
          event.target.matches('[data-role="display-name"]') &&
          event.target.value.trim()
        ) {
          incrementMenuOutcome("name_saved");
          paintPreview();
        }
        return;
      }
      if (!state.machine) return;
      const trigger = state.machine.view().step.trigger;
      if (!trigger || trigger.type !== "ui") return;
      const control = window.SOWCampaignView.resolveUiTarget(trigger.action, document, episodeId);
      const expectedEvent = control && control.matches("input") ? "change" : "click";
      if (event.type !== expectedEvent) return;
      if (control && (control === event.target || control.contains(event.target))) simulateObjective(1);
    }
    $("#previewFrame").addEventListener("click", function (event) {
      const menu = $("#sow-menu");
      if (menu && menu.contains(event.target)) {
        if (state.isMenuGuides) {
          handleMenuPreviewClick(event);
          return;
        }
        if (event.target.closest('[data-command="open_campaign"]')) setPreviewMenuScreen("campaign");
        else if (event.target.closest("[data-preview-menu-back]")) setPreviewMenuScreen("home");
        else return;
        paintPreview();
        return;
      }
      const back = event.target.closest("[data-map-back]");
      const group = event.target.closest("[data-map-group]");
      const action = event.target.closest("[data-map-action]");
      const hudRoot = $("#sow-hud");
      if (!hudRoot || !hudRoot.contains(event.target)) return;
      if (previewBuildingAction(event.target)) return;
      if (back) hudRoot.dataset.previewMapMenu = "";
      else if (group) hudRoot.dataset.previewMapMenu = group.dataset.mapGroup;
      else if (action) hudRoot.dataset.previewMapMenu = "";
      else return;
      const transferPanel = $("#sow-hud-transfer");
      if (transferPanel && action && action.dataset.mapAction === "transfer") transferPanel.hidden = false;
      const popup = hudRoot.querySelector(".sample-map-popup");
      if (popup) popup.hidden = !hudRoot.dataset.previewMapMenu;
      hudRoot.querySelectorAll("[data-preview-map-menu]").forEach(function (panel) {
        panel.hidden = panel.dataset.previewMapMenu !== hudRoot.dataset.previewMapMenu;
      });
      paintPreview();
    });
    ["click", "change"].forEach(function (type) {
      $("#previewFrame").addEventListener(type, previewUiAction);
    });
    $("#episodeSelect").addEventListener("change", function () {
      if (state.dirty && !confirm("Discard unsaved changes and switch episode?")) {
        this.value = episodeId;
        return;
      }
      location.href = "logic.html?episode=" + encodeURIComponent(this.value);
    });
    $("#fileIn").addEventListener("change", function () {
      const file = this.files[0];
      if (!file) return;
      if (state.dirty && !confirm("Replace the unsaved draft with this file?")) {
        this.value = "";
        return;
      }
      const reader = new FileReader();
      reader.onload = function () {
        try {
          const next = JSON.parse(reader.result);
          if (next.episode_id !== episodeId) throw new Error("Episode ID does not match this editor tab.");
          if (state.isMenuGuides) {
            const report = window.SOWCampaign.validateMenuGuides(next);
            if (report.errors.length) throw new Error(report.errors[0].message);
          }
          state.definition = next;
          normalizeStrings(next);
          ensureLayout();
          state.dirty = true;
          if (state.isMenuGuides) {
            state.menuGuideId = next.menu_guides[state.menuGuideId] ? state.menuGuideId : "welcome";
            state.flow = "menu";
            state.selected = flowEntry();
            syncMenuGuideSteps();
          } else {
            state.flow = "episode";
            state.selected = next.entry;
          }
          $("#flowSelect").value = state.flow;
          syncStudioMode();
          status("Imported draft", "dirty");
          renderPalette();
          renderSettings();
          renderInspector();
          refresh();
          resetPreview();
        } catch (error) {
          notice(error.message);
        }
      };
      reader.readAsText(file);
    });
    $("#stage").addEventListener("pointermove", function (event) {
      if (!state.drag) return;
      const scale = state.zoom;
      if (state.drag.kind === "node") {
        const pos = state.definition.layout[state.drag.id];
        pos.x = Math.max(0, state.drag.origin.x + (event.clientX - state.drag.x) / scale);
        pos.y = Math.max(0, state.drag.origin.y + (event.clientY - state.drag.y) / scale);
        const card = $("#nodes [data-step-id=" + CSS.escape(state.drag.id) + "]");
        if (card) {
          card.style.left = pos.x + "px";
          card.style.top = pos.y + "px";
        }
        drawWires();
      } else {
        state.pan.x += event.movementX;
        state.pan.y += event.movementY;
        renderGraph();
      }
    });
    $("#stage").addEventListener("pointerup", function () {
      if (state.drag && state.drag.kind === "node") markDirty();
      state.drag = null;
    });
    $("#stage").addEventListener("pointerdown", function (event) {
      if (state.keyboardWire) {
        state.keyboardWire = null;
        $("#stage").dataset.connecting = "false";
        $(".graph-hint").textContent =
          "Drag outputs to inputs · keyboard: Enter on output, then Enter on destination · Escape cancels";
      }
      if (event.target === $("#stage") || event.target.id === "wires") {
        state.drag = { kind: "pan" };
        $("#stage").setPointerCapture(event.pointerId);
      }
    });
    $("#stage").addEventListener("keydown", function (event) {
      if (event.key !== "Escape" || !state.keyboardWire) return;
      event.preventDefault();
      state.keyboardWire = null;
      $("#stage").dataset.connecting = "false";
      $(".graph-hint").textContent =
        "Drag outputs to inputs · keyboard: Enter on output, then Enter on destination · Escape cancels";
    });
    document.addEventListener("pointermove", function (event) {
      if (!state.wire) return;
      const source = $("#nodes [data-step-id=" + CSS.escape(state.wire.source) + "]");
      const output = source && source.querySelectorAll(".pin.out")[state.wire.outputIndex];
      const group = $("#wires g");
      if (!output || !group) return;
      if (!state.wire.path) {
        state.wire.path = document.createElementNS("http://www.w3.org/2000/svg", "path");
        state.wire.path.classList.add("wire-pending");
        group.appendChild(state.wire.path);
      }
      const stageRect = $("#stage").getBoundingClientRect(),
        rect = output.getBoundingClientRect();
      const x1 = (rect.left + rect.width / 2 - stageRect.left - state.pan.x) / state.zoom;
      const y1 = (rect.top + rect.height / 2 - stageRect.top - state.pan.y) / state.zoom;
      const x2 = (event.clientX - stageRect.left - state.pan.x) / state.zoom;
      const y2 = (event.clientY - stageRect.top - state.pan.y) / state.zoom;
      state.wire.path.setAttribute(
        "d",
        "M" + x1 + " " + y1 + " C" + (x1 + 70) + " " + y1 + " " + (x2 - 70) + " " + y2 + " " + x2 + " " + y2,
      );
    });
    document.addEventListener("pointerup", function (event) {
      if (!state.wire) return;
      const target = document.elementFromPoint(event.clientX, event.clientY);
      const port = target && target.closest("[data-in]");
      const wire = state.wire;
      state.wire = null;
      if (wire.path && wire.path.isConnected) wire.path.remove();
      if (port) {
        setDestination(wire, port.dataset.in);
        markDirty();
      }
    });
    $("#stage").addEventListener(
      "wheel",
      function (event) {
        event.preventDefault();
        zoomAt(event.clientX, event.clientY, event.deltaY < 0 ? 1.1 : 1 / 1.1);
      },
      { passive: false },
    );
    document.querySelectorAll("[data-zoom]").forEach(function (button) {
      button.addEventListener("click", function () {
        if (button.dataset.zoom === "fit") fitGraph();
        else zoomAt(innerWidth / 2, innerHeight / 2, button.dataset.zoom === "in" ? 1.2 : 1 / 1.2);
      });
    });
    $("#arrangeGraphBtn").addEventListener("click", arrangeGraph);
    window.addEventListener(
      "resize",
      function () {
        drawWires();
        if (state.machine) paintPreview(false);
      },
      { passive: true },
    );
    window.addEventListener("beforeunload", function (event) {
      if (state.dirty) {
        event.preventDefault();
        event.returnValue = "";
      }
    });
  }
  function zoomAt(x, y, factor) {
    const rect = $("#stage").getBoundingClientRect(),
      sx = x - rect.left,
      sy = y - rect.top;
    const next = Math.max(0.45, Math.min(1.8, state.zoom * factor));
    state.pan.x = sx - ((sx - state.pan.x) * next) / state.zoom;
    state.pan.y = sy - ((sy - state.pan.y) * next) / state.zoom;
    state.zoom = next;
    $("#zlabel").textContent = Math.round(next * 100) + "%";
    renderGraph();
  }
  function fitGraph() {
    const stage = $("#stage"),
      nodes = Array.from($("#nodes").querySelectorAll(".node"));
    if (!nodes.length || !stage.clientWidth || !stage.clientHeight) return;
    const bounds = nodes.reduce(
      function (box, node) {
        const x = node.offsetLeft,
          y = node.offsetTop;
        box.left = Math.min(box.left, x);
        box.top = Math.min(box.top, y);
        box.right = Math.max(box.right, x + node.offsetWidth);
        box.bottom = Math.max(box.bottom, y + node.offsetHeight);
        return box;
      },
      { left: Infinity, top: Infinity, right: -Infinity, bottom: -Infinity },
    );
    const width = bounds.right - bounds.left,
      height = bounds.bottom - bounds.top;
    const zoom = Math.min(1.4, (stage.clientWidth - 64) / width, (stage.clientHeight - 64) / height);
    state.zoom = zoom;
    state.pan = {
      x: (stage.clientWidth - width * zoom) / 2 - bounds.left * zoom,
      y: (stage.clientHeight - height * zoom) / 2 - bounds.top * zoom,
    };
    $("#zlabel").textContent = Math.round(zoom * 100) + "%";
    renderGraph();
  }
  function arrangeGraph() {
    const steps = state.definition.steps,
      byId = Object.create(null),
      depth = Object.create(null),
      queue = [];
    steps.forEach(function (step) {
      byId[step.id] = step;
    });
    const entries = state.isMenuGuides
      ? Object.keys(state.definition.menu_guides).map(function (id) {
          return state.definition.menu_guides[id].entry;
        })
      : [state.definition.entry, state.definition.menu_guide && state.definition.menu_guide.entry];
    entries.forEach(function (entry) {
      if (byId[entry] && depth[entry] == null) {
        depth[entry] = 0;
        queue.push(entry);
      }
    });
    for (let index = 0; index < queue.length; index++) {
      const source = byId[queue[index]],
        nextDepth = depth[source.id] + 1;
      portDefinitions(source).forEach(function (port) {
        if (byId[port.target] && depth[port.target] == null) {
          depth[port.target] = nextDepth;
          queue.push(port.target);
        }
      });
    }
    const lastDepth = queue.reduce(function (max, id) {
      return Math.max(max, depth[id]);
    }, 0);
    steps.forEach(function (step) {
      if (depth[step.id] == null) depth[step.id] = lastDepth + 1;
    });
    const columns = [];
    steps.forEach(function (step) {
      (columns[depth[step.id]] || (columns[depth[step.id]] = [])).push(step);
    });
    state.definition.layout = {};
    columns.forEach(function (column, x) {
      let y = 48;
      column.forEach(function (step) {
        const node = $("#nodes [data-step-id=" + CSS.escape(step.id) + "]");
        state.definition.layout[step.id] = { x: 48 + x * 320, y: y };
        y += (node ? node.offsetHeight : 160) + 48;
      });
    });
    markDirty();
    fitGraph();
  }
  function fillLocales() {
    return fetch("/locales/index.json", { cache: "no-store" })
      .then(function (response) {
        if (!response.ok) throw new Error("locale registry unavailable");
        return response.json();
      })
      .then(function (registry) {
        const version = Number(registry && registry.version);
        if (
          !registry ||
          registry.schema !== 1 ||
          !Number.isInteger(version) ||
          version < 1 ||
          !Array.isArray(registry.languages) ||
          !registry.languages.length
        )
          throw new Error("invalid locale registry");
        localeNames = Object.create(null);
        locales = registry.languages.map(function (entry) {
          const code = String(typeof entry === "string" ? entry : entry.code).toLowerCase();
          if (typeof entry === "object" && typeof entry.name === "string" && entry.name.trim())
            localeNames[code] = entry.name;
          return code;
        });
      })
      .catch(function () {
        locales = ["en", "es"];
        state.localeRegistryFailed = true;
        localeNames = { en: "English", es: "Español" };
      });
  }
  function loadCatalog(locale) {
    const code = String(locale || "").toLowerCase();
    if (state.catalogs[code]) return Promise.resolve(true);
    if (catalogLoads[code]) return catalogLoads[code];
    const request = fetch("/locales/" + encodeURIComponent(code), { cache: "no-store" })
      .then(function (response) {
        if (!response.ok) throw new Error("locale " + code + " unavailable");
        return response.json();
      })
      .then(function (payload) {
        if (
          !payload ||
          !payload.strings ||
          typeof payload.strings !== "object" ||
          Array.isArray(payload.strings)
        )
          throw new Error("locale " + code + " has invalid strings");
        state.catalogs[code] = payload.strings;
        state.localeFailures = state.localeFailures.filter(function (failed) {
          return failed !== code;
        });
        return true;
      })
      .catch(function () {
        if (!state.localeFailures.includes(code)) state.localeFailures.push(code);
        return false;
      })
      .then(function (loaded) {
        delete catalogLoads[code];
        return loaded;
      });
    catalogLoads[code] = request;
    return request;
  }
  function fillAvatars() {
    return fetch("/__avatars", { cache: "no-store" })
      .then(function (response) {
        if (!response.ok) throw new Error("avatar list unavailable");
        return response.json();
      })
      .then(function (avatars) {
        if (
          !Array.isArray(avatars) ||
          avatars.some(function (avatar) {
            return typeof avatar !== "string" || !/^[a-z][a-z0-9_]*$/.test(avatar);
          })
        )
          throw new Error("invalid avatar list");
        state.avatars = avatars;
      });
  }
  function fillEpisodes() {
    const select = $("#episodeSelect"),
      labels = Object.create(null);
    Array.from(select.options).forEach(function (option) {
      labels[option.value] = option.textContent;
    });
    return fetch("/__episodes", { cache: "no-store" })
      .then(function (response) {
        if (!response.ok) throw new Error("episode list unavailable");
        return response.json();
      })
      .then(function (episodes) {
        if (
          !Array.isArray(episodes) ||
          !episodes.length ||
          episodes.some(function (id) {
            return typeof id !== "string" || !/^[a-z][a-z0-9_]{0,63}$/.test(id);
          })
        )
          throw new Error("invalid episode list");
        optionList(
          select,
          episodes.map(function (id) {
            return {
              value: id,
              label:
                labels[id] ||
                id.replace(/_/g, " ").replace(/\b\w/g, function (letter) {
                  return letter.toUpperCase();
                }),
            };
          }),
          episodeId,
        );
      })
      .then(function () {
        return true;
      })
      .catch(function () {
        return false;
      });
  }
  function init() {
    connectControls();
    Promise.all([fillLocales(), fillAvatars(), fillEpisodes(), load()])
      .then(function (results) {
        optionList($("#previewLocale"), localeOptions(), "en");
        state.previewLanguage = $("#previewLocale").value;
        return Promise.all([loadCatalog("en"), loadCatalog(state.previewLanguage)]).then(function () {
          if (!results[2])
            notice("Could not load the episode list; only the built-in episodes are available.");
          else if (state.localeRegistryFailed)
            notice("Language list unavailable; showing English and Spanish previews.");
          else if (state.localeFailures.length)
            notice(
              "Catalog unavailable for " +
                state.localeFailures.join(", ") +
                "; some shared-text previews may be incomplete.",
            );
          refresh();
          renderSettings();
          renderInspector();
          renderPreview();
        });
      })
      .catch(function (error) {
        notice(error.message);
        status("Load failed", "error");
      });
  }
  init();
})();
