/* Shared by the game, campaign editor and build validation. No DOM or game commands. */
(function (host) {
    "use strict";

    const TYPES = ["scene", "choice", "objective", "guide", "end"];
    const METRICS = {
        territory: "tiles_gained", kills: "kills", attack: "attacks", troops: "troops",
        building: "buildings", fleet: "fleets", nuke: "nukes", elapsed: "elapsed_seconds"
    };
    const WORLD_TARGETS = ["expand", "assault", "target_action"];
    const UI_TARGETS = {
        menu_campaign: '#sow-menu [data-command="open_campaign"]',
        campaign_replay: '#sow-menu [data-command="start_campaign_episode"][data-episode-id]',
        menu_multiplayer: '#sow-menu [data-command="quick_match"]',
        menu_heroes: '#sow-menu [data-nav-screen="heroes"]',
        menu_profile: '#sow-menu [data-nav-screen="profile"]',
        attack_ratio: "#sow-hud-slider",
        hud_zoom_in: '#sow-hud [data-command="zoom_in"]',
        hud_zoom_out: '#sow-hud [data-command="zoom_out"]',
        hud_center_camera: '#sow-hud [data-command="center_camera"]',
        hud_inbox: '#sow-hud [data-command="toggle_inbox"]',
        dock_city: '#sow-hud [data-command="select_building"][data-kind="City"]',
        dock_factory: '#sow-hud [data-command="select_building"][data-kind="Factory"]',
        dock_port: '#sow-hud [data-command="select_building"][data-kind="Port"]',
        dock_bunker: '#sow-hud [data-command="select_building"][data-kind="Bunker"]',
        map_spawn: '#sow-hud [data-map-action="spawn"]',
        map_attack: '#sow-hud [data-map-action="attack"]',
        map_transfer: '#sow-hud [data-map-action="transfer"]',
        map_fleet: '#sow-hud [data-map-action="fleet"]',
        map_alliance: '#sow-hud [data-map-action="alliance"]',
        map_build: '#sow-hud [data-map-group="build"]',
        map_nuke: '#sow-hud [data-map-action="nuke"]',
        map_build_city: '#sow-hud [data-map-action="build_city"]',
        map_build_factory: '#sow-hud [data-map-action="build_factory"]',
        map_build_port: '#sow-hud [data-map-action="build_port"]',
        map_build_bunker: '#sow-hud [data-map-action="build_bunker"]',
        map_build_warship: '#sow-hud [data-map-action="build_warship"]',
        map_build_trade_ship: '#sow-hud [data-map-action="build_trade_ship"]',
        map_upgrade_tile: '#sow-hud [data-map-action="upgrade_tile"]',
        map_upgrade_arsenal: '#sow-hud [data-map-action="upgrade_arsenal"]',
        map_upgrade_port: '#sow-hud [data-map-action="upgrade_port"]',
        map_upgrade_foundry: '#sow-hud [data-map-action="upgrade_foundry"]',
        map_nuke_launch: '#sow-hud [data-map-action="nuke"]',
        map_back: '#sow-hud [data-map-back]',
        rankings: '#sow-hud [data-command="toggle_leaderboard"]',
        settings: '#sow-hud [data-command="toggle_settings"]'
    };
    const own = (object, key) => object != null && Object.prototype.hasOwnProperty.call(object, key);
    const object = value => value !== null && typeof value === "object" && !Array.isArray(value);
    const id = value => typeof value === "string" && /^[a-z][a-z0-9_-]{0,95}$/.test(value) && !["constructor", "prototype", "__proto__"].includes(value);
    const copy = value => JSON.parse(JSON.stringify(value));

    function resolveUiTarget(key, root, episodeId) {
        const selector = UI_TARGETS[key];
        if (!selector || !root) return null;
        if (key === "campaign_replay") {
            if (!episodeId || typeof root.querySelectorAll !== "function") return null;
            return Array.from(root.querySelectorAll(selector)).find(node => node.dataset && node.dataset.episodeId === episodeId) || null;
        }
        return typeof root.querySelector === "function" ? root.querySelector(selector) : null;
    }

    function resolveUiAnchor(element) {
        const rect = element.getBoundingClientRect();
        const x = Number.parseFloat(element.dataset && element.dataset.tutorialAnchorX);
        const y = Number.parseFloat(element.dataset && element.dataset.tutorialAnchorY);
        const focus = Number.isFinite(x) && Number.isFinite(y) && element.querySelector && element.querySelector(".sow-hud__map-action-title");
        const focusRect = focus && focus.getClientRects().length ? focus.getBoundingClientRect() : rect;
        return {
            x: focus ? rect.left + rect.width * x / 100 : rect.left + rect.width / 2,
            y: focus ? rect.top + rect.height * y / 100 : rect.top + rect.height / 2,
            width: focusRect.width,
            height: focusRect.height
        };
    }

    function validate(definition, roster, options) {
        options = options || {};
        const errors = [], warnings = [];
        const issue = (step, field, message) => errors.push({ step: step && step.id, field, message });
        const knownFields = (value, allowed, step, field) => {
            if (object(value)) Object.keys(value).filter(key => !allowed.includes(key)).forEach(key => issue(step, field, "Unsupported field (no runtime behavior): " + key));
        };
        if (!object(definition) || definition.version !== 2) {
            issue(null, "version", "Campaign logic must use version 2.");
            return { errors, warnings };
        }
        knownFields(definition, ["version", "episode_id", "default_locale", "settings", "entry", "menu_guide", "speakers", "strings", "layout", "steps"], null, "campaign");
        if (!id(definition.episode_id)) issue(null, "episode_id", "Invalid episode ID.");
        const settings = definition.settings;
        if (!object(settings) || typeof settings.buildings_enabled !== "boolean" || !Number.isInteger(settings.starting_troops) || settings.starting_troops < 1 || settings.starting_troops > 100000) {
            issue(null, "settings", "Choose buildings on/off and 1–100000 starting troops.");
        }
        if (object(settings) && Object.keys(settings).some(key => !["buildings_enabled", "starting_troops"].includes(key))) issue(null, "settings", "Unknown match setting.");
        const factions = new Set(["player"]);
        if (roster) {
            knownFields(roster, ["_comment", "map", "player_spawn", "factions"], null, "roster");
            const expectedMap = definition.episode_id === "boudica" ? ["eastanglia", 896, 504]
                : /^six_sky_ep[123]$/.test(definition.episode_id) ? ["northamerica", 1000, 516] : null;
            if (!object(roster) || typeof roster.map !== "string" || !/^[a-z0-9_-]+$/.test(roster.map) || !Array.isArray(roster.player_spawn) || roster.player_spawn.length !== 2 || !roster.player_spawn.every(n => Number.isInteger(n) && n >= 0) || !Array.isArray(roster.factions) || !roster.factions.length) {
                issue(null, "roster", "Invalid map, player spawn or factions.");
            }
            if (expectedMap && roster.map !== expectedMap[0]) issue(null, "roster.map", "This episode uses the " + expectedMap[0] + " map.");
            if (expectedMap && Array.isArray(roster.player_spawn) && roster.player_spawn.length === 2 && roster.player_spawn.every(Number.isInteger) && (roster.player_spawn[0] >= expectedMap[1] || roster.player_spawn[1] >= expectedMap[2])) issue(null, "roster.player_spawn", "Player spawn is outside the campaign map.");
            (Array.isArray(roster.factions) ? roster.factions : []).forEach(faction => {
                if (!object(faction) || typeof faction.name !== "string" || !faction.name.trim() || factions.has(faction.name)) {
                    issue(null, "roster", "Faction names must be unique and nonempty.");
                    return;
                }
                knownFields(faction, ["name", "x", "y", "role", "iq", "civ", "leader"], null, "roster.factions");
                factions.add(faction.name);
                if (!["kin", "independent", "vassal", "boss", "big_boss", "neutral"].includes(faction.role) || ![faction.x, faction.y].every(n => Number.isInteger(n) && n >= 0)) issue(null, "roster", "Invalid faction role or spawn: " + faction.name);
                else if (expectedMap && (faction.x >= expectedMap[1] || faction.y >= expectedMap[2])) issue(null, "roster", "Faction spawn is outside the campaign map: " + faction.name);
            });
        }
        const strings = definition.strings || {};
        const languageList = Array.isArray(strings) ? strings : object(strings) ? Object.keys(strings) : [];
        if ((!object(strings) && !Array.isArray(strings)) || !/^[a-z]{2,3}(?:-[a-z]{2})?$/.test(definition.default_locale || "") || !languageList.includes(definition.default_locale)) issue(null, "strings", "Choose a base language and language dictionaries.");
        Object.entries(object(strings) ? strings : {}).forEach(([locale, catalog]) => {
            if (!/^[a-z]{2,3}(?:-[a-z]{2})?$/.test(locale) || !object(catalog)) issue(null, "strings", "Invalid language dictionary: " + locale);
            else Object.entries(catalog).forEach(([key, value]) => {
                if (!/^tutorial\.[a-zA-Z0-9_.-]+$/.test(key) || typeof value !== "string" || value.length > 12000) issue(null, "strings", "Invalid episode text: " + key);
            });
        });
        const base = object(strings[definition.default_locale]) ? strings[definition.default_locale] : {};
        function text(step, field, key, required) {
            if (key == null && !required) return;
            if (typeof key !== "string" || !/^tutorial\.[a-zA-Z0-9_.-]+$/.test(key)) issue(step, field, "Choose a tutorial text key.");
            else if (!(own(base, key) && base[key].trim()) && options.hasText && !options.hasText(key)) issue(step, field, "Missing base-language text: " + key);
        }
        const speakers = definition.speakers || {};
        if (!object(speakers)) issue(null, "speakers", "Invalid character dictionary.");
        else Object.entries(speakers).forEach(([key, speaker]) => {
            knownFields(speaker, ["name", "name_key", "avatar"], null, "speakers." + key);
            if (!id(key) || !object(speaker) || !(typeof speaker.name === "string" && speaker.name.trim() || typeof speaker.name_key === "string" && speaker.name_key.trim())) issue(null, "speakers", "Invalid character: " + key);
            else {
                if (speaker.avatar != null && (!/^[a-z][a-z0-9_]*$/.test(speaker.avatar) || options.hasAvatar && !options.hasAvatar(speaker.avatar))) issue(null, "speakers", "Choose an existing avatar: " + key);
                text(null, "speakers", speaker.name_key, false);
            }
        });
        const steps = Array.isArray(definition.steps) ? definition.steps : [];
        if (!steps.length || steps.length > 512) issue(null, "steps", "Use between 1 and 512 steps.");
        const byId = new Map();
        steps.forEach(step => {
            if (!object(step) || !id(step.id) || byId.has(step.id)) issue(step, "id", "Step IDs must be valid and unique.");
            else byId.set(step.id, step);
        });
        if (definition.layout != null) {
            if (!object(definition.layout)) issue(null, "layout", "Step layout must be an object.");
            else Object.entries(definition.layout).forEach(([stepId, position]) => {
                if (!byId.has(stepId)) issue(null, "layout", "Layout refers to an unknown step: " + stepId);
                knownFields(position, ["x", "y"], null, "layout." + stepId);
                if (!object(position) || !Number.isFinite(position.x) || !Number.isFinite(position.y)) issue(null, "layout." + stepId, "Step positions need numeric x and y values.");
            });
        }
        if (!byId.has(definition.entry)) issue(null, "entry", "Select an existing opening step.");
        knownFields(definition.menu_guide, ["entry", "dismissible"], null, "menu_guide");
        if (definition.menu_guide != null && (!object(definition.menu_guide) || !id(definition.menu_guide.entry) || !byId.has(definition.menu_guide.entry) || (definition.menu_guide.dismissible != null && typeof definition.menu_guide.dismissible !== "boolean"))) {
            issue(null, "menu_guide", "The optional menu guide needs an existing opening step and a dismissible setting.");
        }
        function destination(step, field, target) {
            if (!byId.has(target)) issue(step, field, "Unknown destination: " + String(target || "(empty)"));
        }
        function condition(step, when) {
            if (!object(when) || ["choice", "fact"].filter(key => own(when, key)).length !== 1) return issue(step, "routes", "Choose one condition: player decision or game fact.");
            knownFields(when, own(when, "choice") ? ["choice", "equals"] : ["fact", "gte"], step, "routes.when");
            if (own(when, "choice")) {
                const choice = byId.get(when.choice);
                if (!choice || choice.type !== "choice" || !Array.isArray(choice.choices) || !choice.choices.some(option => option.id === when.equals)) issue(step, "routes", "Condition refers to an unknown decision or answer.");
            } else if (!Object.values(METRICS).concat(["tiles", "contacts", "defeated"]).includes(when.fact) || !Number.isFinite(when.gte)) issue(step, "routes", "Invalid game fact condition.");
        }
        const stepFields = {
            scene: ["id", "type", "title_key", "body_key", "speaker", "presentation", "lines", "next", "routes"],
            choice: ["id", "type", "title_key", "body_key", "speaker", "choices"],
            objective: ["id", "type", "title_key", "body_key", "hint_key", "speaker", "trigger", "guide", "marker", "next", "routes"],
            guide: ["id", "type", "title_key", "body_key", "hint_key", "speaker", "trigger", "guide", "marker", "next", "routes"],
            end: ["id", "type", "title_key", "body_key", "speaker", "presentation"]
        };
        steps.filter(object).forEach(step => {
            knownFields(step, stepFields[step.type] || ["id", "type"], step, "fields");
            if (!TYPES.includes(step.type)) issue(step, "type", "Unknown step type.");
            ["title_key", "body_key", "hint_key"].forEach(field => text(step, field, step[field], false));
            if (step.speaker && !own(speakers, step.speaker)) issue(step, "speaker", "Unknown character.");
            if (step.presentation && !["dialogue", "chapter"].includes(step.presentation)) issue(step, "presentation", "Unknown presentation.");
            if (step.lines != null) {
                if (step.type !== "scene" || !Array.isArray(step.lines) || !step.lines.length || step.lines.length > 64) issue(step, "lines", "A conversation needs 1–64 lines.");
                else step.lines.forEach(line => {
                    if (!object(line)) return issue(step, "lines", "Invalid conversation line.");
                    knownFields(line, ["speaker", "title_key", "body_key"], step, "lines");
                    text(step, "lines", line.body_key, true);
                    text(step, "lines", line.title_key, false);
                    if (line.speaker && !own(speakers, line.speaker)) issue(step, "lines", "Unknown speaking character.");
                });
            }
            if (!step.title_key && !step.body_key && !step.lines) issue(step, "title_key", "Add a title, text or conversation.");
            if (step.type === "end") {
                if (step.next || step.routes || step.choices) issue(step, "next", "An ending has no outgoing connections.");
            } else if (step.type === "choice") {
                if (!Array.isArray(step.choices) || step.choices.length < 2 || step.choices.length > 4) issue(step, "choices", "A decision needs 2–4 answers.");
                const seen = new Set();
                (Array.isArray(step.choices) ? step.choices : []).forEach(option => {
                    if (!object(option)) return issue(step, "choices", "Invalid answer.");
                    knownFields(option, ["id", "label_key", "body_key", "next"], step, "choices");
                    if (!id(option.id) || seen.has(option.id)) issue(step, "choices", "Answer IDs must be unique.");
                    seen.add(option.id);
                    text(step, "choices", option.label_key, true);
                    text(step, "choices", option.body_key, false);
                    destination(step, "choices", option.next);
                });
                const choiceTargets = (Array.isArray(step.choices) ? step.choices : []).filter(option => object(option) && byId.has(option.next)).map(option => option.next);
                const choiceUsedLater = steps.some(item => object(item) && item !== step && Array.isArray(item.routes) && item.routes.some(route => object(route) && object(route.when) && route.when.choice === step.id));
                if (step.choices && step.choices.length > 1 && choiceTargets.length === step.choices.length && new Set(choiceTargets).size === 1 && !choiceUsedLater) {
                    warnings.push({ step: step.id, field: "choices", message: "Both answers lead to the same step and no later scene uses this decision." });
                }
                if (step.next || step.routes) issue(step, "next", "Connect each answer to its own next step.");
            } else destination(step, "next", step.next);
            if (step.routes != null) {
                if (!Array.isArray(step.routes)) issue(step, "routes", "Routes must be a list.");
                else step.routes.forEach((route, index) => {
                    if (!object(route)) return issue(step, "routes", "Invalid conditional route.");
                    knownFields(route, ["when", "next"], step, "routes");
                    knownFields(route.when, ["choice", "equals", "fact", "gte"], step, "routes.when");
                    destination(step, "routes", route.next);
                    condition(step, route.when);
                    const when = route.when;
                    if (!object(when)) return;
                    const shadowed = step.routes.slice(0, index).findIndex(previous => {
                        const prior = previous && previous.when;
                        if (!object(prior)) return false;
                        if (typeof when.choice === "string") return prior.choice === when.choice && prior.equals === when.equals;
                        return typeof when.fact === "string" && prior.fact === when.fact && Number.isFinite(prior.gte) && Number.isFinite(when.gte) && prior.gte <= when.gte;
                    });
                    if (shadowed >= 0) warnings.push({ step: step.id, field: "routes", message: "Route " + (index + 1) + " is unreachable because earlier route " + (shadowed + 1) + " matches first." });
                });
            }
            if (["objective", "guide"].includes(step.type)) {
                const trigger = step.trigger;
                if (!object(trigger) || !Object.keys(METRICS).concat(["contact", "defeated", "ui"]).includes(trigger.type)) issue(step, "trigger", "Choose a supported objective.");
                else {
                    knownFields(trigger, ["type", "scope", "value", "target", "action"], step, "trigger");
                    if (!["step", "episode", "total"].includes(trigger.scope)) issue(step, "trigger.scope", "Choose when the objective starts counting.");
                    if (["contact", "defeated"].includes(trigger.type)) {
                        if (!trigger.target || trigger.target === "player" || (roster && !factions.has(trigger.target))) issue(step, "trigger.target", "Choose an existing faction.");
                    } else if (trigger.type === "ui") {
                        if (!own(UI_TARGETS, trigger.action)) issue(step, "trigger.action", "Choose an existing control.");
                    } else if (!Number.isFinite(trigger.value) || trigger.value <= 0) issue(step, "trigger.value", "Objective value must be greater than zero.");
                    if (trigger.target && roster && !factions.has(trigger.target)) issue(step, "trigger.target", "Unknown faction.");
                }
                if (trigger && trigger.type === "elapsed" && step.guide) issue(step, "guide", "Timed waits do not need a hand guide.");
            } else if (step.trigger) issue(step, "trigger", "Only objectives and guides have completion conditions.");
            knownFields(step.marker, ["target"], step, "marker");
            if (step.marker && (!object(step.marker) || !step.marker.target || (roster && !factions.has(step.marker.target)))) issue(step, "marker", "Unknown marked faction.");
            if (step.guide) {
                const guide = step.guide;
                knownFields(guide, ["kind", "target", "gesture", "to"], step, "guide");
                if (!["objective", "guide"].includes(step.type)) issue(step, "guide", "Only mechanics use the hand; decisions never do.");
                if (!object(guide) || !["world", "ui"].includes(guide.kind) || !["tap", "hold", "drag"].includes(guide.gesture)) issue(step, "guide", "Choose a world/control target and a gesture.");
                else if (guide.kind === "world" ? !WORLD_TARGETS.includes(guide.target) : !own(UI_TARGETS, guide.target)) issue(step, "guide.target", "Unknown guide target.");
                if (guide.gesture === "drag" && (guide.kind === "world" ? !WORLD_TARGETS.includes(guide.to) : guide.to != null && !own(UI_TARGETS, guide.to))) issue(step, "guide.to", "Choose a valid drag destination.");
            } else if (step.type === "guide") issue(step, "guide", "A guide step needs a hand target.");
        });
        const edges = step => [step.next].concat((Array.isArray(step.routes) ? step.routes : []).map(r => r && r.next), (Array.isArray(step.choices) ? step.choices : []).map(c => c && c.next)).filter(target => byId.has(target));
        const menuFlowReachable = new Set();
        if (object(definition.menu_guide) && byId.has(definition.menu_guide.entry)) {
            const visitMenu = key => { if (menuFlowReachable.has(key) || !byId.has(key)) return; menuFlowReachable.add(key); edges(byId.get(key)).forEach(visitMenu); };
            visitMenu(definition.menu_guide.entry);
            const menuEnds = new Set(steps.filter(step => step && step.type === "end").map(step => step.id));
            let menuChanged = true;
            while (menuChanged) {
                menuChanged = false;
                byId.forEach(step => { const targets = edges(step); if (!menuEnds.has(step.id) && targets.length && targets.every(target => menuEnds.has(target))) { menuEnds.add(step.id); menuChanged = true; } });
            }
            menuFlowReachable.forEach(key => {
                const step = byId.get(key);
                if (!menuEnds.has(key)) issue(step, "next", "Every menu-guide path must lead to an ending.");
                if (["objective", "guide"].includes(step.type) && (!step.trigger || step.trigger.type !== "ui" || (step.guide && step.guide.kind !== "ui"))) issue(step, "trigger", "The menu guide can teach menu controls, not battlefield actions.");
                if (step.trigger && step.trigger.type === "ui" && (typeof step.trigger.action !== "string" || (!step.trigger.action.startsWith("menu_") && step.trigger.action !== "campaign_replay"))) issue(step, "trigger.action", "Choose a campaign-menu control for the return guide.");
            });
        }
        const reached = new Set();
        const visit = key => { if (reached.has(key) || !byId.has(key)) return; reached.add(key); edges(byId.get(key)).forEach(visit); };
        visit(definition.entry);
        byId.forEach(step => { if (!reached.has(step.id) && !menuFlowReachable.has(step.id)) warnings.push({ step: step.id, field: "id", message: "This step is not connected to either flow." }); });
        const ends = new Set(steps.filter(s => s && s.type === "end").map(s => s.id));
        let changed = true;
        while (changed) {
            changed = false;
            byId.forEach(step => { const targets = edges(step); if (!ends.has(step.id) && targets.length && targets.every(target => ends.has(target))) { ends.add(step.id); changed = true; } });
        }
        reached.forEach(key => { if (!ends.has(key)) issue(byId.get(key), "next", "This path cannot reach an ending."); });
        const checked = new Set(), visiting = new Set();
        const canAdvanceWithoutInput = step => {
            if (!step || !["objective", "guide"].includes(step.type) || !step.trigger) return false;
            return step.trigger.type === "elapsed" || step.trigger.type === "troops" || step.trigger.scope !== "step";
        };
        function cycle(key) {
            const step = byId.get(key);
            if (!canAdvanceWithoutInput(step) || checked.has(key)) return;
            if (visiting.has(key)) { issue(step, "next", "Automatic steps form a loop. Add a conversation or decision."); return; }
            visiting.add(key); edges(step).forEach(cycle); visiting.delete(key); checked.add(key);
        }
        byId.forEach(step => cycle(step.id));
        return { errors, warnings };
    }

    function create(definition, entry) {
        // Each run owns its definition; editor changes cannot mutate an active game.
        definition = copy(definition);
        const byId = new Map(definition.steps.map(step => [step.id, step]));
        const state = { id: null, line: 0, choices: Object.create(null), completed: [], done: false };
        let facts = {}, ui = {}, initial = null, initialUi = {}, baseline = {}, uiBaseline = {}, readyAt = null, pausedAt = null, now = 0;
        function enter(key) {
            if (!byId.has(key)) throw new Error("Unknown campaign destination: " + key);
            state.id = key; state.line = 0; state.done = false;
            baseline = copy(facts); uiBaseline = { ...ui }; readyAt = null;
        }
        function matches(when) {
            if (when.choice) return state.choices[when.choice] === when.equals;
            return Number(facts[when.fact] || 0) >= when.gte;
        }
        function progress() {
            const step = byId.get(state.id), trigger = step.trigger;
            if (!trigger) return { current: 0, target: 0 };
            const reference = trigger.scope === "step" ? baseline : trigger.scope === "episode" ? initial || {} : {};
            let current = 0, target = Number(trigger.value || 1);
            if (trigger.type === "contact" || trigger.type === "defeated") {
                const field = trigger.type === "contact" ? "contact_names" : "defeated_names";
                current = (facts[field] || []).includes(trigger.target) && !(reference[field] || []).includes(trigger.target) ? 1 : 0;
                target = 1;
            } else if (trigger.type === "ui") {
                const uiReference = trigger.scope === "step" ? uiBaseline : trigger.scope === "episode" ? initialUi : {};
                current = Number(ui[trigger.action] || 0) - Number(uiReference[trigger.action] || 0);
                target = 1;
            } else if (trigger.type === "troops") {
                current = Number(facts.troops || 0);
            } else if (trigger.type === "attack" && trigger.target) {
                current = Number((facts.attacks_by_target || {})[trigger.target] || 0) - Number((reference.attacks_by_target || {})[trigger.target] || 0);
            } else {
                let field = METRICS[trigger.type];
                if (trigger.type === "territory" && trigger.scope === "total") field = "tiles";
                current = Number(facts[field] || 0) - Number(reference[field] || 0);
            }
            return { current: Math.min(target, Math.max(0, current)), target };
        }
        function view() {
            const step = byId.get(state.id);
            const line = step.lines ? step.lines[state.line] : step;
            return { definition, step, line, progress: progress(), paused: ["scene", "choice", "end"].includes(step.type), ready: readyAt != null, done: state.done, choices: step.choices || [], state };
        }
        function setPaused(paused, nowMs) {
            const at = Number.isFinite(nowMs) ? nowMs : now;
            if (paused) {
                if (pausedAt == null) pausedAt = at;
            } else if (pausedAt != null) {
                if (readyAt != null) readyAt += Math.max(0, at - pausedAt);
                pausedAt = null;
            }
        }
        function advance(choiceId, expectedStepId) {
            if (state.done || pausedAt != null || (expectedStepId && expectedStepId !== state.id)) return false;
            const step = byId.get(state.id);
            if (step.type === "scene" && step.lines && state.line + 1 < step.lines.length) { state.line++; return true; }
            if (["objective", "guide"].includes(step.type) && readyAt == null) return false;
            if (step.type === "end") {
                if (!state.completed.includes(step.id)) state.completed.push(step.id);
                state.done = true;
                return true;
            }
            let destination = step.next;
            if (step.type === "choice") {
                const answer = step.choices.find(option => option.id === choiceId);
                if (!answer) return false;
                state.choices[step.id] = answer.id;
                destination = answer.next;
            } else {
                const route = (step.routes || []).find(route => matches(route.when));
                if (route) destination = route.next;
            }
            if (!byId.has(destination)) return false;
            if (!state.completed.includes(step.id)) state.completed.push(step.id);
            enter(destination);
            return true;
        }
        function update(nextFacts, nextUi, nowMs) {
            now = Number.isFinite(nowMs) ? nowMs : now;
            facts = copy(nextFacts || {}); ui = { ...(nextUi || {}) };
            if (initial == null) { initial = copy(facts); initialUi = { ...ui }; baseline = copy(facts); uiBaseline = { ...ui }; }
            if (state.done || pausedAt != null) return view();
            const step = byId.get(state.id);
            if (["objective", "guide"].includes(step.type)) {
                const result = progress();
                if (result.current >= result.target && readyAt == null) readyAt = now;
                // A short completion beat belongs to presentation; game waits use elapsed_seconds.
                if (readyAt != null && now - readyAt >= 800) advance(null, step.id);
            }
            return view();
        }
        function jump(key, nextFacts, priorChoices) {
            if (nextFacts) facts = copy(nextFacts);
            Object.entries(object(priorChoices) ? priorChoices : {}).forEach(([decisionId, answerId]) => {
                const decision = byId.get(decisionId);
                if (decision && decision.type === "choice" && Array.isArray(decision.choices) && decision.choices.some(answer => answer.id === answerId)) state.choices[decisionId] = answerId;
            });
            enter(key);
            return view();
        }
        function replaceDefinition(nextDefinition) {
            if (!object(nextDefinition) || !Array.isArray(nextDefinition.steps)) return false;
            const replacement = copy(nextDefinition);
            const nextById = new Map(replacement.steps.filter(object).map(step => [step.id, step]));
            const previousStep = byId.get(state.id), nextStep = nextById.get(state.id);
            if (!nextStep) return false;
            const resetProgress = !previousStep || previousStep.type !== nextStep.type || JSON.stringify(previousStep.trigger) !== JSON.stringify(nextStep.trigger);
            definition = replacement;
            byId.clear();
            nextById.forEach((step, key) => byId.set(key, step));
            state.line = Math.min(state.line, Math.max(0, (nextStep.lines || []).length - 1));
            state.done = state.done && nextStep.type === "end";
            state.completed = state.completed.filter(key => byId.has(key));
            Object.keys(state.choices).forEach(key => {
                const decision = byId.get(key);
                if (!decision || !Array.isArray(decision.choices) || !decision.choices.some(answer => answer && answer.id === state.choices[key])) delete state.choices[key];
            });
            if (resetProgress) { baseline = copy(facts); uiBaseline = { ...ui }; readyAt = null; }
            return true;
        }
        enter(entry || definition.entry);
        return { get definition() { return definition; }, state, update, advance, jump, replaceDefinition, setPaused, view };
    }
    const api = { TYPES, METRICS, UI_TARGETS, resolveUiTarget, resolveUiAnchor, validate, create };
    if (typeof module !== "undefined" && module.exports) module.exports = api;
    else host.SOWCampaign = api;
})(typeof globalThis !== "undefined" ? globalThis : this);
