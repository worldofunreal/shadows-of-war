const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const shell = __dirname;
const coreSource = fs.readFileSync(path.join(shell, "main_menu.core.js"), "utf8");
const shellSource = fs.readFileSync(path.join(shell, "main_menu.shell.js"), "utf8");
const storeSource = fs.readFileSync(path.join(shell, "main_menu.store.js"), "utf8");
const heroesSource = fs.readFileSync(path.join(shell, "main_menu.heroes.js"), "utf8");
const profileCss = fs.readFileSync(path.join(shell, "main_menu.profile.css"), "utf8");
const loaderSource = fs.readFileSync(path.join(shell, "loader.js"), "utf8");
const pokiSource = fs.readFileSync(path.join(shell, "main_menu.poki.js"), "utf8");
const lobbiesSource = fs.readFileSync(path.join(shell, "main_menu.lobbies.js"), "utf8");
const tutorial = fs.readFileSync(path.join(shell, "main_menu.tutorial.js"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");
const hudCss = fs.readFileSync(path.join(shell, "main_menu.hud.css"), "utf8");
const indexTemplate = fs.readFileSync(path.join(shell, "index.html.template"), "utf8");
const windowInput = fs.readFileSync(path.join(shell, "../../sow-client/src/input/window.rs"), "utf8");
const sessionSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/session.rs"), "utf8");
const actionsSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/interact/actions.rs"), "utf8");
const netUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/update/mod.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const progressSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/progress.rs"), "utf8");
const dataDbSource = fs.readFileSync(path.join(shell, "../../sow-data/src/db.rs"), "utf8");
const dataMainSource = fs.readFileSync(path.join(shell, "../../sow-data/src/main.rs"), "utf8");
const accountSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/account.rs"), "utf8");
const bootstrapSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/bootstrap.rs"), "utf8");
const identitySource = fs.readFileSync(path.join(shell, "../../sow-client/src/anonymous_identity.rs"), "utf8");
const assetSource = fs.readFileSync(path.join(shell, "../../sow-client/src/asset.rs"), "utf8");
const relaySource = fs.readFileSync(path.join(shell, "../../sow-relay/src/main.rs"), "utf8");
const serverSource = fs.readFileSync(path.join(shell, "../../sow-server/src/main.rs"), "utf8");
const mapClick = fs.readFileSync(path.join(shell, "../../sow-client/src/input/map_click.rs"), "utf8");
const worldOverlays = fs.readFileSync(path.join(shell, "../../sow-client/src/render/world/overlays.rs"), "utf8");
const menuCssFiles = [
    "main_menu.base.css",
    "main_menu.layout.css",
    "main_menu.lobbies.css",
    "main_menu.create.css",
    "main_menu.queue.css",
    "main_menu.settings.css",
    "main_menu.auth.css",
    "main_menu.campaign.css"
];
const menuCss = Object.fromEntries(menuCssFiles.map((file) => [file, fs.readFileSync(path.join(shell, file), "utf8")]));

function renderSettingsBody(source, endMarker) {
    const start = source.indexOf("function renderSettings()");
    const end = source.indexOf(endMarker, start);
    assert.notEqual(start, -1);
    assert.notEqual(end, -1);
    return source.slice(start, end);
}

test("menu CSS keeps shared layout separate from live screen domains", () => {
    for (const file of menuCssFiles) assert.ok(menuCss[file].length > 0, file);
    assert.match(menuCss["main_menu.base.css"], /#sow-menu/);
    assert.match(menuCss["main_menu.layout.css"], /sow-menu__main-nav/);
    assert.match(menuCss["main_menu.lobbies.css"], /sow-menu__lobbies/);
    assert.match(menuCss["main_menu.create.css"], /sow-create__/);
    assert.match(menuCss["main_menu.queue.css"], /sow-menu__queue-summary-card/);
    assert.match(menuCss["main_menu.settings.css"], /sow-menu__settings-modal/);
    assert.match(menuCss["main_menu.auth.css"], /sow-auth__/);
    assert.match(menuCss["main_menu.campaign.css"], /sow-campaign__/);
    assert.doesNotMatch(menuCss["main_menu.base.css"], /sow-(create|auth|campaign)__/);
    assert.doesNotMatch(menuCss["main_menu.base.css"], /\.sow-menu__queue(?![-\w])/);
    assert.doesNotMatch(menuCss["main_menu.layout.css"], /\.sow-menu__queue(?![-\w])/);
    assert.match(lobbiesSource, /modeClass = "sow-menu__mode-chip--"/);
    assert.match(shellSource, /sow-auth__provider-icon--/);
});

test("match rewards present in the main menu, while endgame keeps only match stats", () => {
    assert.match(hud, /id="sow-hud-endgame-modal"/);
    assert.match(hud, /endgame\.classList\.toggle\("hidden", !isOver\)/);
    assert.match(webMenu, /endgame_active,[\s\S]*endgame_winner:[\s\S]*endgame_team:/);
    assert.match(hud, /id="sow-hud-surrender-kda"/);
    assert.doesNotMatch(hud, /sow-hud-endgame-(xp|leader-xp|crowns|laurels|rewards)/);
    assert.doesNotMatch(sessionSource, /submit_online_stats_on_exit/);
    assert.doesNotMatch(progressSource, /submit_online_stats|SubmitMatchReport|SubmitStatsWithLeader/);
    assert.match(progressSource, /capture_online_reward_preview[\s\S]*RewardInput::default\(\)[\s\S]*preview_participation_laurels/);
    assert.doesNotMatch(progressSource, /reward_cache/);
    assert.match(dataDbSource, /pub async fn record_match_participation/);
    assert.match(dataDbSource, /reward_receipts\.contains_key/);
    assert.doesNotMatch(shellSource, /data-reward-summary|reward-toast/);
    assert.match(shellSource, /sow:loader-cycle-ready/);
    assert.match(shellSource, /runRewardPresentation/);
    assert.match(shellSource, /createRewardStage\(stages/);
    assert.match(shellSource, /revealRewardAmounts\(items, token\)/);
    assert.match(shellSource, /flyRewardCard\(item, token\)\.then[\s\S]*animateProgressionTo\(nextValues, 380, token\)/);
    assert.match(shellSource, /2 \* inverse \* t \* controlX \+ t \* t \* dx/);
    assert.match(shellSource, /gems: from\.gems \+ \(target\.gems - from\.gems\) \* eased/);
    assert.doesNotMatch(shellSource, /writeProgression\(Object\.assign\(\{\}, displayedProgression, \{ gems: Number\(state\.gems\)/);
    assert.match(shellSource, /function pulseRewardCounter\(selector, levelUp\)/);
    assert.match(menuCss["main_menu.base.css"], /\.sow-menu__reward-stage/);
    assert.match(menuCss["main_menu.base.css"], /sow-reward-card-reveal/);
    assert.match(menuCss["main_menu.base.css"], /sow-reward-value-impact/);
    assert.match(shellSource, /acknowledge_reward_receipts/);
    assert.match(shellSource, /acknowledge_reward_presentation/);
    assert.match(shellSource, /send\("acknowledge_reward_receipts", \{ account_id: accountId, receipt_ids: ids \}\)/);
    assert.match(shellSource, /send\("acknowledge_reward_presentation", \{ account_id: accountId, receipt_id: preview\.receipt_id \}\)/);
    assert.match(webMenu, /AcknowledgeRewardReceipts \{[\s\S]*account_id,[\s\S]*receipt_ids,[\s\S]*progress_account_id\.as_deref\(\) == Some\(account_id\.as_str\(\)\)/);
    assert.match(webMenu, /AcknowledgeRewardPresentation \{[\s\S]*account_id,[\s\S]*receipt_id,[\s\S]*progress_account_id\.as_deref\(\) == Some\(account_id\.as_str\(\)\)/);
    assert.match(shellSource, /if \(!shown\) \{\s*writeProgression\(progressionFromState\(\)\);\s*if \(document\.visibilityState === "visible"\) maybePresentRewards\(\);/);
    assert.doesNotMatch(relaySource, /record_client_stats|record_match_report|SubmitStatsWithLeader|SubmitMatchReport/);
    assert.match(serverSource, /SubmitStatsWithLeader \{ \.\. \}[\s\S]*SubmitMatchReport \{ \.\. \} => \{\}/);
    assert.doesNotMatch(webMenu, /hud\.rewards/);
    assert.match(coreSource, /receipt\.leader_xp/);
    assert.match(heroesSource, /var portraitAsset = asset\([\s\S]*_mobile\.webp[\s\S]*var landscapeAsset = asset\([\s\S]*_desktop\.webp[\s\S]*srcset='" \+ esc\(landscapeAsset\)[\s\S]*img src='" \+ esc\(portraitAsset\)/);
});

test("participation receipt stays synced until replay verification resolves", () => {
    assert.match(dataDbSource, /pub async fn mark_match_reward_verification_status/);
    assert.match(dataDbSource, /receipt\.verification_status = Some\(ReplayVerificationStatus::Verified\)/);
    assert.match(dataMainSource, /async fn reject_queued_replay/);
    assert.match(dataMainSource, /else if let Err\(error\) = reject_queued_replay\(&state, &payload\.match_id\)\.await/);
    assert.match(accountSource, /receipt\.verification_status\s*!=\s*Some\(sow_data::profile::ReplayVerificationStatus::Pending\)/);
    assert.match(accountSource, /receipt\.verification_status\s*==\s*Some\(sow_data::profile::ReplayVerificationStatus::Pending\)/);
    assert.match(coreSource, /receipt\.verification_status/);
});

test("pending reward lookups survive reload without crossing accounts", () => {
    assert.match(identitySource, /PENDING_REWARD_RECEIPTS_STORAGE_PREFIX/);
    assert.match(identitySource, /load_pending_reward_receipt_ids\(account_id: &str\)/);
    assert.match(accountSource, /track_reward_receipt_sync[\s\S]*save_pending_reward_receipt_ids/);
    assert.match(accountSource, /load_pending_reward_receipt_ids\(&account_id\)/);
    assert.match(accountSource, /save_pending_reward_receipt_ids\([\s\S]*&account_id,[\s\S]*&self\.pending_reward_receipt_ids/);
    assert.match(bootstrapSource, /stored_account_id[\s\S]*load_pending_reward_receipt_ids/);
});

test("queued identity refresh is applied before an older profile response", () => {
    const profileStart = assetSource.indexOf("DbEvent::ProfileLoaded {");
    const profileEnd = assetSource.indexOf("DbEvent::DisplayNameSaved", profileStart);
    assert.ok(profileStart >= 0 && profileEnd > profileStart);
    const profileEvent = assetSource.slice(profileStart, profileEnd);
    assert.match(profileEvent, /profile_refresh_pending[\s\S]*fetch_cloud_progress\(\);[\s\S]*continue;/);
    assert.ok(profileEvent.indexOf("continue;") < profileEvent.indexOf("profile_last_applied_request = request_id"));

    const failedStart = assetSource.indexOf("DbEvent::LoadFailed { request_id, status }");
    const failedEnd = assetSource.indexOf("DbEvent::TutorialCompletionFailed", failedStart);
    assert.ok(failedStart >= 0 && failedEnd > failedStart);
    assert.match(assetSource.slice(failedStart, failedEnd), /profile_refresh_pending[\s\S]*fetch_cloud_progress\(\);[\s\S]*continue;/);
});

test("lobby cards keep live counts and one DOM card per lobby id", () => {
    assert.match(lobbiesSource, /function lobbyPlayerCount\(lobby\)/);
    assert.match(lobbiesSource, /function lobbyStatusText\(lobby\)/);
    assert.match(lobbiesSource, /data-lobby-status-for/);
    assert.match(lobbiesSource, /data-lobby-count/);
    assert.match(lobbiesSource, /var seen = Object\.create\(null\)/);
    assert.match(lobbiesSource, /card\.remove\(\)/);
    assert.doesNotMatch(lobbiesSource, /sow-menu__lobby-join/);
    assert.doesNotMatch(shellSource, /cardTimers|data-timer-for|lobbyTimerText/);
    assert.match(lobbiesSource, /data-command='join_lobby'/);
    assert.match(lobbiesSource, /menu\.join/);
    assert.match(menuCss["main_menu.lobbies.css"], /\.sow-menu__lobby-count/);
    assert.doesNotMatch(menuCss["main_menu.lobbies.css"], /\.sow-menu__lobby-join/);
});

test("lobby exit keeps the live connection and match exit owns teardown", () => {
    assert.match(sessionSource, /fn leave_lobby_to_main_menu\(&mut self\)/);
    assert.match(sessionSource, /fn finish_lobby_exit_to_main_menu\(&mut self\)/);
    const lobbyActionStart = actionsSource.indexOf("UiAction::LeaveLobby =>");
    const matchActionStart = actionsSource.indexOf("UiAction::ReturnToMenu =>");
    assert.ok(lobbyActionStart >= 0 && matchActionStart > lobbyActionStart);
    const lobbyAction = actionsSource.slice(lobbyActionStart, matchActionStart);
    assert.match(lobbyAction, /leave_lobby_to_main_menu\(\)/);
    assert.doesNotMatch(lobbyAction, /begin_exit_to_main_menu|client = None|reconnect_now/);
    const matchAction = actionsSource.slice(matchActionStart);
    assert.match(matchAction, /send_leave_message\(\);[\s\S]*begin_exit_to_main_menu\(\);/);
    assert.match(webMenu, /WebMenuCommand::ReturnToMenu[\s\S]*UiAction::ReturnToMenu/);
    assert.doesNotMatch(hud, /send\("leave_lobby"\)/);
    assert.match(hud, /send\("return_to_menu"\)/);
    assert.doesNotMatch(sessionSource, /reconnect_now/);
    assert.doesNotMatch(netUpdateSource, /reconnect_now/);
});

test("game exit resets Battle and replays the screen entrance after the loader", () => {
    assert.match(coreSource, /var lastMenuPhase = null/);
    assert.match(coreSource, /var pendingExitScreenIntro = false/);
    const resetStart = shellSource.indexOf("if (returnedFromGame) {");
    const resetEnd = shellSource.indexOf("if (typeof window.SOW_tutorial_menu_state_update", resetStart);
    assert.ok(resetStart >= 0 && resetEnd > resetStart);
    const resetBlock = shellSource.slice(resetStart, resetEnd);
    for (const flag of ["profileOpen", "heroesOpen", "campaignOpen", "storeOpen"]) {
        assert.match(resetBlock, new RegExp(flag + " = false"));
    }
    assert.match(resetBlock, /pendingExitScreenIntro = true/);
    assert.match(shellSource, /sow:loader-cycle-ready/);
    assert.match(shellSource, /previousScreen = null;[\s\S]*render\(\);/);
    assert.match(loaderSource, /sow:loader-cycle-ready/);
    assert.match(loaderSource, /if \(!loaderReadyDispatched\)/);
    assert.ok(shellSource.indexOf("if (returnedFromGame)") < shellSource.indexOf("SOW_open_store_after_match"));
    assert.match(menuCss["main_menu.layout.css"], /sow-screen-panel-enter 240ms/);
});

test("settings panel keeps only useful controls and real account state", () => {
    const settings = [
        renderSettingsBody(shellSource, "function authApi"),
        renderSettingsBody(pokiSource, "function renderFooter")
    ];
    for (const body of settings) {
        assert.match(body, /menu\.settings/);
        assert.match(body, /data-command='toggle_settings'/);
        assert.match(body, /menu\.music_volume/);
        assert.match(body, /menu\.motion_animation/);
        assert.match(body, /menu\.language/);
        assert.match(body, /account-value/);
        assert.match(body, /settings-account/);
        assert.doesNotMatch(body, /system_configuration|menu\.done|master_audio|settings-mute|menu\.account|account-row|WOU-ID|\+1/);
    }
    assert.match(shellSource, /SOW_getWouSession/);
    assert.match(shellSource, /data-command='confirm_sign_out'/);
    assert.match(shellSource, /data-command='cancel_sign_out'/);
    assert.match(shellSource, /data-menu-overlay='settings'/);
    assert.match(shellSource, /event\.target === settingsOverlay/);
    assert.match(shellSource, /event\.key === "Escape" && settingsOpen/);
    assert.doesNotMatch(lobbiesSource, /lobbies\.connecting_server/);
});

test("hero purchase stays server-backed, direct, and visually honest", () => {
    assert.match(storeSource, /var leaderActions = !offer \|\| offer\.owned \? ""/);
    assert.doesNotMatch(storeSource, /offer\.free_rotation \|\| offer\.available/);
    assert.match(storeSource, /data-command='unlock_leader'/);
    assert.match(storeSource, /data-command='open_skin_purchase'/);
    assert.match(storeSource, /sow-store__currency-amount--insufficient/);
    assert.match(storeSource, /command = confirm \? "open_product_purchase" : "buy_product"/);
    assert.match(storeSource, /skin\.direct_product_id, SOW_t\("store\.buy"\), true/);
    assert.match(shellSource, /data-menu-overlay='purchase'/);
    assert.match(shellSource, /openLeaderPurchase/);
    assert.match(shellSource, /send\("unlock_leader", \{ leader_id: purchaseModal\.leaderId, currency: purchaseModal\.currency \}\)/);
    assert.match(shellSource, /send\("unlock_skin", \{ skin_id: purchaseModal\.skinId \}\)/);
    assert.match(storeSource, /purchaseModal\.phase = "success"/);
    assert.match(storeSource, /purchaseItemOwned/);
    assert.match(storeSource, /reducedRewardMotion\(\) \? " is-reduced-motion"/);
    assert.match(storeSource, /sow-purchase-modal__visual/);
    assert.match(menuCss["main_menu.base.css"], /sow-purchase-modal__layout/);
    assert.match(menuCss["main_menu.base.css"], /sow-purchase-art-reveal/);
    assert.match(menuCss["main_menu.base.css"], /sow-purchase-modal-particle/);
});

test("hero selection opens the shared catalog for equipped and purchasable skins", () => {
    const rowStart = storeSource.indexOf("function renderLeaderPurchase");
    const rowEnd = storeSource.indexOf("function renderSkinPickerModal", rowStart);
    assert.ok(rowStart >= 0 && rowEnd > rowStart);
    assert.match(storeSource.slice(rowStart, rowEnd), /data-command='open_skin_picker'/);
    assert.doesNotMatch(storeSource.slice(rowStart, rowEnd), /if \(!offer \|\| offer\.owned\) return/);
    assert.match(storeSource, /function renderSkinPickerModal\(\)[\s\S]*skins\.map\(renderStoreSkinPromo\)/);
    assert.match(storeSource, /data-menu-overlay='skin-picker'/);
    assert.match(storeSource, /data-command='equip_skin'/);
    assert.match(storeSource, /data-command='open_skin_purchase'/);
    assert.match(shellSource, /command === "open_skin_picker"[\s\S]*loadStoreCatalog\(\)/);
    assert.match(shellSource, /command === "close_skin_picker"/);
    assert.match(shellSource, /syncOverlay\("skin-picker"[\s\S]*syncOverlay\("purchase"/);
    assert.match(profileCss, /\.sow-menu__modal\.sow-skin-picker/);
});

test("weekly free rotation is a splash-only status", () => {
    const cardStart = heroesSource.indexOf("function renderHeroesCard");
    const cardEnd = heroesSource.indexOf("function heroesRoster", cardStart);
    assert.ok(cardStart >= 0 && cardEnd > cardStart);
    assert.doesNotMatch(heroesSource.slice(cardStart, cardEnd), /data-hero-status|weekly_free_rotation/);
    const predicateStart = heroesSource.indexOf("function isWeeklyFreeRotation");
    const predicateEnd = heroesSource.indexOf("function renderHeroesCard", predicateStart);
    assert.ok(predicateStart >= 0 && predicateEnd > predicateStart);
    const context = {};
    vm.runInNewContext(heroesSource.slice(predicateStart, predicateEnd) + "this.isWeeklyFreeRotation = isWeeklyFreeRotation;", context);
    assert.equal(context.isWeeklyFreeRotation({ available: false, free_rotation: true, owned: false }), false);
    assert.equal(context.isWeeklyFreeRotation({ available: true, free_rotation: true, owned: false }), true);
    assert.equal((heroesSource.match(/isWeeklyFreeRotation\(activeLeader\)/g) || []).length, 2);
    assert.match(profileCss, /\.sow-heroes__rotation/);
});

test("header exposes server progress currencies and keeps the real XP remainder", () => {
    assert.match(shellSource, /data-progression-gems-value/);
    assert.match(shellSource, /data-progression-laurels-value/);
    assert.doesNotMatch(pokiSource, /function renderTopbar|data-progression-gems-value/);
    assert.match(shellSource, /accountXp % 100/);
    assert.match(shellSource, /state\.gems/);
    assert.match(webMenu, /"gems": progress\.gems/);
});

test("tutorial Continue sends the Rust pause command and owns the dialog event", () => {
    assert.match(tutorial, /setPaused\(false\)/);
    assert.match(tutorial, /send\("set_tutorial_paused", \{ paused: paused \}\)/);
    assert.match(tutorial, /overlay\.addEventListener\("click",[\s\S]*?\}, true\)/);
});

test("map menu sends the Rust-validated session, tile, and action", () => {
    assert.match(hud, /button\.dataset\.mapAction = action/);
    assert.match(hud, /send\("map_menu_action", \{/);
    assert.match(hud, /session: Number\(mapMenu\.dataset\.session\)/);
    assert.match(hud, /tile_idx: Number\(mapMenu\.dataset\.tileIdx\)/);
    assert.match(hud, /action: mapButton\.dataset\.mapAction/);
    assert.match(hud, /menu\.dataset\.renderKey/);
    assert.match(hud, /Math\.min\(Math\.max\(x, minX\), maxX\)/);
    assert.match(hudCss, /\.sow-hud__map-menu\.hidden\s*\{\s*display: none/);
});

test("WASM right-click stays on the Rust pointer-release path; touch hold stays intact", () => {
    assert.match(windowInput, /let right = matches!\([\s\S]*?MouseButton::Right[\s\S]*?\);/);
    const rightClickStart = windowInput.indexOf("} else if right && !pressed");
    const rightClickEnd = windowInput.indexOf("\n        }\n    }\n\n    fn handle_pointer_move", rightClickStart);
    assert.notEqual(rightClickStart, -1);
    assert.notEqual(rightClickEnd, -1);
    const rightClickBody = windowInput.slice(rightClickStart, rightClickEnd);
    assert.match(rightClickBody, /self\.ui\.app\.phase == ClientPhase::Playing/);
    assert.match(rightClickBody, /selected_building_kind\.is_some\(\)[\s\S]*selected_nuke_kind\.is_some\(\)[\s\S]*self\.clear_placement\(\)/);
    assert.match(rightClickBody, /else if !self\.move_selected_warships\(x, y\) \{\s*self\.open_map_context_menu\(x, y\);/);
    assert.match(rightClickBody, /else \{\s*self\.close_map_context_menu\(\);/);
    assert.match(windowInput, /pub\(crate\) fn poll_pointer_hold[\s\S]*?self\.open_map_context_menu\(x, y\);/);
    assert.match(indexTemplate, /<canvas id="blade" oncontextmenu="return false;"/);
    assert.doesNotMatch(hud, /addEventListener\("contextmenu"/);
    assert.doesNotMatch(webMenu, /OpenMapContextMenu/);
    assert.doesNotMatch(mapClick, /handle_secondary_click/);
});

test("map menu stays visible with disabled sectors when no action is available", () => {
    const openStart = mapClick.indexOf("pub(crate) fn open_map_context_menu");
    const openEnd = mapClick.indexOf("pub(crate) fn close_map_context_menu", openStart);
    const openBody = mapClick.slice(openStart, openEnd);
    assert.match(openBody, /if self\.map_menu_actions\(tile_idx\)\.is_empty\(\) \{\s*self\.show_map_menu_unavailable\(tile_idx, \(x, y\)\);\s*\}/);
    assert.match(openBody, /self\.input\.map_context_menu = Some\(MapContextMenu/);

    const renderStart = hud.indexOf("function renderMapMenu(mapMenu)");
    const renderEnd = hud.indexOf("function updateLeaderboard", renderStart);
    const renderBody = hud.slice(renderStart, renderEnd);
    assert.doesNotMatch(renderBody, /if \(!items\.length\)/);
    assert.match(renderBody, /mapDisabledSector\(radial, 1, radialCount, "fleet"/);
});

test("map menu keeps the radial sectors and recovered submenu actions", () => {
    assert.match(hud, /sow-hud__map-radial/);
    assert.match(hud, /function radialSectorGeometry/);
    assert.match(hud, /function mapDisabledSector/);
    assert.match(hud, /var radialCount = 4/);
    assert.match(hud, /mapSector\(radial, transfer, 0, radialCount/);
    assert.match(hud, /mapSector\(radial, fleet, 1, radialCount/);
    assert.match(hud, /mapSector\(radial, alliance, 2, radialCount/);
    assert.match(hud, /dataset\.mapGroup = group/);
    assert.match(hud, /upgrade_arsenal/);
    assert.match(hud, /build_warship/);
    assert.match(hud, /build_trade_ship/);
    assert.match(hudCss, /\.sow-hud__map-sector--build/);
    assert.match(hudCss, /\.sow-hud__map-sector--nuke/);
    assert.match(hudCss, /isolation: isolate/);
    assert.match(hudCss, /\.sow-hud__map-submenu/);
    assert.match(hudCss, /@keyframes sow-map-menu-in/);
    assert.match(hud, /title\.textContent = label\.icon/);
    assert.match(hud, /parts\.push\(item\.level/);
    assert.match(hud, /sow-hud__buildings-strip/);
    assert.match(hudCss, /\.sow-hud__buildings-strip/);
    assert.match(hudCss, /\.sow-hud__building-btn/);
    assert.doesNotMatch(hud, /build_structure|cancel_placement/);
    assert.doesNotMatch(hudCss, /sow-hud__bld-btn|sow-hud__cancel-btn/);
    assert.doesNotMatch(hud, /mapClose|data-map-close|close_map_menu|STRATEGIC STRIKE|CONSTRUCT/);
    assert.doesNotMatch(hudCss, /sow-hud__map-sector-caption|sow-hud__map-close/);
});

test("building dock exposes four emoji selectors without a cancel button", () => {
    const kinds = [...hud.matchAll(/data-command="select_building" data-kind="(City|Factory|Port|Bunker)"/g)]
        .map((match) => match[1]);
    assert.deepEqual(kinds, ["City", "Factory", "Port", "Bunker"]);
    assert.match(hud, /send\("select_building", \{ kind: btn\.dataset\.kind \}\)/);
    assert.match(webMenu, /SelectBuilding\s*\{\s*kind: sow_core::game::BuildingKind/);
    assert.match(webMenu, /WebMenuCommand::SelectBuilding\s*\{\s*kind\s*\}\s*=>\s*\{\s*self\.select_building_kind\(kind\)/);
    assert.match(windowInput, /fn rebase_single_touch_drag/);
    assert.match(windowInput, /\} else if is_touch \{[\s\S]*?camera_x \+=/);
    assert.doesNotMatch(windowInput, /self\.input\.dragging && !is_touch/);
    assert.match(mapClick, /MapMenuAction::BuildCity[\s\S]*?self\.select_building_kind\(kind\)/);
    assert.doesNotMatch(hud, /cancel_placement/);
});

test("mobile hover is Rust-owned and the HUD only presents its state", () => {
    assert.match(hud, /var hov = hud\.hovered/);
    assert.match(hud, /hoverCard\.classList\.remove\("hidden"\)/);
    assert.match(hud, /hoverCard\.classList\.add\("hidden"\)/);
    assert.doesNotMatch(hud, /addEventListener\("(?:touch|pointer)(?:start|move|up|cancel)"/);
});

test("tutorial pointer stays in the Rust/Blade render pass", () => {
    assert.match(worldOverlays, /fn render_tutorial_pointer/);
    assert.match(worldOverlays, /text\.push_ring/);
    assert.match(worldOverlays, /text\.push_triangle/);
    assert.doesNotMatch(tutorial, /tutorial\.markers|data-tutorial-marker|SOW_tutorial_marker_update/);
    assert.doesNotMatch(hudCss, /sow-hud__tutorial-marker/);
});
