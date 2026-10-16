(function () {
    "use strict";

    // ─────────────────────────────────────────────────────────
    // HUD CONTROLLER (DOM Overlay for In-Game RTS Matches)
    // ─────────────────────────────────────────────────────────
    var hudRoot = document.getElementById("sow-hud");
    var hudDevLayer = document.getElementById("sow-hud-dev-layer");
    var hudState = null;
    var lastHudRaw = "";
    var activeHudPanel = null;
    var betrayalOpen = false;
    var pinEmote = false;
    var emoteTextKey = "";
    var emoteTextTarget = null;
    var hudSafeArea = [0, 0, 0, 0];
    var surrenderModalOpen = false;
    var surrenderMessage = null;
    var devSidebarOpen = false;
    var devToolsSchemaKey = "";

    var hudInitialized = false;
    var hudRefs = null;
    var leaderboardRows = Object.create(null);
    var leaderboardRenderKey = "";
    var pendingCounterAttacks = Object.create(null);
    var lastLeaderboardPlayers = [];
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
    var EMPTY_CAMPAIGN_UNLOCKS = { buildings: {}, actions: [] };

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

    function updateHudSafeArea() {
        if (!hudRoot || typeof window.getComputedStyle !== "function") return;
        var probe = document.createElement("span");
        probe.style.cssText = "position:absolute;visibility:hidden;pointer-events:none;padding:env(safe-area-inset-top,0px) env(safe-area-inset-right,0px) env(safe-area-inset-bottom,0px) env(safe-area-inset-left,0px)";
        hudRoot.appendChild(probe);
        var style = window.getComputedStyle(probe);
        hudSafeArea = [style.paddingTop, style.paddingRight, style.paddingBottom, style.paddingLeft].map(function (value) { return Math.max(0, parseFloat(value) || 0); });
        probe.remove();
        syncEmoteTexts(hudState && hudState.hud);
    }

    function syncEmoteTexts(hud) {
        if (!hud || !Array.isArray(hud.emotes) || hud.emotes.length !== 4) return;
        var texts = hud.emotes.map(function (definition) { return SOW_t(definition.bubble_key); });
        var key = String(window.SOW_LOCALE || "en") + ":" + JSON.stringify(texts) + ":" + hudSafeArea.join(",");
        if (key === emoteTextKey && emoteTextTarget === window.SOW_menu_command) return;
        if (send("set_emote_texts", { texts: texts, safe_area: hudSafeArea })) {
            emoteTextKey = key;
            emoteTextTarget = window.SOW_menu_command;
        }
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
        var url = base + "/" + path.split("/").map(encodeURIComponent).join("/");
        return window.SOW_assetUrl ? window.SOW_assetUrl(url) : url;
    }

    function currencyAsset(kind) {
        return asset("gameplay/currency/" + kind + ".webp");
    }

    // One icon per concept: painted art for resources, the shared pixel
    // atlases for relationships and buildings, white glyphs for actions.
    var HUD_ICONS = {
        troops: "gameplay/icons/battle_icon_cutout.webp",
        gold: "gameplay/currency/gold.webp"
    };

    // Same atlas and cell order as NameplateStatusSprite (sow-render/src/text/types.rs).
    var STATUS_CELLS = { request: 0, allied: 1, traitor: 2 };

    var BUILDING_GLYPHS = {
        city: "city",
        factory: "factory",
        port: "anchor",
        bunker: "defense",
        farm: "farm"
    };

    var HUD_GLYPHS = {
        plus: '<path d="M12 4v16M4 12h16"/>',
        minus: '<path d="M4 12h16"/>',
        inbox: '<path d="M4 4h16v16H4zM4 13h4l2 3h4l2-3h4M4 7l8 6 8-6"/>',
        emote: '<path d="M20 11.5a7.5 7.5 0 0 1-8 7.5 8.7 8.7 0 0 1-3.5-.7L4 20l1.4-3.6A7 7 0 0 1 4 12c0-4.1 3.6-7.5 8-7.5s8 3.1 8 7Z"/><path d="M9 11h.01M15 11h.01M9 14.5s1 1.5 3 1.5 3-1.5 3-1.5"/>',
        rankings: '<path d="M3 20h18M5 20v-6h4v6M10 20V4h4v16M15 20v-10h4v10"/>',
        settings: '<path d="m10 3-.6 2.2a7.6 7.6 0 0 0-1.7 1L5.5 5.4 3.8 8.3l1.8 1.5a7.9 7.9 0 0 0 0 2.1l-1.8 1.5 1.7 2.9 2.2-.8a7.6 7.6 0 0 0 1.7 1L10 19h4l.6-2.2a7.6 7.6 0 0 0 1.7-1l2.2.8 1.7-2.9-1.8-1.5a7.9 7.9 0 0 0 0-2.1l1.8-1.5-1.7-2.9-2.2.8a7.6 7.6 0 0 0-1.7-1L14 3h-4Z"/><circle cx="12" cy="11" r="3"/>',
        leave: '<path d="M13 4h6a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-6M10 16l4-4-4-4M14 12H3"/>',
        camera: '<circle cx="12" cy="12" r="7.5"/><circle cx="12" cy="12" r="2.5"/><path d="M12 2v2.5M12 19.5V22M2 12h2.5M19.5 12H22"/>',
        tools: '<path d="M14.5 6.5a5 5 0 0 0-6.7 6.7L3 18l3 3 4.8-4.8a5 5 0 0 0 6.7-6.7L14 13l-3-3 3.5-3.5Z"/>',
        spawn: '<path d="M5 21V4m1 1h12l-3 4 3 4H6"/>',
        attack: '<path d="m5 4 14 16M19 4 5 20M3 6l3-3 3 3M15 18l3 3 3-3M3 18l3 3 3-3M15 6l3-3 3 3"/>',
        fleet: '<path d="M3 15h18l-3 5H7l-4-5ZM6 12l6-9 6 9M12 3v12M2 21c2 1 3 1 5 0s3-1 5 0 3 1 5 0 3-1 5 0"/>',
        transfer: '<path d="M4 7h15l-3-3M20 17H5l3 3M19 7l-3 3M5 17l3-3"/>',
        city: '<path d="M3 21h18M5 21V10l7-6 7 6v11M9 21v-6h6v6"/>',
        nuke: '<path d="M12 22V9M9 12l3-3 3 3M12 9l5-5M17 4h4M17 4V1M7 21h10M9 18h6"/>',
        upgrade: '<path d="m12 3 2.2 5.2L20 10l-4.4 3.5L17 19l-5-3-5 3 1.4-5.5L4 10l5.8-1.8L12 3Z"/>',
        anchor: '<path d="M12 3v12M8 7a4 4 0 1 0 8 0M4 13a8 8 0 0 0 16 0M2 13h4M18 13h4"/>',
        factory: '<path d="M3 21V9l6 3V8l6 4V5h6v16H3ZM17 8h1M17 11h1M7 17h2M12 17h2M17 17h1"/>',
        warship: '<path d="M3 15h18l-3 5H7l-4-5ZM7 12V7h10v5M10 7V4h4v3M2 21c2 1 3 1 5 0s3-1 5 0 3 1 5 0 3-1 5 0"/>',
        trade_ship: '<path d="M3 15h18l-3 5H7l-4-5ZM6 12l6-9 6 9M12 3v12M2 21c2 1 3 1 5 0s3-1 5 0 3 1 5 0 3-1 5 0M8 10h8"/>',
        farm: '<path d="M12 21V8M12 12c-4-1-6-3-6-6 4 0 6 2 6 6ZM12 15c4-1 6-3 6-6-4 0-6 2-6 6Z"/>',
        defense: '<path d="M12 22s8-4 8-11V5l-8-3-8 3v6c0 7 8 11 8 11Z"/>',
        range: '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="2"/><path d="M12 2v3M22 12h-3M12 22v-3M2 12h3"/>',
        speed: '<path d="m13 2-3 8h7l-6 12 1-9H5l8-11Z"/>',
        discount: '<circle cx="7" cy="7" r="2"/><circle cx="17" cy="17" r="2"/><path d="m19 5-14 14"/>',
        clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
        construction: '<path d="M14.5 6.5a5 5 0 0 0-6.7 6.7L3 18l3 3 4.8-4.8a5 5 0 0 0 6.7-6.7L14 13l-3-3 3.5-3.5Z"/>',
        lock: '<rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 1 1 8 0v3"/>'
    };

    function hudIcon(name, cls) {
        var glyph = HUD_GLYPHS[BUILDING_GLYPHS[name] || name];
        if (glyph) return '<svg class="' + (cls || "sow-hud__action-icon") + '" viewBox="0 0 24 24" fill="none" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><g class="sow-hud__glyph-ink">' + glyph + '</g><g class="sow-hud__glyph-face">' + glyph + '</g></svg>';
        if (!HUD_ICONS[name]) return "";
        return '<img class="' + (cls || "sow-hud__action-icon") + '" src="' + asset(HUD_ICONS[name]) + '" alt="" aria-hidden="true">';
    }

    function statusIcon(kind, cls) {
        var style = 'background-image:url(&quot;' + escapeHudText(asset("gameplay/icons/nameplate_status_atlas.webp")) + '&quot;);'
            + 'background-position:' + (STATUS_CELLS[kind] || 0) * 50 + '% 0;';
        return '<span class="' + (cls || "sow-hud__action-icon") + ' sow-hud__status-icon" style="' + style + '" aria-hidden="true"></span>';
    }

    function buildingIcon(kind, level, cls) {
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
            return '<span class="' + (cls || "sow-hud__building-icon") + ' is-pixel-art" style="' + style + '" aria-hidden="true"></span>';
        }
        var glyph = BUILDING_GLYPHS[name.toLowerCase()];
        return glyph ? hudIcon(glyph, cls || "sow-hud__building-icon") : "";
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

    function formatHudMetric(value) {
        var amount = Number(value);
        if (!Number.isFinite(amount)) return "0";
        var abs = Math.abs(amount);
        var sign = amount < 0 ? "-" : "";
        if (abs >= 1000000) return sign + (abs / 1000000).toFixed(1) + "M";
        if (abs >= 10000) return sign + (abs / 1000).toFixed(1) + "K";
        return amount.toLocaleString(undefined, { maximumFractionDigits: 1 });
    }

    function formatHudRate(value) {
        var amount = Number(value) || 0;
        return (amount > 0 ? "+" : "") + formatHudMetric(amount) + "/s";
    }

    function notificationAnchor(storyRoot, objective, nameplate, gap) {
        if (storyRoot && !storyRoot.hidden && objective && !objective.hidden) {
            var objectiveRect = objective.getBoundingClientRect();
            if (objectiveRect.width > 0 && objectiveRect.height > 0) {
                return {
                    source: "objective",
                    left: objectiveRect.left,
                    right: objectiveRect.right,
                    top: objectiveRect.bottom + gap
                };
            }
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
                syncCompactHudDensity();
                scheduleNotificationPlacement();
            });
        }
        [hudRefs.topbar, hudRefs.nameplate, hudRefs.dock, hudRefs.dockStack].forEach(function (element) {
            if (element && notificationResizeObserver) notificationResizeObserver.observe(element);
        });
        observeNotificationAnchors();
        syncCompactHudBounds();
        if (!notificationLayoutStarted) {
            notificationLayoutStarted = true;
            var onViewportChange = function () {
                syncCompactHudBounds();
                renderBuildingPlacement(hudState && hudState.hud && hudState.hud.building_placement);
                renderNotifications([], false);
                scheduleNotificationPlacement();
            };
            window.addEventListener("resize", onViewportChange, { passive: true });
            if (window.visualViewport) {
                window.visualViewport.addEventListener("resize", onViewportChange, { passive: true });
                window.visualViewport.addEventListener("scroll", onViewportChange, { passive: true });
            }
            document.addEventListener("focusout", function () {
                if (window.requestAnimationFrame) window.requestAnimationFrame(syncCompactHudBounds);
                else syncCompactHudBounds();
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
        emoteTextKey = "";
        hudRoot.innerHTML = ''
            + '<header class="sow-hud__topbar">'
            + '  <div class="sow-hud__status-right">'
            + '    <button class="sow-hud__icon-pill sow-hud__exit-pill" type="button" data-command="prompt_surrender" aria-label="' + SOW_t("hud.leave_match") + '" title="' + SOW_t("hud.leave_match") + '">' + hudIcon("leave", "sow-hud__action-icon") + '</button>'
            + '    <button class="sow-hud__icon-pill" type="button" data-command="toggle_settings" aria-label="' + SOW_t("menu.settings") + '" title="' + SOW_t("menu.settings") + '">' + hudIcon("settings", "sow-hud__action-icon") + '</button>'
            + '    <button class="sow-hud__icon-pill sow-hud__inbox-pill" type="button" data-command="toggle_inbox" aria-label="' + SOW_t("hud.inbox") + '" title="' + SOW_t("hud.inbox") + '">' + hudIcon("inbox") + '<span class="sow-hud__inbox-badge" id="sow-hud-inbox-count" hidden>0</span></button>'
            + '    <button class="sow-hud__icon-pill" type="button" data-command="toggle_leaderboard" aria-label="' + SOW_t("hud.rankings") + '" title="' + SOW_t("hud.rankings") + '">' + hudIcon("rankings", "sow-hud__action-icon") + '</button>'
            + '    <div class="sow-hud__diagnostics">'
            + '      <button class="sow-hud__icon-pill hidden" type="button" data-command="toggle_dev_sidebar" id="sow-hud-dev-btn" aria-label="' + SOW_t("hud.dev_tools") + '" title="' + SOW_t("hud.dev_tools") + '">' + hudIcon("tools", "sow-hud__action-icon") + '</button>'
            + '      <span class="sow-hud__fps" id="sow-hud-fps">' + SOW_t("hud.fps", { fps: "--" }) + '</span>'
            + '      <span class="sow-hud__ping" id="sow-hud-ping"></span>'
            + '    </div>'
            + '  </div>'
            + '</header>'
            + '<div class="sow-hud__nameplate" id="sow-hud-nameplate" hidden>'
            + '  <img class="sow-hud__nameplate-avatar" id="sow-hud-nameplate-avatar" alt="" draggable="false">'
            + '  <div class="sow-hud__nameplate-copy"><strong id="sow-hud-nameplate-name"></strong>'
            + '  <div class="sow-hud__nameplate-bar"><i id="sow-hud-nameplate-fill"></i><span class="sow-hud__nameplate-readout"><span id="sow-hud-nameplate-troops"></span>' + hudIcon("troops", "sow-hud__inline-icon") + '</span></div>'
            + '  <div class="sow-hud__nameplate-economy">'
            + '    <span class="sow-hud__nameplate-stat sow-hud__nameplate-stat--gold" title="' + SOW_t("hud.gold_treasury") + '" aria-label="' + SOW_t("hud.gold_treasury") + '"><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' + currencyAsset("gold") + '" alt="" aria-hidden="true"><b id="sow-hud-nameplate-gold">0</b></span>'
            + '    <span class="sow-hud__nameplate-stat sow-hud__nameplate-stat--gold-rate" title="' + SOW_t("hud.gold_net_rate") + '" aria-label="' + SOW_t("hud.gold_net_rate") + '"><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' + currencyAsset("gold") + '" alt="" aria-hidden="true"><b id="sow-hud-nameplate-gold-rate">0/s</b></span>'
            + '    <span class="sow-hud__nameplate-stat sow-hud__nameplate-stat--troop-rate" title="' + SOW_t("hud.troop_production_rate") + '" aria-label="' + SOW_t("hud.troop_production_rate") + '">' + hudIcon("troops", "sow-hud__inline-icon") + '<b id="sow-hud-nameplate-troop-rate">0/s</b></span>'
            + '  </div></div>'
            + '</div>'
            + '<div class="sow-hud__hover-card hidden" id="sow-hud-hover-card">'
            + '  <div class="sow-hud__hover-header"><b id="sow-hud-hover-name">' + SOW_t("hud.territory") + '</b></div>'
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
            + '</aside>'
            + '<aside class="sow-hud__dev-sidebar hidden" id="sow-hud-dev-sidebar">'
            + '  <div class="sow-hud__dev-header"><b>' + SOW_t("hud.dev_tools") + '</b><button type="button" class="sow-hud__close-btn" data-command="toggle_dev_sidebar" aria-label="' + SOW_t("hud.close_developer_tools") + '">✕</button></div>'
            + '  <div class="sow-hud__dev-body">'
            + '    <input class="sow-hud__dev-search" type="search" data-dev-search placeholder="Find shader setting" aria-label="Find shader setting">'
            + '    <div class="sow-hud__dev-controls" id="sow-hud-dev-controls"></div>'
            + '  </div>'
            + '</aside>'
            + '<aside class="sow-hud__panel sow-hud__settings hidden" id="sow-hud-settings">'
            + '  <div class="sow-hud__panel-header"><h3>' + SOW_t("menu.settings") + '</h3><button class="sow-hud__close-btn" type="button" data-command="toggle_settings" aria-label="' + SOW_t("hud.close_settings") + '">✕</button></div>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.sound") + '</span><input type="checkbox" data-hud-setting="mute_all"></label>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.music") + '</span><input type="range" min="0" max="1" step="0.05" data-hud-setting="music_volume"></label>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.reduced_motion") + '</span><input type="checkbox" data-hud-setting="reduced_motion"></label>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.free_zoom_out") + '</span><input type="checkbox" data-hud-setting="free_zoom_out"></label>'
            + '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.show_fps_ping") + '</span><input type="checkbox" data-hud-setting="show_fps_ping"></label>'
            + (hudState.settings && hudState.settings.show_dev_tools_available
                ? '  <label class="sow-hud__setting-row"><span>' + SOW_t("hud.show_dev_tools") + '</span><input type="checkbox" data-hud-setting="show_dev_tools"></label>'
                : '')
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
            + '        <span class="sow-hud__building-btn sow-hud__fleet-status" data-fleet-kind="trade" role="img" aria-label="' + SOW_t("hud.map_action_fleet") + '">' + hudIcon("trade_ship", "sow-hud__building-icon") + '</span>'
            + '        <button type="button" class="sow-hud__building-btn" data-command="select_warship" data-fleet-kind="warship" aria-label="' + SOW_t("hud.map_action_build_warship") + '" title="' + SOW_t("hud.map_action_build_warship") + '">' + hudIcon("warship", "sow-hud__building-icon") + '</button>'
            + '        <button type="button" class="sow-hud__building-btn" data-command="select_nuke" data-fleet-kind="nuke" aria-label="' + SOW_t("hud.map_action_nuke") + '" title="' + SOW_t("hud.map_action_nuke") + '">' + hudIcon("nuke", "sow-hud__building-icon") + '</button>'
            + '      </div>'
            + '    </div>'
            + '  </div>'
            + '</footer>'
            + '<aside class="sow-hud__operations" id="sow-hud-operations" hidden aria-label="' + SOW_t("hud.combat_operations") + '">'
            + '  <div class="sow-hud__operations-list" id="sow-hud-operations-list"></div>'
            + '</aside>'
            + '</div>'
            + '<aside class="sow-hud__placement" id="sow-hud-placement" role="group" aria-label="' + SOW_t("hud.build_placement") + '" hidden>'
            + '  <div class="sow-hud__placement-moves">'
            + '  <button type="button" class="sow-hud__placement-move" data-command="nudge_building_placement" data-direction="up" aria-label="' + SOW_t("hud.build_move_up") + '"><span aria-hidden="true">↑</span></button>'
            + '  <button type="button" class="sow-hud__placement-move" data-command="nudge_building_placement" data-direction="right" aria-label="' + SOW_t("hud.build_move_right") + '"><span aria-hidden="true">→</span></button>'
            + '  <button type="button" class="sow-hud__placement-move" data-command="nudge_building_placement" data-direction="down" aria-label="' + SOW_t("hud.build_move_down") + '"><span aria-hidden="true">↓</span></button>'
            + '  <button type="button" class="sow-hud__placement-move" data-command="nudge_building_placement" data-direction="left" aria-label="' + SOW_t("hud.build_move_left") + '"><span aria-hidden="true">←</span></button>'
            + '  </div>'
            + '  <div class="sow-hud__placement-info" id="sow-hud-placement-info"></div>'
            + '  <div class="sow-hud__placement-actions"><button type="button" class="sow-hud__placement-cancel" data-command="cancel_building_mode" aria-label="' + SOW_t("endgame.cancel") + '">×</button><button type="button" class="sow-hud__placement-confirm" data-command="confirm_building_placement" aria-label="' + SOW_t("hud.build_confirm") + '"><span aria-hidden="true">✓</span><span>' + SOW_t("hud.build_confirm") + '</span></button></div>'
            + '  <div class="sow-hud__placement-reason" id="sow-hud-placement-reason" role="status" aria-live="polite"></div>'
            + '</aside>'
            + '<aside class="sow-hud__panel sow-hud__leaderboard hidden" id="sow-hud-leaderboard">'
            + '  <div class="sow-hud__panel-header">'
            + '    <h3>' + hudIcon("rankings", "sow-hud__inline-icon") + ' ' + SOW_t("hud.rankings_title") + '</h3>'
            + '    <button class="sow-hud__close-btn" type="button" data-command="toggle_leaderboard" aria-label="' + SOW_t("hud.close_rankings") + '">✕</button>'
            + '  </div>'
            + '  <div class="sow-hud__leaderboard-rows" id="sow-hud-lb-rows"></div>'
            + '</aside>'
            + '<aside class="sow-hud__inbox hidden" id="sow-hud-inbox" role="dialog" aria-modal="false" aria-labelledby="sow-hud-inbox-title">'
            + '  <div class="sow-hud__transfer-header sow-hud__inbox-header"><span class="sow-hud__transfer-icon" aria-hidden="true">' + hudIcon("inbox", "sow-hud__inline-icon") + '</span><h3 id="sow-hud-inbox-title">' + SOW_t("hud.inbox") + '</h3><span class="sow-hud__inbox-total" id="sow-hud-inbox-total" aria-hidden="true">0</span><button class="sow-hud__close-btn" id="sow-hud-inbox-close" type="button" data-command="toggle_inbox" aria-label="' + SOW_t("hud.close_inbox") + '">✕</button></div>'
            + '  <div class="sow-hud__inbox-list" id="sow-hud-inbox-rows"></div>'
            + '  <p class="sow-hud__inbox-empty" id="sow-hud-inbox-empty" hidden>' + SOW_t("hud.no_pending_requests") + '</p>'
            + '</aside>'
            + '<aside class="sow-hud__dialog sow-hud__transfer hidden" id="sow-hud-transfer" role="dialog" aria-modal="false" aria-labelledby="sow-hud-transfer-title">'
            + '  <div class="sow-hud__transfer-header"><span class="sow-hud__transfer-icon" aria-hidden="true">' + hudIcon("transfer", "sow-hud__inline-icon") + '</span><h3 id="sow-hud-transfer-title">' + SOW_t("hud.resource_transfer") + '</h3><button class="sow-hud__close-btn" id="sow-hud-transfer-close" type="button" data-command="close_transfer" aria-label="' + SOW_t("hud.close_resource_transfer") + '">✕</button></div>'
            + '  <div class="sow-hud__dialog-body">'
            + '  <div class="sow-hud__dialog-participant" id="sow-hud-transfer-target"></div>'
            + '  <div class="sow-hud__transfer-fields">'
            + '    <label><span>' + hudIcon("troops", "sow-hud__inline-icon") + SOW_t("hud.troops") + '</span><input id="sow-hud-transfer-troops" type="number" inputmode="numeric" min="0" step="1" value="0"></label>'
            + '    <label><span><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' + currencyAsset("gold") + '" alt="">' + SOW_t("hud.gold") + '</span><input id="sow-hud-transfer-gold" type="number" inputmode="numeric" min="0" step="1" value="0"></label>'
            + '  </div>'
            + '  </div>'
            + '  <div class="sow-hud__transfer-actions"><button class="sow-hud__transfer-send" type="button" data-command="send_resources"><span>' + SOW_t("hud.send") + '</span><span aria-hidden="true">→</span></button><button class="sow-hud__transfer-request" type="button" data-command="request_resources">' + SOW_t("hud.request") + '</button></div>'
            + '</aside>'
            + '<div class="sow-hud__modal-backdrop sow-hud__betrayal-backdrop hidden" id="sow-hud-betrayal-modal">'
            + '  <section class="sow-hud__dialog sow-hud__betrayal" role="dialog" aria-modal="true" aria-labelledby="sow-hud-betrayal-title" aria-describedby="sow-hud-betrayal-copy">'
            + '    <div class="sow-hud__transfer-header"><span class="sow-hud__transfer-icon sow-hud__betrayal-mark" aria-hidden="true">' + statusIcon("traitor", "sow-hud__inline-icon") + '</span><h3 id="sow-hud-betrayal-title">' + SOW_t("hud.break_alliance") + '</h3><button class="sow-hud__close-btn" id="sow-hud-betrayal-close" type="button" data-command="cancel_betrayal" aria-label="' + SOW_t("endgame.cancel") + '">✕</button></div>'
            + '    <div class="sow-hud__dialog-body"><div class="sow-hud__dialog-participant" id="sow-hud-betrayal-target"></div><p class="sow-hud__betrayal-copy" id="sow-hud-betrayal-copy">' + SOW_t("hud.break_alliance_body") + '</p></div>'
            + '    <div class="sow-hud__transfer-actions sow-hud__betrayal-actions"><button class="sow-hud__transfer-request" id="sow-hud-betrayal-cancel" type="button" data-command="cancel_betrayal">' + SOW_t("hud.keep_alliance") + '</button><button class="sow-hud__betrayal-confirm" id="sow-hud-betrayal-confirm" type="button" data-command="confirm_betrayal">' + statusIcon("traitor", "sow-hud__inline-icon") + SOW_t("hud.attack") + '</button></div>'
            + '  </section>'
            + '</div>'
            + '<div class="sow-hud__endgame-backdrop hidden" id="sow-hud-surrender-modal">'
            + '  <section class="sow-hud__endgame-card sow-hud__exit-card" data-result="defeat" role="dialog" aria-modal="true" aria-labelledby="sow-hud-surrender-banner" aria-describedby="sow-hud-surrender-desc">'
            + '    <div class="sow-hud__endgame-hero">'
            + '      <div class="sow-hud__endgame-portrait-wrap">'
            + '        <img class="sow-hud__endgame-portrait" id="sow-hud-surrender-portrait" src="" alt="' + SOW_t("hud.leader_avatar") + '" />'
            + '      </div>'
            + '      <div class="sow-hud__endgame-hero-copy">'
            + '        <h2 class="sow-hud__endgame-banner" id="sow-hud-surrender-banner">' + hudIcon("leave", "sow-hud__inline-icon") + ' ' + SOW_t("endgame.leave_match") + '</h2>'
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
            + '        <div class="sow-hud__endgame-kicker"><span class="sow-hud__endgame-icon" id="sow-hud-endgame-icon" aria-hidden="true">' + hudIcon("attack", "sow-hud__inline-icon") + '</span><span>' + SOW_t("hud.match_result") + '</span></div>'
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

        if (hudDevLayer) {
            hudDevLayer.appendChild(hudRoot.querySelector("#sow-hud-dev-sidebar"));
        }

        hudRefs = {
            fps: document.getElementById("sow-hud-fps"),
            ping: document.getElementById("sow-hud-ping"),
            diagnostics: hudRoot.querySelector(".sow-hud__diagnostics"),
            nameplate: document.getElementById("sow-hud-nameplate"),
            plateAvatar: document.getElementById("sow-hud-nameplate-avatar"),
            plateName: document.getElementById("sow-hud-nameplate-name"),
            plateFill: document.getElementById("sow-hud-nameplate-fill"),
            plateTroops: document.getElementById("sow-hud-nameplate-troops"),
            plateGold: document.getElementById("sow-hud-nameplate-gold"),
            plateGoldWrap: hudRoot.querySelector(".sow-hud__nameplate-stat--gold"),
            plateGoldRate: document.getElementById("sow-hud-nameplate-gold-rate"),
            plateTroopRate: document.getElementById("sow-hud-nameplate-troop-rate"),
            inboxCount: document.getElementById("sow-hud-inbox-count"),
            hoverCard: document.getElementById("sow-hud-hover-card"),
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
            placement: document.getElementById("sow-hud-placement"),
            placementInfo: document.getElementById("sow-hud-placement-info"),
            placementReason: document.getElementById("sow-hud-placement-reason"),
            placementConfirm: hudRoot.querySelector('[data-command="confirm_building_placement"]'),
            placementMoves: Array.prototype.slice.call(hudRoot.querySelectorAll('[data-command="nudge_building_placement"]')),
            placementCancel: hudRoot.querySelector('[data-command="cancel_building_mode"]'),
            panelButtons: Array.prototype.slice.call(hudRoot.querySelectorAll(".sow-hud__topbar [data-command^='toggle_']")),
            leaderboard: document.getElementById("sow-hud-leaderboard"),
            rows: document.getElementById("sow-hud-lb-rows"),
            inbox: document.getElementById("sow-hud-inbox"),
            inboxRows: document.getElementById("sow-hud-inbox-rows"),
            inboxClose: document.getElementById("sow-hud-inbox-close"),
            inboxButton: hudRoot.querySelector(".sow-hud__inbox-pill"),
            inboxTotal: document.getElementById("sow-hud-inbox-total"),
            inboxEmpty: document.getElementById("sow-hud-inbox-empty"),
            inboxRequests: Object.create(null),
            inboxRequestKeys: [],
            inboxRenderKey: "",
            transfer: document.getElementById("sow-hud-transfer"),
            transferTarget: document.getElementById("sow-hud-transfer-target"),
            transferClose: document.getElementById("sow-hud-transfer-close"),
            transferReturnFocus: null,
            transferGold: document.getElementById("sow-hud-transfer-gold"),
            transferTroops: document.getElementById("sow-hud-transfer-troops"),
            betrayal: document.getElementById("sow-hud-betrayal-modal"),
            betrayalTarget: document.getElementById("sow-hud-betrayal-target"),
            betrayalCancel: document.getElementById("sow-hud-betrayal-cancel"),
            betrayalReturnFocus: null,
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
            combatOperationRows: Object.create(null),
            combatOperationsOrderKey: "",
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

        hudRefs.transferIdentity = createPlayerIdentity();
        hudRefs.transferTarget.appendChild(hudRefs.transferIdentity.element);
        hudRefs.betrayalIdentity = createPlayerIdentity();
        hudRefs.betrayalTarget.appendChild(hudRefs.betrayalIdentity.element);

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
        hudDevLayer.addEventListener("input", function (event) {
            var previewInput = event.target.closest("[data-dev-preview]");
            if (previewInput) {
                var field = previewInput.dataset.devPreview;
                if (field === "target") return;
                var value = previewInput.type === "checkbox" ? previewInput.checked : Number(previewInput.value);
                if (typeof value === "number" && !Number.isFinite(value)) return;
                if (field === "phase") {
                    hudDevLayer.querySelectorAll('[data-dev-preview="phase"]').forEach(function (sibling) {
                        if (sibling !== previewInput) sibling.value = previewInput.value;
                    });
                }
                var previewUpdate = {};
                previewUpdate[field] = value;
                send("set_dev_vfx_preview", previewUpdate);
                return;
            }
            if (event.target.matches("[data-dev-search]")) {
                renderDevControls(hudState && hudState.hud && hudState.hud.dev_tools || {});
                return;
            }
            if (event.target.matches("[data-dev-picker]")) {
                sendDevControl(event.target);
                return;
            }
            var input = event.target.closest("[data-dev]");
            if (!input) return;
            sendDevControl(input);
        });
        hudDevLayer.addEventListener("change", function (event) {
            var previewTarget = event.target.closest('[data-dev-preview="target"]');
            if (previewTarget) send("set_dev_vfx_preview", { target: Number(previewTarget.value) });
        });
        hudRoot.addEventListener("change", function (event) {
            if (event.target.matches("[data-emote-pin]")) {
                pinEmote = event.target.checked;
                send("set_emote_pinned", { pinned: pinEmote });
                return;
            }
            var input = event.target.closest("[data-hud-setting]");
            if (!input) return;
            var setting = input.dataset.hudSetting;
            if (setting === "mute_all") send("set_mute", { value: !input.checked });
            if (setting === "music_volume") send("set_music_volume", { value: Number(input.value) });
            if (setting === "reduced_motion") send("set_reduced_motion", { value: input.checked });
            if (setting === "free_zoom_out") send("set_free_zoom_out", { value: input.checked });
            if (setting === "show_fps_ping") send("set_show_fps_ping", { value: input.checked });
            if (setting === "show_dev_tools") {
                if (input.checked) {
                    setHudPanel(null);
                    renderHud();
                }
                send("set_show_dev_tools", { value: input.checked });
            }
        });
    }

    var mapActionLabels = {
        emote: { icon: "emote", key: "hud.map_action_emote" },
        spawn: { icon: "spawn", key: "hud.map_action_deploy" },
        attack: { icon: "attack", key: "hud.map_action_attack" },
        fleet: { icon: "fleet", key: "hud.map_action_fleet" },
        transfer: { icon: "transfer", key: "hud.map_action_transfer" },
        alliance: { status: "allied", key: "hud.map_action_alliance" },
        build_city: { key: "hud.map_action_city" },
        build_factory: { key: "hud.map_action_factory" },
        build_port: { key: "hud.map_action_port" },
        build_bunker: { key: "hud.map_action_bunker" },
        build_farm: { text: "Farm" },
        upgrade_structure: { icon: "upgrade", text: "Upgrade" },
        nuke: { icon: "nuke", key: "hud.map_action_nuke" },
        build_warship: { icon: "warship", key: "hud.map_action_build_warship" }
    };

    var allianceActionLabels = {
        request: { state: "request", key: "hud.map_action_request_alliance", status: "allied", disabled: false },
        accept: { state: "accept", key: "hud.map_action_accept_alliance", status: "allied", disabled: false },
        pending: { state: "pending", key: "hud.map_action_alliance_pending", status: "request", disabled: true },
        active: { state: "active", key: "hud.map_action_alliance_active", status: "allied", disabled: true },
        renew: { state: "renew", key: "hud.map_action_renew_alliance", status: "allied", disabled: false }
    };

    var buildMenuActions = {
        build_city: true,
        build_factory: true,
        build_port: true,
        build_bunker: true,
        build_farm: true,
        upgrade_structure: true,
        build_warship: true,
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
        if (item.reason_key) {
            var reason = SOW_t(item.reason_key, item.reason_troops == null
                ? {}
                : { troops: Math.ceil(Number(item.reason_troops)).toLocaleString() });
            if (item.reason_level != null || item.reason_seconds != null) {
                if (item.reason_key !== "lobbies.locked") reason = SOW_t("lobbies.locked") + " · " + reason;
                if (item.reason_level != null) reason += " " + Math.floor(Number(item.reason_level));
                if (item.reason_seconds != null) reason += " · " + Math.max(1, Math.ceil(Number(item.reason_seconds))) + "s";
            }
            return reason;
        }
        if (!item.disabled || item.cost == null) return "";
        return SOW_t("hud.not_enough_gold", {
            cost: Math.ceil(item.cost).toLocaleString(),
            available: Math.floor(Number(availableGold) || 0).toLocaleString()
        });
    }

    function campaignUnlocks(hud) {
        return hud && hud.tutorial && hud.tutorial.active
            ? hud.tutorial.effective_unlocks || EMPTY_CAMPAIGN_UNLOCKS
            : null;
    }
    function campaignAllowsBuilding(unlocks, kind, level) {
        return unlocks === null || window.SOWCampaign.campaignAllowsBuilding(unlocks, kind, level);
    }
    function campaignAllowsAction(unlocks, action) {
        return unlocks === null || window.SOWCampaign.campaignAllowsAction(unlocks, action);
    }
    function setCampaignLock(element, locked, reason) {
        element.classList.toggle("is-campaign-locked", Boolean(locked));
        if (locked && reason) element.dataset.lockLabel = reason;
        else delete element.dataset.lockLabel;
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
                reason_key: item && item.reason_key || null,
                reason_level: item && Number.isFinite(Number(item.reason_level)) ? Number(item.reason_level) : null,
                reason_seconds: item && Number.isFinite(Number(item.reason_seconds)) ? Number(item.reason_seconds) : null,
                reason_troops: item && Number.isFinite(Number(item.reason_troops)) ? Number(item.reason_troops) : null
            };
        });
    }

    function mapActionContent(icon, label, reason) {
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
        if (reason) {
            var reasonLabel = document.createElement("small");
            reasonLabel.className = "sow-hud__map-action-reason";
            reasonLabel.textContent = reason;
            content.appendChild(reasonLabel);
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
        button.disabled = disabled;
        button.setAttribute("aria-disabled", String(disabled));
        var buildingKind = action.indexOf("build_") === 0 ? action.slice(6) : "";
        buildingKind = buildingKind.charAt(0).toUpperCase() + buildingKind.slice(1);
        var buildingImg = buildingKind ? buildingIcon(buildingKind) : "";
        var status = presentation && presentation.status || label.status;
        var icon = status
            ? statusIcon(status, "sow-hud__map-action-icon")
            : buildingImg || (label.icon ? hudIcon(label.icon, "sow-hud__map-action-icon") : "•");
        var actionName = presentation ? SOW_t(presentation.key) : mapLabel(action);
        button.dataset.mapActionLabel = actionName;
        if (item.reason_key) button.dataset.reasonKey = item.reason_key;
        if (item.reason_level != null) button.dataset.reasonLevel = String(item.reason_level);
        if (item.reason_seconds != null) button.dataset.reasonSeconds = String(item.reason_seconds);
        if (item.reason_troops != null) button.dataset.reasonTroops = String(item.reason_troops);
        if (presentation && presentation.state) button.dataset.allianceState = presentation.state;
        var reason = mapActionReason(item, hudState && hudState.hud && hudState.hud.gold);
        var accessibleLabel = reason ? actionName + ". " + reason : actionName;
        button.setAttribute("aria-label", accessibleLabel);
        button.title = reason ? accessibleLabel : "";
        var content = mapActionContent(icon, withDetails ? "" : actionName, withDetails ? "" : reason);
        button.appendChild(content);
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
                reason_key: button.dataset.reasonKey || null,
                reason_level: Number.isFinite(Number(button.dataset.reasonLevel)) ? Number(button.dataset.reasonLevel) : null,
                reason_seconds: Number.isFinite(Number(button.dataset.reasonSeconds)) ? Number(button.dataset.reasonSeconds) : null,
                reason_troops: Number.isFinite(Number(button.dataset.reasonTroops)) ? Number(button.dataset.reasonTroops) : null
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
        var renderKey = stateKey + ":" + mapMenuView + ":" + Boolean(hudState && hudState.hud && hudState.hud.pin_emote) + ":" + Boolean(hudState && hudState.hud && hudState.hud.emote_pinned);
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
                }
                var sectors = [];
                var emoteItem = items.find(function (item) { return item.action === "emote"; });
                if (emoteItem) sectors.push({ group: "emote", icon: hudIcon("emote"), label: SOW_t("hud.map_action_emote") });
                ["transfer", "fleet", "alliance"].forEach(function (action) {
                    var item = items.find(function (candidate) { return candidate.action === action; });
                    if (item) sectors.push({ item: item, kind: action });
                });
                if (buildItems.length) sectors.push({ group: "build", icon: hudIcon("construction", "sow-hud__map-action-icon"), label: SOW_t("hud.map_action_build") });
                if (nukeItem) sectors.push({ item: nukeItem, kind: "nuke" });
                sectors.forEach(function (sector, index) {
                    if (sector.group) mapGroupSector(radial, items, index, sectors.length, sector.group, sector.icon, sector.label);
                    else mapSector(radial, sector.item, index, sectors.length, sector.kind, sector.kind === "alliance" ? allianceActionLabels[mapMenu.alliance_state] : null);
                });
                menu.appendChild(radial);
            } else {
                var panel = document.createElement("div");
                panel.className = "sow-hud__map-submenu";
                var header = document.createElement("div");
                header.className = "sow-hud__map-submenu-header";
                var heading = document.createElement("span");
                heading.className = "sow-hud__map-submenu-icon";
                heading.innerHTML = mapMenuView === "emote" ? hudIcon("emote") : hudIcon("construction", "sow-hud__map-action-icon");
                heading.setAttribute("aria-hidden", "true");
                header.appendChild(heading);
                var back = document.createElement("button");
                back.type = "button";
                back.className = "sow-hud__map-back";
                back.dataset.mapBack = "true";
                back.textContent = "←";
                back.setAttribute("aria-label", SOW_t("hud.map_action_back"));
                header.appendChild(back);
                panel.appendChild(header);
                if (mapMenuView === "emote") {
                    var emotes = hudState && hudState.hud && hudState.hud.emotes || [];
                    emotes.forEach(function (definition) {
                        var choice = document.createElement("button");
                        choice.type = "button";
                        choice.className = "sow-hud__map-action sow-hud__map-card sow-hud__emote-choice";
                        choice.setAttribute("role", "menuitem");
                        choice.dataset.emoteKind = definition.kind;
                        choice.textContent = SOW_t(definition.menu_key);
                        panel.appendChild(choice);
                    });
                    var pinLabel = document.createElement("label");
                    pinLabel.className = "sow-hud__emote-pin";
                    var pinControl = document.createElement("input");
                    pinControl.type = "checkbox";
                    pinControl.dataset.emotePin = "true";
                    pinControl.checked = Boolean(hudState.hud.pin_emote);
                    pinLabel.appendChild(pinControl);
                    pinLabel.appendChild(document.createTextNode(SOW_t("hud.emote_pin")));
                    panel.appendChild(pinLabel);
                    if (hudState.hud.emote_pinned) {
                        var release = document.createElement("button");
                        release.type = "button";
                        release.className = "sow-hud__map-action sow-hud__map-card";
                        release.dataset.command = "clear_emote";
                        release.textContent = SOW_t("hud.emote_clear");
                        panel.appendChild(release);
                    }
                } else {
                    buildItems.forEach(function (item) {
                        panel.appendChild(mapActionButton(item, "sow-hud__map-action sow-hud__map-card", true));
                    });
                }
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

    function devColorHex(value, linear) {
        function channel(v) {
            v = Math.max(0, Math.min(1, Number(v) || 0));
            if (linear) v = v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055;
            return Math.round(v * 255).toString(16).padStart(2, "0");
        }
        return "#" + channel(value[0]) + channel(value[1]) + channel(value[2]);
    }

    function devColorFromHex(hex, linear) {
        var value = [1, 3, 5].map(function (offset) {
            var v = parseInt(hex.slice(offset, offset + 2), 16) / 255;
            return linear ? (v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4)) : v;
        });
        return value;
    }

    function sendDevControl(input) {
        var control = input.closest("[data-dev-control]");
        if (!control) return;
        var id = control.dataset.devControl;
        var kind = control.dataset.devKind;
        var value;
        if (kind === "bool") {
            value = input.checked;
        } else if (kind === "number" || kind === "select") {
            value = Number(input.value);
            if (!Number.isFinite(value)) return;
            if (kind === "number" && input.type === "number") {
                var range = control.querySelector('input[type="range"]');
                if (range) {
                    var rangeMin = Number(range.dataset.devMin);
                    var rangeMax = Number(range.dataset.devMax);
                    var margin = Math.max((rangeMax - rangeMin) * 0.1, Number(range.step) || 0.01);
                    range.min = String(value < rangeMin ? value - margin : rangeMin);
                    range.max = String(value > rangeMax ? value + margin : rangeMax);
                }
            }
            control.querySelectorAll('[data-dev="' + id + '"]').forEach(function (sibling) {
                if (sibling !== input) sibling.value = input.value;
            });
        } else {
            var config = hudState && hudState.hud && hudState.hud.dev_tools && hudState.hud.dev_tools.config || {};
            value = Array.isArray(config[id]) ? config[id].slice() : (kind === "color" ? [1, 1, 1, 1] : [0, 0, 0]);
            control.querySelectorAll("[data-dev-channel]").forEach(function (channel) {
                var index = Number(channel.dataset.devChannel);
                var current = Number(channel.value);
                if (Number.isFinite(current)) value[index] = current;
            });
            if (input.hasAttribute("data-dev-picker")) {
                var rgb = devColorFromHex(input.value, input.dataset.devLinear === "1");
                value[0] = rgb[0]; value[1] = rgb[1]; value[2] = rgb[2];
                control.querySelectorAll("[data-dev-channel]").forEach(function (channel) {
                    channel.value = value[Number(channel.dataset.devChannel)] || 0;
                });
            } else {
                var index = Number(input.dataset.devChannel);
                value[index] = Number(input.value);
                if (!Number.isFinite(value[index])) return;
                var picker = control.querySelector("[data-dev-picker]");
                if (picker && index < 3) picker.value = devColorHex(value, picker.dataset.devLinear === "1");
            }
        }
        send("set_dev_config", { field: id, value: value });
    }

    function renderDevControls(devTools) {
        var host = hudRefs && hudRefs.devSidebar && hudRefs.devSidebar.querySelector("#sow-hud-dev-controls");
        if (!host || !Array.isArray(devTools.schema)) return;
        var key = devTools.schema.map(function (def) { return [def.id, def.kind, def.section, def.label, def.min, def.max, def.step, def.unit].join(":"); }).join("|");
        if (key !== devToolsSchemaKey) {
            var sections = Object.create(null);
            devTools.schema.forEach(function (def) {
                (sections[def.section] || (sections[def.section] = [])).push(def);
            });
            var html = '<div class="sow-hud__dev-row sow-hud__dev-bulk"><button type="button" class="sow-hud__dev-reset" data-command="dev_vfx_all" data-on="1">All On</button><button type="button" class="sow-hud__dev-reset" data-command="dev_vfx_all" data-on="0">All Off</button></div>'
                + '<div class="sow-hud__dev-preview"><b>Effect preview</b><select data-dev-preview="target" aria-label="Preview effect">'
                + '<option value="1">Attack flash</option><option value="2">Conquest</option><option value="3">Alliance celebration</option><option value="4">Expansion</option><option value="5">Combat haze</option><option value="6">Nuclear fallout</option><option value="7">Dialog highlight</option><option value="8">Screen alert</option>'
                + '</select><div class="sow-hud__dev-preview-phase"><span>Phase</span><input type="range" min="0" max="1" step="0.01" data-dev-preview="phase"><input type="number" min="0" max="1" step="0.01" data-dev-preview="phase"></div><label><input type="checkbox" data-dev-preview="paused" checked> Pause</label><label><input type="checkbox" data-dev-preview="repeat" checked> Repeat</label><button type="button" class="sow-hud__dev-reset" data-command="replay_dev_vfx_preview">Replay</button></div>';
            Object.keys(sections).forEach(function (section, sectionIndex) {
                html += '<details class="sow-hud__dev-section"' + (sectionIndex === 0 ? " open" : "") + '><summary><b>' + escapeHudText(section) + '</b><button type="button" class="sow-hud__dev-reset" data-command="reset_dev_config" data-dev-section="' + escapeHudText(section) + '">Reset</button></summary>';
                sections[section].forEach(function (def) {
                    var id = escapeHudText(def.id);
                    var label = escapeHudText(def.label);
                    var unit = def.unit ? ' <small>' + escapeHudText(def.unit) + '</small>' : "";
                    html += '<div class="sow-hud__dev-control" data-dev-control="' + id + '" data-dev-kind="' + def.kind + '" data-dev-search-text="' + escapeHudText((section + " " + def.label + " " + def.id).toLowerCase()) + '"><div class="sow-hud__dev-heading"><span class="sow-hud__dev-label">' + label + unit + '</span><button type="button" class="sow-hud__dev-reset" data-command="reset_dev_config" data-dev-field="' + id + '" aria-label="Reset ' + label + '">Reset</button></div><small class="sow-hud__dev-value" data-dev-meta="' + id + '"></small><div class="sow-hud__dev-inputs">';
                    if (def.kind === "bool") {
                        html += '<input type="checkbox" data-dev="' + id + '">';
                    } else if (def.kind === "number") {
                        html += '<div class="sow-hud__dev-number"><input type="range" min="' + def.min + '" max="' + def.max + '" data-dev-min="' + def.min + '" data-dev-max="' + def.max + '" step="' + def.step + '" data-dev="' + id + '"><input type="number" step="' + def.step + '" data-dev="' + id + '" aria-label="' + label + ' value"></div>';
                    } else if (def.kind === "select") {
                        html += '<select data-dev="' + id + '">' + (def.options || []).map(function (option) {
                            return '<option value="' + option.value + '">' + escapeHudText(option.label) + '</option>';
                        }).join("") + '</select>';
                    } else if (def.kind === "color" || def.kind === "vector") {
                        var linear = def.unit === "linear RGB";
                        if (def.kind === "color") html += '<input class="sow-hud__dev-color-picker" type="color" data-dev-picker="' + id + '" data-dev-linear="' + (linear ? "1" : "0") + '" aria-label="' + label + ' color">';
                        html += '<div class="sow-hud__dev-channels">' + (def.kind === "color" ? ["R", "G", "B", "A"] : ["X", "Y", "Z"]).map(function (channel, index) {
                            var step = def.kind === "color" && index === 3 ? 0.01 : def.step;
                            return '<label>' + channel + '<input type="number" step="' + step + '" data-dev="' + id + '" data-dev-channel="' + index + '" aria-label="' + label + ' ' + channel + '"></label>';
                        }).join("") + '</div>';
                    }
                    html += '</div></div>';
                });
                html += '</details>';
            });
            html += '<button type="button" class="sow-hud__dev-reset" data-command="reset_dev_config">Reset all shader settings</button>';
            host.innerHTML = html;
            devToolsSchemaKey = key;
        }
        var search = hudRefs.devSidebar.querySelector("[data-dev-search]");
        var query = search ? search.value.trim().toLowerCase() : "";
        host.querySelectorAll("[data-dev-control]").forEach(function (control) {
            control.hidden = query !== "" && control.dataset.devSearchText.indexOf(query) < 0;
        });
        host.querySelectorAll(".sow-hud__dev-section").forEach(function (section) {
            var visible = Array.from(section.querySelectorAll("[data-dev-control]")).some(function (control) { return !control.hidden; });
            section.hidden = !visible;
            if (query && visible) section.open = true;
        });
        var config = devTools.config || {};
        devTools.schema.forEach(function (def) {
            var meta = host.querySelector('[data-dev-meta="' + def.id + '"]');
            if (!meta) return;
            var current = config[def.id];
            if (def.kind === "number") {
                var control = host.querySelector('[data-dev-control="' + def.id + '"]');
                var range = control && control.querySelector('input[type="range"]');
                var active = document.activeElement;
                var editingControl = active && active.matches && active.matches("[data-dev]")
                    && active.closest("[data-dev-control]") === control;
                if (range && !editingControl) {
                    var value = Number(current);
                    var min = Number(range.dataset.devMin);
                    var max = Number(range.dataset.devMax);
                    var margin = Math.max((max - min) * 0.1, Number(range.step) || 0.01);
                    range.min = String(value < min ? value - margin : min);
                    range.max = String(value > max ? value + margin : max);
                }
            }
            var format = function (value) {
                if (def.kind === "bool") return value ? "On" : "Off";
                if (def.kind === "select") {
                    var option = (def.options || []).find(function (item) { return Number(item.value) === Number(value); });
                    return option ? option.label : String(value);
                }
                if (Array.isArray(value)) return "[" + value.map(function (part) { return Number(part).toFixed(3); }).join(", ") + "]";
                return Number(value).toFixed(3);
            };
            var note = "Now " + format(current) + " · Default " + format(def.default);
            if (def.id.indexOf("water.") === 0 && /_limit$/.test(def.id) && Number(current) > Number(config["water.depth_cap"])) {
                note += " · above depth limit";
            }
            meta.textContent = note;
        });
        var preview = devTools.preview || {};
        host.querySelectorAll("[data-dev-preview]").forEach(function (input) {
            if (document.activeElement === input) return;
            var value = preview[input.dataset.devPreview];
            if (value == null) return;
            if (input.type === "checkbox") input.checked = Boolean(value);
            else input.value = value;
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

    function buildingPlacementLayout(placement, panelSize, area, docked) {
        var gap = 8, button = 48;
        var halfW = Math.max(0, Number(placement.half_width) || Number(placement.half_height) || 0);
        var halfH = Math.max(0, Number(placement.half_height) || 0);
        var x = Number(placement.x), y = Number(placement.y);
        var base = { left: x - halfW, right: x + halfW, top: y - halfH, bottom: y + halfH };
        var moves = {
            up: { left: x - button / 2, top: base.top - gap - button },
            right: { left: base.right + gap, top: y - button / 2 },
            down: { left: x - button / 2, top: base.bottom + gap },
            left: { left: base.left - gap - button, top: y - button / 2 }
        };
        function fits(rect, width, height) {
            return rect.left >= area.left && rect.top >= area.top
                && rect.left + width <= area.right && rect.top + height <= area.bottom;
        }
        var width = Math.min(panelSize.width, area.right - area.left);
        var height = panelSize.height;
        var left = Math.max(area.left, Math.min(x - width / 2, area.right - width));
        var top = base.top - button - gap * 2 - height;
        if (!fits({ left: left, top: top }, width, height)) top = base.bottom + button + gap * 2;
        var floating = !docked && fits({ left: left, top: top }, width, height)
            && Object.keys(moves).every(function (direction) { return fits(moves[direction], button, button); });
        if (!floating) {
            top = Math.max(area.top, area.bottom - height);
            var overlapsBase = left < base.right + gap && left + width > base.left - gap
                && top < base.bottom + gap && top + height > base.top - gap;
            if (overlapsBase) {
                if (base.top - gap - height >= area.top) top = base.top - gap - height;
                else if (base.bottom + gap + height <= area.bottom) top = base.bottom + gap;
                else if (base.left - gap - width >= area.left) left = base.left - gap - width;
                else if (base.right + gap + width <= area.right) left = base.right + gap;
            }
        }
        return { docked: !floating, left: left, top: top, moves: moves };
    }

    function renderBuildingPlacement(placement) {
        var panel = hudRefs.placement;
        if (!panel) return;
        panel.hidden = !placement;
        if (!placement) return;
        panel.dataset.siteValid = placement.site_valid ? "true" : "false";
        panel.dataset.affordable = placement.affordable === false ? "false" : "true";
        hudRefs.placementConfirm.disabled = !placement.can_confirm;
        hudRefs.placementCancel.disabled = Boolean(placement.pending);
        hudRefs.placementInfo.textContent = SOW_t("hud.build_details", {
            cost: Math.ceil(Number(placement.cost) || 0).toLocaleString(),
            seconds: Math.ceil(Number(placement.seconds) || 0)
        });
        var reason = placement.reason;
        hudRefs.placementReason.textContent = placement.pending ? SOW_t("hud.build_pending")
            : reason && reason.key ? SOW_t(reason.key, reason.values || {}) : SOW_t("hud.build_placement_help");
        var area = { left: 8 + hudSafeArea[3], top: 8 + hudSafeArea[0],
            right: window.innerWidth - 8 - hudSafeArea[1], bottom: window.innerHeight - 8 - hudSafeArea[2] };
        // Existing HUD surfaces define the usable map, including landscape safe areas.
        [hudRefs.topbar, hudRefs.nameplate, hudRefs.dock, hudRefs.railWrap, hudRefs.rightRail].forEach(function (element) {
            if (!element || element.hidden || !element.getBoundingClientRect) return;
            var rect = element.getBoundingClientRect();
            if (!rect.width || !rect.height) return;
            if (rect.width > window.innerWidth * 0.5) {
                if (rect.top < window.innerHeight / 3) area.top = Math.max(area.top, rect.bottom + 8);
                else if (rect.bottom > window.innerHeight * 2 / 3) area.bottom = Math.min(area.bottom, rect.top - 8);
            } else if (rect.height > window.innerHeight * 0.4) {
                if (rect.left < window.innerWidth / 4) area.left = Math.max(area.left, rect.right + 8);
                else if (rect.right > window.innerWidth * 3 / 4) area.right = Math.min(area.right, rect.left - 8);
            }
        });
        panel.dataset.layout = "floating";
        panel.style.maxWidth = Math.max(48, area.right - area.left) + "px";
        var layout = buildingPlacementLayout(placement,
            { width: panel.offsetWidth, height: panel.offsetHeight }, area, false);
        if (layout.docked) {
            panel.dataset.layout = "docked";
            layout = buildingPlacementLayout(placement,
                { width: panel.offsetWidth, height: panel.offsetHeight }, area, true);
        }
        panel.style.left = layout.left + "px";
        panel.style.top = layout.top + "px";
        hudRefs.placementMoves.forEach(function (button) {
            var direction = button.dataset.direction;
            button.disabled = Boolean(placement.pending) || !placement.can_nudge || !placement.can_nudge[direction];
            button.style.left = layout.moves[direction].left + "px";
            button.style.top = layout.moves[direction].top + "px";
        });
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
            if (detail.campaign_upgrade_locked) {
                hudRefs.buildingCardNext.innerHTML = "";
                hudRefs.buildingCardNext.textContent = "";
                hudRefs.buildingCardNext.hidden = true;
            } else {
                hudRefs.buildingCardNext.innerHTML = '<span class="sow-hud__building-level sow-hud__building-level--max" title="Maximum level">MAX</span>';
                hudRefs.buildingCardNext.hidden = false;
            }
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
                left.appendChild(name);
                right.appendChild(territory);
                right.appendChild(troops);
                right.appendChild(transfer);
                card.appendChild(left);
                card.appendChild(right);
                row = { card: card, rank: rank, name: name, territory: territory, troops: troops, key: "" };
            }

            var displayName = player.name || SOW_t("hud.player_name");
            var rowKey = [player.rank, idx, displayName, player.troops, player.tile_count, player.is_alive, player.is_me].join("|");
            if (row.key !== rowKey) {
                row.key = rowKey;
                row.card.classList.toggle("is-me", !!player.is_me);
                row.card.classList.toggle("is-dead", !player.is_alive);
                var displayRank = Number.isFinite(Number(player.rank)) ? Number(player.rank) : idx + 1;
                row.rank.textContent = "#" + displayRank;
                row.name.textContent = displayName;
                row.territory.textContent = Math.round((player.territory_pct || 0) * 100) + "%";
                row.troops.innerHTML = escapeHudText(player.troops > 1000 ? (player.troops / 1000).toFixed(1) + "k" : Math.floor(player.troops)) + " " + hudIcon("troops", "sow-hud__inline-icon");
            }
            fragment.appendChild(row.card);
            nextRows[key] = row;
        });
        hudRefs.rows.replaceChildren(fragment);
        leaderboardRows = nextRows;
        leaderboardRenderKey = renderKey;
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
            bindAvatarFallback(image);
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

    function bindAvatarFallback(image) {
        image.onerror = function () {
            var fallback = asset("gameplay/avatars/null.webp");
            if (this.getAttribute("src") !== fallback) {
                this.hidden = false;
                this.src = fallback;
            } else {
                this.hidden = true;
                this.removeAttribute("src");
            }
        };
    }

    function renderAvatarIdentity(image, emblem, presentation) {
        var identityKey = presentation ? JSON.stringify(presentation) : "";
        if (image._sowAvatarIdentityKey === identityKey) return;
        image._sowAvatarIdentityKey = identityKey;
        image.hidden = true;
        if (image.removeAttribute) image.removeAttribute("src");
        else image.src = "";
        emblem.hidden = true;
        emblem.textContent = "";
        if (!presentation) return;

        if (presentation.kind === "emblem" && typeof presentation.symbol === "string" && presentation.symbol) {
            emblem.textContent = presentation.symbol;
            emblem.hidden = false;
            return;
        }

        var slug = presentation.kind === "portrait" && /^[a-z][a-z0-9_]*$/.test(presentation.slug || "")
            ? presentation.slug
            : "null";
        var src = asset("gameplay/avatars/" + slug + ".webp");
        image.hidden = false;
        if (image.getAttribute("src") !== src) image.src = src;
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
            avatar.image._sowAvatarIdentityKey = null;
            if (avatar.image.removeAttribute) avatar.image.removeAttribute("src");
            else avatar.image.src = "";
            avatar.emblem.hidden = true;
            avatar.emblem.textContent = "";
        });
    }

    function notificationIcon(key) {
        if (key.indexOf("nuke") >= 0) return hudIcon("nuke", "sow-hud__notification-icon");
        if (key.indexOf("betrayal") >= 0) return statusIcon("traitor", "sow-hud__notification-icon");
        if (/attack|elimination/.test(key)) return hudIcon("attack", "sow-hud__notification-icon");
        if (key.indexOf("fleet") >= 0) return hudIcon("fleet", "sow-hud__notification-icon");
        if (key.indexOf("request") >= 0) return hudIcon("inbox", "sow-hud__notification-icon");
        if (key.indexOf("alliance") >= 0) return statusIcon("allied", "sow-hud__notification-icon");
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
            slot.wrapper.hidden = !presentation;
            slot.wrapper.className = "sow-hud__notification-avatar"
                + (index > 0 && avatars[index - 1] ? " sow-hud__notification-avatar--overlap" : "");
            renderAvatarIdentity(slot.image, slot.emblem, presentation);
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

    function compactHudDensity(width, height) {
        width = Number(width);
        height = Number(height);
        if (!Number.isFinite(width) || !Number.isFinite(height) || width <= height || width <= 0 || height <= 0) return 1;
        return Math.max(0.65, Math.min(1, width / 960, height / 720));
    }

    function writeHudDensity(target, density, storyRoot) {
        if (!target) return;
        if (density < 1) {
            if (target.dataset.compact !== "true") target.dataset.compact = "true";
            var values = {
                "--sow-hud-density": density.toFixed(4),
                "--sow-hud-hit-target-zoom": (1 / density).toFixed(4),
                "--sow-hud-text-primary-size": (12 / density).toFixed(2) + "px",
                "--sow-hud-text-secondary-size": (11 / density).toFixed(2) + "px",
                "--sow-player-panel-height": (storyRoot ? 92 * density : 92).toFixed(2) + "px"
            };
            if (!storyRoot) {
                values["--sow-tutorial-progress-height"] = "24px";
                values["--sow-tutorial-progress-label-size"] = (14 / density).toFixed(2) + "px";
            }
            Object.keys(values).forEach(function (property) {
                if (target.style.getPropertyValue(property) !== values[property]) target.style.setProperty(property, values[property]);
            });
        } else {
            if (target.dataset.compact) delete target.dataset.compact;
            ["--sow-hud-density", "--sow-hud-hit-target-zoom", "--sow-hud-text-primary-size", "--sow-hud-text-secondary-size", "--sow-player-panel-height", "--sow-tutorial-progress-height", "--sow-tutorial-progress-label-size"].forEach(function (property) {
                if (target.style.getPropertyValue(property)) target.style.removeProperty(property);
            });
        }
    }

    var compactHudDensityValue = null;

    function syncCompactHudDensity() {
        if (!hudRoot) return 1;
        var active = document.activeElement;
        var isEditingHudField = active && hudRoot.contains(active)
            && /^(INPUT|TEXTAREA|SELECT)$/.test(active.tagName || "");
        var width = hudRoot.clientWidth || window.innerWidth || 0;
        var height = hudRoot.clientHeight || window.innerHeight || 0;
        var density = isEditingHudField && compactHudDensityValue !== null
            ? compactHudDensityValue
            : compactHudDensity(width, height);
        compactHudDensityValue = density;
        writeHudDensity(hudRoot, density, false);
        writeHudDensity(hudDevLayer, density, false);
        writeHudDensity(document.getElementById("sow-story"), density, true);
        if (density < 1) {
            var operationCap = Math.max(0, height * 0.2 / density).toFixed(1) + "px";
            if (hudRoot.style.getPropertyValue("--sow-hud-layout-operations-max-height") !== operationCap) {
                hudRoot.style.setProperty("--sow-hud-layout-operations-max-height", operationCap);
            }
        } else if (hudRoot.style.getPropertyValue("--sow-hud-layout-operations-max-height")) {
            hudRoot.style.removeProperty("--sow-hud-layout-operations-max-height");
        }
        return density;
    }

    function clearCompactHudDensity() {
        compactHudDensityValue = null;
        writeHudDensity(hudRoot, 1, false);
        writeHudDensity(hudDevLayer, 1, false);
        writeHudDensity(document.getElementById("sow-story"), 1, true);
        if (hudRoot && hudRoot.style.getPropertyValue("--sow-hud-layout-operations-max-height")) {
            hudRoot.style.removeProperty("--sow-hud-layout-operations-max-height");
        }
        if (hudRoot && hudRoot.style.getPropertyValue("--sow-hud-layout-operations-bottom")) {
            hudRoot.style.removeProperty("--sow-hud-layout-operations-bottom");
        }
    }

    function syncDefaultDockReserve() {
        if (hudRefs.dock && hudRefs.dock.hidden) {
            hudRoot.style.setProperty("--sow-dock-reserve", "0px");
        } else {
            hudRoot.style.removeProperty("--sow-dock-reserve");
        }
    }

    function syncCompactHudBounds() {
        if (!hudRoot || !hudRefs) return;
        var viewport = window.visualViewport;
        var viewHeight = viewport && viewport.height || hudRoot.clientHeight || window.innerHeight;
        var viewTop = viewport && viewport.offsetTop || 0;
        var viewBottom = Math.max(0, (hudRoot.clientHeight || window.innerHeight) - viewHeight - viewTop);
        var viewportValues = {
            "--sow-hud-dialog-view-height": viewHeight + "px",
            "--sow-hud-dialog-view-top": viewTop + "px",
            "--sow-hud-dialog-view-bottom": viewBottom + "px"
        };
        Object.keys(viewportValues).forEach(function (key) {
            if (hudRoot.style.getPropertyValue(key) !== viewportValues[key]) hudRoot.style.setProperty(key, viewportValues[key]);
        });
        var density = syncCompactHudDensity();
        var coarsePointer = window.matchMedia
            && window.matchMedia("(hover: none) and (pointer: coarse)").matches;
        hudRoot.style.removeProperty("--sow-hud-layout-operations-bottom");
        var operationAnchorTop = window.innerHeight;
        [hudRefs.dock].forEach(function (element) {
            if (!element || element.hidden || !element.getClientRects().length) return;
            operationAnchorTop = Math.min(operationAnchorTop, element.getBoundingClientRect().top);
        });
        if (operationAnchorTop < window.innerHeight) {
            hudRoot.style.setProperty(
                "--sow-hud-layout-operations-bottom",
                Math.ceil(window.innerHeight - operationAnchorTop + 2) + "px"
            );
        }
        if (!coarsePointer) {
            ["--sow-hud-layout-top-reserve", "--sow-hud-layout-action-top", "--sow-hud-layout-dock-height", "--sow-hud-layout-bottom-edge"].forEach(function (property) {
                hudRoot.style.removeProperty(property);
            });
            syncDefaultDockReserve();
            return;
        }
        if (density >= 1) {
            ["--sow-hud-layout-top-reserve", "--sow-hud-layout-action-top", "--sow-hud-layout-dock-height", "--sow-hud-layout-bottom-edge"].forEach(function (property) {
                hudRoot.style.removeProperty(property);
            });
            syncDefaultDockReserve();
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
        var dockInner = hudRefs.dock && hudRefs.dock.querySelector(".sow-hud__dock-inner");
        var dockHeight = dockInner
            ? Math.ceil(dockInner.getBoundingClientRect().height)
            : (hudRefs.dock ? Math.ceil(hudRefs.dock.getBoundingClientRect().height) : 0);
        var safeBottom = Number.parseFloat(window.getComputedStyle(hudRoot).getPropertyValue("--sow-sab")) || 0;
        hudRoot.style.setProperty("--sow-hud-layout-dock-height", dockHeight + "px");
        var dockAnchorTop = window.innerHeight;
        [hudRefs.dock].forEach(function (element) {
            if (!element || element.hidden || !element.getClientRects().length) return;
            dockAnchorTop = Math.min(dockAnchorTop, element.getBoundingClientRect().top);
        });
        var stackTop = window.innerHeight;
        var stackBottom = 0;
        [dockInner, hudRefs.operations].forEach(function (element) {
            if (!element || element.hidden || !element.getClientRects().length) return;
            var rect = element.getBoundingClientRect();
            stackTop = Math.min(stackTop, rect.top);
            stackBottom = Math.max(stackBottom, rect.bottom);
        });
        if (stackBottom === 0) stackBottom = window.innerHeight;
        var stackHeight = Math.max(0, Math.ceil(stackBottom - stackTop));
        var dockReserve = stackHeight > 0 ? stackHeight + Math.max(8, safeBottom) + 8 : 0;
        hudRoot.style.setProperty("--sow-dock-reserve", dockReserve + "px");
        hudRoot.style.setProperty("--sow-hud-layout-bottom-edge", Math.floor(stackTop - 8) + "px");
        var freeOperationsHeight = Math.max(0, dockAnchorTop - Math.max(actionTop, top + 8) - safeBottom - 6);
        var operationCap = Math.max(0, Math.min(window.innerHeight * 0.2, freeOperationsHeight) / density).toFixed(1) + "px";
        if (hudRoot.style.getPropertyValue("--sow-hud-layout-operations-max-height") !== operationCap) {
            hudRoot.style.setProperty("--sow-hud-layout-operations-max-height", operationCap);
        }
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

    function createPlayerIdentity() {
        var identity = document.createElement("div");
        identity.className = "sow-hud__request-identity";
        var avatar = document.createElement("span");
        avatar.className = "sow-hud__notification-avatar";
        var portrait = document.createElement("img");
        portrait.className = "sow-hud__notification-portrait";
        portrait.alt = "";
        portrait.draggable = false;
        bindAvatarFallback(portrait);
        var emblem = document.createElement("span");
        emblem.className = "sow-hud__notification-emblem";
        avatar.append(portrait, emblem);
        var copy = document.createElement("div");
        copy.className = "sow-hud__request-copy";
        var name = document.createElement("bdi");
        name.className = "sow-hud__request-name";
        copy.appendChild(name);
        identity.append(avatar, copy);
        return { element: identity, portrait: portrait, emblem: emblem, copy: copy, name: name };
    }

    function renderPlayerIdentity(parts, name, avatar) {
        var displayName = name || SOW_t("hud.player_name");
        if (parts.name.textContent !== displayName) parts.name.textContent = displayName;
        renderAvatarIdentity(parts.portrait, parts.emblem, avatar || { kind: "fallback" });
    }

    function restoreHudFocus(target) {
        if (target && !target.isConnected && target.id) target = document.getElementById(target.id);
        if (!target || !target.isConnected || !target.getClientRects().length) target = document.getElementById("blade");
        if (target && typeof target.focus === "function") target.focus({ preventScroll: true });
    }

    function createInboxRequest(request) {
        var element = document.createElement("article");
        element.className = "sow-hud__request-card";
        var identity = createPlayerIdentity();
        var type = document.createElement("span");
        type.className = "sow-hud__request-type";
        var resources = document.createElement("div");
        resources.className = "sow-hud__request-resources";
        var amounts = {};
        ["gold", "troops"].forEach(function (kind) {
            var metric = document.createElement("span");
            metric.className = "sow-hud__request-metric";
            if (kind === "gold") {
                var icon = document.createElement("img");
                icon.className = "sow-hud__currency-icon sow-hud__currency-icon--gold";
                icon.src = currencyAsset("gold");
                icon.alt = "";
                metric.appendChild(icon);
            } else metric.innerHTML = hudIcon("troops", "sow-hud__inline-icon");
            var value = document.createElement("bdi");
            metric.appendChild(value);
            resources.appendChild(metric);
            amounts[kind] = { element: metric, value: value };
        });
        identity.copy.append(type, resources);
        var actions = document.createElement("div");
        actions.className = "sow-hud__request-actions";
        var buttons = ["accept", "reject"].map(function (action) {
            var button = document.createElement("button");
            button.type = "button";
            button.className = "sow-hud__request-" + action;
            button.dataset.command = action + (request.kind === "resources" ? "_resource_request" : "_alliance");
            button.dataset.playerId = String(request.requester_id);
            actions.appendChild(button);
            return button;
        });
        element.append(identity.element, actions);
        return { element: element, portrait: identity.portrait, emblem: identity.emblem, name: identity.name, type: type, resources: resources, amounts: amounts, accept: buttons[0], reject: buttons[1] };
    }

    function renderInbox(requests) {
        if (!hudRefs || !hudRefs.inboxRows || !Array.isArray(requests)) return;
        var renderKey = String(window.SOW_LOCALE || "en") + ":" + JSON.stringify(requests);
        if (renderKey === hudRefs.inboxRenderKey) return;
        var previous = hudRefs.inboxRequests;
        var previousKeys = hudRefs.inboxRequestKeys;
        var focusedKey = previousKeys.find(function (key) { return previous[key].element.contains(document.activeElement); });
        var next = Object.create(null);
        var keys = [];
        var numbers = new Intl.NumberFormat(window.SOW_LOCALE || "en", { maximumFractionDigits: 0 });
        requests.forEach(function (request) {
            if (!request || (request.kind !== "alliance" && request.kind !== "resources")
                || !Number.isSafeInteger(request.requester_id) || request.requester_id <= 0) return;
            var key = request.kind + ":" + request.requester_id;
            if (next[key]) return;
            var row = previous[key] || createInboxRequest(request);
            var name = request.name || SOW_t("hud.player_name");
            renderPlayerIdentity(row, name, request.avatar);
            row.type.textContent = SOW_t(request.kind === "resources" ? "hud.resource_request_title" : "hud.alliance_request_title");
            var values = { name: name };
            ["gold", "troops"].forEach(function (kind) {
                var value = Number(request[kind]);
                value = Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0;
                values[kind] = numbers.format(value);
                row.amounts[kind].value.textContent = values[kind];
                row.amounts[kind].element.hidden = value === 0;
                row.amounts[kind].element.setAttribute("aria-label", SOW_t(kind === "gold" ? "hud.gold" : "hud.troops") + ": " + values[kind]);
            });
            row.resources.hidden = request.kind !== "resources";
            row.element.setAttribute("aria-label", SOW_t(request.kind === "resources" ? "hud.resource_request" : "hud.alliance_request", values));
            row.accept.textContent = SOW_t("hud.accept");
            row.reject.textContent = SOW_t("hud.reject");
            row.accept.disabled = request.active === false;
            row.accept.title = row.accept.disabled ? SOW_t("hud.action_unavailable") : "";
            row.element.classList.toggle("is-inactive", row.accept.disabled);
            next[key] = row;
            keys.push(key);
        });
        previousKeys.forEach(function (key) {
            if (!next[key]) previous[key].element.remove();
        });
        keys.forEach(function (key, index) {
            var row = next[key].element;
            var before = hudRefs.inboxRows.children[index] || null;
            if (before !== row) hudRefs.inboxRows.insertBefore(row, before);
        });
        hudRefs.inboxRequests = next;
        hudRefs.inboxRequestKeys = keys;
        hudRefs.inboxRenderKey = renderKey;
        hudRefs.inboxRows.hidden = keys.length === 0;
        hudRefs.inboxEmpty.hidden = keys.length !== 0;
        hudRefs.inboxTotal.textContent = numbers.format(keys.length);
        if (focusedKey && !next[focusedKey]) {
            var nextRow = next[keys[Math.min(previousKeys.indexOf(focusedKey), keys.length - 1)]];
            (nextRow ? (nextRow.accept.disabled ? nextRow.reject : nextRow.accept) : hudRefs.inboxClose).focus();
        }
    }

    function renderNotifications(entries, forceRefresh) {
        if (!hudRefs || !hudRefs.notifications || !Array.isArray(entries)) return;
        var changed = Boolean(forceRefresh);
        var width = hudRoot.clientWidth || window.innerWidth || 0;
        var height = hudRoot.clientHeight || window.innerHeight || 0;
        var limit = window.SOWCampaignView.isCompactViewport(hudRoot, width, height) ? 1 : 3;
        var now = Date.now();
        var unexpired = activeNotifications.filter(function (item) { return item.expiresAt > now; });
        if (unexpired.length !== activeNotifications.length) changed = true;
        activeNotifications = unexpired;
        if (activeNotifications.length > limit) {
            activeNotifications.sort(function (a, b) { return b.priority - a.priority || b.createdAt - a.createdAt; });
            activeNotifications.length = limit;
            changed = true;
        }
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
            if (activeNotifications.length < limit) {
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
        hudRoot.dataset.notificationsVisible = String(activeNotifications.length > 0);
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
        var emblem = document.createElement("span");
        element.className = "sow-hud__operation";
        avatar.className = "sow-hud__operation-avatar";
        portrait.className = "sow-hud__operation-portrait";
        portrait.alt = "";
        portrait.draggable = false;
        portrait.loading = "lazy";
        bindAvatarFallback(portrait);
        emblem.className = "sow-hud__operation-emblem sow-hud__notification-emblem";
        emblem.hidden = true;
        glyph.className = "sow-hud__operation-glyph";
        copy.className = "sow-hud__operation-copy";
        name.className = "sow-hud__operation-name";
        details.className = "sow-hud__operation-details";
        troops.className = "sow-hud__operation-troops";
        actions.className = "sow-hud__operation-actions";
        avatar.append(portrait, emblem, glyph);
        copy.append(name, details);
        element.append(avatar, copy, troops, actions);
        return { element: element, portrait: portrait, emblem: emblem, glyph: glyph, name: name, details: details, troops: troops, actions: actions, actionKey: "" };
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
            var row = hudRefs.combatOperationRows[key] || createCombatOperationRow();
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
            row.element.classList.toggle("is-incoming", incoming);
            row.element.classList.toggle("is-outgoing", outgoing);
            row.element.classList.toggle("is-retreating", retreating);
            row.element.classList.toggle("is-neutral", neutral);
            renderAvatarIdentity(
                row.portrait,
                row.emblem,
                operation.avatar && typeof operation.avatar === "object"
                    ? operation.avatar
                    : { kind: "fallback" }
            );
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
        if (orderKey !== hudRefs.combatOperationsOrderKey) {
            var fragment = document.createDocumentFragment();
            nextKeys.forEach(function (key) { fragment.appendChild(nextRows[key].element); });
            hudRefs.operationRows.replaceChildren(fragment);
            hudRefs.combatOperationsOrderKey = orderKey;
        }
        hudRefs.combatOperationRows = nextRows;
        hudRefs.operations.hidden = nextKeys.length === 0;
    }

    function renderHud(forceNotifications) {
        if (!hudRoot) return;
        if (!hudState || hudState.phase !== "Playing" || !hudState.hud) {
            hudRoot.hidden = true;
            if (hudDevLayer) hudDevLayer.hidden = true;
            clearCompactHudDensity();
            activeHudPanel = null;
            betrayalOpen = false;
            pinEmote = false;
            emoteTextKey = "";
            devSidebarOpen = false;

            hudRoot.dataset.overlayOpen = "false";
            hudRoot.dataset.notificationsVisible = "false";
            if (hudRefs && hudRefs.mapMenu) hudRefs.mapMenu.classList.add("hidden");
            leaderboardRows = Object.create(null);
            leaderboardRenderKey = "";
            lastLeaderboardPlayers = [];
            if (hudRefs) {
                hudRefs.inboxRequests = Object.create(null);
                hudRefs.inboxRequestKeys = [];
                hudRefs.inboxRenderKey = "";
                if (hudRefs.inboxRows) hudRefs.inboxRows.replaceChildren();
            }
            if (hudRefs && hudRefs.rows) hudRefs.rows.replaceChildren();
            if (notificationTimer) window.clearTimeout(notificationTimer);
            notificationTimer = null;
            activeNotifications = [];
            renderCombatOperations([]);
            pendingCounterAttacks = Object.create(null);
            notificationCards.forEach(clearNotificationCard);
            notificationCursor = 0;
            mapFeedbackCursor = 0;
            clearMapFeedback();
            return;
        }
        ensureHudDom();
        hudRoot.hidden = false;
        syncCompactHudDensity();

        var hud = hudState.hud;
        var settings = hudState.settings || {};
        var settingsOpen = activeHudPanel === "settings";
        var inboxOpen = activeHudPanel === "inbox";
        var leaderboardOpen = activeHudPanel === "leaderboard";
        hudRoot.classList.toggle("is-reduced-motion", Boolean(hudState.settings && hudState.settings.reduced_motion));
        var devTools = hud.dev_tools || {};
        var showDevTools = settings.show_dev_tools_available && settings.show_dev_tools;
        var diagnosticsVisible = settings.show_fps_ping === true || showDevTools || devSidebarOpen;
        if (hudRefs.diagnostics) hudRefs.diagnostics.hidden = !diagnosticsVisible;
        if (hudRefs.devBtn) {
            hudRefs.devBtn.classList.toggle("hidden", !showDevTools);
        }
        if (hudDevLayer) hudDevLayer.hidden = !diagnosticsVisible;
        if (devTools.schema && hudRefs.devSidebar) renderDevControls(devTools);
        if (devTools.config && hudRefs.devSidebar) {
            hudRefs.devSidebar.querySelectorAll("[data-dev]").forEach(function (input) {
                if (document.activeElement === input) return;
                var value = devTools.config[input.dataset.dev];
                if (value == null) return;
                var control = input.closest("[data-dev-control]");
                if (!control) return;
                if (control.dataset.devKind === "bool") input.checked = Boolean(value);
                else if (control.dataset.devKind === "color" || control.dataset.devKind === "vector") {
                    if (input.hasAttribute("data-dev-channel")) input.value = value[Number(input.dataset.devChannel)];
                } else input.value = value;
            });
            hudRefs.devSidebar.querySelectorAll("[data-dev-picker]").forEach(function (picker) {
                var value = devTools.config[picker.dataset.devPicker];
                if (Array.isArray(value) && document.activeElement !== picker) picker.value = devColorHex(value, picker.dataset.devLinear === "1");
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
        var hasBuildingItems = false;
        var hasFleetItems = false;

        var goldText = formatHudMetric(gold);
        if (hudRefs.plateGold && hudRefs.plateGold.dataset.val !== goldText) {
            hudRefs.plateGold.textContent = goldText;
            hudRefs.plateGold.dataset.val = goldText;
        }
        var goldRateText = formatHudRate(goldRate);
        if (hudRefs.plateGoldRate && hudRefs.plateGoldRate.textContent !== goldRateText) {
            hudRefs.plateGoldRate.textContent = goldRateText;
        }
        if (hudRefs.plateGoldRate) hudRefs.plateGoldRate.classList.toggle("is-negative", goldRate < 0);
        if (hudRefs.plateGoldWrap) hudRefs.plateGoldWrap.classList.toggle("is-in-debt", gold < 0);
        var troopRateText = formatHudRate(prod);
        if (hudRefs.plateTroopRate && hudRefs.plateTroopRate.textContent !== troopRateText) {
            hudRefs.plateTroopRate.textContent = troopRateText;
        }

        var playerPlateHidden = Boolean(hud.is_spectating);
        if (hudRefs.nameplate) {
            if (hudRefs.nameplate.hidden !== playerPlateHidden) hudRefs.nameplate.hidden = playerPlateHidden;
            if (!playerPlateHidden) {
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

        if (hudRefs.fps) {
            hudRefs.fps.hidden = settings.show_fps_ping !== true;
            var hasFps = Number.isFinite(hud.fps) && hud.fps > 0;
            hudRefs.fps.textContent = SOW_t("hud.fps", { fps: hasFps ? hud.fps : "--" });
        }
        if (hudRefs.ping) {
            var hasPing = settings.show_fps_ping === true && Number.isFinite(hud.ping);
            hudRefs.ping.hidden = !hasPing;
            if (hasPing) hudRefs.ping.textContent = Math.floor(hud.ping) + "ms";
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
                    if (hov.cities > 0) bldText.push(buildingIcon("City", 1, "sow-hud__inline-icon") + " x" + hov.cities);
                    if (hov.factories > 0) bldText.push(buildingIcon("Factory", 1, "sow-hud__inline-icon") + " x" + hov.factories);
                    if (hov.ports > 0) bldText.push(buildingIcon("Port", 1, "sow-hud__inline-icon") + " x" + hov.ports);
                    if (hov.bunkers > 0) bldText.push(buildingIcon("Bunker", 1, "sow-hud__inline-icon") + " x" + hov.bunkers);
                    if (hov.farms > 0) bldText.push(buildingIcon("Farm", 1, "sow-hud__inline-icon") + " x" + hov.farms);
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
            var selectedBuilding = hud.selected_building;
            var buildingCosts = hud.building_costs || {};
            var buildUnlocks = campaignUnlocks(hud);
            hudRefs.buildingButtons.forEach(function (button) {
                var kind = button.dataset.kind || "";
                var available = campaignAllowsBuilding(buildUnlocks, kind, 1);
                button.hidden = !available;
                button.style.display = available ? "" : "none";
                var cost = Number(buildingCosts[kind.toLowerCase()]);
                var hasCost = Number.isFinite(cost) && cost > 0;
                var affordable = !hasCost || gold >= cost;
                var selected = selectedBuilding === kind;
                button.disabled = false;
                var disabled = !affordable && !selected;
                button.setAttribute("aria-disabled", "false");
                button.classList.toggle("is-unaffordable", disabled);
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
            hasBuildingItems = hudRefs.buildingButtons.some(function (button) { return !button.hidden; });
            hudRefs.buildingsStrip.style.display = !isDeploying && hasBuildingItems ? "flex" : "none";
        }
        if (hudRefs.fleetStrip) {
            var fleetPanel = hud.fleet_panel || {};
            var fleetUnlocks = campaignUnlocks(hud);
            var portLevels = Math.max(0, Number(fleetPanel.port_levels) || 0);
            var cityLevel = Math.max(0, Number(fleetPanel.city_level) || 0);
            var tradeRequiredPortLevel = Math.max(1, Number(fleetPanel.trade_required_port_level) || 1);
            var warshipRequiredPortLevel = Math.max(1, Number(fleetPanel.warship_required_port_level) || 1);
            var nukeRequiredCityLevel = Math.max(1, Number(fleetPanel.nuke_required_city_level) || 1);
            var tradeCampaignLocked = !campaignAllowsAction(fleetUnlocks, "trade_ship")
                || !campaignAllowsBuilding(fleetUnlocks, "Port", tradeRequiredPortLevel);
            var warshipCampaignLocked = !campaignAllowsAction(fleetUnlocks, "warship")
                || !campaignAllowsBuilding(fleetUnlocks, "Port", warshipRequiredPortLevel);
            var nukeCampaignLocked = !campaignAllowsAction(fleetUnlocks, "nuke")
                || !campaignAllowsBuilding(fleetUnlocks, "City", nukeRequiredCityLevel);
            var tradeUnlocked = !tradeCampaignLocked && portLevels >= tradeRequiredPortLevel;
            var warshipUnlocked = !warshipCampaignLocked && portLevels >= warshipRequiredPortLevel;
            var nukeUnlocked = !nukeCampaignLocked && cityLevel >= nukeRequiredCityLevel;
            var militaryFull = Number(fleetPanel.military_used) >= Number(fleetPanel.military_capacity);
            var warshipSelected = Boolean(hud.selected_warship_build);
            if (hudRefs.fleetTrade) {
                var hideTrade = fleetUnlocks !== null && tradeCampaignLocked;
                hudRefs.fleetTrade.hidden = hideTrade;
                hudRefs.fleetTrade.style.display = hideTrade ? "none" : "";
                var tradeCount = Math.floor(Number(fleetPanel.trade_ships) || 0);
                var tradeCapacity = Math.floor(Number(fleetPanel.trade_capacity) || 0);
                var tradeStatus = SOW_t("hud.map_action_fleet") + " · " + tradeCount + "/" + tradeCapacity;
                var tradeReason = !tradeUnlocked ? SOW_t("hud.map_action_port") + " " + tradeRequiredPortLevel : "";
                if (tradeReason) tradeStatus += " · " + tradeReason;
                hudRefs.fleetTrade.setAttribute("aria-label", tradeStatus);
                hudRefs.fleetTrade.title = tradeStatus;
                hudRefs.fleetTrade.classList.toggle("is-locked", !tradeUnlocked);
                setCampaignLock(hudRefs.fleetTrade, fleetUnlocks === null && !tradeUnlocked, tradeReason);
            }
            if (hudRefs.fleetWarship) {
                var hideWarship = fleetUnlocks !== null && warshipCampaignLocked;
                hudRefs.fleetWarship.hidden = hideWarship;
                hudRefs.fleetWarship.style.display = hideWarship ? "none" : "";
                var warshipCost = Math.max(0, Number(fleetPanel.warship_cost) || 0);
                var warshipDisabled = warshipCampaignLocked || !warshipUnlocked || militaryFull || gold < warshipCost;
                hudRefs.fleetWarship.disabled = warshipDisabled;
                hudRefs.fleetWarship.setAttribute("aria-disabled", String(warshipDisabled));
                hudRefs.fleetWarship.setAttribute("aria-pressed", String(warshipSelected));
                hudRefs.fleetWarship.classList.toggle("active", warshipSelected);
                hudRefs.fleetWarship.classList.toggle("is-locked", warshipDisabled && !warshipSelected);
                var warshipReason = !warshipUnlocked
                    ? SOW_t("lobbies.locked") + " · " + SOW_t("hud.map_action_port") + " " + warshipRequiredPortLevel
                    : militaryFull
                        ? Math.floor(Number(fleetPanel.military_used) || 0) + "/" + Math.floor(Number(fleetPanel.military_capacity) || 0)
                        : gold < warshipCost
                            ? SOW_t("hud.not_enough_gold", { cost: Math.ceil(warshipCost).toLocaleString(), available: Math.floor(gold).toLocaleString() })
                            : SOW_t("hud.map_action_port") + " " + warshipRequiredPortLevel + " · " + Math.floor(warshipCost).toLocaleString() + "g";
                var warshipLabel = SOW_t("hud.map_action_build_warship") + " · " + warshipReason;
                hudRefs.fleetWarship.setAttribute("aria-label", warshipLabel);
                hudRefs.fleetWarship.title = warshipLabel;
                setCampaignLock(hudRefs.fleetWarship, fleetUnlocks === null && warshipDisabled, warshipReason);
            }
            if (hudRefs.fleetNuke) {
                var hideNuke = fleetUnlocks !== null && nukeCampaignLocked;
                hudRefs.fleetNuke.hidden = hideNuke;
                hudRefs.fleetNuke.style.display = hideNuke ? "none" : "";
                var cooldownTicks = Math.max(0, Number(fleetPanel.nuke_cooldown_ticks) || 0);
                var nukeCost = Math.max(0, Number(fleetPanel.nuke_cost) || 0);
                var nukeReady = Boolean(fleetPanel.nuke_available) && cooldownTicks === 0 && gold >= nukeCost;
                var nukeDisabled = nukeCampaignLocked || !nukeUnlocked || !nukeReady;
                hudRefs.fleetNuke.disabled = nukeDisabled;
                hudRefs.fleetNuke.setAttribute("aria-disabled", String(nukeDisabled));
                hudRefs.fleetNuke.setAttribute("aria-pressed", String(hud.selected_nuke === "AtomBomb"));
                hudRefs.fleetNuke.classList.toggle("is-locked", nukeDisabled);
                hudRefs.fleetNuke.classList.toggle("active", hud.selected_nuke === "AtomBomb");
                var nukeReason = !nukeUnlocked
                    ? SOW_t("lobbies.locked") + " · " + SOW_t("hud.map_action_city") + " " + nukeRequiredCityLevel
                    : cooldownTicks > 0
                        ? SOW_t("lobbies.locked") + " · " + Math.ceil(cooldownTicks / 10) + "s"
                        : gold < nukeCost
                            ? SOW_t("hud.not_enough_gold", { cost: Math.ceil(nukeCost).toLocaleString(), available: Math.floor(gold).toLocaleString() })
                        : !nukeReady
                            ? SOW_t("lobbies.locked")
                            : SOW_t("hud.map_action_nuke") + " · " + Math.floor(nukeCost).toLocaleString() + "g";
                var nukeLabel = SOW_t("hud.map_action_nuke") + " · " + nukeReason;
                hudRefs.fleetNuke.setAttribute("aria-label", nukeLabel);
                hudRefs.fleetNuke.title = nukeLabel;
                setCampaignLock(hudRefs.fleetNuke, fleetUnlocks === null && nukeDisabled, nukeReason);
            }
            hasFleetItems = (hudRefs.fleetTrade && !hudRefs.fleetTrade.hidden)
                || (hudRefs.fleetWarship && !hudRefs.fleetWarship.hidden)
                || (hudRefs.fleetNuke && !hudRefs.fleetNuke.hidden);
            hudRefs.fleetStrip.style.display = isDeploying || !hasFleetItems ? "none" : "flex";
        }

        pinEmote = Boolean(hud.pin_emote);
        syncEmoteTexts(hud);
        if (hudRefs.devSidebar) {
            hudRefs.devSidebar.classList.toggle("hidden", !showDevTools || !devSidebarOpen);
        }
        if (hudRefs.devBtn) {
            hudRefs.devBtn.classList.toggle("active", devSidebarOpen);
        }


        if (hudRefs.settings) {
            hudRefs.settings.classList.toggle("hidden", !settingsOpen);
            if (settingsOpen && hudState.settings) {
                var settings = hudState.settings;
                var muteInput = hudRefs.settings.querySelector('[data-hud-setting="mute_all"]');
                var musicInput = hudRefs.settings.querySelector('[data-hud-setting="music_volume"]');
                var motionInput = hudRefs.settings.querySelector('[data-hud-setting="reduced_motion"]');
                var freeZoomInput = hudRefs.settings.querySelector('[data-hud-setting="free_zoom_out"]');
                var fpsPingInput = hudRefs.settings.querySelector('[data-hud-setting="show_fps_ping"]');
                var devToolsInput = hudRefs.settings.querySelector('[data-hud-setting="show_dev_tools"]');
                if (muteInput && document.activeElement !== muteInput) muteInput.checked = !settings.mute_all;
                if (musicInput && document.activeElement !== musicInput) musicInput.value = settings.music_volume == null ? 0.8 : settings.music_volume;
                if (motionInput && document.activeElement !== motionInput) motionInput.checked = Boolean(settings.reduced_motion);
                if (freeZoomInput && document.activeElement !== freeZoomInput) freeZoomInput.checked = Boolean(settings.free_zoom_out);
                if (fpsPingInput && document.activeElement !== fpsPingInput) fpsPingInput.checked = settings.show_fps_ping === true;
                if (devToolsInput && document.activeElement !== devToolsInput) devToolsInput.checked = Boolean(settings.show_dev_tools);
            }
        }

        if (hudRefs.inbox) {
            var inboxWasHidden = hudRefs.inbox.classList.contains("hidden");
            hudRefs.inbox.classList.toggle("hidden", !inboxOpen);
            if (inboxOpen) {
                renderInbox(hud.inbox);
                if (inboxWasHidden && hudRefs.inboxClose) hudRefs.inboxClose.focus();
            } else if (!inboxWasHidden && hudRefs.inbox.contains(document.activeElement) && hudRefs.inboxButton) {
                hudRefs.inboxButton.focus();
            }
        }
        if (hudRefs.transfer) {
            var transferWasHidden = hudRefs.transfer.classList.contains("hidden");
            hudRefs.transfer.classList.toggle("hidden", !hud.transfer);
            if (hud.transfer) {
                hudRefs.transfer.dataset.targetId = String(hud.transfer.target_id);
                renderPlayerIdentity(hudRefs.transferIdentity, hud.transfer.target_name, hud.transfer.target_avatar);
                hudRefs.transferTarget.setAttribute("aria-label", SOW_t("hud.target_player", {
                    name: hud.transfer.target_name || SOW_t("hud.player_name")
                }));
                if (transferWasHidden || hudRefs.transfer.dataset.suggestedTarget !== String(hud.transfer.target_id)) {
                    hudRefs.transfer.dataset.suggestedTarget = String(hud.transfer.target_id);
                    if (hudRefs.transferGold) hudRefs.transferGold.value = String(Math.floor(hud.transfer.suggested_gold || 0));
                    if (hudRefs.transferTroops) hudRefs.transferTroops.value = String(Math.floor(hud.transfer.suggested_troops || 0));
                }
                if (transferWasHidden) {
                    hudRefs.transferReturnFocus = document.activeElement;
                    hudRefs.transferClose.focus({ preventScroll: true });
                }
            } else if (!transferWasHidden && hudRefs.transfer.contains(document.activeElement)) {
                restoreHudFocus(hudRefs.transferReturnFocus);
            }
        }
        if (hudRefs.betrayal) {
            var betrayalWasHidden = hudRefs.betrayal.classList.contains("hidden");
            betrayalOpen = Boolean(hud.betrayal);
            hudRefs.betrayal.classList.toggle("hidden", !betrayalOpen);
            if (betrayalOpen) {
                renderPlayerIdentity(hudRefs.betrayalIdentity, hud.betrayal.ally_name, hud.betrayal.ally_avatar);
                if (betrayalWasHidden) {
                    hudRefs.betrayalReturnFocus = document.activeElement;
                    hudRefs.betrayalCancel.focus({ preventScroll: true });
                }
            } else if (!betrayalWasHidden && hudRefs.betrayal.contains(document.activeElement)) {
                restoreHudFocus(hudRefs.betrayalReturnFocus);
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
                if (hudRefs.endgameIcon) hudRefs.endgameIcon.innerHTML = hudIcon(isWinner ? "upgrade" : "attack", "sow-hud__inline-icon");
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
        renderBuildingPlacement(hud.building_placement);
        renderBuildingCard(hud.map_menu, hud.gold);
        var panelStates = { toggle_leaderboard: leaderboardOpen, toggle_inbox: inboxOpen, toggle_settings: settingsOpen, toggle_dev_sidebar: devSidebarOpen };
        hudRefs.panelButtons.forEach(function (button) {
            var expanded = Boolean(panelStates[button.dataset.command]);
            button.classList.toggle("active", expanded);
            var value = String(expanded);
            if (button.getAttribute("aria-expanded") !== value) button.setAttribute("aria-expanded", value);
        });
        hudRoot.dataset.overlayOpen = String(Boolean(
            leaderboardOpen || inboxOpen || hud.transfer || betrayalOpen ||
            surrenderModalOpen || isOver
            || devSidebarOpen || settingsOpen || mapMenuOpen
        ));
        var dockHidden = !(isDeploying || hasBuildingItems || hasFleetItems);
        if (hudRefs.dock && hudRefs.dock.hidden !== dockHidden) {
            hudRefs.dock.hidden = dockHidden;
            syncCompactHudBounds();
        }
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
            if (event.target.closest && event.target.closest("#sow-hud-placement")) {
                event.stopPropagation();
                return;
            }
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
        function handleHudClick(event) {
            var emoteChoice = event.target.closest("[data-emote-kind]");
            if (emoteChoice) {
                event.preventDefault();
                event.stopPropagation();
                send("express_emote", { session: Number(hudRefs.mapMenu.dataset.session), tile_idx: Number(hudRefs.mapMenu.dataset.tileIdx), kind: emoteChoice.dataset.emoteKind, pinned: pinEmote });
                return;
            }
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
                    var firstChoice = mapMenu.querySelector("[data-emote-kind], [data-map-action]");
                    if (firstChoice) firstChoice.focus();
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
                send("reset_dev_config", {
                    section: btn.dataset.devSection || null,
                    field: btn.dataset.devField || null
                });
            } else if (cmd === "replay_dev_vfx_preview") {
                send("replay_dev_vfx_preview");
            } else if (cmd === "dev_vfx_all") {
                send("dev_vfx_all", { on: btn.dataset.on === "1" });
            } else if (cmd === "clear_emote") {
                pinEmote = false;
                send("clear_emote");
            } else if (cmd === "open_transfer") {
                setHudPanel(null);
                send("open_transfer", { target_player_id: Number(btn.dataset.playerId) });
                renderHud();
            } else if (cmd === "close_transfer") {
                send("close_transfer");
            } else if (cmd === "send_resources" || cmd === "request_resources") {
                var transfer = hudRefs && hudRefs.transfer;
                var targetId = transfer ? Number(transfer.dataset.targetId) : 0;
                var gold = hudRefs && hudRefs.transferGold ? Number(hudRefs.transferGold.value) : 0;
                var transferTroops = hudRefs && hudRefs.transferTroops ? Number(hudRefs.transferTroops.value) : 0;
                if (!Number.isFinite(gold)) gold = 0;
                if (!Number.isFinite(transferTroops)) transferTroops = 0;
                if (targetId > 0) send(cmd, { target_player_id: targetId, gold: gold, troops: transferTroops });
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
            } else if (cmd === "nudge_building_placement") {
                var placement = hudState && hudState.hud && hudState.hud.building_placement;
                var direction = btn.dataset.direction;
                if (placement && !placement.pending && placement.can_nudge && placement.can_nudge[direction]) {
                    send("nudge_building_placement", { direction: direction });
                }
            } else if (cmd === "confirm_building_placement") {
                if (hudState && hudState.hud && hudState.hud.building_placement && hudState.hud.building_placement.can_confirm) send("confirm_building_placement");
            } else if (cmd === "cancel_building_mode") {
                send("cancel_building_mode");
            } else if (cmd === "close_building_card") {
                send("close_map_context_menu");
            } else if (cmd === "confirm_endgame_leave") {
                send("return_to_menu");
            } else if (cmd === "open_store") {
                if (window.SOW_PORTAL === "poki" || window.SOW_PORTAL === "jest") return;
                send("return_to_menu", { destination: "store" });
            } else if (cmd === "continue_observing") {
                send("continue_observing");
            }
        }
        hudRoot.addEventListener("click", handleHudClick);
        hudDevLayer.addEventListener("click", handleHudClick);
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
            if (!previousHud) emoteTextKey = "";
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

    if (window.SOW_I18N_READY && typeof window.SOW_I18N_READY.then === "function") {
        window.SOW_I18N_READY.then(function () { emoteTextKey = ""; syncEmoteTexts(hudState && hudState.hud); });
    }
    if (hudRoot) hudRoot.addEventListener("keydown", function (event) {
        if (hudRefs && hudRefs.placement && !hudRefs.placement.hidden && hudRefs.placement.contains(event.target)) {
            var directions = { ArrowUp: "up", ArrowRight: "right", ArrowDown: "down", ArrowLeft: "left" };
            var direction = directions[event.key];
            if (direction || event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                var placement = hudState && hudState.hud && hudState.hud.building_placement;
                if (placement && !placement.pending) {
                    if (direction && placement.can_nudge && placement.can_nudge[direction]) send("nudge_building_placement", { direction: direction });
                    else if (!direction) send("cancel_building_mode");
                }
                return;
            }
            // Enter and Space activate the focused button exactly once.
            if (event.key === "Enter" || event.key === " ") event.stopPropagation();
        }

        if (hudRefs && hudRefs.betrayal && !hudRefs.betrayal.classList.contains("hidden")) {
            if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                pendingCounterAttacks = Object.create(null);
                send("cancel_betrayal");
                return;
            }
            if (event.key === "Tab") {
                var buttons = Array.prototype.slice.call(hudRefs.betrayal.querySelectorAll("button:not(:disabled)"));
                var index = buttons.indexOf(document.activeElement);
                if (buttons.length && (index < 0 || event.shiftKey && index === 0 || !event.shiftKey && index === buttons.length - 1)) {
                    event.preventDefault();
                    (event.shiftKey ? buttons[buttons.length - 1] : buttons[0]).focus({ preventScroll: true });
                }
                return;
            }
        }
        if (event.key === "Escape" && hudRefs && hudRefs.transfer && !hudRefs.transfer.classList.contains("hidden")) {
            event.preventDefault();
            event.stopPropagation();
            send("close_transfer");
            return;
        }
        if (event.key === "Escape" && activeHudPanel === "inbox") {
            event.preventDefault();
            event.stopPropagation();
            setHudPanel(null);
            renderHud();
            return;
        }
        var menu = hudRefs && hudRefs.mapMenu;
        if (!menu || menu.classList.contains("hidden") || !menu.contains(event.target)) return;
        if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            if (mapMenuView !== "root") {
                mapMenuView = "root";
                renderMapMenu(hudState && hudState.hud && hudState.hud.map_menu);
                var group = menu.querySelector("[data-map-group]");
                if (group) group.focus();
            } else send("close_map_context_menu");
        } else if (["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight"].indexOf(event.key) >= 0) {
            var choices = Array.prototype.slice.call(menu.querySelectorAll("button:not(:disabled), input:not(:disabled)"));
            if (!choices.length) return;
            event.preventDefault();
            event.stopPropagation();
            var direction = event.key === "ArrowUp" || event.key === "ArrowLeft" ? -1 : 1;
            var index = choices.indexOf(document.activeElement);
            choices[(index + direction + choices.length) % choices.length].focus();
        }
    });

    window.addEventListener("sow:locale-change", function () {
        emoteTextKey = "";
        if (!hudState) return;
        var active = document.activeElement;
        var transferDraft = hudRefs && hudRefs.transfer && !hudRefs.transfer.classList.contains("hidden") ? {
            target: hudRefs.transfer.dataset.targetId,
            gold: hudRefs.transferGold.value,
            troops: hudRefs.transferTroops.value,
            focus: hudRefs.transfer.contains(active) ? active.id : null,
            returnFocus: hudRefs.transferReturnFocus
        } : null;
        var betrayalFocus = hudRefs && hudRefs.betrayal && hudRefs.betrayal.contains(active) ? active.id : null;
        var betrayalReturnFocus = hudRefs && hudRefs.betrayalReturnFocus;
        hudInitialized = false;
        hudRefs = null;
        if (hudRoot) hudRoot.replaceChildren();
        if (hudDevLayer && hudDevLayer.parentNode) {
            var freshDevLayer = hudDevLayer.cloneNode(false);
            hudDevLayer.parentNode.replaceChild(freshDevLayer, hudDevLayer);
            hudDevLayer = freshDevLayer;
        }
        renderHud(true);
        if (transferDraft && hudRefs && hudRefs.transfer.dataset.targetId === transferDraft.target) {
            hudRefs.transferGold.value = transferDraft.gold;
            hudRefs.transferTroops.value = transferDraft.troops;
            hudRefs.transferReturnFocus = transferDraft.returnFocus;
            if (transferDraft.focus) restoreHudFocus(document.getElementById(transferDraft.focus));
        }
        if (betrayalFocus && hudRefs) {
            hudRefs.betrayalReturnFocus = betrayalReturnFocus;
            restoreHudFocus(document.getElementById(betrayalFocus));
        }
    });

    window.addEventListener("resize", updateHudSafeArea);
    updateHudSafeArea();
    if (hudRoot) hudRoot.hidden = true;
    if (hudDevLayer) hudDevLayer.hidden = true;
    window.SOW_onStateUpdate(window.SOW_MENU_STATE);
})();
