(function () {
    "use strict";

    // ─────────────────────────────────────────────────────────
    // HUD CONTROLLER (DOM Overlay for In-Game RTS Matches)
    // ─────────────────────────────────────────────────────────
    var hudRoot = document.getElementById("sow-hud");
    var hudState = null;
    var lastHudRaw = "";
    var activeHudPanel = null;
    var transferOpen = false;
    var betrayalOpen = false;
    var pinEmoji = false;
    var surrenderModalOpen = false;
    var surrenderMessage = null;
    var emojiPickerOpen = false;
    var devSidebarOpen = false;

    var hudInitialized = false;
    var hudRefs = null;
    var leaderboardRows = Object.create(null);
    var leaderboardRenderKey = "";
    var combatOperationRows = Object.create(null);
    var combatOperationsOrderKey = "";
    var pendingCounterAttacks = Object.create(null);
    var lastLeaderboardPlayers = [];
    var inboxRenderKey = "";
    var notificationCursor = 0;
    var notificationTimer = null;
    var activeNotifications = [];
    var notificationCards = [];
    var notificationLayoutFrame = 0;
    var notificationResizeObserver = null;
    var notificationMutationObserver = null;
    var notificationLayoutStarted = false;
    var notificationLayoutStory = null;
    var notificationLayoutObjective = null;
    var notificationLayoutNameplate = null;
    var notificationLayoutNotifications = null;
    var notificationLayoutState = "";
    var mapFeedbackCursor = 0;
    var mapFeedbackTimer = null;
    var activeMapFeedback = null;
    var mapFeedbackCard = null;
    var MAP_FEEDBACK_DURATION_MS = 2000;
    var MAP_FEEDBACK_FADE_OUT_MS = 180;
    var mapMenuView = "root";
    var mapMenuStateKey = "";
    var allocationHoverNone = window.matchMedia ? window.matchMedia("(hover: none)") : null;
    var lastAllocationKey = "";

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

    function setHudPanel(panel) {
        var previous = activeHudPanel;
        var next = previous === panel ? null : panel;
        if (previous === "leaderboard" && next !== previous) send("toggle_leaderboard");
        if (previous === "inbox" && next !== previous) send("toggle_inbox");
        activeHudPanel = next;
        if (next === "leaderboard") send("toggle_leaderboard");
        if (next === "inbox") send("toggle_inbox");
    }

    function asset(path) {
        var base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
        return base + "/" + path.split("/").map(encodeURIComponent).join("/");
    }

    function currencyAsset(kind) {
        return asset("gameplay/currency/" + kind + ".webp");
    }

    var HUD_ICONS = {
        city: "gameplay/icons/city_temple_cutout.webp",
        factory: "gameplay/icons/industry_forge_cutout.webp",
        port: "gameplay/icons/port_gate_1to1.webp",
        bunker: "gameplay/icons/defense_bunker_concrete_1to1.webp",
        farm: "gameplay/icons/battle_icon_cutout.webp",
        troops: "gameplay/icons/battle_icon_cutout.webp",
        alliance: "gameplay/icons/alliance_flag_1to1.webp",
        betray: "gameplay/icons/betray_dagger_1to1.webp",
        nuke: "gameplay/icons/silo_tower_1to1.webp",
        surrender: "gameplay/icons/surrender_flag_1to1.webp",
        warship: "gameplay/icons/port_galley_1to1.webp"
    };

    var BUILDING_EMOJIS = {
        city: "🏕️",
        factory: "🛠️",
        port: "⚓",
        bunker: "👁️",
        farm: "🌱"
    };

    var HUD_GLYPHS = {
        plus: '<path d="M12 4v16M4 12h16"/>',
        minus: '<path d="M4 12h16"/>',
        inbox: '<path d="M4 4h16v16H4zM4 13h4l2 3h4l2-3h4M4 7l8 6 8-6"/>',
        reaction: '<path d="M20 11.5a7.5 7.5 0 0 1-8 7.5 8.7 8.7 0 0 1-3.5-.7L4 20l1.4-3.6A7 7 0 0 1 4 12c0-4.1 3.6-7.5 8-7.5s8 3.1 8 7Z"/><path d="M9 11h.01M15 11h.01M9 14.5s1 1.5 3 1.5 3-1.5 3-1.5"/>',
        rankings: '<path d="M3 20h18M5 20v-6h4v6M10 20V4h4v16M15 20v-10h4v10"/>',
        settings: '<path d="m10 3-.6 2.2a7.6 7.6 0 0 0-1.7 1L5.5 5.4 3.8 8.3l1.8 1.5a7.9 7.9 0 0 0 0 2.1l-1.8 1.5 1.7 2.9 2.2-.8a7.6 7.6 0 0 0 1.7 1L10 19h4l.6-2.2a7.6 7.6 0 0 0 1.7-1l2.2.8 1.7-2.9-1.8-1.5a7.9 7.9 0 0 0 0-2.1l1.8-1.5-1.7-2.9-2.2.8a7.6 7.6 0 0 0-1.7-1L14 3h-4Z"/><circle cx="12" cy="11" r="3"/>',
        leave: '<path d="M13 4h6a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-6M10 16l4-4-4-4M14 12H3"/>',
        camera: '<circle cx="12" cy="12" r="7.5"/><circle cx="12" cy="12" r="2.5"/><path d="M12 2v2.5M12 19.5V22M2 12h2.5M19.5 12H22"/>',
        tools: '<path d="M14.5 6.5a5 5 0 0 0-6.7 6.7L3 18l3 3 4.8-4.8a5 5 0 0 0 6.7-6.7L14 13l-3-3 3.5-3.5Z"/>',
        spawn: '<path d="M5 21V4m1 1h12l-3 4 3 4H6"/>',
        attack: '<path d="m5 4 14 16M19 4 5 20M3 6l3-3 3 3M15 18l3 3 3-3M3 18l3 3 3-3M15 6l3-3 3 3"/>',
        fleet: '<path d="M3 15h18l-3 5H7l-4-5ZM6 12l6-9 6 9M12 3v12M2 21c2 1 3 1 5 0s3-1 5 0 3 1 5 0 3-1 5 0"/>',
        transfer: '<path d="M4 7h15l-3-3M20 17H5l3 3M19 7l-3 3M5 17l3-3"/>',
        alliance: '<path d="M8 12 5 9a3 3 0 0 1 4-4l3 3 3-3a3 3 0 0 1 4 4l-3 3M8 12l4 4 4-4M5 15l4 4M19 15l-4 4M12 8v8"/>',
        nuke: '<path d="M12 22V9M9 12l3-3 3 3M12 9l5-5M17 4h4M17 4V1M7 21h10M9 18h6"/>',
        upgrade: '<path d="m12 3 2.2 5.2L20 10l-4.4 3.5L17 19l-5-3-5 3 1.4-5.5L4 10l5.8-1.8L12 3Z"/>',
        anchor: '<path d="M12 3v12M8 7a4 4 0 1 0 8 0M4 13a8 8 0 0 0 16 0M2 13h4M18 13h4"/>',
        factory: '<path d="M3 21V9l6 3V8l6 4V5h6v16H3ZM17 8h1M17 11h1M7 17h2M12 17h2M17 17h1"/>',
        warship: '<path d="M3 15h18l-3 5H7l-4-5ZM7 12V7h10v5M10 7V4h4v3M2 21c2 1 3 1 5 0s3-1 5 0 3 1 5 0 3-1 5 0"/>',
        trade_ship: '<path d="M3 15h18l-3 5H7l-4-5ZM6 12l6-9 6 9M12 3v12M2 21c2 1 3 1 5 0s3-1 5 0 3 1 5 0 3-1 5 0M8 10h8"/>',
        farm: '<path d="M12 21V8M12 12c-4-1-6-3-6-6 4 0 6 2 6 6ZM12 15c4-1 6-3 6-6-4 0-6 2-6 6Z"/>',
        gold: '<circle cx="12" cy="12" r="8"/><path d="M14.5 8.5c-.6-.6-1.4-.9-2.5-.9-1.4 0-2.4.7-2.4 1.8 0 2.9 4.8 1.1 4.8 4 0 1.2-1 2-2.5 2-1 0-2-.4-2.7-1.1M12 6v12"/>',
        defense: '<path d="M12 22s8-4 8-11V5l-8-3-8 3v6c0 7 8 11 8 11Z"/>',
        range: '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="2"/><path d="M12 2v3M22 12h-3M12 22v-3M2 12h3"/>',
        speed: '<path d="m13 2-3 8h7l-6 12 1-9H5l8-11Z"/>',
        discount: '<circle cx="7" cy="7" r="2"/><circle cx="17" cy="17" r="2"/><path d="m19 5-14 14"/>',
        clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
        construction: '<path d="M14.5 6.5a5 5 0 0 0-6.7 6.7L3 18l3 3 4.8-4.8a5 5 0 0 0 6.7-6.7L14 13l-3-3 3.5-3.5Z"/>',
        lock: '<rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 1 1 8 0v3"/>'
    };

    function hudIcon(name, cls) {
        if (HUD_GLYPHS[name]) return '<svg class="' + (cls || "sow-hud__action-icon") + '" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' + HUD_GLYPHS[name] + '</svg>';
        if (!HUD_ICONS[name]) return "";
        return '<img class="' + (cls || "sow-hud__action-icon") + '" src="' + asset(HUD_ICONS[name]) + '" alt="" aria-hidden="true">';
    }

    function emojiIcon(emoji, cls) {
        return '<span class="' + cls + ' is-emoji" aria-hidden="true">' + emoji + '</span>';
    }

    function buildingIcon(kind, level) {
        var name = String(kind || "");
        var art = window.SOW_BUILDING_ART;
        var canonical = name.charAt(0).toUpperCase() + name.slice(1).toLowerCase();
        var atlas = art && art.sprites && art.sprites[canonical];
        var sprite = atlas && atlas[String(Number(level) || 1)];
        if (sprite && art.cell_size > 0 && art.atlas_width > art.cell_size && art.atlas_height > art.cell_size) {
            var backgroundWidth = (art.atlas_width * 100 / art.cell_size).toFixed(4);
            var backgroundHeight = (art.atlas_height * 100 / art.cell_size).toFixed(4);
            var positionX = (sprite.x * 100 / (art.atlas_width - art.cell_size)).toFixed(4);
            var positionY = (sprite.y * 100 / (art.atlas_height - art.cell_size)).toFixed(4);
            var style = 'background-image:url(&quot;' + escapeHudText(asset(art.atlas)) + '&quot;);'
                + 'background-size:' + backgroundWidth + '% ' + backgroundHeight + '%;'
                + 'background-position:' + positionX + '% ' + positionY + '%;';
            return '<span class="sow-hud__building-icon is-pixel-art" style="' + style + '" aria-hidden="true"></span>';
        }
        var emoji = BUILDING_EMOJIS[name.toLowerCase()];
        return emoji ? emojiIcon(emoji, "sow-hud__building-icon") : "";
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

    function formatNameplateTroops(value) {
        var count = Math.max(0, Math.floor(Number(value) || 0));
        if (count < 100000) return count.toLocaleString();
        var useMillions = count >= 999950;
        return (count / (useMillions ? 1000000 : 1000)).toFixed(1) + (useMillions ? "M" : "K");
    }

    function notificationAnchor(storyRoot, objective, nameplate, gap) {
        if (storyRoot && !storyRoot.hidden && objective && !objective.hidden
            && objective.offsetWidth > 0 && objective.offsetHeight > 0) {
            var offsetParent = objective.offsetParent || storyRoot;
            var parentRect = offsetParent.getBoundingClientRect();
            var left = parentRect.left + objective.offsetLeft;
            var top = parentRect.top + objective.offsetTop;
            return {
                source: "objective",
                left: left,
                right: left + objective.offsetWidth,
                top: top + objective.offsetHeight + gap
            };
        }
        if (nameplate && !nameplate.hidden) {
            var rect = nameplate.getBoundingClientRect();
            if (rect.width > 0 && rect.height > 0) {
                return { source: "nameplate", left: rect.left, right: rect.right, top: rect.bottom + gap };
            }
        }
        return { source: "safe", left: null, right: null, top: null };
    }

    function scheduleNotificationPlacement() {
        if (!hudRefs || !hudRefs.notifications) return;
        if (!window.requestAnimationFrame) { syncNotificationPlacement(); return; }
        if (notificationLayoutFrame) return;
        notificationLayoutFrame = window.requestAnimationFrame(function () {
            notificationLayoutFrame = 0;
            syncNotificationPlacement();
        });
    }

    function observeNotificationAnchors() {
        var story = document.getElementById("sow-story");
        var objective = story && story.querySelector(".sow-story__objective");
        var nameplate = hudRefs && hudRefs.nameplate;
        if (story === notificationLayoutStory && objective === notificationLayoutObjective
            && nameplate === notificationLayoutNameplate) return;
        if (objective !== notificationLayoutObjective && notificationResizeObserver) {
            if (notificationLayoutObjective) notificationResizeObserver.unobserve(notificationLayoutObjective);
            if (objective) notificationResizeObserver.observe(objective);
        }
        if (nameplate !== notificationLayoutNameplate && notificationResizeObserver) {
            if (notificationLayoutNameplate) notificationResizeObserver.unobserve(notificationLayoutNameplate);
            if (nameplate) notificationResizeObserver.observe(nameplate);
        }
        notificationLayoutStory = story;
        notificationLayoutObjective = objective;
        notificationLayoutNameplate = nameplate;
        if (notificationMutationObserver) {
            // MutationObserver disconnects all targets; rebind the current anchors together.
            notificationMutationObserver.disconnect();
            if (nameplate) notificationMutationObserver.observe(nameplate, { attributes: true, attributeFilter: ["hidden"] });
            if (story) notificationMutationObserver.observe(story, { childList: true, attributes: true, attributeFilter: ["hidden"] });
            if (objective) {
                notificationMutationObserver.observe(objective, notificationResizeObserver
                    ? { attributes: true, attributeFilter: ["hidden"] }
                    : { attributes: true, attributeFilter: ["hidden"], childList: true, subtree: true, characterData: true });
            }
        }
    }

    function observeNotificationLayout() {
        if (!hudRefs || !hudRefs.nameplate || !hudRefs.notifications) return;
        if (notificationLayoutNotifications !== hudRefs.notifications) {
            notificationLayoutNotifications = hudRefs.notifications;
            notificationLayoutState = "";
        }
        if (!notificationResizeObserver && typeof window.ResizeObserver === "function") {
            notificationResizeObserver = new window.ResizeObserver(function () {
                syncCompactHudBounds();
                syncNotificationPlacement();
            });
        }
        if (!notificationMutationObserver && typeof window.MutationObserver === "function") {
            notificationMutationObserver = new window.MutationObserver(function () {
                observeNotificationAnchors();
                scheduleNotificationPlacement();
            });
        }
        [hudRefs.topbar, hudRefs.nameplate, hudRefs.dock, hudRefs.dockStack, hudRefs.buildingCancel].forEach(function (element) {
            if (element && notificationResizeObserver) notificationResizeObserver.observe(element);
        });
        observeNotificationAnchors();
        syncCompactHudBounds();
        if (!notificationLayoutStarted) {
            notificationLayoutStarted = true;
            window.addEventListener("resize", function () {
                syncCompactHudBounds();
                scheduleNotificationPlacement();
            }, { passive: true });
            if (window.visualViewport) window.visualViewport.addEventListener("resize", function () {
                syncCompactHudBounds();
                scheduleNotificationPlacement();
            }, { passive: true });
            window.addEventListener("sow:locale-change", scheduleNotificationPlacement);
        }
        scheduleNotificationPlacement();
    }

    function syncNotificationPlacement() {
        if (!hudRefs || !hudRefs.notifications) return;
        var notifications = hudRefs.notifications;
        var story = document.getElementById("sow-story");
        var objective = story && story.querySelector(".sow-story__objective");
        var anchor = notificationAnchor(story, objective, hudRefs.nameplate, 12);
        var inlineStart = null;
        if (anchor.source !== "safe") {
            var direction = window.getComputedStyle ? window.getComputedStyle(hudRoot).direction : document.documentElement.dir;
            inlineStart = direction === "rtl" ? window.innerWidth - anchor.right : anchor.left;
            var fallbackWidth = Math.min(window.innerWidth <= 720 ? 300 : 360, Math.max(0, window.innerWidth - 32));
            var notificationWidth = notifications.offsetWidth || fallbackWidth;
            inlineStart = Math.max(8, Math.min(inlineStart, window.innerWidth - notificationWidth - 8));
        }
        var topValue = anchor.source === "safe" ? null : Math.round(Math.max(0, anchor.top));
        var inlineStartValue = inlineStart == null ? null : Math.round(inlineStart);
        var state = [anchor.source, topValue, inlineStartValue].join(":");
        if (state === notificationLayoutState) return;
        notificationLayoutState = state;
        if (topValue == null) notifications.style.removeProperty("--sow-hud-notifications-top");
        else notifications.style.setProperty("--sow-hud-notifications-top", topValue + "px");
        if (inlineStartValue == null) notifications.style.removeProperty("--sow-hud-notifications-inline-start");
        else notifications.style.setProperty("--sow-hud-notifications-inline-start", inlineStartValue + "px");
        notifications.classList.toggle("is-positioned", inlineStartValue != null);
        var hasVisibleNotification = notificationCards.some(function (parts) { return parts && parts.card && !parts.card.hidden; });
        if (hasVisibleNotification && !hudRoot.classList.contains("is-reduced-motion")) {
            notifications.classList.remove("is-reanchoring");
            void notifications.offsetWidth;
            notifications.classList.add("is-reanchoring");
        } else {
            notifications.classList.remove("is-reanchoring");
        }
    }

    function ensureHudDom() {
        if (!hudRoot || hudInitialized) return;
        hudInitialized = true;
        hudRoot.innerHTML = ''
            + '<header class="sow-hud__topbar">'
            + '  <div class="sow-hud__status-left">'
            + '    <button class="sow-hud__icon-pill hidden" type="button" data-command="toggle_dev_sidebar" id="sow-hud-dev-btn" aria-label="' + SOW_t("hud.dev_tools") + '" title="' + SOW_t("hud.dev_tools") + '">' + hudIcon("tools", "sow-hud__action-icon") + '</button>'
            + '  </div>'
            + '  <div class="sow-hud__status-right">'
            + '    <button class="sow-hud__icon-pill sow-hud__exit-pill" type="button" data-command="prompt_surrender" aria-label="' + SOW_t("hud.leave_match") + '" title="' + SOW_t("hud.leave_match") + '">' + hudIcon("leave", "sow-hud__action-icon") + '</button>'
            + '    <button class="sow-hud__icon-pill" type="button" data-command="toggle_settings" aria-label="' + SOW_t("menu.settings") + '" title="' + SOW_t("menu.settings") + '">' + hudIcon("settings", "sow-hud__action-icon") + '</button>'
            + '    <button class="sow-hud__icon-pill sow-hud__inbox-pill" type="button" data-command="toggle_inbox" aria-label="' + SOW_t("hud.inbox") + '" title="' + SOW_t("hud.inbox") + '">' + hudIcon("inbox") + '<span class="sow-hud__inbox-badge" id="sow-hud-inbox-count" hidden>0</span></button>'
            + '    <button class="sow-hud__icon-pill" type="button" data-command="toggle_leaderboard" aria-label="' + SOW_t("hud.rankings") + '" title="' + SOW_t("hud.rankings") + '">' + hudIcon("rankings", "sow-hud__action-icon") + '</button>'
            + '  </div>'
            + '</header>'
            + '<div class="sow-hud__nameplate" id="sow-hud-nameplate" hidden>'
            + '  <img class="sow-hud__nameplate-avatar" id="sow-hud-nameplate-avatar" alt="" draggable="false">'
            + '  <div class="sow-hud__nameplate-copy"><strong id="sow-hud-nameplate-name"></strong>'
            + '  <div class="sow-hud__nameplate-bar"><i id="sow-hud-nameplate-fill"></i><span class="sow-hud__nameplate-readout"><span id="sow-hud-nameplate-troops"></span>' + hudIcon("troops", "sow-hud__inline-icon") + '</span></div></div>'
            + '</div>'
            + '<div class="sow-hud__hover-card hidden" id="sow-hud-hover-card">'
            + '  <div class="sow-hud__hover-header"><span id="sow-hud-hover-avatar">👑</span> <b id="sow-hud-hover-name">' + SOW_t("hud.territory") + '</b></div>'
            + '  <div class="sow-hud__hover-stats">'
            + '    <span><b id="sow-hud-hover-pct">0%</b> ' + SOW_t("hud.land") + '</span>'
            + '    <span><b id="sow-hud-hover-troops">0</b> ' + hudIcon("troops", "sow-hud__inline-icon") + '</span>'
            + '    <span><b id="sow-hud-hover-gold">0</b> <img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' + currencyAsset("gold") + '" alt="" aria-hidden="true"></span>'
            + '  </div>'
            + '  <div class="sow-hud__hover-buildings" id="sow-hud-hover-blds"></div>'
            + '</div>'
            + '<aside class="sow-hud__left-rail" id="sow-hud-left-rail">'
            + '  <div class="sow-hud__rail-card">'
            + '    <div class="sow-hud__slider-vertical-wrap">'
            + '      <input type="range" min="5" max="100" value="50" step="5" class="sow-hud__range-vertical" id="sow-hud-slider" aria-label="' + SOW_t("hud.map_action_attack") + '">'
            + '    </div>'
            + '    <div class="sow-hud__army-allocation">'
            + '      <strong id="sow-hud-army-ratio" aria-hidden="true">50%</strong>'
            + '    </div>'
            + '  </div>'
            + '  <output class="sow-hud__allocation-detail" id="sow-hud-army-allocation" for="sow-hud-slider" aria-hidden="true"></output>'
            + '</aside>'
            + '<aside class="sow-hud__right-rail" id="sow-hud-right-rail">'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_in" aria-label="' + SOW_t("hud.zoom_in") + '" title="' + SOW_t("hud.zoom_in") + '">' + hudIcon("plus") + '</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_out" aria-label="' + SOW_t("hud.zoom_out") + '" title="' + SOW_t("hud.zoom_out") + '">' + hudIcon("minus") + '</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="center_camera" aria-label="' + SOW_t("hud.center_camera") + '" title="' + SOW_t("hud.center_camera") + '">' + hudIcon("camera", "sow-hud__action-icon") + '</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="toggle_emoji" aria-label="' + SOW_t("hud.emojis") + '" title="' + SOW_t("hud.emojis") + '">' + hudIcon("reaction") + '</button>'
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
            + '    <details class="sow-hud__dev-section" open><summary><b>' + SOW_t("hud.map_borders") + '</b></summary>'
            + '      <label class="sow-hud__dev-row">' + SOW_t("hud.border_thickness") + ' <input type="range" min="0" max="1" step="0.01" value="0.5" data-dev="thickness"></label>'
            + '      <label class="sow-hud__dev-row">' + SOW_t("hud.border_darkness") + ' <input type="range" min="0" max="1" step="0.01" value="0.5" data-dev="darkness"></label>'
            + '      <label class="sow-hud__dev-row">' + SOW_t("hud.shore_thickness") + ' <input type="range" min="0" max="1" step="0.01" value="0.5" data-dev="shore_thickness"></label>'
            + '      <label class="sow-hud__dev-row">Shore Drk <input type="range" min="0" max="1" step="0.01" value="1" data-dev="shore_darkness"></label>'
            + '      <label class="sow-hud__dev-row">' + SOW_t("hud.conquest_duration") + ' <input type="range" min="0.1" max="10" step="0.1" value="1.5" data-dev="conquest_duration"></label>'
            + '      <label class="sow-hud__dev-row">' + SOW_t("hud.opacity") + ' <input type="range" min="0" max="1" step="0.01" value="1" data-dev="territory_opacity"></label>'
            + '      <label class="sow-hud__dev-row">Blend Mode <select data-dev="blend_mode"><option value="0">Normal Mix</option><option value="1">Multiply</option><option value="2">Overlay</option><option value="3">All Albedo</option></select></label>'
            + '    </details>'
            + '    <details class="sow-hud__dev-section"><summary><b>Font Settings (SDF)</b></summary>'
            + '      <label class="sow-hud__dev-row">Font Size <input type="range" min="0.5" max="2.5" step="0.01" value="2" data-dev="font_size_scale"></label>'
            + '      <label class="sow-hud__dev-row">Face Dilate <input type="range" min="-1" max="2" step="0.01" value="-0.2" data-dev="font_face_dilate"></label>'
            + '      <label class="sow-hud__dev-row">Outline <input type="range" min="0" max="3" step="0.01" value="1.4" data-dev="font_outline_thickness"></label>'
            + '      <label class="sow-hud__dev-row">Shadow Y <input type="range" min="0" max="5" step="0.01" value="2" data-dev="font_shadow_y"></label>'
            + '      <label class="sow-hud__dev-row">Softness <input type="range" min="0" max="2" step="0.01" value="0.1" data-dev="font_underlay_softness"></label>'
            + '      <label class="sow-hud__dev-row">Spacing <input type="range" min="0.8" max="1.8" step="0.01" value="0.95" data-dev="font_char_spacing"></label>'
            + '    </details>'
            + '    <details class="sow-hud__dev-section"><summary><b>VFX Toggles (Benchmark)</b></summary>'
            + '      <div class="sow-hud__dev-row"><button type="button" class="sow-hud__dev-reset" data-command="dev_vfx_all" data-on="1">All On</button><button type="button" class="sow-hud__dev-reset" data-command="dev_vfx_all" data-on="0">All Off</button></div>'
            + '      <label class="sow-hud__dev-row">Conquer shockwave <input type="checkbox" data-dev="vfx_conquer"></label>'
            + '      <label class="sow-hud__dev-row">Border breathe <input type="checkbox" data-dev="vfx_border_breathe"></label>'
            + '      <label class="sow-hud__dev-row">Contested shimmer <input type="checkbox" data-dev="vfx_energy_flow"></label>'
            + '      <label class="sow-hud__dev-row">Territory heartbeat <input type="checkbox" data-dev="vfx_heartbeat"></label>'
            + '      <label class="sow-hud__dev-row">War fog / Frontier <input type="checkbox" data-dev="vfx_war_fog"></label>'
            + '      <label class="sow-hud__dev-row">Fog of War <input type="checkbox" data-dev="fog_of_war"></label>'
            + '      <label class="sow-hud__dev-row">Impact zone <input type="checkbox" data-dev="vfx_fallout"></label>'
            + '      <label class="sow-hud__dev-row">Ambient grading <input type="checkbox" data-dev="vfx_ambient_grade"></label>'
            + '      <label class="sow-hud__dev-row">Holographic grid <input type="checkbox" data-dev="vfx_holo_grid"></label>'
            + '      <label class="sow-hud__dev-row">Mover trails <input type="checkbox" data-dev="vfx_mover_trails"></label>'
            + '      <label class="sow-hud__dev-row">Click markers <input type="checkbox" data-dev="vfx_click_markers"></label>'
            + '      <label class="sow-hud__dev-row">Attack troop badges <input type="checkbox" data-dev="vfx_attack_badges"></label>'
            + '      <label class="sow-hud__dev-row">World buildings <input type="checkbox" data-dev="vfx_world_buildings"></label>'
            + '      <label class="sow-hud__dev-row">Bot avatars <input type="checkbox" data-dev="vfx_bot_avatars"></label>'
            + '      <label class="sow-hud__dev-row">Nameplate names <input type="checkbox" data-dev="vfx_nameplate_names"></label>'
            + '      <label class="sow-hud__dev-row">Nameplate troops <input type="checkbox" data-dev="vfx_nameplate_troops"></label>'
            + '    </details>'
            + '    <button type="button" class="sow-hud__dev-reset" data-command="reset_dev_config">' + SOW_t("hud.reset") + '</button>'
            + '  </div>'
            + '</aside>'
            + '<aside class="sow-hud__panel sow-hud__settings hidden" id="sow-hud-settings">'
            + '  <div class="sow-hud__panel-header"><h3>' + SOW_t("menu.settings") + '</h3><button class="sow-hud__close-btn" type="button" data-command="toggle_settings" aria-label="' + SOW_t("hud.close_settings") + '">✕</button></div>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.sound") + '</span><input type="checkbox" data-hud-setting="mute_all"></label>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.music") + '</span><input type="range" min="0" max="1" step="0.05" data-hud-setting="music_volume"></label>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.reduced_motion") + '</span><input type="checkbox" data-hud-setting="reduced_motion"></label>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.free_zoom_out") + '</span><input type="checkbox" data-hud-setting="free_zoom_out"></label>'
            + '  <label class="sow-hud__setting-row sow-hud__setting-row--hint" title="' + SOW_t("menu.sticky_building_mode_hint") + '"><span><b>' + SOW_t("menu.sticky_building_mode") + '</b><small>' + SOW_t("menu.sticky_building_mode_hint") + '</small></span><input type="checkbox" data-hud-setting="sticky_building_mode"></label>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.show_dev_tools") + '</span><input type="checkbox" data-hud-setting="show_dev_tools"></label>'
            + '</aside>'
            + '<div class="sow-hud__dock-stack" id="sow-hud-dock-stack">'
            + '<footer class="sow-hud__dock" id="sow-hud-dock">'
            + '  <div class="sow-hud__dock-inner" id="sow-hud-dock-inner">'
            + '    <div class="sow-hud__dock-actions">'
            + '      <div class="sow-hud__deploy-panel hidden" id="sow-hud-deploy-btn">'
            + '        <span class="sow-hud__deploy-title">' + SOW_t("hud.choose_spawn") + '</span>'
            + '        <b class="sow-hud__deploy-timer" id="sow-hud-deploy-timer">' + SOW_t("hud.ready") + '</b>'
            + '      </div>'
            + '      <div class="sow-hud__buildings-strip" id="sow-hud-buildings-strip">'
            + '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="City" aria-label="' + SOW_t("hud.map_action_city") + '">' + buildingIcon("City") + '</button>'
            + '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="Factory" aria-label="' + SOW_t("hud.map_action_factory") + '">' + buildingIcon("Factory") + '</button>'
            + '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="Port" aria-label="' + SOW_t("hud.map_action_port") + '">' + buildingIcon("Port") + '</button>'
            + '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="Bunker" aria-label="' + SOW_t("hud.map_action_bunker") + '">' + buildingIcon("Bunker") + '</button>'
            + '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="Farm" aria-label="Farm">' + buildingIcon("Farm") + '</button>'
            + '      </div>'
            + '      <div class="sow-hud__fleet-strip" id="sow-hud-fleet-strip">'
            + '        <button type="button" class="sow-hud__fleet-btn" data-fleet-kind="trade" disabled aria-label="' + SOW_t("hud.map_action_build_trade_ship") + '">' + hudIcon("trade_ship", "sow-hud__fleet-icon") + '<span><b data-role="trade-ships">0 / 0</b><small>' + SOW_t("hud.map_action_build_trade_ship") + '</small></span></button>'
            + '        <button type="button" class="sow-hud__fleet-btn" data-command="select_warship" data-fleet-kind="warship" aria-label="' + SOW_t("hud.map_action_build_warship") + '">' + hudIcon("warship", "sow-hud__fleet-icon") + '<span><b data-role="warships">0</b><small>' + SOW_t("hud.map_action_build_warship") + '</small></span></button>'
            + '        <button type="button" class="sow-hud__fleet-btn" data-command="select_nuke" data-fleet-kind="nuke" aria-label="' + SOW_t("hud.map_action_nuke") + '">' + hudIcon("nuke", "sow-hud__fleet-icon") + '<span><b data-role="nuke-status">' + SOW_t("lobbies.locked") + '</b><small>' + SOW_t("hud.map_action_nuke") + '</small></span></button>'
            + '      </div>'
            + '    </div>'
            + '    <div class="sow-hud__resource-row" id="sow-hud-resource-row">'
            + '      <div class="sow-hud__res-rate" id="sow-hud-res-rate" title="' + SOW_t("hud.troop_production_rate") + '">'
            + '        <span class="sow-hud__rate-text" data-role="prod">' + hudIcon("troops", "sow-hud__inline-icon") + ' +0/s</span>'
            + '      </div>'
            + '      <div class="sow-hud__res-bar-wrap" title="' + SOW_t("hud.troop_pool_capacity") + '">'
            + '        <div class="sow-hud__res-bar-fill" id="sow-hud-troop-fill" style="width: 0%;"></div>'
            + '        <span class="sow-hud__res-bar-text" data-role="troops">0 / 0 ' + hudIcon("troops", "sow-hud__inline-icon") + '</span>'
            + '      </div>'
            + '      <div class="sow-hud__res-gold" id="sow-hud-res-gold" title="' + SOW_t("hud.gold_net_rate") + '">'
            + '        <span class="sow-hud__gold-text"><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' + currencyAsset("gold") + '" alt="" aria-hidden="true"><b data-role="gold">0</b></span>'
            + '        <small class="sow-hud__gold-rate" data-role="gold-rate">+0/s</small>'
            + '      </div>'
            + '      <span class="sow-hud__fps" id="sow-hud-fps">' + SOW_t("hud.fps", { fps: "--" }) + '</span>'
            + '    </div>'
            + '  </div>'
            + '</footer>'
            + '<button type="button" class="sow-hud__building-cancel" data-command="cancel_building_mode" aria-label="' + SOW_t("endgame.cancel") + '" title="' + SOW_t("endgame.cancel") + '" hidden><span class="sow-hud__building-cancel-icon" aria-hidden="true">×</span><span>' + SOW_t("endgame.cancel") + '</span></button>'
            + '<aside class="sow-hud__operations" id="sow-hud-operations" hidden aria-label="' + SOW_t("hud.combat_operations") + '">'
            + '  <div class="sow-hud__operations-list" id="sow-hud-operations-list"></div>'
            + '</aside>'
            + '</div>'
            + '<aside class="sow-hud__panel sow-hud__leaderboard hidden" id="sow-hud-leaderboard">'
            + '  <div class="sow-hud__panel-header">'
            + '    <h3>' + hudIcon("rankings", "sow-hud__inline-icon") + ' ' + SOW_t("hud.rankings_title") + '</h3>'
            + '    <button class="sow-hud__close-btn" type="button" data-command="toggle_leaderboard" aria-label="' + SOW_t("hud.close_rankings") + '">✕</button>'
            + '  </div>'
            + '  <div class="sow-hud__leaderboard-rows" id="sow-hud-lb-rows"></div>'
            + '</aside>'
            + '<aside class="sow-hud__panel sow-hud__inbox hidden" id="sow-hud-inbox">'
            + '  <div class="sow-hud__panel-header"><h3>' + SOW_t("hud.inbox") + '</h3><button class="sow-hud__close-btn" type="button" data-command="toggle_inbox" aria-label="' + SOW_t("hud.close_inbox") + '">✕</button></div>'
            + '  <div class="sow-hud__panel-rows" id="sow-hud-inbox-rows"></div>'
            + '</aside>'
            + '<aside class="sow-hud__panel sow-hud__transfer hidden" id="sow-hud-transfer">'
            + '  <div class="sow-hud__transfer-header"><span class="sow-hud__transfer-icon" aria-hidden="true">' + hudIcon("transfer", "sow-hud__inline-icon") + '</span><h3>' + SOW_t("hud.resource_transfer") + '</h3><button class="sow-hud__close-btn" type="button" data-command="close_transfer" aria-label="' + SOW_t("hud.close_resource_transfer") + '">✕</button></div>'
            + '  <p class="sow-hud__transfer-target" id="sow-hud-transfer-target"></p>'
            + '  <div class="sow-hud__transfer-fields">'
            + '    <label><span>' + SOW_t("hud.troops") + '</span><input id="sow-hud-transfer-troops" type="number" min="0" step="1" value="0"></label>'
            + '    <label><span>' + SOW_t("hud.gold") + '</span><input id="sow-hud-transfer-gold" type="number" min="0" step="1" value="0"></label>'
            + '  </div>'
            + '  <div class="sow-hud__transfer-actions"><button class="sow-hud__transfer-send" type="button" data-command="send_resources"><span>' + SOW_t("hud.send") + '</span><span aria-hidden="true">↗</span></button><button class="sow-hud__transfer-request" type="button" data-command="request_resources">' + SOW_t("hud.request") + '</button></div>'
            + '</aside>'
            + '<div class="sow-hud__modal-backdrop hidden" id="sow-hud-betrayal-modal">'
            + '  <div class="sow-hud__modal-card"><h3>' + hudIcon("betray", "sow-hud__inline-icon") + ' ' + SOW_t("hud.break_alliance") + '</h3><p id="sow-hud-betrayal-copy">' + SOW_t("hud.break_alliance_body") + '</p><div class="sow-hud__panel-actions"><button type="button" data-command="cancel_betrayal">' + SOW_t("hud.keep_alliance") + '</button><button class="sow-hud__btn-danger" type="button" data-command="confirm_betrayal">' + SOW_t("hud.attack") + '</button></div></div>'
            + '</div>'
            + '<div class="sow-hud__endgame-backdrop hidden" id="sow-hud-surrender-modal">'
            + '  <section class="sow-hud__endgame-card sow-hud__exit-card" data-result="defeat" role="dialog" aria-modal="true" aria-labelledby="sow-hud-surrender-banner" aria-describedby="sow-hud-surrender-desc">'
            + '    <div class="sow-hud__endgame-hero">'
            + '      <div class="sow-hud__endgame-portrait-wrap">'
            + '        <img class="sow-hud__endgame-portrait" id="sow-hud-surrender-portrait" src="" alt="' + SOW_t("hud.leader_avatar") + '" />'
            + '      </div>'
            + '      <div class="sow-hud__endgame-hero-copy">'
            + '        <h2 class="sow-hud__endgame-banner" id="sow-hud-surrender-banner">' + hudIcon("surrender", "sow-hud__inline-icon") + ' ' + SOW_t("endgame.leave_match") + '</h2>'
            + '        <p class="sow-hud__endgame-desc" id="sow-hud-surrender-desc">' + SOW_t("endgame.your_battle_will_end") + '</p>'
            + '      </div>'
            + '    </div>'
            + '    <div class="sow-hud__endgame-stats" id="sow-hud-surrender-stats">'
            + '      <div class="sow-hud__endgame-stat"><span class="sow-hud__endgame-stat-icon" aria-hidden="true">' + hudIcon("troops", "sow-hud__inline-icon") + '</span><span class="sow-hud__endgame-stat-label">' + SOW_t("profile.kda") + '</span><b id="sow-hud-surrender-kda">0 / 0 / 0</b></div>'
            + '    </div>'
            + '    <p class="sow-hud__endgame-desc" id="sow-hud-surrender-save-note">' + SOW_t("endgame.leave_rewards_saved") + '</p>'
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
            + '<div class="sow-hud__map-menu hidden" id="sow-hud-map-menu" role="menu" aria-label="' + SOW_t("hud.map_actions") + '"></div>'
            + '<aside class="sow-hud__building-card hidden" id="sow-hud-building-card" aria-live="polite">'
            + '  <div class="sow-hud__building-card-head"><span class="sow-hud__building-card-title"><span class="sow-hud__building-card-icon" id="sow-hud-building-card-icon"></span><span id="sow-hud-building-card-kind"></span><span class="sow-hud__building-card-level" id="sow-hud-building-card-level"></span></span><button type="button" class="sow-hud__close-btn" data-command="close_building_card" aria-label="Close">✕</button></div>'
            + '  <div class="sow-hud__building-card-benefit" id="sow-hud-building-card-benefit"></div>'
            + '  <div class="sow-hud__building-card-next" id="sow-hud-building-card-next"></div>'
            + '  <div class="sow-hud__building-card-gold" id="sow-hud-building-card-gold" hidden></div>'
            + '  <button type="button" class="sow-hud__building-card-upgrade" id="sow-hud-building-card-upgrade" data-map-action="upgrade_structure"><span aria-hidden="true">↑</span><span id="sow-hud-building-card-upgrade-level"></span></button>'
            + '</aside>';

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
            goldRate: hudRoot.querySelector('[data-role="gold-rate"]'),
            goldWrap: document.getElementById("sow-hud-res-gold"),
            troops: hudRoot.querySelector('[data-role="troops"]'),
            prod: hudRoot.querySelector('[data-role="prod"]'),
            fps: document.getElementById("sow-hud-fps"),
            nameplate: document.getElementById("sow-hud-nameplate"),
            plateAvatar: document.getElementById("sow-hud-nameplate-avatar"),
            plateName: document.getElementById("sow-hud-nameplate-name"),
            plateFill: document.getElementById("sow-hud-nameplate-fill"),
            plateTroops: document.getElementById("sow-hud-nameplate-troops"),
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
            armyAllocation: document.getElementById("sow-hud-army-allocation"),
            armyRatio: document.getElementById("sow-hud-army-ratio"),
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
            fleetStrip: document.getElementById("sow-hud-fleet-strip"),
            fleetTrade: hudRoot.querySelector("[data-fleet-kind='trade']"),
            fleetWarship: hudRoot.querySelector("[data-fleet-kind='warship']"),
            fleetNuke: hudRoot.querySelector("[data-fleet-kind='nuke']"),
            fleetTradeCount: hudRoot.querySelector("[data-role='trade-ships']"),
            fleetWarshipCount: hudRoot.querySelector("[data-role='warships']"),
            fleetNukeStatus: hudRoot.querySelector("[data-role='nuke-status']"),
            buildingCancel: hudRoot.querySelector("[data-command='cancel_building_mode']"),
            panelButtons: Array.prototype.slice.call(hudRoot.querySelectorAll(".sow-hud__topbar [data-command^='toggle_'], .sow-hud__right-rail [data-command='toggle_emoji']")),
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
            surrenderStats: document.getElementById("sow-hud-surrender-stats"),
            surrenderKda: document.getElementById("sow-hud-surrender-kda"),
            surrenderSaveNote: document.getElementById("sow-hud-surrender-save-note"),
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
            endgameStore: document.getElementById("sow-hud-endgame-store"),
            endgameStoreName: document.getElementById("sow-hud-endgame-store-name"),
            endgameStoreCopy: document.getElementById("sow-hud-endgame-store-copy"),
            endgameStoreAction: document.getElementById("sow-hud-endgame-store-action"),
            endgameObserve: document.getElementById("sow-hud-endgame-observe"),
            mapMenu: document.getElementById("sow-hud-map-menu"),
            operations: document.getElementById("sow-hud-operations"),
            operationRows: document.getElementById("sow-hud-operations-list"),
            buildingCard: document.getElementById("sow-hud-building-card"),
            buildingCardIcon: document.getElementById("sow-hud-building-card-icon"),
            buildingCardKind: document.getElementById("sow-hud-building-card-kind"),
            buildingCardLevel: document.getElementById("sow-hud-building-card-level"),
            buildingCardBenefit: document.getElementById("sow-hud-building-card-benefit"),
            buildingCardNext: document.getElementById("sow-hud-building-card-next"),
            buildingCardGold: document.getElementById("sow-hud-building-card-gold"),
            buildingCardUpgrade: document.getElementById("sow-hud-building-card-upgrade"),
            buildingCardUpgradeLevel: document.getElementById("sow-hud-building-card-upgrade-level"),
            notifications: document.createElement("div"),
            topbar: hudRoot.querySelector(".sow-hud__topbar"),
            dock: document.getElementById("sow-hud-dock"),
            dockStack: document.getElementById("sow-hud-dock-stack"),
            mapFeedback: document.createElement("div")
        };


        hudRefs.notifications.className = "sow-hud__notifications";
        hudRefs.notifications.setAttribute("aria-live", "polite");
        hudRoot.appendChild(hudRefs.notifications);
        hudRefs.mapFeedback.className = "sow-hud__map-feedback";
        hudRefs.mapFeedback.setAttribute("aria-live", "polite");
        hudRefs.mapFeedback.hidden = true;
        hudRoot.appendChild(hudRefs.mapFeedback);
        notificationCards = Array.from({ length: 3 }, function () {
            return createNotificationCard(hudRefs.notifications, false);
        });
        mapFeedbackCard = createNotificationCard(hudRefs.mapFeedback, true);
        observeNotificationLayout();

        if (hudRefs.slider) hudRefs.slider.style.setProperty("--sow-crossed-swords", 'url("' + asset(HUD_ICONS.troops) + '")');
        if (hudRefs.slider) {
            var allocationRail = hudRefs.leftRail;
            var refreshAllocationDetail = function () {
                var currentHud = hudState && hudState.hud ? hudState.hud : {};
                var ratio = hudRefs.slider ? Number(hudRefs.slider.value) / 100 : 0.5;
                updateArmyAllocation(currentHud.troops, ratio, true);
            };
            hudRefs.slider.addEventListener("pointerdown", function () {
                if (allocationRail) allocationRail.classList.add("is-adjusting");
                refreshAllocationDetail();
            });
            hudRefs.slider.addEventListener("pointerup", function () {
                if (allocationRail) allocationRail.classList.remove("is-adjusting");
            });
            hudRefs.slider.addEventListener("pointercancel", function () {
                if (allocationRail) allocationRail.classList.remove("is-adjusting");
            });
            hudRefs.slider.addEventListener("keydown", function () {
                if (allocationRail) allocationRail.classList.add("is-adjusting");
            });
            hudRefs.slider.addEventListener("keyup", function () {
                if (allocationRail) allocationRail.classList.remove("is-adjusting");
            });
            hudRefs.slider.addEventListener("blur", function () {
                if (allocationRail) allocationRail.classList.remove("is-adjusting");
            });
            hudRefs.slider.addEventListener("input", function (e) {
                var val = parseFloat(e.target.value);
                var ratio = val / 100.0;
                var currentHud = hudState && hudState.hud ? hudState.hud : {};
                updateArmyAllocation(currentHud.troops, ratio);
                send("set_attack_ratio", { ratio: ratio });
            });
            if (allocationRail) {
                allocationRail.addEventListener("pointerenter", function (event) {
                    if (event.pointerType !== "touch") refreshAllocationDetail();
                });
                allocationRail.addEventListener("focusin", function () {
                    if (!allocationHoverNone || !allocationHoverNone.matches) refreshAllocationDetail();
                });
            }
        }
        hudRoot.addEventListener("input", function (event) {
            var input = event.target.closest("[data-dev]");
            if (!input) return;
            var value = input.type === "checkbox" ? (input.checked ? 1 : 0) : Number(input.value);
            send("set_dev_config", { field: input.dataset.dev, value: value });
        });
        hudRoot.addEventListener("change", function (event) {
            var input = event.target.closest("[data-dev]");
            if (!input || input.type !== "checkbox") return;
            send("set_dev_config", { field: input.dataset.dev, value: input.checked ? 1 : 0 });
        });
        hudRoot.addEventListener("change", function (event) {
            var input = event.target.closest("[data-hud-setting]");
            if (!input) return;
            var setting = input.dataset.hudSetting;
            if (setting === "mute_all") send("set_mute", { value: !input.checked });
            if (setting === "music_volume") send("set_music_volume", { value: Number(input.value) });
            if (setting === "reduced_motion") send("set_reduced_motion", { value: input.checked });
            if (setting === "free_zoom_out") send("set_free_zoom_out", { value: input.checked });
            if (setting === "sticky_building_mode") send("set_sticky_building_mode", { value: input.checked });
            if (setting === "show_dev_tools") send("set_show_dev_tools", { value: input.checked });
        });
    }

    var mapActionLabels = {
        spawn: { icon: "spawn", emoji: "🌱", key: "hud.map_action_deploy" },
        attack: { icon: "attack", emoji: "⚔️", key: "hud.map_action_attack" },
        fleet: { icon: "fleet", emoji: "⛵", key: "hud.map_action_fleet" },
        transfer: { icon: "transfer", emoji: "📦", key: "hud.map_action_transfer" },
        alliance: { icon: "alliance", emoji: "🤝", key: "hud.map_action_alliance" },
        build_city: { key: "hud.map_action_city" },
        build_factory: { key: "hud.map_action_factory" },
        build_port: { key: "hud.map_action_port" },
        build_bunker: { key: "hud.map_action_bunker" },
        build_farm: { text: "Farm" },
        upgrade_structure: { icon: "upgrade", text: "Upgrade" },
        nuke: { icon: "nuke", emoji: "🚀", key: "hud.map_action_nuke" },
        build_warship: { icon: "warship", key: "hud.map_action_build_warship" },
        build_trade_ship: { icon: "trade_ship", key: "hud.map_action_build_trade_ship" }
    };

    var allianceActionLabels = {
        request: { state: "request", key: "hud.map_action_request_alliance", emoji: "🤝", disabled: false },
        accept: { state: "accept", key: "hud.map_action_accept_alliance", emoji: "🤝", disabled: false },
        pending: { state: "pending", key: "hud.map_action_alliance_pending", emoji: "⏳", disabled: true },
        active: { state: "active", key: "hud.map_action_alliance_active", emoji: "🛡️", disabled: true },
        renew: { state: "renew", key: "hud.map_action_renew_alliance", emoji: "🔄", disabled: false }
    };

    var buildMenuActions = {
        build_city: true,
        build_factory: true,
        build_port: true,
        build_bunker: true,
        build_farm: true,
        upgrade_structure: true,
        build_warship: true,
        build_trade_ship: true
    };

    function mapLabel(action) {
        var label = mapActionLabels[action] || { icon: "•", text: action };
        return label.key ? SOW_t(label.key) : label.text;
    }

    function updateArmyAllocation(totalTroops, ratio, forceDetails) {
        if (!hudRefs) return;
        var troopCount = Math.max(0, Number(totalTroops) || 0);
        var total = Math.floor(troopCount);
        var share = Number(ratio);
        if (!Number.isFinite(share)) share = 0.5;
        share = Math.max(0, Math.min(1, share));
        var percent = Math.round(share * 100);
        if (hudRefs.armyRatio && hudRefs.armyRatio.textContent !== percent + "%") hudRefs.armyRatio.textContent = percent + "%";
        var fill = ((percent - 5) / 95 * 100) + "%";
        if (hudRefs.slider && hudRefs.slider.style.getPropertyValue("--sow-ratio") !== fill) hudRefs.slider.style.setProperty("--sow-ratio", fill);
        var allocationRail = hudRefs.leftRail;
        var detailsVisible = forceDetails || (allocationRail && (
            allocationRail.classList.contains("is-adjusting") || (
                !(allocationHoverNone && allocationHoverNone.matches) &&
                (allocationRail.matches(":hover") || allocationRail.contains(document.activeElement))
            )
        ));
        if (!detailsVisible) return;
        var allocationKey = total + ":" + share + ":" + String(window.SOW_LOCALE || "en");
        if (allocationKey === lastAllocationKey) return;
        lastAllocationKey = allocationKey;
        var sent = Math.min(total, Math.floor(troopCount * share));
        var kept = total - sent;
        var text = SOW_t("hud.attack_allocation", {
            send: sent.toLocaleString(),
            keep: kept.toLocaleString(),
            ratio: Math.round(share * 100)
        });
        if (hudRefs.armyAllocation && hudRefs.armyAllocation.textContent !== text) {
            hudRefs.armyAllocation.textContent = text;
        }
        if (hudRefs.slider && hudRefs.slider.getAttribute("aria-valuetext") !== text) {
            hudRefs.slider.setAttribute("aria-valuetext", text);
        }
    }

    function mapActionReason(item, availableGold) {
        if (item.reason_key) return SOW_t(item.reason_key);
        if (!item.disabled || item.cost == null) return "";
        return SOW_t("hud.not_enough_gold", {
            cost: Math.ceil(item.cost).toLocaleString(),
            available: Math.floor(Number(availableGold) || 0).toLocaleString()
        });
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
                cost: item && item.cost != null && Number.isFinite(Number(item.cost)) ? Number(item.cost) : null,
                level: item && Number.isFinite(Number(item.level)) ? Number(item.level) : null,
                disabled: Boolean(item && item.disabled),
                reason_key: item && item.reason_key || null
            };
        });
    }

    function mapActionContent(icon, label) {
        var content = document.createElement("span");
        content.className = "sow-hud__map-action-content";
        var title = document.createElement("span");
        title.className = "sow-hud__map-action-title";
        title.innerHTML = icon;
        content.appendChild(title);
        if (label) {
            var caption = document.createElement("span");
            caption.className = "sow-hud__map-sector-caption";
            caption.textContent = label;
            content.appendChild(caption);
        }
        return content;
    }

    function mapActionButton(item, className, withDetails, presentation) {
        var label = mapActionLabels[item.action] || { icon: "•", text: item.action };
        var button = document.createElement("button");
        var action = item.action;
        button.type = "button";
        button.className = className;
        button.dataset.mapAction = action;
        button.setAttribute("role", "menuitem");
        var disabled = Boolean(item.disabled || (presentation && presentation.disabled));
        button.disabled = disabled && !item.reason_key && !(item.disabled && item.cost != null && !(presentation && presentation.disabled));
        button.setAttribute("aria-disabled", String(disabled));
        var buildingKind = action.indexOf("build_") === 0 ? action.slice(6) : "";
        buildingKind = buildingKind.charAt(0).toUpperCase() + buildingKind.slice(1);
        var buildingImg = buildingKind ? buildingIcon(buildingKind) : "";
        var icon = presentation && presentation.emoji
            ? emojiIcon(presentation.emoji, "sow-hud__map-action-icon")
            : label.emoji
                ? emojiIcon(label.emoji, "sow-hud__map-action-icon")
                : buildingImg || (label.icon ? hudIcon(label.icon, "sow-hud__map-action-icon") : "•");
        var actionName = presentation ? SOW_t(presentation.key) : mapLabel(action);
        button.dataset.mapActionLabel = actionName;
        if (item.reason_key) button.dataset.reasonKey = item.reason_key;
        if (presentation && presentation.state) button.dataset.allianceState = presentation.state;
        var reason = mapActionReason(item, hudState && hudState.hud && hudState.hud.gold);
        var accessibleLabel = reason ? actionName + ". " + reason : actionName;
        button.setAttribute("aria-label", accessibleLabel);
        button.title = reason ? accessibleLabel : "";
        button.appendChild(mapActionContent(icon, withDetails ? "" : actionName));
        if (withDetails) {
            var copy = document.createElement("span");
            var name = document.createElement("span");
            name.className = "sow-hud__map-action-label";
            name.textContent = actionName;
            copy.appendChild(name);
            if (reason) {
                copy.appendChild(document.createElement("br"));
                var reasonLabel = document.createElement("small");
                reasonLabel.className = "sow-hud__map-action-reason";
                reasonLabel.textContent = reason;
                copy.appendChild(reasonLabel);
            }
            button.appendChild(copy);
        }
        if (withDetails && (item.cost != null || item.level != null)) {
            var details = document.createElement("small");
            details.className = "sow-hud__map-action-meta";
            var parts = [];
            if (item.level != null) parts.push(item.level + "→" + (item.level + 1));
            if (item.cost != null) parts.push(Math.ceil(item.cost) + "g");
            details.textContent = parts.join(" · ");
            button.appendChild(details);
        }
        if (item.cost != null) button.dataset.mapCost = String(item.cost);
        return button;
    }

    function updateDisabledMapActionReasons(menu, availableGold) {
        menu.querySelectorAll('.sow-hud__map-card[aria-disabled="true"]').forEach(function (button) {
            var cost = Number(button.dataset.mapCost);
            var reason = mapActionReason({
                disabled: true,
                cost: Number.isFinite(cost) ? cost : null,
                reason_key: button.dataset.reasonKey || null
            }, availableGold);
            if (!reason) return;
            var reasonLabel = button.querySelector(".sow-hud__map-action-reason");
            if (reasonLabel && reasonLabel.textContent !== reason) reasonLabel.textContent = reason;
            var accessibleLabel = button.dataset.mapActionLabel + ". " + reason;
            if (button.getAttribute("aria-label") !== accessibleLabel) {
                button.setAttribute("aria-label", accessibleLabel);
            }
            if (button.title !== accessibleLabel) button.title = accessibleLabel;
        });
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
        var anchorRadius = (outerRadius + innerRadius) / 2;
        var anchorX = 50 + Math.cos(midpointRadians) * anchorRadius;
        var anchorY = 50 + Math.sin(midpointRadians) * anchorRadius;
        button.dataset.tutorialAnchorX = String(anchorX);
        button.dataset.tutorialAnchorY = String(anchorY);
        var content = button.querySelector(".sow-hud__map-action-content");
        if (content) {
            content.style.left = anchorX + "%";
            content.style.top = anchorY + "%";
            content.style.transform = "translate(-50%, -50%)";
        }
    }

    function mapSector(radial, item, index, count, kind, presentation) {
        var button = mapActionButton(item, "sow-hud__map-sector sow-hud__map-sector--" + kind, false, presentation);
        radialSectorGeometry(button, index, count);
        radial.appendChild(button);
    }

    function mapDisabledSector(radial, index, count, kind, icon, label, presentation) {
        var button = document.createElement("button");
        button.type = "button";
        button.className = "sow-hud__map-sector sow-hud__map-sector--" + kind;
        button.setAttribute("role", "menuitem");
        var actionName = presentation ? SOW_t(presentation.key) : label;
        button.setAttribute("aria-label", actionName);
        button.setAttribute("aria-disabled", "true");
        button.disabled = true;
        button.title = actionName;
        if (presentation && presentation.state) button.dataset.allianceState = presentation.state;
        button.appendChild(mapActionContent(
            presentation ? emojiIcon(presentation.emoji, "sow-hud__map-action-icon") : icon,
            actionName
        ));
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
        button.appendChild(mapActionContent(icon, text));
        radialSectorGeometry(button, index, count);
        radial.appendChild(button);
    }

    function renderMapMenu(mapMenu) {
        if (!hudRefs || !hudRefs.mapMenu) return false;
        var menu = hudRefs.mapMenu;
        var open = Boolean(mapMenu && mapMenu.open);
        var showRadial = Boolean(open && mapMenu.view === "radial");
        menu.classList.toggle("hidden", !showRadial);
        if (!open) {
            menu.dataset.renderKey = "";
            mapMenuStateKey = "";
            mapMenuView = "root";
            return false;
        }
        menu.dataset.session = String(mapMenu.session);
        menu.dataset.tileIdx = String(mapMenu.tile_idx);
        if (mapMenu.view === "building_details") {
            if (menu.dataset.renderKey !== "building_details") menu.replaceChildren();
            menu.dataset.renderKey = "building_details";
            mapMenuStateKey = "";
            mapMenuView = "root";
            return !(mapMenu.building && mapMenu.building.under_construction);
        }
        if (!showRadial) return false;
        var items = mapItems(mapMenu);
        var stateKey = String(window.SOW_LOCALE || "en") + ":" + String(mapMenu.session) + ":" + String(mapMenu.tile_idx) + ":" + String(showRadial) + ":" + String(mapMenu.alliance_state || "") + ":" + JSON.stringify(items);
        if (stateKey !== mapMenuStateKey) {
            mapMenuStateKey = stateKey;
            mapMenuView = "root";
        }
        var renderKey = stateKey + ":" + mapMenuView;
        if (menu.dataset.renderKey !== renderKey) {
            menu.replaceChildren();
            menu.classList.toggle("is-submenu", mapMenuView !== "root");
            var buildItems = items.filter(function (item) { return buildMenuActions[item.action]; });
            var nukeItem = items.find(function (item) { return item.action === "nuke"; });
            if (mapMenuView === "root") {
                var radial = document.createElement("div");
                radial.className = "sow-hud__map-radial";
                var centerItem = items.find(function (item) { return item.action === "spawn" || item.action === "attack"; });
                if (centerItem) {
                    var center = mapActionButton(centerItem, "sow-hud__map-center " + (centerItem.action === "spawn" ? "is-spawn" : "is-attack"), false);
                    radial.appendChild(center);
                } else {
                    var disabledCenter = mapActionButton({ action: "attack", disabled: true }, "sow-hud__map-center is-attack", false);
                    disabledCenter.disabled = true;
                    disabledCenter.removeAttribute("data-map-action");
                    disabledCenter.setAttribute("aria-label", mapLabel("attack"));
                    disabledCenter.title = mapLabel("attack");
                    radial.appendChild(disabledCenter);
                }
                var transfer = items.find(function (item) { return item.action === "transfer"; });
                var fleet = items.find(function (item) { return item.action === "fleet"; });
                var alliance = items.find(function (item) { return item.action === "alliance"; });
                var alliancePresentation = allianceActionLabels[mapMenu.alliance_state] || null;
                var radialCount = 4;
                if (transfer) mapSector(radial, transfer, 0, radialCount, "transfer");
                else mapDisabledSector(radial, 0, radialCount, "transfer", emojiIcon("📦", "sow-hud__map-action-icon"), mapLabel("transfer"));
                if (fleet) mapSector(radial, fleet, 1, radialCount, "fleet");
                else mapDisabledSector(radial, 1, radialCount, "fleet", emojiIcon("⛵", "sow-hud__map-action-icon"), mapLabel("fleet"));
                if (alliance) mapSector(radial, alliance, 2, radialCount, "alliance", alliancePresentation);
                else mapDisabledSector(radial, 2, radialCount, "alliance", emojiIcon("🤝", "sow-hud__map-action-icon"), mapLabel("alliance"), alliancePresentation);
                if (buildItems.length) {
                    var buildIcon = emojiIcon("🏗️", "sow-hud__map-action-icon");
                    mapGroupSector(radial, buildItems, 3, radialCount, "build", buildIcon, SOW_t("hud.map_action_build"));
                } else if (nukeItem) {
                    mapSector(radial, nukeItem, 3, radialCount, "nuke");
                } else {
                    mapDisabledSector(radial, 3, radialCount, "build", emojiIcon("🏗️", "sow-hud__map-action-icon"), SOW_t("hud.map_action_build"));
                }
                menu.appendChild(radial);
            } else {
                var panel = document.createElement("div");
                panel.className = "sow-hud__map-submenu";
                var header = document.createElement("div");
                header.className = "sow-hud__map-submenu-header";
                var heading = document.createElement("span");
                heading.className = "sow-hud__map-submenu-icon";
                heading.innerHTML = emojiIcon("🏗️", "sow-hud__map-action-icon");
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
                buildItems.forEach(function (item) {
                    panel.appendChild(mapActionButton(item, "sow-hud__map-action sow-hud__map-card", true));
                });
                menu.appendChild(panel);
            }
            menu.dataset.renderKey = renderKey;
        }
        updateDisabledMapActionReasons(menu, hudState && hudState.hud && hudState.hud.gold);
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

    function escapeHudText(value) {
        return String(value).replace(/[&<>"']/g, function (character) {
            return { "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;", "'": "&#39;" }[character];
        });
    }

    function buildingMetricMarkup(metrics) {
        if (!Array.isArray(metrics)) return "";
        return metrics.map(function (metric) {
            var icon = String(metric.icon || "upgrade");
            var label = String(metric.label || "");
            var rawValue = metric.value;
            var value = typeof rawValue === "number" && Number.isFinite(rawValue)
                ? rawValue.toLocaleString(undefined, { maximumFractionDigits: 2 })
                : String(rawValue == null ? "" : rawValue);
            value = String(metric.prefix || "") + value + String(metric.unit || "");
            var safeLabel = escapeHudText(label);
            var safeValue = escapeHudText(value);
            return '<span class="sow-hud__building-metric" title="' + safeLabel + '" aria-label="' + safeLabel + ': ' + safeValue + '">'
                + hudIcon(icon, "sow-hud__building-metric-icon") + '<b>' + safeValue + '</b></span>';
        }).join("");
    }

    function buildingMetricDeltaMarkup(currentMetrics, nextMetrics) {
        if (!Array.isArray(nextMetrics)) return "";
        var currentByKey = Object.create(null);
        (Array.isArray(currentMetrics) ? currentMetrics : []).forEach(function (metric) {
            currentByKey[String(metric.icon || "upgrade") + "|" + String(metric.label || "")] = metric;
        });
        var deltas = [];
        nextMetrics.forEach(function (metric) {
            var previous = currentByKey[String(metric.icon || "upgrade") + "|" + String(metric.label || "")];
            var before = previous && Number(previous.value);
            var after = Number(metric.value);
            if (previous && Number.isFinite(before) && Number.isFinite(after)) {
                var difference = after - before;
                if (Math.abs(difference) < 0.000001) return;
                deltas.push(Object.assign({}, metric, {
                    value: Math.abs(difference),
                    prefix: difference < 0 ? "−" : (metric.prefix || "+")
                }));
            } else if (!previous) {
                deltas.push(Object.assign({}, metric, {
                    prefix: metric.prefix || (Number.isFinite(after) ? "+" : "")
                }));
            } else if (String(previous.value) !== String(metric.value)) {
                deltas.push(metric);
            }
        });
        return buildingMetricMarkup(deltas);
    }

    function positionBuildingCard(mapMenu) {
        var card = hudRefs.buildingCard;
        var width = card.offsetWidth;
        var height = card.offsetHeight;
        var anchorX = Number(mapMenu.x) || 0;
        var anchorY = Number(mapMenu.y) || 0;
        var gap = 14;
        var side = anchorX + gap + width <= window.innerWidth - 8 ? "right" : "left";
        var left = side === "right" ? anchorX + gap : anchorX - width - gap;
        var top = anchorY - height * 0.5;
        left = Math.max(8, Math.min(left, window.innerWidth - width - 8));
        top = Math.max(8, Math.min(top, window.innerHeight - height - 8));
        card.dataset.anchorSide = side;
        card.style.left = left + "px";
        card.style.top = top + "px";
        card.style.bottom = "auto";
    }

    function renderBuildingCard(mapMenu, playerGold) {
        if (!hudRefs || !hudRefs.buildingCard) return;
        var detail = mapMenu && mapMenu.building;
        var open = Boolean(mapMenu && mapMenu.open && mapMenu.view === "building_details" && detail && !detail.under_construction);
        hudRefs.buildingCard.classList.toggle("hidden", !open);
        if (!open) return;
        hudRefs.buildingCard.dataset.session = String(mapMenu.session);
        hudRefs.buildingCard.dataset.tileIdx = String(mapMenu.tile_idx);
        hudRefs.buildingCardIcon.innerHTML = buildingIcon(detail.kind, detail.level);
        hudRefs.buildingCardKind.textContent = detail.name || detail.kind || "Building";
        hudRefs.buildingCardLevel.innerHTML = '<span class="sow-hud__building-level">Lv ' + escapeHudText(detail.level || 1) + '</span>';
        var activeMetrics = Array.isArray(detail.metrics) ? detail.metrics.slice() : [];
        if (detail.boat_slots) {
            activeMetrics.push({
                icon: "port",
                label: "Boats in use / capacity",
                value: String(detail.boat_slots.used) + "/" + String(detail.boat_slots.total)
            });
            activeMetrics.push({
                icon: "speed",
                label: "Combined boat speed",
                value: Number(detail.boat_slots.speed_percent) || 0,
                prefix: "+",
                unit: "%"
            });
        }
        hudRefs.buildingCardBenefit.innerHTML = buildingMetricMarkup(activeMetrics);
        hudRefs.buildingCardBenefit.setAttribute("aria-label", detail.benefit_label || "Building effects");
        hudRefs.buildingCardBenefit.hidden = !activeMetrics.length;
        var unmet = Array.isArray(detail.requirements) ? detail.requirements.filter(function (item) { return !item.met; }) : [];
        if (detail.next_level) {
            var nextMetrics = Array.isArray(detail.next_metrics) ? detail.next_metrics : [];
            var deltaMarkup = buildingMetricDeltaMarkup(activeMetrics, nextMetrics);
            var requirementLabel = unmet.length ? SOW_t(unmet[0].key || "hud.action_unavailable") : "";
            var requirementMarkup = unmet.length
                ? '<span class="sow-hud__building-requirement" title="' + escapeHudText(requirementLabel) + '" aria-label="' + escapeHudText(requirementLabel) + '">'
                    + hudIcon("lock", "sow-hud__building-metric-icon") + '</span>'
                : "";
            var duration = Number(detail.duration_seconds);
            var durationMarkup = Number.isFinite(duration)
                ? buildingMetricMarkup([{ icon: "clock", label: "Upgrade time", value: Math.max(0, duration), unit: "s" }])
                : "";
            hudRefs.buildingCardBenefit.hidden = true;
            hudRefs.buildingCardNext.innerHTML = deltaMarkup + durationMarkup + requirementMarkup;
            hudRefs.buildingCardNext.setAttribute("aria-label", detail.next_benefit_label || "Upgrade effects");
            hudRefs.buildingCardNext.hidden = !deltaMarkup && !durationMarkup && !requirementMarkup;
            var cost = Math.floor(Number(detail.cost) || 0).toLocaleString();
            var availableGold = Number(playerGold);
            if (!Number.isFinite(availableGold)) availableGold = 0;
            var goldText = Math.floor(availableGold).toLocaleString();
            var enoughGold = availableGold >= Number(detail.cost);
            hudRefs.buildingCardGold.innerHTML = '<span class="sow-hud__building-gold-balance">'
                + hudIcon("gold", "sow-hud__building-metric-icon") + '<b>' + escapeHudText(goldText) + '</b></span>'
                + '<span class="sow-hud__building-gold-arrow" aria-hidden="true">→</span>'
                + '<span class="sow-hud__building-gold-cost" title="Upgrade cost">'
                + hudIcon("gold", "sow-hud__building-metric-icon") + '<b>' + escapeHudText(cost) + '</b></span>';
            hudRefs.buildingCardGold.classList.toggle("is-short", !enoughGold);
            hudRefs.buildingCardGold.setAttribute("aria-label", "Gold: " + goldText + ". Upgrade cost: " + cost);
            hudRefs.buildingCardGold.hidden = false;
            var upgradeLevel = "Lv " + detail.next_level;
            if (hudRefs.buildingCardUpgradeLevel.textContent !== upgradeLevel) {
                hudRefs.buildingCardUpgradeLevel.textContent = upgradeLevel;
            }
            hudRefs.buildingCardUpgrade.setAttribute("aria-label", "Upgrade to level " + detail.next_level);
            hudRefs.buildingCardUpgrade.title = "Upgrade to level " + detail.next_level;
            hudRefs.buildingCardUpgrade.disabled = !detail.can_upgrade || !enoughGold;
            hudRefs.buildingCardUpgrade.hidden = false;
        } else {
            hudRefs.buildingCardGold.hidden = true;
            hudRefs.buildingCardBenefit.hidden = !activeMetrics.length;
            hudRefs.buildingCardBenefit.innerHTML = buildingMetricMarkup(activeMetrics);
            hudRefs.buildingCardNext.innerHTML = '<span class="sow-hud__building-level sow-hud__building-level--max" title="Maximum level">MAX</span>';
            hudRefs.buildingCardNext.hidden = false;
            hudRefs.buildingCardUpgrade.hidden = true;
        }
        positionBuildingCard(mapMenu);
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
                left.className = "sow-hud__player-identity";
                right.className = "sow-hud__player-stats";
                transfer.type = "button";
                transfer.className = "sow-hud__row-action";
                transfer.dataset.command = "open_transfer";
                transfer.dataset.playerId = String(player.id);
                transfer.textContent = SOW_t("hud.gift");
                rank.className = "sow-hud__player-rank";
                territory.className = "sow-hud__player-land";
                troops.className = "sow-hud__player-troops";
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

            var displayName = player.name || SOW_t("hud.player_name");
            var rowKey = [player.rank, idx, displayName, player.troops, player.tile_count, player.is_alive, player.is_me].join("|");
            if (row.key !== rowKey) {
                row.key = rowKey;
                row.card.classList.toggle("is-me", !!player.is_me);
                row.card.classList.toggle("is-dead", !player.is_alive);
                var displayRank = Number.isFinite(Number(player.rank)) ? Number(player.rank) : idx + 1;
                row.rank.textContent = "#" + displayRank;
                row.status.textContent = player.is_alive ? (displayRank === 1 ? "👑" : "🛡️") : "💀";
                row.name.textContent = displayName;
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

    function createNotificationCard(container, contextual) {
        var card = document.createElement("div");
        card.className = "sow-hud__notification sow-hud__notification--premium"
            + (contextual ? " sow-hud__notification--contextual" : "");
        card.hidden = true;
        var seal = document.createElement("span");
        seal.className = "sow-hud__notification-seal";
        seal.setAttribute("aria-hidden", "true");
        var portraits = document.createElement("span");
        portraits.className = "sow-hud__notification-portraits";
        var avatars = [];
        for (var index = 0; index < 2; index += 1) {
            var wrapper = document.createElement("span");
            wrapper.className = "sow-hud__notification-avatar";
            wrapper.hidden = true;
            var image = document.createElement("img");
            image.className = "sow-hud__notification-portrait";
            image.alt = "";
            image.draggable = false;
            image.hidden = true;
            image.onerror = function () {
                var fallback = asset("gameplay/avatars/null.webp");
                if (this.getAttribute("src") !== fallback) this.src = fallback;
            };
            var emblem = document.createElement("span");
            emblem.className = "sow-hud__notification-emblem";
            emblem.hidden = true;
            wrapper.append(image, emblem);
            portraits.appendChild(wrapper);
            avatars.push({ wrapper: wrapper, image: image, emblem: emblem });
        }
        var copy = document.createElement("span");
        copy.className = "sow-hud__notification-copy";
        card.append(seal, portraits, copy);
        container.appendChild(card);
        return { card: card, seal: seal, avatars: avatars, copy: copy, renderKey: "" };
    }

    function clearNotificationCard(parts) {
        parts.card.hidden = true;
        parts.renderKey = "";
        parts.seal.innerHTML = "";
        parts.copy.textContent = "";
        parts.avatars.forEach(function (avatar) {
            avatar.wrapper.hidden = true;
            avatar.wrapper.className = "sow-hud__notification-avatar";
            avatar.image.hidden = true;
            if (avatar.image.removeAttribute) avatar.image.removeAttribute("src");
            else avatar.image.src = "";
            avatar.emblem.hidden = true;
            avatar.emblem.textContent = "";
        });
    }

    function notificationIcon(key) {
        if (key.indexOf("nuke") >= 0) return hudIcon("nuke", "sow-hud__notification-icon");
        if (key.indexOf("betrayal") >= 0) return hudIcon("betray", "sow-hud__notification-icon");
        if (/attack|elimination/.test(key)) return hudIcon("attack", "sow-hud__notification-icon");
        if (key.indexOf("fleet") >= 0) return hudIcon("fleet", "sow-hud__notification-icon");
        if (key.indexOf("request") >= 0) return hudIcon("inbox", "sow-hud__notification-icon");
        if (key.indexOf("alliance") >= 0) return hudIcon("alliance", "sow-hud__notification-icon");
        if (key.indexOf("resource") >= 0) return hudIcon("transfer", "sow-hud__notification-icon");
        if (/build|building|gold/.test(key)) return hudIcon("tools", "sow-hud__notification-icon");
        return '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"></circle><path d="M12 7v10M7 12h10"></path></svg>';
    }

    function renderNotificationCard(parts, item, contextual) {
        var entry = item.entry || {};
        var key = String(entry.key || "hud.event");
        var avatars = contextual ? [] : (Array.isArray(entry.avatars) ? entry.avatars : []);
        var renderKey = String(entry.id) + ":" + key + ":" + JSON.stringify(entry.values || {})
            + ":" + JSON.stringify(avatars) + ":" + item.priority;
        parts.card.hidden = false;
        if (parts.renderKey === renderKey) return;
        parts.renderKey = renderKey;
        parts.card.className = "sow-hud__notification sow-hud__notification--premium"
            + (contextual ? " sow-hud__notification--contextual" : "")
            + (item.priority >= 4 ? " sow-hud__notification--urgent" : "")
            + (/resource_(received|sent)|ally_support/.test(key) ? " sow-hud__notification--support" : "");
        parts.seal.innerHTML = notificationIcon(key);
        parts.avatars.forEach(function (slot, index) {
            var presentation = avatars[index] || null;
            var kind = presentation && String(presentation.kind || "");
            slot.wrapper.hidden = !presentation;
            slot.wrapper.className = "sow-hud__notification-avatar"
                + (index > 0 && avatars[index - 1] ? " sow-hud__notification-avatar--overlap" : "");
            slot.image.hidden = true;
            if (slot.image.removeAttribute) slot.image.removeAttribute("src");
            else slot.image.src = "";
            slot.emblem.hidden = true;
            slot.emblem.textContent = "";
            if (!presentation) return;

            if (kind === "emblem" && typeof presentation.symbol === "string" && presentation.symbol) {
                slot.emblem.textContent = presentation.symbol;
                slot.emblem.hidden = false;
                return;
            }

            var slug = kind === "portrait" && /^[a-z][a-z0-9_]*$/.test(presentation.slug || "")
                ? presentation.slug
                : "null";
            var src = asset("gameplay/avatars/" + slug + ".webp");
            slot.image.hidden = false;
            if (slot.image.getAttribute("src") !== src) slot.image.src = src;
        });
        parts.copy.textContent = SOW_t(key, entry.values || {});
    }

    function positionMapFeedback(entry) {
        var width = Math.max(1, window.innerWidth || 1);
        var height = Math.max(1, window.innerHeight || 1);
        var rect = mapFeedbackCard.card.getBoundingClientRect
            ? mapFeedbackCard.card.getBoundingClientRect()
            : null;
        var cardWidth = Math.min(rect && rect.width ? rect.width : 320, width - 16);
        var cardHeight = rect && rect.height ? rect.height : 52;
        var dockReserve = 0;
        if (width <= 720 && window.getComputedStyle && hudRoot) {
            dockReserve = Number.parseFloat(window.getComputedStyle(hudRoot).getPropertyValue("--sow-dock-reserve")) || 128;
        }
        var x = Number(entry.x);
        var y = Number(entry.y);
        if (!Number.isFinite(x)) x = width * 0.5;
        if (!Number.isFinite(y)) y = height * 0.5;
        var halfWidth = cardWidth * 0.5;
        var maxLeft = Math.max(halfWidth + 8, width - halfWidth - 8);
        var left = Math.max(halfWidth + 8, Math.min(maxLeft, x));
        var safeTop = Math.min(72, Math.max(8, height - cardHeight - 8));
        var maxTop = Math.max(safeTop, height - cardHeight - dockReserve - 8);
        var top = y - cardHeight - 14;
        if (top < safeTop) top = y + 14;
        top = Math.max(safeTop, Math.min(maxTop, top));
        hudRefs.mapFeedback.style.left = left + "px";
        hudRefs.mapFeedback.style.top = top + "px";
    }

    function syncCompactHudBounds() {
        if (!hudRoot || !hudRefs || !window.matchMedia) return;
        if (!window.matchMedia("(orientation: landscape) and (max-height: 560px) and (hover: none) and (pointer: coarse)").matches) {
            ["--sow-hud-layout-top-reserve", "--sow-hud-layout-action-top", "--sow-hud-layout-dock-height", "--sow-hud-layout-bottom-edge", "--sow-dock-reserve"].forEach(function (property) {
                hudRoot.style.removeProperty(property);
            });
            return;
        }
        var top = 0;
        [hudRefs.topbar, hudRefs.nameplate, document.querySelector("#sow-story .sow-story__objective")].forEach(function (element) {
            if (!element || element.hidden || !element.getClientRects().length) return;
            top = Math.max(top, element.getBoundingClientRect().bottom);
        });
        hudRoot.style.setProperty("--sow-hud-layout-top-reserve", Math.ceil(top + 8) + "px");
        var actionTop = hudRefs.topbar && hudRefs.topbar.getClientRects().length
            ? Math.ceil(hudRefs.topbar.getBoundingClientRect().bottom + 8)
            : 56;
        hudRoot.style.setProperty("--sow-hud-layout-action-top", actionTop + "px");
        var dockHeight = hudRefs.dock ? Math.ceil(hudRefs.dock.getBoundingClientRect().height) : 0;
        if (hudRefs.buildingCancel && !hudRefs.buildingCancel.hidden) {
            dockHeight += Math.ceil(hudRefs.buildingCancel.getBoundingClientRect().height) + 6;
        }
        var stackHeight = hudRefs.dockStack ? Math.ceil(hudRefs.dockStack.getBoundingClientRect().height) : dockHeight;
        var safeBottom = Number.parseFloat(window.getComputedStyle(hudRoot).getPropertyValue("--sow-sab")) || 0;
        hudRoot.style.setProperty("--sow-hud-layout-dock-height", dockHeight + "px");
        hudRoot.style.setProperty("--sow-dock-reserve", (stackHeight + Math.max(8, safeBottom) + 8) + "px");
        var stackTop = hudRefs.dockStack ? hudRefs.dockStack.getBoundingClientRect().top : window.innerHeight;
        hudRoot.style.setProperty("--sow-hud-layout-bottom-edge", Math.floor(stackTop - 8) + "px");
        if (activeMapFeedback && hudRefs.mapFeedback && !hudRefs.mapFeedback.hidden) positionMapFeedback(activeMapFeedback.entry);
    }

    function clearMapFeedback() {
        if (mapFeedbackTimer) window.clearTimeout(mapFeedbackTimer);
        mapFeedbackTimer = null;
        activeMapFeedback = null;
        if (hudRefs && hudRefs.mapFeedback) {
            hudRefs.mapFeedback.hidden = true;
            hudRefs.mapFeedback.style.removeProperty("left");
            hudRefs.mapFeedback.style.removeProperty("top");
        }
        if (mapFeedbackCard) {
            mapFeedbackCard.card.classList.remove("sow-hud__map-feedback-card--visible");
            clearNotificationCard(mapFeedbackCard);
        }
    }

    function renderMapFeedback(entry) {
        if (!hudRefs || !hudRefs.mapFeedback || !mapFeedbackCard) return;
        if (!entry) {
            if (activeMapFeedback || mapFeedbackTimer) clearMapFeedback();
            return;
        }
        var id = Number(entry.id);
        if (!Number.isSafeInteger(id) || id <= mapFeedbackCursor) return;
        mapFeedbackCursor = id;
        var age = Math.max(0, Number(entry.age_ms) || 0);
        clearMapFeedback();
        if (age >= MAP_FEEDBACK_DURATION_MS) return;
        var now = Date.now();
        activeMapFeedback = {
            entry: entry,
            priority: 1,
            expiresAt: now + MAP_FEEDBACK_DURATION_MS - age
        };
        hudRefs.mapFeedback.hidden = false;
        renderNotificationCard(mapFeedbackCard, activeMapFeedback, true);
        positionMapFeedback(entry);
        mapFeedbackCard.card.classList.remove("sow-hud__map-feedback-card--visible");
        void mapFeedbackCard.card.offsetWidth;
        mapFeedbackCard.card.classList.add("sow-hud__map-feedback-card--visible");
        mapFeedbackTimer = window.setTimeout(function () {
            mapFeedbackCard.card.classList.remove("sow-hud__map-feedback-card--visible");
            mapFeedbackTimer = window.setTimeout(function () {
                mapFeedbackTimer = null;
                clearMapFeedback();
            }, MAP_FEEDBACK_FADE_OUT_MS);
        }, Math.max(0, activeMapFeedback.expiresAt - Date.now()));
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

    function renderNotifications(entries, forceRefresh) {
        if (!hudRefs || !hudRefs.notifications || !Array.isArray(entries)) return;
        var changed = Boolean(forceRefresh);
        var now = Date.now();
        var unexpired = activeNotifications.filter(function (item) { return item.expiresAt > now; });
        if (unexpired.length !== activeNotifications.length) changed = true;
        activeNotifications = unexpired;
        if (hudRoot) hudRoot.dataset.notificationsVisible = String(activeNotifications.length > 0);
        entries.forEach(function (entry) {
            var id = Number(entry && entry.id);
            if (!Number.isSafeInteger(id) || id <= notificationCursor) return;
            notificationCursor = id;
            var age = Math.max(0, Number(entry.age_ms) || 0);
            if (age >= 3600) return;
            var createdAt = Date.now();
            var item = {
                entry: entry,
                priority: Math.max(0, Number(entry.priority) || 0),
                group: entry.group ? String(entry.group) : "event:" + id,
                sumValues: Boolean(entry.sum_values),
                expiresAt: createdAt + 3600 - age,
                createdAt: createdAt,
                totals: null
            };
            var existing = activeNotifications.find(function (candidate) { return candidate.group === item.group; });
            if (existing) {
                if (item.sumValues) {
                    var merged = Object.assign({}, existing.entry.values || {});
                    var totals = existing.totals || Object.create(null);
                    ["gold", "troops", "count", "attackers"].forEach(function (key) {
                        var addedRaw = (entry.values || {})[key];
                        if (totals[key] == null && merged[key] != null) {
                            totals[key] = Number(String(merged[key]).replace(/,/g, "")) || 0;
                        }
                        if (addedRaw != null) {
                            var added = Number(String(addedRaw).replace(/,/g, ""));
                            if (Number.isFinite(added)) totals[key] = (totals[key] || 0) + added;
                        }
                        if (totals[key] != null) {
                            merged[key] = new Intl.NumberFormat(window.SOW_LOCALE || "en", { maximumFractionDigits: 0 }).format(totals[key]);
                        }
                    });
                    var mergedKey = entry.key;
                    if (item.group.indexOf("resource:") === 0) {
                        var direction = item.group.split(":")[1];
                        var hasGold = Number(totals.gold || 0) > 0;
                        var hasTroops = Number(totals.troops || 0) > 0;
                        var resourceSuffix = hasGold && hasTroops ? "both" : hasGold ? "gold" : "troops";
                        mergedKey = "hud.resource_" + direction + "_" + resourceSuffix;
                    }
                    item.entry = Object.assign({}, entry, { key: mergedKey, values: merged });
                    item.totals = totals;
                }
                existing.entry = item.entry;
                existing.priority = Math.max(existing.priority, item.priority);
                existing.expiresAt = item.expiresAt;
                existing.createdAt = item.createdAt;
                existing.sumValues = item.sumValues;
                existing.group = item.group;
                existing.totals = item.totals;
                changed = true;
                return;
            }
            if (activeNotifications.length < 3) {
                activeNotifications.push(item);
                changed = true;
                return;
            }
            var replaceIndex = 0;
            for (var i = 1; i < activeNotifications.length; i += 1) {
                var current = activeNotifications[i];
                var candidate = activeNotifications[replaceIndex];
                if (current.priority < candidate.priority || (current.priority === candidate.priority && current.createdAt < candidate.createdAt)) replaceIndex = i;
            }
            if (item.priority >= activeNotifications[replaceIndex].priority) {
                activeNotifications[replaceIndex] = item;
                changed = true;
            }
        });
        if (!changed) return;
        activeNotifications.sort(function (a, b) { return b.priority - a.priority || a.createdAt - b.createdAt; });
        notificationCards.forEach(function (parts, index) {
            var item = activeNotifications[index];
            if (!item) {
                clearNotificationCard(parts);
                return;
            }
            renderNotificationCard(parts, item, false);
        });
        if (notificationTimer) window.clearTimeout(notificationTimer);
        notificationTimer = null;
        if (activeNotifications.length) {
            var nextExpiry = Math.min.apply(Math, activeNotifications.map(function (item) { return item.expiresAt; }));
            notificationTimer = window.setTimeout(function () {
                notificationTimer = null;
                renderNotifications([], true);
            }, Math.max(0, nextExpiry - Date.now()));
        }
    }

    function createCombatOperationRow() {
        var element = document.createElement("article");
        var portrait = document.createElement("img");
        var glyph = document.createElement("span");
        var avatar = document.createElement("span");
        var copy = document.createElement("span");
        var name = document.createElement("b");
        var details = document.createElement("small");
        var troops = document.createElement("b");
        var actions = document.createElement("span");
        element.className = "sow-hud__operation";
        avatar.className = "sow-hud__operation-avatar";
        portrait.className = "sow-hud__operation-portrait";
        portrait.alt = "";
        portrait.draggable = false;
        portrait.loading = "lazy";
        portrait.onerror = function () {
            this.hidden = true;
            this.removeAttribute("src");
        };
        glyph.className = "sow-hud__operation-glyph";
        copy.className = "sow-hud__operation-copy";
        name.className = "sow-hud__operation-name";
        details.className = "sow-hud__operation-details";
        troops.className = "sow-hud__operation-troops";
        actions.className = "sow-hud__operation-actions";
        avatar.append(portrait, glyph);
        copy.append(name, details);
        element.append(avatar, copy, troops, actions);
        return { element: element, portrait: portrait, glyph: glyph, name: name, details: details, troops: troops, actions: actions, avatarSrc: "", actionKey: "" };
    }

    function operationAction(command, glyph, label, operation) {
        var button = document.createElement("button");
        button.type = "button";
        button.className = "sow-hud__operation-action";
        button.dataset.command = command;
        button.setAttribute("aria-label", label);
        button.title = label;
        button.innerHTML = hudIcon(glyph, "sow-hud__operation-action-icon");
        if (command === "focus_world") {
            button.dataset.focusX = String(operation.focus_x);
            button.dataset.focusY = String(operation.focus_y);
        } else if (command === "counter_attack") {
            button.dataset.playerId = String(operation.player_id);
            button.dataset.incomingTroops = String(operation.incoming_troops || operation.troops || 0);
        } else if (command === "cancel_attack") {
            button.dataset.attackId = String(operation.id);
        } else if (command === "recall_fleet") {
            button.dataset.fleetId = String(operation.id);
        }
        return button;
    }

    function renderCombatOperations(operations) {
        if (!hudRefs || !hudRefs.operations || !hudRefs.operationRows) return;
        operations = Array.isArray(operations) ? operations : [];
        var incomingTotals = Object.create(null);
        operations.forEach(function (operation) {
            if (operation.direction === "incoming" && !operation.retreating) {
                var playerId = String(operation.player_id);
                incomingTotals[playerId] = (incomingTotals[playerId] || 0) + Math.max(0, Number(operation.troops) || 0);
            }
        });
        Object.keys(pendingCounterAttacks).forEach(function (playerId) {
            if ((incomingTotals[playerId] || 0) < pendingCounterAttacks[playerId] - 0.5) {
                delete pendingCounterAttacks[playerId];
            }
        });

        var nextRows = Object.create(null);
        var nextKeys = [];
        operations.forEach(function (operation) {
            if (!operation || (operation.kind !== "attack" && operation.kind !== "fleet")) return;
            var key = operation.kind + ":" + operation.id;
            nextKeys.push(key);
            var row = combatOperationRows[key] || createCombatOperationRow();
            var incoming = operation.direction === "incoming";
            var outgoing = operation.direction === "outgoing";
            var neutral = Boolean(operation.neutral);
            var retreating = Boolean(operation.retreating);
            var playerId = String(operation.player_id);
            var name = neutral ? SOW_t("hud.land") : (operation.name || SOW_t("hud.player_name"));
            var troops = Math.max(0, Number(operation.troops) || 0);
            var details = operation.kind === "attack" && incoming
                ? SOW_t("hud.attack_incoming", { count: formatNameplateTroops(troops) })
                : SOW_t(operation.kind === "fleet" ? "hud.map_action_fleet" : "hud.map_action_attack");
            var avatarSlug = typeof operation.avatar === "string" && /^[a-z0-9_-]+$/i.test(operation.avatar)
                ? operation.avatar
                : "";
            var avatarSrc = avatarSlug ? asset("gameplay/avatars/" + avatarSlug + ".webp") : "";
            row.element.classList.toggle("is-incoming", incoming);
            row.element.classList.toggle("is-outgoing", outgoing);
            row.element.classList.toggle("is-retreating", retreating);
            row.element.classList.toggle("is-neutral", neutral);
            if (row.avatarSrc !== avatarSrc) {
                row.avatarSrc = avatarSrc;
                row.portrait.hidden = !avatarSrc;
                if (avatarSrc) row.portrait.src = avatarSrc;
                else row.portrait.removeAttribute("src");
            }
            if (row.name.textContent !== name) row.name.textContent = name;
            if (row.details.textContent !== details) row.details.textContent = details;
            var troopText = formatNameplateTroops(troops);
            if (row.troops.textContent !== troopText) row.troops.textContent = troopText;
            var canFocus = operation.focus_x != null && operation.focus_y != null
                && Number.isFinite(Number(operation.focus_x)) && Number.isFinite(Number(operation.focus_y));
            var canCancel = outgoing && !retreating;
            var canCounter = incoming && !retreating;
            var canRecall = outgoing && operation.kind === "fleet" && !retreating;
            var pending = canCounter && pendingCounterAttacks[playerId] != null;
            var actionKey = [canFocus, canCancel, canCounter, canRecall, pending].join(":");
            if (row.actionKey !== actionKey) {
                row.actions.replaceChildren();
                if (canFocus) row.actions.appendChild(operationAction("focus_world", "camera", SOW_t("hud.center_camera"), operation));
                if (canCounter) {
                    var counterOperation = Object.assign({}, operation, {
                        incoming_troops: incomingTotals[playerId]
                    });
                    var counterButton = operationAction("counter_attack", "attack", SOW_t("hud.map_action_attack"), counterOperation);
                    counterButton.disabled = pending;
                    row.actions.appendChild(counterButton);
                }
                if (canCancel) {
                    var cancelLabel = SOW_t("endgame.cancel");
                    row.actions.appendChild(operationAction("cancel_attack", "minus", cancelLabel, operation));
                }
                if (canRecall) {
                    row.actions.appendChild(operationAction("recall_fleet", "transfer", SOW_t("hud.recall_transport"), operation));
                }
                row.actionKey = actionKey;
            }
            row.actions.querySelectorAll('[data-command="focus_world"]').forEach(function (button) {
                button.dataset.focusX = String(operation.focus_x);
                button.dataset.focusY = String(operation.focus_y);
            });
            row.actions.querySelectorAll('[data-command="counter_attack"]').forEach(function (button) {
                button.dataset.incomingTroops = String(incomingTotals[playerId] || 0);
            });
            nextRows[key] = row;
        });

        var orderKey = nextKeys.join("\u001e");
        if (orderKey !== combatOperationsOrderKey) {
            var fragment = document.createDocumentFragment();
            nextKeys.forEach(function (key) { fragment.appendChild(nextRows[key].element); });
            hudRefs.operationRows.replaceChildren(fragment);
            combatOperationsOrderKey = orderKey;
        }
        combatOperationRows = nextRows;
        hudRefs.operations.hidden = nextKeys.length === 0;
    }

    function renderHud(forceNotifications) {
        if (!hudRoot) return;
        if (!hudState || hudState.phase !== "Playing" || !hudState.hud) {
            hudRoot.hidden = true;
            activeHudPanel = null;
            transferOpen = false;
            betrayalOpen = false;
            emojiPickerOpen = false;
            devSidebarOpen = false;

            hudRoot.dataset.overlayOpen = "false";
            hudRoot.dataset.notificationsVisible = "false";
            if (hudRefs && hudRefs.mapMenu) hudRefs.mapMenu.classList.add("hidden");
            leaderboardRows = Object.create(null);
            leaderboardRenderKey = "";
            lastLeaderboardPlayers = [];
            inboxRenderKey = "";
            if (hudRefs && hudRefs.rows) hudRefs.rows.replaceChildren();
            if (notificationTimer) window.clearTimeout(notificationTimer);
            notificationTimer = null;
            activeNotifications = [];
            combatOperationRows = Object.create(null);
            combatOperationsOrderKey = "";
            pendingCounterAttacks = Object.create(null);
            notificationCards.forEach(clearNotificationCard);
            notificationCursor = 0;
            mapFeedbackCursor = 0;
            clearMapFeedback();
            return;
        }
        ensureHudDom();
        hudRoot.hidden = false;

        var hud = hudState.hud;
        var settingsOpen = activeHudPanel === "settings";
        var inboxOpen = activeHudPanel === "inbox";
        var leaderboardOpen = activeHudPanel === "leaderboard";
        hudRoot.classList.toggle("is-reduced-motion", Boolean(hudState.settings && hudState.settings.reduced_motion));
        var devTools = hud.dev_tools || {};
        if (hudRefs.devBtn) {
            hudRefs.devBtn.classList.toggle("hidden", !devTools.available);
        }
        if (devTools.config && hudRefs.devSidebar) {
            hudRefs.devSidebar.querySelectorAll("[data-dev]").forEach(function (input) {
                if (document.activeElement === input) return;
                var value = devTools.config[input.dataset.dev];
                if (value == null) return;
                if (input.type === "checkbox") input.checked = Boolean(value);
                else input.value = value;
            });
        }
        var gold = Math.floor(hud.gold || 0);
        var goldRate = Number(hud.gold_rate) || 0;
        var troops = Math.floor(hud.troops || 0);
        var maxTroops = Math.floor(hud.max_troops || 0);
        var prod = Math.floor(hud.troop_rate || 0);
        var currentRatio = Number.isFinite(hud.attack_ratio) ? hud.attack_ratio : 0.5;
        var spawnSecs = hud.spawn_timer_secs;
        var isDeploying = spawnSecs != null && spawnSecs > 0;

        if (hudRefs.gold && hudRefs.gold.dataset.val !== String(gold)) {
            hudRefs.gold.textContent = gold.toLocaleString();
            hudRefs.gold.dataset.val = String(gold);
        }
        if (hudRefs.goldRate) {
            var goldRateText = (goldRate > 0 ? "+" : "") + goldRate.toLocaleString(undefined, { maximumFractionDigits: 1, minimumFractionDigits: 1 }) + "/s";
            if (hudRefs.goldRate.textContent !== goldRateText) hudRefs.goldRate.textContent = goldRateText;
            hudRefs.goldRate.classList.toggle("is-negative", goldRate < 0);
        }
        if (hudRefs.goldWrap) hudRefs.goldWrap.classList.toggle("is-in-debt", gold < 0);

        if (hudRefs.troops) {
            var troopText = maxTroops > 0 ? troops.toLocaleString() + ' / ' + maxTroops.toLocaleString() + ' ' + hudIcon("troops", "sow-hud__inline-icon") : troops.toLocaleString() + ' ' + hudIcon("troops", "sow-hud__inline-icon");
            if (hudRefs.troops.dataset.val !== troopText) {
                hudRefs.troops.innerHTML = troopText;
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

        var tutorialOn = Boolean(hud.tutorial && hud.tutorial.active);
        if (hudRefs.nameplate) {
            if (hudRefs.nameplate.hidden === tutorialOn) hudRefs.nameplate.hidden = !tutorialOn;
            if (tutorialOn) {
                var plateLeader = leaderById(hud.player_leader || (hudState && hudState.selected_leader));
                var plateSrc = asset("gameplay/avatars/" + plateLeader.slug + ".webp");
                if (hudRefs.plateAvatar && hudRefs.plateAvatar.getAttribute("src") !== plateSrc) {
                    hudRefs.plateAvatar.src = plateSrc;
                    hudRefs.plateAvatar.alt = plateLeader.name;
                }
                if (hudRefs.plateName && hudRefs.plateName.textContent !== plateLeader.name) {
                    hudRefs.plateName.textContent = plateLeader.name;
                }
                var plateText = formatNameplateTroops(troops) + " / " + formatNameplateTroops(maxTroops);
                if (hudRefs.plateTroops && hudRefs.plateTroops.textContent !== plateText) {
                    hudRefs.plateTroops.textContent = plateText;
                }
                var plateFillPct = maxTroops > 0 ? Math.min(100, Math.max(0, (troops / maxTroops) * 100)) : 0;
                var plateFillStr = plateFillPct.toFixed(1) + '%';
                if (hudRefs.plateFill && hudRefs.plateFill.style.width !== plateFillStr) {
                    hudRefs.plateFill.style.width = plateFillStr;
                }
            }
        }

        if (hudRefs.prod && hudRefs.prod.dataset.val !== String(prod)) {
            hudRefs.prod.innerHTML = hudIcon("troops", "sow-hud__inline-icon") + ' +' + prod.toLocaleString() + '/s';
            hudRefs.prod.dataset.val = String(prod);
        }

        if (hudRefs.fps) {
            var hasFps = Number.isFinite(hud.fps) && hud.fps > 0;
            var hasPing = Number.isFinite(hud.ping);
            hudRefs.fps.textContent = hasFps && hasPing
                ? SOW_t("hud.fps_ping", { fps: hud.fps, ping: hud.ping })
                : SOW_t("hud.fps", { fps: hasFps ? hud.fps : "--" });
        }
        if (hudRefs.inboxCount) {
            hudRefs.inboxCount.textContent = String(hud.inbox_count || 0);
            hudRefs.inboxCount.hidden = !hud.inbox_count;
        }

        // Hover Card
        if (hudRefs.hoverCard) {
            var hov = hud.hovered;
            if (hov) {
                hudRefs.hoverCard.classList.remove("hidden");
                if (hudRefs.hoverName) hudRefs.hoverName.textContent = hov.name || SOW_t("hud.territory");
                if (hudRefs.hoverPct) hudRefs.hoverPct.textContent = Math.round((hov.territory_pct || 0) * 100) + "%";
                if (hudRefs.hoverTroops) hudRefs.hoverTroops.textContent = (hov.troops > 1000 ? (hov.troops / 1000).toFixed(1) + "k" : Math.floor(hov.troops || 0));
                if (hudRefs.hoverGold) hudRefs.hoverGold.textContent = Math.floor(hov.gold || 0).toLocaleString();
                if (hudRefs.hoverBlds) {
                    var bldText = [];
                    if (hov.cities > 0) bldText.push(hudIcon("city", "sow-hud__inline-icon") + " x" + hov.cities);
                    if (hov.factories > 0) bldText.push(hudIcon("factory", "sow-hud__inline-icon") + " x" + hov.factories);
                    if (hov.ports > 0) bldText.push(hudIcon("port", "sow-hud__inline-icon") + " x" + hov.ports);
                    if (hov.bunkers > 0) bldText.push(hudIcon("bunker", "sow-hud__inline-icon") + " x" + hov.bunkers);
                    if (hov.farms > 0) bldText.push(hudIcon("farm", "sow-hud__inline-icon") + " x" + hov.farms);
                    hudRefs.hoverBlds.innerHTML = bldText.join(" ");
                }
            } else {
                hudRefs.hoverCard.classList.add("hidden");
            }
        }

        // Left Rail Slider
        if (hudRefs.slider && document.activeElement !== hudRefs.slider) {
            hudRefs.slider.value = Math.round(currentRatio * 100);
        }
        var displayRatio = hudRefs.slider && document.activeElement === hudRefs.slider
            ? Number(hudRefs.slider.value) / 100
            : currentRatio;
        updateArmyAllocation(hud.troops, displayRatio);

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
                button.disabled = false;
                var disabled = !affordable && !selected;
                button.setAttribute("aria-disabled", String(disabled));
                button.classList.toggle("active", selected);
                button.setAttribute("aria-pressed", String(selected));
                var costText = hasCost ? Math.floor(cost).toLocaleString() + "g" : "";
                var label = button.dataset.label || button.getAttribute("aria-label") || kind;
                button.dataset.label = label;
                var reason = mapActionReason({ disabled: !affordable, cost: cost }, gold);
                var accessibleLabel = reason
                    ? label + ". " + reason
                    : (costText ? label + " " + costText : label);
                button.setAttribute("aria-label", accessibleLabel);
                if (button.title !== accessibleLabel) button.title = accessibleLabel;
            });
        }
        if (hudRefs.fleetStrip) {
            hudRefs.fleetStrip.style.display = isDeploying ? "none" : "flex";
            var fleetPanel = hud.fleet_panel || {};
            var portLevels = Math.max(0, Number(fleetPanel.port_levels) || 0);
            var cityLevel = Math.max(0, Number(fleetPanel.city_level) || 0);
            var tradeRequiredPortLevel = Math.max(1, Number(fleetPanel.trade_required_port_level) || 1);
            var warshipRequiredPortLevel = Math.max(1, Number(fleetPanel.warship_required_port_level) || 1);
            var nukeRequiredCityLevel = Math.max(1, Number(fleetPanel.nuke_required_city_level) || 1);
            var tradeUnlocked = portLevels >= tradeRequiredPortLevel;
            var warshipUnlocked = portLevels >= warshipRequiredPortLevel;
            var nukeUnlocked = cityLevel >= nukeRequiredCityLevel;
            var militaryFull = Number(fleetPanel.military_used) >= Number(fleetPanel.military_capacity);
            var warshipSelected = Boolean(hud.selected_warship_build);
            if (hudRefs.fleetTrade) {
                hudRefs.fleetTrade.disabled = !tradeUnlocked;
                hudRefs.fleetTrade.setAttribute("aria-disabled", String(!tradeUnlocked));
                hudRefs.fleetTrade.title = tradeUnlocked
                    ? SOW_t("hud.map_action_build_trade_ship")
                    : SOW_t("lobbies.locked") + " · " + SOW_t("hud.map_action_port") + " " + tradeRequiredPortLevel;
                hudRefs.fleetTrade.classList.toggle("is-locked", !tradeUnlocked);
            }
            if (hudRefs.fleetTradeCount) {
                hudRefs.fleetTradeCount.textContent = Math.floor(Number(fleetPanel.trade_ships) || 0).toLocaleString()
                    + " / " + Math.floor(Number(fleetPanel.trade_capacity) || 0).toLocaleString();
            }
            if (hudRefs.fleetWarship) {
                var warshipCost = Math.max(0, Number(fleetPanel.warship_cost) || 0);
                var warshipDisabled = !warshipUnlocked || militaryFull || gold < warshipCost;
                hudRefs.fleetWarship.disabled = warshipDisabled && !warshipSelected;
                hudRefs.fleetWarship.setAttribute("aria-disabled", String(warshipDisabled));
                hudRefs.fleetWarship.classList.toggle("active", warshipSelected);
                hudRefs.fleetWarship.classList.toggle("is-locked", warshipDisabled && !warshipSelected);
                hudRefs.fleetWarship.title = !warshipUnlocked
                    ? SOW_t("lobbies.locked") + " · " + SOW_t("hud.map_action_port") + " " + warshipRequiredPortLevel
                    : militaryFull
                        ? SOW_t("hud.map_action_build_warship") + " · " + Math.floor(Number(fleetPanel.military_used) || 0) + "/" + Math.floor(Number(fleetPanel.military_capacity) || 0)
                        : SOW_t("hud.map_action_build_warship") + " · " + Math.floor(Number(fleetPanel.military_used) || 0) + "/" + Math.floor(Number(fleetPanel.military_capacity) || 0);
            }
            if (hudRefs.fleetWarshipCount) {
                hudRefs.fleetWarshipCount.textContent = Math.floor(Number(fleetPanel.warships) || 0).toLocaleString();
            }
            if (hudRefs.fleetNuke) {
                var cooldownTicks = Math.max(0, Number(fleetPanel.nuke_cooldown_ticks) || 0);
                var nukeReady = Boolean(fleetPanel.nuke_available) && cooldownTicks === 0;
                var nukeDisabled = !nukeUnlocked || !nukeReady;
                hudRefs.fleetNuke.disabled = nukeDisabled;
                hudRefs.fleetNuke.setAttribute("aria-disabled", String(nukeDisabled));
                hudRefs.fleetNuke.classList.toggle("is-locked", nukeDisabled);
                hudRefs.fleetNuke.classList.toggle("active", hud.selected_nuke === "AtomBomb");
                hudRefs.fleetNuke.title = !nukeUnlocked
                    ? SOW_t("lobbies.locked") + " · " + SOW_t("hud.map_action_city") + " " + nukeRequiredCityLevel
                    : cooldownTicks > 0
                        ? SOW_t("lobbies.locked") + " · " + Math.ceil(cooldownTicks / 10) + "s"
                        : SOW_t("hud.map_action_nuke");
                if (hudRefs.fleetNukeStatus) {
                    hudRefs.fleetNukeStatus.textContent = !nukeUnlocked
                        ? SOW_t("lobbies.locked")
                        : cooldownTicks > 0
                            ? Math.ceil(cooldownTicks / 10) + "s"
                            : SOW_t("hud.ready");
                }
            }
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
            hudRefs.settings.classList.toggle("hidden", !settingsOpen);
            if (settingsOpen && hudState.settings) {
                var settings = hudState.settings;
                var muteInput = hudRefs.settings.querySelector('[data-hud-setting="mute_all"]');
                var musicInput = hudRefs.settings.querySelector('[data-hud-setting="music_volume"]');
                var motionInput = hudRefs.settings.querySelector('[data-hud-setting="reduced_motion"]');
                var freeZoomInput = hudRefs.settings.querySelector('[data-hud-setting="free_zoom_out"]');
                var stickyBuildingInput = hudRefs.settings.querySelector('[data-hud-setting="sticky_building_mode"]');
                var devToolsInput = hudRefs.settings.querySelector('[data-hud-setting="show_dev_tools"]');
                if (muteInput && document.activeElement !== muteInput) muteInput.checked = !settings.mute_all;
                if (musicInput && document.activeElement !== musicInput) musicInput.value = settings.music_volume == null ? 0.8 : settings.music_volume;
                if (motionInput && document.activeElement !== motionInput) motionInput.checked = Boolean(settings.reduced_motion);
                if (freeZoomInput && document.activeElement !== freeZoomInput) freeZoomInput.checked = Boolean(settings.free_zoom_out);
                if (stickyBuildingInput && document.activeElement !== stickyBuildingInput) stickyBuildingInput.checked = Boolean(settings.sticky_building_mode);
                if (devToolsInput && document.activeElement !== devToolsInput) devToolsInput.checked = Boolean(settings.show_dev_tools);
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

        renderNotifications(hud.notifications, forceNotifications);
        renderCombatOperations(hud.operations);
        renderMapFeedback(hud.map_feedback);

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
            var surrenderKda = hud.player_kda || {};
            if (hudRefs.surrenderStats) hudRefs.surrenderStats.classList.toggle("hidden", tutorialActive);
            if (hudRefs.surrenderKda) hudRefs.surrenderKda.textContent = [surrenderKda.kills || 0, surrenderKda.deaths || 0, surrenderKda.assists || 0].join(" / ");
            if (hudRefs.surrenderSaveNote) {
                hudRefs.surrenderSaveNote.textContent = SOW_t("endgame.leave_rewards_saved");
                hudRefs.surrenderSaveNote.classList.toggle("hidden", tutorialActive);
            }
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
                var featuredSkin = (window.SOW_PORTAL === "poki" || window.SOW_PORTAL === "jest") ? null : hud.featured_skin;
                if (hudRefs.endgameStore) hudRefs.endgameStore.classList.toggle("hidden", !featuredSkin);
                if (featuredSkin) {
                    if (hudRefs.endgameStoreName) hudRefs.endgameStoreName.textContent = featuredSkin.name || SOW_t("store.featured_skin");
                    if (hudRefs.endgameStoreCopy) hudRefs.endgameStoreCopy.textContent = SOW_t("store.gems_count", { amount: featuredSkin.cost_gems || 0 }) + " · " + SOW_t("store.original_cosmetic");
                }
                if (hudRefs.endgameObserve) hudRefs.endgameObserve.classList.toggle("hidden", isWinner || Boolean(hud.winner_name));
            }
        }
        var mapMenuOpen = renderMapMenu(hud.map_menu);
        if (hudRefs.buildingCancel) hudRefs.buildingCancel.hidden = !hud.selected_building;
        renderBuildingCard(hud.map_menu, hud.gold);
        var panelStates = { toggle_leaderboard: leaderboardOpen, toggle_inbox: inboxOpen, toggle_settings: settingsOpen, toggle_dev_sidebar: devSidebarOpen, toggle_emoji: emojiPickerOpen };
        hudRefs.panelButtons.forEach(function (button) {
            var expanded = Boolean(panelStates[button.dataset.command]);
            button.classList.toggle("active", expanded);
            var value = String(expanded);
            if (button.getAttribute("aria-expanded") !== value) button.setAttribute("aria-expanded", value);
        });
        hudRoot.dataset.overlayOpen = String(Boolean(
            leaderboardOpen || inboxOpen || transferOpen || betrayalOpen ||
            surrenderModalOpen || emojiPickerOpen || isOver
            || devSidebarOpen || settingsOpen || mapMenuOpen
        ));
    }

    if (hudRoot) {
        var zoomHold = null;
        function stopZoomHold() {
            if (!zoomHold) return;
            window.clearInterval(zoomHold.repeat);
            zoomHold.button.classList.remove("is-holding", "is-zoom-limited");
            zoomHold = null;
        }
        function zoomAtLimit(command) {
            var zoom = hudState && hudState.hud && hudState.hud.camera_zoom_state;
            if (!zoom || !Number.isFinite(Number(zoom.current))) return false;
            return command === "zoom_out"
                ? Number(zoom.current) <= Number(zoom.floor) + 0.02
                : Number(zoom.current) >= Number(zoom.ceiling) - 0.02;
        }
        hudRoot.addEventListener("pointerdown", function (event) {
            var button = event.target.closest && event.target.closest('[data-command="zoom_in"], [data-command="zoom_out"]');
            if (!button || button.disabled || event.button !== 0) return;
            stopZoomHold();
            event.preventDefault();
            event.stopPropagation();
            var command = button.dataset.command;
            button.classList.add("is-holding");
            send(command);
            zoomHold = {
                button: button,
                repeat: window.setInterval(function () {
                    if (zoomAtLimit(command)) {
                        window.clearInterval(zoomHold.repeat);
                        button.classList.add("is-zoom-limited");
                        return;
                    }
                    send(command);
                }, 180)
            };
        });
        ["pointerup", "pointercancel", "blur"].forEach(function (type) {
            window.addEventListener(type, stopZoomHold, { passive: true });
        });
        hudRoot.addEventListener("click", function (event) {
            var mapButton = event.target.closest("[data-map-action]");
            var mapGroup = event.target.closest("[data-map-group]");
            var mapBack = event.target.closest("[data-map-back]");
            if (mapButton || mapGroup || mapBack) {
                event.preventDefault();
                event.stopPropagation();
                var mapMenu = hudRefs && (mapButton && mapButton.closest(".sow-hud__building-card") || hudRefs.mapMenu);
                if (!mapMenu) return;
                if (mapBack) {
                    mapMenuView = "root";
                    renderMapMenu(hudState && hudState.hud && hudState.hud.map_menu);
                } else if (mapGroup) {
                    mapMenuView = mapGroup.dataset.mapGroup || "build";
                    renderMapMenu(hudState && hudState.hud && hudState.hud.map_menu);
                } else {
                    if (mapButton.disabled) return;
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
            if (cmd === "zoom_in" || cmd === "zoom_out") {
                if (event.detail === 0) send(cmd);
                return;
            }
            if (cmd === "toggle_dev_sidebar") {
                setHudPanel(null);
                devSidebarOpen = !devSidebarOpen;
                send("toggle_dev_sidebar");
                renderHud();
            } else if (cmd === "toggle_leaderboard") {
                setHudPanel("leaderboard");
                renderHud();
            } else if (cmd === "toggle_inbox") {
                setHudPanel("inbox");
                renderHud();
            } else if (cmd === "toggle_settings") {
                setHudPanel("settings");
                renderHud();
            } else if (cmd === "reset_dev_config") {
                send("reset_dev_config");
            } else if (cmd === "dev_vfx_all") {
                send("dev_vfx_all", { on: btn.dataset.on === "1" });
            } else if (cmd === "toggle_pin_emoji") {
                pinEmoji = !pinEmoji;
                send("set_emoji_pinned", { pinned: pinEmoji });
                renderHud();
            } else if (cmd === "open_transfer") {
                setHudPanel(null);
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
                if (!Number.isFinite(gold)) gold = 0;
                if (!Number.isFinite(transferTroops)) transferTroops = 0;
                if (targetId > 0) send(cmd, { target_player_id: targetId, gold: gold, troops: transferTroops });
                transferOpen = false;
                renderHud();
            } else if (cmd === "accept_alliance" || cmd === "reject_alliance" || cmd === "accept_resource_request" || cmd === "reject_resource_request") {
                var requesterId = Number(btn.dataset.playerId);
                if (requesterId > 0) send(cmd, { target_player_id: requesterId });
            } else if (cmd === "cancel_betrayal") {
                betrayalOpen = false;
                pendingCounterAttacks = Object.create(null);
                send("cancel_betrayal");
                renderHud();
            } else if (cmd === "confirm_betrayal") {
                betrayalOpen = false;
                send("confirm_betrayal");
                renderHud();
            } else if (cmd === "cancel_attack" || cmd === "recall_fleet") {
                var id = Number(cmd === "cancel_attack" ? btn.dataset.attackId : btn.dataset.fleetId);
                if (id > 0) btn.disabled = send(cmd, cmd === "cancel_attack" ? { attack_id: id } : { fleet_id: id });
            } else if (cmd === "counter_attack") {
                var counterTarget = Number(btn.dataset.playerId);
                var incomingTotal = Number(btn.dataset.incomingTroops);
                if (counterTarget > 0 && incomingTotal > 0 && pendingCounterAttacks[counterTarget] == null) {
                    pendingCounterAttacks[counterTarget] = incomingTotal;
                    if (hudRefs && hudRefs.operationRows) {
                        hudRefs.operationRows.querySelectorAll('[data-command="counter_attack"]').forEach(function (button) {
                            if (Number(button.dataset.playerId) === counterTarget) button.disabled = true;
                        });
                    }
                    if (!send("counter_attack", { target_player_id: counterTarget })) {
                        delete pendingCounterAttacks[counterTarget];
                        if (hudRefs && hudRefs.operationRows) {
                            hudRefs.operationRows.querySelectorAll('[data-command="counter_attack"]').forEach(function (button) {
                                if (Number(button.dataset.playerId) === counterTarget) button.disabled = false;
                            });
                        }
                    }
                }
            } else if (cmd === "focus_world") {
                var focusX = Number(btn.dataset.focusX);
                var focusY = Number(btn.dataset.focusY);
                if (Number.isFinite(focusX) && Number.isFinite(focusY)) send("focus_world", { x: focusX, y: focusY });
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
            } else if (cmd === "select_warship") {
                send("select_warship");
            } else if (cmd === "select_nuke") {
                send("select_nuke");
            } else if (cmd === "cancel_building_mode") {
                var selectedBuilding = hudState && hudState.hud && hudState.hud.selected_building;
                if (selectedBuilding) send("select_building", { kind: selectedBuilding });
            } else if (cmd === "close_building_card") {
                send("close_map_context_menu");
            } else if (cmd === "confirm_endgame_leave") {
                send("return_to_menu");
            } else if (cmd === "open_store") {
                if (window.SOW_PORTAL === "poki" || window.SOW_PORTAL === "jest") return;
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
        renderHud(true);
    });

    if (hudRoot) hudRoot.hidden = true;
    window.SOW_onStateUpdate(window.SOW_MENU_STATE);
})();
