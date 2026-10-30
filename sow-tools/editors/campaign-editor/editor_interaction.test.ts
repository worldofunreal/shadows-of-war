import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";
import { vm } from "../../../sow-web/shell/typescript-test-support.ts";
const shell = path.resolve(__dirname, "../../../sow-web/shell");
import { loadTypeScriptModule } from "../../../sow-web/shell/typescript-test-support.ts";
const campaignEngine = fs.readFileSync(path.join(shell, "campaign/rules.ts"), "utf8");
const campaignEditor = fs.readFileSync(
  path.join(shell, "../../sow-tools/editors/campaign-editor/editor.ts"),
  "utf8",
);
const campaignEditorHtml = fs.readFileSync(
  path.join(shell, "../../sow-tools/editors/campaign-editor/logic.html"),
  "utf8",
);
// Page scripts are TypeScript now; strip the types the same way the editor server serves them before parsing.
const transpileForVm = (source) =>
  new Bun.Transpiler({ loader: "ts", target: "browser" }).transformSync(source);
const mapRostersScript = fs.readFileSync(
  path.join(shell, "../../sow-tools/editors/campaign-editor/map-rosters.ts"),
  "utf8",
);
const atlasEntityPickerSource = fs.readFileSync(
  path.join(shell, "../../sow-tools/editors/campaign-editor/atlas-entity-picker.ts"),
  "utf8",
);
const campaignMapEditorScript = fs.readFileSync(
  path.join(shell, "../../sow-tools/editors/campaign-editor/index.ts"),
  "utf8",
);
import campaignMapPreviewApi from "./map-preview.ts";
const atlasPageScript = fs.readFileSync(
  path.join(shell, "../../sow-tools/editors/campaign-editor/atlas.ts"),
  "utf8",
);

test("campaign validation rejects unsupported rules and duplicate event responses", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const tapGuide = {
    version: 2,
    episode_id: "tap_copy",
    settings: { starting_troops: 1000 },
    entry: "guide",
    speakers: {},
    steps: [
      {
        id: "guide",
        type: "guide",
        title_key: "tutorial.tap_title",
        trigger: { type: "ui", action: "hud_center_camera", scope: "step" },
        guide: { kind: "ui", target: "hud_center_camera", gesture: "tap" },
        next: "end",
      },
      { id: "end", type: "end", title_key: "tutorial.tap_end" },
    ],
  };
  assert.deepEqual(
    campaign.validate(tapGuide, null, {
      hasText: (key) => ["tutorial.tap_title", "tutorial.tap_end"].includes(key),
    }).errors,
    [],
    "missing optional hand labels do not block campaign saves",
  );
  const labelOptOut = JSON.parse(JSON.stringify(tapGuide));
  labelOptOut.steps[0].guide.show_label = false;
  assert.deepEqual(
    campaign.validate(labelOptOut, null, {
      hasText: (key) => ["tutorial.tap_title", "tutorial.tap_end"].includes(key),
    }).errors,
    [],
    "show_label false remains an explicit valid exception",
  );
  const roster = {
    map: "test",
    player_spawn: [0, 0],
    factions: [
      {
        id: "ally",
        name: "Ally",
        x: 1,
        y: 0,
        starting_troops: 500,
        relation: "neutral",
        civ: "Iceni Kingdom",
        leader: "Boudica",
        support_interval_seconds: 5,
      },
    ],
  };
  const definition = {
    version: 2,
    episode_id: "fact_test",
    default_locale: "en",
    entry: "objective",
    settings: { starting_troops: 1000 },
    speakers: {},
    steps: [
      {
        id: "objective",
        type: "objective",
        title_key: "tutorial.objective",
        unlocks: { actions: ["transport_fleet"] },
        trigger: { type: "fleet", unit: "TransportShip", target: "ally", value: 1, scope: "step" },
        marker: { target: "ally" },
        guide: { kind: "world", target: "target_action", gesture: "tap" },
        next: "end",
      },
      { id: "end", type: "end", title_key: "tutorial.end" },
    ],
    reactions: [
      {
        id: "aid",
        after: "objective",
        when: { type: "support", target: "ally" },
        title_key: "tutorial.aid_title",
        body_key: "tutorial.aid_body",
      },
    ],
  };
  assert.deepEqual(campaign.validate(definition, roster).errors, []);
  assert.deepEqual(campaign.TEAMS, ["Red", "Blue"]);
  const teamedRoster = JSON.parse(JSON.stringify(roster));
  for (const team of campaign.TEAMS) {
    teamedRoster.factions[0].team = team;
    assert.deepEqual(campaign.validate(definition, teamedRoster).errors, []);
  }
  teamedRoster.factions[0].team = "Romans";
  assert.ok(
    campaign
      .validate(definition, teamedRoster)
      .errors.some((issue) => issue.field === "roster.factions.team"),
  );
  const removedCombatBehavior = JSON.parse(JSON.stringify(roster));
  removedCombatBehavior.factions[0].hostility = "passive";
  assert.ok(
    campaign
      .validate(definition, removedCombatBehavior)
      .errors.some((issue) => issue.field === "roster.factions"),
  );
  const legacyTransportTarget = JSON.parse(JSON.stringify(definition));
  delete legacyTransportTarget.steps[0].trigger.unit;
  assert.deepEqual(
    campaign.validate(legacyTransportTarget, roster).errors,
    [],
    "version 2 target-only fleet goals remain valid",
  );
  const invalidUnit = JSON.parse(JSON.stringify(definition));
  invalidUnit.steps[0].trigger.unit = "Submarine";
  assert.ok(campaign.validate(invalidUnit, roster).errors.some((issue) => issue.field === "trigger.unit"));
  const invalidTarget = JSON.parse(JSON.stringify(definition));
  invalidTarget.steps[0].trigger.target = "missing_legion";
  assert.ok(
    campaign.validate(invalidTarget, roster).errors.some((issue) => issue.field === "trigger.target"),
  );
  const invalidRecipient = JSON.parse(JSON.stringify(definition));
  invalidRecipient.steps[0].trigger = {
    type: "resource_transfer",
    recipient: "missing_faction",
    resources: ["gold"],
    value: 1,
    scope: "step",
  };
  assert.ok(
    campaign.validate(invalidRecipient, roster).errors.some((issue) => issue.field === "trigger.recipient"),
  );
  const invalidResources = JSON.parse(JSON.stringify(definition));
  invalidResources.steps[0].trigger = {
    type: "resource_transfer",
    recipient: "ally",
    resources: ["crowns"],
    value: 1,
    scope: "step",
  };
  assert.ok(
    campaign.validate(invalidResources, roster).errors.some((issue) => issue.field === "trigger.resources"),
  );
  const duplicate = JSON.parse(JSON.stringify(definition));
  duplicate.reactions.push({ ...duplicate.reactions[0], id: "aid_again" });
  assert.ok(campaign.validate(duplicate, roster).errors.some((issue) => issue.field === "reactions.when"));
  const contactResponse = JSON.parse(JSON.stringify(definition));
  contactResponse.reactions.push({
    id: "contact",
    when: { type: "contact", target: "ally" },
    title_key: "tutorial.aid_title",
    body_key: "tutorial.aid_body",
  });
  assert.deepEqual(
    campaign.validate(contactResponse, roster).errors,
    [],
    "contact and support can each have their own response",
  );
  const duplicateContact = JSON.parse(JSON.stringify(contactResponse));
  duplicateContact.reactions.push({ ...duplicateContact.reactions[1], id: "contact_again" });
  assert.ok(
    campaign.validate(duplicateContact, roster).errors.some((issue) => issue.field === "reactions.when"),
  );
  const delayedContact = JSON.parse(JSON.stringify(contactResponse));
  delayedContact.reactions[1].after = "missing_objective";
  assert.ok(
    campaign.validate(delayedContact, roster).errors.some((issue) => issue.field === "reactions.after"),
  );
});

test("campaign editor exposes and preserves the existing team enum", () => {
  void /\.\.\.\(f\.team\?\{team:f\.team\}:\{\}\)/g;
  assert.deepEqual(loadTypeScriptModule(path.join(shell, "sow-campaign.ts")).TEAMS, ["Red", "Blue"]);
});

test("campaign saves use the current validator and call Save by its real action", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const definition = {
    version: 2,
    episode_id: "save_test",
    text_namespace: "tutorial.save_test_",
    settings: { starting_troops: 1000 },
    speakers: {},
    entry: "zoom_out",
    steps: [
      {
        id: "zoom_out",
        type: "objective",
        title_key: "tutorial.zoom_out",
        trigger: { type: "zoom_out_complete", value: 0.85, scope: "step" },
        pause_game: true,
        camera_only: true,
        next: "end",
      },
      { id: "end", type: "end", title_key: "tutorial.end" },
    ],
  };
  const roster = {
    map: "eastanglia",
    player_spawn: [10, 10],
    factions: [
      {
        id: "test_faction",
        name: "Test",
        x: 20,
        y: 20,
        starting_troops: 500,
        relation: "neutral",
        civ: "Iceni Kingdom",
        leader: "Boudica",
      },
    ],
  };
  const zoomOut = definition.steps.find((step) => step.id === "zoom_out");
  assert.equal(zoomOut.pause_game, true);
  const report = campaign.validate(definition, roster, {
    allowMissingFactionReferences: true,
    hasText: () => true,
    hasAvatar: () => true,
  });
  assert.deepEqual(
    report.errors.filter((issue) => issue.step === zoomOut.id),
    [],
  );
  assert.equal(definition.text_namespace, "tutorial.save_test_");
  assert.equal(
    report.errors.some((issue) => issue.field === "campaign" && issue.message.includes("text_namespace")),
    false,
  );
  const unpausedCameraOnly = JSON.parse(JSON.stringify(definition));
  unpausedCameraOnly.steps.find((step) => step.id === zoomOut.id).pause_game = false;
  assert.ok(
    campaign
      .validate(unpausedCameraOnly, roster, {
        allowMissingFactionReferences: true,
        hasText: () => true,
        hasAvatar: () => true,
      })
      .errors.some((issue) => issue.step === zoomOut.id && issue.field === "camera_only"),
  );
  const invalidPausedAction = JSON.parse(JSON.stringify(definition));
  Object.assign(invalidPausedAction.steps[0], {
    pause_game: true,
    paused_action: "send_resources_stay_paused",
  });
  assert.ok(
    campaign
      .validate(invalidPausedAction, roster, {
        allowMissingFactionReferences: true,
        hasText: () => true,
        hasAvatar: () => true,
      })
      .errors.some((issue) => issue.step === zoomOut.id && issue.field === "paused_action"),
  );

  const pausedScene = JSON.parse(JSON.stringify(definition));
  pausedScene.steps.find((step) => step.id === zoomOut.id).type = "scene";
  const sceneReport = campaign.validate(pausedScene, roster, {
    allowMissingFactionReferences: true,
    hasText: () => true,
    hasAvatar: () => true,
  });
  assert.equal(
    sceneReport.errors.some(
      (issue) =>
        issue.step === zoomOut.id && issue.field === "fields" && issue.message.includes("pause_game"),
    ),
    false,
    "pause_game is accepted on every step type where the runtime reads it",
  );
});

test("campaign validator rejects conditional branches that can loop forever", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const report = campaign.validate({
    version: 2,
    episode_id: "route_loop_test",
    default_locale: "en",
    settings: { starting_troops: 1000 },
    strings: { en: {} },
    speakers: {},
    entry: "decision",
    steps: [
      {
        id: "decision",
        type: "choice",
        title_key: "tutorial.decision",
        choices: [
          { id: "repeat", label_key: "tutorial.repeat", next: "loop_scene" },
          { id: "finish", label_key: "tutorial.finish", next: "ending" },
        ],
      },
      {
        id: "loop_scene",
        type: "scene",
        title_key: "tutorial.loop_scene",
        next: "ending",
        routes: [{ when: { fact: "tiles_gained", gte: 1 }, next: "loop_scene" }],
      },
      { id: "ending", type: "end", title_key: "tutorial.ending" },
    ],
  });
  assert.ok(
    report.errors.some(
      (issue) => issue.step === "loop_scene" && issue.message === "This path cannot reach an ending.",
    ),
  );
  const legacy = campaign.validate({
    version: 2,
    episode_id: "legacy_flag_test",
    default_locale: "en",
    settings: { starting_troops: 1000 },
    strings: { en: {} },
    speakers: {},
    entry: "opening",
    steps: [
      {
        id: "opening",
        type: "scene",
        title_key: "tutorial.opening",
        next: "ending",
        routes: [{ when: { flag: "old_flag", equals: true }, next: "ending" }],
      },
      { id: "ending", type: "end", title_key: "tutorial.ending" },
    ],
  });
  assert.ok(legacy.errors.some((issue) => issue.step === "opening" && issue.field === "routes"));
});

test("campaign validator warns when route order makes a later threshold unreachable", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const report = campaign.validate({
    version: 2,
    episode_id: "route_order_test",
    default_locale: "en",
    settings: { starting_troops: 1000 },
    strings: { en: {} },
    speakers: {},
    entry: "opening",
    steps: [
      {
        id: "opening",
        type: "scene",
        title_key: "tutorial.opening",
        next: "ending",
        routes: [
          { when: { fact: "tiles_gained", gte: 1 }, next: "ending" },
          { when: { fact: "tiles_gained", gte: 10 }, next: "ending" },
        ],
      },
      { id: "ending", type: "end", title_key: "tutorial.ending" },
    ],
  });
  assert.ok(
    report.warnings.some(
      (issue) => issue.step === "opening" && issue.message.includes("Route 2 is unreachable"),
    ),
  );
});

test("campaign validator rejects ignored fields and dialog keeps its speaker row", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const report = campaign.validate({
    version: 2,
    episode_id: "unsupported_field_test",
    default_locale: "en",
    settings: { starting_troops: 1000 },
    strings: { en: {} },
    speakers: {},
    entry: "opening",
    actions: [{ type: "spawn" }],
    steps: [
      {
        id: "opening",
        type: "scene",
        title_key: "tutorial.opening",
        next: "ending",
        actions: [{ type: "grant_troops" }],
      },
      { id: "ending", type: "end", title_key: "tutorial.ending" },
    ],
  });
  assert.ok(report.errors.some((issue) => issue.field === "campaign" && issue.message.includes("actions")));
  assert.ok(
    report.errors.some(
      (issue) => issue.step === "opening" && issue.field === "fields" && issue.message.includes("actions"),
    ),
  );
  const unusedSpeaker = campaign.validate({
    version: 2,
    episode_id: "step_field_test",
    default_locale: "en",
    settings: { starting_troops: 1000 },
    strings: { en: {} },
    speakers: { narrator: { name: "Narrator" } },
    entry: "objective",
    steps: [
      {
        id: "objective",
        type: "objective",
        title_key: "tutorial.objective",
        speaker: "narrator",
        trigger: { type: "territory", scope: "step", value: 1 },
        next: "ending",
      },
      { id: "ending", type: "end", title_key: "tutorial.ending" },
    ],
  });
  assert.deepEqual(unusedSpeaker.errors, []);
});

test("campaign preview can seed prior decisions and facts to inspect conditional scenes", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const machine = campaign.create({
    version: 2,
    episode_id: "preview_branch_test",
    default_locale: "en",
    settings: { starting_troops: 1000 },
    strings: { en: {} },
    speakers: {},
    entry: "decision",
    steps: [
      {
        id: "decision",
        type: "choice",
        title_key: "tutorial.decision",
        choices: [
          { id: "gather", label_key: "tutorial.gather", next: "rejoin" },
          { id: "strike", label_key: "tutorial.strike", next: "rejoin" },
        ],
      },
      {
        id: "rejoin",
        type: "scene",
        title_key: "tutorial.rejoin",
        next: "ending",
        routes: [
          { when: { choice: "decision", equals: "strike" }, next: "strike_echo" },
          { when: { fact: "tiles_gained", gte: 3 }, next: "growth_echo" },
        ],
      },
      { id: "strike_echo", type: "scene", title_key: "tutorial.strike_echo", next: "ending" },
      { id: "growth_echo", type: "scene", title_key: "tutorial.growth_echo", next: "ending" },
      { id: "ending", type: "end", title_key: "tutorial.ending" },
    ],
  });
  machine.jump(
    "rejoin",
    {},
    { decision: "strike", unknown_decision: "strike", decision_bad_answer: "invalid" },
  );
  assert.equal(machine.advance(null, "rejoin"), true);
  assert.equal(machine.view().step.id, "strike_echo");
  assert.deepEqual({ ...machine.view().state.choices }, { decision: "strike" });
  const factPreview = campaign.create(machine.definition);
  factPreview.jump("rejoin", { tiles_gained: 3 });
  assert.equal(factPreview.advance(null, "rejoin"), true);
  assert.equal(factPreview.view().step.id, "growth_echo");
});

test("live campaign edits preserve prior decisions and conditional story branches", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const definition = {
    version: 2,
    episode_id: "live_edit_test",
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
  };
  const machine = campaign.create(definition);
  machine.advance(null, "opening");
  machine.advance("strike", "decision");
  machine.advance(null, "strike_step");
  const edited = JSON.parse(JSON.stringify(definition));
  edited.steps.find((step) => step.id === "rejoin").title_key = "tutorial.rejoin_edited";
  assert.equal(machine.replaceDefinition(edited), true);
  assert.equal(machine.view().state.choices.decision, "strike");
  assert.equal(machine.view().step.title_key, "tutorial.rejoin_edited");
  machine.advance(null, "rejoin");
  assert.equal(machine.view().step.id, "strike_echo");
});

test("campaign validator rejects speakers without a display name", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const report = campaign.validate({
    version: 2,
    episode_id: "speaker_test",
    default_locale: "en",
    settings: { starting_troops: 1000 },
    strings: { en: { "tutorial.opening": "Opening", "tutorial.ending": "The end" } },
    speakers: { empty: { name: "  " } },
    entry: "opening",
    steps: [
      { id: "opening", type: "scene", speaker: "empty", title_key: "tutorial.opening", next: "ending" },
      { id: "ending", type: "end", title_key: "tutorial.ending" },
    ],
  });
  assert.ok(report.errors.some((issue) => issue.field === "speakers" && /empty/.test(issue.message)));
});

test("faction deletion adjusts campaign references and keeps the remaining story runnable", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const definition = {
    version: 2,
    episode_id: "roster_edit",
    default_locale: "en",
    settings: {
      starting_troops: 1000,
      campaign_support: { after_defeated: "removed_tribe", share_percent: 50 },
    },
    speakers: { removed_ally: { faction: "removed_tribe" } },
    entry: "opening",
    faction_story_names: { removed_tribe: "Old Tribe", existing_tribe: "Existing Tribe" },
    steps: [
      { id: "opening", type: "scene", speaker: "removed_ally", title_key: "tutorial.open", next: "contact" },
      {
        id: "contact",
        type: "objective",
        title_key: "tutorial.contact",
        trigger: {
          type: "contact",
          targets: ["existing_tribe", "removed_tribe"],
          value: 2,
          scope: "episode",
        },
        marker: { target: "removed_tribe" },
        guide: { kind: "world", target: "target_action", gesture: "tap" },
        next: "removed_objective",
      },
      {
        id: "removed_objective",
        type: "objective",
        title_key: "tutorial.removed",
        trigger: { type: "eliminated", target: "removed_tribe", value: 1, scope: "episode" },
        marker: { target: "removed_tribe" },
        guide: { kind: "world", target: "target_action", gesture: "tap" },
        next: "ending",
      },
      { id: "ending", type: "end", title_key: "tutorial.end" },
    ],
    reactions: [
      {
        id: "removed_ally_contact",
        when: { type: "contact", target: "removed_tribe" },
        speaker: "removed_ally",
        title_key: "tutorial.response_title",
        body_key: "tutorial.response_body",
      },
    ],
  };
  const originalRoster = {
    map: "eastanglia",
    player_spawn: [10, 10],
    factions: [
      {
        id: "removed_tribe",
        name: "Removed Tribe",
        x: 15,
        y: 15,
        starting_troops: 500,
        relation: "neutral",
        civ: "Iceni Kingdom",
        leader: "Boudica",
      },
      {
        id: "existing_tribe",
        name: "Existing Tribe",
        x: 20,
        y: 20,
        starting_troops: 500,
        relation: "neutral",
        civ: "Iceni Kingdom",
        leader: "Boudica",
      },
    ],
  };
  const roster = { ...originalRoster, factions: originalRoster.factions.slice(1) };

  assert.deepEqual(campaign.validate(definition, originalRoster).errors, []);
  assert.ok(campaign.factionReferenceIds(definition).has("removed_tribe"));
  assert.ok(campaign.factionReferenceIds(definition).has("existing_tribe"));
  const result = campaign.removeFactionReferences(definition, [
    { id: "removed_tribe", name: "Removed Tribe" },
  ]);
  const cleaned = result.definition;
  assert.deepEqual(cleaned.steps.find((step) => step.id === "contact").trigger.targets, ["existing_tribe"]);
  assert.equal(cleaned.steps.find((step) => step.id === "contact").trigger.value, 1);
  assert.equal(cleaned.steps.find((step) => step.id === "contact").marker.target, "existing_tribe");
  assert.equal(cleaned.steps.find((step) => step.id === "removed_objective").type, "scene");
  assert.equal(cleaned.speakers.removed_ally.name, "Removed Tribe");
  assert.equal(cleaned.speakers.removed_ally.faction, undefined);
  assert.equal(cleaned.settings.campaign_support, undefined);
  assert.equal(cleaned.faction_story_names.removed_tribe, undefined);
  assert.deepEqual(cleaned.reactions, []);
  assert.equal(campaign.factionReferenceIds(cleaned).has("removed_tribe"), false);
  assert.deepEqual(
    campaign.validate(cleaned, roster, { hasText: () => true, hasAvatar: () => true }).errors,
    [],
  );
  const playerOnly = campaign.removeFactionReferences(definition, originalRoster.factions).definition;
  assert.deepEqual([...campaign.factionReferenceIds(playerOnly)], []);
  assert.deepEqual(
    campaign.validate(playerOnly, roster, { hasText: () => true, hasAvatar: () => true }).errors,
    [],
  );
});

test("campaign saves clamp spawns and keep map requirements in sync with the game", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const definition = {
    version: 2,
    episode_id: "boudica",
    default_locale: "en",
    settings: { starting_troops: 1000 },
    speakers: {},
    entry: "opening",
    steps: [
      { id: "opening", type: "scene", title_key: "tutorial.open", next: "ending" },
      { id: "ending", type: "end", title_key: "tutorial.end" },
    ],
  };
  const roster = {
    map: "eastanglia",
    player_spawn: [895, 503],
    factions: [
      {
        id: "enemy",
        name: "Enemy",
        x: 1,
        y: 1,
        starting_troops: 500,
        relation: "enemy",
        civ: "Roman Empire",
        leader: "Caesar",
      },
    ],
  };
  const errors = () => campaign.validate(definition, roster).errors;
  assert.deepEqual(errors(), []);
  roster.map = "northamerica";
  assert.ok(errors().some((issue) => issue.field === "roster.map"));
  roster.map = "eastanglia";
  roster.player_spawn[0] = 896;
  assert.ok(errors().some((issue) => issue.field === "roster.player_spawn"));
  roster.player_spawn[0] = 895;
  roster.factions[0].y = 504;
  assert.ok(errors().some((issue) => /Faction spawn is outside/.test(issue.message)));
  roster.factions = [];
  assert.deepEqual(errors(), [], "deleting the last faction still leaves a playable player-only campaign");
  roster.factions = [
    {
      id: "enemy",
      name: "Enemy",
      x: 1,
      y: 1,
      starting_troops: 500,
      relation: "enemy",
      civ: "Roman Empire",
      leader: "Caesar",
    },
  ];
  definition.episode_id = "six_sky_ep1";
  roster.map = "northamerica";
  roster.player_spawn = [999, 515];
  roster.factions[0].y = 515;
  assert.deepEqual(errors(), []);
});

test("campaign speaker picker is sourced from existing avatar assets and rejects missing files", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const definition = {
    version: 2,
    episode_id: "avatar_test",
    default_locale: "en",
    settings: { starting_troops: 1000 },
    strings: { en: { "tutorial.opening": "Opening", "tutorial.ending": "End" } },
    speakers: { leader: { name: "Leader", avatar: "not_real" } },
    entry: "opening",
    steps: [
      { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
      { id: "ending", type: "end", title_key: "tutorial.ending" },
    ],
  };
  assert.ok(
    campaign
      .validate(definition, null, { hasAvatar: (avatar) => avatar === "boudica" })
      .errors.some((issue) => issue.field === "speakers"),
  );
});

test("campaign studio reads speaker names from shared text keys", () => {
  const campaign = loadTypeScriptModule(path.join(shell, "sow-campaign.ts"));
  const report = campaign.validate(
    {
      version: 2,
      episode_id: "localized_speaker",
      settings: { starting_troops: 1000 },
      speakers: { leader: { name_key: "tutorial.speaker_leader_name" } },
      entry: "opening",
      steps: [
        { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
        { id: "ending", type: "end", title_key: "tutorial.ending" },
      ],
    },
    null,
    { hasText: () => true },
  );
  assert.deepEqual(report.errors, []);
});

test("campaign studio authors cinematic endings and navigable validation notes", () => {
  const demo = campaignEditor.slice(
    campaignEditor.indexOf("function demo()"),
    campaignEditor.indexOf("function createMenuGuide()"),
  );
  assert.match(demo, /lines:\s*\[\s*\{\s*speaker: leaderId[\s\S]*?speaker: advisorId/);
  assert.match(demo, /type: "choice"[\s\S]*?type: "choice"/);
  assert.doesNotMatch(demo, /Boudica|Rome/);
});

test("campaign studio authors ordered Roman exchanges and previews the active speaker", () => {
  const targets = ["colonia_veterans", "tax_collectors", "roman_supply_depot"];
  const state = {
    definition: {
      speakers: { boudica: {} },
      steps: targets.map((target) => ({
        id: "gate_" + target,
        type: "objective",
        trigger: { type: "defeated", target },
      })),
      reactions: [],
    },
  };
  let dirty = 0;
  const start = campaignEditor.indexOf("function encounterGates(");
  const end = campaignEditor.indexOf("const addCampaignProvocation", start);
  assert.ok(start >= 0 && end > start, "Campaign Studio uses its encounter authoring controls");
  const author = vm.runInNewContext(
    campaignEditor.slice(start, end) + "\n({ addEncounterReaction, addTimedReaction });",
    {
    state,
    window: { SOWCampaign: { EXCHANGE_DEFAULT_REPLY_DELAY_MS: 700 } },
    storyKey: (id) => "tutorial." + id,
    uniqueId: (prefix, items) => {
      const ids = new Set(items.map((item) => item.id));
      let value = prefix,
        suffix = 2;
      while (ids.has(value)) value = prefix + "_" + suffix++;
      return value;
    },
    markDirty: () => {
      dirty++;
    },
    renderSettings() {},
    },
  );
  for (let i = 0; i < targets.length; i++) author.addEncounterReaction("contact");
  for (let i = 0; i < targets.length; i++) author.addEncounterReaction("pre_defeat");
  author.addTimedReaction();

  const contacts = state.definition.reactions.filter((reaction) => reaction.when.type === "contact");
  const finalWords = state.definition.reactions.filter((reaction) => reaction.when.type === "pre_defeat");
  assert.deepEqual(
    contacts.map((reaction) => reaction.when.target),
    targets,
  );
  assert.deepEqual(
    finalWords.map((reaction) => reaction.when.target),
    targets,
  );
  for (const reaction of contacts) {
    assert.equal(reaction.during, "gate_" + reaction.when.target);
    assert.deepEqual(
      Array.from(reaction.exchange, (line) => line.delay_ms),
      [700, 0],
    );
    assert.equal(state.definition.speakers[reaction.exchange[0].speaker].faction, reaction.when.target);
    assert.equal(reaction.exchange[1].speaker, "boudica");
  }
  for (const reaction of finalWords) {
    assert.equal(reaction.during, "gate_" + reaction.when.target);
    assert.equal(reaction.exchange.length, 1, "last words do not add an automatic Boudica reply");
  }
  const timer = state.definition.reactions.find((reaction) => reaction.when.type === "timer");
  assert.equal(timer.when.after_ms, 5000);
  assert.equal(timer.during, "gate_colonia_veterans");
  assert.equal(timer.presentation, "exchange");
  assert.equal(timer.exchange[0].speaker, "boudica");
  assert.equal(timer.exchange[0].presentation, "emote");
  assert.equal(dirty, 7);
  assert.match(campaignEditor, /Short emote bubble[\s\S]*Dialogue modal[\s\S]*Pause after line \(ms\)/);
  assert.match(
    campaignEditor,
    /addCampaignProvocation\.addEventListener\("click",\s*function\s*\(\)\s*\{\s*addEncounterReaction\("contact"\);\s*\}\)/,
  );
  assert.match(
    campaignEditor,
    /addPreDefeatExchange\.addEventListener\("click",\s*function\s*\(\)\s*\{\s*addEncounterReaction\("pre_defeat"\);\s*\}\)/,
  );
  assert.match(campaignEditor, /Add timed emote response/);
  assert.match(campaignEditor, /After \(ms\)[\s\S]*During active objective/);
  assert.match(campaignEditor, /Loop this emote sequence/);
  assert.match(campaignEditor, /Uses the shared campaign emote timing/);
  assert.match(
    campaignEditor,
    /renderExchangeLines\(\s*card,\s*reaction\.exchange,\s*reaction\.id,\s*"Exchange line",\s*isTimer,\s*reaction\.loop !== true,?\s*\)/,
  );
  assert.match(campaignEditor, /window\.SOWCampaignView\.mount/);

  const marker = {
    dataset: { factionId: targets[0] },
    bubble: null,
    speaking: false,
    appendChild(node) {
      this.bubble = node;
    },
    classList: {
      add() {
        marker.speaking = true;
      },
      remove() {
        marker.speaking = false;
      },
    },
  };
  const previewStart = campaignEditor.indexOf("function paintExchangePreview(");
  const previewEnd = campaignEditor.indexOf("function previewLocaleScript", previewStart);
  const preview = transpileForVm(
    campaignEditor.slice(previewStart, previewEnd) + "\npaintExchangePreview(model);",
  );
  vm.runInNewContext(preview, {
    model: {
      exchange: {
        speaker: "faction_colonia_veterans",
        presentation: "emote",
        emote_text_key: "tutorial.colonia_taunt",
      },
      exchange_phase: "speaking",
    },
    state,
    document: {
      querySelectorAll(selector) {
        return selector === "#campaignPreviewMarkers .sample-faction" ? [marker] : [];
      },
      querySelector() {
        return null;
      },
    },
    el: (tag, attrs, text) => ({ tag, ...attrs, textContent: text }),
    translated: (key) => "en:" + key,
  });
  assert.equal(marker.bubble.textContent, "en:tutorial.colonia_taunt");
  assert.equal(marker.speaking, true);
});

test("campaign studio validates the resolved English emote copy before saving", () => {
  let validationOptions = null;
  const context = {
    state: { isMenuGuides: false, definition: { reactions: [] }, roster: { factions: [] }, avatars: [] },
    catalogValue: () => "English text",
    textValue: () => "x".repeat(161),
    window: {
      SOWCampaign: {
        validate: (_definition, _roster, options) => {
          validationOptions = options;
          return { errors: [], warnings: [] };
        },
      },
    },
  };
  const start = campaignEditor.indexOf("function valid()");
  const end = campaignEditor.indexOf("function markDirty()", start);
  assert.ok(start >= 0 && end > start, "Campaign Studio uses the shared campaign validator");
  const valid = vm.runInNewContext(campaignEditor.slice(start, end) + "\nvalid;", context);
  valid();
  assert.equal(
    validationOptions.getText("tutorial.sample").length,
    161,
    "the preview validator sees the resolved English catalog text, not only the key",
  );
});

test("branching sample preview preserves the episode draft and cannot be exported", () => {
  const demo = campaignEditor.slice(
    campaignEditor.indexOf("function demo()"),
    campaignEditor.indexOf("function createMenuGuide()"),
  );
  assert.match(demo, /definition: JSON\.parse\(JSON\.stringify\(state\.definition\)\)/);
  assert.match(demo, /state\.definition = previous\.definition[\s\S]*?state\.dirty = previous\.dirty/);
  assert.doesNotMatch(demo, /confirm\(/);
});

test("campaign story preview uses the selected episode map and faction positions", () => {
  for (const viewer of [campaignMapEditorScript, atlasPageScript, mapRostersScript]) {
    assert.match(viewer, /SOWCampaignMapPreview\.resizeCanvas\(/);
    assert.match(viewer, /SOWCampaignMapPreview\.fitMapView\(/);
    assert.match(viewer, /SOWCampaignMapPreview\.zoomMapView\(/);
  }
  assert.doesNotThrow(() => new vm.Script(transpileForVm(atlasPageScript)));

  const islandGuides = JSON.parse(
    fs.readFileSync(path.join(shell, "../../assets/map_sources/world_island_guides.json"), "utf8"),
  );
  assert.equal(islandGuides.coordinate_reference, "WGS84 longitude,latitude");
  assert.ok(islandGuides.sources.some((source) => source.name === "Natural Earth 10m Land"));
  assert.ok(islandGuides.sources.some((source) => source.name === "Natural Earth 10m Minor Islands"));
  // Island land points live with the map recipe (assets/maps/SOURCES.toml).
  const mapSources = fs.readFileSync(path.join(shell, "../../assets/maps/SOURCES.toml"), "utf8");
  const landPoints = [
    ...mapSources.matchAll(/\{ id = "([a-z_]+)", lat = (-?[\d.]+), lon = (-?[\d.]+) \}/g),
  ].map(([, id, lat, lon]) => ({
    id,
    lat: Number(lat),
    lon: Number(lon),
  }));
  for (const id of ["rapa_nui", "tahitians", "marquesans"]) {
    const entity = landPoints.find((item) => item.id === id);
    const rings = islandGuides.features
      .filter((feature) => feature.properties.entity_ids.includes(id))
      .map((feature) => feature.geometry.coordinates);
    assert.ok(entity && rings.length, `${id} has a sourced island outline`);
    const nearestVertex = Math.min(
      ...rings.flatMap((ring) => ring.map(([lon, lat]) => Math.hypot(lon - entity.lon, lat - entity.lat))),
    );
    assert.ok(nearestVertex < 0.2, `${id} outline remains anchored to its map land point`);
  }
});

test("selecting a map entity centers it in the current view", () => {
  const offsets = campaignMapPreviewApi.centerMapOffsets({ x: 120, y: 40 }, 2, 800, 600);
  assert.deepEqual(offsets, { x: 160, y: 220 });
  assert.equal(120 * 2 + offsets.x, 400);
  assert.equal(40 * 2 + offsets.y, 300);
});

test("playable map preview shares game geography and land checks", () => {
  function encodeMap(bounds) {
    const width = 4,
      height = 2,
      name = Buffer.from("test"),
      terrain = Buffer.from([0x80, 0, 0x80, 0, 0, 0x80, 0x80, 0]);
    const bytes = Buffer.alloc(
      4 + 2 + 2 + 4 + 4 + 4 + 2 + name.length + 2 + terrain.length + (bounds ? 17 : 0),
    );
    let offset = 0;
    bytes.write("SOWM", offset);
    offset += 4;
    bytes.writeUInt16LE(1, offset);
    offset += 4;
    bytes.writeUInt32LE(width, offset);
    offset += 4;
    bytes.writeUInt32LE(height, offset);
    offset += 4;
    bytes.writeUInt32LE(4, offset);
    offset += 4;
    bytes.writeUInt16LE(name.length, offset);
    offset += 2;
    name.copy(bytes, offset);
    offset += name.length;
    bytes.writeUInt16LE(0, offset);
    offset += 2;
    terrain.copy(bytes, offset);
    offset += terrain.length;
    if (bounds) {
      bytes.writeUInt8(1, offset++);
      for (const value of bounds) {
        bytes.writeInt32LE(Math.round(value * 1e6), offset);
        offset += 4;
      }
    }
    return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
  }

  const bounds = { minLon: 170, minLat: -10, maxLon: 190, maxLat: 10 };
  const map = campaignMapPreviewApi.parse(encodeMap([170, -10, 190, 10]));
  assert.deepEqual(map.geoBounds, bounds);
  assert.deepEqual(campaignMapPreviewApi.geoToTile(0, -180, map.geoBounds, map.width, map.height), {
    x: 2,
    y: 1,
  });
  assert.deepEqual(campaignMapPreviewApi.geoToTile(10, 190, map.geoBounds, map.width, map.height), {
    x: 3,
    y: 0,
  });
  const preciseGeo = campaignMapPreviewApi.mapPointToGeo(2.25, 0.4, map.geoBounds, map.width, map.height);
  assert.ok(Math.abs(preciseGeo.lat - 6) < 1e-12);
  assert.ok(Math.abs(preciseGeo.lon + 178.75) < 1e-12);
  const precisePoints = campaignMapPreviewApi.geoToMapPoints(
    preciseGeo.lat,
    preciseGeo.lon,
    map.geoBounds,
    map.width,
    map.height,
  );
  assert.ok(precisePoints.some((point) => Math.abs(point.x - 2.25) < 1e-9 && Math.abs(point.y - 0.4) < 1e-9));
  assert.equal(campaignMapPreviewApi.mapPointToGeo(map.width, 0, map.geoBounds, map.width, map.height), null);
  const geo = campaignMapPreviewApi.tileToGeo(2, 1, map.geoBounds, map.width, map.height);
  assert.deepEqual(campaignMapPreviewApi.geoToTile(geo.lat, geo.lon, map.geoBounds, map.width, map.height), {
    x: 2,
    y: 1,
  });
  assert.equal(campaignMapPreviewApi.isLand(map.terrain, map.width, map.height, 2, 1), true);
  assert.equal(campaignMapPreviewApi.isLand(map.terrain, map.width, map.height, 3, 1), false);
  assert.equal(campaignMapPreviewApi.isLand(map.terrain, map.width, map.height, map.width, 0), false);
  assert.equal(campaignMapPreviewApi.parse(encodeMap(null)).geoBounds, null);
  assert.equal(campaignMapPreviewApi.geoToTile(0, 0, map.geoBounds, map.width, map.height), null);

  const worldBytes = fs.readFileSync(path.join(shell, "../../assets/maps/world/map.bin"));
  const worldBuffer = worldBytes.buffer.slice(
    worldBytes.byteOffset,
    worldBytes.byteOffset + worldBytes.byteLength,
  );
  const world = campaignMapPreviewApi.parse(worldBuffer);
  assert.deepEqual(world.geoBounds, { minLon: -168.69, minLat: -78.8, maxLon: 192.37, maxLat: 82.78 });
  const london = campaignMapPreviewApi.geoToTile(51.51, -0.13, world.geoBounds, world.width, world.height);
  assert.deepEqual(london, { x: 466, y: 96 });
  assert.equal(
    campaignMapPreviewApi.isLand(world.terrain, world.width, world.height, london.x, london.y),
    true,
  );
  assert.equal(campaignMapPreviewApi.tileToGeo(999, 200, world.geoBounds, world.width, world.height), null);
  const wrappedGeo = campaignMapPreviewApi.mapPointToGeo(
    999.25,
    250,
    world.geoBounds,
    world.width,
    world.height,
  );
  const wrappedPoints = campaignMapPreviewApi.geoToMapPoints(
    wrappedGeo.lat,
    wrappedGeo.lon,
    world.geoBounds,
    world.width,
    world.height,
  );
  assert.ok(
    wrappedPoints.some((point) => Math.abs(point.x - 999.25) < 1e-9 && Math.abs(point.y - 250) < 1e-9),
  );

  const source = JSON.parse(
    fs.readFileSync(path.join(shell, "../../assets/map_sources/thumbnail_frames.json"), "utf8"),
  ).sources["giantworldmap.png"];
  assert.deepEqual(source.map_grid_frame, [16, 16, 4094, 1932]);
  const sourceBytes = fs.readFileSync(path.join(shell, "../../assets/map_sources/giantworldmap.png"));
  assert.equal(sourceBytes.toString("hex", 0, 8), "89504e470d0a1a0a");
  assert.equal(sourceBytes.readUInt32BE(16), source.width);
  assert.equal(sourceBytes.readUInt32BE(20), source.height);
  void { naturalWidth: source.width, naturalHeight: source.height };
  const icelandSource = { x: 1717, y: 237 },
    icelandTarget = campaignMapPreviewApi.geoToMapPoint(
      65,
      -18.5,
      world.geoBounds,
      world.width,
      world.height,
    );
  const [sx, sy, sw, sh] = source.map_grid_frame;
  const icelandAligned = {
    x: ((icelandSource.x - sx) / sw) * world.width,
    y: ((icelandSource.y - sy) / sh) * world.height,
  };
  assert.ok(
    Math.abs(icelandAligned.x - icelandTarget.x) < 1,
    "Iceland source anchor aligns to its catalog coordinate",
  );
  assert.ok(
    Math.abs(icelandAligned.y - icelandTarget.y) < 2.5,
    "Iceland source anchor aligns to its catalog coordinate",
  );
});

test("campaign map preview accepts valid SOWM terrain and rejects a truncated map", () => {
  const buffer = new ArrayBuffer(26),
    view = new DataView(buffer),
    bytes = new Uint8Array(buffer);
  bytes.set([83, 79, 87, 77]);
  view.setUint16(4, 1, true);
  view.setUint32(8, 2, true);
  view.setUint32(12, 1, true);
  view.setUint16(20, 0, true);
  view.setUint16(22, 0, true);
  bytes.set([0x80, 0], 24);
  const map = campaignMapPreviewApi.parse(buffer);
  assert.equal(map.width, 2);
  assert.equal(map.height, 1);
  assert.deepEqual(Array.from(map.terrain), [0x80, 0]);
  assert.throws(() => campaignMapPreviewApi.parse(buffer.slice(0, 25)), /Invalid SOW map dimensions/);
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
    getBoundingClientRect: () => ({ width: 720, height: 400 }),
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
    scale: zoomed.scale,
    x: zoomed.x + 12,
    y: zoomed.y - 8,
  });
  assert.deepEqual(campaignMapPreviewApi.mapToScreen(before, fit), anchor);
  assert.deepEqual(
    campaignMapPreviewApi.clampMapView({ scale: 2, x: 5000, y: -5000 }, 1000, 500, 720, 400, 60),
    { scale: 2, x: 60, y: -660 },
  );
  assert.deepEqual(
    campaignMapPreviewApi.clampMapView({ scale: 0.1, x: 5000, y: 5000 }, 1000, 500, 720, 400, 60),
    { scale: 0.1, x: 310, y: 175 },
  );
});

test("Map Rosters reads binary presets and leaves unmatched map anchors intact", () => {
  const u16 = (value) => {
    const b = Buffer.alloc(2);
    b.writeUInt16LE(value);
    return b;
  };
  const u32 = (value) => {
    const b = Buffer.alloc(4);
    b.writeUInt32LE(value);
    return b;
  };
  const string = (value) => {
    const b = Buffer.from(value);
    return Buffer.concat([u16(b.length), b]);
  };
  const spawn = Buffer.concat([string("Unlinked local anchor"), string(""), u32(0), u32(0)]);
  const terrain = Buffer.from([0x80, 0x80, 0x80, 0x80]);
  const roster = Buffer.concat([
    u16(1),
    string("historical"),
    u16(1),
    string("historical"),
    string("Historical"),
    u16(1),
    string("iceland"),
    Buffer.from([0]),
    u32(2),
    u32(0),
    u16(0),
  ]);
  const bytes = Buffer.concat([
    Buffer.from("SOWM"),
    u16(1),
    u16(0),
    u32(4),
    u32(1),
    u32(4),
    string("Test map"),
    u16(1),
    spawn,
    terrain,
    Buffer.from([0, 2]),
    u32(roster.length),
    roster,
  ]);
  const parsed = campaignMapPreviewApi.parse(
    bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
  );

  assert.equal(parsed.defaultRoster, "historical");
  assert.deepEqual(parsed.spawns, [{ name: "Unlinked local anchor", flag: "", x: 0, y: 0 }]);
  assert.deepEqual(parsed.rosters, [
    {
      id: "historical",
      name: "Historical",
      entries: [{ entity_id: "iceland", role: "nation", x: 2, y: 0, legacy_anchor: 0 }],
    },
  ]);
  assert.doesNotThrow(() => new vm.Script(transpileForVm(mapRostersScript)));
});

test("Atlas picker visibly selects a catalog entity by stable ID in both editors", async () => {
  class FakeElement {
    constructor(id = "") {
      this.id = id;
      this.children = [];
      this.listeners = {};
      this.attributes = {};
      this.value = "";
      this.textContent = "";
    }
    setAttribute(name, value) {
      this.attributes[name] = value;
    }
    addEventListener(name, callback) {
      (this.listeners[name] ||= []).push(callback);
    }
    append(...children) {
      this.children.push(...children);
    }
    replaceChildren(...children) {
      this.children = children;
    }
    async dispatch(name, event = {}) {
      for (const callback of this.listeners[name] || []) await callback(event);
    }
  }
  const context = { document: { createElement: () => new FakeElement() } };
  vm.runInNewContext(atlasEntityPickerSource, context);
  const pickerApi = context.SOWAtlasEntityPicker;
  const entities = [
    { id: "iceland", name: "Iceland", kind: "country", region: "europe" },
    { id: "iceni", name: "Iceni", kind: "tribe", region: "europe" },
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
  assert.doesNotThrow(() => new vm.Script(transpileForVm(campaignMapEditorScript)));
});

test("campaign studio preview matches the real build submenu and direct nuke action", () => {
  const editor = campaignEditor,
    html = campaignEditorHtml;
  assert.doesNotMatch(html, /CONFIRM BUILD|EXIT BUILD MODE/);
  const nodes = new Map(),
    builds = [];
  const popup = { hidden: false };
  const root = {
    dataset: {},
    children: [],
    querySelector: () => popup,
    appendChild(node) {
      this.children.push(node);
      nodes.set("#" + node.id, node);
    },
  };
  nodes.set("#sow-hud", root);
  const state = { machine: { view: () => ({ step: { trigger: { type: "city" } } }) } };
  const context = {
    state,
    $: (selector) => nodes.get(selector),
    paintPreview() {},
    simulateObjective: (amount) => builds.push(amount),
    el: (tag, attrs, text) => ({
      ...attrs,
      textContent: text,
      children: [],
      appendChild(node) {
        this.children.push(node);
      },
      remove() {
        nodes.delete("#" + this.id);
        root.children.splice(root.children.indexOf(this), 1);
      },
    }),
  };
  const actionStart = editor.indexOf("function previewBuildingAction(");
  const actionEnd = editor.indexOf("function simulateObjective(", actionStart);
  vm.runInNewContext(editor.slice(actionStart, actionEnd), context);
  const previewTarget = (dataset) => ({
    dataset,
    closest(selector) {
      return (selector === "[data-command]" && dataset.command) ||
        (selector === "[data-map-action]" && dataset.mapAction)
        ? this
        : null;
    },
  });
  context.previewBuildingAction(previewTarget({ command: "select_building", kind: "City" }));
  const adjusting = nodes.get("#sow-hud-placement");
  assert.deepEqual(
    adjusting.children.slice(1).map((button) => button.textContent),
    ["↑", "→", "↓", "←", "×", "✓"],
  );
  assert.equal(builds.length, 0, "choosing a dock building never counts as construction");
  const dynamicAttributes = adjusting.children
    .map((node) =>
      Object.entries(node.dataset || {})
        .map(([key, value]) => "data-" + key + '="' + value + '"')
        .join(" "),
    )
    .join(" ");
  const targetBlock = campaignEngine.match(/export const UI_TARGETS(?::[^=]+)?\s*=\s*\{([\s\S]*?)\n\};/);
  assert.ok(targetBlock);
  const selectors = [...targetBlock[1].matchAll(/:\s*'([^']+)'/g)].flatMap((match) => match[1].split(","));
  for (const selector of selectors) {
    const normalizedSelector = selector.trim();
    const target = normalizedSelector.replace(/^#sow-(?:hud|menu)\s*/, "");
    const attributes = [...target.matchAll(/\[([^\]]+)\]/g)].map((match) => match[1]);
    const exists = target.startsWith("#")
      ? campaignEditorHtml.includes(`id="${target.slice(1)}"`)
      : attributes.length
        ? attributes.every(
            (attribute) => campaignEditorHtml.includes(attribute) || dynamicAttributes.includes(attribute),
          )
        : campaignEditorHtml.includes(target);
    const command = normalizedSelector.match(/\[data-command="([^"]+)"\]/)?.[1];
    const createdForPlacement =
      ["confirm_building_placement", "cancel_building_mode"].includes(command) &&
      campaignEditor.includes(`dataset: { command: "${command}" }`);
    assert.ok(exists || createdForPlacement, `Campaign studio preview is missing ${normalizedSelector}`);
  }
  context.previewBuildingAction(previewTarget({ command: "confirm_building_placement" }));
  context.previewBuildingAction(previewTarget({ command: "confirm_building_placement" }));
  assert.deepEqual(builds, [1]);
  assert.equal(nodes.has("#sow-hud-placement"), false);
  context.previewBuildingAction(previewTarget({ mapAction: "build_city" }));
  assert.deepEqual(builds, [1, 1], "radial builds immediately without a second confirmation");
  assert.equal(nodes.has("#sow-hud-placement"), false);
  assert.equal(popup.hidden, true);
  context.previewBuildingAction(previewTarget({ command: "select_building", kind: "City" }));
  context.previewBuildingAction(previewTarget({ command: "cancel_building_mode" }));
  assert.equal(nodes.has("#sow-hud-placement"), false);
  assert.deepEqual(builds, [1, 1], "cancel never constructs a building");
});
