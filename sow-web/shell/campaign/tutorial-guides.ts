import type { CampaignMachineView, CampaignRecord, CampaignStep } from "./types.ts";

export const TUTORIAL_HOVER_RADIUS = 72;

export type TutorialAnchor = CampaignRecord & {
    x: number;
    y: number;
    offscreen?: boolean;
    cameraTracked?: boolean;
    toX?: number;
    toY?: number;
};

export type TutorialRenderContext = CampaignRecord & {
    anchor: CampaignRecord | null;
    reducedMotion: boolean;
    direction: string;
    zoomMode?: string;
    hintOverride?: string;
    gestureHint?: string;
    gestureLabel?: string;
    gestureOverride?: string;
    guideMetric?: {
        kind?: string;
        current: number;
        target: number;
        direction?: string;
        label?: string;
    };
    statusOnly?: boolean;
    statusLabel?: string;
};

interface TutorialGuideState {
    episodeId: string | null;
    latestHud: CampaignRecord | null;
    cameraShotStepId: string | null;
    guidedScrollStepId: string | null;
    guidedControl: HTMLElement | null;
    guidedAmountControls: HTMLElement[];
}

interface TutorialGuideDependencies {
    window: Window;
    document: Document;
    runtime: TutorialGuideState;
    send(type: string, fields?: CampaignRecord): boolean;
    tr(key: string): string;
    hasText(key: string): boolean;
    stepTextKey(step: CampaignStep, suffix: string): string;
    zoomMode(): "pinch" | "trackpad" | "wheel";
}

export function createTutorialGuides(dependencies: TutorialGuideDependencies) {
    const { window, document, runtime, send, tr, hasText, stepTextKey, zoomMode } = dependencies;
    const HOVER_TARGET_RADIUS = TUTORIAL_HOVER_RADIUS;

    function targetPlayer(step: CampaignRecord | null, hud: CampaignRecord | null | undefined) {
        const target = step && ((step.trigger && step.trigger.target) || (step.marker && step.marker.target));
        const players = (hud && hud.players) || [];
        return (
            players.find(function (player: CampaignRecord) {
                return player && (target === "player" ? player.is_me : player.campaign_faction_id === target);
            }) || null
        );
    }

    function targetMapMenuOpen(step: CampaignStep, hud: CampaignRecord | null | undefined) {
        const target = targetPlayer(step, hud),
            menu = hud && hud.map_menu;
        return Boolean(
            target &&
            menu &&
            menu.open &&
            menu.view === "radial" &&
            Number(menu.target_player_id) === Number(target.id),
        );
    }

    function cameraTargetDistance(step: CampaignStep, hud: CampaignRecord | null) {
        const player = targetPlayer(step, hud),
            camera = (hud && hud.tutorial && hud.tutorial.camera) || {};
        const zoom = Number(camera.zoom),
            width = Number(camera.width),
            height = Number(camera.height);
        if (
            !player ||
            !Number.isFinite(Number(player.centroid_x)) ||
            !Number.isFinite(Number(player.centroid_y)) ||
            !Number.isFinite(zoom) ||
            zoom <= 0 ||
            !Number.isFinite(width) ||
            !Number.isFinite(height)
        )
            return null;
        const centerX = (width * 0.5 - Number(camera.x || 0)) / zoom;
        const centerY = (height * 0.5 - Number(camera.y || 0)) / zoom;
        return Math.hypot(Number(player.centroid_x) - centerX, Number(player.centroid_y) - centerY);
    }

    function cameraTargetAnchor(
        step: CampaignRecord,
        hud: CampaignRecord | null | undefined,
    ): TutorialAnchor | null {
        const player = targetPlayer(step, hud),
            camera = (hud && hud.tutorial && hud.tutorial.camera) || {};
        const zoom = Number(camera.zoom),
            scale = Math.max(Number(camera.scale) || 1, 0.01);
        const width = Number(camera.width) / scale,
            height = Number(camera.height) / scale;
        if (
            !player ||
            !Number.isFinite(Number(player.centroid_x)) ||
            !Number.isFinite(Number(player.centroid_y)) ||
            !Number.isFinite(zoom) ||
            zoom <= 0 ||
            !Number.isFinite(width) ||
            !Number.isFinite(height)
        )
            return null;
        const x = (Number(camera.x || 0) + Number(player.centroid_x) * zoom) / scale;
        const y = (Number(camera.y || 0) + Number(player.centroid_y) * zoom) / scale;
        const margin = Math.min(80, Math.max(24, Math.min(width, height) * 0.35));
        return {
            x: Math.max(margin, Math.min(width - margin, x)),
            y: Math.max(margin, Math.min(height - margin, y)),
            offscreen: x < 0 || x > width || y < 0 || y > height,
        };
    }

    function metricText(key: string, values: CampaignRecord) {
        let text = tr(key);
        Object.keys(values).forEach(function (name) {
            text = text.replace(new RegExp("\\{" + name + "\\}", "g"), String(values[name]));
        });
        return text;
    }

    function renderContext(
        step: CampaignStep,
        hud: CampaignRecord | null,
        anchor: CampaignRecord | null,
        machineView: CampaignMachineView,
    ) {
        const context: TutorialRenderContext = {
            anchor: anchor,
            reducedMotion: Boolean(hud && hud.settings && hud.settings.reduced_motion),
            direction: document.documentElement.dir,
        };
        if (step && step.guide) {
            context.zoomMode = zoomMode();
            const cameraCue =
                step.camera_only === true &&
                ["zoom_in", "zoom_out", "drag", "hover"].includes(step.guide.gesture);
            const facts = (hud && hud.tutorial && hud.tutorial.facts) || {};
            let hintKey = "";
            if (!cameraCue && (step.guide.gesture === "zoom_in" || step.guide.gesture === "zoom_out")) {
                const zoomHintKey = stepTextKey(step, "_" + context.zoomMode + "_hint");
                const zoomLabelKey = stepTextKey(step, "_" + context.zoomMode + "_label");
                if (hasText(zoomHintKey)) {
                    context.hintOverride = tr(zoomHintKey);
                    context.gestureHint = context.hintOverride;
                }
                if (hasText(zoomLabelKey)) context.gestureLabel = tr(zoomLabelKey);
                const zoomTarget =
                    step.trigger && step.trigger.type === "zoom_out_complete"
                        ? window.SOWCampaign.zoomOutTarget(facts, step.trigger.value)
                        : step.trigger && step.trigger.type === "zoom_in_complete"
                          ? facts.camera_zoom_target
                          : null;
                if (
                    zoomTarget != null &&
                    Number.isFinite(Number(zoomTarget)) &&
                    Number.isFinite(Number(facts.camera_zoom))
                ) {
                    context.guideMetric = {
                        current: Number(facts.camera_zoom),
                        target: Number(zoomTarget),
                        direction: step.guide.gesture === "zoom_in" ? "min" : "max",
                    };
                }
            } else if (!cameraCue && ["drag", "hover"].includes(step.guide.gesture)) {
                if (!cameraCue)
                    hintKey = stepTextKey(
                        step,
                        "_" + (context.zoomMode === "pinch" ? "mobile" : "desktop") + "_hint",
                    );
                if (
                    step.trigger &&
                    step.trigger.type === "camera_target" &&
                    Number.isFinite(Number(step.trigger.distance))
                ) {
                    const distance = cameraTargetDistance(step, hud);
                    let targetRadius = Number(facts.camera_target_radius);
                    if (!Number.isFinite(targetRadius) || targetRadius <= 0)
                        targetRadius = step.trigger.distance;
                    const progressKey = stepTextKey(step, "_progress");
                    if (distance !== null && Number.isFinite(distance)) {
                        const currentDistance = Math.ceil(distance),
                            targetDistance = Math.ceil(targetRadius);
                        context.guideMetric = {
                            kind: "distance",
                            current: currentDistance,
                            target: targetDistance,
                            label: hasText(progressKey)
                                ? metricText(progressKey, {
                                      current: currentDistance,
                                      target: targetDistance,
                                  })
                                : "",
                        };
                    }
                }
            }
            if (step.guide.gesture === "tap" && step.guide.show_label !== false) {
                const expansionResumed =
                    step.paused_action === "expand_once_then_resume" &&
                    machineView &&
                    machineView.paused_action == null;
                const actionSuffix = expansionResumed
                    ? "_continue_action"
                    : context.zoomMode === "pinch"
                      ? "_mobile_action"
                      : "_desktop_action";
                const actionKey = stepTextKey(step, actionSuffix);
                const action = window.SOWCampaign.guideAction(step);
                const fallbackKey =
                    action === "upgrade_progress"
                        ? "tutorial.hand_upgrade_progress"
                        : action === "interact"
                          ? "tutorial.hand_" + (context.zoomMode === "pinch" ? "tap" : "click")
                          : "tutorial.hand_" +
                            (context.zoomMode === "pinch" ? "tap" : "click") +
                            "_" +
                            action;
                context.gestureLabel = hasText(actionKey) ? tr(actionKey) : tr(fallbackKey);
            }
            if (step.trigger && step.trigger.type === "ui" && step.trigger.action === "map_menu_open") {
                context.gestureOverride = context.zoomMode === "pinch" ? "hold" : "tap";
            }
            if (hintKey && tr(hintKey) !== hintKey) {
                context.hintOverride = tr(hintKey);
                if (step.guide.gesture === "hover") context.gestureHint = context.hintOverride;
            }
        }
        if (step && step.type === "objective") {
            const guideAction = step.guide ? window.SOWCampaign.guideAction(step) : "";
            if (!step.guide || guideAction === "upgrade_progress") {
                context.statusOnly = true;
                context.statusLabel =
                    context.gestureLabel ||
                    (step.trigger && step.trigger.type === "structure_level"
                        ? tr("hud.building_in_progress")
                        : tr(step.hint_key || step.body_key));
            }
        }
        return context;
    }

    function anchorFor(
        step: CampaignStep,
        hud: CampaignRecord | null | undefined,
        guideFacts?: CampaignRecord,
    ): TutorialAnchor | null {
        if (!step) return null;
        const tutorial = (hud && hud.tutorial) || {};
        const triggerType = step.trigger && step.trigger.type;
        if (!step.guide) {
            if (step.type !== "objective") return null;
            if (triggerType === "structure_level") {
                const buildingNames: Record<string, string> = { city_level: "City", port_level: "Port" };
                const passiveKind = step.trigger.kind || buildingNames[String(triggerType)];
                const passiveBuilding =
                    tutorial.upgrading_buildings && tutorial.upgrading_buildings[passiveKind];
                if (
                    passiveBuilding &&
                    Number.isFinite(Number(passiveBuilding.x)) &&
                    Number.isFinite(Number(passiveBuilding.y))
                ) {
                    return { x: Number(passiveBuilding.x), y: Number(passiveBuilding.y) };
                }
            }
            const passivePlayer = tutorial.player;
            return passivePlayer &&
                Number.isFinite(Number(passivePlayer.x)) &&
                Number.isFinite(Number(passivePlayer.y))
                ? { x: Number(passivePlayer.x), y: Number(passivePlayer.y), cameraTracked: true }
                : null;
        }
        const guide = step.guide;
        const factionAction =
            step.trigger && ["attack", "contact", "alliance", "defeated", "fleet"].includes(triggerType);
        if (
            guide.kind === "world" &&
            factionAction &&
            ["target_action", "nameplate", "player"].includes(guide.target)
        ) {
            const factionNameplate = tutorial.target_nameplate;
            if (
                factionNameplate &&
                Number.isFinite(Number(factionNameplate.x)) &&
                Number.isFinite(Number(factionNameplate.y))
            ) {
                return {
                    x: Number(factionNameplate.x),
                    y: Number(factionNameplate.y),
                    offscreen: Boolean(factionNameplate.offscreen),
                    cameraTracked: true,
                };
            }
            return null;
        }

        if (guide.kind === "world" && guide.target === "upgrade_building") {
            const trigger = step.trigger || {};
            const buildingNames: Record<string, string> = { city_level: "City", port_level: "Port" };
            const buildingKind = trigger.kind || buildingNames[String(trigger.type)];
            const anchors = ["structure_level", "city_level", "port_level"].includes(trigger.type)
                ? tutorial.upgrading_buildings
                : tutorial.upgrade_buildings;
            const buildingAnchor = anchors && anchors[buildingKind];
            if (
                !buildingAnchor ||
                !Number.isFinite(Number(buildingAnchor.x)) ||
                !Number.isFinite(Number(buildingAnchor.y))
            ) {
                const upgradeWaitPlayer = trigger.type === "structure_level" && tutorial.player;
                return upgradeWaitPlayer &&
                    Number.isFinite(Number(upgradeWaitPlayer.x)) &&
                    Number.isFinite(Number(upgradeWaitPlayer.y))
                    ? { x: Number(upgradeWaitPlayer.x), y: Number(upgradeWaitPlayer.y), cameraTracked: true }
                    : null;
            }
            return { x: Number(buildingAnchor.x), y: Number(buildingAnchor.y) };
        }

        if (
            guide.kind === "world" &&
            guide.target === "player" &&
            step.trigger &&
            ["camera_target", "hover", "zoom_in_complete"].includes(step.trigger.type)
        ) {
            if (["hover", "zoom_in_complete"].includes(step.trigger.type)) {
                const nameplate = tutorial.nameplate;
                if (
                    nameplate &&
                    Number.isFinite(Number(nameplate.x)) &&
                    Number.isFinite(Number(nameplate.y))
                ) {
                    const pointer = tutorial.pointer;
                    if (
                        step.trigger.type === "hover" &&
                        pointer &&
                        Number.isFinite(Number(pointer.x)) &&
                        Number.isFinite(Number(pointer.y)) &&
                        Math.hypot(
                            Number(pointer.x) - Number(nameplate.x),
                            Number(pointer.y) - Number(nameplate.y),
                        ) <= HOVER_TARGET_RADIUS
                    ) {
                        return { x: Number(pointer.x), y: Number(pointer.y) };
                    }
                    return { x: Number(nameplate.x), y: Number(nameplate.y), cameraTracked: true };
                }
            }
            const cameraTarget = cameraTargetAnchor(step, hud);
            if (cameraTarget) {
                cameraTarget.cameraTracked = true;
                return cameraTarget;
            }
        }
        if (
            guide.kind === "world" &&
            guide.target === "target_action" &&
            step.trigger &&
            step.trigger.type === "defeated"
        ) {
            const targetNameplate = tutorial.target_nameplate;
            if (
                targetNameplate &&
                Number.isFinite(Number(targetNameplate.x)) &&
                Number.isFinite(Number(targetNameplate.y))
            ) {
                return {
                    x: Number(targetNameplate.x),
                    y: Number(targetNameplate.y),
                    offscreen: Boolean(targetNameplate.offscreen),
                    cameraTracked: true,
                };
            }
        }
        let result;
        if (guide.kind === "world") {
            const point = tutorial[guide.target];
            if (!point || !Number.isFinite(Number(point.x)) || !Number.isFinite(Number(point.y))) return null;
            result = { x: Number(point.x), y: Number(point.y) };
        } else {
            if (guide.target === "map_transfer" && step.marker && !targetMapMenuOpen(step, hud)) {
                const playerPoint = tutorial.player;
                return playerPoint &&
                    Number.isFinite(Number(playerPoint.x)) &&
                    Number.isFinite(Number(playerPoint.y))
                    ? { x: Number(playerPoint.x), y: Number(playerPoint.y) }
                    : null;
            }
            const source = window.SOWCampaignView.resolveUiTarget(
                guide.target,
                document,
                runtime.episodeId || undefined,
                guideFacts,
            );
            if (!source || source.matches(":disabled") || source.getClientRects().length === 0) return null;
            const spotlightPanel =
                step.id === "boudica_transfer_send" && source.closest
                    ? (source.closest("#sow-hud-transfer") as HTMLElement | null)
                    : null;
            if (
                step.id === "boudica_transfer_send" &&
                (!spotlightPanel || spotlightPanel.getClientRects().length === 0)
            )
                return null;
            result = window.SOWCampaignView.resolveUiAnchor(source, spotlightPanel);
            if (
                guide.target === "attack_ratio" &&
                step.trigger &&
                step.trigger.type === "ui" &&
                step.trigger.action === "attack_ratio" &&
                source.tagName === "INPUT" &&
                (source as HTMLInputElement).type === "range"
            ) {
                const slider = source as HTMLInputElement;
                const sliderMin = Number(slider.min),
                    sliderMax = Number(slider.max);
                const ratioTarget = sliderMin + (sliderMax - sliderMin) * Number(step.trigger.value || 1);
                result = window.SOWCampaignView.resolveUiRangeGuideAnchor(
                    slider,
                    Number(slider.value),
                    ratioTarget,
                );
                if (!result) return null;
            }
        }
        if (guide.gesture === "drag" && guide.to) {
            if (guide.kind === "world") {
                const worldDestination = tutorial[guide.to];
                if (
                    !worldDestination ||
                    !Number.isFinite(Number(worldDestination.x)) ||
                    !Number.isFinite(Number(worldDestination.y))
                )
                    return null;
                result.toX = Number(worldDestination.x);
                result.toY = Number(worldDestination.y);
            } else {
                const destination = window.SOWCampaignView.resolveUiTarget(
                    guide.to,
                    document,
                    runtime.episodeId || undefined,
                );
                if (!destination || destination.matches(":disabled") || !destination.getClientRects().length)
                    return null;
                const targetAnchor = window.SOWCampaignView.resolveUiAnchor(destination);
                result.toX = targetAnchor.x;
                result.toY = targetAnchor.y;
            }
        }
        return result;
    }

    function revealGuidedUiTarget(step: CampaignStep) {
        if (
            !step ||
            !step.guide ||
            step.guide.kind !== "ui" ||
            runtime.guidedScrollStepId === step.id ||
            !window.SOWCampaignView ||
            typeof window.SOWCampaignView.resolveUiTarget !== "function"
        )
            return;
        const target = window.SOWCampaignView.resolveUiTarget(
            step.guide.target,
            document,
            runtime.episodeId || undefined,
        );
        const strip = target && target.closest(".sow-hud__dock-actions");
        if (!target || target.matches(":disabled") || !strip || strip.scrollWidth <= strip.clientWidth + 1)
            return;
        const stripRect = strip.getBoundingClientRect();
        const targetRect = target.getBoundingClientRect();
        if (targetRect.left >= stripRect.left && targetRect.right <= stripRect.right) {
            runtime.guidedScrollStepId = step.id;
            return;
        }
        target.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "instant" });
        runtime.guidedScrollStepId = step.id;
    }

    function syncTransferGuide(step: CampaignStep | null) {
        const highlightAmounts = Boolean(
            step && step.guide && step.guide.kind === "ui" && step.guide.target === "transfer_send",
        );
        const guideTarget = step && step.guide && step.guide.kind === "ui" ? step.guide.target : null;
        const target =
            ["transfer_troops", "transfer_gold", "transfer_send"].includes(guideTarget) &&
            window.SOWCampaignView &&
            typeof window.SOWCampaignView.resolveUiTarget === "function"
                ? window.SOWCampaignView.resolveUiTarget(
                      guideTarget,
                      document,
                      runtime.episodeId || undefined,
                  )
                : null;
        let amountControls: HTMLElement[] = [];
        if (
            highlightAmounts &&
            window.SOWCampaignView &&
            typeof window.SOWCampaignView.resolveUiTarget === "function"
        ) {
            amountControls = ["transfer_troops", "transfer_gold"]
                .map(function (key) {
                    return window.SOWCampaignView.resolveUiTarget(
                        key,
                        document,
                        runtime.episodeId || undefined,
                    );
                })
                .filter((input): input is HTMLElement => input !== null);
        }
        runtime.guidedAmountControls.forEach(function (input) {
            if (!amountControls.includes(input)) input.classList.remove("sow-tutorial-guide-target");
        });
        amountControls.forEach(function (input) {
            input.classList.add("sow-tutorial-guide-target");
        });
        runtime.guidedAmountControls = amountControls;
        if (runtime.guidedControl !== target) {
            if (runtime.guidedControl) runtime.guidedControl.classList.remove("sow-tutorial-guide-target");
            runtime.guidedControl = target;
        }
        if (target) target.classList.add("sow-tutorial-guide-target");
    }

    function campaignPlayer(hud: CampaignRecord | null, factionId: string) {
        if (!hud) return null;
        return factionId === "player"
            ? (hud.players || []).find(function (player: CampaignRecord) {
                  return player && player.is_me;
              })
            : factionId &&
                  (hud.players || []).find(function (player: CampaignRecord) {
                      return player && player.campaign_faction_id === factionId;
                  });
    }

    function onMap(player: CampaignRecord | null) {
        return Boolean(
            player &&
            player.is_alive === true &&
            Number.isFinite(Number(player.centroid_x)) &&
            Number.isFinite(Number(player.centroid_y)),
        );
    }

    function syncCameraShot(step: CampaignStep, hud: CampaignRecord) {
        const camera = step.camera;
        if (!camera || runtime.cameraShotStepId === step.id) return;
        const to = campaignPlayer(hud, camera.to);
        if (!onMap(to)) return;
        let from = campaignPlayer(hud, camera.from || "player");
        if (!onMap(from)) from = to;
        if (
            send("focus_world", {
                x: Number(to.centroid_x) + 0.5,
                y: Number(to.centroid_y) + 0.5,
                shot: {
                    kind: camera.shot,
                    from_x: Number(from.centroid_x) + 0.5,
                    from_y: Number(from.centroid_y) + 0.5,
                    seconds: Number.isFinite(camera.seconds) ? camera.seconds : 3,
                    ...(camera.machinima === true ? { machinima: true } : {}),
                },
            })
        ) {
            runtime.cameraShotStepId = step.id;
        }
    }

    function markerTargetFor(step: CampaignStep) {
        const facts =
            (runtime.latestHud && runtime.latestHud.tutorial && runtime.latestHud.tutorial.facts) || {};
        const eliminated = facts.eliminated_faction_ids || [];
        if (step.trigger && Array.isArray(step.trigger.targets)) {
            const observed =
                step.trigger.type === "contact"
                    ? facts.contact_faction_ids || []
                    : facts.defeated_faction_ids || [];
            const remaining = step.trigger.targets.find(function (target: string) {
                return !observed.includes(target) && !eliminated.includes(target);
            });
            if (remaining) return remaining;
            return null;
        }
        if (
            step.trigger &&
            ["attack", "contact", "alliance", "defeated", "fleet", "resource_transfer"].includes(
                step.trigger.type,
            )
        ) {
            const targetFaction = step.trigger.target || step.trigger.recipient;
            if (targetFaction && targetFaction !== "player")
                return eliminated.includes(targetFaction) ? null : targetFaction;
        }
        if (
            step.guide &&
            step.guide.kind === "world" &&
            step.guide.target === "target_action" &&
            step.trigger &&
            step.trigger.target
        ) {
            return eliminated.includes(step.trigger.target) ? null : step.trigger.target;
        }
        if (
            step.trigger &&
            ["camera_target", "hover", "zoom_in_complete"].includes(step.trigger.type) &&
            step.trigger.target
        ) {
            return eliminated.includes(step.trigger.target) ? null : step.trigger.target;
        }
        return step.marker && !eliminated.includes(step.marker.target) ? step.marker.target : null;
    }
    return {
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
    };
}
