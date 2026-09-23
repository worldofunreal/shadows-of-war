(function () {
    "use strict";

    // ─────────────────────────────────────────────────────────
    // HUD CONTROLLER (DOM Overlay for In-Game RTS Matches)
    // ─────────────────────────────────────────────────────────
    var hudRoot = document.getElementById("sow-hud");
    var hudState = null;
    var lastHudRaw = "";
    var leaderboardOpen = false;
    var inboxOpen = false;
    var transferOpen = false;
    var betrayalOpen = false;
    var pinEmoji = false;
    var surrenderModalOpen = false;
    var surrenderMessage = null;
    var emojiPickerOpen = false;
    var devSidebarOpen = false;
    var hudSettingsOpen = false;

    var hudInitialized = false;
    var hudRefs = null;
    var leaderboardRows = Object.create(null);
    var leaderboardRenderKey = "";
    var lastLeaderboardPlayers = [];
    var inboxRenderKey = "";
    var mapMenuView = "root";
    var mapMenuStateKey = "";

    var EMOJIS = [
        "😀", "😎", "😏", "😂", "🤣", "😋", "😉", "😜", "😍", "🥰", "🥳", "🥺", "😇", "🤩", "👍",
        "❤️", "😮", "🤔", "🧐", "🙄", "🤯", "🤡", "💩", "🤫", "😠", "😡", "🤬", "😤", "🥵", "🥶",
        "🤢", "🤮", "⚔️", "🛡️", "🏹", "💣", "💥", "💀", "👑", "💪", "🔥", "👀", "🏳️", "🤝", "💔",
        "🔌", "⭐", "🐺"
    ];

    var surrenderMessageKeys = [
        "endgame.match_message_1",
        "endgame.match_message_2",
        "endgame.match_message_3",
        "endgame.match_message_4",
        "endgame.match_message_5"
    ];

    function send(type, extra) {
        if (typeof window.SOW_menu_command !== "function") return false;
        var command = Object.assign({ type: type }, extra || {});
        window.SOW_menu_command(JSON.stringify(command));
        return true;
    }

    function asset(path) {
        var base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
        return base + "/" + path.split("/").map(encodeURIComponent).join("/");
    }

    function currencyAsset(kind) {
        return asset("gameplay/currency/" + kind + ".webp");
    }

    function leaderById(id) {
        var leaders = hudState && Array.isArray(hudState.leaders) ? hudState.leaders : [];
        var found = leaders.find(function (leader) { return leader.id === id; });
        if (found) return found;
        if (id) {
            return {
                id: id,
                name: String(id),
                slug: String(id).replace(/([a-z])([A-Z])/g, "$1_$2").replace(/\s+/g, "_").toLowerCase()
            };
        }
        return leaders[0] || { id: "Caesar", name: "Caesar", slug: "caesar" };
    }

    function ensureHudDom() {
        if (!hudRoot || hudInitialized) return;
        hudInitialized = true;
        hudRoot.innerHTML = ''
            + '<header class="sow-hud__topbar">'
            + '  <div class="sow-hud__status-left">'
            + '    <button class="sow-hud__icon-pill" type="button" data-command="toggle_leaderboard" aria-label="' + SOW_t("hud.rankings") + '" title="' + SOW_t("hud.rankings") + '">🏆</button>'
            + '    <button class="sow-hud__icon-pill hidden" type="button" data-command="toggle_dev_sidebar" id="sow-hud-dev-btn" aria-label="' + SOW_t("hud.dev_tools") + '" title="' + SOW_t("hud.dev_tools") + '">🛠</button>'
            + '  </div>'
            + '  <div class="sow-hud__status-right">'
            + '    <span class="sow-hud__fps" id="sow-hud-fps">' + SOW_t("hud.fps", { fps: "--" }) + '</span>'
            + '    <button class="sow-hud__icon-pill sow-hud__inbox-pill" type="button" data-command="toggle_inbox" aria-label="' + SOW_t("hud.inbox") + '" title="' + SOW_t("hud.inbox") + '">📩 <span class="sow-hud__inbox-badge" id="sow-hud-inbox-count">0</span></button>'
            + '    <button class="sow-hud__icon-pill" type="button" data-command="toggle_settings" aria-label="' + SOW_t("menu.settings") + '" title="' + SOW_t("menu.settings") + '">⚙</button>'
            + '    <button class="sow-hud__icon-pill sow-hud__exit-pill" type="button" data-command="prompt_surrender" aria-label="' + SOW_t("hud.leave_match") + '" title="' + SOW_t("hud.leave_match") + '">✕</button>'
            + '  </div>'
            + '</header>'
            + '<div class="sow-hud__hover-card hidden" id="sow-hud-hover-card">'
            + '  <div class="sow-hud__hover-header"><span id="sow-hud-hover-avatar">👑</span> <b id="sow-hud-hover-name">' + SOW_t("hud.territory") + '</b></div>'
            + '  <div class="sow-hud__hover-stats">'
            + '    <span><b id="sow-hud-hover-pct">0%</b> ' + SOW_t("hud.land") + '</span>'
            + '    <span><b id="sow-hud-hover-troops">0</b> ⚔</span>'
            + '    <span><b id="sow-hud-hover-gold">0</b> <img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' + currencyAsset("gold") + '" alt="" aria-hidden="true"></span>'
            + '  </div>'
            + '  <div class="sow-hud__hover-buildings" id="sow-hud-hover-blds"></div>'
            + '</div>'
            + '<aside class="sow-hud__left-rail" id="sow-hud-left-rail">'
            + '  <div class="sow-hud__rail-card">'
            + '    <div class="sow-hud__slider-vertical-wrap">'
            + '      <input type="range" min="5" max="100" value="50" step="5" class="sow-hud__range-vertical" id="sow-hud-slider">'
            + '    </div>'
            + '  </div>'
            + '</aside>'
            + '<aside class="sow-hud__right-rail" id="sow-hud-right-rail">'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_in" aria-label="' + SOW_t("hud.zoom_in") + '" title="' + SOW_t("hud.zoom_in") + '">➕</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_out" aria-label="' + SOW_t("hud.zoom_out") + '" title="' + SOW_t("hud.zoom_out") + '">➖</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="center_camera" aria-label="' + SOW_t("hud.center_camera") + '" title="' + SOW_t("hud.center_camera") + '">🏠</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="toggle_emoji" aria-label="' + SOW_t("hud.emojis") + '" title="' + SOW_t("hud.emojis") + '">😀</button>'
            + '</aside>'
            + '<div class="sow-hud__emoji-popout hidden" id="sow-hud-emoji-popout">'
            + '  <div class="sow-hud__emoji-header">'
            + '    <span>' + SOW_t("hud.express_reaction") + '</span>'
            + '    <button type="button" class="sow-hud__pin-btn" data-command="toggle_pin_emoji" aria-pressed="false">' + SOW_t("hud.pin") + '</button>'
            + '    <button type="button" class="sow-hud__close-btn" data-command="toggle_emoji" aria-label="' + SOW_t("hud.close_emojis") + '">✕</button>'
            + '  </div>'
            + '  <div class="sow-hud__emoji-grid" id="sow-hud-emoji-grid"></div>'
            + '</div>'
            + '<aside class="sow-hud__dev-sidebar hidden" id="sow-hud-dev-sidebar">'
            + '  <div class="sow-hud__dev-header"><b>' + SOW_t("hud.dev_tools") + '</b><button type="button" class="sow-hud__close-btn" data-command="toggle_dev_sidebar" aria-label="' + SOW_t("hud.close_developer_tools") + '">✕</button></div>'
            + '  <div class="sow-hud__dev-body">'
            + '    <div class="sow-hud__dev-section"><b>' + SOW_t("hud.map_borders") + '</b>'
            + '      <label class="sow-hud__dev-row">' + SOW_t("hud.border_thickness") + ' <input type="range" min="0" max="1" step="0.01" value="0.5" data-dev="thickness"></label>'
            + '      <label class="sow-hud__dev-row">' + SOW_t("hud.border_darkness") + ' <input type="range" min="0" max="1" step="0.01" value="0.5" data-dev="darkness"></label>'
            + '      <label class="sow-hud__dev-row">' + SOW_t("hud.shore_thickness") + ' <input type="range" min="0" max="1" step="0.01" value="0.5" data-dev="shore_thickness"></label>'
            + '      <label class="sow-hud__dev-row">' + SOW_t("hud.conquest_duration") + ' <input type="range" min="0.1" max="10" step="0.1" value="1.5" data-dev="conquest_duration"></label>'
            + '      <label class="sow-hud__dev-row">' + SOW_t("hud.opacity") + ' <input type="range" min="0" max="1" step="0.01" value="1" data-dev="territory_opacity"></label>'
            + '      <button type="button" class="sow-hud__dev-reset" data-command="reset_dev_config">' + SOW_t("hud.reset") + '</button>'
            + '    </div>'
            + '  </div>'
            + '</aside>'
            + '<aside class="sow-hud__settings hidden" id="sow-hud-settings">'
            + '  <div class="sow-hud__panel-header"><h3>' + SOW_t("menu.settings") + '</h3><button class="sow-hud__close-btn" type="button" data-command="toggle_settings" aria-label="' + SOW_t("hud.close_settings") + '">✕</button></div>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.sound") + '</span><input type="checkbox" data-hud-setting="mute_all"></label>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.music") + '</span><input type="range" min="0" max="1" step="0.05" data-hud-setting="music_volume"></label>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.reduced_motion") + '</span><input type="checkbox" data-hud-setting="reduced_motion"></label>'
            + '</aside>'
            + '<footer class="sow-hud__dock" id="sow-hud-dock">'
            + '  <div class="sow-hud__dock-inner" id="sow-hud-dock-inner">'
            + '    <div class="sow-hud__dock-actions">'
            + '      <div class="sow-hud__deploy-panel hidden" id="sow-hud-deploy-btn">'
            + '        <span class="sow-hud__deploy-title">' + SOW_t("hud.choose_spawn") + '</span>'
            + '        <b class="sow-hud__deploy-timer" id="sow-hud-deploy-timer">' + SOW_t("hud.ready") + '</b>'
            + '      </div>'
            + '      <div class="sow-hud__buildings-strip" id="sow-hud-buildings-strip">'
            + '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="City" aria-label="' + SOW_t("hud.map_action_city") + '">🏛️</button>'
            + '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="Factory" aria-label="' + SOW_t("hud.map_action_factory") + '">🏭</button>'
            + '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="Port" aria-label="' + SOW_t("hud.map_action_port") + '">⚓</button>'
            + '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="Bunker" aria-label="' + SOW_t("hud.map_action_bunker") + '">🛡️</button>'
            + '      </div>'
            + '    </div>'
            + '    <div class="sow-hud__resource-row" id="sow-hud-resource-row">'
            + '      <div class="sow-hud__res-rate" id="sow-hud-res-rate" title="' + SOW_t("hud.troop_production_rate") + '">'
            + '        <span class="sow-hud__rate-text" data-role="prod">⚔ +0/s</span>'
            + '      </div>'
            + '      <div class="sow-hud__res-bar-wrap" title="' + SOW_t("hud.troop_pool_capacity") + '">'
            + '        <div class="sow-hud__res-bar-fill" id="sow-hud-troop-fill" style="width: 0%;"></div>'
            + '        <span class="sow-hud__res-bar-text" data-role="troops">0 / 0 ⚔</span>'
            + '      </div>'
            + '      <div class="sow-hud__res-gold" id="sow-hud-res-gold" title="' + SOW_t("hud.gold_treasury") + '">'
            + '        <span class="sow-hud__gold-text"><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' + currencyAsset("gold") + '" alt="" aria-hidden="true"><b data-role="gold">0</b></span>'
            + '      </div>'
            + '    </div>'
            + '  </div>'
            + '</footer>'
            + '<aside class="sow-hud__leaderboard hidden" id="sow-hud-leaderboard">'
            + '  <div class="sow-hud__panel-header">'
            + '    <h3>' + SOW_t("hud.rankings_title") + '</h3>'
            + '    <button class="sow-hud__close-btn" type="button" data-command="toggle_leaderboard" aria-label="' + SOW_t("hud.close_rankings") + '">✕</button>'
            + '  </div>'
            + '  <div class="sow-hud__leaderboard-rows" id="sow-hud-lb-rows"></div>'
            + '</aside>'
            + '<aside class="sow-hud__panel sow-hud__inbox hidden" id="sow-hud-inbox">'
            + '  <div class="sow-hud__panel-header"><h3>' + SOW_t("hud.inbox") + '</h3><button class="sow-hud__close-btn" type="button" data-command="toggle_inbox" aria-label="' + SOW_t("hud.close_inbox") + '">✕</button></div>'
            + '  <div class="sow-hud__panel-rows" id="sow-hud-inbox-rows"></div>'
            + '</aside>'
            + '<aside class="sow-hud__panel sow-hud__transfer hidden" id="sow-hud-transfer">'
            + '  <div class="sow-hud__panel-header"><h3>' + SOW_t("hud.resource_transfer") + '</h3><button class="sow-hud__close-btn" type="button" data-command="close_transfer" aria-label="' + SOW_t("hud.close_resource_transfer") + '">✕</button></div>'
            + '  <p id="sow-hud-transfer-target"></p>'
            + '  <label>' + SOW_t("hud.gold") + '<input id="sow-hud-transfer-gold" type="number" min="0" step="1" value="0"></label>'
            + '  <label>' + SOW_t("hud.troops") + '<input id="sow-hud-transfer-troops" type="number" min="0" step="1" value="0"></label>'
            + '  <div class="sow-hud__panel-actions"><button type="button" data-command="send_resources">' + SOW_t("hud.send") + '</button><button type="button" data-command="request_resources">' + SOW_t("hud.request") + '</button></div>'
            + '</aside>'
            + '<div class="sow-hud__modal-backdrop hidden" id="sow-hud-betrayal-modal">'
            + '  <div class="sow-hud__modal-card"><h3>' + SOW_t("hud.break_alliance") + '</h3><p id="sow-hud-betrayal-copy">' + SOW_t("hud.break_alliance_body") + '</p><div class="sow-hud__panel-actions"><button type="button" data-command="cancel_betrayal">' + SOW_t("hud.keep_alliance") + '</button><button class="sow-hud__btn-danger" type="button" data-command="confirm_betrayal">' + SOW_t("hud.attack") + '</button></div></div>'
            + '</div>'
            + '<div class="sow-hud__endgame-backdrop hidden" id="sow-hud-surrender-modal">'
            + '  <section class="sow-hud__endgame-card sow-hud__exit-card" data-result="defeat" role="dialog" aria-modal="true" aria-labelledby="sow-hud-surrender-banner" aria-describedby="sow-hud-surrender-desc">'
            + '    <div class="sow-hud__endgame-hero">'
            + '      <div class="sow-hud__endgame-portrait-wrap">'
            + '        <img class="sow-hud__endgame-portrait" id="sow-hud-surrender-portrait" src="" alt="' + SOW_t("hud.leader_avatar") + '" />'
            + '      </div>'
            + '      <div class="sow-hud__endgame-hero-copy">'
            + '        <h2 class="sow-hud__endgame-banner" id="sow-hud-surrender-banner">' + SOW_t("endgame.leave_match") + '</h2>'
            + '        <p class="sow-hud__endgame-desc" id="sow-hud-surrender-desc">' + SOW_t("endgame.your_battle_will_end") + '</p>'
            + '      </div>'
            + '    </div>'
            + '    <div class="sow-hud__endgame-actions">'
            + '      <button class="sow-hud__endgame-secondary" type="button" data-command="close_surrender_modal" id="sow-hud-surrender-cancel">' + SOW_t("endgame.cancel") + '</button>'
            + '      <button class="sow-hud__endgame-primary" type="button" data-command="confirm_surrender"><span aria-hidden="true">⌂</span> <span id="sow-hud-surrender-action-label">' + SOW_t("endgame.leave_match") + '</span></button>'
            + '    </div>'
            + '  </section>'
            + '</div>'
            + '<div class="sow-hud__endgame-backdrop hidden" id="sow-hud-endgame-modal">'
            + '  <section class="sow-hud__endgame-card" role="dialog" aria-modal="true" aria-labelledby="sow-hud-endgame-title">'
            + '    <div class="sow-hud__endgame-hero">'
            + '      <div class="sow-hud__endgame-portrait-wrap">'
            + '        <img class="sow-hud__endgame-portrait" id="sow-hud-endgame-portrait" src="" alt="' + SOW_t("hud.leader_avatar") + '" />'
            + '      </div>'
            + '      <div class="sow-hud__endgame-hero-copy">'
            + '        <div class="sow-hud__endgame-kicker"><span class="sow-hud__endgame-icon" id="sow-hud-endgame-icon" aria-hidden="true">⚔</span><span>' + SOW_t("hud.match_result") + '</span></div>'
            + '        <h2 class="sow-hud__endgame-banner" id="sow-hud-endgame-banner">' + SOW_t("hud.defeat") + '</h2>'
            + '        <h3 class="sow-hud__endgame-title" id="sow-hud-endgame-title">' + SOW_t("hud.match_lost") + '</h3>'
            + '        <p class="sow-hud__endgame-desc" id="sow-hud-endgame-desc">' + SOW_t("hud.match_ended") + '</p>'
            + '      </div>'
            + '    </div>'
            + '    <div class="sow-hud__endgame-stats" id="sow-hud-endgame-stats">'
            + '      <div class="sow-hud__endgame-stat"><span class="sow-hud__endgame-stat-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 4l14 14M18 4L4 18M5 5l3 3M19 5l-3 3M5 19l3-3M19 19l-3-3"/></svg></span><span class="sow-hud__endgame-stat-label">' + SOW_t("profile.kda") + '</span><b id="sow-hud-endgame-kda">0 / 0 / 0</b></div>'
            + '      <div class="sow-hud__endgame-stat"><span class="sow-hud__endgame-stat-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m12 3 2.1 5.1L19 10l-4.9 1.9L12 17l-2.1-5.1L5 10l4.9-1.9z"/></svg></span><span class="sow-hud__endgame-stat-label">' + SOW_t("endgame.xp") + '</span><b id="sow-hud-endgame-xp">+0</b></div>'
            + '      <div class="sow-hud__endgame-stat"><span class="sow-hud__endgame-stat-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m4 7 4 4 4-6 4 6 4-4-2 10H6zM6 20h12"/></svg></span><span class="sow-hud__endgame-stat-label">' + SOW_t("endgame.leader_xp") + '</span><b id="sow-hud-endgame-leader-xp">+0</b></div>'
            + '      <div class="sow-hud__endgame-stat"><span class="sow-hud__endgame-stat-icon" aria-hidden="true"><img src="' + currencyAsset("crown") + '" alt=""></span><span class="sow-hud__endgame-stat-label">' + SOW_t("endgame.crowns") + '</span><b id="sow-hud-endgame-crowns">+0</b></div>'
            + '      <div class="sow-hud__endgame-stat"><span class="sow-hud__endgame-stat-icon" aria-hidden="true"><img src="' + currencyAsset("laurel") + '" alt=""></span><span class="sow-hud__endgame-stat-label">' + SOW_t("endgame.laurels") + '</span><b id="sow-hud-endgame-laurels">+0</b></div>'
            + '    </div>'
            + '    <div class="sow-hud__endgame-store" id="sow-hud-endgame-store" aria-label="' + SOW_t("store.featured_skin") + '">'
            + '      <span class="sow-hud__endgame-store-icon" aria-hidden="true">✦</span>'
            + '      <span class="sow-hud__endgame-store-copy"><b id="sow-hud-endgame-store-name">' + SOW_t("store.featured_skin") + '</b><small id="sow-hud-endgame-store-copy">' + SOW_t("store.original_cosmetic") + '</small></span>'
            + '      <button class="sow-hud__endgame-store-state" type="button" data-command="open_store" id="sow-hud-endgame-store-action">' + SOW_t("hud.view_shop") + '</button>'
            + '    </div>'
            + '    <div class="sow-hud__endgame-actions">'
            + '      <button class="sow-hud__endgame-secondary hidden" type="button" data-command="continue_observing" id="sow-hud-endgame-observe"><span aria-hidden="true">◉</span> ' + SOW_t("hud.continue_observing") + '</button>'
            + '      <button class="sow-hud__endgame-primary" type="button" data-command="confirm_endgame_leave"><span aria-hidden="true">⌂</span> ' + SOW_t("hud.back_to_menu") + '</button>'
            + '    </div>'
            + '  </section>'
            + '</div>'
            + '<div class="sow-hud__map-menu hidden" id="sow-hud-map-menu" role="menu" aria-label="' + SOW_t("hud.map_actions") + '"></div>';

        var emojiGrid = document.getElementById("sow-hud-emoji-grid");
        if (emojiGrid) {
            EMOJIS.forEach(function (emoji) {
                var cell = document.createElement("button");
                cell.type = "button";
                cell.className = "sow-hud__emoji-cell";
                cell.textContent = emoji;
                cell.dataset.command = "express_emoji";
                cell.dataset.emoji = emoji;
                emojiGrid.appendChild(cell);
            });
        }

        hudRefs = {
            gold: hudRoot.querySelector('[data-role="gold"]'),
            troops: hudRoot.querySelector('[data-role="troops"]'),
            prod: hudRoot.querySelector('[data-role="prod"]'),
            fps: document.getElementById("sow-hud-fps"),
            inboxCount: document.getElementById("sow-hud-inbox-count"),
            hoverCard: document.getElementById("sow-hud-hover-card"),
            hoverAvatar: document.getElementById("sow-hud-hover-avatar"),
            hoverName: document.getElementById("sow-hud-hover-name"),
            hoverPct: document.getElementById("sow-hud-hover-pct"),
            hoverTroops: document.getElementById("sow-hud-hover-troops"),
            hoverGold: document.getElementById("sow-hud-hover-gold"),
            hoverBlds: document.getElementById("sow-hud-hover-blds"),
            leftRail: document.getElementById("sow-hud-left-rail"),
            slider: document.getElementById("sow-hud-slider"),
            rightRail: document.getElementById("sow-hud-right-rail"),
            emojiPopout: document.getElementById("sow-hud-emoji-popout"),
            pinEmoji: hudRoot.querySelector("[data-command='toggle_pin_emoji']"),
            devSidebar: document.getElementById("sow-hud-dev-sidebar"),
            devBtn: document.getElementById("sow-hud-dev-btn"),
            settings: document.getElementById("sow-hud-settings"),
            deployBtn: document.getElementById("sow-hud-deploy-btn"),
            deployTimer: document.getElementById("sow-hud-deploy-timer"),
            buildingsStrip: document.getElementById("sow-hud-buildings-strip"),
            buildingButtons: Array.prototype.slice.call(hudRoot.querySelectorAll("[data-command='select_building']")),
            troopFill: document.getElementById("sow-hud-troop-fill"),
            leaderboard: document.getElementById("sow-hud-leaderboard"),
            rows: document.getElementById("sow-hud-lb-rows"),
            inbox: document.getElementById("sow-hud-inbox"),
            inboxRows: document.getElementById("sow-hud-inbox-rows"),
            transfer: document.getElementById("sow-hud-transfer"),
            transferTarget: document.getElementById("sow-hud-transfer-target"),
            transferGold: document.getElementById("sow-hud-transfer-gold"),
            transferTroops: document.getElementById("sow-hud-transfer-troops"),
            betrayal: document.getElementById("sow-hud-betrayal-modal"),
            betrayalCopy: document.getElementById("sow-hud-betrayal-copy"),
            surrender: document.getElementById("sow-hud-surrender-modal"),
            surrenderBanner: document.getElementById("sow-hud-surrender-banner"),
            surrenderDesc: document.getElementById("sow-hud-surrender-desc"),
            surrenderPortrait: document.getElementById("sow-hud-surrender-portrait"),
            surrenderCancel: document.getElementById("sow-hud-surrender-cancel"),
            surrenderActionLabel: document.getElementById("sow-hud-surrender-action-label"),
            endgame: document.getElementById("sow-hud-endgame-modal"),
            endgameCard: document.querySelector("#sow-hud-endgame-modal .sow-hud__endgame-card"),
            endgameIcon: document.getElementById("sow-hud-endgame-icon"),
            endgameBanner: document.getElementById("sow-hud-endgame-banner"),
            endgameTitle: document.getElementById("sow-hud-endgame-title"),
            endgameDesc: document.getElementById("sow-hud-endgame-desc"),
            endgamePortrait: document.getElementById("sow-hud-endgame-portrait"),
            endgameStats: document.getElementById("sow-hud-endgame-stats"),
            endgameKda: document.getElementById("sow-hud-endgame-kda"),
            endgameXp: document.getElementById("sow-hud-endgame-xp"),
            endgameLeaderXp: document.getElementById("sow-hud-endgame-leader-xp"),
            endgameCrowns: document.getElementById("sow-hud-endgame-crowns"),
            endgameLaurels: document.getElementById("sow-hud-endgame-laurels"),
            endgameStore: document.getElementById("sow-hud-endgame-store"),
            endgameStoreName: document.getElementById("sow-hud-endgame-store-name"),
            endgameStoreCopy: document.getElementById("sow-hud-endgame-store-copy"),
            endgameStoreAction: document.getElementById("sow-hud-endgame-store-action"),
            endgameObserve: document.getElementById("sow-hud-endgame-observe"),
            mapMenu: document.getElementById("sow-hud-map-menu"),
            notifications: document.createElement("div")
        };


        hudRefs.notifications.className = "sow-hud__notifications";
        hudRefs.notifications.setAttribute("aria-live", "polite");
        hudRoot.appendChild(hudRefs.notifications);

        if (hudRefs.slider) {
            hudRefs.slider.addEventListener("input", function (e) {
                var val = parseFloat(e.target.value);
                var ratio = val / 100.0;
                send("set_attack_ratio", { ratio: ratio });
            });
        }
        hudRoot.addEventListener("input", function (event) {
            var input = event.target.closest("[data-dev]");
            if (!input) return;
            send("set_dev_config", { field: input.dataset.dev, value: Number(input.value) });
        });
        hudRoot.addEventListener("change", function (event) {
            var input = event.target.closest("[data-hud-setting]");
            if (!input) return;
            var setting = input.dataset.hudSetting;
            if (setting === "mute_all") send("set_mute", { value: !input.checked });
            if (setting === "music_volume") send("set_music_volume", { value: Number(input.value) });
            if (setting === "reduced_motion") send("set_reduced_motion", { value: input.checked });
        });
    }

    var mapActionLabels = {
        spawn: { icon: "🌱", key: "hud.map_action_deploy" },
        attack: { icon: "⚔", key: "hud.map_action_attack" },
        fleet: { icon: "⛵", key: "hud.map_action_fleet" },
        transfer: { icon: "⚖️", key: "hud.map_action_transfer" },
        alliance: { icon: "🤝", key: "hud.map_action_alliance" },
        build_city: { icon: "🏛️", key: "hud.map_action_city" },
        build_factory: { icon: "🏭", key: "hud.map_action_factory" },
        build_port: { icon: "⚓", key: "hud.map_action_port" },
        build_bunker: { icon: "🛡️", key: "hud.map_action_bunker" },
        nuke: { icon: "🚀", key: "hud.map_action_nuke" },
        upgrade_tile: { icon: "✦", text: "Upgrade tile" },
        upgrade_arsenal: { icon: "🚀", text: "Arsenal District" },
        upgrade_port: { icon: "⚓", text: "Port District" },
        upgrade_foundry: { icon: "🏭", text: "Foundry District" },
        build_warship: { icon: "🚢", text: "Warship" },
        build_trade_ship: { icon: "⛴️", text: "Trade Ship" }
    };

    var buildMenuActions = {
        build_city: true,
        build_factory: true,
        build_port: true,
        build_bunker: true,
        upgrade_tile: true,
        upgrade_arsenal: true,
        upgrade_port: true,
        upgrade_foundry: true,
        build_warship: true,
        build_trade_ship: true
    };

    function mapLabel(action) {
        var label = mapActionLabels[action] || { icon: "•", text: action };
        return label.key ? SOW_t(label.key) : label.text;
    }

    function mapItems(mapMenu) {
        var actions = Array.isArray(mapMenu.actions) ? mapMenu.actions : [];
        var supplied = Array.isArray(mapMenu.items) ? mapMenu.items : [];
        return actions.map(function (action) {
            var item = supplied.find(function (candidate) {
                return candidate && candidate.action === action;
            });
            return {
                action: action,
                cost: item && Number.isFinite(Number(item.cost)) ? Number(item.cost) : null,
                level: item && Number.isFinite(Number(item.level)) ? Number(item.level) : null,
                disabled: Boolean(item && item.disabled)
            };
        });
    }

    function mapActionButton(item, className, withDetails) {
        var label = mapActionLabels[item.action] || { icon: "•", text: item.action };
        var button = document.createElement("button");
        var action = item.action;
        button.type = "button";
        button.className = className;
        button.dataset.mapAction = action;
        button.setAttribute("role", "menuitem");
        button.setAttribute("aria-label", mapLabel(action));
        button.disabled = Boolean(item.disabled);
        button.setAttribute("aria-disabled", String(Boolean(item.disabled)));
        var title = document.createElement("span");
        title.className = "sow-hud__map-action-title";
        title.textContent = label.icon;
        button.appendChild(title);
        if (withDetails && (item.cost != null || item.level != null)) {
            var details = document.createElement("small");
            details.className = "sow-hud__map-action-meta";
            var parts = [];
            if (item.level != null) parts.push(item.level + "→" + (item.level + 1));
            if (item.cost != null) parts.push(Math.round(item.cost) + "g");
            details.textContent = parts.join(" · ");
            button.appendChild(details);
        }
        return button;
    }

    function radialSectorGeometry(button, index, count) {
        var span = 360 / count;
        var gap = Math.min(7, span * 0.14);
        var origin = -90 - span / 2;
        var start = origin + index * span + gap / 2;
        var end = origin + (index + 1) * span - gap / 2;
        var outerRadius = 46;
        var innerRadius = 17;
        var points = [];
        var steps = Math.max(4, Math.ceil((end - start) / 12));
        var point = function (angle, radius) {
            var radians = angle * Math.PI / 180;
            return (50 + Math.cos(radians) * radius).toFixed(2) + "% "
                + (50 + Math.sin(radians) * radius).toFixed(2) + "%";
        };
        for (var outer = 0; outer <= steps; outer += 1) {
            points.push(point(start + (end - start) * outer / steps, outerRadius));
        }
        for (var inner = steps; inner >= 0; inner -= 1) {
            points.push(point(start + (end - start) * inner / steps, innerRadius));
        }
        var polygon = "polygon(" + points.join(", ") + ")";
        button.style.clipPath = polygon;
        button.style.webkitClipPath = polygon;

        var midpoint = origin + (index + 0.5) * span;
        var midpointRadians = midpoint * Math.PI / 180;
        var icon = button.querySelector(".sow-hud__map-action-title");
        if (icon) {
            icon.style.left = (50 + Math.cos(midpointRadians) * ((outerRadius + innerRadius) / 2)) + "%";
            icon.style.top = (50 + Math.sin(midpointRadians) * ((outerRadius + innerRadius) / 2)) + "%";
            icon.style.transform = "translate(-50%, -50%)";
        }
    }

    function mapSector(radial, item, index, count, kind) {
        var label = mapActionLabels[item.action] || { icon: "•", text: item.action };
        var button = mapActionButton(item, "sow-hud__map-sector sow-hud__map-sector--" + kind, false);
        button.querySelector(".sow-hud__map-action-title").textContent = label.icon;
        radialSectorGeometry(button, index, count);
        radial.appendChild(button);
    }

    function mapDisabledSector(radial, index, count, kind, icon, label) {
        var button = document.createElement("button");
        button.type = "button";
        button.disabled = true;
        button.className = "sow-hud__map-sector sow-hud__map-sector--" + kind;
        button.setAttribute("aria-label", label);
        button.setAttribute("aria-disabled", "true");
        var title = document.createElement("span");
        title.className = "sow-hud__map-action-title";
        title.textContent = icon;
        button.appendChild(title);
        radialSectorGeometry(button, index, count);
        radial.appendChild(button);
    }

    function mapGroupSector(radial, items, index, count, group, icon, text) {
        var button = document.createElement("button");
        button.type = "button";
        button.className = "sow-hud__map-sector sow-hud__map-sector--" + group;
        button.dataset.mapGroup = group;
        button.setAttribute("role", "menuitem");
        button.setAttribute("aria-label", text);
        var title = document.createElement("span");
        title.className = "sow-hud__map-action-title";
        title.textContent = icon;
        button.appendChild(title);
        radialSectorGeometry(button, index, count);
        radial.appendChild(button);
    }

    function renderMapMenu(mapMenu) {
        if (!hudRefs || !hudRefs.mapMenu) return false;
        var menu = hudRefs.mapMenu;
        var open = Boolean(mapMenu && mapMenu.open);
        menu.classList.toggle("hidden", !open);
        if (!open) {
            menu.dataset.renderKey = "";
            mapMenuStateKey = "";
            mapMenuView = "root";
            return false;
        }
        var items = mapItems(mapMenu);
        var stateKey = String(window.SOW_LOCALE || "en") + ":" + String(mapMenu.session) + ":" + String(mapMenu.tile_idx) + ":" + JSON.stringify(items);
        if (!items.length) {
            menu.replaceChildren();
            menu.dataset.renderKey = "";
            menu.classList.add("hidden");
            mapMenuStateKey = stateKey;
            mapMenuView = "root";
            return false;
        }
        if (stateKey !== mapMenuStateKey) {
            mapMenuStateKey = stateKey;
            mapMenuView = "root";
        }
        var renderKey = stateKey + ":" + mapMenuView;
        if (menu.dataset.renderKey !== renderKey) {
            menu.replaceChildren();
            menu.classList.toggle("is-submenu", mapMenuView !== "root");
            var buildItems = items.filter(function (item) { return buildMenuActions[item.action]; });
            var nukeItems = items.filter(function (item) { return item.action === "nuke"; });
            if (mapMenuView === "root") {
                var radial = document.createElement("div");
                radial.className = "sow-hud__map-radial";
                var centerItem = items.find(function (item) { return item.action === "spawn" || item.action === "attack"; });
                if (centerItem) {
                    var center = mapActionButton(centerItem, "sow-hud__map-center " + (centerItem.action === "spawn" ? "is-spawn" : "is-attack"), false);
                    center.querySelector(".sow-hud__map-action-title").textContent = "⚔";
                    if (centerItem.action === "spawn") center.querySelector(".sow-hud__map-action-title").textContent = "🌱";
                    radial.appendChild(center);
                } else {
                    var disabledCenter = mapActionButton({ action: "attack", disabled: true }, "sow-hud__map-center is-attack", false);
                    disabledCenter.querySelector(".sow-hud__map-action-title").textContent = "⚔";
                    radial.appendChild(disabledCenter);
                }
                var transfer = items.find(function (item) { return item.action === "transfer"; });
                var fleet = items.find(function (item) { return item.action === "fleet"; });
                var alliance = items.find(function (item) { return item.action === "alliance"; });
                var radialCount = 4;
                if (transfer) mapSector(radial, transfer, 0, radialCount, "transfer");
                else mapDisabledSector(radial, 0, radialCount, "transfer", "⚖️", mapLabel("transfer"));
                if (fleet) mapSector(radial, fleet, 1, radialCount, "fleet");
                else mapDisabledSector(radial, 1, radialCount, "fleet", "⛵", mapLabel("fleet"));
                if (alliance) mapSector(radial, alliance, 2, radialCount, "alliance");
                else mapDisabledSector(radial, 2, radialCount, "alliance", "🤝", mapLabel("alliance"));
                if (buildItems.length) {
                    var buildIcon = buildItems.some(function (item) {
                        return item.action === "build_warship" || item.action === "build_trade_ship";
                    }) ? "⚓" : "🔧";
                    mapGroupSector(radial, buildItems, 3, radialCount, "build", buildIcon, "Build");
                } else if (nukeItems.length === 1) {
                    mapSector(radial, nukeItems[0], 3, radialCount, "nuke");
                } else if (nukeItems.length > 1) {
                    mapGroupSector(radial, nukeItems, 3, radialCount, "nuke", "🚀", mapLabel("nuke"));
                } else {
                    mapDisabledSector(radial, 3, radialCount, "build", "🔧", "Build");
                }
                menu.appendChild(radial);
            } else {
                var panel = document.createElement("div");
                panel.className = "sow-hud__map-submenu";
                var header = document.createElement("div");
                header.className = "sow-hud__map-submenu-header";
                var heading = document.createElement("span");
                heading.className = "sow-hud__map-submenu-icon";
                heading.textContent = mapMenuView === "nuke" ? "🚀" : "🔧";
                heading.setAttribute("aria-hidden", "true");
                header.appendChild(heading);
                var back = document.createElement("button");
                back.type = "button";
                back.className = "sow-hud__map-back";
                back.dataset.mapBack = "true";
                back.textContent = "←";
                back.setAttribute("aria-label", "Back");
                header.appendChild(back);
                panel.appendChild(header);
                var submenuItems = mapMenuView === "nuke" ? nukeItems : buildItems;
                submenuItems.forEach(function (item) {
                    panel.appendChild(mapActionButton(item, "sow-hud__map-action sow-hud__map-card", true));
                });
                menu.appendChild(panel);
            }
            menu.dataset.renderKey = renderKey;
        }
        menu.dataset.session = String(mapMenu.session);
        menu.dataset.tileIdx = String(mapMenu.tile_idx);
        var x = Number(mapMenu.x || 0);
        var y = Number(mapMenu.y || 0);
        var halfWidth = menu.offsetWidth * 0.5;
        var halfHeight = menu.offsetHeight * 0.5;
        var minX = halfWidth + 8;
        var minY = halfHeight + 8;
        var maxX = Math.max(minX, window.innerWidth - halfWidth - 8);
        var maxY = Math.max(minY, window.innerHeight - halfHeight - 8);
        menu.style.left = Math.min(Math.max(x, minX), maxX) + "px";
        menu.style.top = Math.min(Math.max(y, minY), maxY) + "px";
        return true;
    }


    function updateLeaderboard(players) {
        if (!hudRefs || !hudRefs.rows) return;
        if (Array.isArray(players)) {
            lastLeaderboardPlayers = players;
        } else {
            players = lastLeaderboardPlayers;
        }
        if (!Array.isArray(players)) return;
        var renderKey = players.map(function (player, idx) {
            return [idx, player.id, player.rank, player.name, player.troops, player.tile_count, player.territory_pct, player.is_alive, player.is_me].join("|");
        }).join("\u001e");
        if (renderKey === leaderboardRenderKey) return;
        var nextRows = Object.create(null);
        var fragment = document.createDocumentFragment();
        (players || []).forEach(function (player, idx) {
            var key = String(player.id);
            var row = leaderboardRows[key];
            if (!row) {
                var card = document.createElement("div");
                var left = document.createElement("div");
                var rank = document.createElement("span");
                var status = document.createElement("span");
                var name = document.createElement("b");
                var right = document.createElement("div");
                var territory = document.createElement("span");
                var troops = document.createElement("span");
                var transfer = document.createElement("button");
                card.className = "sow-hud__player-card";
                card.dataset.command = "focus_player";
                card.dataset.playerId = String(player.id);
                left.style.cssText = "display:flex;align-items:center;gap:6px;";
                right.style.cssText = "display:flex;gap:10px;font-weight:700;";
                transfer.type = "button";
                transfer.className = "sow-hud__row-action";
                transfer.dataset.command = "open_transfer";
                transfer.dataset.playerId = String(player.id);
                transfer.textContent = SOW_t("hud.gift");
                rank.style.cssText = "font-size:10px;color:var(--sow-muted);font-weight:800;";
                territory.style.color = "var(--sow-gold)";
                troops.style.color = "#86efac";
                left.appendChild(rank);
                left.appendChild(status);
                left.appendChild(name);
                right.appendChild(territory);
                right.appendChild(troops);
                right.appendChild(transfer);
                card.appendChild(left);
                card.appendChild(right);
                row = { card: card, rank: rank, status: status, name: name, territory: territory, troops: troops, key: "" };
            }

            var rowKey = [player.rank, idx, player.name, player.troops, player.tile_count, player.is_alive, player.is_me].join("|");
            if (row.key !== rowKey) {
                row.key = rowKey;
                row.card.classList.toggle("is-me", !!player.is_me);
                row.card.classList.toggle("is-dead", !player.is_alive);
                var displayRank = Number.isFinite(Number(player.rank)) ? Number(player.rank) : idx + 1;
                row.rank.textContent = "#" + displayRank;
                row.status.textContent = player.is_alive ? (displayRank === 1 ? "👑" : "🛡️") : "💀";
                row.name.textContent = player.name || SOW_t("hud.player_name");
                row.territory.textContent = Math.round((player.territory_pct || 0) * 100) + "%";
                row.troops.textContent = (player.troops > 1000 ? (player.troops / 1000).toFixed(1) + "k" : Math.floor(player.troops)) + " ⚔";
            }
            fragment.appendChild(row.card);
            nextRows[key] = row;
        });
        hudRefs.rows.replaceChildren(fragment);
        leaderboardRows = nextRows;
        leaderboardRenderKey = renderKey;
    }

    function appendPanelRows(container, rows) {
        if (!container || !Array.isArray(rows)) return;
        var fragment = document.createDocumentFragment();
        rows.forEach(function (row) { fragment.appendChild(row); });
        container.replaceChildren(fragment);
    }

    function panelRow(text) {
        var row = document.createElement("div");
        row.className = "sow-hud__panel-row";
        row.textContent = text;
        return row;
    }

    function renderInbox(requests) {
        if (!hudRefs || !Array.isArray(requests)) return;
        var renderKey = requests.map(function (request) {
            return [request.kind, request.requester_id, request.name, request.gold, request.troops, request.active].join("|");
        }).join("\u001e");
        if (renderKey === inboxRenderKey) return;
        var rows = requests.map(function (request) {
            var row = panelRow(request.kind === "resources"
                ? SOW_t("hud.resource_request", {
                    name: request.name || SOW_t("hud.player_name"),
                    gold: Math.floor(request.gold || 0),
                    troops: Math.floor(request.troops || 0)
                })
                : SOW_t("hud.alliance_request", { name: request.name || SOW_t("hud.player_name") }));
            var actions = document.createElement("span");
            actions.className = "sow-hud__panel-actions";
            [request.kind === "resources" ? "accept_resource_request" : "accept_alliance", request.kind === "resources" ? "reject_resource_request" : "reject_alliance"].forEach(function (command) {
                var button = document.createElement("button");
                button.type = "button";
                button.textContent = command.indexOf("reject") >= 0 ? SOW_t("hud.reject") : SOW_t("hud.accept");
                button.dataset.command = command;
                button.dataset.playerId = String(request.requester_id);
                actions.appendChild(button);
            });
            row.appendChild(actions);
            return row;
        });
        if (!rows.length) rows.push(panelRow(SOW_t("hud.no_pending_requests")));
        appendPanelRows(hudRefs.inboxRows, rows);
        inboxRenderKey = renderKey;
    }

    function renderNotifications(entries) {
        if (!hudRefs || !hudRefs.notifications || !Array.isArray(entries)) return;
        var visible = entries.slice(-3);
        var key = visible.map(function (entry) {
            return String(entry && entry.key || "") + JSON.stringify(entry && entry.values || {});
        }).join("\u001f");
        if (hudRefs.notifications.dataset.key === key) return;
        hudRefs.notifications.dataset.key = key;
        hudRefs.notifications.replaceChildren.apply(hudRefs.notifications, visible.map(function (entry) {
            var node = document.createElement("div");
            node.className = "sow-hud__notification";
            node.textContent = entry && entry.key ? SOW_t(entry.key, entry.values || {}) : SOW_t("hud.event");
            return node;
        }));
    }

    function renderHud() {
        if (!hudRoot) return;
        if (!hudState || hudState.phase !== "Playing" || !hudState.hud) {
            hudRoot.hidden = true;
            leaderboardOpen = false;
            inboxOpen = false;
            transferOpen = false;
            betrayalOpen = false;
            emojiPickerOpen = false;
            devSidebarOpen = false;
            hudSettingsOpen = false;

            hudRoot.dataset.overlayOpen = "false";
            if (hudRefs && hudRefs.mapMenu) hudRefs.mapMenu.classList.add("hidden");
            leaderboardRows = Object.create(null);
            leaderboardRenderKey = "";
            lastLeaderboardPlayers = [];
            inboxRenderKey = "";
            if (hudRefs && hudRefs.rows) hudRefs.rows.replaceChildren();
            if (hudRefs && hudRefs.notifications) {
                hudRefs.notifications.dataset.key = "";
                hudRefs.notifications.replaceChildren();
            }
            return;
        }
        ensureHudDom();
        hudRoot.hidden = false;

        var hud = hudState.hud;
        var devTools = hud.dev_tools || {};
        if (hudRefs.devBtn) {
            hudRefs.devBtn.classList.toggle("hidden", !devTools.available);
        }
        if (devTools.config && hudRefs.devSidebar) {
            hudRefs.devSidebar.querySelectorAll("[data-dev]").forEach(function (input) {
                if (document.activeElement === input) return;
                var value = devTools.config[input.dataset.dev];
                if (value != null) input.value = value;
            });
        }
        var gold = Math.floor(hud.gold || 0);
        var troops = Math.floor(hud.troops || 0);
        var maxTroops = Math.floor(hud.max_troops || 0);
        var prod = Math.floor(hud.troop_rate || 0);
        var currentRatio = hud.attack_ratio || 0.5;
        var spawnSecs = hud.spawn_timer_secs;
        var isDeploying = spawnSecs != null && spawnSecs > 0;

        if (hudRefs.gold && hudRefs.gold.dataset.val !== String(gold)) {
            hudRefs.gold.textContent = gold.toLocaleString();
            hudRefs.gold.dataset.val = String(gold);
        }

        if (hudRefs.troops) {
            var troopText = maxTroops > 0 ? troops.toLocaleString() + ' / ' + maxTroops.toLocaleString() + ' ⚔' : troops.toLocaleString() + ' ⚔';
            if (hudRefs.troops.dataset.val !== troopText) {
                hudRefs.troops.textContent = troopText;
                hudRefs.troops.dataset.val = troopText;
            }
        }

        if (hudRefs.troopFill) {
            var fillPct = maxTroops > 0 ? Math.min(100, Math.max(0, (troops / maxTroops) * 100)) : 0;
            var fillStr = fillPct.toFixed(1) + '%';
            if (hudRefs.troopFill.style.width !== fillStr) {
                hudRefs.troopFill.style.width = fillStr;
            }
        }

        if (hudRefs.prod && hudRefs.prod.dataset.val !== String(prod)) {
            hudRefs.prod.textContent = '⚔ +' + prod.toLocaleString() + '/s';
            hudRefs.prod.dataset.val = String(prod);
        }

        if (hudRefs.fps) {
            var hasFps = Number.isFinite(hud.fps) && hud.fps > 0;
            var hasPing = Number.isFinite(hud.ping);
            hudRefs.fps.textContent = hasFps && hasPing
                ? SOW_t("hud.fps_ping", { fps: hud.fps, ping: hud.ping })
                : SOW_t("hud.fps", { fps: hasFps ? hud.fps : "--" });
        }
        if (hudRefs.inboxCount) hudRefs.inboxCount.textContent = String(hud.inbox_count || 0);

        // Hover Card
        if (hudRefs.hoverCard) {
            var hov = hud.hovered;
            if (hov) {
                hudRefs.hoverCard.classList.remove("hidden");
                if (hudRefs.hoverName) hudRefs.hoverName.textContent = hov.name || SOW_t("hud.territory");
                if (hudRefs.hoverPct) hudRefs.hoverPct.textContent = Math.round((hov.territory_pct || 0) * 100) + "%";
                if (hudRefs.hoverTroops) hudRefs.hoverTroops.textContent = (hov.troops > 1000 ? (hov.troops / 1000).toFixed(1) + "k" : Math.floor(hov.troops || 0)) + " ⚔";
                if (hudRefs.hoverGold) hudRefs.hoverGold.textContent = Math.floor(hov.gold || 0).toLocaleString();
                if (hudRefs.hoverBlds) {
                    var bldText = [];
                    if (hov.cities > 0) bldText.push("🏛️ x" + hov.cities);
                    if (hov.factories > 0) bldText.push("🏭 x" + hov.factories);
                    if (hov.ports > 0) bldText.push("⚓ x" + hov.ports);
                    if (hov.bunkers > 0) bldText.push("🛡️ x" + hov.bunkers);
                    hudRefs.hoverBlds.textContent = bldText.join(" ");
                }
            } else {
                hudRefs.hoverCard.classList.add("hidden");
            }
        }

        // Left Rail Slider
        if (hudRefs.slider && document.activeElement !== hudRefs.slider) {
            hudRefs.slider.value = Math.round(currentRatio * 100);
        }

        // Bottom Dock: Phase Transformation
        if (hudRefs.deployBtn) {
            hudRefs.deployBtn.style.display = isDeploying ? "flex" : "none";
            if (isDeploying && hudRefs.deployTimer) {
                hudRefs.deployTimer.textContent = spawnSecs.toFixed(1) + 's';
            }
        }
        if (hudRefs.buildingsStrip) {
            hudRefs.buildingsStrip.style.display = isDeploying ? "none" : "flex";
            var selectedBuilding = hud.selected_building;
            var buildingCosts = hud.building_costs || {};
            hudRefs.buildingButtons.forEach(function (button) {
                var kind = button.dataset.kind || "";
                var cost = Number(buildingCosts[kind.toLowerCase()]);
                var hasCost = Number.isFinite(cost) && cost > 0;
                var affordable = !hasCost || gold >= cost;
                var selected = selectedBuilding === kind;
                button.disabled = !affordable && !selected;
                button.classList.toggle("active", selected);
                button.setAttribute("aria-pressed", String(selected));
                var costText = hasCost ? Math.floor(cost).toLocaleString() + "g" : "";
                var label = button.dataset.label || button.getAttribute("aria-label") || kind;
                button.dataset.label = label;
                button.setAttribute("aria-label", costText ? label + " " + costText : label);
                button.title = costText;
            });
        }

        // Emoji Popout
        if (hudRefs.emojiPopout) {
            hudRefs.emojiPopout.classList.toggle("hidden", !emojiPickerOpen);
        }
        if (hudRefs.devSidebar) {
            hudRefs.devSidebar.classList.toggle("hidden", !devSidebarOpen);
        }
        if (hudRefs.devBtn) {
            hudRefs.devBtn.classList.toggle("active", devSidebarOpen);
        }
        if (hudRefs.pinEmoji) {
            pinEmoji = Boolean(hud.pin_emoji);
            hudRefs.pinEmoji.classList.toggle("active", pinEmoji);
            hudRefs.pinEmoji.setAttribute("aria-pressed", String(pinEmoji));
        }

        if (hudRefs.settings) {
            hudRefs.settings.classList.toggle("hidden", !hudSettingsOpen);
            if (hudSettingsOpen && hudState.settings) {
                var settings = hudState.settings;
                var muteInput = hudRefs.settings.querySelector('[data-hud-setting="mute_all"]');
                var musicInput = hudRefs.settings.querySelector('[data-hud-setting="music_volume"]');
                var motionInput = hudRefs.settings.querySelector('[data-hud-setting="reduced_motion"]');
                if (muteInput && document.activeElement !== muteInput) muteInput.checked = !settings.mute_all;
                if (musicInput && document.activeElement !== musicInput) musicInput.value = settings.music_volume == null ? 0.8 : settings.music_volume;
                if (motionInput && document.activeElement !== motionInput) motionInput.checked = Boolean(settings.reduced_motion);
            }
        }

        if (hudRefs.inbox) {
            hudRefs.inbox.classList.toggle("hidden", !inboxOpen);
            if (inboxOpen) renderInbox(hud.inbox);
        }
        if (hudRefs.transfer) {
            transferOpen = Boolean(hud.transfer) || transferOpen;
            hudRefs.transfer.classList.toggle("hidden", !transferOpen || !hud.transfer);
            if (hud.transfer) {
                hudRefs.transfer.dataset.targetId = String(hud.transfer.target_id);
                if (hudRefs.transferTarget) hudRefs.transferTarget.textContent = SOW_t("hud.target_player", {
                    name: hud.transfer.target_name || SOW_t("hud.player_name")
                });
                if (hudRefs.transfer.dataset.suggestedTarget !== String(hud.transfer.target_id)) {
                    hudRefs.transfer.dataset.suggestedTarget = String(hud.transfer.target_id);
                    if (hudRefs.transferGold) hudRefs.transferGold.value = String(Math.floor(hud.transfer.suggested_gold || 0));
                    if (hudRefs.transferTroops) hudRefs.transferTroops.value = String(Math.floor(hud.transfer.suggested_troops || 0));
                }
            }
        }
        if (hudRefs.betrayal) {
            betrayalOpen = Boolean(hud.betrayal);
            hudRefs.betrayal.classList.toggle("hidden", !betrayalOpen);
            if (betrayalOpen && hudRefs.betrayalCopy) {
                hudRefs.betrayalCopy.textContent = SOW_t("hud.break_alliance_with", {
                    name: hud.betrayal.ally_name || SOW_t("hud.no_name")
                });
            }
        }

        renderNotifications(hud.notifications);

        // Leaderboard
        if (hudRefs.leaderboard) {
            hudRefs.leaderboard.classList.toggle("hidden", !leaderboardOpen);
        }
        if (leaderboardOpen) {
            updateLeaderboard(Array.isArray(hud.leaderboard) ? hud.leaderboard : lastLeaderboardPlayers);
        }

        // Surrender Modal
        if (hudRefs.surrender) {
            hudRefs.surrender.classList.toggle("hidden", !surrenderModalOpen);
            var tutorialActive = Boolean(hud.tutorial && hud.tutorial.active);
            var surrenderLeader = leaderById((hud && hud.player_leader) || (hudState && hudState.selected_leader));
            if (surrenderModalOpen && !tutorialActive && surrenderMessage == null) {
                var messages = surrenderMessageKeys.map(function (key) { return SOW_t(key); });
                surrenderMessage = messages.length ? messages[Math.floor(Math.random() * messages.length)] : "";
            }
            if (tutorialActive) surrenderMessage = null;
            if (hudRefs.surrenderBanner) hudRefs.surrenderBanner.textContent = tutorialActive
                ? SOW_t("endgame.leave_tutorial")
                : SOW_t("endgame.leave_match");
            if (hudRefs.surrenderDesc) hudRefs.surrenderDesc.textContent = tutorialActive
                ? SOW_t("endgame.tutorial_description")
                : (surrenderMessage || SOW_t("endgame.your_battle_will_end"));
            if (hudRefs.surrenderPortrait) {
                hudRefs.surrenderPortrait.src = asset("gameplay/avatars/" + surrenderLeader.slug + ".webp");
                hudRefs.surrenderPortrait.alt = surrenderLeader.name || SOW_t("hud.leader_avatar");
            }
            if (hudRefs.surrenderCancel) hudRefs.surrenderCancel.textContent = SOW_t("endgame.cancel");
            if (hudRefs.surrenderActionLabel) hudRefs.surrenderActionLabel.textContent = tutorialActive
                ? SOW_t("endgame.leave_tutorial")
                : SOW_t("endgame.leave_match");
        }

        // Endgame Screen
        var isOver = Boolean(hud.match_over);
        var isWinner = Boolean(hud.is_winner);
        if (hudRefs.endgame) {
            hudRefs.endgame.classList.toggle("hidden", !isOver);
            if (isOver) {
                var kda = hud.player_kda || {};
                var rewards = hud.rewards || {};
                var result = isWinner ? "victory" : "defeat";
                var kdaText = [kda.kills || 0, kda.deaths || 0, kda.assists || 0].join(" / ");
                if (hudRefs.endgameCard) hudRefs.endgameCard.dataset.result = result;
                if (hudRefs.endgameIcon) hudRefs.endgameIcon.textContent = isWinner ? "♛" : "⚔";
                if (hudRefs.endgameBanner) hudRefs.endgameBanner.textContent = isWinner ? SOW_t("hud.victory") : SOW_t("hud.defeat");
                if (hudRefs.endgameTitle) hudRefs.endgameTitle.textContent = isWinner ? SOW_t("hud.match_won") : SOW_t("hud.match_lost");
                if (hudRefs.endgameDesc) hudRefs.endgameDesc.textContent = isWinner
                    ? SOW_t("hud.map_control_secured")
                    : (hud.winner_name ? SOW_t("hud.winner", { name: hud.winner_name }) : SOW_t("hud.empire_eliminated"));
                var activeLeaderId = (hud && hud.player_leader) || (hudState && hudState.selected_leader);
                var activeLeader = leaderById(activeLeaderId);
                if (hudRefs.endgamePortrait) {
                    hudRefs.endgamePortrait.src = asset("gameplay/avatars/" + activeLeader.slug + ".webp");
                    hudRefs.endgamePortrait.alt = activeLeader.name || SOW_t("hud.leader_avatar");
                }
                if (hudRefs.endgameKda) hudRefs.endgameKda.textContent = kdaText;
                if (hudRefs.endgameXp) hudRefs.endgameXp.textContent = "+" + (rewards.xp || 0);
                if (hudRefs.endgameLeaderXp) hudRefs.endgameLeaderXp.textContent = "+" + (rewards.leader_xp || 0);
                if (hudRefs.endgameCrowns) hudRefs.endgameCrowns.textContent = "+" + (rewards.crowns || 0);
                if (hudRefs.endgameLaurels) hudRefs.endgameLaurels.textContent = "+" + (rewards.laurels || 0);
                var featuredSkin = window.SOW_PORTAL === "poki" ? null : hud.featured_skin;
                if (hudRefs.endgameStore) hudRefs.endgameStore.classList.toggle("hidden", !featuredSkin);
                if (featuredSkin) {
                    if (hudRefs.endgameStoreName) hudRefs.endgameStoreName.textContent = featuredSkin.name || SOW_t("store.featured_skin");
                    if (hudRefs.endgameStoreCopy) hudRefs.endgameStoreCopy.textContent = SOW_t("store.gems_count", { amount: featuredSkin.cost_gems || 0 }) + " · " + SOW_t("store.original_cosmetic");
                }
                if (hudRefs.endgameObserve) hudRefs.endgameObserve.classList.toggle("hidden", isWinner || Boolean(hud.winner_name));
            }
        }
        var mapMenuOpen = renderMapMenu(hud.map_menu);
        hudRoot.dataset.overlayOpen = String(Boolean(
            leaderboardOpen || inboxOpen || transferOpen || betrayalOpen ||
            surrenderModalOpen || emojiPickerOpen || isOver
            || devSidebarOpen || hudSettingsOpen || mapMenuOpen
        ));
    }

    if (hudRoot) {
        hudRoot.addEventListener("click", function (event) {
            var mapButton = event.target.closest("[data-map-action]");
            var mapGroup = event.target.closest("[data-map-group]");
            var mapBack = event.target.closest("[data-map-back]");
            if (mapButton || mapGroup || mapBack) {
                event.preventDefault();
                event.stopPropagation();
                var mapMenu = hudRefs && hudRefs.mapMenu;
                if (!mapMenu) return;
                if (mapBack) {
                    mapMenuView = "root";
                    renderMapMenu(hudState && hudState.hud && hudState.hud.map_menu);
                } else if (mapGroup) {
                    mapMenuView = mapGroup.dataset.mapGroup || "build";
                    renderMapMenu(hudState && hudState.hud && hudState.hud.map_menu);
                } else {
                    if (mapButton.disabled || mapButton.getAttribute("aria-disabled") === "true") return;
                    send("map_menu_action", {
                        session: Number(mapMenu.dataset.session),
                        tile_idx: Number(mapMenu.dataset.tileIdx),
                        action: mapButton.dataset.mapAction
                    });
                }
                return;
            }
            var btn = event.target.closest("[data-command]");
            if (!btn) return;
            var cmd = btn.dataset.command;
            if (cmd === "toggle_dev_sidebar") {
                devSidebarOpen = !devSidebarOpen;
                send("toggle_dev_sidebar");
                renderHud();
            } else if (cmd === "toggle_leaderboard") {
                leaderboardOpen = !leaderboardOpen;
                send("toggle_leaderboard");
                renderHud();
            } else if (cmd === "toggle_inbox") {
                inboxOpen = !inboxOpen;
                send("toggle_inbox");
                renderHud();
            } else if (cmd === "toggle_settings") {
                hudSettingsOpen = !hudSettingsOpen;
                renderHud();
            } else if (cmd === "reset_dev_config") {
                send("reset_dev_config");
            } else if (cmd === "toggle_pin_emoji") {
                pinEmoji = !pinEmoji;
                send("set_emoji_pinned", { pinned: pinEmoji });
                renderHud();
            } else if (cmd === "open_transfer") {
                if (leaderboardOpen) {
                    leaderboardOpen = false;
                    send("toggle_leaderboard");
                }
                transferOpen = true;
                send("open_transfer", { target_player_id: Number(btn.dataset.playerId) });
                renderHud();
            } else if (cmd === "close_transfer") {
                transferOpen = false;
                send("close_transfer");
                renderHud();
            } else if (cmd === "send_resources" || cmd === "request_resources") {
                var transfer = hudRefs && hudRefs.transfer;
                var targetId = transfer ? Number(transfer.dataset.targetId) : 0;
                var gold = hudRefs && hudRefs.transferGold ? Number(hudRefs.transferGold.value) : 0;
                var transferTroops = hudRefs && hudRefs.transferTroops ? Number(hudRefs.transferTroops.value) : 0;
                if (targetId > 0) send(cmd, { target_player_id: targetId, gold: gold, troops: transferTroops });
                transferOpen = false;
                renderHud();
            } else if (cmd === "accept_alliance" || cmd === "reject_alliance" || cmd === "accept_resource_request" || cmd === "reject_resource_request") {
                var requesterId = Number(btn.dataset.playerId);
                if (requesterId > 0) send(cmd, { target_player_id: requesterId });
            } else if (cmd === "cancel_betrayal") {
                betrayalOpen = false;
                send("cancel_betrayal");
                renderHud();
            } else if (cmd === "confirm_betrayal") {
                betrayalOpen = false;
                send("confirm_betrayal");
                renderHud();
            } else if (cmd === "cancel_attack" || cmd === "recall_fleet") {
                var id = Number(cmd === "cancel_attack" ? btn.dataset.attackId : btn.dataset.fleetId);
                if (id > 0) send(cmd, cmd === "cancel_attack" ? { attack_id: id } : { fleet_id: id });
            } else if (cmd === "prompt_surrender") {
                surrenderModalOpen = true;
                surrenderMessage = null;
                renderHud();
            } else if (cmd === "close_surrender_modal") {
                surrenderModalOpen = false;
                surrenderMessage = null;
                renderHud();
            } else if (cmd === "confirm_surrender") {
                surrenderModalOpen = false;
                surrenderMessage = null;
                send("return_to_menu");
                renderHud();
            } else if (cmd === "toggle_emoji") {
                emojiPickerOpen = !emojiPickerOpen;
                renderHud();

            } else if (cmd === "express_emoji") {
                var emoji = btn.dataset.emoji || "😀";
                send("express_emoji", { emoji: emoji, pinned: pinEmoji });
                emojiPickerOpen = false;
                renderHud();
            } else if (cmd === "zoom_in") {
                send("zoom_in");
            } else if (cmd === "zoom_out") {
                send("zoom_out");
            } else if (cmd === "center_camera") {
                send("center_camera");
            } else if (cmd === "focus_player") {
                var pid = parseInt(btn.dataset.playerId, 10);
                if (!isNaN(pid)) {
                    send("focus_player", { player_id: pid });
                }
            } else if (cmd === "spawn_troops") {
                send("spawn_troops");
            } else if (cmd === "select_building") {
                send("select_building", { kind: btn.dataset.kind });
            } else if (cmd === "confirm_endgame_leave") {
                send("return_to_menu");
            } else if (cmd === "open_store") {
                if (window.SOW_PORTAL === "poki") return;
                window.SOW_open_store_after_match = true;
                send("return_to_menu");
            } else if (cmd === "continue_observing") {
                send("continue_observing");
            }
        });
    }

    function handleHudStateUpdate(raw) {
        if (typeof raw !== "string" || raw === lastHudRaw) return;
        lastHudRaw = raw;
        var previousHud = hudState && hudState.phase === "Playing" ? hudState.hud : null;
        try {
            hudState = JSON.parse(raw);
        } catch (error) {
            console.warn("[WEB HUD] invalid state:", error);
            return;
        }
        if (hudState.phase === "Playing") {
            hudState.hud = hudState.hud || previousHud;
            if (hudState.hud && hudState.hud.dev_tools && typeof hudState.hud.dev_tools.open === "boolean") {
                devSidebarOpen = hudState.hud.dev_tools.open;
            }
        }
        renderHud();
        if (typeof window.SOW_tutorial_state_update === "function") {
            window.SOW_tutorial_state_update(hudState);
        }
    }

    window.SOW_onStateUpdate = function (raw) {
        if (typeof window.SOW_menu_state_update === "function") {
            window.SOW_menu_state_update(raw);
        }
        handleHudStateUpdate(raw);
    };

    window.addEventListener("sow:locale-change", function () {
        if (!hudState) return;
        hudInitialized = false;
        hudRefs = null;
        if (hudRoot) hudRoot.replaceChildren();
        renderHud();
    });

    if (hudRoot) hudRoot.hidden = true;
    window.SOW_onStateUpdate(window.SOW_MENU_STATE);
})();
