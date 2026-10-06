/* The editor previews this very same DOM/CSS presentation used over the battlefield. */
(function (host) {
    "use strict";
    let instance = 0;
    function mount(root, options) {
        const doc = root.ownerDocument;
        const uid = "sow-story-" + (++instance);
        let model = null, renderKey = "", lastBeatKey = "", focusBefore = null, wasModal = false, lastAction = -Infinity;
        let objectiveStepId = null;
        let guideWasVisible = false, cameraTracking = false, guideX = null, guideY = null, guideLabelWidth = 0, guideLabelMeasureKey = "", nudgeTimer = 0;
        root.classList.add("sow-story");
        root.innerHTML = '<div class="sow-story__shade" hidden></div>' +
            '<article class="sow-story__dialog" tabindex="-1" hidden>' +
                '<div class="sow-story__main"><div class="sow-story__portrait" hidden><img alt="" draggable="false"><span class="sow-story__portrait-line" aria-hidden="true"></span></div>' +
                '<div class="sow-story__conversation"><div class="sow-story__cinematic" hidden><video playsinline controls preload="metadata"></video><button class="sow-story__cinematic-play" type="button" data-story-play></button><button class="sow-story__cinematic-skip" type="button" data-story-skip></button></div><header class="sow-story__heading"><p class="sow-story__speaker"></p><button class="sow-story__close" type="button" data-story-dismiss>×</button></header>' +
                '<div class="sow-story__scroll"><h2 class="sow-story__title" id="' + uid + '-title"></h2><p class="sow-story__body" id="' + uid + '-body" aria-live="polite" aria-atomic="true"></p></div></div></div>' +
                '<div class="sow-story__actions"><div class="sow-story__choices"></div><footer class="sow-story__footer"><span class="sow-story__lines" aria-hidden="true"></span>' +
                '<button class="sow-story__continue" type="button" data-story-continue><span></span><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m8 5 7 7-7 7"/></svg></button></footer></div>' +
            '</article>' +
            '<aside class="sow-story__objective" hidden>' +
                '<div class="sow-story__objective-copy"><h3></h3><p></p><div class="sow-story__meter"><progress></progress><output></output></div></div>' +
                '<button class="sow-story__locate" type="button" data-story-focus>⌖</button></aside>' +
            '<div class="sow-story__spotlight" aria-hidden="true" hidden></div>' +
            '<div class="sow-story__gesture" data-tutorial-hand aria-hidden="true" hidden><span class="sow-story__ripple"></span><span class="sow-story__hand"><img alt="" aria-hidden="true" draggable="false"></span><span class="sow-story__zoom"><span class="sow-story__zoom-fingers"><i></i><i></i><b>↔</b></span><span class="sow-story__zoom-wheel">↕</span></span><span class="sow-story__pan-keys"><kbd>↑</kbd><span><kbd>←</kbd><kbd>↓</kbd><kbd>→</kbd></span></span><span class="sow-story__gesture-label" hidden><span class="sow-story__gesture-copy"></span><span class="sow-story__gesture-hint" hidden></span><span class="sow-story__gesture-metric" hidden><b class="sow-story__zoom-current"></b><i aria-hidden="true">→</i><b class="sow-story__zoom-target"></b></span></span></div>';
        const find = selector => root.querySelector(selector);
        const dialog = find(".sow-story__dialog"), shade = find(".sow-story__shade");
        const portrait = find(".sow-story__portrait"), image = portrait.querySelector("img");
        const speaker = find(".sow-story__speaker"), title = find(".sow-story__title"), body = find(".sow-story__body");
        const heading = find(".sow-story__heading"), conversation = find(".sow-story__conversation"), scrollContent = find(".sow-story__scroll"), actions = find(".sow-story__actions"), choices = find(".sow-story__choices"), footer = find(".sow-story__footer");
        const continueButton = find("[data-story-continue]"), lines = find(".sow-story__lines");
        const cinematic = find(".sow-story__cinematic"), cinematicVideo = cinematic.querySelector("video");
        const cinematicPlay = find("[data-story-play]"), cinematicSkip = find("[data-story-skip]");
        const objective = find(".sow-story__objective"), objectiveTitle = objective.querySelector("h3"), hint = objective.querySelector("p");
        const meter = objective.querySelector("progress"), amount = objective.querySelector("output");
        const gesture = find(".sow-story__gesture"), gestureLabel = find(".sow-story__gesture-label"), gestureCopy = find(".sow-story__gesture-copy"), gestureHint = find(".sow-story__gesture-hint");
        const gestureMetric = find(".sow-story__gesture-metric"), guideCurrent = find(".sow-story__zoom-current"), guideTarget = find(".sow-story__zoom-target");
        const spotlight = find(".sow-story__spotlight");
        const view = doc.defaultView;
        let cinematicStepKey = "", cinematicStatus = "none";
        let portraitFrame = 0, guideStepId = "", guideLastPulseValue = NaN, guideLastPulseAt = 0, guidePulse = null;
        function clearNudge() {
            if (nudgeTimer) { clearTimeout(nudgeTimer); nudgeTimer = 0; }
            dialog.classList.remove("is-nudged", "is-waiting");
        }
        function triggerNudge() {
            if (!model || model.step.type !== "choice") return;
            dialog.classList.remove("is-nudged", "is-waiting");
            void dialog.offsetWidth;
            dialog.classList.add("is-nudged");
            try {
                const nav = doc.defaultView && doc.defaultView.navigator;
                if (nav && typeof nav.vibrate === "function") nav.vibrate(12);
            } catch (ignored) { /* haptics are best-effort */ }
            if (nudgeTimer) clearTimeout(nudgeTimer);
            nudgeTimer = setTimeout(() => {
                nudgeTimer = 0;
                if (model && model.step.type === "choice" && !root.hidden) {
                    dialog.classList.remove("is-nudged");
                    dialog.classList.add("is-waiting");
                }
            }, 5000);
        }
        function syncMobilePortrait() {
            if (root.hidden) return;
            const shortLandscape = Boolean(view && view.matchMedia && view.matchMedia("(orientation: landscape) and (max-height: 560px) and (hover: none) and (pointer: coarse)").matches);
            if (root.clientWidth > 640 && !shortLandscape) { root.style.removeProperty("--story-portrait-size"); return; }
            if (portrait.hidden) return;
            const headingStyle = view && typeof view.getComputedStyle === "function" ? view.getComputedStyle(heading) : null;
            const headingGap = headingStyle ? parseFloat(headingStyle.marginBottom) || 0 : 0;
            const contentHeight = heading.getBoundingClientRect().height + headingGap + scrollContent.scrollHeight;
            const maxSize = Math.min(root.clientWidth * 0.36, root.clientHeight * 0.4);
            if (!contentHeight || !maxSize) return;
            const size = Math.min(contentHeight, maxSize);
            const current = parseFloat(root.style.getPropertyValue("--story-portrait-size"));
            if (!Number.isFinite(current) || Math.abs(current - size) >= 1) root.style.setProperty("--story-portrait-size", size + "px");
        }
        const ResizeObserverCtor = view && view.ResizeObserver;
        const portraitObserver = typeof ResizeObserverCtor === "function" ? new ResizeObserverCtor(() => {
            if (portraitFrame || !view) return;
            portraitFrame = view.requestAnimationFrame(() => { portraitFrame = 0; syncMobilePortrait(); });
        }) : null;
        if (portraitObserver) { portraitObserver.observe(conversation); portraitObserver.observe(root); }
        find(".sow-story__hand img").src = options.asset("gameplay/icons/tutorial_hand.webp");
        const dismissButtons = Array.from(root.querySelectorAll("[data-story-dismiss]"));
        const t = key => key ? options.translate(key) : "";
        function stopCinematic() {
            cinematicStepKey = ""; cinematicStatus = "none";
            cinematicVideo.pause(); cinematicVideo.removeAttribute("src"); cinematicVideo.load();
            cinematic.hidden = true; root.classList.remove("is-cinematic");
        }
        function syncCinematic(step) {
            const source = step.type === "scene" && step.presentation === "cinematic" ? step.video_src || "" : "";
            const nextKey = source ? step.id + "\u0000" + source : "";
            if (nextKey !== cinematicStepKey) {
                cinematicStepKey = nextKey; cinematicStatus = source ? "ready" : "none";
                cinematicVideo.pause(); cinematicVideo.removeAttribute("src");
                if (source) { cinematicVideo.src = source; cinematicVideo.load(); }
                else cinematicVideo.load();
            }
            const active = Boolean(source) && cinematicStatus !== "fallback";
            cinematic.hidden = !active;
            cinematicPlay.hidden = !active || cinematicStatus === "playing" || cinematicStatus === "ended";
            cinematicSkip.hidden = !active || cinematicStatus === "ended";
            cinematicPlay.textContent = t("play");
            cinematicSkip.textContent = t("tutorial.skip_cinematic");
            cinematicPlay.setAttribute("aria-label", t("play"));
            cinematicSkip.setAttribute("aria-label", t("tutorial.skip_cinematic"));
            root.classList.toggle("is-cinematic", active);
            root.classList.toggle("is-chapter", step.presentation === "chapter" || step.presentation === "cinematic" && !active);
            footer.hidden = step.type === "choice" || step.pause_game === true || active && cinematicStatus !== "ended";
        }
        function failCinematic() {
            if (!cinematicStepKey || cinematicStatus === "fallback") return;
            const focusVideo = doc.activeElement === cinematicVideo || doc.activeElement === cinematicPlay;
            cinematicStatus = "fallback";
            cinematicVideo.pause(); cinematicVideo.removeAttribute("src"); cinematicVideo.load();
            if (model && model.step) { syncCinematic(model.step); if (focusVideo) continueButton.focus({ preventScroll: true }); }
        }
        cinematicVideo.addEventListener("playing", () => {
            if (!cinematicStepKey || cinematicStatus === "fallback") return;
            cinematicStatus = "playing"; syncCinematic(model.step);
            if (doc.activeElement === cinematicPlay) cinematicVideo.focus({ preventScroll: true });
        });
        cinematicVideo.addEventListener("pause", () => {
            if (!cinematicStepKey || cinematicStatus !== "playing" || cinematicVideo.ended) return;
            cinematicStatus = "ready"; syncCinematic(model.step);
        });
        cinematicVideo.addEventListener("ended", () => {
            if (!cinematicStepKey || cinematicStatus === "fallback") return;
            cinematicStatus = "ended"; syncCinematic(model.step);
            continueButton.focus({ preventScroll: true });
        });
        cinematicVideo.addEventListener("error", failCinematic);
        function dismissDialog() {
            if (!model) return;
            if (model.step.type === "choice" || model.choices.length) { triggerNudge(); return; }
            if (options.onDismiss) options.onDismiss();
            else if (options.onContinue) options.onContinue();
        }
        function setText(node, value) { if (node.textContent !== value) node.textContent = value; }
        function formatZoom(value) {
            const number = Number(value);
            return Number.isFinite(number) ? number.toFixed(2).replace(/\.?0+$/, "") + "×" : "";
        }
        function clearGuideMetric() {
            if (guidePulse) { guidePulse.cancel(); guidePulse = null; }
            guideStepId = ""; guideLastPulseValue = NaN; guideLastPulseAt = 0;
            gestureMetric.hidden = true;
            gestureMetric.classList.remove("is-text");
            setText(guideCurrent, ""); setText(guideTarget, "");
        }
        function renderGuideMetric(metric, reducedMotion, stepId) {
            if (!metric) { clearGuideMetric(); return; }
            if (typeof metric.text === "string" && metric.text) {
                if (guidePulse) { guidePulse.cancel(); guidePulse = null; }
                guideStepId = stepId; guideLastPulseValue = NaN; guideLastPulseAt = 0;
                gestureMetric.hidden = false; gestureMetric.classList.add("is-text");
                setText(guideCurrent, ""); setText(guideTarget, metric.text);
                return;
            }
            if (metric.current == null || metric.target == null) { clearGuideMetric(); return; }
            const current = Number(metric && metric.current), target = Number(metric && metric.target);
            if (!Number.isFinite(current) || !Number.isFinite(target)) { clearGuideMetric(); return; }
            gestureMetric.classList.remove("is-text");
            gestureMetric.hidden = false;
            const operator = metric.direction === "min" ? "≥" : metric.direction === "max" ? "≤" : "";
            setText(guideTarget, operator + formatZoom(target));
            const now = view && view.performance ? view.performance.now() : Date.now();
            if (stepId !== guideStepId) {
                if (guidePulse) { guidePulse.cancel(); guidePulse = null; }
                guideStepId = stepId;
                guideLastPulseValue = current; guideLastPulseAt = now;
                setText(guideCurrent, formatZoom(current));
                return;
            }
            if (reducedMotion) {
                if (guidePulse) { guidePulse.cancel(); guidePulse = null; }
                guideLastPulseValue = current; guideLastPulseAt = now;
            }
            const currentText = formatZoom(current);
            if (guideCurrent.textContent === currentText) return;
            setText(guideCurrent, currentText);
            if (reducedMotion) return;
            if (Math.abs(current - guideLastPulseValue) >= 0.2 && now - guideLastPulseAt >= 100 && guideCurrent.animate) {
                if (guidePulse) guidePulse.cancel();
                guidePulse = guideCurrent.animate([
                    { opacity: 0.35, transform: "translateY(7px) scale(.78) rotateX(-22deg)", filter: "blur(2px)" },
                    { opacity: 1, transform: "translateY(-2px) scale(1.14) rotateX(0)", filter: "blur(0)", offset: 0.68 },
                    { opacity: 1, transform: "translateY(0) scale(1)", filter: "blur(0)" }
                ], { duration: 300, easing: "cubic-bezier(.16, 1.25, .3, 1)" });
                guideLastPulseValue = current; guideLastPulseAt = now;
            }
        }
        function releaseFocus() {
            if (focusBefore && focusBefore.isConnected && typeof focusBefore.focus === "function") focusBefore.focus({ preventScroll: true });
            focusBefore = null;
        }
        function focusAction() {
            const target = model.step.type === "choice" ? choices.querySelector("button") : cinematicStatus === "ready" ? cinematicPlay : model.step.pause_game ? dialog : continueButton;
            (target || dialog).focus({ preventScroll: true });
        }
        function render(next, context) {
            context = context || {};
            model = next;
            const wasHidden = root.hidden;
            root.hidden = !model || model.done;
            if (root.hidden) { guideWasVisible = false; cameraTracking = false; gesture.classList.remove("is-following", "is-camera-following"); clearNudge(); clearGuideMetric(); objectiveStepId = null; stopCinematic(); if (wasModal) releaseFocus(); wasModal = false; return; }
            const step = model.step, line = model.line || step;
            if (objectiveStepId !== step.id) { objectiveStepId = step.id; }
            footer.hidden = step.type === "choice" || step.pause_game === true;
            const beatKey = JSON.stringify([step.id, model.state && model.state.line]);
            const beatChanged = Boolean(lastBeatKey && beatKey !== lastBeatKey);
            lastBeatKey = beatKey;
            if (beatChanged || step.type !== "choice") clearNudge();
            const reducedMotion = Boolean(context.reducedMotion || (doc.defaultView && doc.defaultView.matchMedia && doc.defaultView.matchMedia("(prefers-reduced-motion: reduce)").matches));
            root.dir = context.direction || doc.documentElement.dir || "ltr";
            root.dataset.localeScript = context.localeScript || doc.documentElement.dataset.localeScript || "latin";
            const modal = ["scene", "choice", "end"].includes(step.type);
            root.classList.toggle("is-modal", modal);
            root.classList.toggle("is-celebration", step.presentation === "celebration");
            root.classList.toggle("is-reduced", reducedMotion);
            syncCinematic(step);
            root.dataset.stepId = step.id;
            root.dataset.stepType = step.type;
            dialog.hidden = !modal; shade.hidden = !modal; objective.hidden = modal || context.hideObjective === true;
            dialog.setAttribute("role", "dialog");
            dialog.setAttribute("aria-modal", "true");
            dialog.setAttribute("aria-labelledby", uid + (line.title_key || step.title_key ? "-title" : "-body"));
            dialog.setAttribute("aria-describedby", uid + "-body");
            const focusModal = modal && (!wasModal || wasHidden);
            if (modal && !wasModal) focusBefore = doc.activeElement;
            if (!modal && wasModal) releaseFocus();
            const speakerKey = line.speaker || step.speaker;
            const character = Object.assign({}, (model.definition.speakers || {})[speakerKey] || {});
            if (!speakerKey && model.reactionTarget) character.faction = model.reactionTarget;
            if (character.faction) {
                const roster = typeof options.roster === "function" ? options.roster() : options.roster;
                const faction = (roster && roster.factions || []).find(function (item) { return item.id === character.faction; });
                if (faction) { character.name = faction.name; character.name_key = null; character.avatar = faction.avatar || "null"; }
            }
            const speakerName = character.name_key ? t(character.name_key) : character.name || "";
            const copyTitle = t(line.title_key || step.title_key), copyBody = t(line.body_key || step.body_key) || (modal ? context.hintOverride || t(step.hint_key) : "");
            const key = JSON.stringify([step.id, model.state && model.state.line, copyTitle, copyBody, speakerName, character.avatar, step.presentation, step.video_src, model.choices.map(c => [c.id, t(c.label_key), t(c.body_key), c.gold_available, c.gold_insufficient])]);
            if (renderKey !== key) {
                renderKey = key;
                conversation.scrollTop = 0;
                scrollContent.scrollTop = 0;
                actions.scrollTop = 0;
                setText(speaker, speakerName); speaker.hidden = !speakerName;
                setText(title, copyTitle); title.hidden = !copyTitle;
                setText(body, copyBody); body.hidden = !copyBody;
                cinematicVideo.setAttribute("aria-label", copyTitle || speakerName || "Campaign cinematic");
                const avatar = character.avatar ? options.asset("gameplay/avatars/" + character.avatar + ".webp") : "";
                portrait.hidden = !avatar;
                if (avatar && image.getAttribute("src") !== avatar) image.src = avatar;
                image.alt = speakerName;
                root.classList.toggle("has-portrait", Boolean(avatar));
                choices.replaceChildren();
                model.choices.forEach(choice => {
                    const button = doc.createElement("button");
                    button.type = "button"; button.className = "sow-story__choice"; button.dataset.storyChoice = choice.id;
                    const mark = doc.createElement("span"); mark.className = "sow-story__choice-mark"; mark.textContent = "◇"; mark.setAttribute("aria-hidden", "true");
                    const copy = doc.createElement("span"); copy.className = "sow-story__choice-copy";
                    const label = doc.createElement("strong");
                    label.textContent = t(choice.label_key); copy.appendChild(label);
                    if (Number(choice.gold_cost || 0) > 0 && Number.isFinite(choice.gold_available)) {
                        const price = doc.createElement("span");
                        price.className = "sow-story__choice-price" + (choice.gold_insufficient ? " is-insufficient" : "");
                        const icon = doc.createElement("img");
                        icon.src = options.asset("gameplay/currency/gold.webp");
                        icon.alt = ""; icon.setAttribute("aria-hidden", "true");
                        const amounts = doc.createElement("span");
                        const formatGold = value => Number(value).toLocaleString(undefined, { maximumFractionDigits: 2 });
                        amounts.textContent = formatGold(choice.gold_available) + " / " + formatGold(choice.gold_cost);
                        price.append(icon, amounts); copy.appendChild(price);
                        button.setAttribute("aria-label", label.textContent + ". " + t("hud.gold") + " " + amounts.textContent);
                    }
                    button.append(mark, copy); choices.appendChild(button);
                });
                lines.replaceChildren();
                if (step.lines && step.lines.length > 1) step.lines.forEach((_, index) => {
                    const dot = doc.createElement("i"); dot.className = index === model.state.line ? "is-current" : ""; lines.appendChild(dot);
                });
                if (modal && (focusModal || beatChanged)) focusAction();
                if (beatChanged && !reducedMotion) {
                    [speaker, title, body, image, choices].forEach(node => {
                        if (!node.hidden && node.animate) node.animate(
                            [{ opacity: 0, transform: "translateY(6px)" }, { opacity: 1, transform: "translateY(0)" }],
                            { duration: 180, easing: "cubic-bezier(.2,.7,.2,1)" }
                        );
                    });
                }
                syncMobilePortrait();
            } else if (focusModal) focusAction();
            wasModal = modal;
            setText(continueButton.querySelector("span"), t(step.type === "end" ? "tutorial.complete" : "tutorial.continue"));
            const dismissContinues = modal && step.type !== "choice" && model.choices.length === 0;
            dismissButtons.forEach(button => {
                button.hidden = dismissContinues ? !options.onContinue : !(step.type === "choice" || options.onDismiss);
                button.setAttribute("aria-label", t(dismissContinues || step.type === "choice" ? "tutorial.continue" : "hud.leave_match"));
            });
            setText(objectiveTitle, t(step.title_key));
            setText(hint, context.hintOverride || t(step.hint_key || step.body_key)); hint.hidden = !hint.textContent;
            const progress = model.progress;
            objective.querySelector(".sow-story__meter").hidden = progress.target <= 1;
            meter.max = Math.max(1, progress.target); meter.value = progress.current;
            meter.setAttribute("aria-label", t(step.title_key));
            setText(amount, Math.floor(progress.current).toLocaleString() + " / " + Math.ceil(progress.target).toLocaleString());
            const focus = find("[data-story-focus]");
            const hasMapTarget = !(step.trigger && ["camera_target", "hover", "ui"].includes(step.trigger.type)) && (step.marker || (step.guide && step.guide.kind === "world" && step.guide.target === "target_action" && step.trigger && (step.trigger.target || Array.isArray(step.trigger.targets))));
            focus.hidden = !options.onFocus || !hasMapTarget;
            focus.setAttribute("aria-label", t("hud.center_camera"));
            objective.setAttribute("role", "status");
            if (beatChanged && !reducedMotion && objective.animate) objective.animate(
                [{ opacity: 0, transform: "translateY(-6px)" }, { opacity: 1, transform: "translateY(0)" }],
                { duration: 180, easing: "cubic-bezier(.2,.7,.2,1)" }
            );
            const gestureType = step.guide && step.guide.gesture;
            const zoomGuide = gestureType && gestureType.startsWith("zoom_");
            const zoomButtonGuide = step.guide && step.guide.kind === "ui" && ["hud_zoom_in", "hud_zoom_out"].includes(step.guide.target);
            const anchor = context.anchor;
            const labeledGesture = zoomGuide || zoomButtonGuide || ["drag", "pan_keys", "hover"].includes(gestureType) || Boolean(context.gestureLabel);
            gestureLabel.hidden = !labeledGesture;
            const gestureTitleKey = zoomGuide ? (gestureType === "zoom_in" ? "hud.zoom_in" : "hud.zoom_out")
                : zoomButtonGuide ? (step.guide.target === "hud_zoom_in" ? "hud.zoom_in" : "hud.zoom_out") : step.title_key;
            if (labeledGesture) setText(gestureCopy, zoomGuide || zoomButtonGuide ? t(gestureTitleKey) : context.gestureLabel || t(gestureTitleKey));
            const showGestureHint = gestureType === "hover" && Boolean(context.gestureHint || context.hintOverride);
            gestureHint.hidden = !showGestureHint;
            if (showGestureHint) setText(gestureHint, context.gestureHint || context.hintOverride);
            renderGuideMetric(context.guideMetric, reducedMotion, step.id);
            const guideVisible = !modal && step.guide && anchor && Number.isFinite(anchor.x) && Number.isFinite(anchor.y);
            gesture.hidden = !guideVisible; spotlight.hidden = !guideVisible || !anchor.width;
            if (guideVisible) {
                cameraTracking = anchor.cameraTracked === true;
                gesture.classList.toggle("is-camera-following", cameraTracking);
                gesture.dataset.gesture = step.guide.gesture;
                gesture.dataset.zoomMode = context.zoomMode || "pinch";
                gesture.dataset.guidePath = anchor.toX != null && anchor.toY != null ? "true" : "false";
                const follow = guideWasVisible && !wasHidden;
                gesture.classList.toggle("is-following", follow);
                if (!follow || anchor.x !== guideX || anchor.y !== guideY) {
                    gesture.style.transform = "translate3d(" + anchor.x + "px, " + anchor.y + "px, 0)";
                }
                guideX = anchor.x; guideY = anchor.y; guideWasVisible = true;
                if (labeledGesture) {
                    const measureKey = gestureCopy.textContent + ":" + gestureMetric.textContent + ":" + root.clientWidth;
                    if (measureKey !== guideLabelMeasureKey) {
                        guideLabelMeasureKey = measureKey;
                        guideLabelWidth = gestureLabel.getBoundingClientRect().width;
                    }
                    const halfLabel = guideLabelWidth / 2, viewportWidth = root.clientWidth;
                    const shift = Math.max(12 - (anchor.x - halfLabel), Math.min(0, viewportWidth - 12 - (anchor.x + halfLabel)));
                    const labelTransform = "translateX(calc(-50% + " + shift + "px))";
                    if (gestureLabel.style.transform !== labelTransform) gestureLabel.style.transform = labelTransform;
                }
                const direction = root.dir === "rtl" ? -1 : 1;
                const localDragX = anchor.width ? Math.min(64, anchor.width * 0.35) * direction : 0;
                gesture.style.setProperty("--guide-dx", ((anchor.toX == null ? anchor.x + localDragX : anchor.toX) - anchor.x) + "px");
                gesture.style.setProperty("--guide-dy", ((anchor.toY == null ? anchor.y : anchor.toY) - anchor.y) + "px");
                if (anchor.width) {
                    spotlight.style.left = (anchor.x - anchor.width / 2 - 6) + "px";
                    spotlight.style.top = (anchor.y - anchor.height / 2 - 6) + "px";
                    spotlight.style.width = (anchor.width + 12) + "px";
                    spotlight.style.height = (anchor.height + 12) + "px";
                }
            } else {
                gesture.classList.remove("is-following");
                gesture.classList.remove("is-camera-following");
                cameraTracking = false;
                guideWasVisible = false;
            }
        }
        function updateCameraAnchor(x, y) {
            if (!cameraTracking || root.hidden || !Number.isFinite(x) || !Number.isFinite(y)) return false;
            if (x !== guideX || y !== guideY) gesture.style.transform = "translate3d(" + x + "px, " + y + "px, 0)";
            guideX = x; guideY = y;
            return true;
        }
        function click(event) {
            event.stopPropagation();
            if (!model) return;
            const button = event.target.closest("button");
            if (!button || !root.contains(button)) return;
            event.preventDefault();
            if (event.timeStamp - lastAction < 220) return;
            lastAction = event.timeStamp;
            if (button.hasAttribute("data-story-choice")) options.onChoice(button.dataset.storyChoice);
            else if (button.hasAttribute("data-story-continue")) options.onContinue();
            else if (button.hasAttribute("data-story-play")) {
                const sourceKey = cinematicStepKey;
                const playback = cinematicVideo.play();
                if (playback && typeof playback.catch === "function") playback.catch(() => { if (cinematicStepKey === sourceKey) failCinematic(); });
            }
            else if (button.hasAttribute("data-story-skip")) options.onContinue();
            else if (button.hasAttribute("data-story-focus") && options.onFocus) options.onFocus();
            else if (button.hasAttribute("data-story-dismiss")) dismissDialog();
        }
        function outsideClick(event) {
            if (!model || root.hidden || !model.paused || root.contains(event.target)) return;
            const control = event.target.closest && event.target.closest("button, a[href], input:not([type='hidden']), select, textarea, [role='button'], [data-command], [data-map-action]");
            if (control) return;
            event.preventDefault();
            event.stopPropagation();
            if (model.step.type === "scene") return;
            if (model.step.type !== "choice" && event.timeStamp - lastAction >= 220) {
                lastAction = event.timeStamp;
                options.onContinue();
            } else if (model.step.type === "choice" && event.timeStamp - lastAction >= 220) {
                lastAction = event.timeStamp;
                triggerNudge();
            }
        }
        function stop(event) { event.stopPropagation(); }
        function keys(event) {
            if (!model || root.hidden) return;
            if (model.paused) event.stopPropagation();
            if (event.repeat) return;
            if (event.key === "Escape" && (options.onDismiss || options.onContinue)) { event.preventDefault(); dismissDialog(); }
            else if (event.key === "Tab" && model.paused && ["scene", "choice", "end"].includes(model.step.type)) {
                const buttons = Array.from(dialog.querySelectorAll("button")).filter(button => !button.hidden && !button.closest("[hidden]"))
                    .sort((a, b) => Number(a.hasAttribute("data-story-dismiss")) - Number(b.hasAttribute("data-story-dismiss")));
                const index = buttons.indexOf(doc.activeElement);
                const next = event.shiftKey ? (index <= 0 ? buttons.length - 1 : index - 1) : (index + 1) % buttons.length;
                event.preventDefault();
                if (buttons.length) buttons[next].focus();
                else dialog.focus({ preventScroll: true });
            } else if ((event.key === "Enter" || event.key === " ") && doc.activeElement === dialog && model.step.type !== "choice" && !model.step.pause_game) {
                event.preventDefault(); continueButton.click();
            }
        }
        image.addEventListener("error", () => { portrait.hidden = true; root.classList.remove("has-portrait"); });
        root.addEventListener("click", click);
        doc.addEventListener("click", outsideClick, true);
        ["pointerdown", "pointerup", "touchstart", "touchend", "wheel"].forEach(type => root.addEventListener(type, stop, { passive: true }));
        root.addEventListener("keydown", keys);
        return {
            render,
            updateCameraAnchor,
            destroy() {
                releaseFocus();
                clearNudge();
                clearGuideMetric();
                stopCinematic();
                if (portraitObserver) portraitObserver.disconnect();
                root.style.removeProperty("--story-portrait-size");
                root.removeEventListener("click", click); root.removeEventListener("keydown", keys);
                doc.removeEventListener("click", outsideClick, true);
                ["pointerdown", "pointerup", "touchstart", "touchend", "wheel"].forEach(type => root.removeEventListener(type, stop));
                root.replaceChildren(); root.classList.remove("sow-story", "is-modal"); root.hidden = true;
            }
        };
    }
    host.SOWCampaignView = { mount };
})(typeof globalThis !== "undefined" ? globalThis : this);
