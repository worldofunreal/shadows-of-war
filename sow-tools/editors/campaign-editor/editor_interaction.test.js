const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const shell = path.resolve(__dirname, "../../../sow-web/shell");
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const campaignEngine = fs.readFileSync(path.join(shell, "sow-campaign.js"), "utf8");
const campaignView = fs.readFileSync(path.join(shell, "sow-campaign-view.js"), "utf8");
const storyCss = fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
const campaignEditor = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/editor.js"), "utf8");
const localeRuntime = fs.readFileSync(path.join(shell, "sow-i18n.js"), "utf8");
const campaignEditorServer = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/server.mjs"), "utf8");
const campaignEditorHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/logic.html"), "utf8");
const campaignEditorCss = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/editor.css"), "utf8");
const mapRostersHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/map-rosters.html"), "utf8");
const atlasEntityPickerSource = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/atlas-entity-picker.js"), "utf8");
const mapRosterWriter = fs.readFileSync(path.join(shell, "../../sow-tools/src/map_roster.rs"), "utf8");
const gameFonts = fs.readFileSync(path.join(shell, "../../sow-web/site/fonts/fonts.css"), "utf8");
const campaignMapEditorHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/index.html"), "utf8");
const campaignMapPreview = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/map-preview.js"), "utf8");
const campaignMapPreviewApi = require(path.join(shell, "../../sow-tools/editors/campaign-editor/map-preview.js"));
const atlasHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/atlas.html"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");

test("campaign validation rejects unsupported rules and duplicate event responses", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const tapGuide = {
        version: 2, episode_id: "tap_copy", settings: { starting_troops: 1000 },
        entry: "guide", speakers: {}, steps: [
            { id: "guide", type: "guide", title_key: "tutorial.tap_title", trigger: { type: "ui", action: "hud_center_camera", scope: "step" }, guide: { kind: "ui", target: "hud_center_camera", gesture: "tap" }, next: "end" },
            { id: "end", type: "end", title_key: "tutorial.tap_end" }
        ]
    };
    assert.deepEqual(campaign.validate(tapGuide, null, { hasText: key => ["tutorial.tap_title", "tutorial.tap_end"].includes(key) }).errors, [], "missing optional hand labels do not block campaign saves");
    const labelOptOut = JSON.parse(JSON.stringify(tapGuide));
    labelOptOut.steps[0].guide.show_label = false;
    assert.deepEqual(campaign.validate(labelOptOut, null, { hasText: key => ["tutorial.tap_title", "tutorial.tap_end"].includes(key) }).errors, [], "show_label false remains an explicit valid exception");
    const roster = { map: "test", player_spawn: [0, 0], factions: [{ id: "ally", name: "Ally", x: 1, y: 0, starting_troops: 500, relation: "neutral", civ: "Iceni Kingdom", leader: "Boudica", support_interval_seconds: 5 }] };
    const definition = {
        version: 2, episode_id: "fact_test", default_locale: "en", entry: "objective",
        settings: { starting_troops: 1000 },
        speakers: {},
        steps: [
            { id: "objective", type: "objective", title_key: "tutorial.objective", unlocks: { actions: ["transport_fleet"] }, trigger: { type: "fleet", unit: "TransportShip", target: "ally", value: 1, scope: "step" }, marker: { target: "ally" }, guide: { kind: "world", target: "target_action", gesture: "tap" }, next: "end" },
            { id: "end", type: "end", title_key: "tutorial.end" }
        ],
        reactions: [{ id: "aid", after: "objective", when: { type: "support", target: "ally" }, title_key: "tutorial.aid_title", body_key: "tutorial.aid_body" }]
    };
    assert.deepEqual(campaign.validate(definition, roster).errors, []);
    assert.deepEqual(campaign.TEAMS, ["Red", "Blue"]);
    const teamedRoster = JSON.parse(JSON.stringify(roster));
    for (const team of campaign.TEAMS) {
        teamedRoster.factions[0].team = team;
        assert.deepEqual(campaign.validate(definition, teamedRoster).errors, []);
    }
    teamedRoster.factions[0].team = "Romans";
    assert.ok(campaign.validate(definition, teamedRoster).errors.some(issue => issue.field === "roster.factions.team"));
    const removedCombatBehavior = JSON.parse(JSON.stringify(roster));
    removedCombatBehavior.factions[0].hostility = "passive";
    assert.ok(campaign.validate(definition, removedCombatBehavior).errors.some(issue => issue.field === "roster.factions"));
    const legacyTransportTarget = JSON.parse(JSON.stringify(definition));
    delete legacyTransportTarget.steps[0].trigger.unit;
    assert.deepEqual(campaign.validate(legacyTransportTarget, roster).errors, [], "version 2 target-only fleet goals remain valid");
    const invalidUnit = JSON.parse(JSON.stringify(definition));
    invalidUnit.steps[0].trigger.unit = "Submarine";
    assert.ok(campaign.validate(invalidUnit, roster).errors.some(issue => issue.field === "trigger.unit"));
    const invalidTarget = JSON.parse(JSON.stringify(definition));
    invalidTarget.steps[0].trigger.target = "missing_legion";
    assert.ok(campaign.validate(invalidTarget, roster).errors.some(issue => issue.field === "trigger.target"));
    const invalidRecipient = JSON.parse(JSON.stringify(definition));
    invalidRecipient.steps[0].trigger = { type: "resource_transfer", recipient: "missing_faction", resources: ["gold"], value: 1, scope: "step" };
    assert.ok(campaign.validate(invalidRecipient, roster).errors.some(issue => issue.field === "trigger.recipient"));
    const invalidResources = JSON.parse(JSON.stringify(definition));
    invalidResources.steps[0].trigger = { type: "resource_transfer", recipient: "ally", resources: ["crowns"], value: 1, scope: "step" };
    assert.ok(campaign.validate(invalidResources, roster).errors.some(issue => issue.field === "trigger.resources"));
    const duplicate = JSON.parse(JSON.stringify(definition));
    duplicate.reactions.push({ ...duplicate.reactions[0], id: "aid_again" });
    assert.ok(campaign.validate(duplicate, roster).errors.some(issue => issue.field === "reactions.when"));
    const contactResponse = JSON.parse(JSON.stringify(definition));
    contactResponse.reactions.push({ id: "contact", when: { type: "contact", target: "ally" }, title_key: "tutorial.aid_title", body_key: "tutorial.aid_body" });
    assert.deepEqual(campaign.validate(contactResponse, roster).errors, [], "contact and support can each have their own response");
    const duplicateContact = JSON.parse(JSON.stringify(contactResponse));
    duplicateContact.reactions.push({ ...duplicateContact.reactions[1], id: "contact_again" });
    assert.ok(campaign.validate(duplicateContact, roster).errors.some(issue => issue.field === "reactions.when"));
    const delayedContact = JSON.parse(JSON.stringify(contactResponse));
    delayedContact.reactions[1].after = "missing_objective";
    assert.ok(campaign.validate(delayedContact, roster).errors.some(issue => issue.field === "reactions.after"));
});


test("campaign editor exposes and preserves the existing team enum", () => {
    const teamSpread = /\.\.\.\(f\.team\?\{team:f\.team\}:\{\}\)/g;
    assert.deepEqual(require(path.join(shell, "sow-campaign.js")).TEAMS, ["Red", "Blue"]);
    assert.match(campaignMapEditorHtml, /SOWCampaign\.TEAMS\.map/);
    assert.match(campaignMapEditorHtml, /id="fteam"/);
    assert.match(campaignMapEditorHtml, /f\.team=e\.target\.value/);
    assert.equal((campaignMapEditorHtml.match(teamSpread) || []).length, 2, "import and export both retain an assigned team");
    assert.match(campaignMapEditorHtml, /factionFields = new Set\(\[[^\]]*"team"/);
});


test("campaign editor refresh serves current source files without browser caching", () => {
    assert.match(campaignEditorServer, /pathname === "\/tools\/campaign-editor\/"[\s\S]*?path\.join\(editorDir, "index\.html"\)/);
    assert.match(campaignEditorServer, /pathname\.startsWith\("\/tools\/campaign-editor\/"\)\) return safeFile\(editorDir/);
    assert.match(campaignEditorServer, /const body = await fs\.readFile\(file\)/);
    assert.match(campaignEditorServer, /"Cache-Control": "no-store"/);
});


test("campaign saves use the current validator and call Save by its real action", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "save_test", text_namespace: "tutorial.save_test_",
        settings: { starting_troops: 1000 }, speakers: {}, entry: "zoom_out",
        steps: [
            { id: "zoom_out", type: "objective", title_key: "tutorial.zoom_out", trigger: { type: "zoom_out_complete", value: 0.85, scope: "step" }, pause_game: true, camera_only: true, next: "end" },
            { id: "end", type: "end", title_key: "tutorial.end" }
        ]
    };
    const roster = { map: "eastanglia", player_spawn: [10, 10], factions: [{ id: "test_faction", name: "Test", x: 20, y: 20, starting_troops: 500, relation: "neutral", civ: "Iceni Kingdom", leader: "Boudica" }] };
    const zoomOut = definition.steps.find(step => step.id === "zoom_out");
    assert.equal(zoomOut.pause_game, true);
    const report = campaign.validate(definition, roster, { allowMissingFactionReferences: true, hasText: () => true, hasAvatar: () => true });
    assert.deepEqual(report.errors.filter(issue => issue.step === zoomOut.id), []);
    assert.equal(definition.text_namespace, "tutorial.save_test_");
    assert.equal(report.errors.some(issue => issue.field === "campaign" && issue.message.includes("text_namespace")), false);
    assert.match(campaignEditor, /function storyKey\(suffix\) \{ return \(state\.definition\.text_namespace \|\| "tutorial\."\) \+ suffix; \}/);
    assert.match(campaignEditor, /catalogTextField\(container, label, key, rows\)/);
    assert.match(campaignEditor, /if \(state\.isMenuGuides\)/);
    assert.match(campaignEditorServer, /key\.replace\(\/\^tutorial\\\./);
    assert.match(campaignEditorServer, /sow-i18n\/strings\/en\/web\.toml/);
    assert.match(campaignEditor, /Allow camera controls only while paused/);
    assert.match(campaignEditor, /Action allowed while paused/);
    assert.match(campaignEditor, /if \(keepsMechanics && step\.paused_action\) replacement\.paused_action = step\.paused_action;/);
    assert.match(campaignEditor, /state\.facts\.touch_controls = \$\("#device"\)\.value === "mobile" \? 1 : 0/);
    const unpausedCameraOnly = JSON.parse(JSON.stringify(definition));
    unpausedCameraOnly.steps.find(step => step.id === zoomOut.id).pause_game = false;
    assert.ok(campaign.validate(unpausedCameraOnly, roster, { allowMissingFactionReferences: true, hasText: () => true, hasAvatar: () => true }).errors.some(issue => issue.step === zoomOut.id && issue.field === "camera_only"));
    const invalidPausedAction = JSON.parse(JSON.stringify(definition));
    Object.assign(invalidPausedAction.steps[0], { pause_game: true, paused_action: "send_resources_stay_paused" });
    assert.ok(campaign.validate(invalidPausedAction, roster, { allowMissingFactionReferences: true, hasText: () => true, hasAvatar: () => true }).errors.some(issue => issue.step === zoomOut.id && issue.field === "paused_action"));

    const pausedScene = JSON.parse(JSON.stringify(definition));
    pausedScene.steps.find(step => step.id === zoomOut.id).type = "scene";
    const sceneReport = campaign.validate(pausedScene, roster, { allowMissingFactionReferences: true, hasText: () => true, hasAvatar: () => true });
    assert.equal(sceneReport.errors.some(issue => issue.step === zoomOut.id && issue.field === "fields" && issue.message.includes("pause_game")), false,
        "pause_game is accepted on every step type where the runtime reads it");

    assert.match(campaignEditorServer, /function loadCampaignRuntime\(\) \{\s*const modulePath = campaignRequire\.resolve\(campaignFile\);\s*delete campaignRequire\.cache\[modulePath\];\s*return campaignRequire\(modulePath\);\s*\}/);
    assert.match(campaignEditorServer, /loadCampaignRuntime\(\)\.validate\(definition, roster,/);
    assert.match(campaignEditorServer, /renameFactionText\([\s\S]*?validatePair\(episodeId, value, definition\)/);
    assert.match(campaignMapEditorHtml, /<h2>Save<\/h2>[\s\S]*?<button class="primary" id="exportBtn">Save<\/button>/);
    assert.match(campaignMapEditorHtml, /<button id="reloadBtn">Reload saved<\/button>/);
    assert.match(campaignMapEditorHtml, /faction_renames/);
    assert.match(campaignMapEditorHtml, /async function saveRoster\(\)/);
    assert.match(campaignMapEditorHtml, /Save failed: /);
    assert.match(campaignEditorHtml, /<button id="reloadBtn">Reload saved<\/button><button id="exportBtn" class="primary">Save<\/button>/);
});


test("campaign studio preview loads the game's shared typography", () => {
    assert.match(campaignMapEditorHtml, /<link rel="stylesheet" href="\/fonts\/fonts\.css"\s*\/>/);
    assert.match(campaignEditorHtml, /<link rel="stylesheet" href="\/fonts\/fonts\.css">/);
    assert.match(campaignEditorServer, /pathname\.startsWith\("\/fonts\/"\).*sow-web\/site\/fonts/s);
    assert.match(campaignEditorServer, /"\.woff2": "font\/woff2"/);
    assert.match(campaignEditorServer, /"\.ttc": "font\/collection"/);
    assert.match(storyCss, /font-family: var\(--sow-ui-font/);
    assert.match(campaignView, /root\.dataset\.localeScript = context\.localeScript \|\| doc\.documentElement\.dataset\.localeScript/);
    assert.match(campaignEditor, /localeScript: previewLocaleScript\(state\.previewLanguage\)/);
    assert.match(localeRuntime, /function localeScript\(locale\) \{[\s\S]*?if \(code === "ar"\) return "arabic";[\s\S]*?if \(code === "zh-cn" \|\| code === "ja" \|\| code === "ko"\) return "cjk";[\s\S]*?if \(code === "ru"\) return "cyrillic";/);
    assert.match(campaignEditor, /function previewLocaleScript\(locale\)[\s\S]*?code === "ar"[\s\S]*?\["zh-cn", "ja", "ko"\][\s\S]*?code === "ru"/);
    // One UI family by design: the stylesheet ships Work Sans woff2 faces and
    // lets the device system font cover other scripts. Webfont TTF/TTC faces
    // were a 19.5 MB uncached boot cost for CJK players.
    assert.match(gameFonts, /--sow-ui-font: 'Work Sans', system-ui, sans-serif;/);
    assert.doesNotMatch(gameFonts, /SOW Noto|\.ttf|\.ttc/);
});


test("campaign editor never treats a missing catalog entry as valid text", () => {
    assert.match(campaignEditorServer, /sow-i18n\/strings\/en\/web\.toml/);
    assert.match(campaignEditorServer, /return new RegExp\("\^" \+ textKey/);
});


test("campaign editor preview and game share RTL locale coverage", () => {
    const localeSet = /new Set\(\["ar", "arc", "ckb", "dv", "fa", "he", "iw", "nqo", "pnb", "ps", "sd", "syr", "ug", "ur", "yi"\]\)/;
    assert.match(localeRuntime, localeSet);
    assert.match(campaignEditor, localeSet);
    assert.match(campaignEditor, /direction: rtlLanguages\.has\(state\.previewLanguage\.toLowerCase\(\)\.split\("-"\)\[0\]\) \? "rtl" : "ltr"/);
});


test("campaign validator rejects conditional branches that can loop forever", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const report = campaign.validate({
        version: 2, episode_id: "route_loop_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "decision", steps: [
            { id: "decision", type: "choice", title_key: "tutorial.decision", choices: [
                { id: "repeat", label_key: "tutorial.repeat", next: "loop_scene" },
                { id: "finish", label_key: "tutorial.finish", next: "ending" }
            ] },
            { id: "loop_scene", type: "scene", title_key: "tutorial.loop_scene", next: "ending", routes: [
                { when: { fact: "tiles_gained", gte: 1 }, next: "loop_scene" }
            ] },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(report.errors.some(issue => issue.step === "loop_scene" && issue.message === "This path cannot reach an ending."));
    const legacy = campaign.validate({
        version: 2, episode_id: "legacy_flag_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending", routes: [{ when: { flag: "old_flag", equals: true }, next: "ending" }] },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(legacy.errors.some(issue => issue.step === "opening" && issue.field === "routes"));
});


test("campaign validator warns when route order makes a later threshold unreachable", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const report = campaign.validate({
        version: 2, episode_id: "route_order_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending", routes: [
                { when: { fact: "tiles_gained", gte: 1 }, next: "ending" },
                { when: { fact: "tiles_gained", gte: 10 }, next: "ending" }
            ] },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(report.warnings.some(issue => issue.step === "opening" && issue.message.includes("Route 2 is unreachable")));
});


test("campaign validator rejects ignored fields and dialog keeps its speaker row", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const report = campaign.validate({
        version: 2, episode_id: "unsupported_field_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "opening", actions: [{ type: "spawn" }], steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending", actions: [{ type: "grant_troops" }] },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(report.errors.some(issue => issue.field === "campaign" && issue.message.includes("actions")));
    assert.ok(report.errors.some(issue => issue.step === "opening" && issue.field === "fields" && issue.message.includes("actions")));
    const unusedSpeaker = campaign.validate({
        version: 2, episode_id: "step_field_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: {} }, speakers: { narrator: { name: "Narrator" } }, entry: "objective", steps: [
            { id: "objective", type: "objective", title_key: "tutorial.objective", speaker: "narrator", trigger: { type: "territory", scope: "step", value: 1 }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.deepEqual(unusedSpeaker.errors, []);
    assert.match(campaignView, /sow-story__speaker/);
    assert.doesNotMatch(campaignView, /sow-story__objective-speaker/);
});


test("campaign preview can seed prior decisions and facts to inspect conditional scenes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "preview_branch_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "decision", steps: [
            { id: "decision", type: "choice", title_key: "tutorial.decision", choices: [
                { id: "gather", label_key: "tutorial.gather", next: "rejoin" },
                { id: "strike", label_key: "tutorial.strike", next: "rejoin" }
            ] },
            { id: "rejoin", type: "scene", title_key: "tutorial.rejoin", next: "ending", routes: [
                { when: { choice: "decision", equals: "strike" }, next: "strike_echo" },
                { when: { fact: "tiles_gained", gte: 3 }, next: "growth_echo" }
            ] },
            { id: "strike_echo", type: "scene", title_key: "tutorial.strike_echo", next: "ending" },
            { id: "growth_echo", type: "scene", title_key: "tutorial.growth_echo", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    machine.jump("rejoin", {}, { decision: "strike", unknown_decision: "strike", decision_bad_answer: "invalid" });
    assert.equal(machine.advance(null, "rejoin"), true);
    assert.equal(machine.view().step.id, "strike_echo");
    assert.deepEqual({ ...machine.view().state.choices }, { decision: "strike" });
    const factPreview = campaign.create(machine.definition);
    factPreview.jump("rejoin", { tiles_gained: 3 });
    assert.equal(factPreview.advance(null, "rejoin"), true);
    assert.equal(factPreview.view().step.id, "growth_echo");
    assert.match(campaignEditorHtml, /id="previewBranchSelect"/);
    assert.match(campaignEditor, /condition\.fact && Number\.isFinite\(condition\.gte\)/);
    assert.match(campaignEditor, /state\.machine\.jump\(start, context\.facts, context\.choices\)/);
});


test("live campaign edits preserve prior decisions and conditional story branches", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "live_edit_test", default_locale: "en", settings: { starting_troops: 1000 },
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
    };
    const machine = campaign.create(definition);
    machine.advance(null, "opening");
    machine.advance("strike", "decision");
    machine.advance(null, "strike_step");
    const edited = JSON.parse(JSON.stringify(definition));
    edited.steps.find(step => step.id === "rejoin").title_key = "tutorial.rejoin_edited";
    assert.equal(machine.replaceDefinition(edited), true);
    assert.equal(machine.view().state.choices.decision, "strike");
    assert.equal(machine.view().step.title_key, "tutorial.rejoin_edited");
    machine.advance(null, "rejoin");
    assert.equal(machine.view().step.id, "strike_echo");
});


test("campaign validator rejects speakers without a display name", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const report = campaign.validate({
        version: 2, episode_id: "speaker_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: { "tutorial.opening": "Opening", "tutorial.ending": "The end" } },
        speakers: { empty: { name: "  " } }, entry: "opening", steps: [
            { id: "opening", type: "scene", speaker: "empty", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(report.errors.some(issue => issue.field === "speakers" && /empty/.test(issue.message)));
});


test("faction deletion adjusts campaign references and keeps the remaining story runnable", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "roster_edit", default_locale: "en",
        settings: { starting_troops: 1000, campaign_support: { after_defeated: "removed_tribe", share_percent: 50 } },
        speakers: { removed_ally: { faction: "removed_tribe" } }, entry: "opening",
        faction_story_names: { removed_tribe: "Old Tribe", existing_tribe: "Existing Tribe" },
        steps: [
            { id: "opening", type: "scene", speaker: "removed_ally", title_key: "tutorial.open", next: "contact" },
            { id: "contact", type: "objective", title_key: "tutorial.contact", trigger: { type: "contact", targets: ["existing_tribe", "removed_tribe"], value: 2, scope: "episode" }, marker: { target: "removed_tribe" }, guide: { kind: "world", target: "target_action", gesture: "tap" }, next: "removed_objective" },
            { id: "removed_objective", type: "objective", title_key: "tutorial.removed", trigger: { type: "eliminated", target: "removed_tribe", value: 1, scope: "episode" }, marker: { target: "removed_tribe" }, guide: { kind: "world", target: "target_action", gesture: "tap" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.end" }
        ],
        reactions: [{ id: "removed_ally_contact", when: { type: "contact", target: "removed_tribe" }, speaker: "removed_ally", title_key: "tutorial.response_title", body_key: "tutorial.response_body" }]
    };
    const originalRoster = { map: "eastanglia", player_spawn: [10, 10], factions: [
        { id: "removed_tribe", name: "Removed Tribe", x: 15, y: 15, starting_troops: 500, relation: "neutral", civ: "Iceni Kingdom", leader: "Boudica" },
        { id: "existing_tribe", name: "Existing Tribe", x: 20, y: 20, starting_troops: 500, relation: "neutral", civ: "Iceni Kingdom", leader: "Boudica" }
    ] };
    const roster = { ...originalRoster, factions: originalRoster.factions.slice(1) };

    assert.deepEqual(campaign.validate(definition, originalRoster).errors, []);
    assert.ok(campaign.factionReferenceIds(definition).has("removed_tribe"));
    assert.ok(campaign.factionReferenceIds(definition).has("existing_tribe"));
    const result = campaign.removeFactionReferences(definition, [{ id: "removed_tribe", name: "Removed Tribe" }]);
    const cleaned = result.definition;
    assert.deepEqual(cleaned.steps.find(step => step.id === "contact").trigger.targets, ["existing_tribe"]);
    assert.equal(cleaned.steps.find(step => step.id === "contact").trigger.value, 1);
    assert.equal(cleaned.steps.find(step => step.id === "contact").marker.target, "existing_tribe");
    assert.equal(cleaned.steps.find(step => step.id === "removed_objective").type, "scene");
    assert.equal(cleaned.speakers.removed_ally.name, "Removed Tribe");
    assert.equal(cleaned.speakers.removed_ally.faction, undefined);
    assert.equal(cleaned.settings.campaign_support, undefined);
    assert.equal(cleaned.faction_story_names.removed_tribe, undefined);
    assert.deepEqual(cleaned.reactions, []);
    assert.equal(campaign.factionReferenceIds(cleaned).has("removed_tribe"), false);
    assert.deepEqual(campaign.validate(cleaned, roster, { hasText: () => true, hasAvatar: () => true }).errors, []);
    const playerOnly = campaign.removeFactionReferences(definition, originalRoster.factions).definition;
    assert.deepEqual([...campaign.factionReferenceIds(playerOnly)], []);
    assert.deepEqual(campaign.validate(playerOnly, roster, { hasText: () => true, hasAvatar: () => true }).errors, []);
    assert.match(campaignEditorServer, /removeFactionReferences\(definition, removedFactions\)/);
    assert.doesNotMatch(campaignEditorServer, /!Array\.isArray\(roster\.factions\) \|\| !roster\.factions\.length/);
    assert.doesNotMatch(campaignMapEditorHtml, /!Array\.isArray\(value\.factions\) \|\| !value\.factions\.length/);
    assert.doesNotMatch(campaignMapEditorHtml, /Cannot delete: campaign story still uses this faction/);
    assert.match(campaignMapEditorHtml, /story references will be adjusted when saved/);
});


test("campaign saves clamp spawns and keep map requirements in sync with the game", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "boudica", default_locale: "en", settings: { starting_troops: 1000 },
        speakers: {}, entry: "opening",
        steps: [{ id: "opening", type: "scene", title_key: "tutorial.open", next: "ending" }, { id: "ending", type: "end", title_key: "tutorial.end" }]
    };
    const roster = { map: "eastanglia", player_spawn: [895, 503], factions: [{ id: "enemy", name: "Enemy", x: 1, y: 1, starting_troops: 500, relation: "enemy", civ: "Roman Empire", leader: "Caesar" }] };
    const errors = () => campaign.validate(definition, roster).errors;
    assert.deepEqual(errors(), []);
    roster.map = "northamerica";
    assert.ok(errors().some(issue => issue.field === "roster.map"));
    roster.map = "eastanglia"; roster.player_spawn[0] = 896;
    assert.ok(errors().some(issue => issue.field === "roster.player_spawn"));
    roster.player_spawn[0] = 895; roster.factions[0].y = 504;
    assert.ok(errors().some(issue => /Faction spawn is outside/.test(issue.message)));
    roster.factions = [];
    assert.deepEqual(errors(), [], "deleting the last faction still leaves a playable player-only campaign");
    roster.factions = [{ id: "enemy", name: "Enemy", x: 1, y: 1, starting_troops: 500, relation: "enemy", civ: "Roman Empire", leader: "Caesar" }];
    definition.episode_id = "six_sky_ep1"; roster.map = "northamerica";
    roster.player_spawn = [999, 515]; roster.factions[0].y = 515;
    assert.deepEqual(errors(), []);
    assert.match(campaignMapEditorHtml, /function clampRosterPositions\(\)/);
    assert.match(campaignMapEditorHtml, /if \(clampRosterPositions\(\)\)/);
});


test("campaign speaker picker is sourced from existing avatar assets and rejects missing files", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.match(campaignEditorServer, /url\.pathname === "\/__avatars"/);
    assert.match(campaignEditor, /fetch\("\/__avatars"/);
    assert.match(campaignEditor, /selectField\("Portrait"/);
    assert.match(campaignEditor, /hasAvatar: function \(avatar\) \{ return state\.avatars\.includes\(avatar\); \}/);
    const definition = {
        version: 2, episode_id: "avatar_test", default_locale: "en", settings: { starting_troops: 1000 },
        strings: { en: { "tutorial.opening": "Opening", "tutorial.ending": "End" } }, speakers: { leader: { name: "Leader", avatar: "not_real" } }, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    };
    assert.ok(campaign.validate(definition, null, { hasAvatar: avatar => avatar === "boudica" }).errors.some(issue => issue.field === "speakers"));
});


test("campaign studio reads speaker names from shared text keys", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.match(campaignEditor, /var keyBase = storyKey\("speaker_" \+ speakerId \+ "_name"\)/);
    assert.match(campaignEditor, /catalogTextField\(card, "Character name", speaker\.name_key, 2\)/);
    assert.doesNotMatch(campaignEditor, /dictionary\[speaker\.name_key\] = value/);
    assert.match(campaignEditor, /Use one name for every language/);
    assert.match(campaignView, /character\.name_key \? t\(character\.name_key\)/);
    const report = campaign.validate({
        version: 2, episode_id: "localized_speaker", settings: { starting_troops: 1000 },
        speakers: { leader: { name_key: "tutorial.speaker_leader_name" } }, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    }, null, { hasText: () => true });
    assert.deepEqual(report.errors, []);
});


test("campaign studio previews a locale with shared English fallback", () => {
    assert.match(campaignEditor, /function textValue\(key, locale\)[\s\S]*?catalogValue\(locale \|\| state\.previewLanguage, key\) \|\| catalogValue\("en", key\)/);
    assert.match(campaignEditor, /Text is managed by the shared language catalogs\./);
    assert.match(campaignEditor, /Missing shared English text\. Add this key to sow-i18n\/strings\/en\/web\.toml\./);
    assert.doesNotMatch(campaignEditor, /state\.definition\.default_locale/);
});


test("campaign studio loads shared English and preview catalogs", () => {
    assert.match(campaignEditor, /localeFailures: \[\], localeRegistryFailed: false/);
    assert.match(campaignEditor, /function loadCatalog\(locale\)/);
    assert.match(campaignEditor, /if \(catalogLoads\[code\]\) return catalogLoads\[code\]/);
    assert.match(campaignEditor, /fetch\("\/locales\/" \+ encodeURIComponent\(code\)/);
    assert.match(campaignEditor, /if \(!state\.localeFailures\.includes\(code\)\) state\.localeFailures\.push\(code\)/);
    assert.match(campaignEditor, /Promise\.all\(\[loadCatalog\("en"\), loadCatalog\(state\.previewLanguage\)\]\)/);
    assert.match(campaignEditor, /Catalog unavailable for .*shared-text previews may be incomplete/);
    assert.match(campaignEditor, /var version = Number\(registry && registry\.version\)/);
    assert.match(campaignEditor, /registry\.schema !== 1[\s\S]*?Array\.isArray\(registry\.languages\)/);
    assert.match(campaignEditor, /locales = \["en", "es"\]; state\.localeRegistryFailed = true/);
});


test("campaign studio generates distinct fallback text keys for decisions and dialogue lines", () => {
    assert.match(campaignEditor, /parts\[0\] === "choices" && step\.choices\[index\]/);
    assert.match(campaignEditor, /step\.choices\[index\]\.id \+ \(keyPath === "label_key"/);
    assert.match(campaignEditor, /keyPath === "label_key" \? "_label" : "_detail"/);
    assert.match(campaignEditor, /parts\[0\] === "lines" && step\.lines\[index\]/);
    assert.match(campaignEditor, /"_line_" \+ \(index \+ 1\) \+ "_body"/);
});


test("campaign editor and shared locale runtime normalize regional codes", () => {
    assert.match(campaignEditor, /entry\.code\)\.toLowerCase\(\)/);
    assert.match(localeRuntime, /toLowerCase\(\)\.replace\(\/_\/g, "-"\)/);
    assert.match(localeRuntime, /function normalizeLocale\(value\)/);
});


test("campaign graph Fit centers every node using its actual dimensions", () => {
    assert.match(campaignEditor, /function fitGraph\(\)[\s\S]*?node\.offsetWidth[\s\S]*?node\.offsetHeight[\s\S]*?state\.pan = \{ x: \(stage\.clientWidth - width \* zoom\) \/ 2 - bounds\.left \* zoom/);
    assert.match(campaignEditor, /button\.dataset\.zoom === "fit"\) fitGraph\(\)/);
    assert.doesNotMatch(campaignEditor, /button\.dataset\.zoom === "fit"\) \{ state\.zoom = 1; state\.pan = \{ x: 36, y: 44 \}/);
});


test("campaign graph Arrange lays out both entries and follows every branch", () => {
    assert.match(campaignEditorHtml, /id="arrangeGraphBtn"/);
    assert.match(campaignEditor, /\[state\.definition\.entry, state\.definition\.menu_guide && state\.definition\.menu_guide\.entry\]/);
    assert.match(campaignEditor, /portDefinitions\(source\)\.forEach/);
    assert.match(campaignEditor, /state\.definition\.layout\[step\.id\] = \{ x: 48 \+ x \* 320, y: y \}/);
    assert.match(campaignEditor, /markDirty\(\); fitGraph\(\)/);
});


test("campaign studio episode picker derives options from paired campaign files", () => {
    assert.match(campaignEditorServer, /url\.pathname === "\/__episodes"/);
    assert.match(campaignEditorServer, /name\.endsWith\("\.triggers\.json"\)/);
    assert.match(campaignEditorServer, /files\.has\(id \+ "\.json"\)/);
    assert.match(campaignEditor, /fetch\("\/__episodes", \{ cache: "no-store" \}\)/);
    assert.match(campaignEditor, /fillEpisodes\(\)/);
    assert.match(campaignEditorHtml, /id="episodeSelect"/);
    assert.match(campaignMapEditorHtml, /id="episodeSelect"/);
    assert.match(campaignMapEditorHtml, /fetch\("\/__episodes"/);
    assert.match(campaignMapEditorHtml, /Discard unsaved map changes and switch episode/);
});


test("campaign studio language selectors use readable names from the locale registry", () => {
    assert.match(campaignEditor, /localeNames\[code\] = entry\.name/);
    assert.match(campaignEditor, /function localeOptions\(\)[\s\S]*?label: localeNames\[code\] \|\| code\.toUpperCase\(\)/);
    assert.match(campaignEditorHtml, /<label>Language <select id="previewLocale">/);
    assert.match(campaignEditor, /optionList\(\$\("#previewLocale"\), localeOptions\(\)/);
    assert.doesNotMatch(campaignEditorHtml, /id="editLocale"/);
});


test("campaign map coordinate fields retain focus and refresh placement feedback", () => {
    assert.match(campaignMapEditorHtml, /const updateFactionHint = \(\) =>/);
    assert.match(campaignMapEditorHtml, /getElementById\("fx"\)\.oninput = e => \{.*updateFactionHint\(\); renderList\(\); draw\(\); \}/);
    assert.match(campaignMapEditorHtml, /getElementById\("fy"\)\.oninput = e => \{.*updateFactionHint\(\); renderList\(\); draw\(\); \}/);
    assert.match(campaignMapEditorHtml, /getElementById\("fx"\)\.onchange = e => \{ f\.x=SOWCampaignMapPreview\.clampTile/);
    assert.match(campaignMapEditorHtml, /getElementById\("fy"\)\.onchange = e => \{ f\.y=SOWCampaignMapPreview\.clampTile/);
    assert.match(campaignMapEditorHtml, /if \(clampRosterPositions\(\)\)/);
    assert.doesNotMatch(campaignMapEditorHtml, /getElementById\("f[xy]"\)\.oninput = e => \{[^\n]*renderSide\(\)/);
    assert.match(campaignMapEditorHtml, /const scrollTop = list\.scrollTop;[\s\S]*?list\.innerHTML = html;[\s\S]*?list\.scrollTop = scrollTop;/);
});


test("campaign map editor renders faction names as text in its HTML templates", () => {
    assert.match(campaignMapEditorHtml, /function escapeHtml\(value\)/);
    assert.match(campaignMapEditorHtml, /value="\$\{escapeHtml\(f\.name\)\}"/);
    assert.match(campaignMapEditorHtml, /class="nm">\$\{escapeHtml\(f\.name\)\}/);
    assert.match(campaignMapEditorHtml, /inWater\.map\(escapeHtml\)/);
});


test("campaign studio authors cinematic endings and navigable validation notes", () => {
    assert.match(campaignEditor, /step\.type === "scene" \|\| step\.type === "end"/);
    assert.match(campaignEditor, /Add closing text/);
    assert.match(campaignEditor, /warningsByStep\[issue\.step\]/);
    assert.match(campaignEditor, /report\.warnings\.forEach[\s\S]*?if \(issue\.step\) button\.addEventListener\("click", function \(\) \{ selectStep\(issue\.step\); \}\)/);
    const demo = campaignEditor.slice(campaignEditor.indexOf("function demo()"), campaignEditor.indexOf("function createMenuGuide()"));
    assert.match(demo, /lines: \[\{ speaker: leaderId[\s\S]*?speaker: advisorId/);
    assert.match(demo, /type: "choice"[\s\S]*?type: "choice"/);
    assert.doesNotMatch(demo, /Boudica|Rome/);
    assert.match(campaignEditor, /label: textValue\(choice\.label_key, state\.previewLanguage\) \|\| choice\.id/);
    assert.match(campaignEditor, /Add consequence detail/);
    assert.match(campaignEditorHtml, /<summary>Quick start<\/summary>/);
    assert.match(campaignEditorHtml, /Use Create next step on each decision answer to build its path in place/);
});


test("branching sample preview preserves the episode draft and cannot be exported", () => {
    const demo = campaignEditor.slice(campaignEditor.indexOf("function demo()"), campaignEditor.indexOf("function createMenuGuide()"));
    assert.match(demo, /definition: JSON\.parse\(JSON\.stringify\(state\.definition\)\)/);
    assert.match(demo, /state\.definition = previous\.definition[\s\S]*?state\.dirty = previous\.dirty/);
    assert.doesNotMatch(demo, /confirm\(/);
    assert.match(campaignEditor, /function markDirty\(\) \{\s*if \(state\.demoBackup\) return;/);
    assert.match(campaignEditor, /async function save\(\) \{\s*if \(state\.demoBackup\) return false;/);
    assert.match(campaignEditor, /function download\(\) \{\s*if \(state\.demoBackup\) return;/);
    assert.match(campaignEditor, /async function watchExternalFiles\(\) \{\s*if \(state\.demoBackup \|\|/);
    assert.match(campaignEditorHtml, /id="demoBtn" class="sample-toggle"[^>]*>Preview branching sample/);
    assert.match(campaignEditorHtml, /id="graphPanel"[\s\S]*?id="inspectorPanel"/);
});


test("campaign story preview uses the selected episode map and faction positions", () => {
    assert.match(campaignEditorHtml, /id="campaignMapPreview"/);
    assert.match(campaignEditorHtml, /id="campaignPreviewMarkers"/);
    assert.match(campaignEditor, /SOWCampaignMapPreview\.load\(mapId\)/);
    assert.match(campaignEditor, /dataset: \{ factionId: faction\.id \}/);
    assert.match(campaignEditor, /Number\(faction\.x\) \/ map\.width \* 100/);
    assert.match(campaignEditor, /function previewWorldMarker\(frame, target, step\)/);
    assert.match(campaignMapEditorHtml, /SOWCampaignMapPreview\.load\(mapId\)/);
    assert.match(campaignMapPreview, /!== "SOWM"/);
    assert.match(campaignMapPreview, /function terrainCanvas\(terrain, width, height\)/);
    assert.match(campaignMapEditorHtml, /SOWCampaignMapPreview\.isLand\(terrain, MAPW, MAPH, mx, my\)/);
    assert.match(campaignMapEditorHtml, /sel=\+el\.dataset\.i; focusFaction\(sel\)/);
    assert.match(atlasHtml, /button\.onclick=\(\)=>select\(entity\.id,true\)/);
    assert.match(atlasHtml, /SOWCampaignMapPreview\.load\("world"\)/);
    assert.match(atlasHtml, /thumbnail_frames\.json/);
    assert.match(atlasHtml, /giantworldmap\.png/);
    assert.match(atlasHtml, /const frame=worldReference&&!worldReferenceError&&worldReference\.source\.map_grid_frame/);
    assert.match(atlasHtml, /ctx\.drawImage\(worldReference\.image,frame\[0\],frame\[1\],frame\[2\],frame\[3\],0,0,world\.width,world\.height\)/);
    assert.doesNotMatch(atlasHtml, /drawEquirectangularImage/);
    assert.match(atlasHtml, /ctx\.globalAlpha=0\.2/);
    assert.match(atlasHtml, /zoomMapView\(viewState\(\),point,factor,minScale,fitScale\*64\)/);
    assert.match(campaignMapEditorHtml, /canvas \{ position:absolute; top:0; left:0; width:100%; height:100%;/);
    for (const viewer of [campaignMapEditorHtml, atlasHtml, mapRostersHtml]) {
        assert.match(viewer, /SOWCampaignMapPreview\.resizeCanvas\(/);
        assert.match(viewer, /SOWCampaignMapPreview\.fitMapView\(/);
        assert.match(viewer, /SOWCampaignMapPreview\.zoomMapView\(/);
    }
    assert.match(mapRostersHtml, /window\.addEventListener\("resize",resize\);resize\(\);\s*document\.addEventListener/);
    assert.match(atlasHtml, /mapPointToGeo/);
    assert.match(atlasHtml, /world_island_guides\.json/);
    assert.match(atlasHtml, /new Path2D\(\)/);
    assert.match(atlasHtml, /ctx\.globalAlpha=1;ctx\.fillStyle=[^;]+;ctx\.fill\(worldIslandPath,"evenodd"\);ctx\.strokeStyle=[^;]+;ctx\.lineWidth=0\.75\/scale;[^;]+;[^;]+;ctx\.stroke\(worldIslandPath\)/);
    assert.match(atlasHtml, /pointerStart=pointer\.slice\(\)/);
    assert.match(atlasHtml, /Math\.hypot\(point\[0\]-pointerStart\[0\],point\[1\]-pointerStart\[1\]\)>Math\.min\(2,scale\*0\.5\)/);
    assert.match(atlasHtml, /body:JSON\.stringify\(catalog\)/);
    assert.match(campaignEditorServer, /JSON\.stringify\(value, null, 2\)/);
    const atlasScript = atlasHtml.match(/<script>\s*([\s\S]*?)<\/script>/);
    assert.ok(atlasScript, "Atlas inline script exists");
    assert.doesNotThrow(() => new vm.Script(atlasScript[1]));
    assert.match(atlasHtml, /geoToTile\(entity\.lat,entity\.lon,world\.geoBounds/);
    assert.match(atlasHtml, /SOWCampaignMapPreview\.isLand\(world\.terrain,world\.width,world\.height,tile\.x,tile\.y\)/);

    const islandGuides = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/map_sources/world_island_guides.json"), "utf8"));
    assert.equal(islandGuides.coordinate_reference, "WGS84 longitude,latitude");
    assert.ok(islandGuides.sources.some(source => source.name === "Natural Earth 10m Land"));
    assert.ok(islandGuides.sources.some(source => source.name === "Natural Earth 10m Minor Islands"));
    const entities = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/geo_entities.json"), "utf8")).entities;
    for (const id of ["rapa_nui", "tahitians", "marquesans"]) {
        const entity = entities.find(item => item.id === id);
        const rings = islandGuides.features.filter(feature => feature.properties.entity_ids.includes(id)).map(feature => feature.geometry.coordinates);
        assert.ok(entity && rings.length, `${id} has a sourced island outline`);
        const nearestVertex = Math.min(...rings.flatMap(ring => ring.map(([lon, lat]) => Math.hypot(lon - entity.lon, lat - entity.lat))));
        assert.ok(nearestVertex < 0.2, `${id} outline remains anchored to its catalog coordinates`);
    }
});


test("selecting a map entity centers it in the current view", () => {
    const offsets = campaignMapPreviewApi.centerMapOffsets({ x: 120, y: 40 }, 2, 800, 600);
    assert.deepEqual(offsets, { x: 160, y: 220 });
    assert.equal(120 * 2 + offsets.x, 400);
    assert.equal(40 * 2 + offsets.y, 300);
});


test("Atlas deletes the selected entity with Delete outside editable fields", () => {
    assert.match(atlasHtml, /function deleteSelected\(\)[\s\S]*?catalog\.entities\.splice\(index,1\);[\s\S]*?selectedId=null;[\s\S]*?markDirty\(\)/);
    assert.match(atlasHtml, /document\.addEventListener\("keydown",event=>\{[\s\S]*?event\.key!=="Delete"[\s\S]*?event\.key!=="Backspace"[\s\S]*?input,textarea,select,\[contenteditable='true'\][\s\S]*?event\.preventDefault\(\);deleteSelected\(\)/);
});


test("playable map preview shares game geography and land checks", () => {
    function encodeMap(bounds) {
        const width = 4, height = 2, name = Buffer.from("test"), terrain = Buffer.from([0x80, 0, 0x80, 0, 0, 0x80, 0x80, 0]);
        const bytes = Buffer.alloc(4 + 2 + 2 + 4 + 4 + 4 + 2 + name.length + 2 + terrain.length + (bounds ? 17 : 0));
        let offset = 0;
        bytes.write("SOWM", offset); offset += 4;
        bytes.writeUInt16LE(1, offset); offset += 4;
        bytes.writeUInt32LE(width, offset); offset += 4;
        bytes.writeUInt32LE(height, offset); offset += 4;
        bytes.writeUInt32LE(4, offset); offset += 4;
        bytes.writeUInt16LE(name.length, offset); offset += 2;
        name.copy(bytes, offset); offset += name.length;
        bytes.writeUInt16LE(0, offset); offset += 2;
        terrain.copy(bytes, offset); offset += terrain.length;
        if (bounds) {
            bytes.writeUInt8(1, offset++);
            for (const value of bounds) { bytes.writeInt32LE(Math.round(value * 1e6), offset); offset += 4; }
        }
        return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    }

    const bounds = { minLon: 170, minLat: -10, maxLon: 190, maxLat: 10 };
    const map = campaignMapPreviewApi.parse(encodeMap([170, -10, 190, 10]));
    assert.deepEqual(map.geoBounds, bounds);
    assert.deepEqual(campaignMapPreviewApi.geoToTile(0, -180, map.geoBounds, map.width, map.height), { x: 2, y: 1 });
    assert.deepEqual(campaignMapPreviewApi.geoToTile(10, 190, map.geoBounds, map.width, map.height), { x: 3, y: 0 });
    const preciseGeo = campaignMapPreviewApi.mapPointToGeo(2.25, 0.4, map.geoBounds, map.width, map.height);
    assert.ok(Math.abs(preciseGeo.lat - 6) < 1e-12);
    assert.ok(Math.abs(preciseGeo.lon + 178.75) < 1e-12);
    const precisePoints = campaignMapPreviewApi.geoToMapPoints(preciseGeo.lat, preciseGeo.lon, map.geoBounds, map.width, map.height);
    assert.ok(precisePoints.some(point => Math.abs(point.x - 2.25) < 1e-9 && Math.abs(point.y - 0.4) < 1e-9));
    assert.equal(campaignMapPreviewApi.mapPointToGeo(map.width, 0, map.geoBounds, map.width, map.height), null);
    const geo = campaignMapPreviewApi.tileToGeo(2, 1, map.geoBounds, map.width, map.height);
    assert.deepEqual(campaignMapPreviewApi.geoToTile(geo.lat, geo.lon, map.geoBounds, map.width, map.height), { x: 2, y: 1 });
    assert.equal(campaignMapPreviewApi.isLand(map.terrain, map.width, map.height, 2, 1), true);
    assert.equal(campaignMapPreviewApi.isLand(map.terrain, map.width, map.height, 3, 1), false);
    assert.equal(campaignMapPreviewApi.isLand(map.terrain, map.width, map.height, map.width, 0), false);
    assert.equal(campaignMapPreviewApi.parse(encodeMap(null)).geoBounds, null);
    assert.equal(campaignMapPreviewApi.geoToTile(0, 0, map.geoBounds, map.width, map.height), null);

    const worldBytes = fs.readFileSync(path.join(shell, "../../assets/maps/world/map.bin"));
    const worldBuffer = worldBytes.buffer.slice(worldBytes.byteOffset, worldBytes.byteOffset + worldBytes.byteLength);
    const world = campaignMapPreviewApi.parse(worldBuffer);
    assert.deepEqual(world.geoBounds, { minLon: -168.69, minLat: -78.8, maxLon: 192.37, maxLat: 82.78 });
    const london = campaignMapPreviewApi.geoToTile(51.51, -0.13, world.geoBounds, world.width, world.height);
    assert.deepEqual(london, { x: 466, y: 96 });
    assert.equal(campaignMapPreviewApi.isLand(world.terrain, world.width, world.height, london.x, london.y), true);
    assert.equal(campaignMapPreviewApi.tileToGeo(999, 200, world.geoBounds, world.width, world.height), null);
    const wrappedGeo = campaignMapPreviewApi.mapPointToGeo(999.25, 250, world.geoBounds, world.width, world.height);
    const wrappedPoints = campaignMapPreviewApi.geoToMapPoints(wrappedGeo.lat, wrappedGeo.lon, world.geoBounds, world.width, world.height);
    assert.ok(wrappedPoints.some(point => Math.abs(point.x - 999.25) < 1e-9 && Math.abs(point.y - 250) < 1e-9));

    const source = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/map_sources/thumbnail_frames.json"), "utf8")).sources["giantworldmap.png"];
    assert.deepEqual(source.map_grid_frame, [16, 16, 4094, 1932]);
    const sourceBytes = fs.readFileSync(path.join(shell, "../../assets/map_sources/giantworldmap.png"));
    assert.equal(sourceBytes.toString("hex", 0, 8), "89504e470d0a1a0a");
    assert.equal(sourceBytes.readUInt32BE(16), source.width);
    assert.equal(sourceBytes.readUInt32BE(20), source.height);
    const referenceImage = { naturalWidth: source.width, naturalHeight: source.height };
    const icelandSource = { x: 1717, y: 237 }, icelandTarget = campaignMapPreviewApi.geoToMapPoint(65, -18.5, world.geoBounds, world.width, world.height);
    const [sx, sy, sw, sh] = source.map_grid_frame;
    const icelandAligned = { x: (icelandSource.x - sx) / sw * world.width, y: (icelandSource.y - sy) / sh * world.height };
    assert.ok(Math.abs(icelandAligned.x - icelandTarget.x) < 1, "Iceland source anchor aligns to its catalog coordinate");
    assert.ok(Math.abs(icelandAligned.y - icelandTarget.y) < 2.5, "Iceland source anchor aligns to its catalog coordinate");
});


test("invalid campaign drafts preserve the last valid preview and disable simulation", () => {
    assert.match(campaignEditor, /function syncPreview\(\) \{\s*if \(state\.validation\.errors\.length\)/);
    assert.match(campaignEditor, /Fix validation errors to preview this draft/);
    assert.match(campaignEditor, /\["playSelected", "restartBtn", "simulateBtn", "tickBtn", "previewBranchSelect"\][\s\S]*?\.disabled = blocked/);
    assert.match(campaignEditor, /function paintPreview\(updateMachine\) \{\s*if \(!state\.machine \|\| state\.validation\.errors\.length\) return/);
    assert.match(campaignEditor, /function renderPreview\(forceStep\) \{\s*if \(!state\.definition\) return;\s*if \(state\.validation\.errors\.length\)/);
});


test("campaign map preview accepts valid SOWM terrain and rejects a truncated map", () => {
    const mapPreview = require(path.join(shell, "../../sow-tools/editors/campaign-editor/map-preview.js"));
    const buffer = new ArrayBuffer(26), view = new DataView(buffer), bytes = new Uint8Array(buffer);
    bytes.set([83, 79, 87, 77]);
    view.setUint16(4, 1, true); view.setUint32(8, 2, true); view.setUint32(12, 1, true);
    view.setUint16(20, 0, true); view.setUint16(22, 0, true);
    bytes.set([0x80, 0], 24);
    const map = mapPreview.parse(buffer);
    assert.equal(map.width, 2);
    assert.equal(map.height, 1);
    assert.deepEqual(Array.from(map.terrain), [0x80, 0]);
    assert.throws(() => mapPreview.parse(buffer.slice(0, 25)), /Invalid SOW map dimensions/);
});


test("campaign map edits clamp coordinates to playable tiles", () => {
    assert.equal(campaignMapPreviewApi.clampTile(-3, 10), 0);
    assert.equal(campaignMapPreviewApi.clampTile(9, 10), 9);
    assert.equal(campaignMapPreviewApi.clampTile(12, 10), 9);
    assert.equal(campaignMapPreviewApi.clampTile(3.6, 10), 4);
    assert.equal(campaignMapPreviewApi.clampTile("invalid", 10), 0);
    assert.equal(campaignMapPreviewApi.clampTile(3, 0), 0);
});


test("campaign map preview shares canvas sizing and anchored viewport math", () => {
    const canvas = {
        width: 300,
        height: 150,
        getBoundingClientRect: () => ({ width: 720, height: 400 })
    };
    assert.deepEqual(campaignMapPreviewApi.resizeCanvas(canvas, 2), { width: 720, height: 400, dpr: 2 });
    assert.equal(canvas.width, 1440);
    assert.equal(canvas.height, 800);

    const fit = campaignMapPreviewApi.fitMapView(1000, 500, 720, 400);
    assert.deepEqual(fit, { scale: 0.72, fitScale: 0.72, minScale: 0.36, x: 0, y: 20 });
    const anchor = [360, 200];
    const before = campaignMapPreviewApi.screenToMap(anchor, fit);
    const zoomed = campaignMapPreviewApi.zoomMapView(fit, anchor, 0.1, fit.minScale, fit.fitScale * 64);
    const after = campaignMapPreviewApi.screenToMap(anchor, zoomed);
    assert.ok(Math.abs(zoomed.scale - fit.minScale) < 1e-12, "zoom out stops at half Fit");
    assert.ok(Math.abs(after[0] - before[0]) < 1e-12);
    assert.ok(Math.abs(after[1] - before[1]) < 1e-12);
    assert.deepEqual(campaignMapPreviewApi.panMapView(zoomed, 12, -8), {
        scale: zoomed.scale, x: zoomed.x + 12, y: zoomed.y - 8
    });
    assert.deepEqual(campaignMapPreviewApi.mapToScreen(before, fit), anchor);
});


test("Map Rosters reads binary presets and leaves unmatched map anchors intact", () => {
    const mapPreview = require(path.join(shell, "../../sow-tools/editors/campaign-editor/map-preview.js"));
    const u16 = value => { const b = Buffer.alloc(2); b.writeUInt16LE(value); return b; };
    const u32 = value => { const b = Buffer.alloc(4); b.writeUInt32LE(value); return b; };
    const string = value => { const b = Buffer.from(value); return Buffer.concat([u16(b.length), b]); };
    const spawn = Buffer.concat([string("Unlinked local anchor"), string(""), u32(0), u32(0)]);
    const terrain = Buffer.from([0x80, 0x80, 0x80, 0x80]);
    const roster = Buffer.concat([
        u16(1), string("historical"), u16(1), string("historical"), string("Historical"), u16(1),
        string("iceland"), Buffer.from([0]), u32(2), u32(0), u16(0)
    ]);
    const bytes = Buffer.concat([
        Buffer.from("SOWM"), u16(1), u16(0), u32(4), u32(1), u32(4), string("Test map"),
        u16(1), spawn, terrain, Buffer.from([0, 2]), u32(roster.length), roster
    ]);
    const parsed = mapPreview.parse(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));

    assert.equal(parsed.defaultRoster, "historical");
    assert.deepEqual(parsed.spawns, [{ name: "Unlinked local anchor", flag: "", x: 0, y: 0 }]);
    assert.deepEqual(parsed.rosters, [{
        id: "historical", name: "Historical", entries: [
            { entity_id: "iceland", role: "nation", x: 2, y: 0, legacy_anchor: 0 }
        ]
    }]);
    assert.match(mapRostersHtml, /SOWCampaignMapPreview\.load\(id\)/);
    assert.match(mapRostersHtml, /assets\/gameplay\/avatars/);
    assert.match(mapRostersHtml, /SOWCampaignMapPreview\.drawMapLabel/);
    const rosterScript = mapRostersHtml.match(/<script>\s*([\s\S]*?)<\/script>/);
    assert.ok(rosterScript, "Map Rosters inline script exists");
    assert.doesNotThrow(() => new vm.Script(rosterScript[1]));
    assert.match(mapRosterWriter, /map_file::encode\(&map\)/);
    assert.match(mapRosterWriter, /map\.bin\.br/);
    assert.doesNotMatch(mapRosterWriter.split("#[cfg(test)]")[0], /roster\.json/i);
});


test("Atlas picker visibly selects a catalog entity by stable ID in both editors", async () => {
    class FakeElement {
        constructor(id = "") { this.id = id; this.children = []; this.listeners = {}; this.attributes = {}; this.value = ""; this.textContent = ""; }
        setAttribute(name, value) { this.attributes[name] = value; }
        addEventListener(name, callback) { (this.listeners[name] ||= []).push(callback); }
        append(...children) { this.children.push(...children); }
        replaceChildren(...children) { this.children = children; }
        async dispatch(name, event = {}) { for (const callback of this.listeners[name] || []) await callback(event); }
    }
    const context = { document: { createElement: () => new FakeElement() } };
    vm.runInNewContext(atlasEntityPickerSource, context);
    const pickerApi = context.SOWAtlasEntityPicker;
    const entities = [
        { id: "iceland", name: "Iceland", kind: "country", region: "europe" },
        { id: "iceni", name: "Iceni", kind: "tribe", region: "europe" }
    ];
    const input = new FakeElement("search");
    const results = new FakeElement("results");
    const status = new FakeElement("status");
    const picker = pickerApi.attach({ input, results, status, getEntities: () => entities });

    input.value = "icel";
    await input.dispatch("input");
    assert.equal(results.children.length, 1);
    assert.equal(results.children[0].children[0].textContent, "Iceland");
    await results.children[0].dispatch("click");
    assert.equal(input.value, "Iceland");
    assert.equal(picker.getSelected().id, "iceland");

    input.value = "ICENI";
    await input.dispatch("input");
    assert.equal(picker.getSelected().id, "iceni", "exact name input resolves to its stable Atlas ID");
    assert.match(mapRostersHtml, /SOWAtlasEntityPicker\.attach/);
    assert.match(mapRostersHtml, /entityPicker\.select\(selectedEntity\)/);
    assert.match(mapRostersHtml, /entity_id:found\.id,role:/);
    assert.match(mapRostersHtml, /pendingId=found\.id/);
    assert.match(mapRostersHtml, /if\(placeAt\(pendingId,point\)\)entityPicker\?\.clear\(\)/);
    assert.match(campaignMapEditorHtml, /SOWAtlasEntityPicker\.attach/);
    assert.match(campaignMapEditorHtml, /atlasEntityPicker\.getSelected\(\)/);
    assert.match(campaignMapEditorHtml, /color:FACTION_COLORS\[roster\.factions\.length%FACTION_COLORS\.length\]/);
    const episodeEditorScript = campaignMapEditorHtml.match(/<script>\s*([\s\S]*?)<\/script>/);
    assert.ok(episodeEditorScript, "Campaign Studio Episode script exists");
    assert.doesNotThrow(() => new vm.Script(episodeEditorScript[1]));
    assert.match(atlasHtml, /map-rosters-link/);
    assert.match(atlasHtml, /entity=\$\{encodeURIComponent\(selectedId\)\}/);
});


test("campaign studio preserves edits made while a save is in flight", () => {
    assert.match(campaignEditor, /var savedDraft = JSON\.stringify\(state\.definition, null, 2\) \+ "\\n"/);
    assert.match(campaignEditor, /if \(report\.errors\.length\)[\s\S]*?if \(state\.saving\) return false/);
    assert.match(campaignEditor, /finally \{ state\.saving = false; refresh\(\); \}/);
    assert.match(campaignEditor, /body: savedDraft/);
    assert.match(campaignEditor, /JSON\.stringify\(state\.definition, null, 2\) \+ "\\n" !== savedDraft/);
});


test("campaign studio reports each external draft conflict only once per file version", () => {
    assert.match(campaignEditor, /externalChangeTag: null/);
    assert.match(campaignEditor, /var externalTag = responses\.map\(function \(response\) \{ return response\.headers\.get\("ETag"\); \}\)\.join\("\\n"\)/);
    assert.match(campaignEditor, /if \(externalTag === state\.externalChangeTag\) return/);
    assert.match(campaignEditor, /state\.externalChangeTag = null;\s*await load\(\)/);
});


test("campaign studio preview matches the real build submenu and direct nuke action", () => {
    const targetBlock = campaignEngine.match(/const UI_TARGETS = \{([\s\S]*?)\n    \};/);
    assert.ok(targetBlock);
    const selectors = [...targetBlock[1].matchAll(/:\s*'([^']+)'/g)].flatMap((match) => match[1].split(","));
    for (const selector of selectors) {
        const target = selector.trim().replace(/^#sow-(?:hud|menu)\s*/, "");
        const attributes = [...target.matchAll(/\[([^\]]+)\]/g)].map((match) => match[1]);
        const exists = target.startsWith("#")
            ? campaignEditorHtml.includes(`id="${target.slice(1)}"`)
            : attributes.length
                ? attributes.every((attribute) => campaignEditorHtml.includes(attribute))
                : campaignEditorHtml.includes(target);
        assert.ok(exists, `Campaign studio preview is missing ${selector.trim()}`);
    }
    assert.match(campaignEditorHtml, /data-preview-map-menu="build"[\s\S]*?data-map-action="build_city"/);
    assert.match(campaignEditorHtml, /<button[^>]*data-map-action="nuke"/);
    assert.doesNotMatch(campaignEditorHtml, /data-preview-map-menu="nuke"|data-map-group="nuke"/);
    assert.match(campaignEditorCss, /\.sample-menu\{[^}]*z-index:3/);
    assert.match(campaignEditorCss, /#previewRoot\s*\{[^}]*z-index:\s*4/);
    assert.match(hud, /if \(nukeItem\) sectors\.push\(\{ item: nukeItem, kind: "nuke" \}\)/);
    assert.doesNotMatch(hud, /mapMenuView === "nuke"|mapGroupSector\([^;]*"nuke"/);
    assert.match(campaignEngine, /map_nuke: '#sow-hud \[data-map-action="nuke"\]'/);
    assert.equal([...campaignEditorHtml.matchAll(/data-map-back/g)].length, 1);
    assert.doesNotMatch(campaignEditor, /syncPreviewMapMenu/);
    assert.match(campaignEditor, /else if \(group\) hudRoot\.dataset\.previewMapMenu = group\.dataset\.mapGroup;[\s\S]*?paintPreview\(\);/);
    assert.match(campaignEditor, /var menuHint = requiredMenu && \$\("#sow-hud"\)\.dataset\.previewMapMenu !== requiredMenu/);
    assert.match(campaignEditor, /function previewAnchor\(step, facts\)[\s\S]*?target\.getClientRects\(\)\.length/);
});

