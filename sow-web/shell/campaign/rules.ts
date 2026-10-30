// Campaign data rules shared by validation, runtime, and authoring tools.
import type {
    CampaignDefinition,
    CampaignFaction,
    CampaignRecord,
    CampaignStep,
    CampaignStepType,
    CampaignUnlocks,
} from "./types.ts";

export const TYPES: CampaignStepType[] = ["scene", "choice", "objective", "guide", "end"];
export const TEAMS = Object.freeze(["Red", "Blue"]);
export const CAMERA_SHOTS = Object.freeze(["reveal", "approach", "threat", "link"]);
// Emote = a short bubble on the speaker's nameplate. Exchange = ordered campaign lines;
// a reaction can play one automatically, and a line can use the shared dialogue modal.
export const EMOTE_KINDS = Object.freeze(["greet", "help", "threat", "celebrate"]);
// Dialogue fields a contact response shows; validated and rendered from this one list.
export const REACTION_DIALOGUE_FIELDS = Object.freeze([
    "speaker_role_key",
    "title_key",
    "show_title",
    "body_key",
]);
export const WAIT_MS = 1100;
// Match EMOTE_DURATION_SECS in sow-core. Clear the previous bubble before another text is installed.
export const EXCHANGE_LINE_MS = 3000;
export const EXCHANGE_GAP_MS = 250;
export const EXCHANGE_EMOTE_ACK_TIMEOUT_MS = 2500;
export const EXCHANGE_DEFAULT_REPLY_DELAY_MS = 700;
export const EXCHANGE_MAX_DELAY_MS = 10000;
export const EMOTE_TEXT_MAX_CHARS = 160;
export const METRICS: Record<string, string> = {
    territory: "tiles_gained",
    kills: "kills",
    attack: "attacks",
    troops: "troops",
    building: "buildings",
    city: "cities",
    farm: "farms",
    factory: "factories",
    port: "ports",
    tower: "towers",
    structure_upgrade: "structure_upgrades",
    structure_level: "structure_levels",
    city_upgrade: "city_upgrades",
    city_level: "city_levels",
    port_upgrade: "port_upgrades",
    port_level: "port_levels",
    resource_transfer: "resource_transfers",
    alliance: "alliances_formed",
    support: "ally_support_deliveries",
    alliance_request: "alliance_requests_submitted",
    resources_available: "resources_available",
    fleet: "fleets",
    nuke: "nukes",
    elapsed: "elapsed_seconds",
    zoom_in: "zoom_in_events",
    zoom_out: "zoom_out_events",
    camera_drag: "camera_drag_events",
    camera_key_pan: "camera_key_pan_events",
    hover: "hover_events",
    zoom_out_complete: "zoom_out_complete",
    zoom_in_complete: "zoom_in_complete",
    camera_target: "camera_target",
};
export const WORLD_TARGETS = [
    "expand",
    "assault",
    "target_action",
    "player",
    "nameplate",
    "build_site",
    "upgrade_building",
    "upgrade_structure",
];
export const STRUCTURE_LEVEL_LIMITS: Record<string, number> = {
    City: 4,
    Farm: 4,
    Factory: 4,
    Tower: 4,
    Port: 4,
};
export const CAMPAIGN_ACTIONS: readonly string[] = ["transport_fleet", "trade_ship", "warship", "nuke"];
export const CAMPAIGN_ACTION_BUILDING_REQUIREMENTS: Record<string, { kind: string; level: number } | null> =
    Object.freeze({
        transport_fleet: null,
        trade_ship: Object.freeze({ kind: "Port", level: 1 }),
        warship: Object.freeze({ kind: "Port", level: 2 }),
        nuke: Object.freeze({ kind: "City", level: STRUCTURE_LEVEL_LIMITS.City }),
    });
export const BUILDING_METRIC_KINDS: Record<string, string> = {
    city: "City",
    city_level: "City",
    city_upgrade: "City",
    farm: "Farm",
    farm_level: "Farm",
    farm_upgrade: "Farm",
    factory: "Factory",
    factory_level: "Factory",
    factory_upgrade: "Factory",
    tower: "Tower",
    tower_level: "Tower",
    tower_upgrade: "Tower",
    port: "Port",
    port_level: "Port",
    port_upgrade: "Port",
};
export function emptyCampaignUnlocks(): CampaignUnlocks {
    return { buildings: {}, actions: [] };
}
export function applyCampaignUnlocks(
    previous: CampaignUnlocks | null,
    grant: Partial<CampaignUnlocks> | null,
): CampaignUnlocks {
    const result = copy(previous || emptyCampaignUnlocks());
    const buildings: Record<string, number> = grant && object(grant.buildings) ? grant.buildings : {};
    Object.keys(buildings).forEach((kind) => {
        result.buildings[kind] = Math.max(Number(result.buildings[kind]) || 0, Number(buildings[kind]) || 0);
    });
    const actions = new Set(
        (result.actions || []).concat(grant && Array.isArray(grant.actions) ? grant.actions : []),
    );
    result.actions = CAMPAIGN_ACTIONS.filter((action) => actions.has(action));
    return result;
}
export function intersectCampaignUnlocks(a: CampaignUnlocks, b: CampaignUnlocks): CampaignUnlocks {
    const buildings: Record<string, number> = {};
    Object.keys(STRUCTURE_LEVEL_LIMITS).forEach((kind) => {
        const level = Math.min(
            Number(a && a.buildings && a.buildings[kind]) || 0,
            Number(b && b.buildings && b.buildings[kind]) || 0,
        );
        if (level > 0) buildings[kind] = level;
    });
    const actions = CAMPAIGN_ACTIONS.filter(
        (action) => ((a && a.actions) || []).includes(action) && ((b && b.actions) || []).includes(action),
    );
    return { buildings, actions };
}
export function campaignUnlocksEqual(a: CampaignUnlocks, b: CampaignUnlocks): boolean {
    return (
        Object.keys(STRUCTURE_LEVEL_LIMITS).every(
            (kind) =>
                (Number(a && a.buildings && a.buildings[kind]) || 0) ===
                (Number(b && b.buildings && b.buildings[kind]) || 0),
        ) &&
        CAMPAIGN_ACTIONS.every(
            (action) =>
                ((a && a.actions) || []).includes(action) === ((b && b.actions) || []).includes(action),
        )
    );
}
export function campaignAllowsBuilding(
    unlocks: CampaignUnlocks | null,
    kind: string,
    level: number,
): boolean {
    return Boolean(
        STRUCTURE_LEVEL_LIMITS[kind] &&
        Number(unlocks && unlocks.buildings && unlocks.buildings[kind]) >= level,
    );
}
export function campaignAllowsAction(unlocks: CampaignUnlocks | null, action: string): boolean {
    if (
        !CAMPAIGN_ACTIONS.includes(action) ||
        !(unlocks && Array.isArray(unlocks.actions) && unlocks.actions.includes(action))
    )
        return false;
    const requirement = CAMPAIGN_ACTION_BUILDING_REQUIREMENTS[action];
    return !requirement || campaignAllowsBuilding(unlocks, requirement.kind, requirement.level);
}
export function campaignEdges(
    step: CampaignStep,
    byId: Map<string, CampaignStep>,
    choices?: CampaignRecord,
    facts?: CampaignRecord,
): string[] {
    if (step.type === "choice" && choices && choices[step.id]) {
        const selected = (step.choices || []).find(
            (choice: CampaignRecord) => choice && choice.id === choices[step.id],
        );
        return selected && byId.has(selected.next) ? [selected.next] : [];
    }
    if (Array.isArray(step.routes) && (choices || facts)) {
        const route = step.routes.find((candidate: CampaignRecord) => {
            const when = candidate && candidate.when;
            if (!object(when)) return false;
            if (when.choice) return choices && choices[when.choice] === when.equals;
            if (when.fact && facts && Number.isFinite(Number(facts[when.fact])))
                return Number(facts[when.fact]) >= Number(when.gte);
            return false;
        });
        if (route) return byId.has(route.next) ? [route.next] : [];
        const allConditionsKnown = step.routes.every((candidate: CampaignRecord) => {
            const when = candidate && candidate.when;
            return (
                object(when) &&
                (when.choice
                    ? choices && own(choices, when.choice)
                    : when.fact && facts && Number.isFinite(Number(facts[when.fact])))
            );
        });
        if (allConditionsKnown)
            return typeof step.next === "string" && byId.has(step.next) ? [step.next] : [];
    }
    return [step.next]
        .concat(step.unavailable_next || [])
        .concat(
            (Array.isArray(step.routes) ? step.routes : []).map(
                (route: CampaignRecord) => route && route.next,
            ),
        )
        .concat(
            (Array.isArray(step.choices) ? step.choices : []).map(
                (choice: CampaignRecord) => choice && choice.next,
            ),
        )
        .filter((target): target is string => typeof target === "string" && byId.has(target));
}
export function campaignUnlockStates(
    byId: Map<string, CampaignStep>,
    entry: string,
    choices?: CampaignRecord,
    facts?: CampaignRecord,
): Map<string, CampaignUnlocks> {
    const states = new Map<string, CampaignUnlocks>();
    if (!byId.has(entry)) return states;
    states.set(entry, emptyCampaignUnlocks());
    const queue = [entry];
    const queued = new Set(queue);
    while (queue.length) {
        const key = queue.shift()!;
        queued.delete(key);
        const step = byId.get(key)!;
        const outgoing = applyCampaignUnlocks(states.get(key) ?? null, step && step.unlocks);
        campaignEdges(step, byId, choices, facts).forEach((target) => {
            const previous = states.get(target);
            const merged = previous ? intersectCampaignUnlocks(previous, outgoing) : outgoing;
            if (!previous || !campaignUnlocksEqual(previous, merged)) {
                states.set(target, merged);
                if (!queued.has(target)) {
                    queued.add(target);
                    queue.push(target);
                }
            }
        });
    }
    return states;
}
export function campaignUnlocksAtStep(
    byId: Map<string, CampaignStep>,
    entry: string,
    key: string,
    choices?: CampaignRecord,
    facts?: CampaignRecord,
): CampaignUnlocks {
    const step = byId.get(key);
    return step
        ? applyCampaignUnlocks(
              campaignUnlockStates(byId, entry, choices, facts).get(key) ?? null,
              step.unlocks,
          )
        : emptyCampaignUnlocks();
}
export function stepCampaignRequirements(step: CampaignStep): CampaignRecord[] {
    const requirements: CampaignRecord[] = [];
    const trigger = (step && step.trigger) || {};
    const addBuilding = (kind: string, level: number) => {
        if (STRUCTURE_LEVEL_LIMITS[kind])
            requirements.push({ type: "building", kind, level: Math.max(1, Number(level) || 1) });
    };
    const addAction = (action: string) => {
        if (!CAMPAIGN_ACTIONS.includes(action)) return;
        requirements.push({ type: "action", action });
        const building = CAMPAIGN_ACTION_BUILDING_REQUIREMENTS[action];
        if (building) addBuilding(building.kind, building.level);
    };
    const triggerKind = BUILDING_METRIC_KINDS[trigger.type];
    if (triggerKind)
        addBuilding(
            triggerKind,
            trigger.type.endsWith("_level") ? trigger.value : trigger.type.endsWith("_upgrade") ? 2 : 1,
        );
    if (trigger.type === "structure_level") addBuilding(trigger.kind, trigger.value);
    if (trigger.type === "building_selected")
        addBuilding(trigger.kind, step.guide && step.guide.target === "upgrade_building" ? 2 : 1);
    if (trigger.type === "structure_upgrade") addBuilding(trigger.kind, 2);
    if (trigger.type === "nuke") addAction("nuke");
    if (trigger.type === "fleet") {
        const fleetActions: Record<string, string> = {
            TransportShip: "transport_fleet",
            TradeShip: "trade_ship",
            Warship: "warship",
        };
        const fleetAction = fleetActions[trigger.unit];
        if (fleetAction) addAction(fleetAction);
    }
    const uiTargets = [trigger.action, step.guide && step.guide.target].filter(
        (value) => typeof value === "string",
    );
    uiTargets.forEach((target) => {
        const dock = /^dock_(city|factory|port|tower|farm)$/.exec(target);
        if (dock) addBuilding(dock[1] === "tower" ? "Tower" : dock[1][0].toUpperCase() + dock[1].slice(1), 1);
        if (target === "dock_trade_ship") addAction("trade_ship");
        if (target === "dock_warship") addAction("warship");
        if (target === "dock_nuke" || target === "map_nuke" || target === "map_nuke_launch")
            addAction("nuke");
        if (target === "map_fleet") addAction("transport_fleet");
    });
    return requirements;
}
export function zoomOutProgress(facts: CampaignRecord): number {
    const start = Number(facts && facts.camera_zoom_start);
    const floor = Number(facts && facts.camera_zoom_floor);
    const current = Number(facts && facts.camera_zoom);
    if (![start, floor, current].every(Number.isFinite) || start <= floor)
        return Number((facts && facts.zoom_out_complete) || 0);
    return Math.max(0, Math.min(1, (start - current) / (start - floor)));
}
export function zoomOutTarget(facts: CampaignRecord, amount: number): number {
    const start = Number(facts && facts.camera_zoom_start);
    const floor = Number(facts && facts.camera_zoom_floor);
    const portion = Number(amount);
    if (![start, floor].every(Number.isFinite) || start <= floor) return floor;
    const normalizedPortion = Number.isFinite(portion) ? Math.max(0, Math.min(1, portion)) : 1;
    if (normalizedPortion === 1) return floor;
    return start - (start - floor) * normalizedPortion;
}
export const UI_TARGETS: Record<string, string> = {
    menu_campaign: '#sow-menu [data-command="open_campaign"]',
    campaign_replay: '#sow-menu [data-command="start_campaign_episode"][data-episode-id]',
    menu_lobby: "#sow-menu .sow-menu__home-public [data-lobby-card]",
    menu_heroes: '#sow-menu [data-nav-screen="heroes"]',
    attack_ratio: "#sow-hud-slider",
    hud_center_camera: '#sow-hud [data-command="center_camera"]',
    hud_inbox: '#sow-hud [data-command="toggle_inbox"]',
    dock_city: '#sow-hud [data-command="select_building"][data-kind="City"]',
    dock_factory: '#sow-hud [data-command="select_building"][data-kind="Factory"]',
    dock_port: '#sow-hud [data-command="select_building"][data-kind="Port"]',
    dock_tower: '#sow-hud [data-command="select_building"][data-kind="Tower"]',
    dock_farm: '#sow-hud [data-command="select_building"][data-kind="Farm"]',
    dock_trade_ship: '#sow-hud [data-fleet-kind="trade"]',
    dock_warship: '#sow-hud [data-command="select_warship"]',
    dock_nuke: '#sow-hud [data-command="select_nuke"]',
    transfer_gold: "#sow-hud-transfer-gold",
    transfer_troops: "#sow-hud-transfer-troops",
    transfer_send: '#sow-hud-transfer [data-command="send_resources"]',
    transfer_request: '#sow-hud-transfer [data-command="request_resources"]',
    map_menu_open: "#sow-hud-map-menu",
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
    map_build_tower: '#sow-hud [data-map-action="build_tower"]',
    map_nuke_launch: '#sow-hud [data-map-action="nuke"]',
    map_back: "#sow-hud [data-map-back]",
    rankings: '#sow-hud [data-command="toggle_leaderboard"]',
    settings: '#sow-hud [data-command="toggle_settings"]',
};
export const ROUTE_FACTS: readonly string[] = Object.freeze([
    ...Object.values(METRICS),
    "tiles",
    "contacts",
    "defeated",
    "touch_controls",
]);
export const FACTION_FIELDS: readonly string[] = Object.freeze([
    "id",
    "name",
    "x",
    "y",
    "starting_troops",
    "relation",
    "team",
    "can_request_alliance",
    "can_attack_player",
    "color",
    "iq",
    "civ",
    "leader",
    "avatar",
    "support_interval_seconds",
    "gold_loot_bonus",
    "gold_loot_override",
    "alliance_group",
    "assault_force",
    "hold_structures_until_contact",
    "city_requires_gold",
    "accepts_alliance_requests",
]);
export const MENU_GUIDE_OUTCOMES: Record<string, CampaignRecord> = Object.freeze({
    heroes_ready: Object.freeze({ control: "menu_heroes", kind: "screen", screen: "heroes" }),
});

export function zoomInputMode(info: CampaignRecord = {}): "pinch" | "trackpad" | "wheel" {
    info = info || {};
    const platform = String(info.platform || "");
    const userAgent = String(info.userAgent || "");
    const appleTouchDevice = /mac/i.test(platform) && Number(info.maxTouchPoints) > 1;
    if (info.androidTwa || info.mobile || appleTouchDevice || /android|iphone|ipad|ipod/i.test(userAgent))
        return "pinch";
    return /mac/i.test(platform) ? "trackpad" : "wheel";
}
export function guideAction(step: CampaignStep): string | null {
    const guide = step && step.guide;
    const trigger = (step && step.trigger) || {};
    if (!guide || guide.gesture !== "tap") return null;
    const target = guide.target;
    if (trigger.type === "structure_level") return "upgrade_progress";
    if (trigger.type === "building_selected") return "select";
    if (trigger.type === "structure_upgrade") return "upgrade";
    if (trigger.type === "resource_transfer") return "send";
    if (trigger.type === "alliance_request") return "select";
    if (["territory", "attack", "kills", "defeated", "eliminated"].includes(trigger.type)) {
        return trigger.type === "territory" ? "expand" : "attack";
    }
    if (["contact", "alliance"].includes(trigger.type)) return target === "expand" ? "expand" : "contact";
    if (["city", "factory", "tower"].includes(trigger.type)) return "build";
    if (trigger.type === "ui") {
        if (trigger.action === "map_transfer" || target === "transfer_send")
            return target === "transfer_send" ? "send" : "select";
        if (target === "hud_center_camera") return "home";
        if (target === "menu_lobby") return "return";
        if (/^dock_/.test(target || "")) return "build";
        return "select";
    }
    if (target === "expand") return "expand";
    if (target === "assault" || target === "target_action" || target === "nameplate") return "attack";
    if (target === "build_site") return "build";
    if (target === "upgrade_building" || target === "confirm_building_placement") return "select";
    return "interact";
}
export const own = (object: CampaignRecord | null | undefined, key: string): boolean =>
    object != null && Object.prototype.hasOwnProperty.call(object, key);
export const object = (value: unknown): value is CampaignRecord =>
    value !== null && typeof value === "object" && !Array.isArray(value);
export const id = (value: unknown): value is string =>
    typeof value === "string" &&
    /^[a-z][a-z0-9_-]{0,95}$/.test(value) &&
    !["constructor", "prototype", "__proto__"].includes(value);
export const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));

export function replaceFactionNames(text: string, replacements: Map<string, string>): string {
    if (typeof text !== "string" || !(replacements instanceof Map) || !replacements.size) return text;
    const names = Array.from(replacements.keys()).sort((a, b) => b.length - a.length);
    const pattern = new RegExp(
        "(^|[^\\p{L}\\p{N}_])(" +
            names.map((name) => name.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&")).join("|") +
            ")(?=$|[^\\p{L}\\p{N}_])",
        "gu",
    );
    return text.replace(pattern, (_, prefix, name) => prefix + replacements.get(name));
}

export function renameFactionText(
    definition: CampaignDefinition,
    renames: CampaignRecord[],
    factions: CampaignFaction[],
): CampaignDefinition {
    const replacements = new Map(
        (Array.isArray(renames) ? renames : [])
            .filter(
                (item) =>
                    object(item) &&
                    typeof item.from === "string" &&
                    item.from &&
                    typeof item.to === "string" &&
                    item.to &&
                    item.from !== item.to,
            )
            .map((item) => [item.from, item.to]),
    );
    const result = copy(definition);
    if (replacements.size && Array.isArray(factions)) {
        const storyNames = object(result.faction_story_names) ? result.faction_story_names : {};
        factions.forEach((faction) => {
            if (
                object(faction) &&
                replacements.has(faction.name) &&
                typeof faction.id === "string" &&
                !storyNames[faction.id]
            ) {
                storyNames[faction.id] = faction.name;
            }
        });
        if (Object.keys(storyNames).length) result.faction_story_names = storyNames;
    }
    return result;
}

export function replaceFactionStoryNames(
    text: string,
    definition: CampaignDefinition,
    roster: CampaignRecord,
): string {
    const sourceNames = object(definition && definition.faction_story_names)
        ? definition.faction_story_names
        : {};
    const factions = Array.isArray(roster && roster.factions) ? roster.factions : [];
    const replacements = new Map();
    Object.keys(sourceNames).forEach((id) => {
        const faction = factions.find((item: CampaignFaction) => item && item.id === id);
        const source = sourceNames[id];
        if (
            faction &&
            typeof source === "string" &&
            source &&
            typeof faction.name === "string" &&
            faction.name
        )
            replacements.set(source, faction.name);
    });
    return replaceFactionNames(text, replacements);
}

export function factionReferenceIds(definition: CampaignDefinition): Set<string> {
    const refs = new Set<string>();
    const add = (value: unknown) => {
        if (typeof value === "string" && value && value !== "player") refs.add(value);
    };
    if (!object(definition)) return refs;
    const settings = definition.settings || {};
    add(settings.campaign_support && settings.campaign_support.after_defeated);
    Object.values(object(definition.speakers) ? definition.speakers : {}).forEach((speaker) => {
        add(speaker && speaker.faction);
        add(speaker && speaker.base_expression && speaker.base_expression?.until?.target);
    });
    (Array.isArray(definition.steps) ? definition.steps : []).forEach((step) => {
        if (!object(step)) return;
        if (object(step.trigger)) {
            add(step.trigger.target);
            add(step.trigger.recipient);
            (Array.isArray(step.trigger.targets) ? step.trigger.targets : []).forEach(add);
        }
        if (object(step.campaign_attack_on_enter)) add(step.campaign_attack_on_enter.faction_id);
        if (object(step.campaign_halt_on_enter)) add(step.campaign_halt_on_enter.faction_id);
        if (object(step.campaign_accept_alliance_on_enter))
            add(step.campaign_accept_alliance_on_enter.faction_id);
        if (
            object(step.campaign_assault_on_enter) &&
            step.campaign_assault_on_enter.target !== "player_and_allies"
        )
            add(step.campaign_assault_on_enter.target);
        if (object(step.marker)) add(step.marker.target);
        if (object(step.camera)) {
            add(step.camera.from);
            add(step.camera.to);
        }
    });
    (Array.isArray(definition.reactions) ? definition.reactions : []).forEach((reaction) => {
        if (!object(reaction) || !object(reaction.when)) return;
        add(reaction.when.target);
        (Array.isArray(reaction.when.targets) ? reaction.when.targets : []).forEach(add);
    });
    return refs;
}

export function removeFactionReferences(
    definition: CampaignDefinition,
    removedFactions: CampaignFaction[],
): CampaignRecord {
    const removed = new Map(
        (Array.isArray(removedFactions) ? removedFactions : [])
            .filter((faction) => object(faction) && typeof faction.id === "string")
            .map((faction) => [faction.id, faction.name]),
    );
    const result = copy(definition);
    const adjusted = { steps_to_scenes: 0, reactions_removed: 0, speakers_unlinked: 0, settings_cleared: 0 };
    if (!removed.size || !object(result)) return { definition: result, adjusted };

    const settings = object(result.settings) ? result.settings : {};
    if (object(settings.campaign_support) && removed.has(settings.campaign_support.after_defeated)) {
        delete settings.campaign_support;
        adjusted.settings_cleared++;
    }
    Object.values(object(result.speakers) ? result.speakers : {}).forEach((speaker) => {
        if (speaker && speaker.base_expression && removed.has(speaker.base_expression?.until?.target)) {
            delete speaker.base_expression;
        }
        if (!object(speaker) || !removed.has(speaker.faction)) return;
        const factionId = speaker.faction;
        delete speaker.faction;
        if (
            !(typeof speaker.name === "string" && speaker.name.trim()) &&
            !(typeof speaker.name_key === "string" && speaker.name_key.trim())
        ) {
            speaker.name = removed.get(factionId) || factionId;
        }
        adjusted.speakers_unlinked++;
    });

    (Array.isArray(result.steps) ? result.steps : []).forEach((step) => {
        if (!object(step)) return;
        const trigger = object(step.trigger) ? step.trigger : null;
        let targetRemoved = false;
        if (trigger && Array.isArray(trigger.targets)) {
            const originalTargetCount = trigger.targets.length;
            const targets = trigger.targets.filter((target) => !removed.has(target));
            if (targets.length !== trigger.targets.length) trigger.targets = targets;
            if (targets.length) {
                if (trigger.type === "contact" && Number.isInteger(trigger.value))
                    trigger.value = Math.min(trigger.value, targets.length);
                if (trigger.type === "defeated") trigger.value = targets.length;
            } else if (originalTargetCount) targetRemoved = true;
        }
        if (trigger && removed.has(trigger.target)) {
            delete trigger.target;
            if (
                ["contact", "defeated", "eliminated", "camera_target"].includes(trigger.type) &&
                !Array.isArray(trigger.targets)
            )
                targetRemoved = true;
        }
        if (trigger && removed.has(trigger.recipient)) delete trigger.recipient;

        if (targetRemoved && ["objective", "guide"].includes(step.type)) {
            step.type = "scene";
            ["trigger", "guide", "marker", "hint_key", "camera_only"].forEach((field) => delete step[field]);
            adjusted.steps_to_scenes++;
            return;
        }
        if (object(step.marker) && removed.has(step.marker.target)) {
            const targets = trigger && (Array.isArray(trigger.targets) ? trigger.targets : [trigger.target]);
            const replacement =
                Array.isArray(targets) &&
                targets.find((target) => typeof target === "string" && !removed.has(target));
            if (replacement) step.marker.target = replacement;
            else delete step.marker;
        }
        if (object(step.campaign_assault_on_enter) && removed.has(step.campaign_assault_on_enter.target)) {
            delete step.campaign_assault_on_enter;
        }
        if (object(step.campaign_attack_on_enter) && removed.has(step.campaign_attack_on_enter.faction_id)) {
            delete step.campaign_attack_on_enter;
        }
        if (object(step.campaign_halt_on_enter) && removed.has(step.campaign_halt_on_enter.faction_id)) {
            delete step.campaign_halt_on_enter;
        }
        if (
            object(step.campaign_accept_alliance_on_enter) &&
            removed.has(step.campaign_accept_alliance_on_enter.faction_id)
        )
            delete step.campaign_accept_alliance_on_enter;
        if (object(step.camera) && removed.has(step.camera.to)) delete step.camera;
        if (object(step.camera) && removed.has(step.camera.from)) delete step.camera.from;
    });

    if (Array.isArray(result.reactions)) {
        result.reactions = result.reactions.filter((reaction) => {
            const when = object(reaction && reaction.when) ? reaction.when : null;
            if (!when) return true;
            if (removed.has(when.target)) {
                adjusted.reactions_removed++;
                return false;
            }
            if (Array.isArray(when.targets)) {
                when.targets = when.targets.filter((target: string) => !removed.has(target));
                if (!when.targets.length) {
                    adjusted.reactions_removed++;
                    return false;
                }
            }
            return true;
        });
    }
    if (object(result.faction_story_names)) {
        Object.keys(result.faction_story_names).forEach((id) => {
            if (removed.has(id)) delete result.faction_story_names[id];
        });
    }
    return { definition: result, adjusted };
}

// A speaker's authored mood ends on a confirmed, cumulative campaign fact.
export function campaignBaseExpressions(
    definition: CampaignDefinition,
    facts: CampaignRecord,
): Record<string, string> {
    const expressions: Record<string, string> = {};
    for (const speaker of Object.values(definition.speakers || {}) as CampaignRecord[]) {
        const rule = speaker.base_expression;
        if (!rule) continue;
        const allied = (facts.alliance_faction_ids || []).includes(rule.until.target);
        const expression = allied ? rule.after_expression : rule.expression;
        if (expression) expressions[speaker.faction || "player"] = expression;
    }
    return expressions;
}
