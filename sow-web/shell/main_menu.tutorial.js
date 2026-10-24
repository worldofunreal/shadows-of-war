(function () {
    "use strict";

    var hudRoot = document.getElementById("sow-hud");
    var root = document.createElement("div");
    root.id = "sow-story";
    root.hidden = true;
    document.body.appendChild(root);
    var cinematicPreviewRoot = document.createElement("div");
    cinematicPreviewRoot.id = "sow-campaign-cinematic-preview";
    cinematicPreviewRoot.className = "sow-story--menu-preview";
    cinematicPreviewRoot.hidden = true;
    document.body.appendChild(cinematicPreviewRoot);
    var cinematicPreviewView = null;
    var cinematicPreviewGeneration = 0;
    var HOVER_TARGET_RADIUS = 72;

    var runtime = {
        generation: 0,
        episodeId: null,
        roster: null,
        activating: null,
        definition: null,
        machine: null,
        view: null,
        active: false,
        starting: null,
        loading: Object.create(null),
        latestHud: null,
        uiCounts: Object.create(null),
        uiPaused: false,
        cameraOnly: false,
        pausedAction: null,
        hoverEvents: 0,
        hoveredEntityId: null,
        hoverStepId: null,
        hoverStepRecorded: false,
        mapUiStepRecorded: false,
        cameraTargetFactionId: null,
        reactionHighlightPlayerId: null,
        stepRevealTimer: null,
        stepRevealStepId: null,
        stepRevealToken: 0,
        completionSent: false,
        analyticsStepId: null,
        analyticsStartedStepIds: new Set(),
        analyticsCompletedStepIds: new Set(),
        observingAfterDefeat: false,
        lastActionStepId: null,
        resolvedReactions: new Set(),
        priorChoices: Object.create(null),
        failedEpisode: null,
        lastUnlockCommand: null,
        bootEpisode: null,
        lastPhase: null,
        modalObserver: null,
        guidedScrollStepId: null,
        guidedControl: null,
        guidedAmountControls: []
    };
    var menuJourney = {
        definition: null,
        loading: null,
        accountId: null,
        machine: null,
        view: null,
        state: null,
        facts: null,
        paused: true,
        updateRevision: 0,
        lastSaveAt: 0,
        quickMatchRevision: 0,
        pendingSave: null,
        saveRetryTimer: null
    };

    function clearStepRevealTimer() {
        runtime.stepRevealToken++;
        if (runtime.stepRevealTimer !== null && typeof window.clearTimeout === "function") window.clearTimeout(runtime.stepRevealTimer);
        runtime.stepRevealTimer = null;
        runtime.stepRevealStepId = null;
    }

    function asset(path) {
        var base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
        var url = base + "/" + path.split("/").map(encodeURIComponent).join("/");
        return window.SOW_assetUrl ? window.SOW_assetUrl(url) : url;
    }

    function send(type, fields) {
        if (typeof window.SOW_menu_command !== "function") return false;
        window.SOW_menu_command(JSON.stringify(Object.assign({ type: type }, fields || {})));
        return true;
    }

    function trackExperience(name, props) {
        if (typeof window.SOW_trackExperienceEvent === "function") {
            window.SOW_trackExperienceEvent(JSON.stringify({ name: name, props: props || null }));
        }
    }

    function trackTutorialStep(action, stepId) {
        var steps = runtime.definition && runtime.definition.steps || [];
        var index = steps.findIndex(function (step) { return step.id === stepId; });
        if (index < 0 || index >= 512 || !runtime.episodeId) return;
        trackExperience("tutorial_step", {
            episode_id: runtime.episodeId,
            step_id: stepId,
            step_index: index,
            action: action
        });
    }

    function tutorialExitContext(episodeId) {
        if (!runtime.active || runtime.episodeId !== episodeId) return null;
        if (runtime.completionSent || (runtime.machine && runtime.machine.view().done)) return false;
        var stepId = runtime.analyticsStepId;
        if (!stepId || !runtime.analyticsStartedStepIds.has(stepId)
            || runtime.analyticsCompletedStepIds.has(stepId)) return {};
        var steps = runtime.definition && runtime.definition.steps || [];
        var stepIndex = steps.findIndex(function (step) { return step.id === stepId; });
        var completed = runtime.machine && runtime.machine.state && runtime.machine.state.completed || [];
        if (stepIndex < 0 || stepIndex >= 512 || completed.includes(stepId)) return {};
        return { step_id: stepId, step_index: stepIndex, action: "fail" };
    }

    function availableGold(hud) {
        var gold = Number(hud && hud.gold);
        return Number.isFinite(gold) ? Math.max(0, gold) : 0;
    }

    function tr(key) {
        var definition = runtime.definition;
        var value = typeof window.SOW_t === "function" ? window.SOW_t(key) : "[" + key + "]";
        if (!value || value === "[" + key + "]") return key;
        return window.SOWCampaign && typeof window.SOWCampaign.replaceFactionStoryNames === "function"
            ? window.SOWCampaign.replaceFactionStoryNames(value, definition, runtime.roster)
            : value;
    }

    function stepTextKey(step, suffix) {
        var namespace = runtime.definition && runtime.definition.text_namespace;
        return (typeof namespace === "string" ? namespace : "tutorial.") + step.id + suffix;
    }

    function zoomMode() {
        var nav = window.navigator || {}, data = nav.userAgentData || {};
        return window.SOWCampaign.zoomInputMode({
            androidTwa: typeof window.SOW_isAndroidTwa === "function" && window.SOW_isAndroidTwa(),
            mobile: Boolean(data.mobile),
            userAgent: nav.userAgent || "",
            platform: data.platform || nav.platform || "",
            maxTouchPoints: nav.maxTouchPoints
        });
    }

    function targetPlayer(step, hud) {
        var target = step && ((step.trigger && step.trigger.target) || (step.marker && step.marker.target));
        var players = hud && hud.players || [];
        return players.find(function (player) {
            return player && (target === "player" ? player.is_me : player.campaign_faction_id === target);
        }) || null;
    }

    function targetMapMenuOpen(step, hud) {
        var target = targetPlayer(step, hud), menu = hud && hud.map_menu;
        return Boolean(target && menu && menu.open && menu.view === "radial"
            && Number(menu.target_player_id) === Number(target.id));
    }

    function cameraTargetDistance(step, hud) {
        var player = targetPlayer(step, hud), camera = hud && hud.tutorial && hud.tutorial.camera || {};
        var zoom = Number(camera.zoom), width = Number(camera.width), height = Number(camera.height);
        if (!player || !Number.isFinite(Number(player.centroid_x)) || !Number.isFinite(Number(player.centroid_y))
            || !Number.isFinite(zoom) || zoom <= 0 || !Number.isFinite(width) || !Number.isFinite(height)) return null;
        var centerX = (width * 0.5 - Number(camera.x || 0)) / zoom;
        var centerY = (height * 0.5 - Number(camera.y || 0)) / zoom;
        return Math.hypot(Number(player.centroid_x) - centerX, Number(player.centroid_y) - centerY);
    }

    function cameraTargetAnchor(step, hud) {
        var player = targetPlayer(step, hud), camera = hud && hud.tutorial && hud.tutorial.camera || {};
        var zoom = Number(camera.zoom), scale = Math.max(Number(camera.scale) || 1, 0.01);
        var width = Number(camera.width) / scale, height = Number(camera.height) / scale;
        if (!player || !Number.isFinite(Number(player.centroid_x)) || !Number.isFinite(Number(player.centroid_y))
            || !Number.isFinite(zoom) || zoom <= 0 || !Number.isFinite(width) || !Number.isFinite(height)) return null;
        var x = (Number(camera.x || 0) + Number(player.centroid_x) * zoom) / scale;
        var y = (Number(camera.y || 0) + Number(player.centroid_y) * zoom) / scale;
        var margin = Math.min(80, Math.max(24, Math.min(width, height) * 0.35));
        return {
            x: Math.max(margin, Math.min(width - margin, x)),
            y: Math.max(margin, Math.min(height - margin, y)),
            offscreen: x < 0 || x > width || y < 0 || y > height
        };
    }

    function metricText(key, values) {
        var text = tr(key);
        Object.keys(values).forEach(function (name) {
            text = text.replace(new RegExp("\\{" + name + "\\}", "g"), String(values[name]));
        });
        return text;
    }

    function renderContext(step, hud, anchor, machineView) {
        var context = {
            anchor: anchor,
            reducedMotion: Boolean(hud && hud.settings && hud.settings.reduced_motion),
            direction: document.documentElement.dir
        };
        if (step && step.guide) {
            context.zoomMode = zoomMode();
            var facts = hud && hud.tutorial && hud.tutorial.facts || {};
            var hintKey = "";
            if (step.guide.gesture === "zoom_in" || step.guide.gesture === "zoom_out") {
                var zoomHintKey = stepTextKey(step, "_" + context.zoomMode + "_hint");
                var zoomLabelKey = stepTextKey(step, "_" + context.zoomMode + "_label");
                if (hasText(zoomHintKey)) {
                    context.hintOverride = tr(zoomHintKey);
                    context.gestureHint = context.hintOverride;
                }
                if (hasText(zoomLabelKey)) context.gestureLabel = tr(zoomLabelKey);
                var zoomTarget = step.trigger && step.trigger.type === "zoom_out_complete" ? window.SOWCampaign.zoomOutTarget(facts, step.trigger.value)
                    : step.trigger && step.trigger.type === "zoom_in_complete" ? facts.camera_zoom_target : null;
                if (zoomTarget != null && Number.isFinite(Number(zoomTarget)) && Number.isFinite(Number(facts.camera_zoom))) {
                    context.guideMetric = {
                        current: Number(facts.camera_zoom),
                        target: Number(zoomTarget),
                        direction: step.guide.gesture === "zoom_in" ? "min" : "max"
                    };
                }
            } else if (["drag", "hover"].includes(step.guide.gesture)) {
                hintKey = stepTextKey(step, "_" + (context.zoomMode === "pinch" ? "mobile" : "desktop") + "_hint");
                if (step.trigger && step.trigger.type === "camera_target" && Number.isFinite(Number(step.trigger.distance))) {
                    var distance = cameraTargetDistance(step, hud);
                    var targetRadius = Number(facts.camera_target_radius);
                    if (!Number.isFinite(targetRadius) || targetRadius <= 0) targetRadius = step.trigger.distance;
                    var progressKey = stepTextKey(step, "_progress");
                    if (Number.isFinite(distance) && hasText(progressKey)) context.guideMetric = { text: metricText(progressKey, {
                        current: Math.ceil(distance), target: Math.ceil(targetRadius)
                    }) };
                }
            }
            if (step.guide.gesture === "tap" && step.guide.show_label !== false) {
                var expansionResumed = step.paused_action === "expand_once_then_resume"
                    && machineView && machineView.paused_action == null;
                var actionSuffix = expansionResumed ? "_continue_action"
                    : context.zoomMode === "pinch" ? "_mobile_action" : "_desktop_action";
                var actionKey = stepTextKey(step, actionSuffix);
                var action = window.SOWCampaign.guideAction(step);
                var fallbackKey = action === "upgrade_progress" ? "tutorial.hand_upgrade_progress"
                    : action === "interact" ? "tutorial.hand_" + (context.zoomMode === "pinch" ? "tap" : "click")
                        : "tutorial.hand_" + (context.zoomMode === "pinch" ? "tap" : "click") + "_" + action;
                context.gestureLabel = hasText(actionKey) ? tr(actionKey) : tr(fallbackKey);
            }
            if (hintKey && tr(hintKey) !== hintKey) {
                context.hintOverride = tr(hintKey);
                if (step.guide.gesture === "hover") context.gestureHint = context.hintOverride;
            }
        }
        if (step && step.type === "objective") {
            var guideAction = step.guide ? window.SOWCampaign.guideAction(step) : "";
            if (!step.guide || guideAction === "upgrade_progress") {
                context.statusOnly = true;
                context.statusLabel = context.gestureLabel || (step.trigger && step.trigger.type === "structure_level"
                    ? tr("hud.building_in_progress") : tr(step.hint_key || step.body_key));
            }
        }
        return context;
    }

    function fetchJson(path) {
        return fetch(asset(path), { cache: "no-store", credentials: "same-origin" }).then(function (response) {
            if (!response.ok) throw new Error(path + " returned " + response.status);
            return response.json();
        });
    }

    function hasText(key) {
        return typeof window.SOW_hasText === "function" && window.SOW_hasText(key);
    }

    function loadEpisode(episodeId) {
        if (runtime.loading[episodeId]) return runtime.loading[episodeId];
        var request = Promise.all([
            fetchJson("campaign/" + episodeId + ".json"),
            fetchJson("campaign/" + episodeId + ".triggers.json")
        ]).then(function (files) {
            var roster = files[0], definition = files[1];
            if (!window.SOWCampaign || !window.SOWCampaignView) throw new Error("Campaign runtime is missing.");
            if (definition.episode_id !== episodeId) throw new Error("Campaign filename and episode ID do not match.");
            var result = window.SOWCampaign.validate(definition, roster, { hasText: hasText });
            if (result.errors.length) throw new Error(result.errors.map(function (issue) { return issue.message; }).join("\n"));
            return { roster: roster, definition: definition };
        });
        runtime.loading[episodeId] = request;
        function releaseRequest() {
            if (runtime.loading[episodeId] === request) delete runtime.loading[episodeId];
        }
        request.then(releaseRequest, releaseRequest);
        return request;
    }

    function closeCampaignCinematic() {
        cinematicPreviewGeneration++;
        if (cinematicPreviewView) cinematicPreviewView.destroy();
        cinematicPreviewView = null;
        cinematicPreviewRoot.hidden = true;
        cinematicPreviewRoot.classList.remove("sow-story--menu-preview");
    }

    function watchCampaignCinematic(episodeId) {
        episodeId = String(episodeId || "");
        if (!/^[a-z][a-z0-9_]{0,63}$/.test(episodeId)) return;
        if (cinematicPreviewView) cinematicPreviewView.destroy();
        cinematicPreviewView = null;
        var generation = ++cinematicPreviewGeneration;
        cinematicPreviewRoot.classList.add("sow-story--menu-preview");
        cinematicPreviewRoot.hidden = true;
        loadEpisode(episodeId).then(function (data) {
            if (generation !== cinematicPreviewGeneration) return;
            var entry = (data.definition.steps || []).find(function (step) { return step.id === data.definition.entry; });
            if (!entry || entry.type !== "scene" || entry.presentation !== "cinematic" || !entry.video_src) {
                throw new Error("This episode has no opening cinematic.");
            }
            var translate = function (key) {
                var value = typeof window.SOW_t === "function" ? window.SOW_t(key) : "[" + key + "]";
                if (!value || value === "[" + key + "]") return key;
                return window.SOWCampaign.replaceFactionStoryNames(value, data.definition, data.roster);
            };
            cinematicPreviewView = window.SOWCampaignView.mount(cinematicPreviewRoot, {
                translate: translate,
                asset: asset,
                roster: function () { return data.roster; },
                onContinue: closeCampaignCinematic,
                onDismiss: closeCampaignCinematic
            });
            var machine = window.SOWCampaign.create(data.definition, data.definition.entry, data.roster);
            cinematicPreviewView.render(machine.view(), {
                direction: document.documentElement.dir || "ltr",
                localeScript: document.documentElement.dataset.localeScript || "latin",
                reducedMotion: Boolean(window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches)
            });
        }).catch(function (error) {
            if (generation !== cinematicPreviewGeneration) return;
            console.error("[SOW CAMPAIGN CINEMATIC]", error);
            closeCampaignCinematic();
        });
    }

    function showError(error, episodeId, retryAction, dismissAction) {
        console.error("[SOW CAMPAIGN]", error);
        if (runtime.latestHud && runtime.latestHud.tutorial && runtime.latestHud.tutorial.active) {
            runtime.uiPaused = true;
            runtime.pausedAction = null;
            send("set_tutorial_paused", { paused: true, camera_only: runtime.cameraOnly, paused_action: null });
        }
        var previous = document.getElementById("sow-campaign-error");
        if (previous) previous.remove();
        var focusBefore = document.activeElement;
        var panel = document.createElement("section");
        panel.id = "sow-campaign-error";
        panel.className = "sow-campaign-error";
        panel.setAttribute("role", "alertdialog");
        var message = document.createElement("p");
        message.textContent = tr("tutorial.unavailable");
        var retry = document.createElement("button");
        retry.type = "button";
        retry.textContent = tr("tutorial.retry");
        retry.addEventListener("click", function () {
            panel.remove(); runtime.failedEpisode = null; delete runtime.loading[episodeId];
            if (retryAction) { retryAction(); return; }
            if (runtime.latestHud && runtime.latestHud.tutorial && runtime.latestHud.tutorial.active && runtime.latestHud.tutorial.episode_id === episodeId) {
                runtime.activating = null; activate(runtime.latestHud.tutorial, runtime.latestHud);
            } else startEpisode(episodeId, false);
        });
        panel.append(message, retry);
        if (dismissAction) {
            var dismiss = document.createElement("button");
            dismiss.type = "button";
            dismiss.textContent = tr("tutorial.continue");
            dismiss.addEventListener("click", function () {
                panel.remove(); dismissAction();
                if (focusBefore && focusBefore.isConnected) focusBefore.focus({ preventScroll: true });
            });
            panel.appendChild(dismiss);
        }
        document.body.appendChild(panel);
        retry.focus({ preventScroll: true });
    }

    function makeView() {
        if (runtime.view) runtime.view.destroy();
        if (!root.isConnected) document.body.appendChild(root);
        runtime.view = window.SOWCampaignView.mount(root, {
            translate: tr,
            asset: asset,
            roster: function () { return runtime.roster; },
            onContinue: continueScene,
            onChoice: choose,
            onFocus: focusMarker,
            onCinematicChange: function (active) {
                if (typeof window.SOW_menu_music_cinematic === "function") {
                    window.SOW_menu_music_cinematic(active);
                }
            },
            onDismiss: null
        });
    }

    function startEpisode(episodeId, fromBoot) {
        episodeId = String(episodeId || "");
        if (!/^[a-z][a-z0-9_]{0,63}$/.test(episodeId) || runtime.starting === episodeId || (runtime.active && runtime.episodeId === episodeId)) return;
        pauseMenuGuide();
        runtime.priorChoices = Object.create(null);
        runtime.lastUnlockCommand = null;
        var generation = ++runtime.generation;
        runtime.starting = episodeId;
        runtime.failedEpisode = null;
        var error = document.getElementById("sow-campaign-error");
        if (error) error.remove();
        loadEpisode(episodeId).then(function (data) {
            if (generation !== runtime.generation) return;
            runtime.episodeId = episodeId;
            runtime.roster = data.roster;
            runtime.definition = data.definition;
            runtime.machine = null;
            runtime.active = false;
            runtime.uiCounts = Object.create(null);
            runtime.uiPaused = null;
            runtime.cameraOnly = null;
            runtime.pausedAction = null;
            runtime.observingAfterDefeat = false;
            runtime.hoverEvents = 0;
            runtime.cameraTargetFactionId = null;
            runtime.hoveredEntityId = null;
            runtime.hoverStepId = null;
            runtime.hoverStepRecorded = false;
            runtime.markerId = undefined;
            runtime.completionSent = false;
            runtime.analyticsStepId = null;
            runtime.analyticsStartedStepIds = new Set();
            runtime.analyticsCompletedStepIds = new Set();
            runtime.lastActionStepId = null;
            runtime.resolvedReactions = new Set();
            if (!send("start_campaign_episode", { episode_id: episodeId, roster: data.roster, match: data.definition.settings })) {
                throw new Error("Campaign could not reach the game.");
            }
        }).catch(function (reason) {
            if (generation === runtime.generation) { runtime.failedEpisode = episodeId; showError(reason, episodeId); }
        }).then(function () {
            if (generation === runtime.generation) runtime.starting = null;
        });
        if (fromBoot) runtime.bootEpisode = episodeId;
    }

    function activate(tutorial, hud) {
        if (runtime.machine && runtime.episodeId === tutorial.episode_id) {
            runtime.latestHud = hud;
            update(hud);
            return;
        }
        var episodeId = String(tutorial.episode_id || "");
        if (runtime.failedEpisode === episodeId) return;
        if (runtime.activating === episodeId) return;
        runtime.activating = episodeId;
        var generation = runtime.generation;
        var episode = runtime.episodeId === episodeId && runtime.definition
            ? Promise.resolve({ definition: runtime.definition, roster: runtime.roster })
            : loadEpisode(episodeId);
        episode.then(function (data) {
            if (generation !== runtime.generation || !runtime.latestHud || !runtime.latestHud.tutorial || runtime.latestHud.tutorial.episode_id !== episodeId) return;
            if (runtime.machine && runtime.episodeId === episodeId) { update(runtime.latestHud); return; }
            runtime.episodeId = episodeId;
            runtime.roster = data.roster;
            runtime.definition = data.definition;
            runtime.machine = window.SOWCampaign.create(data.definition, undefined, data.roster, undefined, { onStepComplete: applyStepEmote });
            runtime.active = true;
            window.SOW_emote_text_overrides = data.definition.emote_texts || {};
            runtime.lastUnlockCommand = null;
            runtime.uiCounts = Object.create(null);
            runtime.uiPaused = null;
            runtime.cameraOnly = null;
            runtime.pausedAction = null;
            runtime.hoverEvents = 0;
            runtime.cameraTargetFactionId = null;
            var hovered = runtime.latestHud.hovered;
            runtime.hoveredEntityId = hovered && !hovered.is_me && Number.isInteger(Number(hovered.id))
                ? Number(hovered.id) : null;
            runtime.hoverStepId = null;
            runtime.hoverStepRecorded = false;
            runtime.markerId = undefined;
            runtime.completionSent = false;
            runtime.analyticsStepId = null;
            runtime.analyticsStartedStepIds = new Set();
            runtime.analyticsCompletedStepIds = new Set();
            runtime.lastActionStepId = null;
            runtime.resolvedReactions = new Set();
            makeView();
            update(runtime.latestHud);
        }).catch(function (error) {
            if (generation === runtime.generation) { runtime.failedEpisode = episodeId; showError(error, episodeId); }
        }).then(function () {
            if (runtime.activating === episodeId) runtime.activating = null;
        });
    }

    function placementGuideView(model, hud) {
        if (!model || !model.step || model.step.paused_action !== "build_until_started" || !hud || !hud.building_placement || hud.building_placement.pending || !hud.building_placement.can_confirm) return model;
        var step = Object.assign({}, model.step, {
            guide: { kind: "world", target: "confirm_building_placement", gesture: "tap" }
        });
        return Object.assign({}, model, { step: step });
    }

    function anchorFor(step, hud, guideFacts) {
        if (!step) return null;
        var tutorial = hud && hud.tutorial || {};
        var triggerType = step.trigger && step.trigger.type;
        if (!step.guide) {
            if (step.type !== "objective") return null;
            if (triggerType === "structure_level") {
                var passiveKind = step.trigger.kind || ({ city_level: "City", port_level: "Port" })[triggerType];
                var passiveBuilding = tutorial.upgrading_buildings && tutorial.upgrading_buildings[passiveKind];
                if (passiveBuilding && Number.isFinite(Number(passiveBuilding.x)) && Number.isFinite(Number(passiveBuilding.y))) {
                    return { x: Number(passiveBuilding.x), y: Number(passiveBuilding.y) };
                }
            }
            var passivePlayer = tutorial.player;
            return passivePlayer && Number.isFinite(Number(passivePlayer.x)) && Number.isFinite(Number(passivePlayer.y))
                ? { x: Number(passivePlayer.x), y: Number(passivePlayer.y), cameraTracked: true } : null;
        }
        var guide = step.guide;
        var factionAction = step.trigger && ["attack", "contact", "alliance", "defeated", "fleet"].includes(triggerType);
        if (guide.kind === "world" && factionAction
            && ["target_action", "nameplate", "player"].includes(guide.target)) {
            var factionNameplate = tutorial.target_nameplate;
            if (factionNameplate && Number.isFinite(Number(factionNameplate.x)) && Number.isFinite(Number(factionNameplate.y))) {
                return { x: Number(factionNameplate.x), y: Number(factionNameplate.y), offscreen: Boolean(factionNameplate.offscreen), cameraTracked: true };
            }
            return null;
        }

        if (guide.kind === "world" && guide.target === "upgrade_building") {
            var trigger = step.trigger || {};
            var buildingKind = trigger.kind || ({ city_level: "City", port_level: "Port" })[trigger.type];
            var anchors = ["structure_level", "city_level", "port_level"].includes(trigger.type)
                ? tutorial.upgrading_buildings : tutorial.upgrade_buildings;
            var buildingAnchor = anchors && anchors[buildingKind];
            if (!buildingAnchor || !Number.isFinite(Number(buildingAnchor.x)) || !Number.isFinite(Number(buildingAnchor.y))) {
                var upgradeWaitPlayer = trigger.type === "structure_level" && tutorial.player;
                return upgradeWaitPlayer && Number.isFinite(Number(upgradeWaitPlayer.x)) && Number.isFinite(Number(upgradeWaitPlayer.y))
                    ? { x: Number(upgradeWaitPlayer.x), y: Number(upgradeWaitPlayer.y), cameraTracked: true } : null;
            }
            return { x: Number(buildingAnchor.x), y: Number(buildingAnchor.y) };
        }

        if (guide.kind === "world" && guide.target === "player" && step.trigger
            && ["camera_target", "hover", "zoom_in_complete"].includes(step.trigger.type)) {
            if (["hover", "zoom_in_complete"].includes(step.trigger.type)) {
                var nameplate = tutorial.nameplate;
                if (nameplate && Number.isFinite(Number(nameplate.x)) && Number.isFinite(Number(nameplate.y))) {
                    var pointer = tutorial.pointer;
                    if (step.trigger.type === "hover" && pointer
                        && Number.isFinite(Number(pointer.x)) && Number.isFinite(Number(pointer.y))
                        && Math.hypot(Number(pointer.x) - Number(nameplate.x), Number(pointer.y) - Number(nameplate.y)) <= HOVER_TARGET_RADIUS) {
                        return { x: Number(pointer.x), y: Number(pointer.y) };
                    }
                    return { x: Number(nameplate.x), y: Number(nameplate.y), cameraTracked: true };
                }
            }
            var cameraTarget = cameraTargetAnchor(step, hud);
            if (cameraTarget) {
                cameraTarget.cameraTracked = true;
                return cameraTarget;
            }
        }
        if (guide.kind === "world" && guide.target === "target_action"
            && step.trigger && step.trigger.type === "defeated") {
            var targetNameplate = tutorial.target_nameplate;
            if (targetNameplate && Number.isFinite(Number(targetNameplate.x)) && Number.isFinite(Number(targetNameplate.y))) {
                return { x: Number(targetNameplate.x), y: Number(targetNameplate.y), offscreen: Boolean(targetNameplate.offscreen), cameraTracked: true };
            }
        }
        var result;
        if (guide.kind === "world") {
            var point = tutorial[guide.target];
            if (!point || !Number.isFinite(Number(point.x)) || !Number.isFinite(Number(point.y))) return null;
            result = { x: Number(point.x), y: Number(point.y) };
        } else {
            if (guide.target === "map_transfer" && step.marker && !targetMapMenuOpen(step, hud)) {
                var playerPoint = tutorial.player;
                return playerPoint && Number.isFinite(Number(playerPoint.x)) && Number.isFinite(Number(playerPoint.y))
                    ? { x: Number(playerPoint.x), y: Number(playerPoint.y) } : null;
            }
            var source = window.SOWCampaign.resolveUiTarget(guide.target, document, runtime.episodeId, guideFacts);
            if (!source || source.disabled || source.getClientRects().length === 0) return null;
            var spotlightPanel = step.id === "boudica_transfer_send" && source.closest
                ? source.closest("#sow-hud-transfer") : null;
            if (step.id === "boudica_transfer_send" && (!spotlightPanel || spotlightPanel.getClientRects().length === 0)) return null;
            result = window.SOWCampaign.resolveUiAnchor(source, spotlightPanel);
            if (guide.target === "attack_ratio" && step.trigger && step.trigger.type === "ui"
                && step.trigger.action === "attack_ratio" && source.type === "range") {
                var sliderMin = Number(source.min), sliderMax = Number(source.max);
                var ratioTarget = sliderMin + (sliderMax - sliderMin) * Number(step.trigger.value || 1);
                result = window.SOWCampaign.resolveUiRangeGuideAnchor(source, Number(source.value), ratioTarget);
                if (!result) return null;
            }
        }
        if (guide.gesture === "drag" && guide.to) {
            if (guide.kind === "world") {
                var worldDestination = tutorial[guide.to];
                if (!worldDestination || !Number.isFinite(Number(worldDestination.x)) || !Number.isFinite(Number(worldDestination.y))) return null;
                result.toX = Number(worldDestination.x); result.toY = Number(worldDestination.y);
            } else {
                var destination = window.SOWCampaign.resolveUiTarget(guide.to, document, runtime.episodeId);
                if (!destination || destination.disabled || !destination.getClientRects().length) return null;
                var targetAnchor = window.SOWCampaign.resolveUiAnchor(destination);
                result.toX = targetAnchor.x;
                result.toY = targetAnchor.y;
            }
        }
        return result;
    }

    function revealGuidedUiTarget(step) {
        if (!step || !step.guide || step.guide.kind !== "ui" || runtime.guidedScrollStepId === step.id
            || !window.SOWCampaign || typeof window.SOWCampaign.resolveUiTarget !== "function") return;
        var target = window.SOWCampaign.resolveUiTarget(step.guide.target, document, runtime.episodeId);
        var strip = target && target.closest(".sow-hud__dock-actions");
        if (!target || target.disabled || !strip || strip.scrollWidth <= strip.clientWidth + 1) return;
        var stripRect = strip.getBoundingClientRect();
        var targetRect = target.getBoundingClientRect();
        if (targetRect.left >= stripRect.left && targetRect.right <= stripRect.right) {
            runtime.guidedScrollStepId = step.id;
            return;
        }
        target.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "instant" });
        runtime.guidedScrollStepId = step.id;
    }

    function syncTransferGuide(step) {
        var highlightAmounts = Boolean(step && step.guide && step.guide.kind === "ui" && step.guide.target === "transfer_send");
        var guideTarget = step && step.guide && step.guide.kind === "ui" ? step.guide.target : null;
        var target = ["transfer_troops", "transfer_gold", "transfer_send"].includes(guideTarget)
            && window.SOWCampaign && typeof window.SOWCampaign.resolveUiTarget === "function"
            ? window.SOWCampaign.resolveUiTarget(guideTarget, document, runtime.episodeId)
            : null;
        var amountControls = [];
        if (highlightAmounts && window.SOWCampaign && typeof window.SOWCampaign.resolveUiTarget === "function") {
            amountControls = ["transfer_troops", "transfer_gold"].map(function (key) {
                return window.SOWCampaign.resolveUiTarget(key, document, runtime.episodeId);
            }).filter(Boolean);
        }
        runtime.guidedAmountControls.forEach(function (input) {
            if (!amountControls.includes(input)) input.classList.remove("sow-tutorial-guide-target");
        });
        amountControls.forEach(function (input) { input.classList.add("sow-tutorial-guide-target"); });
        runtime.guidedAmountControls = amountControls;
        if (runtime.guidedControl !== target) {
            if (runtime.guidedControl) runtime.guidedControl.classList.remove("sow-tutorial-guide-target");
            runtime.guidedControl = target;
        }
        if (target) target.classList.add("sow-tutorial-guide-target");
    }

    function markerTargetFor(step) {
        var facts = runtime.latestHud && runtime.latestHud.tutorial && runtime.latestHud.tutorial.facts || {};
        var eliminated = facts.eliminated_faction_ids || [];
        if (step.trigger && Array.isArray(step.trigger.targets)) {
            var observed = step.trigger.type === "contact" ? facts.contact_faction_ids || [] : facts.defeated_faction_ids || [];
            var remaining = step.trigger.targets.find(function (target) { return !observed.includes(target) && !eliminated.includes(target); });
            if (remaining) return remaining;
            return null;
        }
        if (step.trigger && ["attack", "contact", "alliance", "defeated", "fleet", "resource_transfer"].includes(step.trigger.type)) {
            var targetFaction = step.trigger.target || step.trigger.recipient;
            if (targetFaction && targetFaction !== "player") return eliminated.includes(targetFaction) ? null : targetFaction;
        }
        if (step.guide && step.guide.kind === "world" && step.guide.target === "target_action" && step.trigger && step.trigger.target) {
            return eliminated.includes(step.trigger.target) ? null : step.trigger.target;
        }
        if (step.trigger && ["camera_target", "hover", "zoom_in_complete"].includes(step.trigger.type) && step.trigger.target) {
            return eliminated.includes(step.trigger.target) ? null : step.trigger.target;
        }
        return step.marker && !eliminated.includes(step.marker.target) ? step.marker.target : null;
    }

    function update(hud) {
        if (!runtime.machine || runtime.modalOpen || !hud || !hud.tutorial || !hud.tutorial.active) return;
        if (!root.isConnected) document.body.appendChild(root);
        var tutorial = hud.tutorial;
        var currentStep = runtime.machine.view().step;
        var stepChanged = runtime.hoverStepId !== currentStep.id;
        if (stepChanged) {
            runtime.hoverStepId = currentStep.id;
            runtime.hoverStepRecorded = false;
            runtime.mapUiStepRecorded = false;
        }
        var hoveredEntityId = hud.hovered && !hud.hovered.is_me && Number.isInteger(Number(hud.hovered.id))
            ? Number(hud.hovered.id) : null;
        var hoverTarget = currentStep.trigger && currentStep.trigger.type === "hover" && currentStep.trigger.target;
        var tutorialTargetHovered = Boolean(tutorial.facts && tutorial.facts.tutorial_target_hovered);
        var nameplate = tutorial.nameplate;
        var hoverAnchor = hoverTarget && nameplate && Number.isFinite(Number(nameplate.x)) && Number.isFinite(Number(nameplate.y))
            ? { x: Number(nameplate.x), y: Number(nameplate.y) }
            : hoverTarget ? cameraTargetAnchor(currentStep, hud) : null;
        var pointer = tutorial.pointer;
        var pointerNearTarget = Boolean(hoverAnchor && !hoverAnchor.offscreen && pointer
            && Number.isFinite(Number(pointer.x)) && Number.isFinite(Number(pointer.y))
            && Math.hypot(Number(pointer.x) - hoverAnchor.x, Number(pointer.y) - hoverAnchor.y) <= HOVER_TARGET_RADIUS);
        var hoverMatchesTarget = tutorialTargetHovered || !hoverTarget || (hud.players || []).some(function (player) {
            return player && Number(player.id) === hoveredEntityId && (hoverTarget === "player" ? player.is_me : player.campaign_faction_id === hoverTarget);
        }) || pointerNearTarget;
        var hoverDetected = tutorialTargetHovered || (hoveredEntityId !== null && hoverMatchesTarget) || pointerNearTarget;
        if (currentStep.trigger && currentStep.trigger.type === "hover" && !runtime.hoverStepRecorded
            && hoverDetected
            && (stepChanged || hoveredEntityId !== runtime.hoveredEntityId || pointerNearTarget || tutorialTargetHovered)) {
            runtime.hoverStepRecorded = true;
            runtime.hoverEvents++;
        }
        runtime.hoveredEntityId = hoveredEntityId;
        var mapMenu = hud.map_menu;
        var selectedBuildingKind = mapMenu && mapMenu.open && mapMenu.view === "building_details" && mapMenu.building
            ? String(mapMenu.building.kind || "") : "";
        var facts = Object.assign({}, tutorial.facts || {}, {
            hover_events: runtime.hoverEvents,
            touch_controls: zoomMode() === "pinch" ? 1 : 0,
            selected_building_kind: selectedBuildingKind,
            attack_ratio: Number(hud.attack_ratio)
        });
        if (currentStep.trigger && currentStep.trigger.type === "ui"
            && currentStep.marker && currentStep.marker.target && !runtime.mapUiStepRecorded) {
            var mapAction = currentStep.trigger.action;
            var transferTarget = targetPlayer(currentStep, hud);
            var mapActionComplete = mapAction === "map_menu_open" ? targetMapMenuOpen(currentStep, hud)
                : mapAction === "map_transfer" && transferTarget && hud.transfer
                    && Number(hud.transfer.target_id) === Number(transferTarget.id);
            if (mapActionComplete) {
                runtime.uiCounts[mapAction] = (Number(runtime.uiCounts[mapAction]) || 0) + 1;
                runtime.mapUiStepRecorded = true;
            }
        }
        if (currentStep.trigger && (currentStep.trigger.type === "camera_target"
            || (currentStep.trigger.type === "zoom_in_complete" && currentStep.trigger.target))) {
            var targetDistance = cameraTargetDistance(currentStep, hud);
            if (Number.isFinite(targetDistance)) facts.camera_target_distance = targetDistance;
        }
        var machineView = runtime.machine.update(facts, runtime.uiCounts);
        var campaignSessionId = Number(tutorial.campaign_session_id) || 0;
        var unlockCommandKey = campaignSessionId + ":" + JSON.stringify(machineView.unlocks || { buildings: {}, actions: [] });
        if (campaignSessionId > 0 && runtime.lastUnlockCommand !== unlockCommandKey
            && send("set_campaign_unlocks", { campaign_session_id: campaignSessionId, unlocks: machineView.unlocks || { buildings: {}, actions: [] } })) {
            runtime.lastUnlockCommand = unlockCommandKey;
        }
        if (machineView.step) {
            var visibleStep = runtime.definition.steps.find(function (step) {
                return step.id === machineView.step.id;
            });
            if (visibleStep && runtime.analyticsStepId !== visibleStep.id) {
                var previousStepId = runtime.analyticsStepId;
                if (!previousStepId) {
                    trackExperience("tutorial_start", { episode_id: runtime.episodeId });
                }
                if (previousStepId && runtime.machine.state.completed.includes(previousStepId)
                    && !runtime.analyticsCompletedStepIds.has(previousStepId)) {
                    runtime.analyticsCompletedStepIds.add(previousStepId);
                    trackTutorialStep("complete", previousStepId);
                }
                runtime.analyticsStepId = visibleStep.id;
                if (!runtime.analyticsStartedStepIds.has(visibleStep.id)) {
                    runtime.analyticsStartedStepIds.add(visibleStep.id);
                    trackTutorialStep("start", visibleStep.id);
                }
            }
            if (machineView.done && visibleStep && runtime.machine.state.completed.includes(visibleStep.id)
                && !runtime.analyticsCompletedStepIds.has(visibleStep.id)) {
                runtime.analyticsCompletedStepIds.add(visibleStep.id);
                trackTutorialStep("complete", visibleStep.id);
            }
        }
        if (machineView.reactionData && machineView.reactionData.outcome) {
            resolveReaction(machineView.reactionData, machineView.reactionTarget, null, hud);
        }
        if (machineView.reactionData && machineView.reactionTarget !== runtime.cameraReactionTarget) {
            var faction = (hud.players || []).find(function (item) { return item && item.campaign_faction_id === machineView.reactionTarget; });
            if (faction && Number.isFinite(Number(faction.centroid_x)) && Number.isFinite(Number(faction.centroid_y))) {
                if (send("focus_world", { x: Number(faction.centroid_x) + 0.5, y: Number(faction.centroid_y) + 0.5 })) {
                    runtime.cameraReactionTarget = machineView.reactionTarget;
                }
                var highlightId = Number(faction.id);
                if (Number.isInteger(highlightId) && runtime.reactionHighlightPlayerId !== highlightId) {
                    runtime.reactionHighlightPlayerId = highlightId;
                    send("set_dialog_border_highlight", { player_id: highlightId });
                }
            }
        } else if (!machineView.reactionData) {
            runtime.cameraReactionTarget = null;
            if (runtime.reactionHighlightPlayerId !== null) {
                runtime.reactionHighlightPlayerId = null;
                send("set_dialog_border_highlight", { player_id: null });
            }
        }
        if (machineView.choices && machineView.choices.length) {
            var gold = availableGold(hud);
            machineView.choices = machineView.choices.map(function (choice) {
                if (Number(choice.gold_cost || 0) <= 0) return choice;
                return Object.assign({}, choice, {
                    gold_available: Math.max(0, gold),
                    gold_insufficient: Number(choice.gold_cost || 0) > gold
                });
            });
        }
        runtime.priorChoices = Object.assign(Object.create(null), runtime.machine.state.choices);
        if (machineView.step.id !== runtime.lastActionStepId) {
            runtime.lastActionStepId = machineView.step.id;
            if (Number.isFinite(machineView.step.attack_ratio_on_enter)) send("set_attack_ratio", { ratio: machineView.step.attack_ratio_on_enter });
            if (machineView.step.campaign_attack_on_enter) {
                send("set_campaign_attack_permission", machineView.step.campaign_attack_on_enter);
            }
            if (machineView.step.campaign_assault_on_enter) {
                var assault = machineView.step.campaign_assault_on_enter;
                send("activate_campaign_assault", {
                    attacker_team: assault.attacker_team,
                    target_faction_id: assault.target,
                    preserve_relation: assault.preserve_relation === true,
                    reinforcement: assault.reinforcement || null,
                    hold_last_tile: assault.hold_last_tile === true
                });
            }
        }
        if (!runtime.observingAfterDefeat && (facts.eliminated_faction_ids || []).includes("player")) {
            runtime.observingAfterDefeat = true;
            send("continue_observing");
        }
        var targetStep = machineView.step;
        var targetFactionId = targetStep.type === "choice" && targetStep.marker
            ? markerTargetFor(targetStep)
            : targetStep.guide && targetStep.guide.kind === "world"
            && ["target_action", "nameplate", "player"].includes(targetStep.guide.target)
            && targetStep.trigger && ["attack", "contact", "alliance", "defeated", "fleet"].includes(targetStep.trigger.type)
            ? markerTargetFor(targetStep) : null;
        if (targetFactionId !== runtime.cameraTargetFactionId) {
            runtime.cameraTargetFactionId = targetFactionId;
            var focusPlayer = targetFactionId === "player"
                ? (hud.players || []).find(function (player) { return player && player.is_me; })
                : targetFactionId && (hud.players || []).find(function (player) {
                    return player && player.campaign_faction_id === targetFactionId;
                });
            if (focusPlayer && focusPlayer.is_alive === true
                && Number.isFinite(Number(focusPlayer.centroid_x))
                && Number.isFinite(Number(focusPlayer.centroid_y))) {
                send("focus_world", {
                    x: Number(focusPlayer.centroid_x) + 0.5,
                    y: Number(focusPlayer.centroid_y) + 0.5
                });
            }
        }
        syncTutorialPause(machineView);
        var markerId = null;
        var markerTarget = markerTargetFor(machineView.step);
        if (markerTarget) {
            var marked = markerTarget === "player"
                ? (hud.players || []).find(function (player) { return player && player.is_me; })
                : (hud.players || []).find(function (player) { return player && player.campaign_faction_id === markerTarget; });
            if (marked && Number.isInteger(Number(marked.id))) markerId = Number(marked.id);
        }
        if (markerId !== runtime.markerId) {
            runtime.markerId = markerId;
            send("set_tutorial_marker", { player_id: markerId });
        }
        syncTransferGuide(machineView.step);
        revealGuidedUiTarget(machineView.step);
        if (!runtime.modalOpen) {
            if (machineView.waiting) {
                runtime.view.render(null);
                if (runtime.stepRevealStepId !== machineView.step.id) clearStepRevealTimer();
                if (runtime.stepRevealTimer === null && typeof window.setTimeout === "function") {
                    var waitingMachine = runtime.machine, waitingStepId = machineView.step.id;
                    runtime.stepRevealStepId = waitingStepId;
                    var waitingToken = ++runtime.stepRevealToken;
                    var timerId = window.setTimeout(function () {
                        if (runtime.stepRevealToken !== waitingToken || runtime.stepRevealTimer !== timerId) return;
                        runtime.stepRevealTimer = null;
                        runtime.stepRevealStepId = null;
                        if (runtime.machine === waitingMachine && runtime.latestHud && !runtime.modalOpen
                            && waitingMachine.view().step.id === waitingStepId) update(runtime.latestHud);
                    }, Math.max(1, machineView.wait_remaining_ms));
                    runtime.stepRevealTimer = timerId;
                }
            } else {
                clearStepRevealTimer();
                var displayView = placementGuideView(machineView, hud);
                runtime.view.render(displayView, renderContext(displayView.step, hud, anchorFor(displayView.step, hud), displayView));
            }
        }
        if (machineView.done && !runtime.completionSent) {
            runtime.completionSent = true;
            trackExperience("campaign_episode_complete", { episode_id: runtime.episodeId });
            send("complete_campaign_episode", { episode_id: runtime.episodeId });
        }
    }

    function menuGuideAuthorized() {
        return Boolean(window.SOW_menu_flow && typeof window.SOW_menu_flow.canGuide === "function"
            && window.SOW_menu_flow.canGuide());
    }

    function menuGuideMap(state) {
        return state && state.menu_guides && typeof state.menu_guides === "object" ? state.menu_guides : {};
    }

    function menuGuideAccount(state) {
        return String(state && state.account_id || "");
    }

    function menuGuideLocalKey(accountId) {
        return "sow.menu_guide.v1." + encodeURIComponent(accountId) + ".welcome";
    }

    function readMenuGuideLocal(accountId) {
        try {
            var value = JSON.parse(window.localStorage.getItem(menuGuideLocalKey(accountId)) || "null");
            return value && ["active", "completed", "skipped"].includes(value.status) ? value : null;
        } catch (error) { return null; }
    }

    function normalizeMenuGuideRecord(record) {
        record = record || {};
        var completed = Array.isArray(record.completed_steps) ? record.completed_steps : [];
        var status = ["completed", "skipped"].includes(record.status) ? record.status
            : completed.includes("welcome_heroes") || completed.includes("welcome_complete") ? "completed" : "active";
        return {
            completed_steps: status === "completed" ? ["welcome_heroes", "welcome_complete"] : [],
            current_step: status === "active" ? "welcome_heroes" : null,
            status: status
        };
    }

    function mergeMenuGuideRecord(serverRecord, localRecord) {
        var server = normalizeMenuGuideRecord(serverRecord), local = normalizeMenuGuideRecord(localRecord);
        return server.status !== "active" ? server : local.status !== "active" ? local : server;
    }

    function scheduleMenuGuideSaveRetry() {
        if (menuJourney.saveRetryTimer || !menuJourney.pendingSave) return;
        menuJourney.saveRetryTimer = window.setTimeout(function () {
            menuJourney.saveRetryTimer = null;
            if (menuJourney.pendingSave) saveMenuGuideRecord(menuJourney.pendingSave);
        }, 2000);
    }

    function saveMenuGuideRecord(record) {
        var state = menuJourney.state, accountId = menuGuideAccount(state);
        if (!accountId || record.status === "active") return;
        var serverRecord = menuGuideMap(state).welcome;
        if (serverRecord && ["completed", "skipped"].includes(serverRecord.status)) {
            try { window.localStorage.removeItem(menuGuideLocalKey(accountId)); } catch (error) { /* Keep the server result. */ }
            menuJourney.pendingSave = null;
            if (menuJourney.saveRetryTimer) window.clearTimeout(menuJourney.saveRetryTimer);
            menuJourney.saveRetryTimer = null;
            return;
        }
        try { window.localStorage.setItem(menuGuideLocalKey(accountId), JSON.stringify(record)); }
        catch (error) { console.error("[SOW MENU GUIDE] Could not cache the pending result.", error); }
        menuJourney.pendingSave = record;
        var now = Date.now();
        if (serverRecord && !state.menu_guide_save_pending && (!menuJourney.lastSaveAt || now - menuJourney.lastSaveAt >= 2000)) {
            menuJourney.lastSaveAt = now;
            send("save_menu_guide_progress", Object.assign({ guide_id: "welcome" }, record));
        }
        scheduleMenuGuideSaveRetry();
    }

    function loadMenuGuideDefinition() {
        if (menuJourney.definition) return Promise.resolve(menuJourney.definition);
        if (menuJourney.loading) return menuJourney.loading;
        menuJourney.loading = fetch(asset("guides/menu.triggers.json"), { cache: "no-store" }).then(function (response) {
            if (!response.ok) throw new Error("Menu guides are unavailable.");
            return response.json();
        }).then(function (definition) {
            var report = window.SOWCampaign.validateMenuGuides(definition);
            if (report.errors.length) throw new Error("Invalid menu guide: " + report.errors[0].message);
            menuJourney.definition = definition;
            return definition;
        }).finally(function () { menuJourney.loading = null; });
        return menuJourney.loading;
    }

    function menuGuideTranslate(key) {
        var definition = menuJourney.definition, strings = definition && definition.strings || {};
        var locale = typeof window.SOW_getLocale === "function" ? window.SOW_getLocale() : document.documentElement.lang || "en";
        var value = strings[locale] && strings[locale][key]
            || strings[String(locale).split("-")[0]] && strings[String(locale).split("-")[0]][key]
            || strings.en && strings.en[key];
        return value || tr(key);
    }

    function ensureMenuGuideView() {
        if (menuJourney.view) return;
        if (!root.isConnected) document.body.appendChild(root);
        menuJourney.view = window.SOWCampaignView.mount(root, {
            translate: menuGuideTranslate,
            asset: asset,
            roster: null,
            guideSkipLabel: menuGuideTranslate("menu.guide.skip"),
            onDismiss: skipMenuGuide
        });
    }

    function finishMenuGuide(status) {
        menuJourney.machine = null;
        pauseMenuGuide();
        saveMenuGuideRecord(normalizeMenuGuideRecord({ status: status }));
    }

    function skipMenuGuide() {
        if (menuGuideAuthorized()) finishMenuGuide("skipped");
    }

    function updateMenuGuideRender() {
        if (!menuGuideAuthorized() || !menuJourney.machine || !menuJourney.view) { pauseMenuGuide(); return; }
        var machineView = menuJourney.machine.update(menuJourney.facts || {}, {}), step = machineView.step;
        if (step.type === "end") { finishMenuGuide("completed"); return; }
        var anchor = anchorFor(step, { tutorial: {} }, menuJourney.facts || {});
        var context = renderContext(step, null, anchor, machineView);
        context.reducedMotion = Boolean(window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
        context.hideObjective = true;
        context.gestureLabel = menuGuideTranslate(step.hint_key);
        menuJourney.view.render(machineView, context);
    }

    function updateMenuGuide(state, facts) {
        state = state || {};
        var accountId = menuGuideAccount(state);
        if (menuJourney.accountId !== accountId) {
            pauseMenuGuide();
            menuJourney.machine = null;
            menuJourney.pendingSave = null;
            menuJourney.lastSaveAt = 0;
            menuJourney.quickMatchRevision = Number(state.quick_match_revision) || 0;
            if (menuJourney.saveRetryTimer) window.clearTimeout(menuJourney.saveRetryTimer);
            menuJourney.saveRetryTimer = null;
        }
        menuJourney.accountId = accountId;
        menuJourney.state = state;
        menuJourney.facts = facts || {};
        var serverRecord = menuGuideMap(state).welcome;
        if (!accountId || !serverRecord) { pauseMenuGuide(); return; }
        var record = mergeMenuGuideRecord(serverRecord, readMenuGuideLocal(accountId));
        var quickMatchRevision = Number(state.quick_match_revision) || 0;
        var startedMatch = quickMatchRevision > menuJourney.quickMatchRevision || state.phase === "Playing";
        menuJourney.quickMatchRevision = quickMatchRevision;
        if (record.status === "active" && (startedMatch || Number(state.profile_stats && state.profile_stats.matches_played) > 0)) {
            record = normalizeMenuGuideRecord({ status: "skipped" });
        }
        if (record.status !== "active") {
            menuJourney.machine = null;
            pauseMenuGuide();
            saveMenuGuideRecord(record);
            return;
        }
        if (!menuGuideAuthorized()) { pauseMenuGuide(); return; }
        var updateRevision = ++menuJourney.updateRevision;
        loadMenuGuideDefinition().then(function (definition) {
            if (updateRevision !== menuJourney.updateRevision || !menuGuideAuthorized()) return;
            if (!menuJourney.machine) {
                menuJourney.machine = window.SOWCampaign.create(definition, definition.menu_guides.welcome.entry, null);
            }
            ensureMenuGuideView();
            menuJourney.paused = false;
            menuJourney.machine.setPaused(false);
            updateMenuGuideRender();
        }).catch(function (error) {
            if (updateRevision !== menuJourney.updateRevision) return;
            console.error("[SOW MENU GUIDE]", error);
            menuJourney.loading = null;
            pauseMenuGuide();
        });
    }

    function pauseMenuGuide() {
        menuJourney.updateRevision++;
        menuJourney.paused = true;
        if (menuJourney.machine) menuJourney.machine.setPaused(true);
        if (menuJourney.view) { menuJourney.view.destroy(); menuJourney.view = null; }
    }

    window.SOW_menu_guide = { update: updateMenuGuide, pause: pauseMenuGuide };

    // Each step's emote directive runs when that step completes: a named emote is shown
    // (pinned when emote_pinned), and "clear" releases the current one.
    function applyStepEmote(step) {
        if (!step.emote) return;
        send("tutorial_emote", { kind: step.emote === "clear" ? null : step.emote, pinned: step.emote_pinned === true });
    }

    function continueScene() {
        if (!runtime.machine) return;
        var step = runtime.machine.view().step;
        if (runtime.machine.advance(null, step.id)) update(runtime.latestHud);
    }

    function choose(choiceId) {
        if (!runtime.machine) return;
        var model = runtime.machine.view();
        var step = model.step;
        var isDialogChoice = Boolean(model.reactionData || (step && step.type === "choice"));
        if (model.reactionData) {
            var answer = (model.reactionData.choices || []).find(function (choice) { return choice.id === choiceId; });
            if (!answer) return;
            if (answer.relation === "allied" && Number(answer.gold_cost || 0) > availableGold(runtime.latestHud)) {
                answer = (model.reactionData.choices || []).find(function (choice) {
                    return choice.relation !== "allied" && Number(choice.gold_cost || 0) === 0;
                });
                if (!answer) return;
                choiceId = answer.id;
            }
            if (!resolveReaction(model.reactionData, model.reactionTarget, answer, runtime.latestHud)) return;
        }
        if (runtime.machine.advance(choiceId, step.id)) {
            if (isDialogChoice) trackExperience("tutorial_dialog_choice");
            update(runtime.latestHud);
        }
    }

    function resolveReaction(reaction, targetName, choice, hud) {
        var outcome = choice ? choice.relation : reaction.outcome;
        if (!outcome) return true;
        var key = reaction.id + "@" + targetName;
        if (runtime.resolvedReactions.has(key)) return true;
        var target = (hud && hud.players || []).find(function (player) { return player && player.campaign_faction_id === targetName; });
        var human = (hud && hud.players || []).find(function (player) { return player && player.is_me; });
        var goldCost = Number(choice ? choice.gold_cost || 0 : reaction.gold_cost || 0);
        if (!target || !human || availableGold(hud) < goldCost) return false;
        if (!send("resolve_campaign_diplomacy", {
            target_player_id: Number(target.id), relation: outcome, gold_cost: goldCost
        })) return false;
        runtime.resolvedReactions.add(key);
        return true;
    }

    function focusMarker() {
        var hud = runtime.latestHud;
        if (!hud || !runtime.machine) return;
        var step = runtime.machine.view().step;
        var target = markerTargetFor(step);
        var player = target === "player" ? (hud.players || []).find(function (item) { return item && item.is_me; }) : (hud.players || []).find(function (item) { return item && item.campaign_faction_id === target; });
        if (player) send("focus_player", { player_id: Number(player.id) });
    }

    function syncTutorialPause(machineView) {
        var paused = Boolean(machineView && machineView.paused);
        var cameraOnly = paused && machineView.step.camera_only === true;
        var pausedAction = paused ? machineView.paused_action || null : null;
        if (paused !== runtime.uiPaused || cameraOnly !== runtime.cameraOnly || pausedAction !== runtime.pausedAction) {
            runtime.uiPaused = paused;
            runtime.cameraOnly = cameraOnly;
            runtime.pausedAction = pausedAction;
            send("set_tutorial_paused", { paused: paused, camera_only: cameraOnly, paused_action: pausedAction });
        }
    }

    function modalChanged() {
        var modal = document.getElementById("sow-hud-surrender-modal");
        var open = Boolean(modal && !modal.classList.contains("hidden"));
        if (open === runtime.modalOpen) return;
        runtime.modalOpen = open;
        if (runtime.machine) runtime.machine.setPaused(open);
        if (open) {
            runtime.uiPaused = true;
            runtime.pausedAction = null;
            send("set_tutorial_paused", { paused: true, camera_only: runtime.cameraOnly, paused_action: null });
            root.hidden = true;
        } else if (runtime.machine) {
            if (runtime.latestHud) update(runtime.latestHud);
        }
    }

    function redrawCampaign() {
        if (!runtime.machine || !runtime.view || runtime.modalOpen) return;
        if (!runtime.latestHud) return;
        var model = runtime.machine.view();
        if (model.waiting) runtime.view.render(null);
        else {
            var displayView = placementGuideView(model, runtime.latestHud);
            runtime.view.render(displayView, renderContext(displayView.step, runtime.latestHud, anchorFor(displayView.step, runtime.latestHud), displayView));
        }
    }
    window.addEventListener("resize", function () {
        runtime.guidedScrollStepId = null;
        redrawCampaign();
    }, { passive: true });

    if (hudRoot && typeof MutationObserver !== "undefined") {
        runtime.modalObserver = new MutationObserver(modalChanged);
        runtime.modalObserver.observe(hudRoot, { subtree: true, attributes: true, attributeFilter: ["class"] });
    }

    function recordUiAction(event) {
        if (!runtime.machine) return;
        if (root.contains(event.target)) return;
        var changed = false;
        var currentStep = runtime.machine.view().step;
        Object.keys(window.SOWCampaign.UI_TARGETS).forEach(function (key) {
            // Targeted map actions count only after the HUD confirms the destination.
            if (key === "map_menu_open" || (key === "map_transfer" && currentStep && currentStep.marker
                && currentStep.trigger && currentStep.trigger.type === "ui" && currentStep.trigger.action === key)) return;
            var control = window.SOWCampaign.resolveUiTarget(key, document, runtime.episodeId);
            var actionEvent = control && control.matches("input") ? "change" : "click";
            if (event.type === actionEvent && control && !control.disabled && control.getClientRects().length && (control === event.target || control.contains(event.target))) {
                if (key === "attack_ratio" && currentStep && currentStep.trigger && currentStep.trigger.type === "ui"
                    && currentStep.trigger.action === key) return;
                runtime.uiCounts[key] = (runtime.uiCounts[key] || 0) + 1; changed = true;
            }
        });
        if (changed && runtime.active && runtime.latestHud) update(runtime.latestHud);
    }
    ["click", "input", "change"].forEach(function (type) { document.addEventListener(type, recordUiAction, true); });

    window.SOW_startCampaignEpisode = function (episodeId) { startEpisode(episodeId, false); };
    window.SOW_watchCampaignCinematic = watchCampaignCinematic;
    window.SOW_tutorial_exit_context = tutorialExitContext;
    window.SOW_tutorial_menu_state_update = function (state) {
        var phase = state && state.phase;
        if (phase !== "MainMenu") pauseMenuGuide();
        if (phase === "MainMenu" && runtime.lastPhase !== "MainMenu" && !(state && state.boot_campaign)) {
            reset();
        }
        runtime.lastPhase = phase;
        if (state && state.boot_campaign && runtime.bootEpisode !== state.boot_campaign) startEpisode(state.boot_campaign, true);
    };
    window.SOW_tutorial_camera_anchor_update = function (x, y) {
        if (runtime.view) runtime.view.updateCameraAnchor(Number(x), Number(y));
    };
    window.SOW_tutorial_state_update = function (state) {
        if (!state || state.phase !== "MainMenu") pauseMenuGuide();
        var hud = state && state.phase === "Playing" ? state.hud : null;
        var tutorial = hud && hud.tutorial;
        if (!tutorial || !tutorial.active) {
            runtime.latestHud = null;
            if (runtime.view) runtime.view.render(null);
            if (runtime.machine && state && state.phase === "MainMenu") reset();
            return;
        }
        runtime.latestHud = hud;
        if (!runtime.machine || runtime.episodeId !== tutorial.episode_id) activate(tutorial, hud);
        else update(hud);
    };
    window.addEventListener("sow:locale-change", function () {
        if (runtime.machine && runtime.latestHud) update(runtime.latestHud);
    });

    function reset() {
        runtime.generation++;
        clearStepRevealTimer();
        syncTransferGuide(null);
        runtime.priorChoices = Object.create(null);
        runtime.episodeId = null;
        runtime.roster = null;
        runtime.definition = null;
        runtime.machine = null;
        runtime.active = false;
        window.SOW_emote_text_overrides = {};
        runtime.starting = null;
        runtime.activating = null;
        runtime.bootEpisode = null;
        runtime.cameraReactionTarget = null;
        if (runtime.reactionHighlightPlayerId !== null) {
            runtime.reactionHighlightPlayerId = null;
            send("set_dialog_border_highlight", { player_id: null });
        }
        runtime.latestHud = null;
        runtime.uiPaused = false;
        runtime.cameraOnly = false;
        runtime.pausedAction = null;
        runtime.cameraTargetFactionId = null;
        runtime.hoverEvents = 0;
        runtime.hoveredEntityId = null;
        runtime.hoverStepId = null;
        runtime.hoverStepRecorded = false;
        runtime.modalOpen = false;
        runtime.completionSent = false;
        runtime.resolvedReactions = new Set();
        runtime.lastActionStepId = null;
        runtime.failedEpisode = null;
        runtime.lastUnlockCommand = null;
        runtime.markerId = undefined;
        if (runtime.view) { runtime.view.destroy(); runtime.view = null; }
        var error = document.getElementById("sow-campaign-error");
        if (error) error.remove();
    }
})();
