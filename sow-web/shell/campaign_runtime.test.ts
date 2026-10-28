import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";
import { vm } from "./typescript-test-support.ts";
const shell = __dirname;
import { bundleTs, loadTypeScriptModule } from "./typescript-test-support.ts";
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.ts"), "utf8");
const tutorialBundle =
    '(function () { if (typeof window === "undefined") return; const providedView = window.SOWCampaignView;\n' +
    bundleTs(path.join(shell, "sow-campaign-view.ts")) +
    "\nif (providedView) Object.assign(window.SOWCampaignView, providedView); })();\n" +
    bundleTs(path.join(shell, "main_menu.tutorial.ts"));
const campaignEditor = fs.readFileSync(
    path.join(shell, "../../sow-tools/editors/campaign-editor/editor.ts"),
    "utf8",
);
const campaignModule = fs.readFileSync(path.join(shell, "../../sow-client/src/campaign/mod.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const nameplates = fs.readFileSync(
    path.join(shell, "../../sow-client/src/render/world/nameplates.rs"),
    "utf8",
);

test("emote conversation waits for visible delivery, preserves reading time and clears before the reply", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    let now = 0,
        delivered = false;
    const spoken = [],
        cleared = [];
    const machine = campaign.create(
        {
            entry: "talk",
            speakers: { tribe: { faction: "tribe" }, queen: {} },
            steps: [
                {
                    id: "talk",
                    type: "scene",
                    exchange: [
                        { speaker: "tribe", emote: "greet", emote_text_key: "first" },
                        { speaker: "queen", emote: "greet", emote_text_key: "reply" },
                    ],
                    next: "done",
                },
                { id: "done", type: "end" },
            ],
        },
        undefined,
        undefined,
        () => now,
        {
            onExchangeLine: (line, token) => {
                if (!delivered) return false;
                spoken.push([line.emote_text_key, token]);
                return true;
            },
            onExchangeClear: (line, token) => cleared.push([line.emote_text_key, token]),
        },
    );
    assert.equal(machine.update({}, {}).exchange_phase, "prepare");
    now = 5000;
    assert.equal(machine.update({}, {}).exchange_phase, "prepare", "unseen time cannot consume the line");
    assert.equal(spoken.length, 0);
    delivered = true;
    assert.equal(machine.update({}, {}).paused, true, "the world pauses while characters speak");
    now = 7999;
    assert.equal(machine.update({}, {}).exchange.emote_text_key, "first");
    assert.equal(machine.advance(null, "talk"), false, "clicks cannot skip an emote");
    now = 8000;
    assert.equal(machine.update({}, {}).exchange_phase, "gap");
    assert.deepEqual(
        cleared.map((item) => item[0]),
        ["first"],
    );
    now = 8249;
    machine.update({}, {});
    assert.equal(spoken.length, 1);
    now = 8250;
    assert.equal(machine.update({}, {}).exchange.emote_text_key, "reply");
    assert.equal(spoken.length, 2);
    const interruptedToken = spoken[1][1];
    machine.setPaused(true);
    now = 60000;
    machine.update({}, {});
    assert.equal(spoken.length, 2, "a hidden tab or open menu does not play other lines");
    machine.setPaused(false);
    machine.update({}, {});
    assert.deepEqual(
        spoken.map((item) => item[0]),
        ["first", "reply", "reply"],
    );
    assert.notEqual(
        spoken[2][1],
        interruptedToken,
        "resuming creates a new delivery identity for only this line",
    );
    machine.dispose();
    machine.update({}, {});
    assert.equal(spoken.length, 3);
    assert.equal(cleared.at(-1)[1], spoken[2][1]);
});

test("Trinovantes requires a real request, confirmed alliance and positive targeted transfer before its assault", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const definition = JSON.parse(
        fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json")),
    );
    const roster = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.json")));
    const tribe = roster.factions.find((faction) => faction.id === "trinovantes");
    assert.equal(tribe.relation, "neutral");
    assert.equal(tribe.team, "Blue", "the existing assault team remains intact");
    assert.equal(tribe.can_request_alliance, false, "the player initiates this lesson");
    const facts = {
        contact_faction_ids: ["trinovantes"],
        alliance_request_faction_ids: [],
        alliance_faction_ids: [],
        resource_transfers_by_recipient_faction_id: {},
        resources_available: 1,
        elapsed_seconds: 0,
    };
    const ui = {};
    let now = 0;
    const spoken = [];
    const machine = campaign.create(definition, "boudica_trinovantes_contact", roster, () => now, {
        onExchangeLine: (line) => spoken.push(line.emote_text_key),
    });
    const until = (id) => {
        for (let n = 0; n < 300; n++, now += 100) {
            const model = machine.update(facts, ui);
            if (model.step.id === id && !model.exchange) return model;
        }
        assert.fail("did not reach " + id + "; current " + machine.view().step.id);
    };
    until("boudica_alliance_target");
    ui.map_menu_open = 1;
    until("boudica_alliance_request");
    ui.map_alliance = 1;
    facts.alliance_request_faction_ids = ["another_tribe"];
    now += 10000;
    assert.equal(
        machine.update(facts, ui).step.id,
        "boudica_alliance_request",
        "a click or another faction's request is not admission",
    );
    facts.alliance_request_faction_ids = ["trinovantes"];
    until("boudica_alliance_wait");
    assert.equal(machine.view().paused, true);
    now += 10000;
    assert.equal(
        machine.update(facts, ui).step.id,
        "boudica_alliance_wait",
        "sending a request is not an alliance",
    );
    facts.alliance_request_faction_ids = [];
    facts.alliance_faction_ids = ["trinovantes"];
    facts.resources_available = 0;
    until("boudica_supplies_available");
    assert.equal(machine.view().paused, false, "normal income may run when no supplies can be sent");
    facts.resources_available = 1;
    until("boudica_transfer_target");
    ui.map_menu_open = 2;
    until("boudica_transfer_choose");
    ui.map_transfer = 1;
    until("boudica_transfer_send");
    facts.resource_transfers_by_recipient_faction_id = {
        another_tribe: { total: 1 },
        trinovantes: { total: 0 },
    };
    assert.equal(machine.update(facts, ui).step.id, "boudica_transfer_send");
    facts.resource_transfers_by_recipient_faction_id.trinovantes.total = 1;
    const assault = until("boudica_trinovantes_assault");
    assert.deepEqual(assault.step.campaign_assault_on_enter, {
        attacker_team: "Blue",
        target: "camulodunum",
    });
    assert.equal(spoken.length, 10, "all ten lines play exactly once around the two mechanics");
    facts.elapsed_seconds = 1.2;
    assert.equal(machine.update(facts, ui).step.id, "boudica_camulodunum");
    assert.equal(machine.view().step.next, "boudica_ninth_legion_intro");
    const missing = campaign.create(definition, "boudica_trinovantes_contact", roster, () => now);
    const withoutTribe = { ...facts, eliminated_faction_ids: ["trinovantes"] };
    for (let n = 0; n < 50; n++, now += 100) missing.update(withoutTribe, {});
    assert.equal(missing.view().step.id, "boudica_camulodunum");
    assert.equal(
        missing.state.completed.includes("boudica_trinovantes_assault"),
        false,
        "an eliminated tribe never receives an assault order",
    );
});

test("the game adapter emits one localized speaker cue and waits for the native rendered acknowledgement", () => {
    const start = tutorial.indexOf("    function playExchangeLine(");
    const end = tutorial.indexOf("    function continueScene(", start);
    const commands = [];
    let acceptCommands = false;
    const player = { id: 2, is_alive: true, centroid_x: 5, centroid_y: 6 };
    const runtime = {
        definition: { speakers: { tribe: { faction: "trinovantes" } } },
        latestHud: {
            tutorial: {
                campaign_session_id: 11,
                marker_player_id: 2,
                target_nameplate: { offscreen: true },
                facts: {},
            },
        },
        machine: { view: () => ({ step: {} }) },
    };
    const context = {
        runtime,
        Date,
        send: (type, fields) => {
            if (!acceptCommands) return false;
            commands.push({ type, ...fields });
            return true;
        },
        tr: (key) => "translated:" + key,
        campaignPlayer: () => player,
        onMap: (p) => p.is_alive,
        cameraTargetAnchor: () => ({ offscreen: true }),
    };
    vm.runInNewContext(
        tutorial.slice(start, end) + "this.play = playExchangeLine; this.clear = clearExchangeLine;",
        context,
    );
    const line = { speaker: "tribe", emote: "greet", emote_text_key: "first" };
    assert.equal(context.play(line, "cue1"), false);
    assert.equal(
        commands.some((command) => command.type === "tutorial_emote"),
        false,
        "focus the speaker before emission",
    );
    runtime.latestHud.tutorial.target_nameplate.offscreen = false;
    assert.equal(
        context.play(line, "cue1"),
        "unavailable",
        "a missing command bridge falls back instead of waiting forever",
    );
    assert.equal(runtime.exchangeCue, null, "a missing native bridge does not leave a pending line");
    acceptCommands = true;
    assert.equal(context.play(line, "cue1"), false);
    const emitted = commands.filter((command) => command.type === "tutorial_emote");
    assert.equal(emitted.length, 1, "polling a pending render does not duplicate the line");
    assert.equal(emitted[0].faction_id, "trinovantes");
    assert.equal(emitted[0].message, "translated:first");
    assert.equal(emitted[0].campaign_session_id, 11);
    assert.equal(
        context.play(line, "cue1"),
        "pending",
        "an emitted line waits specifically for native render acknowledgement",
    );
    assert.equal(
        commands.filter((command) => command.type === "tutorial_emote").length,
        1,
        "waiting for acknowledgement does not resend the same line",
    );
    runtime.latestHud.tutorial.emote_token = emitted[0].token;
    assert.equal(context.play(line, "cue1"), true);
    context.clear(line, "stale");
    assert.equal(commands.filter((command) => command.type === "tutorial_emote").length, 1);
    context.clear(line, "cue1");
    assert.equal(commands.at(-1).kind, null);
    player.is_alive = false;
    assert.equal(context.play(line, "cue2"), "unavailable");
});

test("episode loading validates the resolved emote text before starting campaign play", async () => {
    const start = tutorial.indexOf("    function hasText(");
    const end = tutorial.indexOf("    function closeCampaignCinematic(", start);
    const definition = { episode_id: "boudica", steps: [] };
    const roster = { factions: [] };
    let validationOptions = null;
    const context = {
        runtime: { loading: {} },
        fetchJson: (path) => Promise.resolve(path.endsWith(".triggers.json") ? definition : roster),
        window: {
            SOW_hasText: () => true,
            SOW_t: () => "x".repeat(161),
            SOWCampaignView: {},
            SOWCampaign: {
                validate: (loadedDefinition, loadedRoster, options) => {
                    assert.equal(loadedDefinition, definition);
                    assert.equal(loadedRoster, roster);
                    validationOptions = options;
                    return { errors: [] };
                },
                replaceFactionStoryNames: (value) => value,
            },
        },
    };
    vm.runInNewContext(tutorial.slice(start, end) + "\nthis.loadEpisode = loadEpisode;", context);
    await context.loadEpisode("boudica");
    assert.equal(
        validationOptions.getText("tutorial.sample").length,
        161,
        "the load-time validator receives the text that the active locale will actually display",
    );
});

test("authored campaign emotes stay offline while multiplayer emotes keep their network path", () => {
    const campaignStart = webMenu.indexOf("WebMenuCommand::TutorialEmote { kind, pinned");
    const campaignEnd = webMenu.indexOf("WebMenuCommand::SetCampaignPreDefeatTargets", campaignStart);
    const multiplayerStart = webMenu.indexOf("WebMenuCommand::ExpressEmote {", campaignEnd);
    const multiplayerEnd = webMenu.indexOf("WebMenuCommand::ClearEmote", multiplayerStart);
    assert.ok(campaignStart >= 0 && campaignEnd > campaignStart);
    assert.ok(multiplayerStart > campaignEnd && multiplayerEnd > multiplayerStart);
    const campaignHandler = webMenu.slice(campaignStart, campaignEnd);
    const multiplayerHandler = webMenu.slice(multiplayerStart, multiplayerEnd);
    assert.match(campaignHandler, /!self\.ui\.tutorial_active \|\| !self\.net\.is_offline/);
    assert.match(multiplayerHandler, /GameplayIntent::ExpressEmote/);
    assert.match(multiplayerHandler, /self\.send_intent/);

    const renderStart = nameplates.indexOf("let prepared = system.campaign_emote");
    const renderEnd = nameplates.indexOf("let needed = prepared", renderStart);
    const bubbleSelection = nameplates.slice(renderStart, renderEnd);
    assert.match(
        bubbleSelection,
        /cue\.player_id == player\.id && cue\.event_id == emote\.event\.event_id && emote\.event\.scripted/,
    );
    assert.match(
        bubbleSelection,
        /unwrap_or\(&system\.prepared_emotes\[emote\.event\.kind as usize\]\)/,
        "ordinary multiplayer emotes retain their standard bubble",
    );
});

test("Boudica keeps open neutral clans, makes every Roman enemy, and orders the recorded pre-final targets", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const roster = JSON.parse(
        fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.json"), "utf8"),
    );
    const definition = JSON.parse(
        fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"),
    );
    const candidateNames = [
        "Catuvellauni",
        "Cantiaci",
        "Durovernum",
        "Parisi",
        "Corieltauvi",
        "Cornovii",
        "Dobunni",
        "Durotriges",
        "Dumnonii",
        "Silures",
        "Demetae",
        "Ordovices",
        "Deceangli",
        "Caledonii",
        "Votadini",
        "Belgae",
        "Gaulish Traders",
        "Irish Raiders",
        "Picts",
    ];
    const byName = new Map(roster.factions.map((faction) => [faction.name, faction]));
    for (const name of candidateNames)
        assert.equal(byName.get(name).relation, "neutral", name + " remains neutral");
    const romans = roster.factions.filter(
        (faction) => faction.civ.replace(/[^a-z0-9]/gi, "").toLowerCase() === "romanempire",
    );
    assert.equal(romans.length, 13);
    assert.ok(
        romans.every(
            (faction) =>
                faction.relation === "enemy" &&
                faction.team === "Red" &&
                faction.can_request_alliance === false,
        ),
    );
    for (const id of [
        "lindum",
        "viroconium",
        "isca_dumnoniorum",
        "brigantes_1",
        "atrebates",
        "regnenses",
        "calleva",
        "noviomagus",
    ]) {
        const faction = roster.factions.find((entry) => entry.id === id);
        assert.ok(
            faction && faction.relation === "enemy" && faction.team === "Red",
            id + " is hostile to Boudica",
        );
    }
    assert.equal(
        roster.factions.some((faction) => faction.id === "deva"),
        false,
    );
    const targets = ["camulodunum", "legio_ix_hispana", "londinium", "verulamium"];
    const stepsById = new Map(definition.steps.map((step) => [step.id, step]));
    const recordedTargets = [];
    let current = stepsById.get("boudica_camulodunum");
    assert.equal(
        current.next,
        "boudica_ninth_legion_intro",
        "the Ninth warning introduces the City upgrade before the attack",
    );
    assert.equal(stepsById.get("boudica_ninth_legion_intro").next, "boudica_select_city_upgrade");
    assert.equal(
        stepsById.get("boudica_ninth_legion").next,
        "boudica_londinium_intro",
        "defeating the Ninth continues to Londinium, not the City upgrade",
    );
    while (current && current.id !== "boudica_final_battle_intro") {
        if (
            current.trigger &&
            current.trigger.type === "defeated" &&
            targets.includes(current.trigger.target)
        )
            recordedTargets.push(current.trigger.target);
        current = stepsById.get(current.next);
    }
    assert.deepEqual(recordedTargets, targets, "follow the authored mission links before the final battle");
    const resistance = stepsById.get("boudica_last_stand_resist");
    assert.deepEqual(resistance.campaign_attack_on_enter, {
        faction_id: "legio_ii_augusta",
        can_attack_player: true,
    });
    assert.equal(
        stepsById.get("boudica_final_choice").choices.find((choice) => choice.id === "fight").next,
        resistance.id,
    );
    assert.equal(stepsById.has("boudica_postumus_notice"), false);
    assert.equal(
        definition.reactions.some((reaction) => reaction.id === "neutral_contact_terms"),
        false,
    );
    const snettisham = definition.reactions.find((reaction) => reaction.id === "contact_snettisham");
    assert.equal(snettisham.choices.find((choice) => choice.id === "accept").gold_cost, 200);
    assert.equal(byName.get("Snettisham").can_attack_player, false);
    assert.notEqual(byName.get("Thetford").can_attack_player, false);
    const invalidRomanNeutral = structuredClone(roster);
    invalidRomanNeutral.factions.find((faction) => faction.id === "legio_xiv_gemina").relation = "neutral";
    assert.ok(
        campaign
            .validate(definition, invalidRomanNeutral, { hasText: () => true, hasAvatar: () => true })
            .errors.some((issue) => issue.field === "roster.factions.relation"),
    );
});

test("campaign attack permission changes accept existing enemies only", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const definition = {
        version: 2,
        episode_id: "attack_permission",
        entry: "notice",
        settings: { starting_troops: 1000 },
        speakers: {},
        steps: [
            {
                id: "notice",
                type: "scene",
                title_key: "tutorial.notice",
                campaign_attack_on_enter: { faction_id: "enemy", can_attack_player: true },
                next: "end",
            },
            { id: "end", type: "end", title_key: "tutorial.end" },
        ],
    };
    const roster = {
        map: "eastanglia",
        player_spawn: [1, 1],
        factions: [
            {
                id: "enemy",
                name: "Enemy",
                x: 1,
                y: 1,
                starting_troops: 1,
                relation: "enemy",
                civ: "Roman Empire",
                leader: "Caesar",
            },
            {
                id: "clan",
                name: "Clan",
                x: 2,
                y: 2,
                starting_troops: 1,
                relation: "neutral",
                civ: "Gallic Tribes",
                leader: "Caesar",
            },
        ],
    };
    assert.deepEqual(campaign.validate(definition, roster, { hasText: () => true }).errors, []);
    definition.steps[0].campaign_attack_on_enter.faction_id = "clan";
    assert.ok(
        campaign
            .validate(definition, roster, { hasText: () => true })
            .errors.some((issue) => issue.field === "campaign_attack_on_enter.faction_id"),
    );
});

test("scene camera shots film the player or existing factions only", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const definition = {
        version: 2,
        episode_id: "camera_shot",
        entry: "notice",
        settings: { starting_troops: 1000 },
        speakers: {},
        steps: [
            {
                id: "notice",
                type: "scene",
                title_key: "tutorial.notice",
                camera: { shot: "approach", from: "player", to: "enemy", seconds: 3 },
                next: "end",
            },
            { id: "end", type: "end", title_key: "tutorial.end" },
        ],
    };
    const roster = {
        map: "eastanglia",
        player_spawn: [1, 1],
        factions: [
            {
                id: "enemy",
                name: "Enemy",
                x: 1,
                y: 1,
                starting_troops: 1,
                relation: "enemy",
                civ: "Roman Empire",
                leader: "Caesar",
            },
        ],
    };
    const errors = () =>
        campaign.validate(definition, roster, { hasText: () => true }).errors.map((issue) => issue.field);
    assert.deepEqual(errors(), []);
    definition.steps[0].camera = { shot: "spin", to: "nobody", seconds: 9 };
    assert.deepEqual(errors(), ["camera.shot", "camera.to", "camera.seconds"]);
    definition.steps[0] = {
        id: "notice",
        type: "objective",
        title_key: "tutorial.notice",
        pause_game: true,
        camera_only: true,
        trigger: { type: "zoom_out_complete", value: 0.95, scope: "step" },
        camera: { shot: "link", to: "enemy", seconds: 3 },
        next: "end",
    };
    assert.ok(errors().includes("camera"), "a camera lesson cannot move the camera for the player");
});

test("Boudica final decision opens at 5,000 tiles and targets her on the map", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const definition = JSON.parse(
        fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"),
    );
    const stand = definition.steps.find((step) => step.id === "boudica_last_stand");
    const choice = definition.steps.find((step) => step.id === "boudica_final_choice");
    assert.equal(stand.trigger.value, 5000);
    assert.deepEqual(choice.marker, { target: "player" });
    let now = 0;
    const machine = campaign.create(definition, stand.id, { factions: [] }, () => now);
    for (let t = 0; t < 10000 && machine.view().exchange; t += 100) {
        now += 100;
        machine.update({ tiles: 5001 }, {});
    }
    assert.equal(machine.update({ tiles: 5001 }, {}).step.id, stand.id);
    assert.equal(machine.update({ tiles: 5000 }, {}).step.id, choice.id);
    const editorPreviewFocus = campaignEditor.slice(
        campaignEditor.indexOf("function applyPreviewFocus"),
        campaignEditor.indexOf(
            "function renderPreview",
            campaignEditor.indexOf("function applyPreviewFocus"),
        ),
    );
    assert.match(editorPreviewFocus, /step\.marker && step\.marker\.target/);
});

test("Boudica reveal beats share a fixed 1.1-second wait while tribal contact stays immediate", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const definition = JSON.parse(
        fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"),
    );
    const waitSteps = definition.steps.filter((step) => step.wait_before === true).map((step) => step.id);
    assert.equal(campaign.WAIT_MS, 1100);
    assert.deepEqual(waitSteps, ["boudica_city_intro"]);
    const next = (id) => definition.steps.find((step) => step.id === id).next;
    assert.equal(next("boudica_share_sent"), "boudica_trinovantes_assault");
    assert.equal(next("boudica_trinovantes_assault"), "boudica_camulodunum");
    assert.equal(next("boudica_aid_kept"), "boudica_camulodunum");
    assert.equal(next("boudica_outpost_colonia_veterans"), "boudica_outpost_tax_collectors");
    assert.deepEqual(
        definition.steps.find((step) => step.id === "boudica_trinovantes_assault").campaign_assault_on_enter,
        {
            attacker_team: "Blue",
            target: "camulodunum",
        },
    );
    assert.equal(
        definition.steps.find((step) => step.id === "boudica_aid_kept").campaign_assault_on_enter,
        undefined,
    );
    assert.equal(
        definition.steps.some((step) => /factory|tower/.test(step.id)),
        false,
    );
    assert.equal(
        definition.steps.some(
            (step) =>
                Object.hasOwn(step, "start_delay_seconds") || Object.hasOwn(step, "advance_delay_seconds"),
        ),
        false,
    );
    for (const [type, legacyField] of [
        ["scene", "start_delay_seconds"],
        ["objective", "advance_delay_seconds"],
    ]) {
        const legacyStep = { id: "wait", type, title_key: "tutorial.wait", next: "end", [legacyField]: 1 };
        if (type === "objective") legacyStep.trigger = { type: "elapsed", scope: "step", value: 1 };
        const invalid = campaign.validate(
            {
                version: 2,
                episode_id: "legacy_wait",
                settings: { starting_troops: 1000 },
                speakers: {},
                entry: "wait",
                steps: [legacyStep, { id: "end", type: "end", title_key: "tutorial.end" }],
            },
            null,
            { hasText: () => true },
        );
        assert.ok(
            invalid.errors.some((issue) => issue.step === "wait" && issue.field === "fields"),
            `${legacyField} remains rejected`,
        );
    }
    const immediate = campaign.create(
        {
            entry: "scene",
            steps: [
                { id: "scene", type: "scene", title_key: "scene", next: "end" },
                { id: "end", type: "end", title_key: "end" },
            ],
        },
        "scene",
        null,
        () => 0,
    );
    assert.equal(immediate.view().waiting, false, "unflagged dialogue has no artificial wait");
    assert.equal(immediate.advance(null, "scene"), true, "unflagged dialogue continues normally");

    let choiceClock = 0;
    const decision = campaign.create(
        {
            entry: "choice",
            steps: [
                { id: "choice", type: "choice", wait_before: true, choices: [{ id: "yes", next: "scene" }] },
                { id: "scene", type: "scene", title_key: "scene", next: "end" },
                { id: "end", type: "end", title_key: "end" },
            ],
        },
        "choice",
        null,
        () => choiceClock,
    );
    assert.equal(decision.advance("yes", "choice"), false, "a hidden choice cannot be selected");
    choiceClock = 1100;
    assert.equal(decision.view().waiting, false);
    assert.equal(decision.advance("yes", "choice"), true, "the visible choice responds immediately");
    assert.equal(decision.view().step.id, "scene");
});

test("campaign building and action permissions are cumulative, route-safe, and reject legacy global switches", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const rustCampaign = fs.readFileSync(path.join(shell, "../../sow-core/src/campaign.rs"), "utf8");
    const rustGame = fs.readFileSync(path.join(shell, "../../sow-core/src/game.rs"), "utf8");
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const byId = new Map(definition.steps.map((step) => [step.id, step]));

    assert.deepEqual(campaign.campaignUnlocksAtStep(byId, definition.entry, "boudica_choose_city"), {
        buildings: { City: 1 },
        actions: [],
    });
    assert.deepEqual(campaign.campaignUnlocksAtStep(byId, definition.entry, "boudica_select_city_upgrade"), {
        buildings: { City: 2 },
        actions: [],
    });
    assert.deepEqual(campaign.validate(definition, roster).errors, []);
    assert.equal(
        definition.steps.some((step) => /factory|tower/i.test(step.id)),
        false,
    );
    assert.ok(
        definition.steps.every(
            (step) =>
                !step.unlocks ||
                (Object.keys(step.unlocks.buildings || {}).every((kind) => kind === "City") &&
                    (!step.unlocks.actions || step.unlocks.actions.length === 0)),
        ),
    );

    assert.deepEqual(campaign.CAMPAIGN_ACTION_BUILDING_REQUIREMENTS, {
        transport_fleet: null,
        trade_ship: { kind: "Port", level: 1 },
        warship: { kind: "Port", level: 2 },
        nuke: { kind: "City", level: 4 },
    });
    const buildingEnum = rustGame.match(/pub enum BuildingKind \{([\s\S]*?)\n\}/)[1];
    const rustBuildings = Array.from(
        buildingEnum.matchAll(/^\s*([A-Z][A-Za-z0-9]*),?\s*$/gm),
        (match) => match[1],
    ).sort();
    assert.deepEqual(
        rustBuildings,
        Object.keys(campaign.STRUCTURE_LEVEL_LIMITS).sort(),
        "Rust buildings, campaign validation and editor controls must stay in sync",
    );
    const actionEnum = rustCampaign.match(/pub enum CampaignAction \{([\s\S]*?)\n\}/)[1];
    const rustActions = Array.from(actionEnum.matchAll(/^\s*([A-Z][A-Za-z0-9]*),?\s*$/gm), (match) =>
        match[1].replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase(),
    ).sort();
    assert.deepEqual(
        rustActions,
        campaign.CAMPAIGN_ACTIONS.slice().sort(),
        "Rust action permissions and editor controls must stay in sync",
    );
    const cityTwo = { buildings: { City: 2 }, actions: ["transport_fleet", "trade_ship", "warship", "nuke"] };
    assert.equal(campaign.campaignAllowsAction(cityTwo, "transport_fleet"), true);
    assert.equal(campaign.campaignAllowsAction(cityTwo, "trade_ship"), false);
    assert.equal(campaign.campaignAllowsAction(cityTwo, "warship"), false);
    assert.equal(campaign.campaignAllowsAction(cityTwo, "nuke"), false);
    assert.equal(
        campaign.campaignAllowsAction({ buildings: { Port: 2, City: 4 }, actions: cityTwo.actions }, "nuke"),
        true,
    );

    const withoutCityGrant = structuredClone(definition);
    delete withoutCityGrant.steps.find((step) => step.id === "boudica_choose_city").unlocks;
    assert.ok(
        campaign
            .validate(withoutCityGrant, roster)
            .errors.some(
                (issue) =>
                    issue.step === "boudica_build_city" &&
                    issue.field === "unlocks" &&
                    issue.message.includes("City level 1"),
            ),
    );
    const withoutUpgradeGrant = structuredClone(definition);
    delete withoutUpgradeGrant.steps.find((step) => step.id === "boudica_select_city_upgrade").unlocks;
    assert.ok(
        campaign
            .validate(withoutUpgradeGrant, roster)
            .errors.some(
                (issue) =>
                    issue.step === "boudica_structure_upgrade" &&
                    issue.field === "unlocks" &&
                    issue.message.includes("City level 2"),
            ),
    );

    const legacySettings = structuredClone(definition);
    legacySettings.settings.buildings_enabled = true;
    legacySettings.settings.buildings_unlock_after_defeated = "rome";
    assert.ok(
        campaign
            .validate(legacySettings, roster)
            .errors.some((issue) => issue.field === "settings" && issue.message === "Unknown match setting."),
    );

    const branch = {
        version: 2,
        episode_id: "unlock_branch",
        settings: { starting_troops: 1000 },
        entry: "choice",
        steps: [
            {
                id: "choice",
                type: "choice",
                title_key: "choice",
                choices: [
                    { id: "grant", label_key: "grant", next: "grant_city" },
                    { id: "skip", label_key: "skip", next: "skip_city" },
                ],
            },
            {
                id: "grant_city",
                type: "scene",
                title_key: "grant_city",
                unlocks: { buildings: { City: 1 } },
                next: "shared",
            },
            { id: "skip_city", type: "scene", title_key: "skip_city", next: "shared" },
            {
                id: "shared",
                type: "objective",
                title_key: "shared",
                trigger: { type: "structure_level", kind: "City", value: 1 },
                next: "end",
            },
            { id: "end", type: "end", title_key: "end" },
        ],
    };
    const branchById = new Map(branch.steps.map((step) => [step.id, step]));
    assert.deepEqual(campaign.campaignUnlocksAtStep(branchById, branch.entry, "shared"), {
        buildings: {},
        actions: [],
    });
    assert.ok(
        campaign
            .validate(branch, null, { hasText: () => true })
            .errors.some(
                (issue) =>
                    issue.step === "shared" &&
                    issue.field === "unlocks" &&
                    issue.message.includes("City level 1"),
            ),
    );
});

test("Boudica pauses for action steps and resumes only after valid expansion or construction", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const step = (id) => definition.steps.find((candidate) => candidate.id === id);

    const expansion = campaign.create(definition, "boudica_first_expansion", roster);
    assert.equal(expansion.update({ tiles_gained: 0, wilderness_orders_accepted: 0 }, {}).paused, true);
    assert.equal(expansion.view().paused_action, "expand_once_then_resume");
    assert.equal(
        expansion.update({ tiles_gained: 0, wilderness_orders_accepted: 0 }, {}).paused,
        true,
        "a rejected click leaves the game paused",
    );
    const acceptedExpansion = expansion.update({ tiles_gained: 0, wilderness_orders_accepted: 1 }, {});
    assert.equal(acceptedExpansion.paused, false, "accepted neutral attack resumes before capture");
    assert.equal(
        acceptedExpansion.step.id,
        "boudica_first_expansion",
        "the 250-tile goal still waits for actual territory gain",
    );
    assert.equal(
        expansion.update({ tiles_gained: 250, wilderness_orders_accepted: 1 }, {}).step.id,
        "boudica_city_intro",
    );
    assert.equal(expansion.view().paused, true);

    for (const id of ["boudica_first_contact", "boudica_third_contact"]) {
        assert.equal(
            step(id).attack_ratio_on_enter,
            undefined,
            "contact missions preserve the player's troop allocation",
        );
        assert.equal(step(id).pause_game, true);
        assert.equal(step(id).paused_action, "expand_once_then_resume");
        assert.equal(step(id).guide.target, "expand");
    }
    assert.equal(step("boudica_second_contact").attack_ratio_on_enter, undefined);
    assert.equal(
        step("boudica_second_contact").pause_game,
        undefined,
        "the second Iceni meeting does not pause after Stonea's dialogue",
    );
    assert.equal(step("boudica_second_contact").paused_action, undefined);
    assert.equal(step("boudica_second_contact").guide.target, "expand");
    assert.equal(
        definition.settings.starting_gold,
        180,
        "Boudica starts with 30 additional gold for the first paid alliance",
    );
    assert.deepEqual(
        campaign.validate(definition, roster).errors,
        [],
        "the Boudica gold setting is accepted by the shared campaign validator",
    );
    const invalidGold = structuredClone(definition);
    invalidGold.settings.starting_gold = -1;
    assert.ok(
        campaign
            .validate(invalidGold, roster)
            .errors.some((issue) => issue.field === "settings.starting_gold"),
    );
    const secondContact = campaign.create(definition, "boudica_second_contact", roster);
    assert.equal(
        secondContact.update({ contact_faction_ids: [] }, {}).paused,
        false,
        "the Snettisham objective runs without pausing",
    );
    const contact = campaign.create(definition, "boudica_first_contact", roster);
    const contactFacts = { tiles_gained: 250, wilderness_orders_accepted: 0, contact_faction_ids: [] };
    assert.equal(contact.update(contactFacts, {}).paused, true);
    assert.equal(contact.update({ ...contactFacts, wilderness_orders_accepted: 1 }, {}).paused, false);
    assert.equal(
        contact.update({ ...contactFacts, tiles_gained: 251, wilderness_orders_accepted: 1 }, {}).paused,
        false,
    );
    assert.equal(
        contact.update(
            {
                ...contactFacts,
                tiles_gained: 251,
                wilderness_orders_accepted: 1,
                contact_faction_ids: ["stonea"],
            },
            {},
        ).paused,
        true,
    );

    assert.equal(step("boudica_transfer_target").pause_game, true);
    assert.equal(step("boudica_transfer_send").pause_game, true);
    const transfer = campaign.create(definition, "boudica_transfer_send", roster);
    assert.equal(
        transfer.update({ resource_transfers_by_recipient_faction_id: { trinovantes: { total: 0 } } }, {})
            .paused,
        true,
    );
    assert.equal(transfer.view().paused_action, "send_resources_stay_paused");
    const sent = transfer.update(
        { resource_transfers_by_recipient_faction_id: { trinovantes: { total: 1 } } },
        {},
    );
    assert.equal(sent.step.id, "boudica_share_sent");
    assert.equal(sent.paused, true);

    for (const [id, metric, waitId] of [["boudica_build_city", "cities", "boudica_city_construction_wait"]]) {
        assert.equal(step(id).pause_game, true);
        assert.equal(step(id).paused_action, "build_until_started");
        const machine = campaign.create(definition, id, roster);
        assert.equal(machine.update({ [metric]: 0 }, {}).paused, true);
        const rejected = machine.update({ [metric]: 0 }, {});
        assert.equal(
            rejected.paused,
            true,
            "a rejected placement leaves the paused action available for another attempt",
        );
        assert.equal(rejected.step.id, id);
        const started = machine.update({ [metric]: 1 }, {});
        assert.equal(started.step.id, waitId);
        assert.equal(started.paused, false);
        assert.equal(step(waitId).pause_game, undefined);
    }

    assert.equal(step("boudica_select_city_upgrade").pause_game, true);
    const citySelection = campaign.create(definition, "boudica_select_city_upgrade", roster);
    assert.equal(
        citySelection.update({ selected_building_kind: "" }, {}).paused,
        true,
        "the game stays paused while the player has not selected the City",
    );
    const citySelected = citySelection.update({ selected_building_kind: "City" }, {});
    assert.equal(citySelected.step.id, "boudica_structure_upgrade");
    assert.equal(citySelected.paused, true, "selection flows directly into the still-paused upgrade action");

    assert.equal(step("boudica_structure_upgrade").trigger.type, "structure_upgrade");
    assert.equal(step("boudica_structure_upgrade").trigger.kind, "City");
    assert.equal(step("boudica_structure_upgrade").paused_action, "upgrade_until_started");
    const upgradeButton = campaign.create(definition, "boudica_structure_upgrade", roster);
    assert.equal(upgradeButton.update({ structure_upgrades: 0 }, {}).paused, true);
    const rejectedUpgrade = upgradeButton.update({ structure_upgrades: 0 }, {});
    assert.equal(
        rejectedUpgrade.paused,
        true,
        "a rejected upgrade leaves the paused action available for another attempt",
    );
    assert.equal(rejectedUpgrade.step.id, "boudica_structure_upgrade");
    const upgrading = upgradeButton.update({ structure_upgrades: 1 }, {});
    assert.equal(upgrading.step.id, "boudica_city_upgrade_wait");
    assert.equal(upgrading.paused, false, "the upgrade's construction wait runs after upgrade acceptance");
    assert.equal(step("boudica_city_upgrade_wait").pause_game, undefined);
    assert.equal(step("boudica_attack_roman_outpost").pause_game, true);
    assert.equal(step("boudica_rise_of_the_iceni_intro").attack_ratio_on_enter, 0.25);
    assert.equal(step("boudica_attack_roman_outpost").attack_ratio_on_enter, undefined);
    for (const id of [
        "boudica_outpost_colonia_veterans",
        "boudica_outpost_tax_collectors",
        "boudica_outpost_roman_supply_depot",
        "boudica_camulodunum",
        "boudica_londinium",
        "boudica_verulamium",
        "boudica_ninth_legion",
    ]) {
        assert.equal(step(id).pause_game, undefined, `${id} stays available for border expansion`);
    }
});

test("renaming a campaign faction updates catalog copy through stable roster IDs", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const definition = {
        settings: { campaign_support: { after_defeated: "roman_outpost", share_percent: 50 } },
        speakers: { roman: { faction: "roman_outpost" } },
        steps: [
            {
                trigger: { target: "roman_outpost", targets: ["roman_outpost", "other"] },
                marker: { target: "roman_outpost" },
            },
        ],
        reactions: [{ when: { type: "contact", target: "roman_outpost" } }],
    };
    const renamed = campaign.renameFactionText(
        definition,
        [{ from: "Old Name", to: "Roman Outpost" }],
        [{ id: "roman_outpost", name: "Old Name" }],
    );
    assert.equal(renamed.settings.campaign_support.after_defeated, "roman_outpost");
    assert.equal(renamed.speakers.roman.faction, "roman_outpost");
    assert.deepEqual(renamed.steps[0].trigger, {
        target: "roman_outpost",
        targets: ["roman_outpost", "other"],
    });
    assert.equal(renamed.steps[0].marker.target, "roman_outpost");
    assert.equal(renamed.reactions[0].when.target, "roman_outpost");
    assert.deepEqual(renamed.faction_story_names, { roman_outpost: "Old Name" });
    assert.equal(
        campaign.replaceFactionStoryNames("Attack Old Name!", renamed, {
            factions: [{ id: "roman_outpost", name: "Roman Outpost" }],
        }),
        "Attack Roman Outpost!",
    );
    assert.equal(
        campaign.replaceFactionStoryNames("Old Namesake", renamed, {
            factions: [{ id: "roman_outpost", name: "Roman Outpost" }],
        }),
        "Old Namesake",
        "similar faction names stay unchanged",
    );
    assert.equal(definition.steps[0].trigger.target, "roman_outpost", "renaming preserves stable references");
    assert.deepEqual([...campaign.factionReferenceIds(renamed)].sort(), ["other", "roman_outpost"]);
});

test("disabled neutral offers stay hidden while named story dialogues and legacy rosters work", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const roster = {
        factions: [
            { id: "story_faction", relation: "allied", can_request_alliance: false },
            { id: "neutral_faction", relation: "neutral", can_request_alliance: false },
        ],
    };
    const waiting = { id: "waiting", type: "objective", trigger: { type: "elapsed", value: 30 } };
    const storyOffer = { id: "named_story", when: { type: "contact", target: "story_faction" } };
    const neutralOffer = { id: "neutral_contact_terms", when: { type: "contact", relation: "neutral" } };
    const storyFacts = { contact_faction_ids: ["story_faction"] };

    const story = campaign.create(
        { entry: "waiting", steps: [waiting], reactions: [storyOffer] },
        undefined,
        roster,
        () => 0,
    );
    const immediateContact = story.update(storyFacts, {});
    assert.equal(
        immediateContact.reaction,
        "named_story",
        "an exact story response ignores the generic-offer flag",
    );
    assert.equal(
        immediateContact.step.type,
        "scene",
        "contact opens its response on the first campaign update",
    );
    assert.equal(immediateContact.paused, true);

    const genericDefinition = { entry: "waiting", steps: [waiting], reactions: [neutralOffer] };
    const neutralFacts = { contact_faction_ids: ["neutral_faction"] };
    const disabledRoster = JSON.parse(JSON.stringify(roster));
    disabledRoster.factions.find((faction) => faction.id === "neutral_faction").can_request_alliance = false;
    const disabledGeneric = campaign.create(genericDefinition, undefined, disabledRoster);
    assert.equal(
        disabledGeneric.update(neutralFacts, {}).reaction,
        undefined,
        "false hides a generic neutral offer",
    );

    const legacyRoster = JSON.parse(JSON.stringify(disabledRoster));
    delete legacyRoster.factions.find((faction) => faction.id === "neutral_faction").can_request_alliance;
    const legacyGeneric = campaign.create(genericDefinition, undefined, legacyRoster);
    assert.equal(
        legacyGeneric.update(neutralFacts, {}).reaction,
        "neutral_contact_terms",
        "a missing legacy field keeps the old offer behavior",
    );
});

test("defeat scenes wait 1.1 seconds when attack and defeat arrive in the same turn", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    let clock = 0;
    const definition = {
        entry: "attack",
        steps: [
            {
                id: "attack",
                type: "objective",
                trigger: { type: "attack", target: "outpost", value: 1, scope: "total" },
                next: "victory",
            },
            {
                id: "victory",
                type: "objective",
                trigger: { type: "defeated", target: "outpost", value: 1, scope: "total" },
                next: "victory_scene",
            },
            {
                id: "victory_scene",
                type: "scene",
                title_key: "tutorial.victory",
                wait_before: true,
                next: "end",
            },
            { id: "end", type: "end", title_key: "tutorial.end" },
        ],
    };
    const machine = campaign.create(definition, "attack", { factions: [] }, () => clock);
    const beforeDefeat = {
        elapsed_seconds: 3,
        attacks_by_faction_id: {},
        defeated_faction_ids: [],
        eliminated_faction_ids: [],
    };
    machine.update(beforeDefeat, {});

    const attackAndDefeat = {
        elapsed_seconds: 4,
        attacks_by_faction_id: { outpost: 1 },
        defeated_faction_ids: ["outpost"],
        eliminated_faction_ids: ["outpost"],
    };
    const revealed = machine.update(attackAndDefeat, {});
    assert.equal(
        revealed.step.id,
        "victory_scene",
        "objective completion transitions immediately to the 1.1-second scene wait",
    );
    assert.equal(revealed.waiting, true);
    assert.equal(machine.advance(null, "victory_scene"), false, "the hidden scene cannot be skipped");
    clock = 1099;
    assert.equal(machine.view().waiting, true);
    clock = 1100;
    assert.equal(machine.view().waiting, false);
    assert.equal(machine.advance(null, "victory_scene"), true);
    assert.equal(machine.view().step.id, "end");
});

test("contact responses wait behind an open campaign scene", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const machine = campaign.create({
        version: 2,
        episode_id: "queued_contact",
        entry: "scene",
        settings: { starting_troops: 1000 },
        steps: [
            { id: "scene", type: "scene", title_key: "scene", next: "objective" },
            {
                id: "objective",
                type: "objective",
                title_key: "objective",
                trigger: { type: "territory", value: 10, scope: "total" },
                next: "end",
            },
            { id: "end", type: "end", title_key: "end" },
        ],
        reactions: [
            {
                id: "contact",
                when: { type: "contact", target: "snettisham" },
                title_key: "contact",
                body_key: "contact.body",
            },
        ],
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
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const definition = {
        version: 2,
        episode_id: "test",
        entry: "gate",
        settings: { starting_troops: 1000 },
        steps: [
            {
                id: "gate",
                type: "objective",
                title_key: "gate",
                trigger: { type: "territory", value: 1, scope: "step" },
                next: "talk",
            },
            { id: "talk", type: "scene", title_key: "talk", next: "contact" },
            {
                id: "contact",
                type: "objective",
                title_key: "contact",
                trigger: { type: "contact", targets: ["a", "b"], value: 2, scope: "episode" },
                next: "end",
            },
            { id: "end", type: "end", title_key: "end" },
        ],
        reactions: [
            {
                id: "aid_a",
                after: "gate",
                when: { type: "support", target: "a" },
                title_key: "aid.a.title",
                body_key: "aid.a.body",
            },
            {
                id: "aid_b",
                after: "gate",
                when: { type: "support", target: "b" },
                title_key: "aid.b.title",
                body_key: "aid.b.body",
            },
        ],
    };
    const machine = campaign.create(definition);
    const receipts = {
        a: { deliveries: 2, first_tick: 9 },
        b: { deliveries: 1, first_tick: 3 },
    };
    machine.update({ tiles_gained: 0, support_deliveries_by_faction_id: {} }, {});
    assert.equal(machine.view().step.id, "gate");
    machine.update({ tiles_gained: 1, support_deliveries_by_faction_id: {} }, {});
    assert.equal(machine.view().step.id, "talk");
    const deferred = machine.update(
        { contact_faction_ids: [], support_deliveries_by_faction_id: receipts },
        {},
    );
    assert.equal(
        deferred.reaction,
        undefined,
        "support received during a scene waits until gameplay resumes",
    );
    assert.equal(deferred.step.id, "talk");
    machine.advance(null, "talk");
    assert.equal(machine.view().step.id, "contact");

    const firstReaction = machine.update(
        { contact_faction_ids: ["a"], support_deliveries_by_faction_id: receipts },
        {},
    );
    assert.equal(firstReaction.reaction, "aid_b");
    assert.equal(firstReaction.paused, true);
    assert.equal(firstReaction.progress.current, 1);
    assert.equal(machine.advance(null, "reaction-aid_b"), true);
    assert.equal(machine.advance(null, "reaction-aid_b"), false);
    const secondReaction = machine.update(
        { contact_faction_ids: ["a"], support_deliveries_by_faction_id: receipts },
        {},
    );
    assert.equal(secondReaction.reaction, "aid_a");
    machine.advance(null, "reaction-aid_a");
    assert.equal(
        machine.update({ contact_faction_ids: ["a"], support_deliveries_by_faction_id: receipts }, {}).step
            .id,
        "contact",
    );
    assert.equal(machine.view().progress.current, 1);
    assert.deepEqual(machine.state.reactionsShown, ["aid_b", "aid_a"]);
    assert.deepEqual(
        campaign.create(definition).state.reactionsShown,
        [],
        "a new run starts with no previous ally responses shown",
    );
});

test("first-contact responses open on the contact fact and queue each faction once", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const machine = campaign.create({
        version: 2,
        episode_id: "instant_contact",
        entry: "objective",
        settings: { starting_troops: 1000 },
        steps: [
            {
                id: "objective",
                type: "objective",
                title_key: "objective",
                trigger: { type: "territory", value: 10, scope: "total" },
                next: "after",
            },
            { id: "after", type: "scene", title_key: "after", next: "end" },
            { id: "end", type: "end", title_key: "end" },
        ],
        reactions: [
            { id: "contact_a", when: { type: "contact", target: "a" }, title_key: "a", body_key: "a_body" },
            { id: "contact_b", when: { type: "contact", target: "b" }, title_key: "b", body_key: "b_body" },
        ],
    });
    machine.update({ tiles_gained: 0, contact_faction_ids: [] }, {});
    const first = machine.update({ tiles_gained: 0, contact_faction_ids: ["b"] }, {});
    assert.equal(first.reaction, "contact_b");
    machine.advance(null, "reaction-contact_b@b");
    const second = machine.update({ tiles_gained: 0, contact_faction_ids: ["b", "a"] }, {});
    assert.equal(second.reaction, "contact_a");
    machine.advance(null, "reaction-contact_a@a");
    assert.equal(
        machine.update({ tiles_gained: 0, contact_faction_ids: ["b", "a"] }, {}).step.id,
        "objective",
    );
});

test("first Iceni contact is friendly, then each neutral contact negotiates independently", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const roster = {
        factions: [
            { id: "kin_one", name: "Kin One", relation: "neutral" },
            { id: "kin_two", name: "Kin Two", relation: "neutral" },
            { id: "neutral", name: "Neutral", relation: "neutral" },
            { id: "neutral_two", name: "Neutral Two", relation: "neutral" },
            { id: "rome", name: "Rome", relation: "enemy" },
        ],
    };
    const definition = {
        version: 2,
        episode_id: "contact_order",
        entry: "objective",
        settings: { starting_troops: 1000 },
        steps: [
            {
                id: "objective",
                type: "objective",
                title_key: "objective",
                trigger: { type: "territory", value: 10, scope: "total" },
                next: "end",
            },
            { id: "end", type: "end", title_key: "end" },
        ],
        reactions: [
            {
                id: "first",
                when: { type: "first_contact", targets: ["kin_one", "kin_two"] },
                outcome: "allied",
                title_key: "first_title",
                body_key: "first_body",
            },
            {
                id: "kin_two",
                when: { type: "contact", target: "kin_two" },
                choices: [
                    { id: "pay", label_key: "pay", relation: "allied", gold_cost: 200 },
                    { id: "refuse", label_key: "refuse", relation: "enemy" },
                ],
                title_key: "kin_title",
                body_key: "kin_body",
            },
            {
                id: "neutral_terms",
                when: { type: "contact", relation: "neutral" },
                choices: [
                    { id: "pay", label_key: "pay", relation: "allied", gold_cost: 200 },
                    { id: "refuse", label_key: "refuse", relation: "enemy" },
                ],
                title_key: "neutral_title",
                body_key: "neutral_body",
            },
        ],
    };
    const machine = campaign.create(definition, undefined, roster);
    const contacts = {
        tiles_gained: 0,
        contact_faction_ids: ["rome", "neutral", "neutral_two", "kin_two", "kin_one"],
    };
    const first = machine.update(contacts, {});
    assert.equal(first.reaction, "first");
    assert.equal(first.reactionTarget, "kin_one");
    assert.equal(machine.advance(null, "reaction-first@kin_one"), true);
    const second = machine.update(contacts, {});
    assert.equal(
        second.reaction,
        "kin_two",
        "specific faction dialogue takes precedence over the neutral template",
    );
    assert.equal(second.reactionTarget, "kin_two");
    assert.equal(machine.advance("pay", "reaction-kin_two@kin_two"), true);
    const third = machine.update(contacts, {});
    assert.equal(third.reaction, "neutral_terms");
    assert.equal(third.reactionTarget, "neutral");
    assert.equal(third.choices.find((choice) => choice.id === "pay").gold_cost, 200);
    assert.equal(
        machine.update(contacts, {}).reactionTarget,
        "neutral",
        "a pending choice stays active until selected",
    );
    assert.equal(machine.advance("refuse", "reaction-neutral_terms@neutral"), true);
    const fourth = machine.update(contacts, {});
    assert.equal(fourth.reaction, "neutral_terms");
    assert.equal(
        fourth.reactionTarget,
        "neutral_two",
        "each initially neutral faction negotiates on its own first contact",
    );
});

test("campaign negotiation uses the canonical HUD gold balance, not player-list fields", async () => {
    async function chooseAtBalance(balance) {
        const commands = [],
            root = { hidden: true, isConnected: false };
        let viewOptions, rendered;
        const document = {
            body: {
                appendChild(node) {
                    node.isConnected = true;
                },
            },
            documentElement: { dir: "ltr" },
            getElementById: () => null,
            createElement: () => root,
            addEventListener() {},
        };
        const model = {
            step: { id: "terms", type: "choice" },
            paused: true,
            state: { choices: {} },
            reactionData: {
                id: "terms",
                choices: [
                    { id: "pay", relation: "allied", gold_cost: 200 },
                    { id: "refuse", relation: "enemy", gold_cost: 0 },
                ],
            },
            reactionTarget: "snettisham",
            choices: [
                { id: "pay", label_key: "pay", relation: "allied", gold_cost: 200 },
                { id: "refuse", label_key: "refuse", relation: "enemy", gold_cost: 0 },
            ],
        };
        let advanced;
        const window = {
            addEventListener() {},
            SOW_menu_command(message) {
                commands.push(JSON.parse(message));
            },
            SOWCampaign: {
                validate: () => ({ errors: [] }),
                UI_TARGETS: {},
                zoomInputMode: () => "wheel",
                create: () => ({
                    state: { choices: {} },
                    update: () => model,
                    view: () => model,
                    advance(choiceId) {
                        advanced = choiceId;
                        model.reactionData = null;
                        return true;
                    },
                }),
            },
            SOWCampaignView: {
                mount: (_root, options) => {
                    viewOptions = options;
                    return {
                        render(value) {
                            rendered = value;
                        },
                        destroy() {},
                    };
                },
            },
        };
        vm.runInNewContext(tutorialBundle, {
            window,
            document,
            performance: { now: () => 0 },
            console,
            fetch(url) {
                const data = url.endsWith(".triggers.json")
                    ? {
                          episode_id: "boudica",
                          settings: {},
                          strings: {},
                          speakers: {},
                          steps: [{ id: "terms", type: "choice" }],
                      }
                    : { factions: [] };
                return Promise.resolve({ ok: true, json: () => Promise.resolve(data) });
            },
        });
        window.SOW_tutorial_state_update({
            phase: "Playing",
            hud: {
                gold: balance,
                players: [
                    { id: 1, name: "Boudica", is_me: true },
                    {
                        id: 2,
                        name: "Snettisham",
                        campaign_faction_id: "snettisham",
                        centroid_x: 12,
                        centroid_y: 34,
                    },
                ],
                settings: {},
                tutorial: { active: true, episode_id: "boudica", facts: {} },
            },
        });
        await new Promise(setImmediate);
        const displayed = rendered.choices[0];
        viewOptions.onChoice("pay");
        return { commands, displayed, advanced };
    }

    const funded = await chooseAtBalance(6020);
    assert.equal(funded.displayed.gold_available, 6020);
    assert.equal(funded.displayed.gold_insufficient, false);
    const focus = funded.commands.find((command) => command.type === "focus_world");
    assert.deepEqual([focus.x, focus.y], [12.5, 34.5]);
    const highlight = funded.commands.find(
        (command) => command.type === "set_dialog_border_highlight" && command.player_id === 2,
    );
    assert.ok(highlight, "terms dialog highlights the requesting entity");
    assert.ok(
        funded.commands.some(
            (command) => command.type === "set_dialog_border_highlight" && command.player_id === null,
        ),
        "border highlight clears once the dialog is resolved",
    );
    assert.equal(
        funded.commands.find((command) => command.type === "resolve_campaign_diplomacy").relation,
        "allied",
    );
    assert.equal(
        funded.commands.find((command) => command.type === "resolve_campaign_diplomacy").gold_cost,
        200,
    );
    assert.equal(funded.advanced, "pay");

    const short = await chooseAtBalance(0);
    assert.equal(short.displayed.gold_insufficient, true);
    assert.equal(
        short.commands.find((command) => command.type === "resolve_campaign_diplomacy").relation,
        "enemy",
    );
    assert.equal(
        short.commands.find((command) => command.type === "resolve_campaign_diplomacy").gold_cost,
        0,
    );
    assert.equal(short.advanced, "refuse");
});

test("campaign objectives distinguish the intended contact, alliance, fleet and transfer action", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    function currentFor(trigger, before, after) {
        const machine = campaign.create({
            version: 2,
            episode_id: "fact_test",
            entry: "objective",
            settings: { starting_troops: 1000 },
            steps: [
                { id: "objective", type: "objective", title_key: "test", trigger: trigger, next: "pending" },
            ],
        });
        machine.update(before, {});
        return machine.update(after, {}).progress.current;
    }
    assert.equal(
        currentFor(
            { type: "contact", targets: ["snettisham", "stonea"], scope: "total" },
            { contact_faction_ids: [] },
            { contact_faction_ids: ["stonea"] },
        ),
        1,
        "multi-contact targets count stable faction IDs",
    );
    assert.equal(
        currentFor(
            { type: "alliance", target: "catuvellauni", value: 1, scope: "total" },
            { alliance_faction_ids: [] },
            { alliance_faction_ids: ["other_tribe"] },
        ),
        0,
    );
    assert.equal(
        currentFor(
            { type: "fleet", unit: "TransportShip", target: "legio_xiv", value: 1, scope: "step" },
            { fleets_by_type: {}, transport_fleets_by_faction_id: {} },
            { fleets_by_type: { TradeShip: 1 }, transport_fleets_by_faction_id: { legio_xiv: 1 } },
        ),
        0,
    );
    assert.equal(
        currentFor(
            { type: "fleet", unit: "TransportShip", target: "legio_xiv", value: 1, scope: "step" },
            { fleets_by_type: {}, transport_fleets_by_faction_id: {} },
            { fleets_by_type: { TransportShip: 1 }, transport_fleets_by_faction_id: { other_legion: 1 } },
        ),
        0,
    );
    assert.equal(
        currentFor(
            { type: "fleet", unit: "TransportShip", target: "legio_xiv", value: 1, scope: "step" },
            { fleets_by_type: {}, transport_fleets_by_faction_id: {} },
            { fleets_by_type: { TransportShip: 1 }, transport_fleets_by_faction_id: { legio_xiv: 1 } },
        ),
        1,
    );
    const transfer = {
        type: "resource_transfer",
        recipient: "snettisham",
        resources: ["gold", "troops"],
        value: 1,
        scope: "step",
    };
    const emptyTransfer = { resource_transfers_by_recipient_faction_id: {} };
    assert.equal(
        currentFor(transfer, emptyTransfer, {
            resource_transfers_by_recipient_faction_id: { other: { gold_troops: 1 } },
        }),
        0,
    );
    assert.equal(
        currentFor(transfer, emptyTransfer, {
            resource_transfers_by_recipient_faction_id: {
                snettisham: { gold: 1, troops: 0, gold_troops: 0 },
            },
        }),
        0,
    );
    assert.equal(
        currentFor(transfer, emptyTransfer, {
            resource_transfers_by_recipient_faction_id: {
                snettisham: { gold: 1, troops: 1, gold_troops: 1 },
            },
        }),
        1,
    );
    const tutorialTransfer = { type: "resource_transfer", recipient: "stonea", value: 1, scope: "episode" };
    for (const sent of [
        { total: 1, gold: 1, troops: 0 },
        { total: 1, gold: 0, troops: 1 },
    ]) {
        const machine = campaign.create({
            version: 2,
            episode_id: "transfer_race_test",
            entry: "enter_gold",
            steps: [
                {
                    id: "enter_gold",
                    type: "guide",
                    trigger: { type: "ui", action: "transfer_gold", scope: "step" },
                    next: "send_aid",
                },
                { id: "send_aid", type: "guide", trigger: tutorialTransfer, next: "done" },
                { id: "done", type: "scene", title_key: "done" },
            ],
        });
        machine.update(
            { resource_transfers_by_recipient_faction_id: { stonea: { total: 0 } } },
            { transfer_gold: 0 },
        );
        const view = machine.update(
            { resource_transfers_by_recipient_faction_id: { stonea: sent } },
            { transfer_gold: 1 },
        );
        assert.equal(
            view.step.id,
            "done",
            "an accepted Stonea transfer on the guide transition tick must count",
        );
    }
});

test("an active campaign keeps its loaded definition when the editor draft changes", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const definition = {
        version: 2,
        episode_id: "snapshot_test",
        default_locale: "en",
        settings: { starting_troops: 1000 },
        strings: { en: { "tutorial.opening": "Original" } },
        speakers: {},
        entry: "opening",
        steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" },
        ],
    };
    const activeRun = campaign.create(definition);
    definition.strings.en["tutorial.opening"] = "Edited draft";
    definition.steps[0].next = "missing_step";
    assert.equal(activeRun.definition.strings.en["tutorial.opening"], "Original");
    assert.equal(activeRun.view().step.next, "ending");
});

test("campaign start keeps its loaded script and replay clicks belong to that episode", async () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const definition = {
        version: 2,
        episode_id: "boudica",
        settings: { starting_troops: 1000 },
        speakers: {},
        entry: "opening",
        steps: [
            {
                id: "opening",
                type: "scene",
                title_key: "tutorial.open",
                attack_ratio_on_enter: 0.25,
                next: "ending",
            },
            { id: "ending", type: "end", title_key: "tutorial.end" },
        ],
    };
    const roster = {
        map: "eastanglia",
        player_spawn: [1, 1],
        factions: [
            {
                id: "enemy",
                name: "Enemy",
                x: 2,
                y: 2,
                starting_troops: 500,
                relation: "enemy",
                civ: "Roman Empire",
                leader: "Caesar",
            },
        ],
    };
    const clone = (value) => JSON.parse(JSON.stringify(value));
    const buttons = ["boudica", "six_sky_ep2"].map((episodeId) => ({
        dataset: { episodeId },
        disabled: false,
        getClientRects: () => [{}],
        matches: () => false,
        contains(target) {
            return target === this;
        },
    }));
    const listeners = {},
        storyRoot = { hidden: true, isConnected: false, contains: () => false };
    const document = {
        body: {
            appendChild(node) {
                node.isConnected = true;
            },
        },
        documentElement: { dir: "ltr" },
        createElement: () => storyRoot,
        getElementById: () => null,
        querySelector: () => null,
        querySelectorAll: (selector) => (selector === campaign.UI_TARGETS.campaign_replay ? buttons : []),
        addEventListener(type, callback) {
            listeners[type] = callback;
        },
    };
    let fetches = 0,
        activeRun,
        lastUi;
    const commands = [];
    const window = {
        addEventListener() {},
        SOW_menu_command(message) {
            commands.push(JSON.parse(message));
        },
        SOW_t: (key) => ({ "tutorial.open": "Original", "tutorial.end": "End" })[key] || "[" + key + "]",
        SOW_hasText: (key) => Boolean(window.SOW_t(key)),
        SOWCampaign: {
            ...campaign,
            create(data) {
                activeRun = campaign.create(data);
                const update = activeRun.update;
                activeRun.update = (facts, ui, now) => {
                    lastUi = { ...ui };
                    return update(facts, ui, now);
                };
                return activeRun;
            },
        },
        SOWCampaignView: { mount: () => ({ render() {}, destroy() {} }) },
    };
    vm.runInNewContext(tutorialBundle, {
        window,
        document,
        Node: Object,
        performance: { now: () => 0 },
        console,
        fetch(url) {
            fetches++;
            return Promise.resolve({
                ok: true,
                json: () => Promise.resolve(clone(url.endsWith(".triggers.json") ? definition : roster)),
            });
        },
    });
    window.SOW_startCampaignEpisode("boudica");
    await new Promise(setImmediate);
    definition.steps[0].title_key = "tutorial.edited";
    const state = {
        phase: "Playing",
        hud: { tutorial: { active: true, episode_id: "boudica", facts: {} }, players: [], settings: {} },
    };
    window.SOW_tutorial_state_update(state);
    await new Promise(setImmediate);
    assert.equal(fetches, 2);
    assert.equal(activeRun.definition.steps[0].title_key, "tutorial.open");
    assert.equal(commands.find((command) => command.type === "set_attack_ratio").ratio, 0.25);
    listeners.click({ type: "click", target: buttons[1] });
    window.SOW_tutorial_state_update(state);
    assert.equal(lastUi.campaign_replay, undefined);
    listeners.click({ type: "click", target: buttons[0] });
    window.SOW_tutorial_state_update(state);
    assert.equal(lastUi.campaign_replay, 1);
});

test("campaign funnel records each step once and completes only after the ending", async () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const definition = {
        version: 2,
        episode_id: "step_funnel_test",
        default_locale: "en",
        settings: { starting_troops: 1000 },
        strings: {},
        speakers: {},
        entry: "opening",
        steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" },
        ],
    };
    const roster = { map: "test_map", player_spawn: [1, 1], factions: [] };
    const root = { hidden: true, isConnected: false };
    const events = [];
    let viewOptions;
    const document = {
        body: {
            appendChild(node) {
                node.isConnected = true;
            },
        },
        documentElement: { dir: "ltr" },
        getElementById: () => null,
        createElement: () => root,
        addEventListener() {},
        querySelector: () => null,
    };
    const window = {
        addEventListener() {},
        SOW_menu_command() {},
        SOW_trackExperienceEvent(payload) {
            events.push(JSON.parse(payload));
        },
        SOW_t: (key) =>
            ({ "tutorial.opening": "Opening", "tutorial.ending": "Ending" })[key] || "[" + key + "]",
        SOW_hasText: (key) => Boolean(window.SOW_t(key)),
        SOWCampaign: campaign,
        SOWCampaignView: {
            mount: (_root, options) => {
                viewOptions = options;
                return { render() {}, destroy() {} };
            },
        },
    };
    vm.runInNewContext(tutorialBundle, {
        window,
        document,
        performance: { now: () => 0 },
        console,
        fetch(url) {
            return Promise.resolve({
                ok: true,
                json: () => Promise.resolve(url.endsWith(".triggers.json") ? definition : roster),
            });
        },
    });
    window.SOW_startCampaignEpisode("step_funnel_test");
    await new Promise(setImmediate);
    const state = {
        phase: "Playing",
        hud: {
            tutorial: { active: true, episode_id: "step_funnel_test", facts: {} },
            players: [],
            settings: {},
        },
    };
    window.SOW_tutorial_state_update(state);
    await new Promise(setImmediate);
    window.SOW_tutorial_state_update(state);
    assert.deepEqual(JSON.parse(JSON.stringify(window.SOW_tutorial_exit_context("step_funnel_test"))), {
        step_id: "opening",
        step_index: 0,
        action: "fail",
    });
    viewOptions.onContinue();
    viewOptions.onContinue();
    window.SOW_tutorial_state_update(state);
    assert.equal(window.SOW_tutorial_exit_context("step_funnel_test"), false);
    assert.deepEqual(events, [
        { name: "tutorial_start", props: { episode_id: "step_funnel_test" } },
        {
            name: "tutorial_step",
            props: { episode_id: "step_funnel_test", step_id: "opening", step_index: 0, action: "start" },
        },
        {
            name: "tutorial_step",
            props: { episode_id: "step_funnel_test", step_id: "opening", step_index: 0, action: "complete" },
        },
        {
            name: "tutorial_step",
            props: { episode_id: "step_funnel_test", step_id: "ending", step_index: 1, action: "start" },
        },
        {
            name: "tutorial_step",
            props: { episode_id: "step_funnel_test", step_id: "ending", step_index: 1, action: "complete" },
        },
        { name: "campaign_episode_complete", props: { episode_id: "step_funnel_test" } },
    ]);
});

test("campaign decisions keep their selected branch and guard stale callbacks", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const machine = campaign.create({
        version: 2,
        episode_id: "branch_test",
        default_locale: "en",
        settings: { starting_troops: 1000 },
        strings: { en: {} },
        speakers: {},
        entry: "opening",
        steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "decision" },
            {
                id: "decision",
                type: "choice",
                title_key: "tutorial.decision",
                choices: [
                    { id: "gather", label_key: "tutorial.gather", next: "gather_step" },
                    { id: "strike", label_key: "tutorial.strike", next: "strike_step" },
                ],
            },
            { id: "gather_step", type: "scene", title_key: "tutorial.gather_step", next: "rejoin" },
            { id: "strike_step", type: "scene", title_key: "tutorial.strike_step", next: "rejoin" },
            {
                id: "rejoin",
                type: "scene",
                title_key: "tutorial.rejoin",
                next: "ending",
                routes: [{ when: { choice: "decision", equals: "strike" }, next: "strike_echo" }],
            },
            { id: "strike_echo", type: "scene", title_key: "tutorial.strike_echo", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" },
        ],
    });
    assert.equal(machine.advance(null, "opening"), true);
    assert.equal(machine.advance("strike", "decision"), true);
    assert.equal(machine.view().step.id, "strike_step");
    assert.equal(machine.advance(null, "decision"), false);
    assert.equal(machine.state.choices.decision, "strike");
    assert.equal(machine.advance(null, "strike_step"), true);
    assert.equal(machine.advance(null, "rejoin"), true);
    assert.equal(machine.view().step.id, "strike_echo");
});

test("troop objectives advance on the first update that satisfies the minimum", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const machine = campaign.create({
        version: 2,
        episode_id: "troop_minimum_test",
        default_locale: "en",
        settings: { starting_troops: 1000 },
        strings: { en: {} },
        speakers: {},
        entry: "minimum",
        steps: [
            {
                id: "minimum",
                type: "objective",
                title_key: "tutorial.minimum",
                hint_key: "tutorial.minimum_hint",
                trigger: { type: "troops", value: 1500, scope: "step" },
                next: "ending",
            },
            { id: "ending", type: "end", title_key: "tutorial.ending" },
        ],
    });
    assert.deepEqual(machine.update({ troops: 1400 }, {}, 0).progress, { current: 1400, target: 1500 });
    assert.equal(machine.update({ troops: 1500 }, {}).step.id, "ending");
});

test("a completed objective advances immediately after an exit modal closes", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const machine = campaign.create({
        version: 2,
        episode_id: "modal_pause_test",
        default_locale: "en",
        settings: { starting_troops: 1000 },
        strings: { en: {} },
        speakers: {},
        entry: "objective",
        steps: [
            {
                id: "objective",
                type: "objective",
                title_key: "tutorial.objective",
                hint_key: "tutorial.objective_hint",
                trigger: { type: "territory", value: 1, scope: "step" },
                next: "ending",
            },
            { id: "ending", type: "end", title_key: "tutorial.ending" },
        ],
    });
    machine.update({ tiles_gained: 0 }, {});
    machine.setPaused(true);
    assert.equal(machine.update({ tiles_gained: 1 }, {}).step.id, "objective");
    assert.equal(machine.advance(null, "objective"), false);
    machine.setPaused(false);
    assert.equal(machine.update({ tiles_gained: 1 }, {}).step.id, "ending");
});

test("UI objectives respect step, episode, and total scope", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const completed = (scope) => {
        const machine = campaign.create({
            version: 2,
            episode_id: "ui_scope_test",
            default_locale: "en",
            settings: { starting_troops: 0 },
            strings: { en: {} },
            speakers: {},
            entry: "opening",
            steps: [
                { id: "opening", type: "scene", title_key: "tutorial.opening", next: "objective" },
                {
                    id: "objective",
                    type: "objective",
                    title_key: "tutorial.objective",
                    hint_key: "tutorial.objective_hint",
                    trigger: { type: "ui", action: "hud_center_camera", scope },
                    next: "ending",
                },
                { id: "ending", type: "end", title_key: "tutorial.ending" },
            ],
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

test("campaign episode 1 stays locked so only the tutorial can be replayed", () => {
    const unlockStart = campaignModule.indexOf("pub fn is_unlocked(");
    const unlock = campaignModule.slice(unlockStart, campaignModule.indexOf("pub fn advisor(", unlockStart));
    assert.ok(unlockStart >= 0, "is_unlocked must exist");
    assert.match(
        unlock,
        /sow_data::campaign::episode_is_published\(episode\.id\)/,
        "campaign availability follows the shared published-episode registry",
    );
    assert.match(
        unlock,
        /episode\s*\.\s*prerequisite\s*\.\s*is_none_or\s*\(\s*\|required\|\s*progress\.completed_episodes\.contains\(required\)\s*\)/,
        "later campaign episodes follow the shared prerequisite metadata",
    );
    // A locked episode renders the same disabled card as the rest of the chain.
    // "Saga complete" may only appear when every episode is really finished.
});

test("a step's emote directive fires once, when that step completes", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const completed = [];
    const machine = campaign.create(
        {
            entry: "a",
            steps: [
                { id: "a", type: "scene", emote: "threat", emote_pinned: true, next: "b" },
                { id: "b", type: "scene", emote: "clear", next: "c" },
                { id: "c", type: "end", presentation: "chapter" },
            ],
        },
        undefined,
        undefined,
        undefined,
        { onStepComplete: (step) => completed.push(step.id) },
    );
    assert.equal(machine.advance(null, "a"), true);
    assert.deepEqual(completed, ["a"]);
    assert.equal(machine.advance(null, "b"), true);
    assert.deepEqual(completed, ["a", "b"]);
});

test("Snettisham refuse goes straight to its reply while accept waits for the Snettisham City", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const step = (id) => definition.steps.find((candidate) => candidate.id === id);
    const contact = { contact_faction_ids: ["snettisham"], tiles_gained: 250, wilderness_orders_accepted: 1 };

    assert.deepEqual(
        campaign.validate(definition, roster, { hasText: () => true }).errors,
        [],
        "the Snettisham branch is valid data",
    );
    assert.equal(step("boudica_second_contact").next, "boudica_snettisham_accept_wait");
    assert.deepEqual(step("boudica_second_contact").routes, [
        { when: { choice: "contact_snettisham", equals: "refuse" }, next: "boudica_snettisham_refuse_reply" },
    ]);
    let now = 0;
    const playOut = (machine, facts, ms) => {
        for (let t = 0; t < ms; t += 100) {
            now += 100;
            machine.update(facts, {});
        }
    };
    const spoken = [];
    const greetingMachine = campaign.create(definition, "boudica_first_contact", roster, () => now, {
        onExchangeLine: (line) => spoken.push(line.speaker + ":" + line.emote),
    });
    const greeting = greetingMachine.update({ contact_faction_ids: ["stonea"] }, {});
    assert.equal(greeting.reactionData.id, "contact_stonea");
    assert.equal(greeting.step.show_title, true, "contact responses keep their dialogue title");
    assert.equal(greetingMachine.advance(null, greeting.step.id), true);
    greetingMachine.update({ contact_faction_ids: ["stonea"] }, {});
    assert.deepEqual(spoken, ["stonea:greet"], "Stonea speaks first, as soon as her greeting closes");
    assert.equal(
        greetingMachine.advance(null, "boudica_first_contact"),
        false,
        "the step waits while its exchange plays",
    );
    playOut(greetingMachine, { contact_faction_ids: ["stonea"] }, 3300);
    assert.deepEqual(
        spoken,
        ["stonea:greet", "boudica:threat"],
        "Boudica answers after the bubble and pause",
    );
    playOut(greetingMachine, { contact_faction_ids: ["stonea"] }, 3300);
    const after = greetingMachine.update({ contact_faction_ids: ["stonea"] }, {});
    assert.equal(after.exchange, undefined, "the exchange ends after the last line");
    assert.equal(after.waiting, false);
    assert.deepEqual(step("boudica_snettisham_accept_wait").trigger, {
        type: "structure_level",
        kind: "City",
        value: 1,
        scope: "total",
        faction: "snettisham",
    });
    assert.equal(
        step("boudica_snettisham_accept_wait").pause_game,
        undefined,
        "the accept wait does not pause the game",
    );

    const refuse = campaign.create(definition, "boudica_second_contact", roster);
    const refuseView = refuse.update(contact, {});
    assert.equal(refuseView.reactionData.id, "contact_snettisham");
    assert.equal(refuse.advance("refuse", "reaction-contact_snettisham@snettisham", true), true);
    assert.equal(
        refuse.update(contact, {}).step.id,
        "boudica_snettisham_refuse_reply",
        "refuse plays its reply right after the choice",
    );

    const accept = campaign.create(definition, "boudica_second_contact", roster, () => now);
    accept.update(contact, {});
    assert.equal(accept.advance("accept", "reaction-contact_snettisham@snettisham", true), true);
    assert.equal(
        accept.update(contact, {}).step.id,
        "boudica_snettisham_accept_wait",
        "accept waits before its reply",
    );
    assert.equal(
        accept.update({ ...contact, faction_structure_levels: { snettisham: { city: 0 } } }, {}).step.id,
        "boudica_snettisham_accept_wait",
    );
    assert.equal(
        accept.update({ ...contact, structure_levels: { city: 1 } }, {}).step.id,
        "boudica_snettisham_accept_wait",
        "the player's own City does not count for Snettisham",
    );
    for (let t = 0; t < 10000; t += 100) {
        now += 100;
        accept.update(contact, {});
    }
    assert.equal(
        accept.update({ ...contact, faction_structure_levels: { snettisham: { city: 1 } } }, {}).step.id,
        "boudica_snettisham_accept_reply",
    );
});

test("a scene that only plays an exchange continues once its lines have played", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const contact = { contact_faction_ids: ["snettisham"], tiles_gained: 250, wilderness_orders_accepted: 1 };
    let now = 0;
    const spoken = [];
    const machine = campaign.create(definition, "boudica_snettisham_refuse_reply", roster, () => now, {
        onExchangeLine: (line) => spoken.push(line.speaker),
    });

    const playOut = (ms) => {
        for (let t = 0; t < ms; t += 100) {
            now += 100;
            machine.update(contact, {});
        }
    };
    assert.equal(machine.update(contact, {}).exchange.speaker, "snettisham");
    playOut(3300);
    assert.equal(machine.update(contact, {}).exchange.speaker, "boudica");
    playOut(3300);
    const after = machine.update(contact, {});
    assert.equal(after.exchange, undefined);
    assert.notEqual(after.step.id, "boudica_snettisham_refuse_reply", "the reply scene is over");
    assert.deepEqual(spoken, ["snettisham", "boudica"]);
});

test("active-objective exchanges wait for new contact, honor the reply pause, and run once", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const gate = "boudica_outpost_colonia_veterans";
    const romanTargets = ["colonia_veterans", "tax_collectors", "roman_supply_depot"];
    definition.reactions = definition.reactions.filter(
        (reaction) =>
            !(
                ["contact", "pre_defeat"].includes(reaction.when.type) &&
                romanTargets.includes(reaction.when.target)
            ),
    );
    definition.speakers.colonia_veterans = { faction: "colonia_veterans" };
    definition.reactions.push({
        id: "colonia_contact_exchange",
        when: { type: "contact", target: "colonia_veterans" },
        during: gate,
        presentation: "exchange",
        exchange: [
            {
                speaker: "colonia_veterans",
                presentation: "emote",
                emote: "threat",
                emote_text_key: "tutorial.colonia_taunt",
                delay_ms: 700,
            },
            {
                speaker: "boudica",
                presentation: "emote",
                emote: "threat",
                emote_text_key: "tutorial.boudica_reply",
                delay_ms: 0,
            },
        ],
    });

    let now = 0;
    const priorContact = campaign.create(definition, gate, roster, () => now);
    priorContact.update({ contact_faction_ids: [] }, {});
    priorContact.jump(gate, { contact_faction_ids: ["colonia_veterans"] });
    assert.equal(
        priorContact.update({ contact_faction_ids: ["colonia_veterans"] }, {}).reaction,
        undefined,
        "a contact that predates this objective does not provoke a second greeting",
    );

    const spoken = [],
        completed = [];
    const machine = campaign.create(definition, gate, roster, () => now, {
        onExchangeLine: (line) => {
            spoken.push(line.speaker);
            return true;
        },
        onReactionComplete: (reaction) => completed.push(reaction.id),
    });
    machine.update({ contact_faction_ids: [] }, {});
    machine.jump(gate, { contact_faction_ids: [] });
    assert.deepEqual(
        machine.view().pre_defeat_targets,
        [],
        "no final-capture watch exists without an authored pre-defeat response",
    );
    const first = machine.update({ contact_faction_ids: ["colonia_veterans"] }, {});
    assert.equal(first.exchange.speaker, "colonia_veterans");
    assert.deepEqual(spoken, ["colonia_veterans"]);
    machine.update({ contact_faction_ids: ["colonia_veterans"] }, {});
    assert.deepEqual(spoken, ["colonia_veterans"], "repeated state updates do not replay the active line");

    now = 3000;
    machine.update({ contact_faction_ids: ["colonia_veterans"] }, {});
    now = 3699;
    machine.update({ contact_faction_ids: ["colonia_veterans"] }, {});
    assert.deepEqual(
        spoken,
        ["colonia_veterans"],
        "Boudica waits for the configured 700 ms after the Roman bubble",
    );
    now = 3700;
    assert.equal(
        machine.update({ contact_faction_ids: ["colonia_veterans"] }, {}).exchange.speaker,
        "boudica",
    );
    now = 6700;
    machine.update({ contact_faction_ids: ["colonia_veterans"] }, {});
    machine.update({ contact_faction_ids: ["colonia_veterans"] }, {});
    assert.deepEqual(spoken, ["colonia_veterans", "boudica"]);
    assert.deepEqual(completed, ["colonia_contact_exchange"], "the whole response completes once");
});

test("campaign contact, rendered emotes, delayed reply, last words, and defeat continuation connect end to end", () => {
    assert.ok(
        tutorial.includes("onReactionComplete: completeCampaignReaction"),
        "the real campaign adapter wires the completion callback",
    );
    assert.match(
        tutorial,
        /send\("set_campaign_pre_defeat_targets",\s*\{\s*faction_ids: preDefeatTargets,\s*campaign_session_id: campaignSessionId,?\s*\}\)/,
        "the active objective arms the native final-capture hold for its authored target",
    );
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const target = "colonia_veterans";
    const gate = "boudica_outpost_colonia_veterans";
    const romanSpeaker = "roman_colonia_veterans";
    const romanTargets = ["colonia_veterans", "tax_collectors", "roman_supply_depot"];
    definition.reactions = definition.reactions.filter(
        (reaction) =>
            !(
                ["contact", "pre_defeat"].includes(reaction.when.type) &&
                romanTargets.includes(reaction.when.target)
            ),
    );
    definition.speakers[romanSpeaker] = { faction: target };
    definition.reactions.push(
        {
            id: "test_colonia_contact_exchange",
            when: { type: "contact", target },
            during: gate,
            presentation: "exchange",
            exchange: [
                {
                    speaker: romanSpeaker,
                    presentation: "emote",
                    emote: "threat",
                    emote_text_key: "test.colonia.taunt",
                    delay_ms: 700,
                },
                {
                    speaker: "boudica",
                    presentation: "emote",
                    emote: "threat",
                    emote_text_key: "test.colonia.reply",
                    delay_ms: 0,
                },
            ],
        },
        {
            id: "test_colonia_pre_defeat",
            when: { type: "pre_defeat", target },
            during: gate,
            presentation: "exchange",
            exchange: [
                {
                    speaker: romanSpeaker,
                    presentation: "dialogue",
                    body_key: "test.colonia.last_words",
                    delay_ms: 0,
                },
            ],
        },
    );

    let now = 0;
    const commands = [];
    const players = {
        player: { id: 1, is_alive: true, centroid_x: 4, centroid_y: 4 },
        [target]: { id: 9, is_alive: true, centroid_x: 8, centroid_y: 8 },
    };
    const runtime = {
        definition,
        generation: 6,
        campaignSessionId: 41,
        latestHud: {
            tutorial: {
                campaign_session_id: 41,
                marker_player_id: 9,
                target_nameplate: { offscreen: false },
                facts: {},
            },
        },
        exchangeCue: null,
        exchangeFocusToken: null,
    };
    const context = {
        runtime,
        Date,
        send: (type, fields) => {
            commands.push({ type, ...fields });
            if (type === "set_tutorial_marker")
                runtime.latestHud.tutorial.marker_player_id = fields.player_id;
            return true;
        },
        tr: (key) => "localized:" + key,
        campaignPlayer: (_hud, faction) => players[faction] || null,
        onMap: (player) => player.is_alive,
        cameraTargetAnchor: () => ({ offscreen: false }),
    };
    const start = tutorial.indexOf("    function playExchangeLine(");
    const end = tutorial.indexOf("    function continueScene(", start);
    vm.runInNewContext(
        tutorial.slice(start, end) + "this.play = playExchangeLine; this.clear = clearExchangeLine;",
        context,
    );
    const completionStart = tutorial.indexOf("    function completeCampaignReaction(");
    const completionEnd = tutorial.indexOf("    function continueScene(", completionStart);
    vm.runInNewContext(
        tutorial.slice(completionStart, completionEnd) + "this.complete = completeCampaignReaction;",
        context,
    );

    const machine = campaign.create(definition, gate, roster, () => now, {
        onExchangeLine: (line, token) => context.play(line, token),
        onExchangeClear: (line, token) => context.clear(line, token),
        onReactionComplete: context.complete,
    });
    runtime.machine = machine;
    const contactFacts = {
        contact_faction_ids: [target],
        defeated_faction_ids: [],
        pre_defeat_faction_ids: [],
    };
    machine.update({ ...contactFacts, contact_faction_ids: [] }, {});
    machine.jump(gate, { ...contactFacts, contact_faction_ids: [] });
    // The outpost camera shot has played on entry; its emotes wait for that shot, as in the live runtime.
    runtime.cameraShotStepId = gate;

    let view = machine.update(contactFacts, {});
    const romanCue = commands.find(
        (command) => command.type === "tutorial_emote" && command.message === "localized:test.colonia.taunt",
    );
    assert.ok(romanCue, "the active contact emits the Roman line through the game's command path");
    assert.equal(romanCue.faction_id, target);
    assert.equal(view.exchange_phase, "prepare", "the line waits for the native nameplate's exact token");
    machine.update(contactFacts, {});
    assert.equal(
        commands.filter(
            (command) => command.type === "tutorial_emote" && command.message === romanCue.message,
        ).length,
        1,
        "polling for the native acknowledgement never duplicates the line",
    );

    runtime.latestHud.tutorial.emote_token = romanCue.token;
    view = machine.update(contactFacts, {});
    assert.equal(view.exchange_phase, "speaking");
    now = 3000;
    view = machine.update(contactFacts, {});
    assert.equal(view.exchange_phase, "gap");
    assert.ok(
        commands.some(
            (command) =>
                command.type === "tutorial_emote" &&
                command.kind === null &&
                command.token === romanCue.token,
        ),
        "the Roman bubble is cleared before Boudica can speak",
    );
    now = 3699;
    machine.update(contactFacts, {});
    assert.equal(
        commands.some(
            (command) =>
                command.type === "tutorial_emote" && command.message === "localized:test.colonia.reply",
        ),
        false,
        "Boudica waits the configured 700 ms after the Roman bubble",
    );
    now = 3700;
    view = machine.update(contactFacts, {});
    const boudicaCue = commands.find(
        (command) => command.type === "tutorial_emote" && command.message === "localized:test.colonia.reply",
    );
    assert.ok(boudicaCue);
    assert.equal(view.exchange.speaker, "boudica");
    runtime.latestHud.tutorial.emote_token = boudicaCue.token;
    assert.equal(machine.update(contactFacts, {}).exchange_phase, "speaking");
    now = 6700;
    machine.update(contactFacts, {});
    machine.update(contactFacts, {});

    const preDefeatFacts = { ...contactFacts, pre_defeat_faction_ids: [target] };
    view = machine.update(preDefeatFacts, {});
    assert.equal(view.step.presentation, "dialogue");
    assert.equal(view.step.body_key, "test.colonia.last_words");
    assert.equal(machine.advance(null, view.step.id), true);
    assert.deepEqual(
        commands
            .filter((command) => command.type === "continue_campaign_pre_defeat")
            .map((command) => [command.faction_id, command.campaign_session_id]),
        [[target, 41]],
        "only the completed last-words exchange releases this Roman's held capture",
    );
    view = machine.update({ ...preDefeatFacts, defeated_faction_ids: [target] }, {});
    assert.equal(
        view.step.id,
        "boudica_outpost_tax_collectors",
        "the original defeat then advances the campaign objective",
    );
});

test("pre-defeat exchange uses the shared modal and releases only after its final line", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const gate = "boudica_outpost_colonia_veterans";
    const romanTargets = ["colonia_veterans", "tax_collectors", "roman_supply_depot"];
    definition.reactions = definition.reactions.filter(
        (reaction) =>
            !(
                ["contact", "pre_defeat"].includes(reaction.when.type) &&
                romanTargets.includes(reaction.when.target)
            ),
    );
    definition.speakers.colonia_veterans = { faction: "colonia_veterans" };
    definition.reactions.push({
        id: "colonia_pre_defeat",
        when: { type: "pre_defeat", target: "colonia_veterans" },
        during: gate,
        presentation: "exchange",
        exchange: [
            {
                speaker: "colonia_veterans",
                presentation: "dialogue",
                body_key: "tutorial.colonia_last_words",
                delay_ms: 0,
            },
        ],
    });
    const now = 0;
    let released = 0;
    const machine = campaign.create(definition, gate, roster, () => now, {
        onReactionComplete: (reaction) => {
            if (reaction.id === "colonia_pre_defeat") released++;
        },
    });
    machine.jump(gate, { defeated_faction_ids: [], pre_defeat_faction_ids: [] });
    assert.deepEqual(machine.view().pre_defeat_targets, ["colonia_veterans"]);
    const held = machine.update(
        { defeated_faction_ids: [], pre_defeat_faction_ids: ["colonia_veterans"] },
        {},
    );
    assert.equal(held.exchange.presentation, "dialogue");
    assert.equal(held.waiting, false, "the existing campaign dialogue viewer owns a long final line");
    assert.equal(machine.advance(null, "wrong-exchange-step"), false);
    assert.equal(machine.advance(null, held.step.id), true);
    assert.equal(released, 1, "the final-capture hold releases after the shared modal is continued");
    assert.deepEqual(
        machine.update(
            { defeated_faction_ids: ["colonia_veterans"], pre_defeat_faction_ids: ["colonia_veterans"] },
            {},
        ).step.id,
        "boudica_outpost_tax_collectors",
        "the same defeated fact advances the objective after the authored line",
    );
});

test("a missing emote delivery is skipped without a modal and cannot hold a defeat forever", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const definition = {
        entry: "outpost",
        speakers: { roman: { faction: "colonia_veterans" } },
        steps: [
            {
                id: "outpost",
                type: "objective",
                trigger: { type: "defeated", target: "colonia_veterans", scope: "episode" },
                next: "next",
            },
            { id: "next", type: "end" },
        ],
        reactions: [
            {
                id: "last_words",
                when: { type: "pre_defeat", target: "colonia_veterans" },
                during: "outpost",
                presentation: "exchange",
                exchange: [
                    {
                        speaker: "roman",
                        presentation: "emote",
                        emote: "threat",
                        emote_text_key: "tutorial.last_words",
                        delay_ms: 0,
                    },
                ],
            },
        ],
    };
    const cleared = [],
        completed = [];
    const machine = campaign.create(definition, undefined, undefined, () => 0, {
        onExchangeLine: () => "unavailable",
        onExchangeClear: (line, token) => cleared.push([line.emote_text_key, token]),
        onReactionComplete: (reaction) => completed.push(reaction.id),
    });
    machine.jump("outpost", { defeated_faction_ids: [], pre_defeat_faction_ids: [] });
    const model = machine.update(
        { defeated_faction_ids: [], pre_defeat_faction_ids: ["colonia_veterans"] },
        {},
    );
    assert.equal(
        model.exchange_phase,
        undefined,
        "an emote that cannot show is skipped, not turned into a modal",
    );
    assert.equal(model.paused, false, "a skipped emote never pauses the game");
    assert.deepEqual(
        cleared.map((item) => item[0]),
        ["tutorial.last_words"],
    );
    assert.deepEqual(completed, ["last_words"], "skipping the last emote completes its response");
    assert.deepEqual(
        machine.view().pre_defeat_targets,
        [],
        "the story hold is disarmed as soon as its response completes",
    );
});

test("a scripted emote that never receives a native render acknowledgement is skipped after a bounded wait", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    let now = 0,
        calls = 0;
    const definition = {
        entry: "talk",
        speakers: { roman: { faction: "colonia_veterans" } },
        steps: [
            {
                id: "talk",
                type: "scene",
                exchange: [
                    {
                        speaker: "roman",
                        presentation: "emote",
                        emote: "threat",
                        emote_text_key: "tutorial.taunt",
                        delay_ms: 0,
                    },
                ],
                next: "done",
            },
            { id: "done", type: "end" },
        ],
    };
    const machine = campaign.create(definition, undefined, undefined, () => now, {
        onExchangeLine: () => (++calls === 1 ? false : "pending"),
    });
    machine.update({}, {});
    now = 100;
    assert.equal(machine.update({}, {}).exchange_phase, "prepare");
    now = 2599;
    assert.equal(
        machine.update({}, {}).exchange_phase,
        "prepare",
        "the renderer gets time to acknowledge the visible line",
    );
    now = 2600;
    const fallback = machine.update({}, {});
    assert.notEqual(
        fallback.exchange_phase,
        "dialogue",
        "an unacknowledged emote never opens a pausing modal",
    );
    assert.equal(fallback.step.id, "done", "the unacknowledged emote is skipped and the sequence moves on");
});

test("the three Roman encounters speak once on their active objective and release that defeat after the last word", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const encounters = [
        {
            target: "colonia_veterans",
            gate: "boudica_outpost_colonia_veterans",
            next: "boudica_outpost_tax_collectors",
            last: "dialogue",
        },
        {
            target: "tax_collectors",
            gate: "boudica_outpost_tax_collectors",
            next: "boudica_outpost_roman_supply_depot",
            last: "emote",
        },
        {
            target: "roman_supply_depot",
            gate: "boudica_outpost_roman_supply_depot",
            next: "boudica_trinovantes_contact",
            last: "dialogue",
        },
    ];
    const romanTargets = ["colonia_veterans", "tax_collectors", "roman_supply_depot"];
    definition.reactions = definition.reactions.filter(
        (reaction) =>
            !(
                ["contact", "pre_defeat"].includes(reaction.when.type) &&
                romanTargets.includes(reaction.when.target)
            ),
    );
    const textKeys = new Map();
    for (const encounter of encounters) {
        const roman = "roman_" + encounter.target;
        definition.speakers[roman] = { faction: encounter.target };
        const taunt = "test." + encounter.target + ".taunt";
        const reply = "test." + encounter.target + ".reply";
        const lastWord = "test." + encounter.target + ".last_words";
        textKeys.set(encounter.target, { roman, taunt, reply, lastWord });
        definition.reactions.push(
            {
                id: "test_" + encounter.target + "_contact",
                when: { type: "contact", target: encounter.target },
                during: encounter.gate,
                presentation: "exchange",
                exchange: [
                    {
                        speaker: roman,
                        presentation: "emote",
                        emote: "threat",
                        emote_text_key: taunt,
                        delay_ms: 700,
                    },
                    {
                        speaker: "boudica",
                        presentation: "emote",
                        emote: "threat",
                        emote_text_key: reply,
                        delay_ms: 0,
                    },
                ],
            },
            {
                id: "test_" + encounter.target + "_pre_defeat",
                when: { type: "pre_defeat", target: encounter.target },
                during: encounter.gate,
                presentation: "exchange",
                exchange: [
                    encounter.last === "dialogue"
                        ? { speaker: roman, presentation: "dialogue", body_key: lastWord, delay_ms: 0 }
                        : {
                              speaker: roman,
                              presentation: "emote",
                              emote: "help",
                              emote_text_key: lastWord,
                              delay_ms: 0,
                          },
                ],
            },
        );
    }

    let now = 0;
    const delivered = [],
        released = [];
    const machine = campaign.create(definition, encounters[0].gate, roster, () => now, {
        onExchangeLine: (line) => {
            delivered.push(line.emote_text_key);
            return true;
        },
        onReactionComplete: (reaction, target) => {
            if (reaction.when.type === "pre_defeat") released.push(target);
        },
    });
    let contacts = [],
        defeated = [];
    machine.update({ contact_faction_ids: contacts, defeated_faction_ids: defeated }, {});

    for (const encounter of encounters) {
        const { target, gate, next } = encounter;
        const keys = textKeys.get(target);
        assert.equal(machine.view().step.id, gate);
        assert.deepEqual(
            machine.view().pre_defeat_targets,
            [target],
            target + " is watched only while its objective is active",
        );

        contacts = contacts.concat(target);
        const contactFacts = { contact_faction_ids: contacts, defeated_faction_ids: defeated };
        const firstLine = delivered.length;
        assert.equal(machine.update(contactFacts, {}).exchange.speaker, keys.roman);
        now += 3000;
        machine.update(contactFacts, {});
        now += 699;
        assert.equal(
            machine.update(contactFacts, {}).exchange.speaker,
            keys.roman,
            "the reply waits the configured 700 ms",
        );
        now += 1;
        assert.equal(machine.update(contactFacts, {}).exchange.speaker, "boudica");
        now += 3000;
        machine.update(contactFacts, {});
        machine.update(contactFacts, {});
        assert.deepEqual(
            delivered.slice(firstLine),
            [keys.taunt, keys.reply],
            target + " completes one Roman/Boudica exchange",
        );
        assert.equal(
            machine.update(contactFacts, {}).exchange,
            undefined,
            target + " does not replay on repeated contact facts",
        );

        const finalFacts = { ...contactFacts, pre_defeat_faction_ids: [target] };
        const lastLine = machine.update(finalFacts, {});
        assert.equal(lastLine.exchange.speaker, keys.roman);
        assert.equal(lastLine.exchange.presentation, encounter.last);
        if (encounter.last === "dialogue") {
            assert.equal(machine.advance(null, lastLine.step.id), true);
        } else {
            assert.equal(machine.advance(null, lastLine.step.id), false, "the last emote cannot be skipped");
            now += 3000;
            machine.update(finalFacts, {});
            machine.update(finalFacts, {});
        }
        assert.deepEqual(
            released,
            defeated.concat(target),
            target + " releases only after its own final line",
        );
        assert.equal(machine.view().step.id, gate, "the objective cannot advance before defeat is recorded");

        defeated = defeated.concat(target);
        assert.equal(machine.update({ ...finalFacts, defeated_faction_ids: defeated }, {}).step.id, next);
    }
});

test("Boudica camera lessons leave every gesture to the player", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const step = (id) => definition.steps.find((candidate) => candidate.id === id);
    for (const id of [
        "boudica_camera_intro",
        "boudica_zoom_out",
        "boudica_camera_drag",
        "boudica_camera_hover",
        "boudica_zoom_in",
        "boudica_camera_home",
    ]) {
        assert.equal(
            Object.hasOwn(step(id), "camera"),
            false,
            id + " does not move the camera for the player",
        );
    }
    for (const shot of definition.steps.filter((candidate) => candidate.camera)) {
        assert.ok(shot.camera.seconds <= 5, shot.id + " stays a light move");
    }
    assert.equal(
        step("boudica_camera_drag").trigger.type,
        "camera_target",
        "drag still needs the player to move toward Suetonius",
    );
    assert.equal(step("boudica_camera_hover").trigger.type, "hover");
    assert.equal(step("boudica_zoom_in").trigger.type, "zoom_in_complete");
    assert.equal(step("boudica_camera_home").trigger.action, "hud_center_camera");
    assert.deepEqual(campaign.validate(definition, roster, { hasText: () => true }).errors, []);
});

test("structure triggers can watch a campaign faction and the faction flag is validated", () => {
    const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
    const campaignDir = path.join(shell, "../../assets/campaign");
    const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, "boudica.triggers.json"), "utf8"));
    const errorsFor = (change) => {
        const copy = structuredClone(definition);
        change(copy);
        return campaign.validate(copy, roster, { hasText: () => true }).errors.map((issue) => issue.field);
    };
    assert.deepEqual(
        errorsFor(() => {}),
        [],
    );
    assert.ok(
        errorsFor((copy) => {
            copy.steps.find((s) => s.id === "boudica_snettisham_accept_wait").trigger.faction = "nobody";
        }).includes("trigger.faction"),
    );
    assert.ok(
        errorsFor((copy) => {
            copy.steps.find((s) => s.id === "boudica_snettisham_accept_wait").trigger.faction = 3;
        }).includes("trigger.faction"),
    );
    assert.ok(
        errorsFor((copy) => {
            copy.steps.find((s) => s.id === "boudica_build_city").trigger.faction = "snettisham";
        }).includes("trigger.faction"),
    );
    assert.ok(
        errorsFor((copy) => {
            copy.steps.find((s) => s.id === "boudica_second_contact").routes[0].when.equals = "maybe";
        }).includes("routes"),
    );
    const rosterCopy = structuredClone(roster);
    rosterCopy.factions.find((faction) => faction.id === "snettisham").hold_structures_until_contact = "yes";
    assert.ok(
        campaign
            .validate(definition, rosterCopy, { hasText: () => true })
            .errors.some((issue) => issue.field === "roster.factions.hold_structures_until_contact"),
    );
    assert.equal(
        roster.factions.find((faction) => faction.id === "snettisham").hold_structures_until_contact,
        true,
    );
});

test("every Boudica entity objective films its target and speaks an emote", () => {
    const definition = JSON.parse(
        fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"),
    );
    const hasEmote = (lines) => (lines || []).some((line) => line.emote && line.emote_text_key);
    const entities = definition.steps.filter(
        (step) =>
            step.type === "objective" &&
            !step.camera_only &&
            ((step.trigger && step.trigger.target) || step.camera),
    );
    assert.ok(entities.length >= 15, "the campaign keeps its entity objectives");
    const missing = entities.flatMap((step) => {
        const targets = [step.trigger && step.trigger.target, step.camera && step.camera.to].filter(Boolean);
        const previous = definition.steps[definition.steps.indexOf(step) - 1];
        const previousFilmsTarget = previous?.camera && targets.includes(previous.camera.to);
        const reactionEmote = (definition.reactions || []).some(
            (reaction) =>
                targets.includes(reaction.when.target) &&
                (!reaction.during || reaction.during === step.id) &&
                hasEmote(reaction.exchange),
        );
        // A contact objective's encounter is the step it leads to: its greeting speaks the emote.
        const nextEmote = hasEmote(
            definition.steps.find((candidate) => candidate.id === step.next)?.exchange,
        );
        const problems = [];
        if (!step.camera && !previousFilmsTarget) problems.push("camera");
        if (!hasEmote(step.exchange) && !hasEmote(previous?.exchange) && !reactionEmote && !nextEmote)
            problems.push("emote");
        return problems.map((problem) => `${step.id}: ${problem}`);
    });
    assert.deepEqual(
        missing,
        [],
        "each entity objective has a camera move and an emote on the objective, its contact reaction or the encounter that follows",
    );
});
