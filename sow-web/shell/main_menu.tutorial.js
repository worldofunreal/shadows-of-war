(function () {
    "use strict";

    var hudRoot = document.getElementById("sow-hud");
    var root = document.createElement("div");
    root.id = "sow-story";
    root.hidden = true;
    document.body.appendChild(root);
    var pendingMenuGuide = null;
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
        menuGuide: false,
        guidedScrollStepId: null,
        guidedControl: null,
        guidedAmountControls: []
    };

    function clearStepRevealTimer() {
        runtime.stepRevealToken++;
        if (runtime.stepRevealTimer !== null && typeof window.clearTimeout === "function") window.clearTimeout(runtime.stepRevealTimer);
        runtime.stepRevealTimer = null;
        runtime.stepRevealStepId = null;
    }

    function asset(path) {
        var base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
        return base + "/" + path.split("/").map(encodeURIComponent).join("/");
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
        var target = step && step.trigger && step.trigger.target;
        var players = hud && hud.players || [];
        return players.find(function (player) {
            return player && (target === "player" ? player.is_me : player.campaign_faction_id === target);
        }) || null;
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
            } else if (step.trigger && step.trigger.type === "ui" && step.trigger.action === "cancel_building_mode") {
                context.hintOverride = tr("tutorial.building_mode_exit_" + (context.zoomMode === "pinch" ? "mobile" : "desktop") + "_hint");
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
            onDismiss: runtime.menuGuide
                ? (runtime.definition.menu_guide.dismissible === false ? null : dismissMenuGuide)
                : null
        });
    }

    function startEpisode(episodeId, fromBoot) {
        episodeId = String(episodeId || "");
        if (!/^[a-z][a-z0-9_]{0,63}$/.test(episodeId) || runtime.starting === episodeId || (runtime.active && runtime.episodeId === episodeId)) return;
        if (runtime.menuGuide) dismissMenuGuide();
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
            runtime.machine = window.SOWCampaign.create(data.definition, undefined, data.roster);
            runtime.active = true;
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

    function anchorFor(step, hud) {
        if (!step || !step.guide) return null;
        var guide = step.guide, tutorial = hud.tutorial || {};
        var triggerType = step.trigger && step.trigger.type;
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
            if (!buildingAnchor || !Number.isFinite(Number(buildingAnchor.x)) || !Number.isFinite(Number(buildingAnchor.y))) return null;
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
            var source = window.SOWCampaign.resolveUiTarget(guide.target, document, runtime.episodeId);
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
        if (selectedBuildingKind === "Defense Tower") selectedBuildingKind = "Bunker";
        var facts = Object.assign({}, tutorial.facts || {}, {
            hover_events: runtime.hoverEvents,
            touch_controls: zoomMode() === "pinch" ? 1 : 0,
            selected_building_kind: selectedBuildingKind,
            attack_ratio: Number(hud.attack_ratio)
        });
        if (currentStep.trigger && currentStep.trigger.type === "ui"
            && currentStep.trigger.action === "map_transfer" && currentStep.marker
            && currentStep.marker.target && hud.transfer) {
            var transferTarget = (hud.players || []).find(function (player) {
                return player && player.campaign_faction_id === currentStep.marker.target;
            });
            if (transferTarget && Number(hud.transfer.target_id) === Number(transferTarget.id)) {
                runtime.uiCounts.map_transfer = Math.max(1, Number(runtime.uiCounts.map_transfer) || 0);
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
        if (!runtime.menuGuide && machineView.step) {
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
            var targetPlayer = targetFactionId === "player"
                ? (hud.players || []).find(function (player) { return player && player.is_me; })
                : targetFactionId && (hud.players || []).find(function (player) {
                    return player && player.campaign_faction_id === targetFactionId;
                });
            if (targetPlayer && targetPlayer.is_alive === true
                && Number.isFinite(Number(targetPlayer.centroid_x))
                && Number.isFinite(Number(targetPlayer.centroid_y))) {
                send("focus_world", {
                    x: Number(targetPlayer.centroid_x) + 0.5,
                    y: Number(targetPlayer.centroid_y) + 0.5
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
                runtime.view.render(machineView, renderContext(machineView.step, hud, anchorFor(machineView.step, hud), machineView));
            }
        }
        if (machineView.done && !runtime.completionSent) {
            runtime.completionSent = true;
            trackExperience("campaign_episode_complete", { episode_id: runtime.episodeId });
            if (runtime.definition.menu_guide) pendingMenuGuide = { episodeId: runtime.episodeId };
            send("complete_campaign_episode", { episode_id: runtime.episodeId });
        }
    }

    function updateMenuGuide() {
        if (!runtime.menuGuide || !runtime.machine || !runtime.definition || !runtime.latestHud && runtime.lastPhase !== "MainMenu") return;
        var machineView = runtime.machine.update({}, runtime.uiCounts);
        // Owner decision: in the main menu the guide is the hand only. The
        // objective card ("Open Campaigns to continue the story.") had no close
        // button and only vanished when the player obeyed it, and the closing
        // "You're ready" panel was the same nuisance. Both stay hidden here; the
        // in-match objective card is unaffected.
        if (machineView.done || machineView.step.type === "end") { dismissMenuGuide(); return; }
        revealGuidedUiTarget(machineView.step);
        var context = renderContext(machineView.step, null, anchorFor(machineView.step, { tutorial: {} }));
        context.reducedMotion = Boolean(window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
        context.hideObjective = true;
        runtime.view.render(machineView, context);
    }

    function startMenuGuide() {
        if (!pendingMenuGuide || !runtime.lastPhase || runtime.lastPhase !== "MainMenu") return;
        var episodeId = pendingMenuGuide.episodeId;
        var generation = runtime.generation;
        loadEpisode(episodeId).then(function (data) {
            if (runtime.generation !== generation || !pendingMenuGuide || pendingMenuGuide.episodeId !== episodeId) return;
            if (!data.definition.menu_guide) {
                pendingMenuGuide = null;
                runtime.priorChoices = Object.create(null);
                return;
            }
            runtime.generation++;
            runtime.episodeId = episodeId;
            runtime.roster = data.roster;
            runtime.definition = data.definition;
            runtime.latestHud = null;
            runtime.uiCounts = Object.create(null);
            runtime.active = false;
            runtime.menuGuide = true;
            runtime.guidedScrollStepId = null;
            runtime.completionSent = true;
            runtime.machine = window.SOWCampaign.create(data.definition, data.definition.menu_guide.entry, data.roster);
            runtime.machine.jump(data.definition.menu_guide.entry, null, runtime.priorChoices);
            runtime.priorChoices = Object.create(null);
            makeView();
            updateMenuGuide();
            pendingMenuGuide = null;
        }).catch(function (error) {
            if (runtime.generation === generation && pendingMenuGuide && pendingMenuGuide.episodeId === episodeId) {
                showError(error, episodeId, startMenuGuide, function () {
                    pendingMenuGuide = null;
                    runtime.priorChoices = Object.create(null);
                });
            }
        });
    }

    function dismissMenuGuide() {
        if (!runtime.menuGuide) { openLeaveMatch(); return; }
        runtime.menuGuide = false;
        runtime.guidedScrollStepId = null;
        runtime.machine = null;
        syncTransferGuide(null);
        runtime.priorChoices = Object.create(null);
        pendingMenuGuide = null;
        if (runtime.view) runtime.view.render(null);
    }

    function continueScene() {
        if (!runtime.machine) return;
        var step = runtime.machine.view().step;
        if (runtime.machine.advance(null, step.id)) runtime.menuGuide ? updateMenuGuide() : update(runtime.latestHud);
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
            if (isDialogChoice && !runtime.menuGuide) trackExperience("tutorial_dialog_choice");
            runtime.menuGuide ? updateMenuGuide() : update(runtime.latestHud);
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

    function openLeaveMatch() {
        var button = hudRoot && hudRoot.querySelector('[data-command="prompt_surrender"]');
        if (button) button.click();
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
            else {
                var model = runtime.machine.view();
                syncTutorialPause(model);
                runtime.view.render(model, renderContext(model.step, null, null, model));
            }
        }
    }

    function redrawCampaign() {
        if (!runtime.machine || !runtime.view || runtime.modalOpen) return;
        if (runtime.menuGuide) { updateMenuGuide(); return; }
        if (!runtime.latestHud) return;
        var model = runtime.machine.view();
        if (model.waiting) runtime.view.render(null);
        else runtime.view.render(model, renderContext(model.step, runtime.latestHud, anchorFor(model.step, runtime.latestHud), model));
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
            var control = window.SOWCampaign.resolveUiTarget(key, document, runtime.episodeId);
            var actionEvent = control && control.matches("input") ? "change" : "click";
            if (event.type === actionEvent && control && !control.disabled && control.getClientRects().length && (control === event.target || control.contains(event.target))) {
                if (key === "attack_ratio" && currentStep && currentStep.trigger && currentStep.trigger.type === "ui"
                    && currentStep.trigger.action === key) return;
                runtime.uiCounts[key] = (runtime.uiCounts[key] || 0) + 1; changed = true;
            }
        });
        if (changed) {
            if (runtime.menuGuide) {
                updateMenuGuide();
                window.setTimeout(updateMenuGuide, 650);
            } else if (runtime.active && runtime.latestHud) {
                update(runtime.latestHud);
            }
        }
    }
    document.addEventListener("keydown", function (event) {
        var step = runtime.machine && runtime.machine.view().step;
        var selected = runtime.latestHud && runtime.latestHud.selected_building;
        if (event.key !== "Escape" || !selected || !step || !step.trigger
            || step.trigger.type !== "ui" || step.trigger.action !== "cancel_building_mode") return;
        event.preventDefault();
        event.stopImmediatePropagation();
        send("select_building", { kind: selected });
        runtime.uiCounts.cancel_building_mode = (runtime.uiCounts.cancel_building_mode || 0) + 1;
        update(runtime.latestHud);
    }, true);
    ["click", "input", "change"].forEach(function (type) { document.addEventListener(type, recordUiAction, true); });

    window.SOW_startCampaignEpisode = function (episodeId) { startEpisode(episodeId, false); };
    window.SOW_tutorial_exit_context = tutorialExitContext;
    window.SOW_tutorial_menu_state_update = function (state) {
        var phase = state && state.phase;
        if (phase !== "MainMenu" && runtime.menuGuide) dismissMenuGuide();
        if (phase === "MainMenu" && runtime.lastPhase !== "MainMenu" && !(state && state.boot_campaign)) {
            if (runtime.active && runtime.definition && runtime.definition.menu_guide) {
                pendingMenuGuide = pendingMenuGuide || { episodeId: runtime.episodeId };
            }
            reset();
        }
        runtime.lastPhase = phase;
        if (state && state.boot_campaign && runtime.bootEpisode !== state.boot_campaign) startEpisode(state.boot_campaign, true);
    };
    window.SOW_tutorial_camera_anchor_update = function (x, y) {
        if (runtime.view) runtime.view.updateCameraAnchor(Number(x), Number(y));
    };
    window.SOW_tutorial_state_update = function (state) {
        if (runtime.menuGuide && state && state.phase === "MainMenu") { updateMenuGuide(); return; }
        var hud = state && state.phase === "Playing" ? state.hud : null;
        var tutorial = hud && hud.tutorial;
        if (!tutorial || !tutorial.active) {
            runtime.latestHud = null;
            if (runtime.view) runtime.view.render(null);
            if (runtime.machine && state && state.phase === "MainMenu" && !runtime.menuGuide) reset();
            return;
        }
        runtime.latestHud = hud;
        if (!runtime.machine || runtime.episodeId !== tutorial.episode_id) activate(tutorial, hud);
        else update(hud);
    };
    window.addEventListener("sow:locale-change", function () {
        if (runtime.menuGuide) updateMenuGuide();
        else if (runtime.machine && runtime.latestHud) update(runtime.latestHud);
    });
    window.addEventListener("sow:campaign-menu-guide-ready", startMenuGuide);

    function reset() {
        runtime.generation++;
        clearStepRevealTimer();
        syncTransferGuide(null);
        if (!pendingMenuGuide) runtime.priorChoices = Object.create(null);
        runtime.episodeId = null;
        runtime.roster = null;
        runtime.definition = null;
        runtime.machine = null;
        runtime.active = false;
        runtime.starting = null;
        runtime.activating = null;
        runtime.bootEpisode = null;
        runtime.cameraReactionTarget = null;
        if (runtime.reactionHighlightPlayerId !== null) {
            runtime.reactionHighlightPlayerId = null;
            send("set_dialog_border_highlight", { player_id: null });
        }
        runtime.latestHud = null;
        runtime.menuGuide = false;
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
