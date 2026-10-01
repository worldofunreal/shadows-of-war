/* Shared by the game, campaign editor and build validation. No DOM or game commands. */
(function (host) {
    "use strict";

    const TYPES = ["scene", "choice", "objective", "guide", "end"];
    const METRICS = {
        territory: "tiles_gained", kills: "kills", attack: "attacks", troops: "troops",
        building: "buildings", city: "cities", farm: "farms", factory: "factories",
        port: "ports", bunker: "bunkers", structure_upgrade: "structure_upgrades",
        city_upgrade: "city_upgrades", city_level: "city_levels",
        foundry_level: "foundry_level",
        port_upgrade: "port_upgrades", port_level: "port_levels",
        tile_upgrade: "tile_upgrades", resource_transfer: "resource_transfers",
        alliance: "alliances_formed", support: "ally_support_deliveries",
        fleet: "fleets", nuke: "nukes", elapsed: "elapsed_seconds"
    };
    const WORLD_TARGETS = ["expand", "assault", "target_action", "player"];
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
        dock_farm: '#sow-hud [data-command="select_building"][data-kind="Farm"]',
        upgrade_structure: "#sow-hud-building-card-upgrade",
        transfer_gold: "#sow-hud-transfer-gold",
        transfer_troops: "#sow-hud-transfer-troops",
        transfer_send: '#sow-hud-transfer [data-command="send_resources"]',
        transfer_request: '#sow-hud-transfer [data-command="request_resources"]',
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
        const allowMissingFactionReferences = options.allowMissingFactionReferences === true;
        const errors = [], warnings = [];
        const issue = (step, field, message) => errors.push({ step: step && step.id, field, message });
        const knownFields = (value, allowed, step, field) => {
            if (object(value)) Object.keys(value).filter(key => !allowed.includes(key)).forEach(key => issue(step, field, "Unsupported field (no runtime behavior): " + key));
        };
        if (!object(definition) || definition.version !== 2) {
            issue(null, "version", "Campaign logic must use version 2.");
            return { errors, warnings };
        }
        knownFields(definition, ["version", "episode_id", "default_locale", "settings", "entry", "menu_guide", "speakers", "strings", "layout", "steps", "reactions"], null, "campaign");
        if (!id(definition.episode_id)) issue(null, "episode_id", "Invalid episode ID.");
        const settings = definition.settings;
        if (!object(settings) || typeof settings.buildings_enabled !== "boolean" || !Number.isInteger(settings.starting_troops) || settings.starting_troops < 1 || settings.starting_troops > 100000) {
            issue(null, "settings", "Choose buildings on/off and 1–100000 starting troops.");
        }
        if (object(settings)) {
            if (Object.keys(settings).some(key => !["buildings_enabled", "starting_troops", "buildings_unlock_after_defeated", "campaign_support"].includes(key))) issue(null, "settings", "Unknown match setting.");
            if (settings.buildings_unlock_after_defeated != null && typeof settings.buildings_unlock_after_defeated !== "string") issue(null, "settings.buildings_unlock_after_defeated", "Choose a faction that unlocks construction.");
            if (settings.campaign_support != null) {
                const support = settings.campaign_support;
                if (!object(support) || Object.keys(support).some(key => !["after_defeated", "share_percent"].includes(key)) || typeof support.after_defeated !== "string" || !Number.isInteger(support.share_percent) || support.share_percent < 1 || support.share_percent > 100) issue(null, "settings.campaign_support", "Choose a valid milestone and 1–100 percent share.");
            }
        }
        const factions = new Set(["player"]), rosterFactions = new Set();
        if (roster) {
            knownFields(roster, ["_comment", "map", "player_spawn", "player_color", "factions"], null, "roster");
            const expectedMap = definition.episode_id === "boudica" ? ["eastanglia", 896, 504]
                : /^six_sky_ep[123]$/.test(definition.episode_id) ? ["northamerica", 1000, 516] : null;
            if (!object(roster) || typeof roster.map !== "string" || !/^[a-z0-9_-]+$/.test(roster.map) || !Array.isArray(roster.player_spawn) || roster.player_spawn.length !== 2 || !roster.player_spawn.every(n => Number.isInteger(n) && n >= 0) || !Array.isArray(roster.factions) || !roster.factions.length) {
                issue(null, "roster", "Invalid map, player spawn or factions.");
            }
            if (expectedMap && roster.map !== expectedMap[0]) issue(null, "roster.map", "This episode uses the " + expectedMap[0] + " map.");
            if (roster.player_color != null && (typeof roster.player_color !== "string" || !/^#[0-9a-fA-F]{6}$/.test(roster.player_color))) issue(null, "roster.player_color", "Use a hex color such as #f0902a.");
            if (expectedMap && Array.isArray(roster.player_spawn) && roster.player_spawn.length === 2 && roster.player_spawn.every(Number.isInteger) && (roster.player_spawn[0] >= expectedMap[1] || roster.player_spawn[1] >= expectedMap[2])) issue(null, "roster.player_spawn", "Player spawn is outside the campaign map.");
            (Array.isArray(roster.factions) ? roster.factions : []).forEach(faction => {
                if (!object(faction) || typeof faction.name !== "string" || !faction.name.trim() || factions.has(faction.name)) {
                    issue(null, "roster", "Faction names must be unique and nonempty.");
                    return;
                }
                knownFields(faction, ["name", "x", "y", "role", "relation", "hostility", "betrayal", "color", "iq", "civ", "leader", "avatar", "support_interval_seconds", "alliance_group"], null, "roster.factions");
                factions.add(faction.name);
                rosterFactions.add(faction.name);
                if (!["kin", "independent", "vassal", "boss", "big_boss", "neutral"].includes(faction.role) || ![faction.x, faction.y].every(n => Number.isInteger(n) && n >= 0)) issue(null, "roster", "Invalid faction role or spawn: " + faction.name);
                else if (expectedMap && (faction.x >= expectedMap[1] || faction.y >= expectedMap[2])) issue(null, "roster", "Faction spawn is outside the campaign map: " + faction.name);
                if (faction.relation != null && !["neutral", "allied", "enemy"].includes(faction.relation)) issue(null, "roster.factions.relation", "Choose neutral, allied or enemy for " + faction.name + ".");
                if (faction.hostility != null && !["passive", "aggressive"].includes(faction.hostility)) issue(null, "roster.factions.hostility", "Choose passive or aggressive for " + faction.name + ".");
                if (faction.betrayal != null && !["never", "opportunistic"].includes(faction.betrayal)) issue(null, "roster.factions.betrayal", "Choose never or opportunistic for " + faction.name + ".");
                if (faction.color != null && (typeof faction.color !== "string" || !/^#[0-9a-fA-F]{6}$/.test(faction.color))) issue(null, "roster.factions.color", "Use a six-digit hex color for " + faction.name + ".");
                if (faction.support_interval_seconds != null && (!Number.isInteger(faction.support_interval_seconds) || faction.support_interval_seconds < 5 || faction.support_interval_seconds > 600)) issue(null, "roster.factions.support_interval_seconds", "Support intervals must be 5–600 seconds.");
                if (faction.alliance_group != null && (typeof faction.alliance_group !== "string" || !/^[a-z][a-z0-9_]{0,63}$/.test(faction.alliance_group))) issue(null, "roster.factions.alliance_group", "Use a lowercase alliance group ID.");
                if (faction.avatar != null && (!/^[a-z][a-z0-9_]*$/.test(faction.avatar) || faction.avatar !== "null" && options.hasAvatar && !options.hasAvatar(faction.avatar))) issue(null, "roster.factions.avatar", "Choose an existing portrait for " + faction.name + ".");
            });
            if (!allowMissingFactionReferences && object(settings) && settings.buildings_unlock_after_defeated && !rosterFactions.has(settings.buildings_unlock_after_defeated)) issue(null, "settings.buildings_unlock_after_defeated", "Construction unlock refers to an unknown faction.");
            if (!allowMissingFactionReferences && object(settings) && object(settings.campaign_support) && !rosterFactions.has(settings.campaign_support.after_defeated)) issue(null, "settings.campaign_support.after_defeated", "Support milestone refers to an unknown faction.");
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
            knownFields(speaker, ["name", "name_key", "avatar", "faction"], null, "speakers." + key);
            if (!id(key) || !object(speaker) || !(typeof speaker.name === "string" && speaker.name.trim() || typeof speaker.name_key === "string" && speaker.name_key.trim() || typeof speaker.faction === "string" && speaker.faction.trim())) issue(null, "speakers", "Invalid character: " + key);
            else {
                if (speaker.avatar != null && (!/^[a-z][a-z0-9_]*$/.test(speaker.avatar) || speaker.avatar !== "null" && options.hasAvatar && !options.hasAvatar(speaker.avatar))) issue(null, "speakers", "Choose an existing avatar: " + key);
                if (!allowMissingFactionReferences && speaker.faction != null && !rosterFactions.has(speaker.faction)) issue(null, "speakers." + key + ".faction", "Choose an existing roster faction.");
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
            scene: ["id", "type", "title_key", "body_key", "speaker", "presentation", "lines", "next", "routes", "attack_ratio_on_enter"],
            choice: ["id", "type", "title_key", "body_key", "speaker", "choices", "attack_ratio_on_enter"],
            objective: ["id", "type", "title_key", "body_key", "hint_key", "speaker", "trigger", "guide", "marker", "next", "routes", "attack_ratio_on_enter"],
            guide: ["id", "type", "title_key", "body_key", "hint_key", "speaker", "trigger", "guide", "marker", "next", "routes", "attack_ratio_on_enter"],
            end: ["id", "type", "title_key", "body_key", "speaker", "presentation", "attack_ratio_on_enter"]
        };
        steps.filter(object).forEach(step => {
            knownFields(step, stepFields[step.type] || ["id", "type"], step, "fields");
            if (!TYPES.includes(step.type)) issue(step, "type", "Unknown step type.");
            if (own(step, "attack_ratio_on_enter") && (!Number.isFinite(step.attack_ratio_on_enter) || step.attack_ratio_on_enter < 0.05 || step.attack_ratio_on_enter > 1)) issue(step, "attack_ratio_on_enter", "Attack ratio must be between 0.05 and 1.");
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
                    knownFields(trigger, ["type", "scope", "value", "target", "targets", "action", "unit", "recipient", "resources"], step, "trigger");
                    if (!["step", "episode", "total"].includes(trigger.scope)) issue(step, "trigger.scope", "Choose when the objective starts counting.");
                    if (trigger.type === "contact") {
                        if (trigger.targets != null) {
                            if (!Array.isArray(trigger.targets) || !trigger.targets.length || new Set(trigger.targets).size !== trigger.targets.length || trigger.targets.some(target => typeof target !== "string" || (roster && !rosterFactions.has(target) && !allowMissingFactionReferences))) issue(step, "trigger.targets", "Choose one or more distinct existing factions.");
                            if (trigger.value != null && (!Number.isInteger(trigger.value) || trigger.value < 1 || trigger.value > trigger.targets.length)) issue(step, "trigger.value", "Choose how many of the selected factions must be contacted.");
                        } else if (!trigger.target || trigger.target === "player" || (roster && !factions.has(trigger.target) && !allowMissingFactionReferences)) issue(step, "trigger.target", "Choose an existing faction.");
                    } else if (trigger.type === "defeated") {
                        if (trigger.targets != null) {
                            if (!Array.isArray(trigger.targets) || !trigger.targets.length || new Set(trigger.targets).size !== trigger.targets.length || trigger.targets.some(target => typeof target !== "string" || (!rosterFactions.has(target) && !allowMissingFactionReferences))) issue(step, "trigger.targets", "Choose one or more distinct existing factions.");
                            if (!Number.isFinite(trigger.value) || trigger.value !== trigger.targets.length) issue(step, "trigger.value", "The required amount must match the selected factions.");
                        } else if (!trigger.target || trigger.target === "player" || (roster && !factions.has(trigger.target) && !allowMissingFactionReferences)) issue(step, "trigger.target", "Choose an existing faction.");
                    } else if (trigger.type === "ui") {
                        if (!own(UI_TARGETS, trigger.action)) issue(step, "trigger.action", "Choose an existing control.");
                    } else {
                        if (!Number.isFinite(trigger.value) || trigger.value <= 0) issue(step, "trigger.value", "Objective value must be greater than zero.");
                        if (trigger.type === "fleet" && trigger.unit != null && !["TransportShip", "TradeShip", "Warship"].includes(trigger.unit)) issue(step, "trigger.unit", "Choose a ship type supported by the game.");
                        if (trigger.type === "fleet" && trigger.target && trigger.unit && trigger.unit !== "TransportShip") issue(step, "trigger.target", "Only a transport can be tied to a landing target.");
                        if (trigger.type === "resource_transfer" && trigger.resources != null && (!Array.isArray(trigger.resources) || !trigger.resources.length || new Set(trigger.resources).size !== trigger.resources.length || trigger.resources.some(resource => !["gold", "troops"].includes(resource)))) issue(step, "trigger.resources", "Choose gold, troops, or both.");
                    }
                    if (["contact", "defeated", "alliance", "resource_transfer"].includes(trigger.type)) {
                        const factionTarget = trigger.target || trigger.recipient;
                        if (trigger.recipient != null && trigger.type !== "resource_transfer") issue(step, "trigger.recipient", "Only resource transfers have a recipient.");
                        if (factionTarget && roster && !factions.has(factionTarget) && !allowMissingFactionReferences) issue(step, "trigger.target", "Unknown faction.");
                    }
                    if (trigger.unit != null && trigger.type !== "fleet") issue(step, "trigger.unit", "Only fleet objectives can select a ship type.");
                    if (trigger.resources != null && trigger.type !== "resource_transfer") issue(step, "trigger.resources", "Only transfer objectives can require specific resources.");
                    if (trigger.recipient != null && roster && !factions.has(trigger.recipient) && !allowMissingFactionReferences) issue(step, "trigger.recipient", "Unknown transfer recipient.");
                    if (trigger.target && roster && !factions.has(trigger.target) && !allowMissingFactionReferences) issue(step, "trigger.target", "Unknown faction.");
                }
                if (trigger && trigger.type === "elapsed" && step.guide) issue(step, "guide", "Timed waits do not need a hand guide.");
            } else if (step.trigger) issue(step, "trigger", "Only objectives and guides have completion conditions.");
            knownFields(step.marker, ["target"], step, "marker");
            if (step.marker && (!object(step.marker) || !step.marker.target || (roster && !factions.has(step.marker.target) && !allowMissingFactionReferences))) issue(step, "marker", "Unknown marked faction.");
            if (step.guide) {
                const guide = step.guide;
                knownFields(guide, ["kind", "target", "gesture", "to"], step, "guide");
                if (!["objective", "guide"].includes(step.type)) issue(step, "guide", "Only mechanics use the hand; decisions never do.");
                if (!object(guide) || !["world", "ui"].includes(guide.kind) || !["tap", "hold", "drag"].includes(guide.gesture)) issue(step, "guide", "Choose a world/control target and a gesture.");
                else if (guide.kind === "world" ? !WORLD_TARGETS.includes(guide.target) : !own(UI_TARGETS, guide.target)) issue(step, "guide.target", "Unknown guide target.");
                if (guide.gesture === "drag" && (guide.kind === "world" ? !WORLD_TARGETS.includes(guide.to) : guide.to != null && !own(UI_TARGETS, guide.to))) issue(step, "guide.to", "Choose a valid drag destination.");
            } else if (step.type === "guide") issue(step, "guide", "A guide step needs a hand target.");
        });
        const reactions = Array.isArray(definition.reactions) ? definition.reactions : [];
        if (definition.reactions != null && !Array.isArray(definition.reactions)) issue(null, "reactions", "Responses must be a list.");
        const reactionIds = new Set(), reactionEvents = new Set();
        reactions.forEach(reaction => {
            if (!object(reaction)) return issue(null, "reactions", "Invalid story response.");
            knownFields(reaction, ["id", "after", "when", "speaker", "title_key", "body_key", "outcome", "gold_cost", "choices"], null, "reactions");
            if (!id(reaction.id) || byId.has(reaction.id) || reactionIds.has(reaction.id)) issue(null, "reactions.id", "Response IDs must be unique and distinct from story steps.");
            reactionIds.add(reaction.id);
            knownFields(reaction.when, ["type", "target", "targets", "relation", "role"], null, "reactions.when");
            const when = reaction.when, type = object(when) && when.type, target = object(when) && when.target;
            if (!["contact", "first_contact", "support"].includes(type)) issue(null, "reactions.when", "Choose a supported contact or support event.");
            if (type === "contact") {
                const hasTarget = typeof target === "string";
                const hasRelation = typeof when.relation === "string";
                if (hasTarget === hasRelation || hasRelation && when.relation !== "neutral" || hasTarget && roster && !rosterFactions.has(target) && !allowMissingFactionReferences) issue(null, "reactions.when", "Choose one faction or all initially neutral factions.");
            }
            if (type === "support" && (typeof target !== "string" || (roster && !rosterFactions.has(target) && !allowMissingFactionReferences))) issue(null, "reactions.when.target", "Choose an existing faction.");
            if (type !== "contact" && when && when.relation != null || type !== "first_contact" && when && (when.targets != null || when.role != null)) issue(null, "reactions.when", "That response condition is not valid for this event.");
            if (type === "first_contact") {
                const targets = when && when.targets;
                const hasTargets = Array.isArray(targets) && targets.length > 0;
                const hasRole = typeof when.role === "string";
                if (target != null || hasTargets === hasRole || hasRole && !["kin", "independent", "vassal", "boss", "big_boss", "neutral"].includes(when.role) || hasTargets && (new Set(targets).size !== targets.length || targets.some(name => typeof name !== "string" || (roster && !rosterFactions.has(name) && !allowMissingFactionReferences)))) issue(null, "reactions.when", "Choose one faction group or distinct existing factions for the first-contact scene.");
                if (reaction.after != null) issue(null, "reactions.after", "First contact happens immediately and cannot wait for another step.");
            }
            if (type === "contact" && reaction.after != null) issue(null, "reactions.after", "Contact responses happen immediately and cannot wait for another step.");
            if (type === "support") {
                const gate = byId.get(reaction.after);
                if (!gate || !["objective", "guide"].includes(gate.type)) issue(null, "reactions.after", "Choose the objective that unlocks this support response.");
                const supportingFaction = (roster && roster.factions || []).find(faction => faction.name === target);
                if (roster && (supportingFaction ? !Number.isInteger(supportingFaction.support_interval_seconds) : !allowMissingFactionReferences)) issue(null, "reactions.when.target", "This faction is not configured to send campaign support.");
            }
            const eventKey = type === "first_contact" ? type : type + ":" + (target || when && when.relation);
            if (reactionEvents.has(eventKey)) issue(null, "reactions.when", "Each event can have only one response.");
            if (type === "first_contact" || typeof target === "string" || when && when.relation) reactionEvents.add(eventKey);
            if (reaction.speaker && !own(speakers, reaction.speaker)) issue(null, "reactions.speaker", "Choose an existing character.");
            text(null, "reactions.title_key", reaction.title_key, true);
            text(null, "reactions.body_key", reaction.body_key, true);
            if (reaction.outcome != null && !["neutral", "allied", "enemy"].includes(reaction.outcome)) issue(null, "reactions.outcome", "Choose a neutral, allied or enemy result.");
            if (reaction.gold_cost != null && (!Number.isFinite(reaction.gold_cost) || reaction.gold_cost < 0 || reaction.gold_cost > 1_000_000 || (reaction.gold_cost > 0 && reaction.outcome !== "allied"))) issue(null, "reactions.gold_cost", "Gold can only be paid for an alliance (0–1,000,000).");
            if (reaction.choices != null) {
                if (!Array.isArray(reaction.choices) || reaction.choices.length < 2 || reaction.choices.length > 4 || reaction.outcome != null || !["contact", "first_contact"].includes(type)) issue(null, "reactions.choices", "Contact negotiations need 2–4 choices and no automatic result.");
                const choiceIds = new Set();
                (Array.isArray(reaction.choices) ? reaction.choices : []).forEach(choice => {
                    if (!object(choice)) return issue(null, "reactions.choices", "Invalid negotiation choice.");
                    knownFields(choice, ["id", "label_key", "body_key", "relation", "gold_cost"], null, "reactions.choices");
                    if (!id(choice.id) || choiceIds.has(choice.id)) issue(null, "reactions.choices.id", "Negotiation choice IDs must be unique.");
                    choiceIds.add(choice.id);
                    text(null, "reactions.choices.label_key", choice.label_key, true);
                    text(null, "reactions.choices.body_key", choice.body_key, false);
                    if (!["neutral", "allied", "enemy"].includes(choice.relation)) issue(null, "reactions.choices.relation", "Every choice needs a neutral, allied or enemy result.");
                    if (choice.gold_cost != null && (!Number.isFinite(choice.gold_cost) || choice.gold_cost < 0 || choice.gold_cost > 1_000_000 || (choice.gold_cost > 0 && choice.relation !== "allied"))) issue(null, "reactions.choices.gold_cost", "Gold can only be paid for an alliance (0–1,000,000).");
                });
            }
            if (reaction.outcome != null && reaction.choices != null) issue(null, "reactions", "Choose an automatic result or negotiation choices, not both.");
            if (type === "support" && (reaction.outcome != null || reaction.gold_cost != null || reaction.choices != null)) issue(null, "reactions", "Support notices cannot change diplomacy.");
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
            byId.forEach(step => {
                const targets = edges(step);
                const canReachEnding = step.type === "choice"
                    ? targets.some(target => ends.has(target))
                    : targets.length && targets.every(target => ends.has(target));
                if (!ends.has(step.id) && canReachEnding) { ends.add(step.id); changed = true; }
            });
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

    function create(definition, entry, roster) {
        // Each run owns its definition; editor changes cannot mutate an active game.
        definition = copy(definition);
        const byId = new Map(definition.steps.map(step => [step.id, step]));
        const state = { id: null, line: 0, choices: Object.create(null), completed: [], reactionsShown: [], reactionChoices: Object.create(null), contactsResolved: [], firstContactTarget: null, done: false };
        let facts = {}, ui = {}, initial = null, initialUi = {}, baseline = {}, uiBaseline = {}, externallyPaused = false;
        function enter(key) {
            if (!byId.has(key)) throw new Error("Unknown campaign destination: " + key);
            state.id = key; state.line = 0; state.done = false;
            baseline = copy(facts); uiBaseline = { ...ui };
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
                if (trigger.type === "contact" && Array.isArray(trigger.targets)) {
                    current = trigger.targets.filter(name => (facts[field] || []).includes(name) && !(reference[field] || []).includes(name)).length;
                    target = Number(trigger.value || 1);
                } else if (trigger.type === "defeated" && Array.isArray(trigger.targets)) {
                    current = trigger.targets.filter(name => (facts[field] || []).includes(name) && !(reference[field] || []).includes(name)).length;
                    target = Number(trigger.value || trigger.targets.length);
                } else {
                    current = (facts[field] || []).includes(trigger.target) && !(reference[field] || []).includes(trigger.target) ? 1 : 0;
                    target = 1;
                }
            } else if (trigger.type === "ui") {
                const uiReference = trigger.scope === "step" ? uiBaseline : trigger.scope === "episode" ? initialUi : {};
                current = Number(ui[trigger.action] || 0) - Number(uiReference[trigger.action] || 0);
                target = 1;
            } else if (trigger.type === "alliance" && trigger.target) {
                current = (facts.alliance_names || []).includes(trigger.target) && !(reference.alliance_names || []).includes(trigger.target) ? 1 : 0;
                target = 1;
            } else if (trigger.type === "troops") {
                current = Number(facts.troops || 0);
            } else if (trigger.type === "attack" && trigger.target) {
                current = Number((facts.attacks_by_target || {})[trigger.target] || 0) - Number((reference.attacks_by_target || {})[trigger.target] || 0);
            } else if (trigger.type === "fleet" && trigger.target) {
                const byTarget = Number((facts.transport_fleets_by_target || {})[trigger.target] || 0) - Number((reference.transport_fleets_by_target || {})[trigger.target] || 0);
                const byType = trigger.unit ? Number((facts.fleets_by_type || {})[trigger.unit] || 0) - Number((reference.fleets_by_type || {})[trigger.unit] || 0) : byTarget;
                current = Math.min(byTarget, byType);
            } else if (trigger.type === "fleet" && trigger.unit) {
                current = Number((facts.fleets_by_type || {})[trigger.unit] || 0) - Number((reference.fleets_by_type || {})[trigger.unit] || 0);
            } else if (trigger.type === "resource_transfer" && trigger.recipient) {
                const resources = trigger.resources && trigger.resources.slice().sort().join("_") || "total";
                const currentCounts = (facts.resource_transfers_by_recipient || {})[trigger.recipient] || {};
                const baselineCounts = (reference.resource_transfers_by_recipient || {})[trigger.recipient] || {};
                current = Number(currentCounts[resources] || 0) - Number(baselineCounts[resources] || 0);
            } else {
                let field = METRICS[trigger.type];
                if (trigger.type === "territory" && trigger.scope === "total") field = "tiles";
                current = Number(facts[field] || 0) - Number(reference[field] || 0);
            }
            return { current: Math.min(target, Math.max(0, current)), target };
        }
        let activeReaction = null;
        function nextReaction() {
            const contactNames = facts.contact_names || [];
            const gameplayActive = !["scene", "choice", "end"].includes(byId.get(state.id).type);
            return (definition.reactions || []).flatMap((reaction, index) => {
                const type = reaction.when.type;
                const isFirstContact = type === "first_contact";
                const isContact = type === "contact" || isFirstContact;
                let targets = [];
                if (isFirstContact && !state.firstContactTarget) {
                    const eligible = reaction.when.role
                        ? (roster && roster.factions || []).filter(faction => faction.role === reaction.when.role).map(faction => faction.name)
                        : reaction.when.targets || [];
                    targets = eligible.filter(name => contactNames.includes(name) && !state.contactsResolved.includes(name)).slice(0, 1);
                } else if (type === "contact" && reaction.when.target) {
                    if (contactNames.includes(reaction.when.target) && !state.contactsResolved.includes(reaction.when.target)) targets = [reaction.when.target];
                } else if (type === "contact" && reaction.when.relation) {
                    const factions = new Map((roster && roster.factions || []).map(faction => [faction.name, faction]));
                    targets = contactNames.filter(name => !state.contactsResolved.includes(name)
                        && ((factions.get(name) && factions.get(name).relation || "neutral") === reaction.when.relation));
                }
                if (isContact) return targets.map(target => ({ reaction, index, target, isContact, isFirstContact, priority: isFirstContact ? 0 : reaction.when.target ? 1 : 2, instanceId: reaction.id + "@" + target }));
                const receipt = type === "support" && facts.support_deliveries_by_faction && facts.support_deliveries_by_faction[reaction.when.target];
                const ready = gameplayActive && state.completed.includes(reaction.after) && receipt && receipt.deliveries > 0;
                return ready ? [{ reaction, index, receipt, target: reaction.when.target, isContact: false, isFirstContact: false, priority: 3, instanceId: reaction.id }] : [];
            }).filter(item => !state.reactionsShown.includes(item.instanceId))
                .sort((a, b) => a.priority - b.priority
                    || (Number(a.receipt && a.receipt.first_tick || 0) - Number(b.receipt && b.receipt.first_tick || 0))
                    || a.index - b.index)[0] || null;
        }
        function view() {
            const step = byId.get(state.id);
            if (activeReaction) {
                const reaction = activeReaction.reaction;
                const hasChoices = Array.isArray(reaction.choices) && reaction.choices.length > 0;
                const factionSpeaker = reaction.when.type === "first_contact" || reaction.when.relation
                    ? Object.keys(definition.speakers || {}).find(key => definition.speakers[key].faction === activeReaction.target)
                    : null;
                const response = {
                    id: "reaction-" + activeReaction.instanceId, type: hasChoices ? "choice" : "scene", speaker: reaction.speaker || factionSpeaker,
                    title_key: reaction.title_key, body_key: reaction.body_key, presentation: "dialogue"
                };
                return { definition, step: response, line: response, progress: progress(), paused: true, done: false, choices: reaction.choices || [], state, reaction: reaction.id, reactionInstance: activeReaction.instanceId, reactionTarget: activeReaction.target, reactionData: reaction };
            }
            const line = step.lines ? step.lines[state.line] : step;
            return { definition, step, line, progress: progress(), paused: ["scene", "choice", "end"].includes(step.type), done: state.done, choices: step.choices || [], state };
        }
        function setPaused(paused) { externallyPaused = Boolean(paused); }
        function advance(choiceId, expectedStepId) {
            if (state.done || externallyPaused) return false;
            if (activeReaction) {
                const reaction = activeReaction.reaction;
                if (expectedStepId !== "reaction-" + activeReaction.instanceId) return false;
                if (reaction.choices && reaction.choices.length) {
                    const answer = reaction.choices.find(option => option.id === choiceId);
                    if (!answer) return false;
                    state.reactionChoices[activeReaction.instanceId] = answer.id;
                }
                if (!state.reactionsShown.includes(activeReaction.instanceId)) state.reactionsShown.push(activeReaction.instanceId);
                if (["contact", "first_contact"].includes(reaction.when.type) && !state.contactsResolved.includes(activeReaction.target)) state.contactsResolved.push(activeReaction.target);
                activeReaction = null;
                return true;
            }
            if (expectedStepId && expectedStepId !== state.id) return false;
            const step = byId.get(state.id);
            if (step.type === "scene" && step.lines && state.line + 1 < step.lines.length) { state.line++; return true; }
            if (["objective", "guide"].includes(step.type)) {
                const result = progress();
                if (result.current < result.target) return false;
            }
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
        function update(nextFacts, nextUi) {
            facts = copy(nextFacts || {}); ui = { ...(nextUi || {}) };
            if (initial == null) { initial = copy(facts); initialUi = { ...ui }; baseline = copy(facts); uiBaseline = { ...ui }; }
            if (state.done || externallyPaused) return view();
            while (!state.done) {
                if (!activeReaction) {
                    const next = nextReaction();
                    if (next) {
                        if (next.isFirstContact) state.firstContactTarget = next.target;
                        activeReaction = { reaction: next.reaction, target: next.target, instanceId: next.instanceId };
                    }
                }
                if (activeReaction) return view();
                const step = byId.get(state.id);
                if (!["objective", "guide"].includes(step.type)) break;
                const result = progress();
                if (result.current < result.target || !advance(null, step.id)) break;
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
            if (resetProgress) { baseline = copy(facts); uiBaseline = { ...ui }; }
            return true;
        }
        enter(entry || definition.entry);
        return { get definition() { return definition; }, state, update, advance, jump, replaceDefinition, setPaused, view };
    }
    const api = { TYPES, METRICS, UI_TARGETS, resolveUiTarget, resolveUiAnchor, validate, create };
    if (typeof module !== "undefined" && module.exports) module.exports = api;
    else host.SOWCampaign = api;
})(typeof globalThis !== "undefined" ? globalThis : this);
