// Campaign tutorial controller; shared guide calculations live in campaign/tutorial-guides.ts.
import { createMenuGuide } from "./main_menu.menu-guide.ts";
import { createTutorialGuides, TUTORIAL_HOVER_RADIUS as HOVER_TARGET_RADIUS } from "./campaign/tutorial-guides.ts";
import type { TutorialAnchor } from "./campaign/tutorial-guides.ts";
import type {
    CampaignDefinition,
    CampaignMachine,
    CampaignMachineView,
    CampaignRecord,
    CampaignRoster,
    CampaignStep,
    CampaignView,
} from "./campaign/types.ts";

interface LoadedTutorialEpisode {
    definition: CampaignDefinition;
    roster: CampaignRoster;
}

interface TutorialExchangeCue {
    token: string | number;
    nativeToken: string;
    line: CampaignRecord;
    faction: string;
    session: number;
}

interface TutorialRuntime {
    generation: number;
    episodeId: string | null;
    roster: CampaignRoster | null;
    activating: string | null;
    definition: CampaignDefinition | null;
    machine: CampaignMachine | null;
    view: CampaignView | null;
    active: boolean;
    starting: string | null;
    loading: Record<string, Promise<LoadedTutorialEpisode> | undefined>;
    latestHud: CampaignRecord | null;
    uiCounts: Record<string, number>;
    uiPaused: boolean | null;
    cameraOnly: boolean | null;
    pausedAction: string | null;
    hoverEvents: number;
    hoveredEntityId: number | null;
    hoverStepId: string | null;
    hoverStepRecorded: boolean;
    hoverPointerAway: boolean;
    mapUiStepRecorded: boolean;
    cameraTargetFactionId: string | null;
    cameraShotStepId: string | null;
    reactionHighlightPlayerId: number | null;
    stepRevealTimer: number | null;
    stepRevealStepId: string | null;
    stepRevealToken: number;
    completionSent: boolean;
    analyticsStepId: string | null;
    analyticsStartedStepIds: Set<string>;
    analyticsCompletedStepIds: Set<string>;
    observingAfterDefeat: boolean;
    lastActionStepId: string | null;
    resolvedReactions: Set<string>;
    priorChoices: CampaignRecord;
    failedEpisode: string | null;
    lastUnlockCommand: string | null;
    bootEpisode: string | null;
    lastPhase: string | null;
    modalObserver: MutationObserver | null;
    guidedScrollStepId: string | null;
    guidedControl: HTMLElement | null;
    guidedAmountControls: HTMLElement[];
    exchangeCue: TutorialExchangeCue | null;
    exchangeFocusToken: string | number | null;
    lastPreDefeatTargets: string | null;
    campaignSessionId: number;
    emoteSerial: number;
    markerId: number | null | undefined;
    modalOpen: boolean | undefined;
    cameraReactionTarget: string | null | undefined;
}

(function () {
    "use strict";

    const hudRoot = document.getElementById("sow-hud");
    const root = document.createElement("div");
    root.id = "sow-story";
    root.hidden = true;
    document.body.appendChild(root);
    const cinematicPreviewRoot = document.createElement("div");
    cinematicPreviewRoot.id = "sow-campaign-cinematic-preview";
    cinematicPreviewRoot.className = "sow-story--menu-preview";
    cinematicPreviewRoot.hidden = true;
    document.body.appendChild(cinematicPreviewRoot);
    let cinematicPreviewView: CampaignView | null = null;
    let cinematicPreviewGeneration = 0;
    
    const runtime: TutorialRuntime = {
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
        hoverPointerAway: false,
        mapUiStepRecorded: false,
        cameraTargetFactionId: null,
        cameraShotStepId: null,
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
        guidedAmountControls: [],
        exchangeCue: null,
        exchangeFocusToken: null,
        lastPreDefeatTargets: null,
        campaignSessionId: 0,
        emoteSerial: 0,
        markerId: undefined as number | null | undefined,
        modalOpen: undefined as boolean | undefined,
        cameraReactionTarget: null,
    };

    const {
        targetPlayer,
        targetMapMenuOpen,
        cameraTargetDistance,
        cameraTargetAnchor,
        renderContext,
        anchorFor,
        revealGuidedUiTarget,
        syncTransferGuide,
        campaignPlayer,
        onMap,
        syncCameraShot,
        markerTargetFor,
    } = createTutorialGuides({
        window,
        document,
        runtime,
        send,
        tr,
        hasText,
        stepTextKey,
        zoomMode,
    });

    function clearStepRevealTimer() {
        runtime.stepRevealToken++;
        if (runtime.stepRevealTimer !== null && typeof window.clearTimeout === "function")
            window.clearTimeout(runtime.stepRevealTimer);
        runtime.stepRevealTimer = null;
        runtime.stepRevealStepId = null;
    }

    function asset(path: string) {
        const base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
        const url = base + "/" + path.split("/").map(encodeURIComponent).join("/");
        return window.SOW_assetUrl ? window.SOW_assetUrl(url) : url;
    }

    function send(type: string, fields?: CampaignRecord) {
        if (typeof window.SOW_menu_command !== "function") return false;
        window.SOW_menu_command(JSON.stringify(Object.assign({ type: type }, fields || {})));
        return true;
    }

    function trackExperience(name: string, props?: CampaignRecord) {
        if (typeof window.SOW_trackExperienceEvent === "function") {
            window.SOW_trackExperienceEvent(JSON.stringify({ name: name, props: props || null }));
        }
    }

    function trackTutorialStep(action: string, stepId: string) {
        const steps = (runtime.definition && runtime.definition.steps) || [];
        const index = steps.findIndex(function (step) {
            return step.id === stepId;
        });
        if (index < 0 || index >= 512 || !runtime.episodeId) return;
        trackExperience("tutorial_step", {
            episode_id: runtime.episodeId,
            step_id: stepId,
            step_index: index,
            action: action,
        });
    }

    function tutorialExitContext(episodeId: string) {
        if (!runtime.active || runtime.episodeId !== episodeId) return null;
        if (runtime.completionSent || (runtime.machine && runtime.machine.view().done)) return false;
        const stepId = runtime.analyticsStepId;
        if (
            !stepId ||
            !runtime.analyticsStartedStepIds.has(stepId) ||
            runtime.analyticsCompletedStepIds.has(stepId)
        )
            return {};
        const steps = (runtime.definition && runtime.definition.steps) || [];
        const stepIndex = steps.findIndex(function (step) {
            return step.id === stepId;
        });
        const completed = (runtime.machine && runtime.machine.state && runtime.machine.state.completed) || [];
        if (stepIndex < 0 || stepIndex >= 512 || completed.includes(stepId)) return {};
        return { step_id: stepId, step_index: stepIndex, action: "fail" };
    }

    function availableGold(hud: CampaignRecord | null) {
        const gold = Number(hud && hud.gold);
        return Number.isFinite(gold) ? Math.max(0, gold) : 0;
    }

    function tr(key: string) {
        const definition = runtime.definition;
        const value = typeof window.SOW_t === "function" ? window.SOW_t(key) : "[" + key + "]";
        if (!value || value === "[" + key + "]") return key;
        return window.SOWCampaign && definition && runtime.roster
            ? window.SOWCampaign.replaceFactionStoryNames(value, definition, runtime.roster)
            : value;
    }

    function stepTextKey(step: CampaignStep, suffix: string) {
        const namespace = runtime.definition && runtime.definition.text_namespace;
        return (typeof namespace === "string" ? namespace : "tutorial.") + step.id + suffix;
    }

    function zoomMode() {
        const nav: Partial<Navigator> & { userAgentData?: { mobile?: boolean; platform?: string } } =
            window.navigator || {};
        const data = nav.userAgentData || {};
        return window.SOWCampaign.zoomInputMode({
            androidTwa: typeof window.SOW_isAndroidTwa === "function" && window.SOW_isAndroidTwa(),
            mobile: Boolean(data.mobile),
            userAgent: nav.userAgent || "",
            platform: data.platform || nav.platform || "",
            maxTouchPoints: nav.maxTouchPoints,
        });
    }







    function fetchJson(path: string): Promise<CampaignRecord> {
        return fetch(asset(path), { cache: "no-store", credentials: "same-origin" }).then(
            function (response) {
                if (!response.ok) throw new Error(path + " returned " + response.status);
                return response.json() as Promise<CampaignRecord>;
            },
        );
    }

    function hasText(key: string) {
        return typeof window.SOW_hasText === "function" && window.SOW_hasText(key);
    }

    function loadEpisode(episodeId: string): Promise<LoadedTutorialEpisode> {
        if (runtime.loading[episodeId]) return runtime.loading[episodeId];
        const request = Promise.all([
            fetchJson("campaign/" + episodeId + ".json"),
            fetchJson("campaign/" + episodeId + ".triggers.json"),
        ]).then(function (files) {
            const roster = files[0] as CampaignRoster,
                definition = files[1] as CampaignDefinition;
            if (!window.SOWCampaign || !window.SOWCampaignView)
                throw new Error("Campaign runtime is missing.");
            if (definition.episode_id !== episodeId)
                throw new Error("Campaign filename and episode ID do not match.");
            const result = window.SOWCampaign.validate(definition, roster, {
                hasText: hasText,
                getText: function (key: string) {
                    const value = typeof window.SOW_t === "function" ? window.SOW_t(key) : null;
                    if (!hasText(key) || !value || value === key || value === "[" + key + "]") return null;
                    return window.SOWCampaign.replaceFactionStoryNames(value, definition, roster);
                },
            });
            if (result.errors.length)
                throw new Error(
                    result.errors
                        .map(function (issue) {
                            return issue.message;
                        })
                        .join("\n"),
                );
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

    function watchCampaignCinematic(episodeId: string) {
        episodeId = String(episodeId || "");
        if (!/^[a-z][a-z0-9_]{0,63}$/.test(episodeId)) return;
        if (cinematicPreviewView) cinematicPreviewView.destroy();
        cinematicPreviewView = null;
        const generation = ++cinematicPreviewGeneration;
        cinematicPreviewRoot.classList.add("sow-story--menu-preview");
        cinematicPreviewRoot.hidden = true;
        loadEpisode(episodeId)
            .then(function (data) {
                if (generation !== cinematicPreviewGeneration) return;
                const entry = (data.definition.steps || []).find(function (step) {
                    return step.id === data.definition.entry;
                });
                if (
                    !entry ||
                    entry.type !== "scene" ||
                    entry.presentation !== "cinematic" ||
                    !entry.video_src
                ) {
                    throw new Error("This episode has no opening cinematic.");
                }
                const translate = function (key: string) {
                    const value = typeof window.SOW_t === "function" ? window.SOW_t(key) : "[" + key + "]";
                    if (!value || value === "[" + key + "]") return key;
                    return window.SOWCampaign.replaceFactionStoryNames(value, data.definition, data.roster);
                };
                cinematicPreviewView = window.SOWCampaignView.mount(cinematicPreviewRoot, {
                    translate: translate,
                    asset: asset,
                    roster: function () {
                        return data.roster;
                    },
                    onContinue: closeCampaignCinematic,
                    onDismiss: closeCampaignCinematic,
                });
                const machine = window.SOWCampaign.create(
                    data.definition,
                    data.definition.entry,
                    data.roster,
                );
                cinematicPreviewView.render(machine.view(), {
                    direction: document.documentElement.dir || "ltr",
                    localeScript: document.documentElement.dataset.localeScript || "latin",
                    reducedMotion: Boolean(
                        window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches,
                    ),
                });
            })
            .catch(function (error) {
                if (generation !== cinematicPreviewGeneration) return;
                console.error("[SOW CAMPAIGN CINEMATIC]", error);
                closeCampaignCinematic();
            });
    }

    function showError(
        error: { message?: string } | string | null,
        episodeId: string,
        retryAction?: () => void,
        dismissAction?: () => void,
    ) {
        console.error("[SOW CAMPAIGN]", error);
        if (runtime.latestHud && runtime.latestHud.tutorial && runtime.latestHud.tutorial.active) {
            runtime.uiPaused = true;
            runtime.pausedAction = null;
            send("set_tutorial_paused", {
                paused: true,
                camera_only: runtime.cameraOnly,
                paused_action: null,
            });
        }
        const previous = document.getElementById("sow-campaign-error");
        if (previous) previous.remove();
        const focusBefore = document.activeElement;
        const panel = document.createElement("section");
        panel.id = "sow-campaign-error";
        panel.className = "sow-campaign-error";
        panel.setAttribute("role", "alertdialog");
        const message = document.createElement("p");
        message.textContent = tr("tutorial.unavailable");
        const retry = document.createElement("button");
        retry.type = "button";
        retry.textContent = tr("tutorial.retry");
        retry.addEventListener("click", function () {
            panel.remove();
            runtime.failedEpisode = null;
            delete runtime.loading[episodeId];
            if (retryAction) {
                retryAction();
                return;
            }
            if (
                runtime.latestHud &&
                runtime.latestHud.tutorial &&
                runtime.latestHud.tutorial.active &&
                runtime.latestHud.tutorial.episode_id === episodeId
            ) {
                runtime.activating = null;
                activate(runtime.latestHud.tutorial, runtime.latestHud);
            } else startEpisode(episodeId, false);
        });
        panel.append(message, retry);
        if (dismissAction) {
            const dismiss = document.createElement("button");
            dismiss.type = "button";
            dismiss.textContent = tr("tutorial.continue");
            dismiss.addEventListener("click", function () {
                panel.remove();
                dismissAction();
                if (focusBefore instanceof HTMLElement && focusBefore.isConnected)
                    focusBefore.focus({ preventScroll: true });
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
            roster: function () {
                return runtime.roster;
            },
            onContinue: continueScene,
            onChoice: choose,
            onFocus: focusMarker,
            onCinematicChange: function (active: boolean) {
                if (typeof window.SOW_menu_music_cinematic === "function") {
                    window.SOW_menu_music_cinematic(active);
                }
            },
        });
    }

    function startEpisode(episodeId: string, fromBoot: boolean) {
        episodeId = String(episodeId || "");
        if (
            !/^[a-z][a-z0-9_]{0,63}$/.test(episodeId) ||
            runtime.starting === episodeId ||
            (runtime.active && runtime.episodeId === episodeId)
        )
            return;
        menuGuide.pause();
        runtime.priorChoices = Object.create(null);
        runtime.lastUnlockCommand = null;
        const generation = ++runtime.generation;
        runtime.starting = episodeId;
        runtime.failedEpisode = null;
        const error = document.getElementById("sow-campaign-error");
        if (error) error.remove();
        loadEpisode(episodeId)
            .then(function (data) {
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
                runtime.cameraShotStepId = null;
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
                if (
                    !send("start_campaign_episode", {
                        episode_id: episodeId,
                        roster: data.roster,
                        match: data.definition.settings,
                    })
                ) {
                    throw new Error("Campaign could not reach the game.");
                }
            })
            .catch(function (reason) {
                if (generation === runtime.generation) {
                    runtime.failedEpisode = episodeId;
                    showError(reason, episodeId);
                }
            })
            .then(function () {
                if (generation === runtime.generation) runtime.starting = null;
            });
        if (fromBoot) runtime.bootEpisode = episodeId;
    }

    function activate(tutorial: CampaignRecord, hud: CampaignRecord) {
        if (runtime.machine && runtime.episodeId === tutorial.episode_id) {
            runtime.latestHud = hud;
            update(hud);
            return;
        }
        const episodeId = String(tutorial.episode_id || "");
        if (runtime.failedEpisode === episodeId) return;
        if (runtime.activating === episodeId) return;
        runtime.activating = episodeId;
        const generation = runtime.generation;
        const episode =
            runtime.episodeId === episodeId && runtime.definition
                ? Promise.resolve({ definition: runtime.definition, roster: runtime.roster })
                : loadEpisode(episodeId);
        episode
            .then(function (data) {
                if (
                    generation !== runtime.generation ||
                    !runtime.latestHud ||
                    !runtime.latestHud.tutorial ||
                    runtime.latestHud.tutorial.episode_id !== episodeId
                )
                    return;
                if (runtime.machine && runtime.episodeId === episodeId) {
                    update(runtime.latestHud);
                    return;
                }
                runtime.episodeId = episodeId;
                runtime.roster = data.roster;
                runtime.definition = data.definition;
                runtime.lastPreDefeatTargets = null;
                runtime.campaignSessionId = Number(runtime.latestHud.tutorial.campaign_session_id) || 0;
                if (runtime.machine) runtime.machine.dispose();
                runtime.machine = window.SOWCampaign.create(
                    data.definition,
                    undefined,
                    data.roster,
                    undefined,
                    {
                        onStepComplete: applyStepEmote,
                        onExchangeLine: playExchangeLine,
                        onExchangeClear: clearExchangeLine,
                        onReactionComplete: completeCampaignReaction,
                    },
                );
                runtime.active = true;
                runtime.lastUnlockCommand = null;
                runtime.uiCounts = Object.create(null);
                runtime.uiPaused = null;
                runtime.cameraOnly = null;
                runtime.pausedAction = null;
                runtime.hoverEvents = 0;
                runtime.cameraTargetFactionId = null;
                runtime.cameraShotStepId = null;
                const hovered = runtime.latestHud.hovered;
                runtime.hoveredEntityId =
                    hovered && !hovered.is_me && Number.isInteger(Number(hovered.id))
                        ? Number(hovered.id)
                        : null;
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
            })
            .catch(function (error) {
                if (generation === runtime.generation) {
                    runtime.failedEpisode = episodeId;
                    showError(error, episodeId);
                }
            })
            .then(function () {
                if (runtime.activating === episodeId) runtime.activating = null;
            });
    }

    function placementGuideView(model: CampaignMachineView, hud: CampaignRecord | null): CampaignMachineView {
        if (
            !model ||
            !model.step ||
            model.step.paused_action !== "build_until_started" ||
            !hud ||
            !hud.building_placement ||
            hud.building_placement.pending ||
            !hud.building_placement.can_confirm
        )
            return model;
        const step = Object.assign({}, model.step, {
            guide: { kind: "world", target: "confirm_building_placement", gesture: "tap" },
        });
        return Object.assign({}, model, { step: step });
    }



    function update(hud: CampaignRecord | null) {
        if (
            !runtime.machine ||
            runtime.modalOpen ||
            document.hidden ||
            !hud ||
            !hud.tutorial ||
            !hud.tutorial.active
        )
            return;
        if (!root.isConnected) document.body.appendChild(root);
        const tutorial = hud.tutorial;
        const currentStep = runtime.machine.view().step;
        const stepChanged = runtime.hoverStepId !== currentStep.id;
        if (stepChanged) {
            runtime.hoverStepId = currentStep.id;
            runtime.hoverStepRecorded = false;
            runtime.hoverPointerAway = false;
            runtime.mapUiStepRecorded = false;
        }
        const hoveredEntityId =
            hud.hovered && !hud.hovered.is_me && Number.isInteger(Number(hud.hovered.id))
                ? Number(hud.hovered.id)
                : null;
        const hoverTarget =
            currentStep.trigger && currentStep.trigger.type === "hover" && currentStep.trigger.target;
        const tutorialTargetHovered = Boolean(tutorial.facts && tutorial.facts.tutorial_target_hovered);
        const nameplate = tutorial.nameplate;
        const hoverAnchor: TutorialAnchor | null =
            hoverTarget &&
            nameplate &&
            Number.isFinite(Number(nameplate.x)) &&
            Number.isFinite(Number(nameplate.y))
                ? { x: Number(nameplate.x), y: Number(nameplate.y) }
                : hoverTarget
                  ? cameraTargetAnchor(currentStep, hud)
                  : null;
        const pointer = tutorial.pointer;
        const pointerNearTarget = Boolean(
            hoverAnchor &&
            !hoverAnchor.offscreen &&
            pointer &&
            Number.isFinite(Number(pointer.x)) &&
            Number.isFinite(Number(pointer.y)) &&
            Math.hypot(Number(pointer.x) - hoverAnchor.x, Number(pointer.y) - hoverAnchor.y) <=
                HOVER_TARGET_RADIUS,
        );
        const hoverMatchesTarget =
            tutorialTargetHovered ||
            !hoverTarget ||
            (hud.players || []).some(function (player: CampaignRecord) {
                return (
                    player &&
                    Number(player.id) === hoveredEntityId &&
                    (hoverTarget === "player" ? player.is_me : player.campaign_faction_id === hoverTarget)
                );
            }) ||
            pointerNearTarget;
        const hoverDetected =
            tutorialTargetHovered || (hoveredEntityId !== null && hoverMatchesTarget) || pointerNearTarget;
        const machinimaMoving = Boolean(tutorial.facts && tutorial.facts.machinima_active);
        // The lesson is the player's gesture: a pointer already resting on the target when the step
        // opens must leave it and come back before the hover counts.
        if (!hoverDetected) runtime.hoverPointerAway = true;
        if (
            currentStep.trigger &&
            currentStep.trigger.type === "hover" &&
            !runtime.hoverStepRecorded &&
            !machinimaMoving &&
            hoverDetected &&
            runtime.hoverPointerAway
        ) {
            runtime.hoverStepRecorded = true;
            runtime.hoverEvents++;
        }
        runtime.hoveredEntityId = hoveredEntityId;
        const mapMenu = hud.map_menu;
        const selectedBuildingKind =
            mapMenu && mapMenu.open && mapMenu.view === "building_details" && mapMenu.building
                ? String(mapMenu.building.kind || "")
                : "";
        const facts = Object.assign({}, tutorial.facts || {}, {
            hover_events: runtime.hoverEvents,
            touch_controls: zoomMode() === "pinch" ? 1 : 0,
            selected_building_kind: selectedBuildingKind,
            attack_ratio: Number(hud.attack_ratio),
        });
        facts.resources_available = availableGold(hud) >= 1 || Number(facts.troops || 0) >= 1 ? 1 : 0;
        if (
            currentStep.trigger &&
            currentStep.trigger.type === "ui" &&
            currentStep.marker &&
            currentStep.marker.target &&
            !runtime.mapUiStepRecorded
        ) {
            const mapAction = currentStep.trigger.action;
            const transferTarget = targetPlayer(currentStep, hud);
            const mapActionComplete =
                mapAction === "map_menu_open"
                    ? targetMapMenuOpen(currentStep, hud)
                    : mapAction === "map_transfer" &&
                      transferTarget &&
                      hud.transfer &&
                      Number(hud.transfer.target_id) === Number(transferTarget.id);
            if (mapActionComplete) {
                runtime.uiCounts[mapAction] = (Number(runtime.uiCounts[mapAction]) || 0) + 1;
                runtime.mapUiStepRecorded = true;
            }
        }
        if (
            currentStep.trigger &&
            (currentStep.trigger.type === "camera_target" ||
                (currentStep.trigger.type === "zoom_in_complete" && currentStep.trigger.target))
        ) {
            const targetDistance = cameraTargetDistance(currentStep, hud);
            if (Number.isFinite(targetDistance)) facts.camera_target_distance = targetDistance;
        }
        const machineView = runtime.machine.update(facts, runtime.uiCounts);
        const campaignSessionId = Number(tutorial.campaign_session_id) || 0;
        const preDefeatTargets = Array.isArray(machineView.pre_defeat_targets)
            ? machineView.pre_defeat_targets
            : [];
        const preDefeatKey = campaignSessionId + ":" + preDefeatTargets.slice().sort().join(",");
        if (
            campaignSessionId > 0 &&
            runtime.lastPreDefeatTargets !== preDefeatKey &&
            send("set_campaign_pre_defeat_targets", {
                faction_ids: preDefeatTargets,
                campaign_session_id: campaignSessionId,
            })
        ) {
            runtime.lastPreDefeatTargets = preDefeatKey;
        }
        const unlockCommandKey =
            campaignSessionId + ":" + JSON.stringify(machineView.unlocks || { buildings: {}, actions: [] });
        if (
            campaignSessionId > 0 &&
            runtime.lastUnlockCommand !== unlockCommandKey &&
            send("set_campaign_unlocks", {
                campaign_session_id: campaignSessionId,
                unlocks: machineView.unlocks || { buildings: {}, actions: [] },
            })
        ) {
            runtime.lastUnlockCommand = unlockCommandKey;
        }
        if (machineView.step) {
            const visibleStep = runtime.definition!.steps.find(function (step) {
                return step.id === machineView.step.id;
            });
            if (visibleStep && runtime.analyticsStepId !== visibleStep.id) {
                const previousStepId = runtime.analyticsStepId;
                if (!previousStepId) {
                    trackExperience("tutorial_start", { episode_id: runtime.episodeId });
                }
                if (
                    previousStepId &&
                    runtime.machine.state.completed.includes(previousStepId) &&
                    !runtime.analyticsCompletedStepIds.has(previousStepId)
                ) {
                    runtime.analyticsCompletedStepIds.add(previousStepId);
                    trackTutorialStep("complete", previousStepId);
                }
                runtime.analyticsStepId = visibleStep.id;
                if (!runtime.analyticsStartedStepIds.has(visibleStep.id)) {
                    runtime.analyticsStartedStepIds.add(visibleStep.id);
                    trackTutorialStep("start", visibleStep.id);
                }
            }
            if (
                machineView.done &&
                visibleStep &&
                runtime.machine.state.completed.includes(visibleStep.id) &&
                !runtime.analyticsCompletedStepIds.has(visibleStep.id)
            ) {
                runtime.analyticsCompletedStepIds.add(visibleStep.id);
                trackTutorialStep("complete", visibleStep.id);
            }
        }
        if (machineView.reactionData && machineView.reactionData.outcome) {
            resolveReaction(machineView.reactionData, machineView.reactionTarget, null, hud);
        }
        if (machineView.reactionData && machineView.reactionTarget !== runtime.cameraReactionTarget) {
            const faction = (hud.players || []).find(function (item: CampaignRecord) {
                return item && item.campaign_faction_id === machineView.reactionTarget;
            });
            if (
                faction &&
                Number.isFinite(Number(faction.centroid_x)) &&
                Number.isFinite(Number(faction.centroid_y))
            ) {
                if (
                    send("focus_world", {
                        x: Number(faction.centroid_x) + 0.5,
                        y: Number(faction.centroid_y) + 0.5,
                    })
                ) {
                    runtime.cameraReactionTarget = machineView.reactionTarget;
                }
                const highlightId = Number(faction.id);
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
            const gold = availableGold(hud);
            machineView.choices = machineView.choices.map(function (choice: CampaignRecord) {
                if (Number(choice.gold_cost || 0) <= 0) return choice;
                return Object.assign({}, choice, {
                    gold_available: Math.max(0, gold),
                    gold_insufficient: Number(choice.gold_cost || 0) > gold,
                });
            });
        }
        runtime.priorChoices = Object.assign(Object.create(null), runtime.machine.state.choices);
        if (machineView.step.id !== runtime.lastActionStepId) {
            runtime.lastActionStepId = machineView.step.id;
            if (Number.isFinite(machineView.step.attack_ratio_on_enter))
                send("set_attack_ratio", { ratio: machineView.step.attack_ratio_on_enter });
            if (machineView.step.campaign_attack_on_enter) {
                send("set_campaign_attack_permission", machineView.step.campaign_attack_on_enter);
            }
            if (machineView.step.campaign_halt_on_enter) {
                send("halt_campaign_faction", machineView.step.campaign_halt_on_enter);
            }
            if (machineView.step.campaign_accept_alliance_on_enter) {
                send("accept_campaign_alliance", {
                    faction_id: machineView.step.campaign_accept_alliance_on_enter.faction_id,
                    campaign_session_id: Number(tutorial.campaign_session_id),
                });
            }
            if (machineView.step.campaign_assault_on_enter) {
                const assault = machineView.step.campaign_assault_on_enter;
                send("activate_campaign_assault", {
                    attacker_team: assault.attacker_team,
                    target_faction_id: assault.target,
                    preserve_relation: assault.preserve_relation === true,
                    reinforcement: assault.reinforcement || null,
                    hold_last_tile: assault.hold_last_tile === true,
                });
            }
        }
        if (!runtime.observingAfterDefeat && (facts.eliminated_faction_ids || []).includes("player")) {
            runtime.observingAfterDefeat = true;
            send("continue_observing");
        }
        const targetStep = machineView.step;
        const targetFactionId = machineView.exchange
            ? null
            : targetStep.type === "choice" && targetStep.marker
              ? markerTargetFor(targetStep)
              : targetStep.guide &&
                  targetStep.guide.kind === "world" &&
                  ["target_action", "nameplate", "player"].includes(targetStep.guide.target) &&
                  targetStep.trigger &&
                  ["attack", "contact", "alliance", "defeated", "fleet"].includes(targetStep.trigger.type)
                ? markerTargetFor(targetStep)
                : null;
        if (targetFactionId !== runtime.cameraTargetFactionId) {
            runtime.cameraTargetFactionId = targetFactionId;
            const focusPlayer = campaignPlayer(hud, targetFactionId);
            if (onMap(focusPlayer)) {
                send("focus_world", {
                    x: Number(focusPlayer.centroid_x) + 0.5,
                    y: Number(focusPlayer.centroid_y) + 0.5,
                });
            }
        }
        syncTutorialPause(machineView);
        let markerId = null;
        const exchangeSpeaker =
            machineView.exchange && runtime.definition!.speakers[machineView.exchange.speaker];
        const markerTarget = exchangeSpeaker
            ? exchangeSpeaker.faction || "player"
            : markerTargetFor(machineView.step);
        if (markerTarget) {
            const marked = campaignPlayer(hud, markerTarget);
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
                runtime.view!.render(null);
                if (machineView.exchange) syncCameraShot(machineView.step, hud);
                if (runtime.stepRevealStepId !== machineView.step.id) clearStepRevealTimer();
                if (runtime.stepRevealTimer === null && typeof window.setTimeout === "function") {
                    const waitingMachine = runtime.machine,
                        waitingStepId = machineView.step.id;
                    runtime.stepRevealStepId = waitingStepId;
                    const waitingToken = ++runtime.stepRevealToken;
                    const timerId = window.setTimeout(
                        function () {
                            if (
                                runtime.stepRevealToken !== waitingToken ||
                                runtime.stepRevealTimer !== timerId
                            )
                                return;
                            runtime.stepRevealTimer = null;
                            runtime.stepRevealStepId = null;
                            if (
                                runtime.machine === waitingMachine &&
                                runtime.latestHud &&
                                !runtime.modalOpen &&
                                waitingMachine.view().step.id === waitingStepId
                            )
                                update(runtime.latestHud);
                        },
                        Math.max(1, machineView.wait_remaining_ms),
                    );
                    runtime.stepRevealTimer = timerId;
                }
            } else {
                clearStepRevealTimer();
                const displayView = placementGuideView(machineView, hud);
                runtime.view!.render(
                    displayView,
                    renderContext(displayView.step, hud, anchorFor(displayView.step, hud), displayView),
                );
                syncCameraShot(machineView.step, hud);
            }
        }
        if (machineView.done && !runtime.completionSent) {
            runtime.completionSent = true;
            trackExperience("campaign_episode_complete", { episode_id: runtime.episodeId });
            send("complete_campaign_episode", { episode_id: runtime.episodeId });
        }
    }

    const menuGuide = createMenuGuide({ root, tr, asset, send, anchorFor, renderContext });
    window.SOW_menu_guide = menuGuide;

    // One emote path for every line: a named emote is shown over the speaker (the player, or the faction the
    // speaker plays) with its own line (pinned when emote_pinned), and "clear" releases the current one.
    // A step's or response's emote runs when it completes; exchange lines run as they play.
    function applyStepEmote(step: CampaignStep) {
        if (step.emote) applyEmote(step);
    }

    function applyEmote(step: CampaignStep) {
        const kind = step.emote === "clear" ? null : step.emote;
        const speaker =
            runtime.definition && runtime.definition.speakers && runtime.definition.speakers[step.speaker];
        const factionId = speaker && speaker.faction;
        const tutorial = runtime.latestHud && runtime.latestHud.tutorial;
        const fields = {
            kind: kind,
            pinned: step.emote_pinned === true,
            ...(factionId ? { faction_id: factionId } : {}),
        };
        if (kind && tutorial)
            Object.assign(fields, {
                token: "story:" + runtime.generation + ":" + ++runtime.emoteSerial,
                campaign_session_id: Number(tutorial.campaign_session_id) || 0,
                message: tr(step.emote_text_key),
            });
        send("tutorial_emote", fields);
    }

    function playExchangeLine(line: CampaignRecord, token: string | number) {
        const hud = runtime.latestHud,
            tutorial = hud && hud.tutorial;
        const nativeToken = (Number(runtime.generation) || 0) + ":" + token;
        const speaker = runtime.definition && runtime.definition.speakers[line.speaker];
        const faction = (speaker && speaker.faction) || "player";
        const player = hud && campaignPlayer(hud, faction);
        if (!player || !onMap(player)) return "unavailable";
        const step = runtime.machine && runtime.machine.view().step;
        if (step && step.camera && runtime.cameraShotStepId !== step.id) return false;
        if (tutorial!.facts && tutorial!.facts.machinima_active) return false;
        if (tutorial!.emote_token === nativeToken) return true;
        if (runtime.exchangeFocusToken !== token) {
            runtime.exchangeFocusToken = token;
            send("set_tutorial_marker", { player_id: Number(player.id) });
            runtime.markerId = Number(player.id);
            const anchor = cameraTargetAnchor({ marker: { target: faction } }, hud);
            if (!anchor || anchor.offscreen)
                send("focus_world", {
                    x: Number(player.centroid_x) + 0.5,
                    y: Number(player.centroid_y) + 0.5,
                });
        }
        if (runtime.exchangeCue && runtime.exchangeCue.token === token) return "pending";
        const nameplate = tutorial!.target_nameplate;
        if (
            Number(tutorial!.marker_player_id) !== Number(player.id) ||
            !nameplate ||
            nameplate.offscreen ||
            (tutorial!.facts && tutorial!.facts.machinima_active)
        )
            return false;
        runtime.exchangeCue = {
            token: token,
            nativeToken: nativeToken,
            line: line,
            faction: faction,
            session: Number(tutorial!.campaign_session_id),
        };
        if (
            !send("tutorial_emote", {
                kind: line.emote,
                token: nativeToken,
                campaign_session_id: runtime.exchangeCue!.session,
                message: tr(line.emote_text_key),
                ...(faction !== "player" ? { faction_id: faction } : {}),
            })
        ) {
            runtime.exchangeCue = null;
            return "unavailable";
        }
        return false;
    }

    function clearExchangeLine(line: CampaignRecord, token: string | number) {
        const cue = runtime.exchangeCue;
        if (!cue || cue.token !== token) return;
        send("tutorial_emote", {
            kind: null,
            token: cue.nativeToken,
            campaign_session_id: cue.session,
            ...(cue.faction !== "player" ? { faction_id: cue.faction } : {}),
        });
        runtime.exchangeCue = null;
    }

    function completeCampaignReaction(reaction: CampaignRecord, target: string) {
        if (reaction.when.type === "pre_defeat")
            send("continue_campaign_pre_defeat", {
                faction_id: target,
                campaign_session_id: runtime.campaignSessionId,
            });
    }

    function continueScene() {
        if (!runtime.machine) return;
        const step = runtime.machine.view().step;
        if (runtime.machine.advance(null, step.id)) update(runtime.latestHud);
    }

    function choose(choiceId: string) {
        if (!runtime.machine) return;
        const model = runtime.machine.view();
        const step = model.step;
        const isDialogChoice = Boolean(model.reactionData || (step && step.type === "choice"));
        if (model.reactionData) {
            let answer = (model.reactionData.choices || []).find(function (choice: CampaignRecord) {
                return choice.id === choiceId;
            });
            if (!answer) return;
            if (
                answer.relation === "allied" &&
                Number(answer.gold_cost || 0) > availableGold(runtime.latestHud)
            ) {
                answer = (model.reactionData.choices || []).find(function (choice: CampaignRecord) {
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

    function resolveReaction(
        reaction: CampaignRecord,
        targetName: string,
        choice: CampaignRecord | null,
        hud: CampaignRecord | null,
    ) {
        const outcome = choice ? choice.relation : reaction.outcome;
        if (!outcome) return true;
        const key = reaction.id + "@" + targetName;
        if (runtime.resolvedReactions.has(key)) return true;
        const target = ((hud && hud.players) || []).find(function (player: CampaignRecord) {
            return player && player.campaign_faction_id === targetName;
        });
        const human = ((hud && hud.players) || []).find(function (player: CampaignRecord) {
            return player && player.is_me;
        });
        const goldCost = Number(choice ? choice.gold_cost || 0 : reaction.gold_cost || 0);
        if (!target || !human || availableGold(hud) < goldCost) return false;
        if (
            !send("resolve_campaign_diplomacy", {
                target_player_id: Number(target.id),
                relation: outcome,
                gold_cost: goldCost,
            })
        )
            return false;
        runtime.resolvedReactions.add(key);
        return true;
    }

    function focusMarker() {
        const hud = runtime.latestHud;
        if (!hud || !runtime.machine) return;
        const step = runtime.machine.view().step;
        const target = markerTargetFor(step);
        const player =
            target === "player"
                ? (hud.players || []).find(function (item: CampaignRecord) {
                      return item && item.is_me;
                  })
                : (hud.players || []).find(function (item: CampaignRecord) {
                      return item && item.campaign_faction_id === target;
                  });
        if (player) send("focus_player", { player_id: Number(player.id) });
    }

    function syncTutorialPause(machineView: CampaignMachineView | null) {
        const paused = Boolean(machineView && machineView.paused);
        const cameraOnly = Boolean(paused && machineView && machineView.step.camera_only === true);
        const pausedAction = paused && machineView ? machineView.paused_action || null : null;
        if (
            paused !== runtime.uiPaused ||
            cameraOnly !== runtime.cameraOnly ||
            pausedAction !== runtime.pausedAction
        ) {
            runtime.uiPaused = paused;
            runtime.cameraOnly = cameraOnly;
            runtime.pausedAction = pausedAction;
            send("set_tutorial_paused", {
                paused: paused,
                camera_only: cameraOnly,
                paused_action: pausedAction,
            });
        }
    }

    function modalChanged() {
        const modal = document.getElementById("sow-hud-surrender-modal");
        const open = Boolean(modal && !modal.classList.contains("hidden"));
        if (open === runtime.modalOpen) return;
        runtime.modalOpen = open;
        if (runtime.machine) runtime.machine.setPaused(open || Boolean(document.hidden));
        if (open) {
            clearStepRevealTimer();
            runtime.uiPaused = true;
            runtime.pausedAction = null;
            send("set_tutorial_paused", {
                paused: true,
                camera_only: runtime.cameraOnly,
                paused_action: null,
            });
            root.hidden = true;
        } else if (runtime.machine) {
            if (runtime.latestHud) update(runtime.latestHud);
        }
    }

    function redrawCampaign() {
        if (!runtime.machine || !runtime.view || runtime.modalOpen) return;
        if (!runtime.latestHud) return;
        const model = runtime.machine.view();
        if (model.waiting) runtime.view.render(null);
        else {
            const displayView = placementGuideView(model, runtime.latestHud);
            runtime.view.render(
                displayView,
                renderContext(
                    displayView.step,
                    runtime.latestHud,
                    anchorFor(displayView.step, runtime.latestHud),
                    displayView,
                ),
            );
        }
    }
    window.addEventListener(
        "resize",
        function () {
            runtime.guidedScrollStepId = null;
            redrawCampaign();
        },
        { passive: true },
    );
    document.addEventListener("visibilitychange", function () {
        if (!runtime.machine) return;
        runtime.machine.setPaused(Boolean(document.hidden) || Boolean(runtime.modalOpen));
        clearStepRevealTimer();
        if (!document.hidden && !runtime.modalOpen && runtime.latestHud) update(runtime.latestHud);
    });

    if (hudRoot && typeof MutationObserver !== "undefined") {
        runtime.modalObserver = new MutationObserver(modalChanged);
        runtime.modalObserver!.observe(hudRoot, {
            subtree: true,
            attributes: true,
            attributeFilter: ["class"],
        });
    }

    function recordUiAction(event: Event) {
        if (!runtime.machine) return;
        const eventTarget = event.target;
        if (eventTarget instanceof Node && root.contains(eventTarget)) return;
        let changed = false;
        const currentStep = runtime.machine.view().step;
        Object.keys(window.SOWCampaign.UI_TARGETS).forEach(function (key) {
            // Targeted map actions count only after the HUD confirms the destination.
            if (
                key === "map_menu_open" ||
                (key === "map_transfer" &&
                    currentStep &&
                    currentStep.marker &&
                    currentStep.trigger &&
                    currentStep.trigger.type === "ui" &&
                    currentStep.trigger.action === key)
            )
                return;
            const control = window.SOWCampaignView.resolveUiTarget(
                key,
                document,
                runtime.episodeId || undefined,
            );
            const actionEvent = control && control.matches("input") ? "change" : "click";
            if (
                event.type === actionEvent &&
                control &&
                !control.matches(":disabled") &&
                control.getClientRects().length &&
                eventTarget instanceof Node &&
                (control === eventTarget || control.contains(eventTarget))
            ) {
                if (
                    key === "attack_ratio" &&
                    currentStep &&
                    currentStep.trigger &&
                    currentStep.trigger.type === "ui" &&
                    currentStep.trigger.action === key
                )
                    return;
                runtime.uiCounts[key] = (runtime.uiCounts[key] || 0) + 1;
                changed = true;
            }
        });
        if (changed && runtime.active && runtime.latestHud) update(runtime.latestHud);
    }
    ["click", "input", "change"].forEach(function (type) {
        document.addEventListener(type, recordUiAction, true);
    });

    window.SOW_startCampaignEpisode = function (episodeId: string) {
        startEpisode(episodeId, false);
    };
    window.SOW_watchCampaignCinematic = watchCampaignCinematic;
    window.SOW_tutorial_exit_context = tutorialExitContext;
    window.SOW_tutorial_menu_state_update = function (state: CampaignRecord) {
        const phase = state && state.phase;
        if (phase !== "MainMenu") menuGuide.pause();
        if (phase === "MainMenu" && runtime.lastPhase !== "MainMenu" && !(state && state.boot_campaign)) {
            reset();
        }
        runtime.lastPhase = phase;
        if (state && state.boot_campaign && runtime.bootEpisode !== state.boot_campaign)
            startEpisode(state.boot_campaign, true);
    };
    window.SOW_tutorial_camera_anchor_update = function (x: number, y: number) {
        if (runtime.view) runtime.view.updateCameraAnchor(Number(x), Number(y));
    };
    window.SOW_tutorial_state_update = function (state: CampaignRecord) {
        if (!state || state.phase !== "MainMenu") menuGuide.pause();
        const hud = state && state.phase === "Playing" ? state.hud : null;
        const tutorial = hud && hud.tutorial;
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
        if (runtime.campaignSessionId > 0)
            send("set_campaign_pre_defeat_targets", {
                faction_ids: [],
                campaign_session_id: runtime.campaignSessionId,
            });
        runtime.generation++;
        if (runtime.machine) runtime.machine.dispose();
        clearStepRevealTimer();
        syncTransferGuide(null);
        runtime.priorChoices = Object.create(null);
        runtime.episodeId = null;
        runtime.roster = null;
        runtime.definition = null;
        runtime.machine = null;
        runtime.exchangeCue = null;
        runtime.exchangeFocusToken = null;
        runtime.lastPreDefeatTargets = null;
        runtime.campaignSessionId = 0;
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
        if (runtime.view) {
            runtime.view.destroy();
            runtime.view = null;
        }
        const error = document.getElementById("sow-campaign-error");
        if (error) error.remove();
    }
})();
