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
const campaignEngine = fs.readFileSync(path.join(shell, "sow-campaign.js"), "utf8");
const campaignView = fs.readFileSync(path.join(shell, "sow-campaign-view.js"), "utf8");
const storyCss = fs.readFileSync(path.join(shell, "main_menu.tutorial.css"), "utf8");
const campaignEditor = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/editor.js"), "utf8");
const localeRuntime = fs.readFileSync(path.join(shell, "sow-i18n.js"), "utf8");
const campaignEditorServer = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/server.mjs"), "utf8");
const campaignEditorHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/logic.html"), "utf8");
const campaignEditorCss = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/editor.css"), "utf8");
const gameFonts = fs.readFileSync(path.join(shell, "../../sow-web/site/fonts/fonts.css"), "utf8");
const campaignMapEditorHtml = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/index.html"), "utf8");
const campaignMapPreview = fs.readFileSync(path.join(shell, "../../sow-tools/editors/campaign-editor/map-preview.js"), "utf8");
const hud = fs.readFileSync(path.join(shell, "main_menu.hud.js"), "utf8");
const hudCss = fs.readFileSync(path.join(shell, "main_menu.hud.css"), "utf8");
const indexTemplate = fs.readFileSync(path.join(shell, "index.html.template"), "utf8");
const windowInput = fs.readFileSync(path.join(shell, "../../sow-client/src/input/window.rs"), "utf8");
const hudStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/ui/hud/state.rs"), "utf8");
const matchStartSource = fs.readFileSync(path.join(shell, "../../sow-client/src/loader/match_start.rs"), "utf8");
const campaignModule = fs.readFileSync(path.join(shell, "../../sow-client/src/campaign/mod.rs"), "utf8");
const sessionSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/session.rs"), "utf8");
const simUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/sim/update.rs"), "utf8");
const actionsSource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/interact/actions.rs"), "utf8");
const netUpdateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/net/update/mod.rs"), "utf8");
const webMenu = fs.readFileSync(path.join(shell, "../../sow-client/src/web_menu.rs"), "utf8");
const appStateSource = fs.readFileSync(path.join(shell, "../../sow-client/src/app/state.rs"), "utf8");
const buildingOverlaySource = fs.readFileSync(path.join(shell, "../../sow-client/src/render/world/overlays.rs"), "utf8");
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
    assert.doesNotMatch(menuCss["main_menu.base.css"], /sow-purchase-art-reveal/);
    assert.match(menuCss["main_menu.base.css"], /sow-purchase-impact-ring/);
    assert.match(menuCss["main_menu.base.css"], /sow-purchase-modal-particle/);
    const leaderStart = coreSource.indexOf("function leaderById(id)");
    const leaderEnd = coreSource.indexOf("function leaderPerk", leaderStart);
    const context = { state: { leaders: [{ id: "SunTzu", slug: "sun_tzu" }] } };
    vm.runInNewContext(coreSource.slice(leaderStart, leaderEnd) + "this.leaderById = leaderById;", context);
    assert.equal(context.leaderById("sun_tzu").slug, "sun_tzu");
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

test("direct purchases wait for delivery before celebrating", () => {
    assert.match(storeSource, /EXTERNAL_PURCHASE_POLL_MS = 3000/);
    assert.match(storeSource, /EXTERNAL_PURCHASE_WAIT_MS = 120000/);
    assert.match(storeSource, /sow_pending_store_purchase_v1/);
    assert.match(storeSource, /profileApi\("\/store\/purchases"\)/);
    assert.match(storeSource, /record\.status === "granted"/);
    assert.match(storeSource, /onComplete: function \(\) \{\s*completeExternalPurchase\(productId\);/);
    assert.match(storeSource, /purchaseModal\.type === "product"[\s\S]*attempt\.delivery_id[\s\S]*profileHasExternalPurchase/);
    assert.match(storeSource, /purchaseModal\.type === "bundle"[\s\S]*beginBundlePurchasePresentation/);
    assert.match(storeSource, /purchaseModal\.phase === "spending" \|\| purchaseModal\.phase === "success"/);
    assert.match(shellSource, /resumeExternalPurchaseDelivery\(true\)/);
    assert.doesNotMatch(shellSource, /URLSearchParams\(window\.location\.search\)\.get\("purchase"\)/);
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

test("campaign runtime shares the JSON interpreter and cinematic view", () => {
    assert.match(tutorial, /SOWCampaign\.validate/);
    assert.match(tutorial, /SOWCampaign\.create/);
    assert.match(tutorial, /SOWCampaignView\.mount/);
    assert.match(tutorial, /start_campaign_episode/);
    assert.match(tutorial, /set_tutorial_paused/);
    assert.match(tutorial, /complete_campaign_episode/);
    assert.match(webMenu, /WebMenuCommand::SetTutorialPaused \{ paused \} => \{[\s\S]*?self\.sim\.paused = paused/);
    assert.match(simUpdateSource, /if self\.sim\.paused \{\s*self\.sim\.offline_tick_timer = 0\.0;/);
    assert.match(campaignEngine, /paused: \["scene", "choice", "end"\]/);
    assert.match(campaignView, /data-story-choice/);
    assert.match(storyCss, /\.sow-story__choices/);
    assert.doesNotMatch(hudCss, /\.sow-hud__tutorial-overlay/);
});

test("campaign episode cards use leader portraits and emphasize the next playable chapter", () => {
    const campaignCss = menuCss["main_menu.campaign.css"];
    assert.doesNotMatch(lobbiesSource, /campaign_tagline/);
    assert.match(lobbiesSource, /var mapName = episode\.id === "boudica" \? "eastanglia" : "northamerica"/);
    assert.match(lobbiesSource, /var mapArt = lobbyThumb\(\{ map_name: mapName \}\)/);
    assert.match(lobbiesSource, /asset\("gameplay\/avatars\/" \+ \(episode\.id === "boudica" \? "boudica" : "lady_six_sky"\) \+ "\.webp"\)/);
    assert.match(lobbiesSource, /sow-campaign__map-art[\s\S]*?sow-campaign__leader-art/);
    assert.match(lobbiesSource, /isNext \? " is-next"/);
    assert.match(campaignCss, /\.sow-campaign__episode-art img[\s\S]*?object-fit: cover/);
    assert.match(campaignCss, /\.sow-campaign__map-art[\s\S]*?brightness\(\.58\)/);
    assert.match(campaignCss, /\.sow-campaign__leader-art[\s\S]*?border-radius: 50%/);
    assert.match(campaignCss, /\.sow-campaign__episode\.is-next/);
    assert.match(campaignCss, /\.sow-campaign__episode\.is-locked \.sow-campaign__episode-art img[\s\S]*?grayscale/);
});

test("campaign card map and leader art match every Rust episode and its JSON assets", () => {
    const start = campaignModule.indexOf("pub fn episode_id(");
    const end = campaignModule.indexOf("pub fn from_episode_id(", start);
    assert.ok(start >= 0 && end > start);
    const episodeIds = [...campaignModule.slice(start, end).matchAll(/=> "([^"]+)"/g)].map(match => match[1]);
    assert.deepEqual(episodeIds, ["boudica", "six_sky_ep1", "six_sky_ep2", "six_sky_ep3"]);
    const expected = { boudica: ["eastanglia", "boudica"], six_sky_ep1: ["northamerica", "lady_six_sky"], six_sky_ep2: ["northamerica", "lady_six_sky"], six_sky_ep3: ["northamerica", "lady_six_sky"] };
    const campaignDir = path.join(shell, "../../assets/campaign");
    for (const episodeId of episodeIds) {
        const roster = JSON.parse(fs.readFileSync(path.join(campaignDir, episodeId + ".json"), "utf8"));
        const definition = JSON.parse(fs.readFileSync(path.join(campaignDir, episodeId + ".triggers.json"), "utf8"));
        assert.equal(definition.episode_id, episodeId);
        assert.equal(roster.map, expected[episodeId][0], episodeId + " card map");
        assert.ok(Object.values(definition.speakers || {}).some(speaker => speaker.avatar === expected[episodeId][1]), episodeId + " card leader portrait");
    }
});

test("Boudica opens with a choice, then guides allied support, rebuilding and three Roman outposts", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const roster = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.json"), "utf8"));
    const definition = JSON.parse(fs.readFileSync(path.join(shell, "../../assets/campaign/boudica.triggers.json"), "utf8"));
    assert.deepEqual(campaign.validate(definition, roster, { hasText: () => true, hasAvatar: () => true }).errors, []);
    assert.equal(definition.settings.starting_troops, 2000);
    const target = roster.factions.find(faction => faction.name === "The Iceni Despoilers");
    assert.deepEqual({ role: target.role, civ: target.civ, x: target.x, y: target.y }, { role: "vassal", civ: "Roman Empire", x: 706, y: 64 });
    for (const name of ["Colonia Veterans", "Tax Collectors"]) {
        const faction = roster.factions.find(item => item.name === name);
        assert.equal(faction.role, "vassal");
        assert.equal(faction.civ, "Roman Empire");
    }
    assert.deepEqual(roster.factions.filter(item => ["Colonia Veterans", "Tax Collectors"].includes(item.name)).map(({ name, x, y }) => ({ name, x, y })), [
        { name: "Colonia Veterans", x: 720, y: 220 },
        { name: "Tax Collectors", x: 670, y: 240 }
    ]);
    assert.equal(definition.settings.buildings_enabled, false);
    assert.equal(definition.settings.buildings_unlock_after_defeated, target.name);
    assert.deepEqual(definition.settings.campaign_support, { after_defeated: target.name, share_percent: 50 });
    assert.match(campaignMapEditorHtml, /\.\.\.\(f\.civ\?\{civ:f\.civ\}:\{\}\)/);
    const attackStep = definition.steps.find(step => step.id === "boudica_first_victory");
    assert.equal(attackStep.trigger.target, target.name);
    assert.equal(attackStep.marker.target, target.name);
    assert.equal(attackStep.guide.target, "target_action");
    assert.equal(attackStep.attack_ratio_on_enter, 1);

    function attackAfter(choice) {
        const machine = campaign.create(definition);
        machine.update({}, {}, 0);
        machine.advance(null, "boudica_rise_of_the_iceni_intro");
        machine.advance(choice, "boudica_council_decision");
        if (choice === "wait") {
            machine.advance(null, "boudica_council_refusal");
            assert.equal(machine.view().step.id, "boudica_council_decision");
            machine.advance("attack", "boudica_council_decision");
        }
        assert.equal(machine.view().step.id, "boudica_first_victory");
        return machine;
    }
    const direct = attackAfter("attack");
    const reluctant = attackAfter("wait");
    assert.equal(reluctant.view().step.id, attackStep.id);
    const victoryFacts = { defeated_names: [target.name] };
    direct.update(victoryFacts, {}, 0);
    const victory = direct.update(victoryFacts, {}, 800);
    assert.equal(victory.step.id, "boudica_first_victory_scene");
    assert.equal(victory.step.attack_ratio_on_enter, 0.5);
    direct.advance(null, victory.step.id);
    assert.equal(direct.view().step.id, "boudica_first_expansion");
    direct.update({ ...victoryFacts, tiles_gained: 256 }, {}, 1000);
    const expansion = direct.update({ ...victoryFacts, tiles_gained: 256 }, {}, 1800);
    assert.equal(expansion.step.id, "boudica_first_contact_intro");
    direct.advance(null, direct.view().step.id);
    assert.equal(direct.view().step.id, "boudica_first_contact");
    assert.deepEqual(direct.view().step.trigger.targets, ["Snettisham", "Iceni Coast", "Stonea", "Venta Icenorum", "Thetford"]);
    const firstContactFacts = { ...victoryFacts, tiles_gained: 256, contact_names: ["Iceni Coast"] };
    direct.update(firstContactFacts, {}, 2000);
    assert.equal(direct.update(firstContactFacts, {}, 2800).step.id, "boudica_first_contact_response");
    direct.advance(null, direct.view().step.id);
    assert.equal(direct.view().step.id, "boudica_contact_snettisham");
    direct.update(firstContactFacts, {}, 3000);
    assert.equal(direct.update(firstContactFacts, {}, 3800).step.id, "boudica_contact_snettisham");
    const firstContactAndSnettisham = { ...firstContactFacts, contact_names: ["Iceni Coast", "Snettisham"] };
    direct.update(firstContactAndSnettisham, {}, 4000);
    assert.equal(direct.update(firstContactAndSnettisham, {}, 4800).step.id, "boudica_snettisham_pledge");
    const support = definition.steps.find(step => step.id === "boudica_ally_support_wait");
    assert.deepEqual(support.trigger, { type: "support", value: 1, scope: "total" });
    const iceniPledges = [
        ["Snettisham", "snettisham", "boudica_snettisham_pledge"],
        ["Iceni Coast", "iceni_coast", "boudica_iceni_coast_pledge"],
        ["Stonea", "stonea", "boudica_stonea_pledge"],
        ["Venta Icenorum", "venta_icenorum", "boudica_venta_pledge"],
        ["Thetford", "thetford", "boudica_thetford_pledge"]
    ];
    for (const [name, speaker, stepId] of iceniPledges) {
        const faction = roster.factions.find(item => item.name === name);
        assert.equal(faction.role, "kin");
        assert.ok(faction.support_interval_seconds >= 5);
        assert.equal(definition.steps.find(step => step.id === stepId).speaker, speaker);
    }
    const pactFactions = roster.factions.filter(item => item.alliance_group === "trinovantes");
    assert.deepEqual(pactFactions.map(item => item.name), ["Trinovantes", "Trinovantian Farms"]);
    assert.ok(pactFactions.every(item => item.support_interval_seconds >= 5));
    const unsupportedPayout = JSON.parse(JSON.stringify(roster));
    unsupportedPayout.factions.find(item => item.name === "Colonia Veterans").support_interval_seconds = 20;
    assert.ok(campaign.validate(definition, unsupportedPayout, { hasText: () => true, hasAvatar: () => true }).errors.some(issue => issue.field === "roster.factions.support_interval_seconds"));
    const trinovantesStep = definition.steps.find(step => step.id === "boudica_trinovantes_alliance");
    assert.equal(trinovantesStep.trigger.target, "Trinovantes");
    const pactCheck = campaign.create({
        entry: "alliance_check",
        steps: [{ id: "alliance_check", type: "objective", title_key: "tutorial.test", trigger: trinovantesStep.trigger }]
    });
    pactCheck.update({ alliance_names: ["Other tribe"], alliances_formed: 1 }, {}, 0);
    assert.equal(pactCheck.view().progress.current, 0);
    pactCheck.update({ alliance_names: ["Trinovantes"], alliances_formed: 2 }, {}, 1000);
    assert.equal(pactCheck.view().progress.current, 1);
    assert.equal(definition.steps.find(step => step.id === "boudica_choose_city").guide.target, "dock_city");
    assert.deepEqual(definition.steps.find(step => step.id === "boudica_build_city").trigger, { type: "city", value: 1, scope: "step" });
    assert.deepEqual(definition.steps.find(step => step.id === "boudica_first_expansion").trigger, { type: "territory", value: 256, scope: "step" });
    assert.match(tutorial, /step\.trigger\.type === "territory" && step\.guide\.target === "expand" && view && view\.progress\.current > 0\) return null/);
    assert.deepEqual(definition.steps.find(step => step.id === "boudica_roman_outposts").trigger, { type: "defeated", targets: ["Colonia Veterans", "Tax Collectors", "Roman Supply Depot"], value: 3, scope: "total" });
    assert.match(webMenu, /"notifications": notifications/);
    assert.match(simUpdateSource, /push_notification_for_players\(/);
    assert.match(simUpdateSource, /\[Some\(transfer\.sender_id\), Some\(my_id\)\]/);
    assert.match(webMenu, /"avatars": avatars/);
    assert.match(hud, /notificationCards = Array\.from\(\{ length: 3 \}/);
    assert.match(hud, /activeNotifications\.length < 3/);
    assert.match(hud, /function renderNotifications\(entries, forceRefresh\)/);
    assert.match(hud, /if \(!changed\) return;/);
    assert.match(hud, /renderNotifications\(\[\], true\)/);
    assert.match(hud, /renderHud\(true\)/);
    assert.match(hud, /gameplay\/avatars\/null\.webp/);
    assert.match(hud, /portrait\.hidden = !rawId/);
    assert.match(hudCss, /\.sow-hud__notification--support/);
    assert.match(campaignModule, /support_interval_seconds/);
    assert.match(campaignMapEditorHtml, /support_interval_seconds/);
    const invalidRatio = JSON.parse(JSON.stringify(definition));
    invalidRatio.steps.find(step => step.id === attackStep.id).attack_ratio_on_enter = 1.01;
    assert.ok(campaign.validate(invalidRatio, roster, { hasText: () => true, hasAvatar: () => true }).errors.some(issue => issue.field === "attack_ratio_on_enter"));
    assert.match(campaignEditor, /Set send percentage on entry/);
    assert.match(campaignEditor, /previewActionStep/);
    assert.match(campaignEditor, /if \(actionRatio != null\) \$\("#sow-hud-slider"\)\.value/);
    assert.match(tutorial, /machineView\.step\.id !== runtime\.lastActionStepId/);
});

test("HUD notifications coalesce, prioritize, reuse both portraits, and expire within fixed bounds", () => {
    const start = hud.indexOf("    function renderNotifications(entries, forceRefresh) {");
    const end = hud.indexOf("\n    function renderHud(", start);
    assert.ok(start >= 0 && end > start);
    const cards = Array.from({ length: 3 }, () => ({
        card: { hidden: true, className: "" },
        seal: { innerHTML: "" },
        portraits: Array.from({ length: 2 }, () => ({
            hidden: true,
            src: "",
            getAttribute(name) { return name === "src" ? this.src : null; }
        })),
        copy: { textContent: "" },
        renderKey: ""
    }));
    let now = 1000;
    const timers = [];
    const context = {
        hudRefs: { notifications: {} },
        notificationCursor: 0,
        notificationTimer: null,
        activeNotifications: [],
        notificationCards: cards,
        Date: { now: () => now },
        asset: path => "/assets/" + path,
        leaderById: id => ({ slug: String(id) }),
        hudIcon: name => name,
        SOW_t: key => key,
        window: {
            SOW_LOCALE: "en",
            clearTimeout(timer) { timer.cleared = true; },
            setTimeout(callback, delay) {
                const timer = { callback, delay, cleared: false };
                timers.push(timer);
                return timer;
            }
        }
    };
    const renderer = hud.slice(start, end) + "\nthis.renderNotifications = renderNotifications;";
    vm.runInNewContext(renderer, context);
    const add = (id, key, priority, group, values, avatars, sumValues = false) => {
        now += 100;
        context.renderNotifications([{
            id, key, priority, group, values: values || {}, avatars: avatars || [],
            sum_values: sumValues, age_ms: 0
        }]);
    };

    add(1, "hud.resource_received_gold", 3, "resource:received:7", { gold: "100" }, ["boudica", "caesar"], true);
    const firstTimerCount = timers.length;
    context.renderNotifications([], false);
    assert.equal(timers.length, firstTimerCount, "unchanged HUD updates must not reset the expiry timer");
    add(2, "hud.resource_received_troops", 3, "resource:received:7", { troops: "25" }, ["boudica", "caesar"], true);
    assert.equal(context.activeNotifications.length, 1);
    assert.equal(context.activeNotifications[0].entry.key, "hud.resource_received_both");
    assert.equal(context.activeNotifications[0].entry.values.gold, "100");
    assert.equal(context.activeNotifications[0].entry.values.troops, "25");
    assert.deepEqual(cards[0].portraits.map(item => item.hidden), [false, false]);
    assert.deepEqual(cards[0].portraits.map(item => item.src), [
        "/assets/gameplay/avatars/boudica.webp", "/assets/gameplay/avatars/caesar.webp"
    ]);

    add(3, "hud.nuke_struck", 4, "nuke:9:7", {}, ["caesar", "boudica"]);
    add(4, "hud.structure_ready", 2, "building:7", {});
    add(5, "hud.water_feedback", 1, "click:7", {});
    assert.equal(context.activeNotifications.length, 3, "a burst must never create a backlog");
    assert.ok(context.activeNotifications.some(item => item.group === "nuke:9:7"));
    assert.ok(context.activeNotifications.some(item => item.group === "resource:received:7"));
    assert.ok(!context.activeNotifications.some(item => item.group === "click:7"), "lower-priority feedback must yield to active event cards");
    const timersBeforeAttack = timers.length;
    add(6, "hud.attack_incoming", 4, "incoming-attack:7", { count: "2" }, ["caesar", "boudica"]);
    add(7, "hud.attack_incoming", 4, "incoming-attack:7", { count: "3" }, ["caesar", "boudica"]);
    assert.equal(context.activeNotifications.length, 3);
    assert.equal(context.activeNotifications.find(item => item.group === "incoming-attack:7").entry.values.count, "3");
    const timersBeforeDuplicate = timers.length;
    context.renderNotifications([{
        id: 7, key: "hud.attack_incoming", priority: 4, group: "incoming-attack:7",
        values: { count: "3" }, avatars: ["caesar", "boudica"], age_ms: 0
    }]);
    assert.equal(timers.length, timersBeforeDuplicate, "duplicate IDs must not refresh or duplicate cards");
    assert.ok(timersBeforeAttack < timersBeforeDuplicate);

    const burst = Array.from({ length: 100 }, (_, index) => ({
        id: index + 8, key: "hud.water_feedback", priority: 1, group: "click:" + index,
        values: {}, avatars: ["boudica"], age_ms: 0
    }));
    const timersBeforeBurst = timers.length;
    context.renderNotifications(burst);
    assert.equal(context.activeNotifications.length, 3);
    assert.equal(timers.length, timersBeforeBurst, "a low-priority spam burst must not churn timers");

    now += 5000;
    timers.at(-1).callback();
    assert.equal(context.activeNotifications.length, 0);
    assert.ok(cards.every(parts => parts.card.hidden), "expired cards release their visible slots");
});

test("campaign editor refresh serves current source files without browser caching", () => {
    assert.match(campaignEditorServer, /pathname === "\/tools\/campaign-editor\/"[\s\S]*?path\.join\(editorDir, "index\.html"\)/);
    assert.match(campaignEditorServer, /pathname\.startsWith\("\/tools\/campaign-editor\/"\)\) return safeFile\(editorDir/);
    assert.match(campaignEditorServer, /const body = await fs\.readFile\(file\)/);
    assert.match(campaignEditorServer, /"Cache-Control": "no-store"/);
});

test("campaign selector removes the multiplayer filler from every locale", () => {
    const strings = path.join(shell, "../../sow-i18n/strings");
    const files = fs.readdirSync(strings).map(locale => path.join(strings, locale, "web.toml")).filter(file => fs.existsSync(file));
    assert.ok(files.length > 0);
    for (const file of files) assert.doesNotMatch(fs.readFileSync(file, "utf8"), /^campaign_tagline\s*=/m, file);
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
    for (const script of ["arabic", "cjk", "cyrillic"]) assert.match(gameFonts, new RegExp("\\.sow-story\\[data-locale-script='" + script + "'\\]"));
});

test("campaign editor never treats a missing catalog entry as valid text", () => {
    assert.match(campaignEditorServer, /catch \{\s*return false;\s*\}/);
});

test("campaign interpreter facts are all published by the WASM HUD payload", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const facts = new Set([...Object.values(campaign.METRICS), "tiles", "defeated", "defeated_names", "contacts", "contact_names", "attacks_by_target"]);
    for (const fact of facts) assert.match(webMenu, new RegExp('"' + fact + '"\\s*:'), `WASM HUD is missing campaign fact ${fact}`);
    assert.match(tutorial, /runtime\.machine\.update\(tutorial\.facts \|\| \{\}/);
});

test("campaign dialogue starts each new beat at the top of its scroll panel", () => {
    assert.match(campaignView, /if \(renderKey !== key\) \{\s*renderKey = key;\s*conversation\.scrollTop = 0;/);
    assert.match(storyCss, /\.sow-story__conversation\s*\{[^}]*overflow-y: auto/);
});

test("campaign editor preview and game share RTL locale coverage", () => {
    const localeSet = /new Set\(\["ar", "arc", "ckb", "dv", "fa", "he", "iw", "nqo", "pnb", "ps", "sd", "syr", "ug", "ur", "yi"\]\)/;
    assert.match(localeRuntime, localeSet);
    assert.match(campaignEditor, localeSet);
    assert.match(campaignEditor, /direction: rtlLanguages\.has\(state\.previewLanguage\.toLowerCase\(\)\.split\("-"\)\[0\]\) \? "rtl" : "ltr"/);
});

test("an active campaign keeps its loaded definition when the editor draft changes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "snapshot_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
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
    assert.match(tutorial, /runtime\.modalOpen = false;\s*runtime\.resumeAfterModal = false;/);
});

test("campaign start keeps its loaded script and replay clicks belong to that episode", async () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "boudica", default_locale: "en",
        settings: { buildings_enabled: false, starting_troops: 1000 }, speakers: {},
        strings: { en: { "tutorial.open": "Original", "tutorial.end": "End" } }, entry: "opening",
        steps: [{ id: "opening", type: "scene", title_key: "tutorial.open", next: "ending" }, { id: "ending", type: "end", title_key: "tutorial.end" }]
    };
    const roster = { map: "eastanglia", player_spawn: [1, 1], factions: [{ name: "Enemy", role: "independent", x: 2, y: 2 }] };
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
    const window = {
        addEventListener() {}, SOW_menu_command() {},
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
    definition.strings.en["tutorial.open"] = "Edited while the match was starting";
    const state = { phase: "Playing", hud: { tutorial: { active: true, episode_id: "boudica", facts: {} }, players: [], settings: {} } };
    window.SOW_tutorial_state_update(state);
    await new Promise(setImmediate);
    assert.equal(fetches, 2);
    assert.equal(activeRun.definition.strings.en["tutorial.open"], "Original");
    listeners.click({ type: "click", target: buttons[1] });
    window.SOW_tutorial_state_update(state);
    assert.equal(lastUi.campaign_replay, undefined);
    listeners.click({ type: "click", target: buttons[0] });
    window.SOW_tutorial_state_update(state);
    assert.equal(lastUi.campaign_replay, 1);
});

test("campaign decisions keep their selected branch and guard stale callbacks", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "branch_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
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

test("campaign validator rejects conditional branches that can loop forever", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const report = campaign.validate({
        version: 2, episode_id: "route_loop_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
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
        version: 2, episode_id: "legacy_flag_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
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
        version: 2, episode_id: "route_order_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
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

test("campaign validator rejects ignored fields and renders objective speakers", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const report = campaign.validate({
        version: 2, episode_id: "unsupported_field_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "opening", actions: [{ type: "spawn" }], steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending", actions: [{ type: "grant_troops" }] },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(report.errors.some(issue => issue.field === "campaign" && issue.message.includes("actions")));
    assert.ok(report.errors.some(issue => issue.step === "opening" && issue.field === "fields" && issue.message.includes("actions")));
    const unusedSpeaker = campaign.validate({
        version: 2, episode_id: "step_field_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: { narrator: { name: "Narrator" } }, entry: "objective", steps: [
            { id: "objective", type: "objective", title_key: "tutorial.objective", speaker: "narrator", trigger: { type: "territory", scope: "step", value: 1 }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.deepEqual(unusedSpeaker.errors, []);
    assert.match(campaignView, /sow-story__objective-speaker/);
    assert.match(campaignView, /objectiveSpeakerName/);
});

test("campaign preview can seed prior decisions and facts to inspect conditional scenes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "preview_branch_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
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
        version: 2, episode_id: "live_edit_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
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
        version: 2, episode_id: "speaker_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.opening": "Opening", "tutorial.ending": "The end" } },
        speakers: { empty: { name: "  " } }, entry: "opening", steps: [
            { id: "opening", type: "scene", speaker: "empty", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.ok(report.errors.some(issue => issue.field === "speakers" && /empty/.test(issue.message)));
});

test("campaign editor rejects maps and spawns that the game would refuse", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "boudica", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.open": "Open", "tutorial.end": "End" } }, speakers: {}, entry: "opening",
        steps: [{ id: "opening", type: "scene", title_key: "tutorial.open", next: "ending" }, { id: "ending", type: "end", title_key: "tutorial.end" }]
    };
    const roster = { map: "eastanglia", player_spawn: [895, 503], factions: [{ name: "Enemy", role: "independent", x: 1, y: 1 }] };
    const errors = () => campaign.validate(definition, roster).errors;
    assert.deepEqual(errors(), []);
    roster.map = "northamerica";
    assert.ok(errors().some(issue => issue.field === "roster.map"));
    roster.map = "eastanglia"; roster.player_spawn[0] = 896;
    assert.ok(errors().some(issue => issue.field === "roster.player_spawn"));
    roster.player_spawn[0] = 895; roster.factions[0].y = 504;
    assert.ok(errors().some(issue => /Faction spawn is outside/.test(issue.message)));
    definition.episode_id = "six_sky_ep1"; roster.map = "northamerica";
    roster.player_spawn = [999, 515]; roster.factions[0].y = 515;
    assert.deepEqual(errors(), []);
});

test("campaign speaker picker is sourced from existing avatar assets and rejects missing files", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.match(campaignEditorServer, /url\.pathname === "\/__avatars"/);
    assert.match(campaignEditor, /fetch\("\/__avatars"/);
    assert.match(campaignEditor, /selectField\("Portrait"/);
    assert.match(campaignEditor, /hasAvatar: function \(avatar\) \{ return state\.avatars\.includes\(avatar\); \}/);
    const definition = {
        version: 2, episode_id: "avatar_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.opening": "Opening", "tutorial.ending": "End" } }, speakers: { leader: { name: "Leader", avatar: "not_real" } }, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    };
    assert.ok(campaign.validate(definition, null, { hasAvatar: avatar => avatar === "boudica" }).errors.some(issue => issue.field === "speakers"));
});

test("campaign studio edits speaker names per language when localized", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.match(campaignEditor, /var keyBase = "tutorial\.speaker_" \+ speakerId \+ "_name"/);
    assert.match(campaignEditor, /while \(Object\.keys\(state\.definition\.strings\)/);
    assert.match(campaignEditor, /Character name · " \+ state\.language/);
    assert.match(campaignEditor, /dictionary\[speaker\.name_key\] = value/);
    assert.match(campaignEditor, /Use one name for every language/);
    assert.match(campaignEditor, /var locale = this\.value; state\.language = locale; renderSettings\(\)/);
    assert.match(campaignEditor, /loadCatalog\(locale\)\.then\(function \(\) \{[\s\S]*?state\.language === locale/);
    assert.match(campaignView, /character\.name_key \? t\(character\.name_key\)/);
    const report = campaign.validate({
        version: 2, episode_id: "localized_speaker", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.speaker_leader_name": "Leader", "tutorial.opening": "Opening", "tutorial.ending": "End" }, es: { "tutorial.speaker_leader_name": "Líder" } },
        speakers: { leader: { name_key: "tutorial.speaker_leader_name" } }, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.deepEqual(report.errors, []);
});

test("campaign studio identifies episode text falling back to the default language", () => {
    assert.match(campaignEditor, /var hasLocaleText = Boolean\(dictionary\[key\] \|\| catalogValue\(state\.language, key\)\)/);
    assert.match(campaignEditor, /var hasDefaultText = Boolean\(textValue\(key, state\.definition\.default_locale\)\)/);
    assert.match(campaignEditor, /Showing episode default-language text\. Editing adds this language\./);
    assert.match(campaignEditor, /fallbackNote\.hidden = state\.language === state\.definition\.default_locale \|\| Boolean\(dictionary\[key\] \|\| catalogValue\(state\.language, key\)\) \|\| !hasDefaultText/);
});

test("campaign studio loads locale catalogs on demand and keeps episode text editable on failure", () => {
    assert.match(campaignEditor, /localeFailures: \[\], localeRegistryFailed: false/);
    assert.match(campaignEditor, /function loadCatalog\(locale\)/);
    assert.match(campaignEditor, /if \(catalogLoads\[code\]\) return catalogLoads\[code\]/);
    assert.match(campaignEditor, /fetch\("\/locales\/" \+ encodeURIComponent\(code\)/);
    assert.match(campaignEditor, /if \(!state\.localeFailures\.includes\(code\)\) state\.localeFailures\.push\(code\)/);
    assert.match(campaignEditor, /Promise\.all\(\[loadCatalog\(state\.language\), loadCatalog\(state\.previewLanguage\)\]\)/);
    assert.match(campaignEditor, /Catalog unavailable for .*episode text remains editable/);
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

test("campaign editor and runtime normalize regional locale codes", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.match(campaignEditor, /entry\.code\)\.toLowerCase\(\)/);
    assert.match(tutorial, /toLowerCase\(\)\.replace\(\/_\/g, "-"\)/);
    const report = campaign.validate({
        version: 2, episode_id: "regional_locale_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.opening": "Opening", "tutorial.ending": "End" }, "pt-br": {}, "zh-cn": {} },
        speakers: {}, entry: "opening", steps: [
            { id: "opening", type: "scene", title_key: "tutorial.opening", next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.deepEqual(report.errors, []);
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
    assert.match(campaignEditor, /Default story language"[^\n]*localeOptions\(\)/);
    assert.match(campaignEditor, /optionList\(\$\("#editLocale"\), localeOptions\(\)/);
    assert.match(campaignEditor, /optionList\(\$\("#previewLocale"\), localeOptions\(\)/);
});

test("campaign map coordinate fields retain focus and refresh placement feedback", () => {
    assert.match(campaignMapEditorHtml, /const updateFactionHint = \(\) =>/);
    assert.match(campaignMapEditorHtml, /getElementById\("fx"\)\.oninput = e => \{ f\.x=.*updateFactionHint\(\); renderList\(\); draw\(\); \}/);
    assert.match(campaignMapEditorHtml, /getElementById\("fy"\)\.oninput = e => \{ f\.y=.*updateFactionHint\(\); renderList\(\); draw\(\); \}/);
    assert.doesNotMatch(campaignMapEditorHtml, /getElementById\("f[xy]"\)\.oninput = e => \{[^\n]*renderSide\(\)/);
    assert.match(campaignMapEditorHtml, /const scrollTop = list\.scrollTop;[\s\S]*?list\.innerHTML = html;[\s\S]*?list\.scrollTop = scrollTop;/);
});

test("campaign map editor renders faction names as text in its HTML templates", () => {
    assert.match(campaignMapEditorHtml, /function escapeHtml\(value\)/);
    assert.match(campaignMapEditorHtml, /value="\$\{escapeHtml\(f\.name\)\}"/);
    assert.match(campaignMapEditorHtml, /class="nm">\$\{escapeHtml\(f\.name\)\}/);
    assert.match(campaignMapEditorHtml, /inWater\.map\(escapeHtml\)/);
});

test("troop objectives check current force against a minimum", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "troop_minimum_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "minimum", steps: [
            { id: "minimum", type: "objective", title_key: "tutorial.minimum", hint_key: "tutorial.minimum_hint", trigger: { type: "troops", value: 1500, scope: "step" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    assert.deepEqual(machine.update({ troops: 1400 }, {}, 0).progress, { current: 1400, target: 1500 });
    assert.equal(machine.update({ troops: 1500 }, {}, 100).ready, true);
});

test("campaign completion beat stays paused while an exit modal is open", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const machine = campaign.create({
        version: 2, episode_id: "modal_pause_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "objective", steps: [
            { id: "objective", type: "objective", title_key: "tutorial.objective", hint_key: "tutorial.objective_hint", trigger: { type: "territory", value: 1, scope: "step" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.ending" }
        ]
    });
    machine.update({ tiles_gained: 0 }, {}, 0);
    assert.equal(machine.update({ tiles_gained: 1 }, {}, 100).ready, true);
    machine.setPaused(true, 200);
    assert.equal(machine.advance(null, "objective"), false);
    assert.equal(machine.update({ tiles_gained: 1 }, {}, 9000).step.id, "objective");
    machine.setPaused(false, 10000);
    assert.equal(machine.update({ tiles_gained: 1 }, {}, 10699).step.id, "objective");
    assert.equal(machine.update({ tiles_gained: 1 }, {}, 10700).step.id, "ending");
});

test("UI objectives respect step, episode, and total scope", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const progress = scope => {
        const machine = campaign.create({
            version: 2, episode_id: "ui_scope_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 0 },
            strings: { en: {} }, speakers: {}, entry: "opening", steps: [
                { id: "opening", type: "scene", title_key: "tutorial.opening", next: "objective" },
                { id: "objective", type: "objective", title_key: "tutorial.objective", hint_key: "tutorial.objective_hint", trigger: { type: "ui", action: "attack_ratio", scope }, next: "ending" },
                { id: "ending", type: "end", title_key: "tutorial.ending" }
            ]
        });
        machine.update({}, { attack_ratio: 0 }, 0);
        machine.update({}, { attack_ratio: 1 }, 100);
        machine.advance(null, "opening");
        return machine.update({}, { attack_ratio: 1 }, 200).progress;
    };

    assert.deepEqual(progress("step"), { current: 0, target: 1 });
    assert.deepEqual(progress("episode"), { current: 1, target: 1 });
    assert.deepEqual(progress("total"), { current: 1, target: 1 });
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
    assert.match(campaignEditor, /label: textValue\(choice\.label_key, state\.language\) \|\| choice\.id/);
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

test("campaign dialogue regains keyboard focus after the leave modal is canceled", () => {
    assert.match(campaignView, /const wasHidden = root\.hidden/);
    assert.match(campaignView, /const focusModal = modal && \(!wasModal \|\| wasHidden\)/);
    assert.match(campaignView, /else if \(focusModal\) focusAction\(\)/);
});

test("campaign dialogue passes HUD controls through and outside-continues only non-choice scenes", () => {
    assert.match(storyCss, /\.sow-story\.is-modal \{ pointer-events: none; \}/);
    assert.match(storyCss, /\.sow-story__shade \{[^}]*pointer-events: none/s);
    assert.match(storyCss, /\.sow-story__dialog \{[^}]*pointer-events: auto/s);
    assert.match(campaignView, /doc\.addEventListener\("click", outsideClick, true\)/);
    assert.match(campaignView, /doc\.removeEventListener\("click", outsideClick, true\)/);
    assert.match(campaignView, /event\.target\.closest\("button, a\[href\], input:not\(\[type='hidden'\]\), select, textarea, \[role='button'\], \[data-command\], \[data-map-action\]"\)/);
    assert.match(campaignView, /if \(control\) return;[\s\S]*?model\.step\.type !== "choice"[\s\S]*?options\.onContinue\(\)/);
});

test("campaign dialogue keeps a compact speaker portrait on narrow screens", () => {
    assert.match(storyCss, /@container \(max-width: 380px\) \{[\s\S]*?grid-template-columns: 64px minmax\(0, 1fr\)/);
    assert.doesNotMatch(storyCss, /@container \(max-width: 380px\) \{[\s\S]*?\.sow-story__portrait \{ display: none/);
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

test("objective completion gets a short confirmation pulse", () => {
    assert.match(storyCss, /\.sow-story\.is-ready \.sow-story__objective \{[^}]*animation: story-objective-complete 800ms/);
    assert.match(storyCss, /@keyframes story-objective-complete/);
    assert.match(storyCss, /\.sow-story\.is-reduced [^{]*\{ animation: none !important/);
    assert.match(storyCss, /\.sow-story__meter progress \{[^}]*height: 10px[^}]*border-radius: 999px/);
});

test("campaign pacing can wait without a hand and guide steps require a target", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "timing_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: { "tutorial.wait": "Wait", "tutorial.end": "Done" } }, speakers: {}, entry: "wait", steps: [
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
        version: 2, episode_id: "guide_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "slider", steps: [
            { id: "slider", type: "objective", title_key: "tutorial.slider", trigger: { type: "ui", action: "attack_ratio", scope: "step" }, guide: { kind: "ui", target: "attack_ratio", gesture: "drag" }, next: "ending" },
            { id: "ending", type: "end", title_key: "tutorial.end" }
        ]
    };
    assert.deepEqual(campaign.validate(definition).errors, []);
    definition.steps[0].guide.to = "not_a_control";
    assert.ok(campaign.validate(definition).errors.some(issue => issue.step === "slider" && issue.field === "guide.to"));
    definition.steps[0].guide = { kind: "world", target: "expand", gesture: "drag", to: "assault" };
    assert.deepEqual(campaign.validate(definition).errors, []);
    const demo = campaignEditor.slice(campaignEditor.indexOf("function demo()"), campaignEditor.indexOf("function createMenuGuide()"));
    assert.match(demo, /trigger: \{ type: "ui", action: "attack_ratio", scope: "step" \}, guide: \{ kind: "ui", target: "attack_ratio", gesture: "drag" \}/);
    assert.match(campaignEditor, /Within this control/);
    assert.match(campaignEditor, /if \(guide\.kind === "world"\) end = previewWorldMarker\(frame, guide\.to, step\)/);
});

test("campaign story preview uses the selected episode map and faction positions", () => {
    assert.match(campaignEditorHtml, /id="campaignMapPreview"/);
    assert.match(campaignEditorHtml, /id="campaignPreviewMarkers"/);
    assert.match(campaignEditor, /SOWCampaignMapPreview\.load\(mapId\)/);
    assert.match(campaignEditor, /dataset: \{ factionName: faction\.name, role: faction\.role \}/);
    assert.match(campaignEditor, /function previewWorldMarker\(frame, target, step\)/);
    assert.match(campaignMapEditorHtml, /SOWCampaignMapPreview\.parse\(buf\)/);
    assert.match(campaignMapPreview, /!== "SOWM"/);
    assert.match(campaignMapPreview, /function terrainCanvas\(terrain, width, height\)/);
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

test("menu guide uses the shared flow engine from its own entry", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const definition = {
        version: 2, episode_id: "menu_guide_test", default_locale: "en", settings: { buildings_enabled: false, starting_troops: 1000 },
        strings: { en: {} }, speakers: {}, entry: "game_open", menu_guide: { entry: "menu_open", dismissible: true },
        steps: [
            { id: "game_open", type: "scene", title_key: "tutorial.game", next: "game_end" },
            { id: "game_end", type: "end", title_key: "tutorial.game_end" },
            { id: "menu_open", type: "scene", title_key: "tutorial.menu", next: "menu_campaign" },
            { id: "menu_campaign", type: "guide", title_key: "tutorial.campaign", trigger: { type: "ui", action: "menu_campaign", scope: "step" }, guide: { kind: "ui", target: "menu_campaign", gesture: "tap" }, next: "menu_end" },
            { id: "menu_end", type: "end", title_key: "tutorial.menu_end" }
        ]
    };
    const report = campaign.validate(definition);
    assert.deepEqual(report.errors, []);
    assert.deepEqual(report.warnings, []);
    const machine = campaign.create(definition, definition.menu_guide.entry);
    assert.equal(machine.view().step.id, "menu_open");
    machine.update({}, {}, 0);
    machine.advance(null, "menu_open");
    machine.update({}, { menu_campaign: 1 }, 1);
    machine.update({}, { menu_campaign: 1 }, 801);
    assert.equal(machine.view().step.id, "menu_end");
});

test("Boudica completion offers localized Campaign and tutorial replay guidance", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    const episode = path.join(shell, "../../assets/campaign");
    const definition = JSON.parse(fs.readFileSync(path.join(episode, "boudica.triggers.json"), "utf8"));
    const roster = JSON.parse(fs.readFileSync(path.join(episode, "boudica.json"), "utf8"));
    const report = campaign.validate(definition, roster, { hasText: () => true, hasAvatar: avatar => avatar === "boudica" });
    assert.deepEqual(report.errors, []);
    assert.deepEqual(report.warnings, []);
    assert.equal(definition.menu_guide.dismissible, true);
    assert.equal(definition.steps.find(step => step.id === definition.menu_guide.entry).trigger.action, "menu_campaign");
    assert.equal(definition.steps.find(step => step.id === "boudica_return_replay").trigger.action, "campaign_replay");
    assert.equal(definition.steps.find(step => step.id === "boudica_return_multiplayer").trigger.action, "menu_multiplayer");
    assert.equal(definition.steps.find(step => step.id === "boudica_return_end").speaker, "boudica");
    assert.equal(definition.steps.find(step => step.id === "boudica_end").body_key, "tutorial.boudica_end_body");
    assert.equal(definition.steps.find(step => step.id === "boudica_complete").type, "end");
    for (const type of ["farm", "factory", "port", "bunker", "structure_upgrade", "city_upgrade", "city_level", "port_upgrade", "port_level", "tile_upgrade", "resource_transfer", "alliance", "fleet"]) {
        assert.ok(campaign.METRICS[type], `missing campaign metric ${type}`);
        assert.match(campaignEditor, new RegExp(`value: "${type}"`), `Campaign Studio cannot select ${type}`);
    }
    const step = id => definition.steps.find(candidate => candidate.id === id);
    assert.equal(step("boudica_first_victory").attack_ratio_on_enter, 1);
    assert.equal(step("boudica_ratio_guide").attack_ratio_on_enter, 0.5);
    assert.equal(step("boudica_first_victory_scene").next, "boudica_first_expansion");
    assert.equal(step("boudica_first_expansion").next, "boudica_first_contact_intro");
    assert.deepEqual(step("boudica_first_contact").trigger.targets, ["Snettisham", "Iceni Coast", "Stonea", "Venta Icenorum", "Thetford"]);
    assert.equal(step("boudica_first_contact").next, "boudica_first_contact_response");
    assert.equal(step("boudica_contact_snettisham").trigger.target, "Snettisham");
    assert.equal(step("boudica_first_contact").guide.target, "target_action");
    assert.match(campaignEditor, /Contact any selected faction/);
    assert.match(campaignEditor, /\+1 faction/);
    assert.deepEqual(step("boudica_roman_outposts").trigger.targets, ["Colonia Veterans", "Tax Collectors", "Roman Supply Depot"]);
    assert.equal(step("boudica_roman_outposts").next, "boudica_posts_fall");
    assert.equal(step("boudica_structure_upgrade").trigger.type, "city_level");
    assert.equal(step("boudica_structure_upgrade").trigger.value, 3);
    assert.equal(step("boudica_pact_offer").trigger.action, "map_alliance");
    assert.equal(step("boudica_pact_formed").trigger.type, "alliance");
    assert.ok(step("boudica_factory_choice").choices.some(choice => choice.id === "foundry"));
    assert.deepEqual(step("boudica_foundry_select_menu").trigger, { type: "ui", action: "map_build", scope: "step" });
    assert.deepEqual(step("boudica_foundry_upgrade").trigger, { type: "city_upgrade", value: 1, scope: "step" });
    assert.equal(step("boudica_foundry_upgrade").guide.target, "map_upgrade_foundry");
    assert.match(mapClick, /self\.sim\.config\.tutorial && building\.kind == sow_core::game::BuildingKind::City[\s\S]*?actions\.push\(MapMenuAction::UpgradeFoundry\)/);
    assert.match(mapClick, /if self\.sim\.config\.tutorial \{\s*actions\.push\(MapMenuAction::UpgradeTile\)/);
    assert.equal(step("boudica_foundry_ready").next, "boudica_bunker_choice");
    assert.equal(step("boudica_port_upgrade").trigger.type, "port_upgrade");
    assert.equal(step("boudica_port_ready").trigger.type, "port_level");
    assert.equal(step("boudica_fleet_target").trigger.type, "fleet");
    assert.equal(step("boudica_fleet_target").guide.target, "player");
    assert.equal(step("boudica_fleet_target").next, "boudica_fleet_ready");
    assert.equal(step("boudica_fleet_ready").next, "boudica_ship_choice");
    assert.equal(step("boudica_ship_choice").choices.find(choice => choice.id === "march").next, "boudica_final_battle_intro");
    assert.deepEqual(step("boudica_trade_city").trigger, { type: "city_level", value: 4, scope: "total" });
    assert.deepEqual(step("boudica_trade_port").trigger, { type: "port_level", value: 3, scope: "total" });
    assert.equal(step("boudica_trade_menu").trigger.action, "map_build");
    assert.deepEqual(step("boudica_trade_ship").trigger, { type: "fleet", value: 1, scope: "step" });
    assert.equal(step("boudica_trade_ship").guide.target, "map_build_trade_ship");
    assert.equal(step("boudica_trade_ready").next, "boudica_final_battle_intro");
    assert.match(mapClick, /BuildingKind::Port if building\.level >= 3\s*=>\s*\{\s*actions\.push\(MapMenuAction::BuildTradeShip\);/);
    assert.match(campaignEngine, /map_build_trade_ship: '#sow-hud \[data-map-action="build_trade_ship"\]'/);
    assert.equal(step("boudica_camulodunum").next, "boudica_ninth_legion_intro");
    assert.equal(step("boudica_ninth_legion").next, "boudica_londinium_intro");
    assert.equal(step("boudica_londinium").next, "boudica_verulamium_intro");
    assert.equal(step("boudica_verulamium").next, "boudica_pact_choice");
    assert.equal(roster.factions.find(faction => faction.name === "Catuvellauni").iq, 60);
    assert.match(campaignEditor, /world:player/);
    assert.match(campaignEngine, /upgrade_structure: "#sow-hud-building-card-upgrade"/);
    assert.match(campaignEditorHtml, /id="sow-hud-building-card-upgrade"/);
    assert.match(tutorial, /if \(runtime\.definition\.menu_guide\) pendingMenuGuide = \{ episodeId: runtime\.episodeId \}/);
    const rewardGate = shellSource.indexOf("!rewardPresentationReady || rewardAnimationRunning) return;");
    const guideReadyEvent = shellSource.indexOf('dispatchEvent(new CustomEvent("sow:campaign-menu-guide-ready"))');
    assert.ok(rewardGate >= 0 && guideReadyEvent > rewardGate, "return guide must wait for reward presentation");
    for (const locale of ["en", "es"]) {
        assert.ok(definition.strings[locale]["tutorial.boudica_menu_campaign_hint"]);
        assert.ok(definition.strings[locale]["tutorial.boudica_menu_replay_hint"]);
        assert.ok(definition.strings[locale]["tutorial.boudica_menu_multiplayer_hint"]);
        assert.ok(definition.strings[locale]["tutorial.boudica_port_ready_hint"]);
        assert.ok(definition.strings[locale]["tutorial.boudica_trade_ship_hint"]);
    }
    const machine = campaign.create(definition, definition.menu_guide.entry);
    machine.update({}, {}, 0);
    machine.update({}, { menu_campaign: 1 }, 1);
    machine.update({}, { menu_campaign: 1 }, 801);
    assert.equal(machine.view().step.id, "boudica_return_replay");
    machine.update({}, { menu_campaign: 1, campaign_replay: 1 }, 802);
    machine.update({}, { menu_campaign: 1, campaign_replay: 1 }, 1602);
    assert.equal(machine.view().step.id, "boudica_return_multiplayer");
    machine.update({}, { menu_campaign: 1, campaign_replay: 1, menu_multiplayer: 1 }, 1603);
    machine.update({}, { menu_campaign: 1, campaign_replay: 1, menu_multiplayer: 1 }, 2403);
    assert.equal(machine.view().step.id, "boudica_return_end");
});

test("tutorial locale and UI guides resolve per episode and point to actual HUD controls", () => {
    assert.match(tutorial, /strings\[locale\]/);
    assert.match(tutorial, /SOWCampaign\.resolveUiTarget\(guide\.target, document, runtime\.episodeId\)/);
    assert.match(tutorial, /tutorial\[guide\.target\]/);
    assert.match(campaignView, /sow-story__gesture/);
    assert.match(campaignEngine, /menu_campaign:/);
    assert.match(campaignEngine, /map_attack:/);
    assert.match(tutorial, /addEventListener\("resize", redrawCampaign/);
    assert.match(tutorial, /var actionEvent = control && control\.matches\("input"\) \? "change" : "click"/);
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
    assert.match(hud, /else if \(nukeItem\) \{\s*mapSector\(radial, nukeItem, 3, radialCount, "nuke"\)/);
    assert.doesNotMatch(hud, /mapMenuView === "nuke"|mapGroupSector\([^;]*"nuke"/);
    assert.match(campaignEngine, /map_nuke: '#sow-hud \[data-map-action="nuke"\]'/);
    assert.equal([...campaignEditorHtml.matchAll(/data-map-back/g)].length, 1);
    assert.doesNotMatch(campaignEditor, /syncPreviewMapMenu/);
    assert.match(campaignEditor, /else if \(group\) hudRoot\.dataset\.previewMapMenu = group\.dataset\.mapGroup;[\s\S]*?paintPreview\(\);/);
    assert.match(campaignEditor, /var menuHint = requiredMenu && \$\("#sow-hud"\)\.dataset\.previewMapMenu !== requiredMenu/);
    assert.match(campaignEditor, /function previewAnchor\(step\)[\s\S]*?target\.getClientRects\(\)\.length/);
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
    assert.match(tutorial, /resolveUiTarget\(guide\.target, document, runtime\.episodeId\)/);
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

test("tutorial dismiss opens the existing leave-match modal and menu exit clears runtime", () => {
    assert.match(tutorial, /data-command=.*prompt_surrender/);
    assert.match(tutorial, /function update\(hud\) \{\s*if \(!runtime\.machine \|\| runtime\.modalOpen \|\|/);
    assert.match(tutorial, /runtime\.machine\.setPaused\(open, performance\.now\(\)\)/);
    assert.match(tutorial, /SOW_tutorial_menu_state_update/);
    assert.match(tutorial, /function reset\(\)/);
    assert.match(hud, /id="sow-hud-surrender-modal"/);
    assert.match(hudCss, /\.sow-campaign-error/);
    assert.doesNotMatch(hudCss, /sow-hud__tutorial-overlay|sow-hud__tutorial-hand/);
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

test("building card separates active construction from an available upgrade", () => {
    const renderStart = hud.indexOf("    function renderBuildingCard(mapMenu) {");
    const renderEnd = hud.indexOf("\n    function updateLeaderboard", renderStart);
    assert.ok(renderStart >= 0 && renderEnd > renderStart);

    const element = () => {
        const classes = new Set();
        return {
            dataset: {},
            textContent: "",
            disabled: false,
            hidden: false,
            classes,
            classList: {
                toggle(name, force) {
                    if (force) classes.add(name);
                    else classes.delete(name);
                }
            }
        };
    };
    const hudRefs = {
        buildingCard: element(),
        buildingCardKind: element(),
        buildingCardLevel: element(),
        buildingCardBenefit: element(),
        buildingCardNext: element(),
        buildingCardUpgrade: element()
    };
    const renderBuildingCard = vm.runInNewContext(
        hud.slice(renderStart, renderEnd) + "\nrenderBuildingCard;",
        { hudRefs }
    );
    const menu = (building, view = "building_details") => ({ open: true, view, session: 4, tile_idx: 12, building });

    renderBuildingCard(menu({
        name: "City",
        kind: "City",
        level: 1,
        benefit: "Current benefit",
        under_construction: true,
        construction_name: "Hamlet",
        remaining_seconds: 7.4,
        can_upgrade: false
    }));
    assert.equal(hudRefs.buildingCardLevel.textContent, "🏗️ Hamlet · 7.4s left");
    assert.equal(hudRefs.buildingCardKind.textContent, "City");
    assert.equal(hudRefs.buildingCardBenefit.hidden, true);
    assert.equal(hudRefs.buildingCardNext.textContent, "");
    assert.equal(hudRefs.buildingCardUpgrade.hidden, true);
    assert.equal(hudRefs.buildingCardUpgrade.disabled, true);

    renderBuildingCard(menu({
        name: "Camp",
        kind: "City",
        level: 1,
        benefit: "Current benefit",
        next_level: 2,
        next_name: "Hamlet",
        next_benefit: "Next benefit",
        duration_seconds: 2.2,
        cost: 150,
        can_upgrade: true
    }));
    assert.equal(hudRefs.buildingCardLevel.textContent, "Level 1");
    assert.equal(hudRefs.buildingCardBenefit.hidden, false);
    assert.equal(hudRefs.buildingCardNext.textContent, "Next: Hamlet · Next benefit · 2.2s");
    assert.equal(hudRefs.buildingCardUpgrade.textContent, "Upgrade · 150g");
    assert.equal(hudRefs.buildingCardUpgrade.hidden, false);
    assert.equal(hudRefs.buildingCardUpgrade.disabled, false);

    renderBuildingCard(menu({ kind: "City", level: 1 }, "radial"));
    assert.equal(hudRefs.buildingCard.classes.has("hidden"), true);
    renderBuildingCard(null);
    assert.equal(hudRefs.buildingCard.classes.has("hidden"), true);
});

test("primary click selects owned buildings without changing other map gestures", () => {
    const clickStart = mapClick.indexOf("pub(crate) fn handle_map_click");
    const clickEnd = mapClick.indexOf("pub(crate) fn open_map_context_menu", clickStart);
    const clickBody = mapClick.slice(clickStart, clickEnd);
    assert.ok(clickBody.indexOf("selected_nuke_kind") < clickBody.indexOf("select_owned_building"));
    assert.ok(clickBody.indexOf("selected_building_kind") < clickBody.indexOf("select_owned_building"));
    assert.ok(clickBody.indexOf("select_warships_at") < clickBody.indexOf("select_owned_building"));
    assert.match(clickBody, /select_owned_building\(x, y\)[\s\S]*?primary_target\(tile_idx/);
    assert.match(mapClick, /building_at_pointer\(/);
    assert.match(mapClick, /target\.owner != target\.my_id/);
    assert.match(buildingOverlaySource, /pub\(crate\) fn building_at_pointer/);
    assert.match(buildingOverlaySource, /cached_buildings\(/);
    assert.match(buildingOverlaySource, /if building\.owner_id != my_id/);
    assert.match(buildingOverlaySource, /hit_radius = .*\.max\(12\.0\)/);
    assert.match(buildingOverlaySource, /nearest_building_in_cluster/);
    assert.match(buildingOverlaySource, /building_marker_size\(building, lod, zoom_scaled\)/);
    assert.match(appStateSource, /pub enum MapContextMenuView\s*\{\s*BuildingDetails,\s*Radial/);
    assert.match(mapClick, /MapContextMenuView::BuildingDetails/);
    assert.match(mapClick, /MapContextMenuView::Radial/);
    assert.match(windowInput, /if is_quick_tap\(elapsed_ms, distance_sq\)\s*\{\s*self\.handle_map_click/);
    assert.match(windowInput, /else if right && pressed[\s\S]*?self\.open_map_context_menu\(x, y\)/);
    assert.match(windowInput, /pub\(crate\) fn poll_pointer_hold[\s\S]*?self\.open_map_context_menu\(x, y\)/);
    assert.match(webMenu, /"view": match menu\.view[\s\S]*?"building_details"[\s\S]*?"radial"/);
    assert.match(hud, /var showRadial = Boolean\(open && mapMenu\.view === "radial"\)/);
    assert.match(hud, /mapMenu\.view === "building_details" && detail/);
    assert.match(hud, /send\("close_map_context_menu"\)/);
    assert.match(webMenu, /WebMenuCommand::CloseMapContextMenu => self\.close_map_context_menu\(\)/);
    assert.match(mapClick, /pub\(crate\) fn close_map_context_menu\(&mut self\)\s*\{\s*self\.input\.map_context_menu = None;/);
    assert.match(webMenu, /map_menu_view: map_menu\.map\(\|menu\| menu\.view\)/);

    const targetStart = mapClick.indexOf("fn primary_target(&mut self");
    const targetEnd = mapClick.indexOf("fn attack_from_tile", targetStart);
    const targetBody = mapClick.slice(targetStart, targetEnd);
    assert.match(targetBody, /target\.owner == target\.my_id\s*\{\s*return;/);
    assert.match(targetBody, /if target\.is_friendly\(\)\s*\{\s*self\.open_transfer_from_tile\(tile_idx\);\s*\} else \{\s*self\.attack_from_tile\(tile_idx\);/);

    const actionStart = mapClick.indexOf("pub(crate) fn handle_map_menu_action");
    const actionEnd = mapClick.indexOf("fn map_menu_cost", actionStart);
    const actionBody = mapClick.slice(actionStart, actionEnd);
    assert.match(actionBody, /menu\.session != session \|\| menu\.tile_idx != tile_idx/);
    assert.match(actionBody, /!self\.map_menu_actions\(tile_idx\)\.contains\(&action\)/);
});

test("WASM right-click opens on pointer press; touch hold stays intact", () => {
    assert.match(windowInput, /let right = matches!\([\s\S]*?MouseButton::Right[\s\S]*?\);/);
    const rightClickStart = windowInput.indexOf("} else if right && pressed");
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
    assert.match(openBody, /if self\.map_menu_actions\(tile_idx\)\.is_empty\(\) \{\s*self\.show_map_menu_unavailable\(tile_idx\);\s*\}/);
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
    assert.match(hud, /title\.innerHTML = hudIcon\(label\.icon/);
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
    assert.doesNotMatch(hud, /addEventListener\("(?:touchstart|touchmove|pointermove)"/);
    assert.match(hud, /hudRefs\.slider\.addEventListener\("pointerdown"/);
});

test("army allocation starts centered and only formats send/keep while shown", () => {
    assert.match(hudStateSource, /attack_ratio: 0\.5/);
    assert.match(hud, /type="range" min="5" max="100" value="50"/);
    assert.doesNotMatch(hud, /allocationHideTimer|setTimeout\([^\n]*allocation/);
    assert.match(hud, /detailsVisible = forceDetails \|\|[\s\S]*?if \(!detailsVisible\) return;[\s\S]*?SOW_t\("hud\.attack_allocation"/);
    assert.match(hud, /pointerup[\s\S]*?remove\("is-adjusting"\)/);
    assert.match(hudCss, /range-vertical::-webkit-slider-runnable-track \{ height: 12px/);
    assert.match(hudCss, /range-vertical::-moz-range-track \{ height: 12px/);
    assert.match(hudCss, /range-vertical::-webkit-slider-thumb/);
    assert.match(hudCss, /range-vertical::-moz-range-thumb/);
    const allocationRail = hud.slice(hud.indexOf("'<aside class=\"sow-hud__left-rail\""), hud.indexOf("'<aside class=\"sow-hud__right-rail\""));
    assert.doesNotMatch(allocationRail, /hudIcon\("troops", "sow-hud__action-icon"\)/);
    assert.match(hud, /slider\.style\.setProperty\("--sow-crossed-swords", 'url\("' \+ asset\(HUD_ICONS\.troops\)/);
    assert.match(hudCss, /background: var\(--sow-gold\) var\(--sow-crossed-swords\) center/);
});

test("campaign players use the active leader name in both map and leaderboard", () => {
    assert.match(matchStartSource, /name: \{\s*if tutorial \{\s*leader\.name\(\)\.to_string\(\)/);
    const leaderboard = hud.slice(hud.indexOf("function updateLeaderboard"), hud.indexOf("function renderInbox"));
    assert.match(leaderboard, /var displayName = player\.name \|\| SOW_t\("hud\.player_name"\)/);
    assert.doesNotMatch(leaderboard, /campaignActive|leaderById/);
});

test("tutorial hand follows the selected campaign target", () => {
    assert.match(webMenu, /WebMenuCommand::SetTutorialMarker \{ player_id \} => \{[\s\S]*?tutorial_marker_player_id = player_id/);
    assert.match(webMenu, /tutorial_target_action_tile\([\s\S]*?target_owner/);
    assert.match(tutorial, /send\("set_tutorial_marker", \{ player_id: markerId \}\)/);
    assert.match(tutorial, /Array\.isArray\(step\.trigger\.targets\)/);
    assert.match(campaignView, /const hasMapTarget = step\.marker \|\| \(step\.guide && step\.guide\.kind === "world" && step\.guide\.target === "target_action" && step\.trigger && \(step\.trigger\.target \|\| Array\.isArray\(step\.trigger\.targets\)\)\)/);
    assert.match(campaignEditor, /Array\.isArray\(step\.trigger && step\.trigger\.targets\)/);
    assert.match(campaignView, /data-tutorial-hand/);
    assert.doesNotMatch(tutorial, /tutorial\.markers|data-tutorial-marker|SOW_tutorial_marker_update/);
    assert.doesNotMatch(hudCss, /sow-hud__tutorial-marker/);
});

test("tutorial hand uses the map radial action's exact icon anchor", () => {
    const campaign = require(path.join(shell, "sow-campaign.js"));
    assert.match(hud, /button\.dataset\.tutorialAnchorX = String\(anchorX\)/);
    assert.match(hud, /button\.dataset\.tutorialAnchorY = String\(anchorY\)/);
    assert.match(tutorial, /SOWCampaign\.resolveUiAnchor\(source\)/);
    assert.match(tutorial, /SOWCampaign\.resolveUiAnchor\(destination\)/);
    assert.match(campaignEditor, /SOWCampaign\.resolveUiAnchor\(target\)/);
    assert.match(campaignEditor, /SOWCampaign\.resolveUiAnchor\(end\)/);
    assert.match(campaignEditor, /window\.addEventListener\("resize", function \(\) \{ drawWires\(\); if \(state\.machine\) paintPreview\(false\); \}/);
    const rect = (left, top, width, height) => ({ left, top, width, height });
    const icon = { getClientRects: () => [{}], getBoundingClientRect: () => rect(265, 75, 24, 28) };
    const radial = {
        dataset: { tutorialAnchorX: "75", tutorialAnchorY: "25" },
        getBoundingClientRect: () => rect(100, 50, 200, 100),
        querySelector: (selector) => selector === ".sow-hud__map-action-title" ? icon : null
    };
    assert.deepEqual(campaign.resolveUiAnchor(radial), { x: 250, y: 75, width: 24, height: 28 });
    assert.deepEqual(campaign.resolveUiAnchor({ getBoundingClientRect: () => rect(10, 20, 40, 60) }), { x: 30, y: 50, width: 40, height: 60 });
});
