const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const shell = __dirname;
const coreSource = fs.readFileSync(path.join(shell, "main_menu.core.js"), "utf8");
const shellSource = fs.readFileSync(path.join(shell, "main_menu.shell.js"), "utf8");
const lobbiesSource = fs.readFileSync(path.join(shell, "main_menu.lobbies.js"), "utf8");
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const webCatalogEn = fs.readFileSync(path.join(shell, "../../sow-i18n/strings/en/web.toml"), "utf8");
const campaignEngine = fs.readFileSync(path.join(shell, "sow-campaign.js"), "utf8");
const campaignView = fs.readFileSync(path.join(shell, "sow-campaign-view.js"), "utf8");
const storyCss = fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
const campaignEditor = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/editor.js"), "utf8");
const campaignEditorServer = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/server.mjs"), "utf8");
const campaignEditorHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/logic.html"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");
const hudCss = fs.readFileSync(path.join(shell, "main_menu.hud.css"), "utf8");
const windowInput = fs.readFileSync(path.join(shell, "../../sow-client/src/input/window.rs"), "utf8");
const matchStartSource = fs.readFileSync(path.join(shell, "../../sow-client/src/loader/match_start.rs"), "utf8");
const campaignModule = fs.readFileSync(path.join(shell, "../../sow-client/src/campaign/mod.rs"), "utf8");
const campaignPolicySource = fs.readFileSync(path.join(shell, "../../sow-data/src/campaign.rs"), "utf8");
const simUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/update.rs"), "utf8");
const surfaceSource = fs.readFileSync(path.join(shell, "../../sow-client/src/input/surface.rs"), "utf8");
const cameraFrameUiSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/frame/ui.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const coreCost = fs.readFileSync(path.join(shell, "../../sow-core/src/building/cost.rs"), "utf8");
const buildingsIntent = fs.readFileSync(path.join(shell, "../../sow-core/src/intent/buildings.rs"), "utf8");
const mapClick = fs.readFileSync(path.join(shell, "../../sow-client/src/input/map_click.rs"), "utf8");

test("campaign runtime shares the JSON interpreter and cinematic view", () => {
    assert.match(tutorial, /SOWCampaign\.validate/);
    assert.match(tutorial, /SOWCampaign\.create/);
    assert.match(tutorial, /SOWCampaignView\.mount/);
    assert.match(tutorial, /onCinematicChange: function \(active\)[\s\S]*?SOW_menu_music_cinematic\(active\)/);
    assert.match(tutorial, /start_campaign_episode/);
    assert.match(tutorial, /set_tutorial_paused/);
    assert.match(tutorial, /complete_campaign_episode/);
    assert.match(webMenu, /WebMenuCommand::SetTutorialPaused \{\s*paused,\s*camera_only,\s*paused_action,\s*\} => \{[\s\S]*?self\.sim\.paused = paused[\s\S]*?tutorial_paused_action/);
    assert.match(simUpdateSource, /if self\.sim\.paused \{\s*self\.sim\.offline_tick_timer = 0\.0;/);
    assert.match(campaignEngine, /wilderness_orders_accepted/);
    assert.match(campaignEngine, /const paused = \["scene", "choice", "end"\]\.includes\(step\.type\) \|\| \(step\.pause_game === true && !expansionStarted\)/);
    assert.match(campaignView, /data-story-choice/);
    assert.match(storyCss, /\.sow-story__choices/);
    assert.doesNotMatch(hudCss, /\.sow-hud__tutorial-overlay/);
});


test("Boudica keeps open neutral clans, makes every Roman enemy, and orders the recorded pre-final targets", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const roster = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    const candidateNames = ["Catuvellauni", "Cantiaci", "Durovernum", "Parisi", "Corieltauvi", "Cornovii", "Dobunni", "Durotriges", "Dumnonii", "Silures", "Demetae", "Ordovices", "Deceangli", "Caledonii", "Votadini", "Belgae", "Gaulish Traders 1", "Gaulish Traders 2", "Gaulish Traders 3", "Irish Raiders", "Picts"];
    const byName = new Map(roster.factions.map(faction => [faction.name, faction]));
    for (const name of candidateNames) assert.equal(byName.get(name).relation, "neutral", name + " remains neutral");
    const romans = roster.factions.filter(faction => faction.civ.replace(/[^a-z0-9]/gi, "").toLowerCase() === "romanempire");
    assert.equal(romans.length, 13);
    assert.ok(romans.every(faction => faction.relation === "enemy" && faction.team === "Red" && faction.can_request_alliance === false));
    for (const id of ["lindum", "viroconium", "isca_dumnoniorum", "brigantes_1", "atrebates", "regnenses", "calleva", "noviomagus"]) {
        const faction = roster.factions.find(entry => entry.id === id);
        assert.ok(faction && faction.relation === "enemy" && faction.team === "Red", id + " is hostile to Boudica");
    }
    assert.equal(roster.factions.some(faction => faction.id === "deva"), false);
    const targets = ["camulodunum", "legio_ix_hispana", "londinium", "verulamium"];
    const stepsById = new Map(definition.steps.map(step => [step.id, step]));
    const recordedTargets = [];
    let current = stepsById.get("boudica_camulodunum");
    assert.equal(current.next, "boudica_select_city_upgrade", "defeating Camulodunum opens the paused City upgrade sequence");
    assert.equal(stepsById.get("boudica_ninth_legion").next, "boudica_londinium_intro",
        "defeating the Ninth continues to Londinium, not the City upgrade");
    while (current && current.id !== "boudica_pact_choice") {
        if (current.trigger && current.trigger.type === "defeated" && targets.includes(current.trigger.target)) recordedTargets.push(current.trigger.target);
        current = stepsById.get(current.next);
    }
    assert.deepEqual(recordedTargets, targets, "follow the authored mission links before the pact and final battle");
    const postumus = definition.steps.find(step => step.id === "boudica_postumus_notice");
    assert.deepEqual(postumus.marker, { target: "legio_ii_augusta" });
    assert.deepEqual(postumus.campaign_attack_on_enter, { faction_id: "legio_ii_augusta", can_attack_player: true });
    assert.equal(definition.steps.find(step => step.id === "boudica_final_choice").choices.find(choice => choice.id === "fight").next, postumus.id);
    const neutralTerms = definition.reactions.find(reaction => reaction.id === "neutral_contact_terms");
    assert.equal(neutralTerms.choices.find(choice => choice.id === "accept").gold_cost, 200);
    const invalidRomanNeutral = structuredClone(roster);
    invalidRomanNeutral.factions.find(faction => faction.id === "legio_xiv_gemina").relation = "neutral";
    assert.ok(campaign.validate(definition, invalidRomanNeutral, { hasText: () => true, hasAvatar: () => true }).errors.some(issue => issue.field === "roster.factions.relation"));
});


test("campaign attack permission changes accept existing enemies only", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = { version: 2, episode_id: "attack_permission", entry: "notice", settings: { starting_troops: 1000 }, speakers: {}, steps: [
        { id: "notice", type: "scene", title_key: "tutorial.notice", campaign_attack_on_enter: { faction_id: "enemy", can_attack_player: true }, next: "end" },
        { id: "end", type: "end", title_key: "tutorial.end" }
    ] };
    const roster = { map: "eastanglia", player_spawn: [1, 1], factions: [
        { id: "enemy", name: "Enemy", x: 1, y: 1, starting_troops: 1, relation: "enemy", civ: "Roman Empire", leader: "Caesar" },
        { id: "clan", name: "Clan", x: 2, y: 2, starting_troops: 1, relation: "neutral", civ: "Gallic Tribes", leader: "Caesar" }
    ] };
    assert.deepEqual(campaign.validate(definition, roster, { hasText: () => true }).errors, []);
    definition.steps[0].campaign_attack_on_enter.faction_id = "clan";
    assert.ok(campaign.validate(definition, roster, { hasText: () => true }).errors.some(issue => issue.field === "campaign_attack_on_enter.faction_id"));
    assert.match(campaignEditor, /Change an enemy's attack behavior when this step begins/);
    assert.match(tutorial, /send\("set_campaign_attack_permission", machineView\.step\.campaign_attack_on_enter\)/);
});


test("Boudica final decision opens at 5,000 tiles and targets her on the map", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    const stand = definition.steps.find(step => step.id === "boudica_last_stand");
    const choice = definition.steps.find(step => step.id === "boudica_final_choice");
    assert.equal(stand.trigger.value, 5000);
    assert.deepEqual(choice.marker, { target: "player" });
    const machine = campaign.create(definition, stand.id, { factions: [] });
    assert.equal(machine.update({ tiles: 5001 }, {}).step.id, stand.id);
    assert.equal(machine.update({ tiles: 5000 }, {}).step.id, choice.id);
    const editorPreviewFocus = campaignEditor.slice(campaignEditor.indexOf("function applyPreviewFocus"), campaignEditor.indexOf("function renderPreview", campaignEditor.indexOf("function applyPreviewFocus")));
    assert.match(editorPreviewFocus, /step\.marker && step\.marker\.target/);
    assert.match(campaignEditor, /var keepsMarker = \["scene", "choice", "objective", "guide"\]/);
    assert.match(campaignEditor, /if \(\["scene", "choice", "objective", "guide"\]\.includes\(step\.type\)\) \{\s*var markerPanel/);
});


test("Boudica reveal beats share a fixed 1.1-second wait while tribal contact stays immediate", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    const waitSteps = definition.steps.filter(step => step.wait_before === true).map(step => step.id);
    assert.equal(campaign.WAIT_MS, 1100);
    assert.deepEqual(waitSteps, [
        "boudica_first_victory_scene", "boudica_city_intro", "boudica_aid_choice",
        "boudica_city_ready", "boudica_city_upgrade_ready", "boudica_outpost_colonia_veterans_scene",
        "boudica_outpost_tax_collectors_scene", "boudica_outpost_roman_supply_depot_scene",
        "boudica_ninth_strike"
    ]);
    const next = id => definition.steps.find(step => step.id === id).next;
    assert.equal(next("boudica_share_sent"), "boudica_camulodunum_intro");
    assert.equal(next("boudica_aid_kept"), "boudica_camulodunum_intro");
    assert.equal(next("boudica_outpost_colonia_veterans_scene"), "boudica_outpost_tax_collectors");
    assert.equal(definition.steps.some(step => /factory|bunker/.test(step.id)), false);
    assert.equal(definition.steps.some(step => Object.hasOwn(step, "start_delay_seconds") || Object.hasOwn(step, "advance_delay_seconds")), false);
    assert.doesNotMatch(campaignEditor, /start_delay_seconds|advance_delay_seconds/);
    assert.match(campaignEditor, /Wait 1\.1 seconds before showing/);
    assert.match(campaignEditor, /var canTick = Boolean\(\(trigger && trigger\.type === "elapsed"\) \|\| timedRoute\)/);
    assert.match(tutorial, /stepRevealToken !== waitingToken[\s\S]*waitingMachine\.view\(\)\.step\.id === waitingStepId/);
    assert.match(campaignEditor, /previewRevealToken !== waitingToken[\s\S]*waitingMachine\.view\(\)\.step\.id === waitingStepId/);
    assert.match(tutorial, /function clearStepRevealTimer\(\)[\s\S]*stepRevealToken\+\+[\s\S]*window\.clearTimeout/);
    assert.match(campaignEditor, /function clearPreviewRevealTimer\(\)[\s\S]*previewRevealToken\+\+[\s\S]*window\.clearTimeout/);
    assert.match(campaignView, /\}, host\.SOWCampaign\.WAIT_MS\);/);
    assert.doesNotMatch(tutorial, /window\.setTimeout\(updateMenuGuide/);
    for (const [type, legacyField] of [["scene", "start_delay_seconds"], ["objective", "advance_delay_seconds"]]) {
        const legacyStep = { id: "wait", type, title_key: "tutorial.wait", next: "end", [legacyField]: 1 };
        if (type === "objective") legacyStep.trigger = { type: "elapsed", scope: "step", value: 1 };
        const invalid = campaign.validate({
            version: 2, episode_id: "legacy_wait", settings: { starting_troops: 1000 },
            speakers: {}, entry: "wait", steps: [legacyStep, { id: "end", type: "end", title_key: "tutorial.end" }]
        }, null, { hasText: () => true });
        assert.ok(invalid.errors.some(issue => issue.step === "wait" && issue.field === "fields"), `${legacyField} remains rejected`);
    }
    const immediate = campaign.create({ entry: "scene", steps: [
        { id: "scene", type: "scene", title_key: "scene", next: "end" },
        { id: "end", type: "end", title_key: "end" }
    ] }, "scene", null, () => 0);
    assert.equal(immediate.view().waiting, false, "unflagged dialogue has no artificial wait");
    assert.equal(immediate.advance(null, "scene"), true, "unflagged dialogue continues normally");

    let choiceClock = 0;
    const decision = campaign.create({ entry: "choice", steps: [
        { id: "choice", type: "choice", wait_before: true, choices: [{ id: "yes", next: "scene" }] },
        { id: "scene", type: "scene", title_key: "scene", next: "end" },
        { id: "end", type: "end", title_key: "end" }
    ] }, "choice", null, () => choiceClock);
    assert.equal(decision.advance("yes", "choice"), false, "a hidden choice cannot be selected");
    choiceClock = 1100;
    assert.equal(decision.view().waiting, false);
    assert.equal(decision.advance("yes", "choice"), true, "the visible choice responds immediately");
    assert.equal(decision.view().step.id, "scene");
});


test("campaign building and action permissions are cumulative, route-safe, and reject legacy global switches", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const rustCampaign = fs.readFileSync(path.join(shell, "../../sow-core/src/campaign.rs"), "utf8");
    const rustGame = fs.readFileSync(path.join(shell, "../../sow-core/src/game.rs"), "utf8");
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const byId = new Map(definition.steps.map(step => [step.id, step]));

    assert.deepEqual(campaign.campaignUnlocksAtStep(byId, definition.entry, "boudica_choose_city"), {
        buildings: { City: 1 }, actions: []
    });
    assert.deepEqual(campaign.campaignUnlocksAtStep(byId, definition.entry, "boudica_select_city_upgrade"), {
        buildings: { City: 2 }, actions: []
    });
    assert.deepEqual(campaign.validate(definition, roster).errors, []);
    assert.equal(definition.steps.some(step => /factory|bunker/i.test(step.id)), false);
    assert.ok(definition.steps.every(step => !step.unlocks ||
        Object.keys(step.unlocks.buildings || {}).every(kind => kind === "City") &&
        (!step.unlocks.actions || step.unlocks.actions.length === 0)));

    assert.deepEqual(campaign.CAMPAIGN_ACTION_BUILDING_REQUIREMENTS, {
        transport_fleet: null,
        trade_ship: { kind: "Port", level: 1 },
        warship: { kind: "Port", level: 2 },
        nuke: { kind: "City", level: 4 }
    });
    const buildingEnum = rustGame.match(/pub enum BuildingKind \{([\s\S]*?)\n\}/)[1];
    const rustBuildings = Array.from(buildingEnum.matchAll(/^\s*([A-Z][A-Za-z0-9]*),?\s*$/gm), match => match[1]).sort();
    assert.deepEqual(rustBuildings, Object.keys(campaign.STRUCTURE_LEVEL_LIMITS).sort(), "Rust buildings, campaign validation and editor controls must stay in sync");
    const actionEnum = rustCampaign.match(/pub enum CampaignAction \{([\s\S]*?)\n\}/)[1];
    const rustActions = Array.from(actionEnum.matchAll(/^\s*([A-Z][A-Za-z0-9]*),?\s*$/gm), match => match[1]
        .replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase()).sort();
    assert.deepEqual(rustActions, campaign.CAMPAIGN_ACTIONS.slice().sort(), "Rust action permissions and editor controls must stay in sync");
    assert.match(rustCampaign, /Self::TransportFleet => None/);
    assert.match(rustCampaign, /Self::TradeShip => Some\(\(BuildingKind::Port, 1\)\)/);
    assert.match(rustCampaign, /Self::Warship => Some\(\(BuildingKind::Port, 2\)\)/);
    assert.match(rustCampaign, /Self::Nuke => Some\(\(BuildingKind::City, BuildingKind::City\.max_level\(\)\)\)/);
    const cityTwo = { buildings: { City: 2 }, actions: ["transport_fleet", "trade_ship", "warship", "nuke"] };
    assert.equal(campaign.campaignAllowsAction(cityTwo, "transport_fleet"), true);
    assert.equal(campaign.campaignAllowsAction(cityTwo, "trade_ship"), false);
    assert.equal(campaign.campaignAllowsAction(cityTwo, "warship"), false);
    assert.equal(campaign.campaignAllowsAction(cityTwo, "nuke"), false);
    assert.equal(campaign.campaignAllowsAction({ buildings: { Port: 2, City: 4 }, actions: cityTwo.actions }, "nuke"), true);

    const withoutCityGrant = structuredClone(definition);
    delete withoutCityGrant.steps.find(step => step.id === "boudica_choose_city").unlocks;
    assert.ok(campaign.validate(withoutCityGrant, roster).errors.some(issue =>
        issue.step === "boudica_build_city" && issue.field === "unlocks" && issue.message.includes("City level 1")));
    const withoutUpgradeGrant = structuredClone(definition);
    delete withoutUpgradeGrant.steps.find(step => step.id === "boudica_select_city_upgrade").unlocks;
    assert.ok(campaign.validate(withoutUpgradeGrant, roster).errors.some(issue =>
        issue.step === "boudica_structure_upgrade" && issue.field === "unlocks" && issue.message.includes("City level 2")));

    const legacySettings = structuredClone(definition);
    legacySettings.settings.buildings_enabled = true;
    legacySettings.settings.buildings_unlock_after_defeated = "rome";
    assert.ok(campaign.validate(legacySettings, roster).errors.some(issue =>
        issue.field === "settings" && issue.message === "Unknown match setting."));

    const branch = {
        version: 2, episode_id: "unlock_branch", settings: { starting_troops: 1000 }, entry: "choice",
        steps: [
            { id: "choice", type: "choice", title_key: "choice", choices: [
                { id: "grant", label_key: "grant", next: "grant_city" },
                { id: "skip", label_key: "skip", next: "skip_city" }
            ] },
            { id: "grant_city", type: "scene", title_key: "grant_city", unlocks: { buildings: { City: 1 } }, next: "shared" },
            { id: "skip_city", type: "scene", title_key: "skip_city", next: "shared" },
            { id: "shared", type: "objective", title_key: "shared", trigger: { type: "structure_level", kind: "City", value: 1 }, next: "end" },
            { id: "end", type: "end", title_key: "end" }
        ]
    };
    const branchById = new Map(branch.steps.map(step => [step.id, step]));
    assert.deepEqual(campaign.campaignUnlocksAtStep(branchById, branch.entry, "shared"), { buildings: {}, actions: [] });
    assert.ok(campaign.validate(branch, null, { hasText: () => true }).errors.some(issue =>
        issue.step === "shared" && issue.field === "unlocks" && issue.message.includes("City level 1")));
});


test("Boudica pauses for action steps and resumes only after valid expansion or construction", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const step = id => definition.steps.find(candidate => candidate.id === id);

    const expansion = campaign.create(definition, "boudica_first_expansion", roster);
    assert.equal(expansion.update({ tiles_gained: 0, wilderness_orders_accepted: 0 }, {}).paused, true);
    assert.equal(expansion.view().paused_action, "expand_once_then_resume");
    assert.equal(expansion.update({ tiles_gained: 0, wilderness_orders_accepted: 0 }, {}).paused, true, "a rejected click leaves the game paused");
    const acceptedExpansion = expansion.update({ tiles_gained: 0, wilderness_orders_accepted: 1 }, {});
    assert.equal(acceptedExpansion.paused, false, "accepted neutral attack resumes before capture");
    assert.equal(acceptedExpansion.step.id, "boudica_first_expansion", "the 250-tile goal still waits for actual territory gain");
    assert.equal(expansion.update({ tiles_gained: 250, wilderness_orders_accepted: 1 }, {}).step.id, "boudica_camera_intro");
    assert.equal(expansion.view().paused, true);

    for (const id of ["boudica_first_contact", "boudica_third_contact"]) {
        assert.equal(step(id).attack_ratio_on_enter, undefined, "contact missions preserve the player's troop allocation");
        assert.equal(step(id).pause_game, true);
        assert.equal(step(id).paused_action, "expand_once_then_resume");
        assert.equal(step(id).guide.target, "expand");
    }
    assert.equal(step("boudica_second_contact").attack_ratio_on_enter, undefined);
    assert.equal(step("boudica_second_contact").pause_game, undefined, "the second Iceni meeting does not pause after Stonea's dialogue");
    assert.equal(step("boudica_second_contact").paused_action, undefined);
    assert.equal(step("boudica_second_contact").guide.target, "expand");
    assert.equal(definition.settings.starting_gold, 180, "Boudica starts with 30 additional gold for the first paid alliance");
    assert.deepEqual(campaign.validate(definition, roster).errors, [], "the Boudica gold setting is accepted by the shared campaign validator");
    const invalidGold = structuredClone(definition);
    invalidGold.settings.starting_gold = -1;
    assert.ok(campaign.validate(invalidGold, roster).errors.some(issue => issue.field === "settings.starting_gold"));
    const secondContact = campaign.create(definition, "boudica_second_contact", roster);
    assert.equal(secondContact.update({ contact_faction_ids: [] }, {}).paused, false, "the Snettisham objective runs without pausing");
    const contact = campaign.create(definition, "boudica_first_contact", roster);
    const contactFacts = { tiles_gained: 250, wilderness_orders_accepted: 0, contact_faction_ids: [] };
    assert.equal(contact.update(contactFacts, {}).paused, true);
    assert.equal(contact.update({ ...contactFacts, wilderness_orders_accepted: 1 }, {}).paused, false);
    assert.equal(contact.update({ ...contactFacts, tiles_gained: 251, wilderness_orders_accepted: 1 }, {}).paused, false);
    assert.equal(contact.update({ ...contactFacts, tiles_gained: 251, wilderness_orders_accepted: 1, contact_faction_ids: ["stonea"] }, {}).paused, true);

    assert.equal(step("boudica_transfer_target").pause_game, true);
    assert.equal(step("boudica_transfer_send").pause_game, true);
    const transfer = campaign.create(definition, "boudica_transfer_send", roster);
    assert.equal(transfer.update({ resource_transfers_by_recipient_faction_id: { trinovantes: { total: 0 } } }, {}).paused, true);
    assert.equal(transfer.view().paused_action, "send_resources_stay_paused");
    const sent = transfer.update({ resource_transfers_by_recipient_faction_id: { trinovantes: { total: 1 } } }, {});
    assert.equal(sent.step.id, "boudica_share_sent");
    assert.equal(sent.paused, true);

    for (const [id, metric, waitId] of [
        ["boudica_build_city", "cities", "boudica_city_construction_wait"]
    ]) {
        assert.equal(step(id).pause_game, true);
        assert.equal(step(id).paused_action, "build_until_started");
        const machine = campaign.create(definition, id, roster);
        assert.equal(machine.update({ [metric]: 0 }, {}).paused, true);
        const rejected = machine.update({ [metric]: 0 }, {});
        assert.equal(rejected.paused, true, "a rejected placement leaves the paused action available for another attempt");
        assert.equal(rejected.step.id, id);
        const started = machine.update({ [metric]: 1 }, {});
        assert.equal(started.step.id, waitId);
        assert.equal(started.paused, false);
        assert.equal(step(waitId).pause_game, undefined);
    }

    assert.equal(step("boudica_select_city_upgrade").pause_game, true);
    const citySelection = campaign.create(definition, "boudica_select_city_upgrade", roster);
    assert.equal(citySelection.update({ selected_building_kind: "" }, {}).paused, true,
        "the game stays paused while the player has not selected the City");
    const citySelected = citySelection.update({ selected_building_kind: "City" }, {});
    assert.equal(citySelected.step.id, "boudica_structure_upgrade");
    assert.equal(citySelected.paused, true, "selection flows directly into the still-paused upgrade action");

    assert.equal(step("boudica_structure_upgrade").trigger.type, "structure_upgrade");
    assert.equal(step("boudica_structure_upgrade").trigger.kind, "City");
    assert.equal(step("boudica_structure_upgrade").paused_action, "upgrade_until_started");
    const upgradeButton = campaign.create(definition, "boudica_structure_upgrade", roster);
    assert.equal(upgradeButton.update({ structure_upgrades: 0 }, {}).paused, true);
    const rejectedUpgrade = upgradeButton.update({ structure_upgrades: 0 }, {});
    assert.equal(rejectedUpgrade.paused, true, "a rejected upgrade leaves the paused action available for another attempt");
    assert.equal(rejectedUpgrade.step.id, "boudica_structure_upgrade");
    const upgrading = upgradeButton.update({ structure_upgrades: 1 }, {});
    assert.equal(upgrading.step.id, "boudica_city_upgrade_wait");
    assert.equal(upgrading.paused, false, "the upgrade's construction wait runs after upgrade acceptance");
    assert.equal(step("boudica_city_upgrade_wait").pause_game, undefined);
    assert.equal(step("boudica_attack_roman_outpost").pause_game, true);
    assert.equal(step("boudica_rise_of_the_iceni_intro").attack_ratio_on_enter, 0.25);
    assert.equal(step("boudica_attack_roman_outpost").attack_ratio_on_enter, undefined);
    for (const id of ["boudica_outpost_colonia_veterans", "boudica_outpost_tax_collectors", "boudica_outpost_roman_supply_depot", "boudica_camulodunum", "boudica_londinium", "boudica_verulamium", "boudica_ninth_legion"]) {
        assert.equal(step(id).pause_game, undefined, `${id} stays available for border expansion`);
    }
});


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
    assert.match(webMenu, /app\.input\.camera_zoom\.is_finite\(\)\s*&& app\.input\.camera_zoom >= zoom_in_target/);
    assert.match(webMenu, /TUTORIAL_MAX_ZOOM\.clamp\(zoom_floor, zoom_ceiling\)/);
    assert.match(surfaceSource, /self\.record_tutorial_zoom\(self\.input\.camera_zoom - old_zoom\)/);
    assert.doesNotMatch(webMenu, /tutorial_zoom_out_completed/);
    assert.doesNotMatch(webMenu, /camera_zoom_tolerance|zoom_in_tolerance/);
    assert.match(webMenu, /tutorial_camera_zoom_hundredths/);
    assert.match(webMenu, /tutorial_target_zoom_hundredths/);
    assert.match(webMenu, /camera_zoom_ceiling/);
    assert.match(campaignView, /sow-story__zoom-current/);
    assert.match(campaignView, /sow-story__zoom-target/);
    assert.match(campaignView, /sow-story__zoom-current[^>]*><\/b><i aria-hidden="true">→<\/i><b class="sow-story__zoom-target/);
    assert.match(campaignView, /guideCurrent\.animate\(/);
    assert.match(campaignView, /const zoomButtonGuide = step\.guide && step\.guide\.kind === "ui"/);
    assert.doesNotMatch(tutorial, /sow:tutorial-ui-action|hud_zoom_in|hud_zoom_out/);
    assert.match(hud, /setInterval\(function \(\)[\s\S]*?zoomAtLimit\(command\)[\s\S]*?send\(command\)/);
    assert.doesNotMatch(hud, /sow:tutorial-ui-action/);
    assert.match(hud, /camera_zoom_state/);
    assert.match(hud, /zoom\.current/);
    assert.match(hud, /zoom\.floor/);
    assert.match(hud, /zoom\.ceiling/);
    assert.match(webMenu, /camera_zoom_hundredths/);
    assert.match(webMenu, /tutorial_marker_player_id: if tutorial_active/);
    assert.match(webMenu, /tutorial_target_contains\(\s*marker_player_id/);
    assert.match(webMenu, /"tutorial_target_hovered": tutorial_target_hovered/);
    assert.match(tutorial, /tutorial\.facts && tutorial\.facts\.tutorial_target_hovered/);
    assert.match(tutorial, /var hoverDetected = tutorialTargetHovered/);
    assert.doesNotMatch(campaignView + storyCss, /sow-story__guide-shade|guide-cutout|mask-image:\s*radial-gradient/);
    assert.match(storyCss, /\.sow-story__spotlight\s*\{[^}]*border: 1px solid var\(--story-accent\)/);
    assert.match(campaignView, /if \(reducedMotion\)/);
    assert.match(tutorial, /direction: step\.guide\.gesture === "zoom_in" \? "min" : "max"/);
    assert.match(campaignView, /const anchor = context\.anchor;/);
    assert.match(campaignView, /metric\.direction === "min" \? "≥" : metric\.direction === "max" \? "≤"/);
    assert.match(campaignEditor, /x: frame\.clientWidth \* 0\.5, y: frame\.clientHeight \* 0\.5/);
    assert.match(windowInput, /1\.0 \+ \(\(distance - last_distance\) as f32 \* 0\.005\)/);
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
    assert.match(tutorial, /machineView\.waiting[\s\S]*runtime\.view\.render\(null\)[\s\S]*window\.setTimeout/);
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


test("renaming a campaign faction updates catalog copy through stable roster IDs", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        settings: { campaign_support: { after_defeated: "roman_outpost", share_percent: 50 } },
        speakers: { roman: { faction: "roman_outpost" } },
        steps: [{ trigger: { target: "roman_outpost", targets: ["roman_outpost", "other"] }, marker: { target: "roman_outpost" } }],
        reactions: [{ when: { type: "contact", target: "roman_outpost" } }]
    };
    const renamed = campaign.renameFactionText(definition, [{ from: "Old Name", to: "Roman Outpost" }], [{ id: "roman_outpost", name: "Old Name" }]);
    assert.equal(renamed.settings.campaign_support.after_defeated, "roman_outpost");
    assert.equal(renamed.speakers.roman.faction, "roman_outpost");
    assert.deepEqual(renamed.steps[0].trigger, { target: "roman_outpost", targets: ["roman_outpost", "other"] });
    assert.equal(renamed.steps[0].marker.target, "roman_outpost");
    assert.equal(renamed.reactions[0].when.target, "roman_outpost");
    assert.deepEqual(renamed.faction_story_names, { roman_outpost: "Old Name" });
    assert.equal(campaign.replaceFactionStoryNames("Attack Old Name!", renamed, { factions: [{ id: "roman_outpost", name: "Roman Outpost" }] }), "Attack Roman Outpost!");
    assert.equal(campaign.replaceFactionStoryNames("Old Namesake", renamed, { factions: [{ id: "roman_outpost", name: "Roman Outpost" }] }), "Old Namesake", "similar faction names stay unchanged");
    assert.equal(definition.steps[0].trigger.target, "roman_outpost", "renaming preserves stable references");
    assert.deepEqual([...campaign.factionReferenceIds(renamed)].sort(), ["other", "roman_outpost"]);
});


test("disabled neutral offers stay hidden while named story dialogues and legacy rosters work", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const roster = { factions: [
        { id: "story_faction", relation: "allied", can_request_alliance: false },
        { id: "neutral_faction", relation: "neutral", can_request_alliance: false }
    ] };
    const waiting = { id: "waiting", type: "objective", trigger: { type: "elapsed", value: 30 } };
    const storyOffer = { id: "named_story", when: { type: "contact", target: "story_faction" } };
    const neutralOffer = { id: "neutral_contact_terms", when: { type: "contact", relation: "neutral" } };
    const storyFacts = { contact_faction_ids: ["story_faction"] };

    const story = campaign.create({ entry: "waiting", steps: [waiting], reactions: [storyOffer] }, undefined, roster, () => 0);
    const immediateContact = story.update(storyFacts, {});
    assert.equal(immediateContact.reaction, "named_story", "an exact story response ignores the generic-offer flag");
    assert.equal(immediateContact.step.type, "scene", "contact opens its response on the first campaign update");
    assert.equal(immediateContact.paused, true);

    const genericDefinition = { entry: "waiting", steps: [waiting], reactions: [neutralOffer] };
    const neutralFacts = { contact_faction_ids: ["neutral_faction"] };
    const disabledRoster = JSON.parse(JSON.stringify(roster));
    disabledRoster.factions.find(faction => faction.id === "neutral_faction").can_request_alliance = false;
    const disabledGeneric = campaign.create(genericDefinition, undefined, disabledRoster);
    assert.equal(disabledGeneric.update(neutralFacts, {}).reaction, undefined, "false hides a generic neutral offer");

    const legacyRoster = JSON.parse(JSON.stringify(disabledRoster));
    delete legacyRoster.factions.find(faction => faction.id === "neutral_faction").can_request_alliance;
    const legacyGeneric = campaign.create(genericDefinition, undefined, legacyRoster);
    assert.equal(legacyGeneric.update(neutralFacts, {}).reaction, "neutral_contact_terms", "a missing legacy field keeps the old offer behavior");
});


test("defeat scenes wait 1.1 seconds when attack and defeat arrive in the same turn", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    let clock = 0;
    const definition = { entry: "attack", steps: [
        { id: "attack", type: "objective", trigger: { type: "attack", target: "outpost", value: 1, scope: "total" }, next: "victory" },
        { id: "victory", type: "objective", trigger: { type: "defeated", target: "outpost", value: 1, scope: "total" }, next: "victory_scene" },
        { id: "victory_scene", type: "scene", title_key: "tutorial.victory", wait_before: true, next: "end" },
        { id: "end", type: "end", title_key: "tutorial.end" }
    ] };
    const machine = campaign.create(definition, "attack", { factions: [] }, () => clock);
    const beforeDefeat = {
        elapsed_seconds: 3,
        attacks_by_faction_id: {},
        defeated_faction_ids: [],
        eliminated_faction_ids: []
    };
    machine.update(beforeDefeat, {});

    const attackAndDefeat = {
        elapsed_seconds: 4,
        attacks_by_faction_id: { outpost: 1 },
        defeated_faction_ids: ["outpost"],
        eliminated_faction_ids: ["outpost"]
    };
    const revealed = machine.update(attackAndDefeat, {});
    assert.equal(revealed.step.id, "victory_scene", "objective completion transitions immediately to the 1.1-second scene wait");
    assert.equal(revealed.waiting, true);
    assert.equal(machine.advance(null, "victory_scene"), false, "the hidden scene cannot be skipped");
    clock = 1099;
    assert.equal(machine.view().waiting, true);
    clock = 1100;
    assert.equal(machine.view().waiting, false);
    assert.equal(machine.advance(null, "victory_scene"), true);
    assert.equal(machine.view().step.id, "end");
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
    assert.ok(campaignEditor.includes("Local cinematic video"));
    assert.ok(campaignEditor.includes('"/assets/campaign/" + state.definition.episode_id'));
    assert.ok(campaignView.includes('<video playsinline preload="none" fetchpriority="low"></video>'), "the cinematic never preloads over the loader");
    assert.match(campaignView, /attachCinematicMedia/, "the cinematic uses derived WebM+MP4 sources");
    assert.match(campaignView, /\.poster\.webp/, "the cinematic ships a poster");
    assert.doesNotMatch(campaignView, /<video[^>]*\bmuted\b/i, "cinematic audio starts enabled by default");
    assert.ok(campaignView.indexOf('<div class="sow-story__cinematic"') < campaignView.indexOf('<article class="sow-story__dialog"'), "cinematic is a root overlay, not dialog content");
    assert.match(storyCss, /\.sow-story\.is-cinematic \.sow-story__cinematic \{[^}]*inset: 0/);
    assert.doesNotMatch(storyCss, /\.sow-story\.is-cinematic \.sow-story__dialog/);
    assert.ok(campaignView.includes("data-story-start"));
    assert.ok(campaignView.includes('class="sow-story__continue sow-story__cinematic-start"'), "the editor Play action reuses the tutorial button system");
    assert.match(campaignView, /cinematicStart\.textContent = t\("lobbies\.play"\)/);
    assert.doesNotMatch(campaignView, /cinematicStatus = "blocked"/);
    assert.doesNotMatch(shellSource, /menuMusicShowUnlockPrompt|data-sow-music-unlock|sow-menu__music-unlock/);
    assert.ok(campaignView.includes("data-story-hold"));
    assert.doesNotMatch(campaignView, /<video[^>]*\bcontrols\b/i);
    assert.ok(campaignView.includes("function failCinematic()"));
    assert.ok(campaignView.includes("cinematicVideo.play()"));
    assert.match(campaignView, /setTimeout\(\(\) => \{[\s\S]*?\}, 1250\)/);
    assert.doesNotMatch(campaignView, /<video[^>]*autoplay/i);
    assert.match(campaignEditor, /autoPlayCinematic: false/);
    assert.match(lobbiesSource, /definition\.steps[\s\S]*?definition\.entry[\s\S]*?video_src/);
    assert.match(lobbiesSource, /data-command='watch_campaign_cinematic'/);
    assert.match(shellSource, /command === "watch_campaign_cinematic"[\s\S]*?SOW_watchCampaignCinematic/);
    assert.match(tutorial, /window\.SOW_watchCampaignCinematic = watchCampaignCinematic/);
    assert.ok(campaignEditorServer.includes('".mp4": "video/mp4"'));
    assert.ok(campaignEditorServer.includes('".webm": "video/webm"'));
    assert.ok(campaignEditorServer.includes('"Accept-Ranges": "bytes"'));
    assert.ok(campaignEditorServer.includes("createReadStream(file, { start, end })"));

});


test("shared cinematic retries muted autoplay and resumes on trusted clicks without prompts", async () => {
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
    const view = sandbox.SOWCampaignView.mount(root, {
        asset: value => "/" + value,
        translate: key => key,
        roster: { factions: [] },
        onContinue() { continued++; },
        onDismiss() { continued++; },
        onCinematicChange(active) { cinematicChanges.push(active); }
    });
    const video = root.querySelector(".sow-story__cinematic").querySelector("video");
    video.play = () => {
        playCalls++; video.paused = false;
        return playCalls === 1
            ? Promise.reject(Object.assign(new Error("gesture required"), { name: "NotAllowedError" }))
            : Promise.resolve();
    };
    const step = { id: "opening", type: "scene", presentation: "cinematic", video_src: "/assets/campaign/cinematic_test/opening.webm", title_key: "opening_title", body_key: "opening_body" };
    const definition = { episode_id: "cinematic_test", speakers: {}, steps: [step] };
    view.render({ definition, step, line: step, paused: true, done: false, choices: [], progress: { target: 0, current: 0 }, state: { line: 0 } }, {});
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

    cinematic.dispatch("pointerdown", { pointerId: 2, pointerType: "touch", isPrimary: true, button: 0 });
    const progressTimer = [...timers.entries()].find(([, timer]) => timer.delay === 16);
    if (progressTimer) { timers.delete(progressTimer[0]); progressTimer[1].callback(); }
    assert.equal(root.querySelector("[data-story-hold]").querySelector("b").style.transform, "scaleX(1)");
    const holdTimer = [...timers.entries()].find(([, timer]) => timer.delay === 1250);
    assert.ok(holdTimer, "mobile hold uses the same 1.25-second threshold");
    timers.delete(holdTimer[0]); holdTimer[1].callback();
    assert.equal(continued, 1, "a completed touch hold continues the scene");

    root.dispatch("keydown", { key: "Escape", repeat: false });
    assert.equal(continued, 2, "Escape continues immediately without waiting for the hold timer");
    video.dispatch("ended");
    assert.deepEqual(cinematicChanges, [true, false], "music is released when the video ends");
    assert.equal(cinematic.hidden, true, "the video overlay closes after playback ends");
    assert.equal(dialog.hidden, false, "the authored scene dialogue appears after the cinematic");
    video.dispatch("error");
    assert.equal(cinematic.hidden, true, "a failed video load falls back to the authored scene dialog");
    assert.equal(cinematic.classList.contains("is-cinematic"), false);
    assert.equal(root.querySelector(".sow-story__footer").hidden, false, "the fallback keeps a way to continue");
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


test("contact responses wait behind an open campaign scene", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "queued_contact", entry: "scene",
        settings: { starting_troops: 1000 },
        steps: [
            { id: "scene", type: "scene", title_key: "scene", next: "objective" },
            { id: "objective", type: "objective", title_key: "objective", trigger: { type: "territory", value: 10, scope: "total" }, next: "end" },
            { id: "end", type: "end", title_key: "end" }
        ],
        reactions: [
            { id: "contact", when: { type: "contact", target: "snettisham" }, title_key: "contact", body_key: "contact.body" }
        ]
    });
    const facts = { contact_faction_ids: ["snettisham"], tiles_gained: 0 };
    assert.equal(machine.update(facts, {}).step.id, "scene");
    assert.equal(machine.view().reaction, undefined, "contact does not replace an open scene");
    assert.equal(machine.advance(null, "scene"), true);
    const response = machine.update(facts, {});
    assert.equal(response.reaction, "contact");
    assert.equal(machine.advance(null, "reaction-contact@snettisham"), true);
    assert.equal(machine.update(facts, {}).step.id, "objective");
    assert.equal(machine.view().reaction, undefined, "the contact response is shown only once");
});

test("campaign support reactions queue by delivery order, wait for their gate and preserve objective progress", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "test", entry: "gate",
        settings: { starting_troops: 1000 },
        steps: [
            { id: "gate", type: "objective", title_key: "gate", trigger: { type: "territory", value: 1, scope: "step" }, next: "talk" },
            { id: "talk", type: "scene", title_key: "talk", next: "contact" },
            { id: "contact", type: "objective", title_key: "contact", trigger: { type: "contact", targets: ["a", "b"], value: 2, scope: "episode" }, next: "end" },
            { id: "end", type: "end", title_key: "end" }
        ],
        reactions: [
            { id: "aid_a", after: "gate", when: { type: "support", target: "a" }, title_key: "aid.a.title", body_key: "aid.a.body" },
            { id: "aid_b", after: "gate", when: { type: "support", target: "b" }, title_key: "aid.b.title", body_key: "aid.b.body" }
        ]
    };
    const machine = campaign.create(definition);
    const receipts = {
        a: { deliveries: 2, first_tick: 9 },
        b: { deliveries: 1, first_tick: 3 }
    };
    machine.update({ tiles_gained: 0, support_deliveries_by_faction_id: {} }, {});
    assert.equal(machine.view().step.id, "gate");
    machine.update({ tiles_gained: 1, support_deliveries_by_faction_id: {} }, {});
    assert.equal(machine.view().step.id, "talk");
    const deferred = machine.update({ contact_faction_ids: [], support_deliveries_by_faction_id: receipts }, {});
    assert.equal(deferred.reaction, undefined, "support received during a scene waits until gameplay resumes");
    assert.equal(deferred.step.id, "talk");
    machine.advance(null, "talk");
    assert.equal(machine.view().step.id, "contact");

    const firstReaction = machine.update({ contact_faction_ids: ["a"], support_deliveries_by_faction_id: receipts }, {});
    assert.equal(firstReaction.reaction, "aid_b");
    assert.equal(firstReaction.paused, true);
    assert.equal(firstReaction.progress.current, 1);
    assert.equal(machine.advance(null, "reaction-aid_b"), true);
    assert.equal(machine.advance(null, "reaction-aid_b"), false);
    const secondReaction = machine.update({ contact_faction_ids: ["a"], support_deliveries_by_faction_id: receipts }, {});
    assert.equal(secondReaction.reaction, "aid_a");
    machine.advance(null, "reaction-aid_a");
    assert.equal(machine.update({ contact_faction_ids: ["a"], support_deliveries_by_faction_id: receipts }, {}).step.id, "contact");
    assert.equal(machine.view().progress.current, 1);
    assert.deepEqual(machine.state.reactionsShown, ["aid_b", "aid_a"]);
    assert.deepEqual(campaign.create(definition).state.reactionsShown, [], "a new run starts with no previous ally responses shown");
});


test("first-contact responses open on the contact fact and queue each faction once", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "instant_contact", entry: "objective",
        settings: { starting_troops: 1000 },
        steps: [
            { id: "objective", type: "objective", title_key: "objective", trigger: { type: "territory", value: 10, scope: "total" }, next: "after" },
            { id: "after", type: "scene", title_key: "after", next: "end" },
            { id: "end", type: "end", title_key: "end" }
        ],
        reactions: [
            { id: "contact_a", when: { type: "contact", target: "a" }, title_key: "a", body_key: "a_body" },
            { id: "contact_b", when: { type: "contact", target: "b" }, title_key: "b", body_key: "b_body" }
        ]
    });
    machine.update({ tiles_gained: 0, contact_faction_ids: [] }, {});
    const first = machine.update({ tiles_gained: 0, contact_faction_ids: ["b"] }, {});
    assert.equal(first.reaction, "contact_b");
    machine.advance(null, "reaction-contact_b@b");
    const second = machine.update({ tiles_gained: 0, contact_faction_ids: ["b", "a"] }, {});
    assert.equal(second.reaction, "contact_a");
    machine.advance(null, "reaction-contact_a@a");
    assert.equal(machine.update({ tiles_gained: 0, contact_faction_ids: ["b", "a"] }, {}).step.id, "objective");
    assert.match(campaignEditor, /Add contact response/);
    assert.match(campaignEditor, /On contact with/);
    assert.match(campaignEditor, /Simulate first " \+ \(isContact \? "contact"/);
});


test("first Iceni contact is friendly, then each neutral contact negotiates independently", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const roster = { factions: [
        { id: "kin_one", name: "Kin One", relation: "neutral" },
        { id: "kin_two", name: "Kin Two", relation: "neutral" },
        { id: "neutral", name: "Neutral", relation: "neutral" },
        { id: "neutral_two", name: "Neutral Two", relation: "neutral" },
        { id: "rome", name: "Rome", relation: "enemy" }
    ] };
    const definition = {
        version: 2, episode_id: "contact_order", entry: "objective",
        settings: { starting_troops: 1000 },
        steps: [
            { id: "objective", type: "objective", title_key: "objective", trigger: { type: "territory", value: 10, scope: "total" }, next: "end" },
            { id: "end", type: "end", title_key: "end" }
        ],
        reactions: [
            { id: "first", when: { type: "first_contact", targets: ["kin_one", "kin_two"] }, outcome: "allied", title_key: "first_title", body_key: "first_body" },
            { id: "kin_two", when: { type: "contact", target: "kin_two" }, choices: [{ id: "pay", label_key: "pay", relation: "allied", gold_cost: 200 }, { id: "refuse", label_key: "refuse", relation: "enemy" }], title_key: "kin_title", body_key: "kin_body" },
            { id: "neutral_terms", when: { type: "contact", relation: "neutral" }, choices: [{ id: "pay", label_key: "pay", relation: "allied", gold_cost: 200 }, { id: "refuse", label_key: "refuse", relation: "enemy" }], title_key: "neutral_title", body_key: "neutral_body" }
        ]
    };
    const machine = campaign.create(definition, undefined, roster);
    const contacts = { tiles_gained: 0, contact_faction_ids: ["rome", "neutral", "neutral_two", "kin_two", "kin_one"] };
    const first = machine.update(contacts, {});
    assert.equal(first.reaction, "first");
    assert.equal(first.reactionTarget, "kin_one");
    assert.equal(machine.advance(null, "reaction-first@kin_one"), true);
    const second = machine.update(contacts, {});
    assert.equal(second.reaction, "kin_two", "specific faction dialogue takes precedence over the neutral template");
    assert.equal(second.reactionTarget, "kin_two");
    assert.equal(machine.advance("pay", "reaction-kin_two@kin_two"), true);
    const third = machine.update(contacts, {});
    assert.equal(third.reaction, "neutral_terms");
    assert.equal(third.reactionTarget, "neutral");
    assert.equal(third.choices.find(choice => choice.id === "pay").gold_cost, 200);
    assert.equal(machine.update(contacts, {}).reactionTarget, "neutral", "a pending choice stays active until selected");
    assert.equal(machine.advance("refuse", "reaction-neutral_terms@neutral"), true);
    const fourth = machine.update(contacts, {});
    assert.equal(fourth.reaction, "neutral_terms");
    assert.equal(fourth.reactionTarget, "neutral_two", "each initially neutral faction negotiates on its own first contact");
});


test("campaign negotiation uses the canonical HUD gold balance, not player-list fields", async () => {
    async function chooseAtBalance(balance) {
        const commands = [], root = { hidden: true, isConnected: false };
        let viewOptions, rendered;
        const document = {
            body: { appendChild(node) { node.isConnected = true; } },
            documentElement: { dir: "ltr" },
            getElementById: () => null,
            createElement: () => root,
            addEventListener() {}
        };
        const model = {
            step: { id: "terms", type: "choice" }, paused: true, state: { choices: {} },
            reactionData: { id: "terms", choices: [
                { id: "pay", relation: "allied", gold_cost: 200 },
                { id: "refuse", relation: "enemy", gold_cost: 0 }
            ] },
            reactionTarget: "snettisham",
            choices: [
                { id: "pay", label_key: "pay", relation: "allied", gold_cost: 200 },
                { id: "refuse", label_key: "refuse", relation: "enemy", gold_cost: 0 }
            ]
        };
        let advanced;
        const window = {
            addEventListener() {},
            SOW_menu_command(message) { commands.push(JSON.parse(message)); },
            SOWCampaign: {
                validate: () => ({ errors: [] }), UI_TARGETS: {}, zoomInputMode: () => "wheel",
                create: () => ({ state: { choices: {} }, update: () => model, view: () => model,
                    advance(choiceId) { advanced = choiceId; model.reactionData = null; return true; } })
            },
            SOWCampaignView: { mount: (_root, options) => {
                viewOptions = options;
                return { render(value) { rendered = value; }, destroy() {} };
            } }
        };
        vm.runInNewContext(tutorial, { window, document, performance: { now: () => 0 }, console,
            fetch(url) {
                const data = url.endsWith(".triggers.json")
                    ? { episode_id: "boudica", settings: {}, strings: {}, speakers: {}, steps: [{ id: "terms", type: "choice" }] }
                    : { factions: [] };
                return Promise.resolve({ ok: true, json: () => Promise.resolve(data) });
            }
        });
        window.SOW_tutorial_state_update({ phase: "Playing", hud: {
            gold: balance,
            players: [{ id: 1, name: "Boudica", is_me: true }, { id: 2, name: "Snettisham", campaign_faction_id: "snettisham", centroid_x: 12, centroid_y: 34 }],
            settings: {}, tutorial: { active: true, episode_id: "boudica", facts: {} }
        } });
        await new Promise(setImmediate);
        const displayed = rendered.choices[0];
        viewOptions.onChoice("pay");
        return { commands, displayed, advanced };
    }

    const funded = await chooseAtBalance(6020);
    assert.equal(funded.displayed.gold_available, 6020);
    assert.equal(funded.displayed.gold_insufficient, false);
    const focus = funded.commands.find(command => command.type === "focus_world");
    assert.deepEqual([focus.x, focus.y], [12.5, 34.5]);
    const highlight = funded.commands.find(command => command.type === "set_dialog_border_highlight" && command.player_id === 2);
    assert.ok(highlight, "terms dialog highlights the requesting entity");
    assert.ok(funded.commands.some(command => command.type === "set_dialog_border_highlight" && command.player_id === null), "border highlight clears once the dialog is resolved");
    assert.equal(funded.commands.find(command => command.type === "resolve_campaign_diplomacy").relation, "allied");
    assert.equal(funded.commands.find(command => command.type === "resolve_campaign_diplomacy").gold_cost, 200);
    assert.equal(funded.advanced, "pay");

    const short = await chooseAtBalance(0);
    assert.equal(short.displayed.gold_insufficient, true);
    assert.equal(short.commands.find(command => command.type === "resolve_campaign_diplomacy").relation, "enemy");
    assert.equal(short.commands.find(command => command.type === "resolve_campaign_diplomacy").gold_cost, 0);
    assert.equal(short.advanced, "refuse");
});


test("Boudica City upgrade guidance and Ninth scene have English and Spanish copy", () => {
    const en = fs.readFileSync(path.join(shell, "../../sow-i18n/strings/en/web.toml"), "utf8");
    const es = fs.readFileSync(path.join(shell, "../../sow-i18n/strings/es/web.toml"), "utf8");
    assert.match(en, /^campaign_boudica_boudica_structure_upgrade_hint = "Upgrade the City once to raise troop capacity before the Ninth arrives\."$/m);
    assert.match(es, /^campaign_boudica_boudica_structure_upgrade_hint = "Mejora la Ciudad una vez para aumentar la capacidad de tropas antes de que llegue la Novena\."$/m);
    assert.match(es, /^campaign_boudica_boudica_city_upgrade_ready_title = "Listos para la Novena"$/m);
    assert.match(es, /^campaign_boudica_boudica_city_upgrade_ready_body = "Nuestra ciudad ya puede sostener un ejército mayor\. Cerialis marcha para socorrer Camulodunum; enfrentemos a la Novena con todas nuestras fuerzas\."$/m);
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
    assert.match(tutorial, /targetStep\.type === "choice" && targetStep\.marker[\s\S]*?send\("focus_world"/);
});


test("campaign objectives distinguish the intended contact, alliance, fleet and transfer action", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    function currentFor(trigger, before, after) {
        const machine = campaign.create({
            version: 2, episode_id: "fact_test", entry: "objective",
            settings: { starting_troops: 1000 },
            steps: [
                { id: "objective", type: "objective", title_key: "test", trigger: trigger, next: "pending" }
            ]
        });
        machine.update(before, {});
        return machine.update(after, {}).progress.current;
    }
    assert.equal(currentFor({ type: "contact", targets: ["snettisham", "stonea"], scope: "total" }, { contact_faction_ids: [] }, { contact_faction_ids: ["stonea"] }), 1, "multi-contact targets count stable faction IDs");
    assert.equal(currentFor({ type: "alliance", target: "catuvellauni", value: 1, scope: "total" }, { alliance_faction_ids: [] }, { alliance_faction_ids: ["other_tribe"] }), 0);
    assert.equal(currentFor({ type: "fleet", unit: "TransportShip", target: "legio_xiv", value: 1, scope: "step" },
        { fleets_by_type: {}, transport_fleets_by_faction_id: {} },
        { fleets_by_type: { TradeShip: 1 }, transport_fleets_by_faction_id: { legio_xiv: 1 } }), 0);
    assert.equal(currentFor({ type: "fleet", unit: "TransportShip", target: "legio_xiv", value: 1, scope: "step" },
        { fleets_by_type: {}, transport_fleets_by_faction_id: {} },
        { fleets_by_type: { TransportShip: 1 }, transport_fleets_by_faction_id: { other_legion: 1 } }), 0);
    assert.equal(currentFor({ type: "fleet", unit: "TransportShip", target: "legio_xiv", value: 1, scope: "step" },
        { fleets_by_type: {}, transport_fleets_by_faction_id: {} },
        { fleets_by_type: { TransportShip: 1 }, transport_fleets_by_faction_id: { legio_xiv: 1 } }), 1);
    const transfer = { type: "resource_transfer", recipient: "snettisham", resources: ["gold", "troops"], value: 1, scope: "step" };
    const emptyTransfer = { resource_transfers_by_recipient_faction_id: {} };
    assert.equal(currentFor(transfer, emptyTransfer, { resource_transfers_by_recipient_faction_id: { other: { gold_troops: 1 } } }), 0);
    assert.equal(currentFor(transfer, emptyTransfer, { resource_transfers_by_recipient_faction_id: { snettisham: { gold: 1, troops: 0, gold_troops: 0 } } }), 0);
    assert.equal(currentFor(transfer, emptyTransfer, { resource_transfers_by_recipient_faction_id: { snettisham: { gold: 1, troops: 1, gold_troops: 1 } } }), 1);
    const tutorialTransfer = { type: "resource_transfer", recipient: "stonea", value: 1, scope: "episode" };
    for (const sent of [{ total: 1, gold: 1, troops: 0 }, { total: 1, gold: 0, troops: 1 }]) {
        const machine = campaign.create({
            version: 2, episode_id: "transfer_race_test", entry: "enter_gold",
            steps: [
                { id: "enter_gold", type: "guide", trigger: { type: "ui", action: "transfer_gold", scope: "step" }, next: "send_aid" },
                { id: "send_aid", type: "guide", trigger: tutorialTransfer, next: "done" },
                { id: "done", type: "scene", title_key: "done" }
            ]
        });
        machine.update({ resource_transfers_by_recipient_faction_id: { stonea: { total: 0 } } }, { transfer_gold: 0 });
        const view = machine.update({ resource_transfers_by_recipient_faction_id: { stonea: sent } }, { transfer_gold: 1 });
        assert.equal(view.step.id, "done", "an accepted Stonea transfer on the guide transition tick must count");
    }
});


test("campaign interpreter facts are all published by the WASM HUD payload", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const facts = new Set([...Object.values(campaign.METRICS).filter(fact => fact !== campaign.METRICS.hover), "tiles", "defeated", "defeated_faction_ids", "contacts", "contact_faction_ids", "attacks_by_faction_id", "fleets_by_type", "transport_fleets_by_faction_id", "support_deliveries_by_faction_id", "resource_transfers_by_recipient_faction_id", "alliance_faction_ids", "camera_target_radius"]);
    for (const fact of facts) assert.match(webMenu, new RegExp('"' + fact + '"\\s*:'), `WASM HUD is missing campaign fact ${fact}`);
    assert.match(tutorial, /hover_events: runtime\.hoverEvents/);
    assert.match(tutorial, /runtime\.machine\.update\(facts, runtime\.uiCounts\)/);
});


test("campaign dialogue starts each new beat at the top of its scroll panel", () => {
    assert.match(campaignView, /if \(renderKey !== key\) \{\s*renderKey = key;\s*conversation\.scrollTop = 0;\s*scrollContent\.scrollTop = 0;\s*actions\.scrollTop = 0;/);
    assert.match(storyCss, /\.sow-story__scroll\s*\{[^}]*overflow-y: auto/);
});


test("an active campaign keeps its loaded definition when the editor draft changes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "snapshot_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: { "tutorial.opening": "Original" } }, speakers: {}, entry: "opening",
        steps: [{ id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" }, { id: "ending", type: "end", title_key: "tutorial.ending" }]
    };
    const activeRun = campaign.create(definition);
    definition.strings.en["tutorial.opening"] = "Edited draft";
    definition.steps[0].next = "missing_step";
    assert.equal(activeRun.definition.strings.en["tutorial.opening"], "Original");
    assert.equal(activeRun.view().step.next, "ending");
});


test("campaign episode JSON is cached only while paired files are loading", () => {
    assert.match(tutorial, /var request = Promise\.all\(\[/);
    assert.match(tutorial, /runtime\.loading\[episodeId\] = request;\s*function releaseRequest\(\) \{\s*if \(runtime\.loading\[episodeId\] === request\) delete runtime\.loading\[episodeId\];\s*}\s*request\.then\(releaseRequest, releaseRequest\)/);
    assert.match(tutorial, /if \(open\) \{\s*runtime\.uiPaused = true;\s*runtime\.pausedAction = null;/);
});


test("campaign start keeps its loaded script and replay clicks belong to that episode", async () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "boudica",
        settings: { starting_troops: 1000 }, speakers: {},
        entry: "opening",
        steps: [{ id: "opening", type: "scene", title_key: "tutorial.open", attack_ratio_on_enter: 0.25, next: "ending" }, { id: "ending", type: "end", title_key: "tutorial.end" }]
    };
    const roster = { map: "eastanglia", player_spawn: [1, 1], factions: [{ id: "enemy", name: "Enemy", x: 2, y: 2, starting_troops: 500, relation: "enemy", civ: "Roman Empire", leader: "Caesar" }] };
    const clone = value => JSON.parse(JSON.stringify(value));
    const buttons = ["boudica", "six_sky_ep2"].map(episodeId => ({
        dataset: { episodeId }, disabled: false, getClientRects: () => [{}], matches: () => false,
        contains(target) { return target === this; }
    }));
    const listeners = {}, storyRoot = { hidden: true, isConnected: false, contains: () => false };
    const document = {
        body: { appendChild(node) { node.isConnected = true; } }, documentElement: { dir: "ltr" },
        createElement: () => storyRoot, getElementById: () => null,
        querySelector: () => null,
        querySelectorAll: selector => selector === campaign.UI_TARGETS.campaign_replay ? buttons : [],
        addEventListener(type, callback) { listeners[type] = callback; }
    };
    let fetches = 0, activeRun, lastUi;
    const commands = [];
    const window = {
        addEventListener() {}, SOW_menu_command(message) { commands.push(JSON.parse(message)); },
        SOW_t: key => ({ "tutorial.open": "Original", "tutorial.end": "End" })[key] || "[" + key + "]",
        SOW_hasText: key => Boolean(window.SOW_t(key)),
        SOWCampaign: { ...campaign, create(data) {
            activeRun = campaign.create(data);
            const update = activeRun.update;
            activeRun.update = (facts, ui, now) => { lastUi = { ...ui }; return update(facts, ui, now); };
            return activeRun;
        } },
        SOWCampaignView: { mount: () => ({ render() {}, destroy() {} }) }
    };
    vm.runInNewContext(tutorial, { window, document, performance: { now: () => 0 }, console,
        fetch(url) { fetches++; return Promise.resolve({ ok: true, json: () => Promise.resolve(clone(url.endsWith(".triggers.json") ? definition : roster)) }); }
    });
    window.SOW_startCampaignEpisode("boudica");
    await new Promise(setImmediate);
    definition.steps[0].title_key = "tutorial.edited";
    const state = { phase: "Playing", hud: { tutorial: { active: true, episode_id: "boudica", facts: {} }, players: [], settings: {} } };
    window.SOW_tutorial_state_update(state);
    await new Promise(setImmediate);
    assert.equal(fetches, 2);
    assert.equal(activeRun.definition.steps[0].title_key, "tutorial.open");
    assert.equal(commands.find(command => command.type === "set_attack_ratio").ratio, 0.25);
    listeners.click({ type: "click", target: buttons[1] });
    window.SOW_tutorial_state_update(state);
    assert.equal(lastUi.campaign_replay, undefined);
    listeners.click({ type: "click", target: buttons[0] });
    window.SOW_tutorial_state_update(state);
    assert.equal(lastUi.campaign_replay, 1);
});


test("campaign funnel records each step once and completes only after the ending", async () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "step_funnel_test", default_locale: "en",
        settings: { starting_troops: 1000 }, strings: {}, speakers: {},
        entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    };
    const roster = { map: "test_map", player_spawn: [1, 1], factions: [] };
    const root = { hidden: true, isConnected: false };
    const events = [];
    let viewOptions;
    const document = {
        body: { appendChild(node) { node.isConnected = true; } }, documentElement: { dir: "ltr" },
        getElementById: () => null, createElement: () => root,
        addEventListener() {}, querySelector: () => null
    };
    const window = {
        addEventListener() {},
        SOW_menu_command() {},
        SOW_trackExperienceEvent(payload) { events.push(JSON.parse(payload)); },
        SOW_t: key => ({ "tutorial.opening": "Opening", "tutorial.ending": "Ending" })[key] || "[" + key + "]",
        SOW_hasText: key => Boolean(window.SOW_t(key)),
        SOWCampaign: campaign,
        SOWCampaignView: { mount: (_root, options) => {
            viewOptions = options;
            return { render() {}, destroy() {} };
        } }
    };
    vm.runInNewContext(tutorial, { window, document, performance: { now: () => 0 }, console,
        fetch(url) {
            return Promise.resolve({ ok: true, json: () => Promise.resolve(url.endsWith(".triggers.json") ? definition : roster) });
        }
    });
    window.SOW_startCampaignEpisode("step_funnel_test");
    await new Promise(setImmediate);
    const state = { phase: "Playing", hud: {
        tutorial: { active: true, episode_id: "step_funnel_test", facts: {} }, players: [], settings: {}
    } };
    window.SOW_tutorial_state_update(state);
    await new Promise(setImmediate);
    window.SOW_tutorial_state_update(state);
    assert.deepEqual(JSON.parse(JSON.stringify(window.SOW_tutorial_exit_context("step_funnel_test"))), {
        step_id: "opening", step_index: 0, action: "fail"
    });
    viewOptions.onContinue();
    viewOptions.onContinue();
    window.SOW_tutorial_state_update(state);
    assert.equal(window.SOW_tutorial_exit_context("step_funnel_test"), false);
    assert.deepEqual(events, [
        { name: "tutorial_start", props: { episode_id: "step_funnel_test" } },
        { name: "tutorial_step", props: { episode_id: "step_funnel_test", step_id: "opening", step_index: 0, action: "start" } },
        { name: "tutorial_step", props: { episode_id: "step_funnel_test", step_id: "opening", step_index: 0, action: "complete" } },
        { name: "tutorial_step", props: { episode_id: "step_funnel_test", step_id: "ending", step_index: 1, action: "start" } },
        { name: "tutorial_step", props: { episode_id: "step_funnel_test", step_id: "ending", step_index: 1, action: "complete" } },
        { name: "campaign_episode_complete", props: { episode_id: "step_funnel_test" } }
    ]);
});


test("campaign decisions keep their selected branch and guard stale callbacks", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "branch_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "decision" },
            { id: "decision", type: "choice", title_key: "tutorial.decision", choices: [
                { id: "gather", label_key: "tutorial.gather", next: "gather_step" },
                { id: "strike", label_key: "tutorial.strike", next: "strike_step" }
            ] },
            { id: "gather_step", type: "scene", title_key: "tutorial.gather_step", next: "rejoin" },
            { id: "strike_step", type: "scene", title_key: "tutorial.strike_step", next: "rejoin" },
            { id: "rejoin", type: "scene", title_key: "tutorial.rejoin", next: "ending", routes: [{ when: { choice: "decision", equals: "strike" }, next: "strike_echo" }] },
            { id: "strike_echo", type: "scene", title_key: "tutorial.strike_echo", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.equal(machine.advance(null, "opening"), true);
    assert.equal(machine.advance("strike", "decision"), true);
    assert.equal(machine.view().step.id, "strike_step");
    assert.equal(machine.advance(null, "decision"), false);
    assert.equal(machine.state.choices.decision, "strike");
    assert.equal(machine.advance(null, "strike_step"), true);
    assert.equal(machine.advance(null, "rejoin"), true);
    assert.equal(machine.view().step.id, "strike_echo");
    assert.match(campaignEngine, /expectedStepId/);
});


test("Boudica spoken cards and contact responses require named existing portraits", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definitionPath = path.join(shell, "../../assets/campaign/boudica.triggers.json");
    const rosterPath = path.join(shell, "../../assets/campaign/boudica.json");
    const avatarDir = path.join(shell, "../../assets/gameplay/avatars");
    const definition = JSON.parse(fs.readFileSync(definitionPath, "utf8"));
    const roster = JSON.parse(fs.readFileSync(rosterPath, "utf8"));
    const options = { hasText: () => true, hasAvatar: avatar => fs.existsSync(path.join(avatarDir, avatar + ".webp")) };
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
    delete missingDynamicRole.reactions.find(reaction => reaction.id === "neutral_contact_terms").speaker_role_key;
    assert.ok(report(missingDynamicRole).errors.some(issue => issue.field === "reactions.speaker"));

    const missingFactionPortrait = structuredClone(definition);
    roster.factions.find(faction => faction.id === "stonea").avatar = "null";
    assert.ok(report(missingFactionPortrait).errors.some(issue => issue.field === "speaker" && /portrait/.test(issue.message)));
    roster.factions.find(faction => faction.id === "stonea").avatar = "stonea";

    const completion = definition.steps.find(step => step.id === "boudica_complete");
    assert.equal(completion.type, "end");
    assert.equal(completion.body_key, undefined);
    assert.equal(completion.speaker, undefined, "the technical completion card remains unvoiced");

    assert.match(campaignView, /data:image\/svg\+xml/);
    assert.match(campaignView, /sow_portrait_retry=1/);
    assert.match(campaignView, /requestId !== portraitRequestId/);
    assert.match(campaignView, /portrait\.hidden = false; root\.classList\.add\("has-portrait"\)/);
    assert.match(campaignView, /dialogue_id: stepId, image: source/);
    assert.doesNotMatch(campaignView, /image\.addEventListener\("error",\s*\(\)\s*=>\s*\{\s*portrait\.hidden\s*=\s*true/);

    assert.match(campaignView, /const speakerName = \[roleName, factionName\]\.filter\(Boolean\)\.join\(" · "\)/);
    assert.match(campaignView, /character\.name_key \? t\(character\.name_key\)/);
    assert.match(campaignEditor, /if \(speaker\.name_key\) catalogTextField\(card, "Role", speaker\.name_key, 2\)/);
    assert.match(campaignEditor, /if \(value\.indexOf\("faction:"\) === 0\) \{ speaker\.faction = value\.slice\(8\); delete speaker\.avatar; \}/);
});


test("campaign view recovers portraits and renders compact objective guidance", async () => {
    const createdImages = [], warnings = [];
    const resizeObservers = [], resizeFrames = [];
    const documentListeners = {};
    let continueCalls = 0;
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
        onContinue() { continueCalls++; }, onDismiss() {}, onChoice() {}
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
    assert.equal(root.querySelector(".sow-story__speaker").textContent, "Recover Speaker");
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
    assert.equal(root.querySelector(".sow-story__speaker").textContent, "First Speaker");

    const objective = { id: "expand_250", type: "objective", title_key: "expand_title", hint_key: "expand_hint", trigger: { type: "territory", value: 250 }, guide: { kind: "world", target: "expand", gesture: "tap" } };
    const objectiveModel = { definition: { episode_id: "compact_test", speakers: {}, steps: [objective] }, step: objective, line: objective, paused: false, done: false, choices: [], progress: { current: 120, target: 250 }, state: { line: 0 } };
    root.clientWidth = 390; root.clientHeight = 844;
    renderer.render(objectiveModel, { anchor: { x: 220, y: 360 }, gestureLabel: "Expand the frontier" });
    assert.equal(root.querySelector(".sow-story__objective").hidden, true, "portrait removes the quest card without reserving its slot");
    assert.equal(root.querySelector(".sow-story__gesture").hidden, false, "the real hand remains visible on its target");
    assert.equal(root.querySelector(".sow-story__gesture-label").hidden, false);
    assert.equal(root.querySelector(".sow-story__gesture-copy").textContent, "Expand the frontier");
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
    assert.equal(root.querySelector(".sow-story__gesture-hint").textContent, "Spread two fingers apart");
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
    assert.equal(root.querySelector(".sow-story__gesture-copy").textContent, "Allied support is approaching");
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
    assert.equal(continueCalls, 0, "an outside map tap cannot skip a scene");
    renderer.destroy();
});


test("troop objectives advance on the first update that satisfies the minimum", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "troop_minimum_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "minimum", steps: [
            { id: "minimum", type: "objective", title_key: "tutorial.minimum", hint_key: "tutorial.minimum_hint", trigger: { type: "troops", value: 1500, scope: "step" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.deepEqual(machine.update({ troops: 1400 }, {}, 0).progress, { current: 1400, target: 1500 });
    assert.equal(machine.update({ troops: 1500 }, {}).step.id, "ending");
});


test("a completed objective advances immediately after an exit modal closes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "modal_pause_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "objective", steps: [
            { id: "objective", type: "objective", title_key: "tutorial.objective", hint_key: "tutorial.objective_hint", trigger: { type: "territory", value: 1, scope: "step" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    machine.update({ tiles_gained: 0 }, {});
    machine.setPaused(true);
    assert.equal(machine.update({ tiles_gained: 1 }, {}).step.id, "objective");
    assert.equal(machine.advance(null, "objective"), false);
    machine.setPaused(false);
    assert.equal(machine.update({ tiles_gained: 1 }, {}).step.id, "ending");
});


test("UI objectives respect step, episode, and total scope", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const completed = scope => {
        const machine = campaign.create({
            version: 2, episode_id: "ui_scope_test", default_locale: "en", settings: { starting_troops: 0 },
            strings: { en: {} }, speakers: {}, entry: "opening", steps: [
                { id: "opening", type: "scene", title_key: "tutorial.opening", next: "objective" },
                { id: "objective", type: "objective", title_key: "tutorial.objective", hint_key: "tutorial.objective_hint", trigger: { type: "ui", action: "hud_center_camera", scope }, next: "ending" },
                { id: "ending", type: "end", title_key: "tutorial.ending" }
            ]
        });
        machine.update({}, { hud_center_camera: 0 }, 0);
        machine.update({}, { hud_center_camera: 1 }, 100);
        machine.advance(null, "opening");
        return machine.update({}, { hud_center_camera: 1 }, 200).step.id === "ending";
    };

    assert.equal(completed("step"), false);
    assert.equal(completed("episode"), true);
    assert.equal(completed("total"), true);
});


test("campaign dialogue regains keyboard focus after the leave modal is canceled", () => {
    assert.match(campaignView, /const wasHidden = root\.hidden/);
    assert.match(campaignView, /const focusModal = modal && \(!wasModal \|\| wasHidden\)/);
    assert.match(campaignView, /else if \(focusModal\) focusAction\(\)/);
});


test("campaign dialogs need an explicit continue and outside-click cannot skip a scene", () => {
    assert.match(campaignView, /objective\.hidden = modal \|\| context\.hideObjective === true/);
    assert.match(storyCss, /\.sow-story\.is-modal \{ pointer-events: none; \}/);
    assert.match(storyCss, /\.sow-story__shade \{[^}]*pointer-events: none/s);
    assert.match(storyCss, /\.sow-story__dialog \{[^}]*pointer-events: auto/s);
    assert.match(campaignView, /doc\.addEventListener\("click", outsideClick, true\)/);
    assert.match(campaignView, /doc\.removeEventListener\("click", outsideClick, true\)/);
    assert.match(campaignView, /event\.target\.closest\("button, a\[href\], input:not\(\[type='hidden'\]\), select, textarea, \[role='button'\], \[data-command\], \[data-map-action\]"\)/);
    assert.match(campaignView, /model\.step\.type === "choice" \|\| model\.choices\.length[\s\S]*?triggerNudge\(\)/);
    assert.match(campaignView, /function dismissDialog\(\)[\s\S]*?if \(model\.step\.type === "choice" \|\| model\.choices\.length\) \{ triggerNudge\(\); return; \}[\s\S]*?if \(options\.onDismiss\) options\.onDismiss\(\);\s*else if \(options\.onContinue\) options\.onContinue\(\)/);
    assert.match(campaignView, /if \(control\) return;[\s\S]*?if \(model\.step\.type === "scene"\) return;[\s\S]*?model\.step\.type !== "choice"[\s\S]*?options\.onContinue\(\)/);
    assert.doesNotMatch(tutorial, /: openLeaveMatch/);
});


test("outside tap on a pending choice ignites the golden button nudge", () => {
    assert.match(campaignView, /model\.step\.type === "choice"[\s\S]*?triggerNudge\(\)/);
    assert.match(campaignView, /dialog\.classList\.add\("is-nudged"\)/);
    assert.match(campaignView, /vibrate\(12\)/);
    assert.match(campaignView, /dialog\.classList\.add\("is-waiting"\)/);
    assert.match(campaignView, /if \(beatChanged \|\| step\.type !== "choice"\) clearNudge\(\)/);
    assert.match(storyCss, /\.sow-story__dialog\.is-nudged \.sow-story__choice \{[^}]*animation: story-nudge-breathe 1\.6s ease-in-out infinite/);
    assert.match(storyCss, /@keyframes story-nudge-breathe \{[\s\S]*?border-color: #ffd98a/);
    assert.match(storyCss, /@keyframes story-nudge-sweep \{/);
    assert.match(storyCss, /@keyframes story-nudge-twinkle \{/);
    assert.match(storyCss, /\.sow-story__dialog\.is-nudged \.sow-story__choice::after \{[^}]*linear-gradient\(100deg, transparent, rgb\(255 236 190 \/ 50%\), transparent\)/);
    assert.match(storyCss, /\.sow-story\.is-reduced \.sow-story__dialog\.is-nudged \.sow-story__choice/);
});


test("campaign dialogue keeps a compact speaker portrait on narrow screens", () => {
    const mobileStart = storyCss.indexOf("@container (max-width: 640px)");
    const mobileEnd = storyCss.indexOf("@container (max-width: 380px)", mobileStart);
    const mobileStory = storyCss.slice(mobileStart, mobileEnd);
    const compactPortrait = storyCss.match(/#sow-story\[data-compact="true"\] \.sow-story\.has-portrait \.sow-story__main \{[^}]*\}/);
    assert.match(mobileStory, /\.sow-story\.has-portrait \.sow-story__main \{ grid-template-columns: var\(--story-portrait-size, min\(36cqw, 40svh\)\) minmax\(0, 1fr\); \}/);
    assert.match(storyCss, /\.sow-story__portrait \{ width: var\(--story-portrait-size,[^}]*height: var\(--story-portrait-size,[^}]*aspect-ratio: 1;[^}]*align-self: start/);
    assert.ok(compactPortrait);
    assert.match(compactPortrait[0], /grid-template-columns: var\(--story-portrait-size, min\(36cqw, 40svh\)\) minmax\(0, 1fr\)/);
    assert.doesNotMatch(mobileStory + compactPortrait[0], /(?:144px|64px)/);
    assert.match(campaignView, /function syncMobilePortrait\(\)[\s\S]*?heading\.getBoundingClientRect\(\)\.height \+ headingGap \+ scrollContent\.scrollHeight[\s\S]*?Math\.min\(root\.clientWidth \* 0\.36, root\.clientHeight \* 0\.4\)[\s\S]*?const size = Math\.min\(contentHeight, maxSize\)[\s\S]*?new ResizeObserverCtor\(\(\) => \{[\s\S]*?view\.requestAnimationFrame\([\s\S]*?syncMobilePortrait\(\)/);
    assert.doesNotMatch(campaignView, /const contentHeight = conversation\.getBoundingClientRect\(\)\.height/);
    assert.match(campaignView, /portraitObserver\.observe\(conversation\)/);
    assert.match(campaignView, /portraitObserver\.observe\(root\)/);
    assert.match(campaignView, /portraitObserver\.disconnect\(\)/);
    assert.doesNotMatch(storyCss, /grid-template-columns: (?:72px|64px|clamp\(64px, 17vw, 72px\)) minmax\(0, 1fr\)/);
    assert.doesNotMatch(storyCss, /@container \(max-width: 380px\) \{[\s\S]*?\.sow-story__portrait \{ display: none/);
});


test("campaign dialogue resolves story copy through the shared catalog", () => {
    const resolverStart = tutorial.indexOf("function tr(key)");
    const resolverEnd = tutorial.indexOf("function zoomMode()", resolverStart);
    assert.ok(resolverStart >= 0 && resolverEnd > resolverStart);
    const resolver = tutorial.slice(resolverStart, resolverEnd);
    assert.match(resolver, /window\.SOW_t\(key\)/);
    assert.match(resolver, /replaceFactionStoryNames\(value, definition, runtime\.roster\)/);
    assert.doesNotMatch(resolver, /definition\.strings/);
    assert.match(campaignView, /options\.translate\(key\)/);
});


test("mobile chapter dialogue hugs its copy while docked at the bottom", () => {
    const mobileStory = storyCss.slice(storyCss.indexOf("@container (max-width: 640px)"), storyCss.indexOf("@container (max-width: 380px)"));
    assert.match(mobileStory, /\.sow-story\.is-chapter \.sow-story__main \{[^}]*flex: 0 1 auto;[^}]*padding-block: 8px/);
    assert.match(mobileStory, /\.sow-story\.is-chapter \.sow-story__conversation \{ padding-block: 0; \}/);
    assert.match(mobileStory, /\.sow-story\.is-chapter \.sow-story__dialog \{ bottom: var\(--story-safe-bottom\); transform: none; \}/);
    assert.match(storyCss, /\.sow-story\.is-chapter \.sow-story__dialog \{ bottom: 50%; transform: translateY\(50%\)/);
});


test("mobile story dialogs scroll long text and choices while keeping speaker controls reachable", () => {
    const mobileStory = storyCss.slice(storyCss.indexOf("@container (max-width: 640px)"), storyCss.indexOf("@container (max-width: 380px)"));
    assert.match(mobileStory, /\.sow-story__main \{[^}]*flex: 0 1 auto/);
    assert.match(storyCss, /@container \(max-width: 640px\) \{[\s\S]*?max-height: min\(calc\(100% - var\(--story-safe-top\) - var\(--story-safe-bottom\)\), 60svh\)/);
    assert.match(storyCss, /#sow-story\[data-compact="true"\] \.sow-story__dialog \{[\s\S]*?max-height: min\(calc\(100% - var\(--story-safe-top\) - var\(--story-safe-bottom\)\), 72svh\)/);
    assert.match(storyCss, /\.sow-story__portrait img \{[^}]*width: 100%; height: 100%;[^}]*object-fit: cover/);
    assert.match(storyCss, /\.sow-story__scroll\s*\{[^}]*flex: 1 1 auto; min-height: 0; overflow-y: auto/);
    assert.match(storyCss, /\.sow-story__actions \{[^}]*overflow-y: auto/);
    assert.match(storyCss, /\.sow-story__choices \{ grid-template-columns: repeat\(auto-fit, minmax\(min\(100%, 280px\), 1fr\)\)/);
    assert.match(campaignView, /<div class="sow-story__main"><div class="sow-story__portrait"[\s\S]*?<div class="sow-story__conversation">[\s\S]*?<header class="sow-story__heading"[\s\S]*?data-story-dismiss[\s\S]*?<div class="sow-story__scroll"><h2[\s\S]*?<p class="sow-story__body"[\s\S]*?<\/div><\/div><\/div>'\s*\+\s*'<div class="sow-story__actions"><div class="sow-story__choices"><\/div><footer class="sow-story__footer"[\s\S]*?data-story-continue/);
    assert.match(campaignView, /footer\.hidden = step\.type === "choice"/);
    assert.match(campaignView, /button\.className = "sow-story__choice"[\s\S]*?choices\.appendChild\(button\)/);
});


test("desktop retains the quest card while compact play uses the shared guided label", () => {
    assert.doesNotMatch(campaignView, /data-story-objective-toggle|objective_details/);
    assert.doesNotMatch(storyCss, /details-toggle|data-details-expanded/);
    assert.match(campaignView, /objective\.hidden = modal \|\| context\.hideObjective === true \|\| compactViewport/);
    assert.match(campaignView, /function isCompactViewport\(root, width, height\)/);
    assert.match(campaignView, /function compactGuideMetric\(step, progress\)/);
    assert.match(campaignView, /class="sow-story__gesture" data-tutorial-hand hidden/);
    assert.doesNotMatch(campaignView, /class="sow-story__gesture"[^>]*aria-hidden/);
    assert.match(storyCss, /\.sow-story__gesture \{[^}]*pointer-events: none/);
    assert.match(storyCss, /\.sow-story__gesture\.is-compact \.sow-story__gesture-copy \{[^}]*white-space: normal;[^}]*overflow-wrap: anywhere/);
    assert.match(storyCss, /\.sow-story__gesture\.is-compact \.sow-story__gesture-label \{[^}]*max-height: min\(180px, calc\(100dvh - 24px\)\);[^}]*overflow: auto/);
    assert.match(hud, /sow-hud-nameplate/);
    assert.match(hud, /hudRefs\.plateAvatar\.src = plateSrc/);
    assert.match(hud, /hudRefs\.plateAvatar\.alt = plateLeader\.name/);
    assert.match(hud, /hudRefs\.plateName\.textContent = plateLeader\.name/);
    assert.match(hud, /hudRefs\.plateTroops\.textContent = plateText/);
    assert.match(hud, /hudRefs\.plateFill\.style\.width = plateFillStr/);
    assert.match(hud, /var playerPlateHidden = Boolean\(hud\.is_spectating\)/);
    assert.doesNotMatch(hud.slice(hud.indexOf("var playerPlateHidden"), hud.indexOf("if (hudRefs.fps)")), /tutorialOn/);
    assert.match(hud, /hudRefs\.plateGold\.dataset\.val = goldText/);
    assert.match(hud, /hudRefs\.plateGoldRate\.textContent = goldRateText/);
    assert.match(hud, /hudRefs\.plateTroopRate\.textContent = troopRateText/);
    const statusRight = hud.slice(hud.indexOf("sow-hud__status-right"), hud.indexOf("</header>"));
    assert.doesNotMatch(statusRight, /sow-hud-nameplate/);
    assert.match(hud, /class="sow-hud__nameplate-readout"><span id="sow-hud-nameplate-troops"><\/span>' \+ hudIcon\("troops", "sow-hud__inline-icon"\)/);
    const nameplateMarkup = hud.slice(hud.indexOf('id="sow-hud-nameplate"'), hud.indexOf('id="sow-hud-hover-card"'));
    assert.match(nameplateMarkup, /sow-hud-nameplate-gold/);
    assert.match(nameplateMarkup, /sow-hud-nameplate-gold-rate/);
    assert.match(nameplateMarkup, /sow-hud-nameplate-troop-rate/);
    assert.doesNotMatch(hud, /id="sow-hud-resource-row"/);
    assert.match(hudCss, /\.sow-hud__nameplate:not\(\[hidden\]\) \{ display: grid/);
    assert.match(hudCss, /\.sow-hud__nameplate \{[^}]*top: max\(12px, var\(--sow-sat\)\)[^}]*inset-inline-start: max\(16px, var\(--sow-sal\)\)[^}]*height: var\(--sow-player-panel-height\)/);
    assert.match(hudCss, /\.sow-hud__nameplate-avatar \{[^}]*grid-row: 1 \/ 3;[^}]*width: 100%;[^}]*height: auto;[^}]*aspect-ratio: 1;/);
    assert.doesNotMatch(hudCss, /\.sow-hud__nameplate-avatar\s*\{[^}]*height: 100%/,
        "responsive rules cannot stretch a square avatar across the whole plate");
    assert.match(hudCss, /\.sow-hud__nameplate-economy \{[^}]*grid-column: 1 \/ -1;[^}]*align-self: stretch;/,
        "the three indicators use the full width and height of the bottom row");
    assert.match(hudCss, /\.sow-hud__nameplate-copy \{\s*display: contents;\s*\}/);
    assert.match(hudCss, /\.sow-hud__nameplate-copy strong \{[^}]*overflow: hidden;[^}]*text-overflow: ellipsis;[^}]*white-space: nowrap/);
    assert.match(hudCss, /\.sow-hud__nameplate-copy strong \{[^}]*font-size: 20px/);
    assert.match(hudCss, /\.sow-hud__nameplate-bar \{[^}]*grid-column: 2;[^}]*grid-row: 2;/);
    assert.match(hudCss, /\.sow-hud__nameplate-bar > span \{[^}]*inset: 0;[^}]*display: flex;[^}]*align-items: center;[^}]*justify-content: center/);
    assert.match(hud, /formatNameplateTroops\(troops\) \+ " \/ " \+ formatNameplateTroops\(maxTroops\)/);
    assert.match(hud, /var plateFillPct = maxTroops > 0 \? Math\.min\(100, Math\.max\(0, \(troops \/ maxTroops\) \* 100\)\) : 0/);
    assert.match(hudCss, /\.sow-hud__nameplate \{[^}]*width: 280px/);
    assert.match(hudCss, /@media \(max-width: 720px\) \{\s*:root \{[^}]*--sow-player-panel-height: 84px;[^}]*\}[^]*?\.sow-hud__nameplate \{[^}]*width: min\(224px,/);
    assert.match(hudCss, /@media \(max-width: 720px\) \{\s*:root \{[^}]*--sow-tutorial-progress-height: 28px;[^}]*--sow-tutorial-progress-label-size: 15px/);
    assert.match(hudCss, /@media \(max-width: 720px\) \{[^]*?\.sow-hud__nameplate-copy strong \{ font-size: 18px; \}/);
    assert.match(hud, /"--sow-player-panel-height": \(storyRoot \? 92 \* density : 92\)\.toFixed\(2\) \+ "px"[\s\S]*?if \(!storyRoot\) \{[\s\S]*?values\["--sow-tutorial-progress-height"\] = "24px"[\s\S]*?values\["--sow-tutorial-progress-label-size"\]/);
    assert.match(hudCss, /grid-template-columns: 46px minmax\(0, 1fr\)/);
    assert.match(storyCss, /\.sow-story__meter progress \{[^}]*height: var\(--sow-tutorial-progress-height\)/);
    assert.match(storyCss, /\.sow-story__meter output \{[^}]*inset: 0;[^}]*display: grid;[^}]*place-items: center;[^}]*font-size: var\(--sow-tutorial-progress-label-size\)/);
    assert.doesNotMatch(campaignView, /objectiveToggle|detailsExpanded|data-story-objective-toggle/);
    assert.match(campaignView, /statusOnly \|\| !step\.guide/);
    assert.match(campaignView, /compactStatusLabel\(step, context, t\)/);
    assert.match(tutorial, /root\.id = "sow-story"/);
    const desktopObjective = storyCss.match(/#sow-story \.sow-story__objective \{[^}]*\}/);
    assert.ok(desktopObjective, "tutorial objective uses its own stacked placement");
    assert.match(desktopObjective[0], /inset-inline-start: max\(16px, var\(--story-inset-left\)\)/);
    assert.match(desktopObjective[0], /calc\(max\(12px, var\(--story-inset-top\)\) \+ var\(--sow-player-panel-height\) \+ var\(--sow-tutorial-player-panel-gap\)\)/);
    assert.doesNotMatch(storyCss, /@container \(max-width: 720px\)[\s\S]*?\.sow-story__objective/);
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


test("short landscape compaction stays on mobile and the editor reuses the shared dialogue", () => {
    assert.match(storyCss, /\.sow-story__dialog \{[^}]*max-height: calc\(100% - var\(--story-safe-top\) - var\(--story-safe-bottom\)\)/);
    assert.match(storyCss, /\.sow-story__main \{ display: contents; \}/);
    assert.match(storyCss, /\.sow-story\.has-portrait \.sow-story__dialog \{ grid-template-columns: clamp\(130px, 22cqw, 220px\) minmax\(0, 1fr\); \}/);
    assert.match(storyCss, /\.sow-story\.has-portrait \.sow-story__actions \{ grid-column: 2; \}/);
    assert.match(campaignView, /objective\.hidden = modal \|\| context\.hideObjective === true \|\| compactViewport/);
    assert.doesNotMatch(storyCss, /#sow-story\[data-compact="true"\] \.sow-story__objective/);
    assert.match(hudCss, /@media \(hover: none\) and \(pointer: coarse\) \{[\s\S]*?#sow-hud\[data-compact="true"\] \.sow-hud__status-right \{ flex-direction: row/);
    assert.match(hudCss, /#sow-hud\[data-compact="true"\] \.sow-hud__right-rail \{[^}]*grid-template-columns: repeat\(2, 44px\)/);
    assert.match(hudCss, /#sow-hud\[data-compact="true"\] \.sow-hud__dock-actions \{[^}]*overflow-x: auto;[^}]*overscroll-behavior-inline: contain/);
    assert.match(hud, /function syncCompactHudBounds\(\)/);
    assert.match(hud, /function compactHudDensity\(width, height\)/);
    assert.match(hud, /width \/ 960, height \/ 720/);
    assert.match(hud, /--sow-hud-layout-top-reserve/);
    assert.match(hud, /--sow-hud-layout-action-top/);
    assert.match(hud, /--sow-hud-layout-bottom-edge/);
    assert.doesNotMatch(campaignView, /objectiveToggle|detailsExpanded|data-story-objective-toggle/);
    assert.match(storyCss, /\.sow-story__objective \{[^}]*pointer-events: none/);
    assert.doesNotMatch(storyCss, /\.sow-story__details-toggle|data-details-expanded/);
    assert.match(storyCss, /\.sow-story\.is-chapter \.sow-story__dialog \{ bottom: 50%; transform: translateY\(50%\)/);
    assert.match(campaignEditorHtml, /href="\/shell\/main_menu\.tutorial\.css"/);
    assert.match(campaignEditorHtml, /src="\/shell\/sow-campaign-view\.js"/);
    assert.match(campaignEditor, /window\.SOWCampaignView\.mount/);
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


test("campaign dialogue announces each new spoken line to screen readers", () => {
    assert.match(campaignView, /class="sow-story__body"[^>]*aria-live="polite" aria-atomic="true"/);
});


test("campaign scene transitions are brief, shared, and disabled for reduced motion", () => {
    assert.match(campaignView, /const beatChanged = Boolean\(lastBeatKey && beatKey !== lastBeatKey\)/);
    assert.match(campaignView, /matchMedia\("\(prefers-reduced-motion: reduce\)"\)/);
    assert.match(campaignView, /if \(beatChanged && !reducedMotion\)/);
    assert.match(campaignView, /\[speaker, title, body, image, choices\]\.forEach/);
    assert.match(campaignView, /duration: 180, easing: "cubic-bezier\(\.2,\.7,\.2,1\)"/);
});


test("objective completion has no artificial wait or delayed preview timer", () => {
    assert.doesNotMatch(campaignEngine, /readyAt|reactionOpenedAt|now - readyAt/);
    assert.doesNotMatch(campaignView, /is-ready|model\.ready/);
    assert.doesNotMatch(storyCss, /story-objective-complete|is-ready/);
    assert.doesNotMatch(campaignEditor, /setTimeout\(function \(\) \{ if \(state\.machine\) paintPreview\(\); \}, 840\)/);
    assert.match(storyCss, /\.sow-story\.is-reduced [^{]*\{ animation: none !important/);
    assert.match(storyCss, /\.sow-story__meter progress \{[^}]*height: var\(--sow-tutorial-progress-height\)[^}]*border-radius: 999px/);
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
    assert.match(campaignEditor, /Add hand guide/);
    assert.match(campaignEditor, /Remove hand guide/);
    assert.match(campaignEditor, /tabindex: "0", role: "group"/);
    assert.match(campaignEditor, /direction: rtlLanguages\.has\(state\.previewLanguage\.toLowerCase\(\)\.split\("-"\)\[0\]\) \? "rtl" : "ltr"/);
    assert.match(campaignView, /root\.dir = context\.direction/);
    assert.match(tutorial, /direction: document\.documentElement\.dir/);
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
    assert.match(campaignEditor, /Within this control/);
    assert.match(campaignEditor, /if \(guide\.kind === "world"\) end = previewWorldMarker\(frame, guide\.to, step\)/);
});


test("campaign episode 1 stays locked so only the tutorial can be replayed", () => {
    const unlockStart = campaignModule.indexOf("pub fn is_unlocked(");
    const unlock = campaignModule.slice(unlockStart, campaignModule.indexOf("pub fn advisor(", unlockStart));
    assert.ok(unlockStart >= 0, "is_unlocked must exist");
    assert.match(campaignPolicySource, /pub const PUBLISHED_EPISODES: &\[&str\] = &\["boudica"\];/,
        "the shared registry keeps all Six Sky episodes unpublished");
    assert.match(unlock, /sow_data::campaign::episode_is_published\(episode\.id\)/,
        "campaign availability follows the shared published-episode registry");
    assert.match(unlock, /episode\s*\.\s*prerequisite\s*\.\s*is_none_or\s*\(\s*\|required\|\s*progress\.completed_episodes\.contains\(required\)\s*\)/,
        "later campaign episodes follow the shared prerequisite metadata");
    // A locked episode renders the same disabled card as the rest of the chain.
    assert.match(lobbiesSource, /: "<button class='sow-menu__secondary sow-campaign__play' type='button' disabled>"/);
    // "Saga complete" may only appear when every episode is really finished.
    assert.match(lobbiesSource, /episodes\.every\(function \(episode\) \{ return episode && episode\.completed; \}\)/);
});


test("campaign menu-guide preview opens Campaign before exposing the replay target", () => {
    assert.match(campaignEditorHtml, /data-preview-menu-screen="home"[\s\S]*?data-command="open_campaign"/);
    assert.match(campaignEditorHtml, /data-preview-menu-screen="campaign"[\s\S]*?data-preview-menu-back[\s\S]*?data-preview-episode-title/);
    assert.match(campaignEditor, /function setPreviewMenuScreen\(screen\)[\s\S]*?panel\.hidden = panel\.dataset\.previewMenuScreen !== screen/);
    assert.match(campaignEditor, /closest\('\[data-command="open_campaign"\]'\)\) setPreviewMenuScreen\("campaign"\)/);
    assert.match(campaignEditor, /replayButton\.dataset\.episodeId = episodeId/);
    assert.match(campaignEditor, /guideTarget === "campaign_replay"[\s\S]*?open Campaign in the preview to reveal Replay/);
    assert.match(campaignEditor, /resolveUiTarget\("campaign_replay", document, episodeId\)/);
    assert.match(lobbiesSource, /data-command='close_campaign'/);
});


test("campaign replay hand resolves the selected episode card, not always Boudica", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const boudica = { dataset: { episodeId: "boudica" } };
    const sixSky = { dataset: { episodeId: "six_sky_ep2" } };
    const root = { querySelectorAll: () => [boudica, sixSky] };
    assert.equal(campaign.resolveUiTarget("campaign_replay", root, "six_sky_ep2"), sixSky);
    assert.equal(campaign.resolveUiTarget("campaign_replay", root, "unknown_episode"), null);
    assert.equal(campaign.resolveUiTarget("campaign_replay", root), null);
    assert.match(tutorial, /resolveUiTarget\(guide\.target, document, runtime\.episodeId, guideFacts\)/);
    assert.doesNotMatch(campaignEngine, /data-episode-id="boudica"/);
});


test("tutorial hand selectors stay tied to controls rendered by the game", () => {
    const targetBlock = campaignEngine.match(/const UI_TARGETS = \{([\s\S]*?)\n    \};/);
    assert.ok(targetBlock);
    const sources = [shellSource, lobbiesSource, coreSource, hud].join("\n");
    for (const [, selector] of targetBlock[1].matchAll(/:\s*'([^']+)'/g)) {
        for (const [, attribute, value] of selector.matchAll(/\[([a-z-]+)="([^"]+)"\]/g)) {
            if (attribute === "data-map-action") {
                assert.match(hud, /dataset\.mapAction = action/);
                assert.ok(hud.includes(value), `No game map action ${value}`);
            } else if (attribute === "data-map-group") {
                assert.match(hud, /dataset\.mapGroup = group/);
                assert.ok(hud.includes(value), `No game map group ${value}`);
            } else if (attribute === "data-episode-id") {
                assert.match(lobbiesSource, /data-episode-id=.*episode\.id/);
                assert.match(campaignModule, new RegExp('Boudica => "' + value + '"'));
            } else {
                assert.ok(sources.includes(`${attribute}='${value}'`) || sources.includes(`${attribute}="${value}"`) || sources.includes(value), `No game UI target ${attribute}=${value}`);
            }
        }
        if (selector === "#sow-hud-slider") assert.match(hud, /id="sow-hud-slider"/);
    }
    assert.match(hud, /data-map-back/);
});


test("tutorial X is not wired to leave match and menu exit clears runtime", () => {
    assert.match(tutorial, /data-command=.*prompt_surrender/);
    assert.match(tutorial, /function update\(hud\) \{\s*if \(!runtime\.machine \|\| runtime\.modalOpen \|\|/);
    assert.match(tutorial, /runtime\.machine\.setPaused\(open\)/);
    assert.match(tutorial, /onDismiss: skipMenuGuide/);
    assert.match(tutorial, /SOW_tutorial_menu_state_update/);
    assert.match(tutorial, /function reset\(\)/);
    assert.match(hud, /id="sow-hud-surrender-modal"/);
    assert.match(hudCss, /\.sow-campaign-error/);
    assert.doesNotMatch(hudCss, /sow-hud__tutorial-overlay|sow-hud__tutorial-hand/);
});


test("campaign building access is enforced by the shared step unlock policy", () => {
    assert.doesNotMatch(coreCost, /structure_kind_enabled/);
    assert.match(buildingsIntent, /campaign_allows_building\(kind, 1\)/);
    assert.match(buildingsIntent, /campaign_allows_building\(building\.kind, target_level\)/);
    assert.match(mapClick, /campaign_building_max_level\(kind\) >= 1/);
    assert.match(webMenu, /"reason_key": item\.reason_key/);
    assert.doesNotMatch(webMenu, /factory_unlocked/);
    assert.match(hud, /if \(item\.reason_key\) \{\s*var reason = SOW_t\(item\.reason_key, item\.reason_troops == null/);
    assert.doesNotMatch(hud, /factoryLocked/);
    assert.match(hud, /button\.hidden = !available/);
    assert.match(hud, /button\.disabled = disabled/);
    assert.match(campaignEditor, /Campaign availability/);
    assert.match(campaignEditor, /STRUCTURE_LEVEL_LIMITS/);
    assert.match(campaignEditor, /Available when this step begins/);
    assert.doesNotMatch(campaignEditor, /Buildings available|Unlock buildings after/);
    assert.doesNotMatch(hud, /campaignActionLockReason/);
    assert.match(hud, /function campaignAllowsBuilding\(unlocks, kind, level\)\s*\{\s*return unlocks === null \|\|/);
    assert.match(hud, /item\.reason_key !== "lobbies\.locked"/);
    assert.match(campaignEditor, /CAMPAIGN_ACTION_BUILDING_REQUIREMENTS/);
    assert.match(mapClick, /reason_level: Option<u8>/);
    assert.match(mapClick, /reason_seconds: Option<u32>/);
    assert.match(mapClick, /MapMenuAction::Nuke => \(Some\(self\.sim\.config\.nuke_cost\), None\)/);
    assert.match(webMenu, /"reason_level": item\.reason_level/);
    assert.match(webMenu, /"reason_seconds": item\.reason_seconds/);
    assert.match(webMenu, /campaign_session_id == self\.ui\.campaign_session_id[\s\S]*?engine\.state\.config\.tutorial/);
    assert.match(matchStartSource, /campaign_session_id = self\.ui\.campaign_session_id\.wrapping_add\(1\)\.max\(1\)/);
    const botBuildingSource = fs.readFileSync(path.join(shell, "../../sow-core/src/intent/nation/build.rs"), "utf8");
    const botCombatSource = fs.readFileSync(path.join(shell, "../../sow-core/src/intent/nation/combat.rs"), "utf8");
    const botEconomySource = fs.readFileSync(path.join(shell, "../../sow-core/src/execution/income.rs"), "utf8");
    assert.match(botBuildingSource, /if !self\.campaign_allows_building\(kind, 1\) \{\s*continue;\s*\}/);
    assert.match(botCombatSource, /if !self\.campaign_allows_action\(crate::campaign::CampaignAction::Nuke\) \{\s*return;\s*\}/);
    assert.match(botEconomySource, /campaign_allows_building\(crate::game::BuildingKind::City, 1\)/);
    assert.match(hud, /button\.setAttribute\("aria-disabled", String\(disabled\)\)/);
    assert.match(hud, /if \(mapButton\.disabled\) return/);
    assert.doesNotMatch(hud, /mapButton\.disabled \|\| mapButton\.getAttribute\("aria-disabled"\) === "true"/);
    assert.match(hudCss, /\.sow-hud__building-btn\[aria-disabled="true"\]/);
    assert.match(buildingsIntent, /factory_upgrades_without_a_city_level_requirement/);
});


test("campaign hides unavailable actions instead of rendering lock-label UI", () => {
    assert.match(mapClick, /campaign_fleet\s*&&\s*target\.is_land/);
    assert.match(mapClick, /MapMenuAction::Fleet => self[\s\S]*?CampaignAction::TransportFleet/);
    assert.doesNotMatch(mapClick, /campaign_locked\.then_some\("hud\.campaign_unlock_required"\)/);
    assert.match(mapClick, /Some\(\("hud\.fleet_capacity_full", None\)\)/);
    assert.match(mapClick, /fleet_route_check\([\s\S]*?FleetRouteCheck::Access\)/);
    assert.match(webMenu, /"reason_troops": item\.reason_troops/);
    assert.doesNotMatch(hud, /mapDisabledSector|var radialCount = 4/);
    assert.match(hud, /fleetTrade\.hidden = hideTrade/);
    assert.match(hud, /fleetWarship\.hidden = hideWarship/);
    assert.match(hud, /fleetNuke\.hidden = hideNuke/);
    assert.match(hud, /buildingCardNext\.hidden = true/);
    assert.doesNotMatch(hud, /campaign_unlock_required/);
    assert.doesNotMatch(hudCss, /\.sow-hud__map-sector \.sow-hud__map-action-reason/);
    assert.doesNotMatch(mapClick, /fleet_no_port|NoPort|completed port/);
});


test("tutorial hand follows the selected campaign target", () => {
    assert.match(webMenu, /WebMenuCommand::SetTutorialMarker \{ player_id \} => \{[\s\S]*?tutorial_marker_player_id = player_id/);
    assert.match(webMenu, /tutorial_target_action_tile\([\s\S]*?target_owner/);
    assert.match(tutorial, /send\("set_tutorial_marker", \{ player_id: markerId \}\)/);
    assert.match(tutorial, /guide\.target === "target_action"[\s\S]*?step\.trigger\.type === "defeated"[\s\S]*?tutorial\.target_nameplate/);
    assert.match(webMenu, /let target_nameplate = nameplate_position[\s\S]*tutorial_screen_anchor\(point, viewport_w, viewport_h\)[\s\S]*"target_nameplate": target_nameplate/);
    assert.match(tutorial, /Array\.isArray\(step\.trigger\.targets\)/);
    assert.match(campaignView, /const hasMapTarget = !\(step\.trigger && \["camera_target", "hover", "ui"\]\.includes\(step\.trigger\.type\)\)/);
    assert.match(campaignEditor, /Array\.isArray\(step\.trigger && step\.trigger\.targets\)/);
    assert.match(campaignView, /data-tutorial-hand/);
    assert.doesNotMatch(tutorial, /tutorial\.markers|data-tutorial-marker|SOW_tutorial_marker_update/);
    assert.doesNotMatch(hudCss, /sow-hud__tutorial-marker/);
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
    assert.match(cameraFrameUiSource, /publish_tutorial_camera_anchor_frame\(self\);\s*crate::web_menu::publish_state\(self\)/);

    const hudKeyStart = webMenu.indexOf("fn hud_publish_key");
    const hudKeyEnd = webMenu.indexOf("\n#[cfg", hudKeyStart);
    const hudKey = webMenu.slice(hudKeyStart, hudKeyEnd);
    assert.match(hudKey, /tutorial_camera_viewport: tutorial_active\.then_some\(\([\s\S]*?camera_x\.to_bits\(\)[\s\S]*?camera_y\.to_bits\(\)/);
    assert.match(webMenu, /LAST_HUD_KEY\.with\([\s\S]*?last\.as_ref\(\) == Some\(&hud_key\)/);
    const guideStart = webMenu.indexOf("fn tutorial_guide_tiles(");
    const guideEnd = webMenu.indexOf("\nfn tutorial_payload(", guideStart);
    const guide = webMenu.slice(guideStart, guideEnd);
    assert.match(guide, /guide_build_site_candidates[\s\S]*tutorial_visible_build_site_candidate\([\s\S]*tutorial_project_tile\([\s\S]*&app\.input[\s\S]*tutorial_screen_point_visible/);
    assert.match(tutorial, /var targetFactionId = targetStep\.type === "choice" && targetStep\.marker[\s\S]*?targetStep\.guide && targetStep\.guide\.kind[\s\S]*?send\("focus_world"/);
    const anchorFunction = tutorial.slice(tutorial.indexOf("function anchorFor"), tutorial.indexOf("function syncTransferGuide"));
    assert.match(anchorFunction, /factionAction[\s\S]*?tutorial\.target_nameplate[\s\S]*?cameraTracked: true/);
    const uiAnchorBranch = anchorFunction.slice(anchorFunction.indexOf("var source = window.SOWCampaign.resolveUiTarget"));
    assert.doesNotMatch(uiAnchorBranch, /cameraTracked/);
    assert.match(tutorial, /SOW_tutorial_camera_anchor_update = function \(x, y\)[\s\S]*?runtime\.view\.updateCameraAnchor/);
    assert.match(campaignView, /cameraTracking = anchor\.cameraTracked === true/);
    assert.match(campaignView, /function updateCameraAnchor\(x, y\)[\s\S]*?if \(!cameraTracking \|\| root\.hidden/);
    assert.match(campaignView, /const guideVisible = Boolean\(!modal && !statusOnly && step\.guide/);
    assert.match(campaignView, /gesture\.hidden = !guideVisible && !statusDrawable/);
    assert.match(storyCss, /\.sow-story__gesture\.is-camera-following \{ transition: none; \}/);
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
    assert.match(english, /campaign_boudica_boudica_first_expansion_desktop_action = "Click to expand"/);
    assert.match(english, /campaign_boudica_boudica_first_expansion_mobile_action = "Tap to expand"/);
    assert.match(english, /campaign_boudica_boudica_first_expansion_continue_action = "Keep expanding"/);
    assert.match(english, /campaign_boudica_boudica_camera_home_desktop_action = "Click Home"/);
    assert.match(english, /campaign_boudica_boudica_camera_home_mobile_action = "Tap Home"/);
    assert.match(campaignView, /Boolean\(context\.gestureLabel\)/);
    assert.match(campaignView, /if \(labeledGesture\) setText\(gestureCopy,[^\n]*context\.gestureLabel/);
    assert.match(campaignView, /showGestureText = !step\.guide \|\| step\.guide\.show_label !== false/);

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
    assert.match(campaignView, /const homeGuide = Boolean\(step\.guide[\s\S]*step\.trigger\.action === "hud_center_camera"\)/);
    assert.match(campaignView, /const animateHomeGuide = homeGuide && !reducedMotion/);
    assert.match(campaignView, /const originX = animateHomeGuide \? root\.clientWidth \* 0\.5 : anchor\.x/);
    assert.match(campaignView, /const originY = animateHomeGuide \? root\.clientHeight \* 0\.5 : anchor\.y/);
    assert.match(campaignView, /const homeMotionPending = animateHomeGuide && homeGuideAnimationStep !== step.id/);
    assert.match(campaignView, /const follow = !homeGuide && guideWasVisible && !wasHidden/);
    assert.match(campaignView, /homeGuide \? anchor\.x - originX/);
    assert.match(campaignView, /homeGuide \? anchor\.y - originY/);
    assert.match(campaignView, /spotlight\.classList\.toggle\("is-dimmed", Boolean\(guideVisible && \(anchor\.dimOutside \|\| step\.guide\.kind === "ui"\)\)\)/);
    assert.match(campaignView, /Number\.isFinite\(anchor\.spotlightWidth\) \? anchor\.spotlightWidth : anchor\.width/);
    assert.match(campaignView, /Number\.isFinite\(anchor\.spotlightX\) \? anchor\.spotlightX : anchor\.x/);
    assert.match(storyCss, /\.sow-story__spotlight\.is-dimmed \{[^}]*9999px/);
    assert.match(storyCss, /\.sow-story__spotlight \{[^}]*pointer-events:\s*none/);
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
    assert.deepEqual([...categories].sort(), ["attack", "build", "contact", "expand", "home", "select", "send", "upgrade", "upgrade_progress"].sort());
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
    assert.match(campaignEditor, /window\.SOWCampaign\.guideAction\(model\.step\)/);
    assert.match(campaignEditor, /gestureLabel:\s*gestureLabel/);
    assert.match(campaignView, /const guideVisible = Boolean\(!modal && !statusOnly && step\.guide/);
    assert.match(campaignView, /gesture\.hidden = !guideVisible && !statusDrawable/);
});


test("Boudica UI guides spotlight the requested controls and Legion IX attack slider", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const campaignSource = fs.readFileSync(path.join(shell, "sow-campaign.js"), "utf8");
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    const requested = [
        "boudica_transfer_choose", "boudica_transfer_send", "boudica_choose_city", "boudica_structure_upgrade"
    ];
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
    assert.equal(intro.next, ratioStepId);
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
    assert.match(tutorial, /attack_ratio: Number\(hud\.attack_ratio\)/);
    assert.match(tutorial, /resolveUiRangeGuideAnchor\(source, Number\(source\.value\), ratioTarget\)/);
    assert.match(campaignEditor, /resolveUiRangeGuideAnchor\(target, Number\(target\.value\), rangeTarget\)/);
    assert.match(campaignSource, /trigger\.action === "attack_ratio"[\s\S]*current = Number\(facts\.attack_ratio \|\| 0\)[\s\S]*target = Number\(trigger\.value \|\| 1\)/);
    assert.match(tutorial, /key === "attack_ratio" && currentStep && currentStep\.trigger[\s\S]*currentStep\.trigger\.action === key\) return/);
    const english = fs.readFileSync(path.join(shell, "../../sow-i18n/strings/en/web.toml"), "utf8");
    assert.match(english, /campaign_boudica_boudica_ninth_set_attack_ratio_title = "Commit the Full Host"/);
    assert.match(english, /campaign_boudica_boudica_ninth_set_attack_ratio_body = "[^"]*100%/);
    assert.match(english, /campaign_boudica_boudica_ninth_strike_title = "No Escape for the Ninth"/);
    assert.match(english, /campaign_boudica_boudica_ninth_strike_body = "The Ninth is Rome’s answer to Camulodunum\.[^"]*leave Cerialis no chance to regroup\./);
    assert.match(campaignView, /anchor\.dimOutside \|\| step\.guide\.kind === "ui"/);
    assert.match(campaignView, /root\.dataset\.guideTarget = "attack_ratio"/);
    assert.doesNotMatch(storyCss, /data-guide-target="attack_ratio"\] \.sow-story__objective/);
    assert.match(storyCss, /data-guide-target="attack_ratio"\]\[data-guide-path="true"\] \.sow-story__hand \{ animation: story-attack-ratio-path/);
    assert.match(storyCss, /@keyframes story-attack-ratio-path[\s\S]*opacity: 0[\s\S]*var\(--guide-dy\)/);
    assert.match(storyCss, /data-guide-target="attack_ratio"\] \.sow-story__gesture-label \{ left: 68px; right: auto/);
    assert.match(storyCss, /#sow-story\[dir="rtl"\].*left: auto; right: 68px/);
    assert.match(campaignView, /spotlightWidth = Number\.isFinite\(anchor\.spotlightWidth\) \? anchor\.spotlightWidth : anchor\.width/);
    assert.match(tutorial, /step\.id === "boudica_transfer_send"[\s\S]*source\.closest\("#sow-hud-transfer"\)/);
    assert.match(campaignEditor, /step\.id === "boudica_transfer_send"[\s\S]*target\.closest\("#sow-hud-transfer"\)/);
    assert.match(campaignEditor, /model\.step\.id === "boudica_transfer_send"\) transferPanel\.hidden = false[\s\S]*previousStep === "boudica_transfer_send"\) transferPanel\.hidden = true/);
    assert.match(campaignEditor, /anchorRect = guide\.kind === "ui" \? \$\("#previewRoot"\)\.getBoundingClientRect\(\) : frameRect/);
    assert.match(campaignEditor, /if \(Number\.isFinite\(anchor\.spotlightX\)\) anchor\.spotlightX -= anchorRect\.left/);
    assert.match(campaignEditor, /if \(Number\.isFinite\(anchor\.spotlightY\)\) anchor\.spotlightY -= anchorRect\.top/);
});


test("Boudica dialogue follows the next mission and source copy exists in English", () => {
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    const catalogValue = (catalog, key) => {
        const shortKey = key.replace(/^tutorial\./, "");
        const match = catalog.match(new RegExp("^" + shortKey + " = (\"(?:\\\\.|[^\"])*\")$", "m"));
        assert.ok(match, "missing catalog key " + key);
        return JSON.parse(match[1]);
    };
    const step = id => definition.steps.find(item => item.id === id);
    assert.equal(step("boudica_outpost_colonia_veterans_scene").next, "boudica_outpost_tax_collectors");
    assert.equal(step("boudica_outpost_tax_collectors_scene").next, "boudica_outpost_roman_supply_depot");
    assert.equal(step("boudica_outpost_roman_supply_depot_scene").next, "boudica_trinovantes_contact");
    assert.match(catalogValue(webCatalogEn, step("boudica_outpost_colonia_veterans_scene").body_key), /tax collectors/i);
    assert.match(catalogValue(webCatalogEn, step("boudica_outpost_tax_collectors_scene").body_key), /supply depot/i);
    assert.match(catalogValue(webCatalogEn, step("boudica_outpost_roman_supply_depot_scene").body_key), /Trinovantes/i);
    assert.equal(step("boudica_last_stand").trigger.comparison, "lte");
    assert.equal(step("boudica_last_stand").trigger.value, 5000);
    assert.match(catalogValue(webCatalogEn, step("boudica_last_stand").hint_key), /5,000/);
    assert.equal(step("boudica_poison_end").speaker, "council");
    assert.equal(step("boudica_end").speaker, "council");
    assert.doesNotMatch(catalogValue(webCatalogEn, step("boudica_poison_end").body_key), /Boudica chooses/i);
    assert.equal(definition.reactions.find(item => item.id === "contact_snettisham").choices[1].relation, "enemy");
    assert.equal(definition.reactions.find(item => item.id === "contact_snettisham").choices[1].gold_cost, 0);
    assert.equal(definition.reactions.find(item => item.id === "contact_thetford").choices[1].relation, "enemy");
    assert.equal(definition.reactions.find(item => item.id === "contact_thetford").choices[1].gold_cost, 0);
    assert.equal(definition.reactions.find(item => item.id === "neutral_contact_terms").choices[0].gold_cost, 200);
});


test("tutorial hand and ripple fade in at screen center and glide together to the dimmed Home button", () => {
    const hand = fs.readFileSync(path.join(shell, "../../assets/gameplay/icons/tutorial_hand.webp"));
    assert.equal(hand.subarray(0, 4).toString(), "RIFF");
    assert.equal(hand.subarray(8, 12).toString(), "WEBP");
    assert.match(campaignView, /options\.asset\("gameplay\/icons\/tutorial_hand\.webp"\)/);
    assert.match(storyCss, /\.sow-story__hand \{[^}]*left: 0; top: 0; transform-origin: 0 0/);
    assert.match(storyCss, /\.sow-story__hand \{[^}]*z-index: 1/);
    assert.match(tutorial, /if \(\["hover", "zoom_in_complete"\]\.includes\(step\.trigger\.type\)\)[\s\S]*tutorial\.nameplate/);
    assert.match(storyCss, /\.sow-story__gesture\[data-gesture="drag"\] \.sow-story__hand \{ animation: story-hover/);
    assert.match(storyCss, /@keyframes story-hover \{ 0%, 100% \{ transform: translateY\(-8px\)/);
    assert.match(storyCss, /data-guide-path="true"\] \.sow-story__hand \{ animation: story-guide-path/);
    assert.match(storyCss, /@keyframes story-guide-path/);
    assert.doesNotMatch(campaignView, /setHomeGuideCurve|home-guide-p(?:25|50|75)/);
    assert.match(campaignView, /gesture\.animate\(\[/);
    assert.match(campaignView, /\{ transform: guideTransform\(originX, originY\), opacity: 0 \}/);
    assert.match(campaignView, /\{ transform: guideTransform\(originX, originY\), opacity: 1, offset: 0\.16 \}/);
    assert.match(campaignView, /\{ transform: guideTransform\(anchor\.x, anchor\.y\), opacity: 1 \}/);
    assert.match(campaignView, /\{ duration: 1100, easing: "cubic-bezier/);
    assert.match(campaignView, /homeGuideAnimationStep = step.id/);
    assert.match(storyCss, /data-guide-target="hud_center_camera"] \.sow-story__hand { animation: story-hover 1.5s ease-in-out infinite; }/);
    assert.match(storyCss, /data-guide-target="hud_center_camera"\] \.sow-story__gesture-label \{ left: 0; top: 61px; \}/);
    assert.doesNotMatch(storyCss, /story-home-arrive|story-home-guide|story-home-ripple/);
    assert.match(campaignView, /spotlight\.classList\.toggle\("is-dimmed", Boolean\(guideVisible && \(anchor\.dimOutside \|\| step\.guide\.kind === "ui"\)\)\)/);
    assert.match(campaignView, /gesture\.hidden = !guideVisible && !statusDrawable/);
    assert.match(campaignEditor, /window\.SOWCampaignView\.mount/);
    assert.doesNotMatch(storyCss, /data-gesture\[\^="zoom_"\]\s+\.sow-story__hand/);
    assert.doesNotMatch(storyCss, /@keyframes story-drag/);
    assert.doesNotMatch(tutorial, /step\.trigger && step\.trigger\.type === "territory" && step\.guide\.target === "expand"/);
    assert.doesNotMatch(tutorial, /step\.id === "boudica_first_victory"[\s\S]*attacks_by_faction_id/);
});




test("construction guidance points to confirmation without advancing on a preview", () => {
    const source = fs.readFileSync(path.join(__dirname, "main_menu.tutorial.js"), "utf8");
    const begin = source.indexOf("    function placementGuideView(");
    const end = source.indexOf("    function anchorFor(", begin);
    const adapt = vm.runInNewContext(source.slice(begin, end) + "\nplacementGuideView;");
    const model = { step: { id: "build_city", paused_action: "build_until_started", trigger: { type: "city", value: 1 }, guide: { kind: "world", target: "build_site", gesture: "tap" } } };
    assert.equal(adapt(model, {}), model);
    assert.equal(adapt(model, { building_placement: { can_confirm: false } }), model);
    const display = adapt(model, { building_placement: { can_confirm: true } });
    assert.equal(display.step.guide.target, "confirm_building_placement");
    assert.equal(display.step.trigger, model.step.trigger);
    assert.equal(model.step.guide.target, "build_site");
});
