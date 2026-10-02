/* The editor previews this very same DOM/CSS presentation used over the battlefield. */
(function (host) {
    "use strict";
    let instance = 0;
    function mount(root, options) {
        const doc = root.ownerDocument;
        const uid = "sow-story-" + (++instance);
        let model = null, renderKey = "", lastBeatKey = "", focusBefore = null, wasModal = false, lastAction = -Infinity;
        let guideWasVisible = false, guideX = null, guideY = null, nudgeTimer = 0;
        root.classList.add("sow-story");
        root.innerHTML = '<div class="sow-story__shade" hidden></div>' +
            '<article class="sow-story__dialog" tabindex="-1" hidden>' +
                '<div class="sow-story__main"><div class="sow-story__portrait" hidden><img alt="" draggable="false"><span class="sow-story__portrait-line" aria-hidden="true"></span></div>' +
                '<div class="sow-story__conversation"><header class="sow-story__heading"><p class="sow-story__speaker"></p><button class="sow-story__close" type="button" data-story-dismiss>×</button></header>' +
                '<div class="sow-story__scroll"><h2 class="sow-story__title" id="' + uid + '-title"></h2><p class="sow-story__body" id="' + uid + '-body" aria-live="polite" aria-atomic="true"></p></div></div></div>' +
                '<div class="sow-story__actions"><div class="sow-story__choices"></div><footer class="sow-story__footer"><span class="sow-story__lines" aria-hidden="true"></span>' +
                '<button class="sow-story__continue" type="button" data-story-continue><span></span><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m8 5 7 7-7 7"/></svg></button></footer></div>' +
            '</article>' +
            '<aside class="sow-story__objective" hidden><span class="sow-story__objective-mark" aria-hidden="true">◇</span>' +
                '<div class="sow-story__objective-copy"><div class="sow-story__objective-speaker" hidden><img alt="" draggable="false"><span></span></div><h3></h3><p></p><div class="sow-story__meter"><progress></progress><output></output></div></div>' +
                '<button class="sow-story__locate" type="button" data-story-focus>⌖</button></aside>' +
            '<div class="sow-story__spotlight" aria-hidden="true" hidden></div>' +
            '<div class="sow-story__gesture" data-tutorial-hand aria-hidden="true" hidden><span class="sow-story__ripple"></span><span class="sow-story__hand"><img alt="" aria-hidden="true" draggable="false"></span><span class="sow-story__zoom"><span class="sow-story__zoom-fingers"><i></i><i></i><b>↔</b></span><span class="sow-story__zoom-wheel">↕</span></span></div>';
        const find = selector => root.querySelector(selector);
        const dialog = find(".sow-story__dialog"), shade = find(".sow-story__shade");
        const portrait = find(".sow-story__portrait"), image = portrait.querySelector("img");
        const speaker = find(".sow-story__speaker"), title = find(".sow-story__title"), body = find(".sow-story__body");
        const conversation = find(".sow-story__conversation"), scrollContent = find(".sow-story__scroll"), actions = find(".sow-story__actions"), choices = find(".sow-story__choices"), footer = find(".sow-story__footer");
        const continueButton = find("[data-story-continue]"), lines = find(".sow-story__lines");
        const objective = find(".sow-story__objective"), objectiveTitle = objective.querySelector("h3"), hint = objective.querySelector("p");
        const objectiveSpeaker = find(".sow-story__objective-speaker"), objectiveSpeakerImage = objectiveSpeaker.querySelector("img"), objectiveSpeakerName = objectiveSpeaker.querySelector("span");
        const meter = objective.querySelector("progress"), amount = objective.querySelector("output");
        const gesture = find(".sow-story__gesture"), spotlight = find(".sow-story__spotlight");
        const view = doc.defaultView;
        let portraitFrame = 0;
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
            const contentHeight = conversation.getBoundingClientRect().height;
            const maxSize = Math.min(144, root.clientWidth * 0.36);
            if (!contentHeight || !maxSize) return;
            const size = Math.max(64, Math.min(contentHeight, maxSize));
            const current = parseFloat(root.style.getPropertyValue("--story-portrait-size"));
            if (!Number.isFinite(current) || Math.abs(current - size) >= 1) root.style.setProperty("--story-portrait-size", size + "px");
        }
        const ResizeObserverCtor = view && view.ResizeObserver;
        const portraitObserver = typeof ResizeObserverCtor === "function" ? new ResizeObserverCtor(() => {
            if (portraitFrame || !view) return;
            portraitFrame = view.requestAnimationFrame(() => { portraitFrame = 0; syncMobilePortrait(); });
        }) : null;
        if (portraitObserver) portraitObserver.observe(conversation);
        find(".sow-story__hand img").src = options.asset("gameplay/icons/tutorial_hand.webp");
        const dismissButtons = Array.from(root.querySelectorAll("[data-story-dismiss]"));
        const t = key => key ? options.translate(key) : "";
        function setText(node, value) { if (node.textContent !== value) node.textContent = value; }
        function releaseFocus() {
            if (focusBefore && focusBefore.isConnected && typeof focusBefore.focus === "function") focusBefore.focus({ preventScroll: true });
            focusBefore = null;
        }
        function focusAction() {
            const target = model.step.type === "choice" ? choices.querySelector("button") : model.step.pause_game ? dialog : continueButton;
            (target || dialog).focus({ preventScroll: true });
        }
        function render(next, context) {
            context = context || {};
            model = next;
            const wasHidden = root.hidden;
            root.hidden = !model || model.done;
            if (root.hidden) { guideWasVisible = false; clearNudge(); if (wasModal) releaseFocus(); wasModal = false; return; }
            const step = model.step, line = model.line || step;
            footer.hidden = step.type === "choice" || step.pause_game === true;
            const beatKey = JSON.stringify([step.id, model.state && model.state.line]);
            const beatChanged = Boolean(lastBeatKey && beatKey !== lastBeatKey);
            lastBeatKey = beatKey;
            if (beatChanged || step.type !== "choice") clearNudge();
            const reducedMotion = Boolean(context.reducedMotion || (doc.defaultView && doc.defaultView.matchMedia && doc.defaultView.matchMedia("(prefers-reduced-motion: reduce)").matches));
            root.dir = context.direction || doc.documentElement.dir || "ltr";
            root.dataset.localeScript = context.localeScript || doc.documentElement.dataset.localeScript || "latin";
            const modal = Boolean(model.paused);
            root.classList.toggle("is-modal", modal);
            root.classList.toggle("is-chapter", step.presentation === "chapter");
            root.classList.toggle("is-reduced", reducedMotion);
            root.dataset.stepId = step.id;
            root.dataset.stepType = step.type;
            dialog.hidden = !modal; shade.hidden = !modal; objective.hidden = modal;
            dialog.setAttribute("role", "dialog");
            dialog.setAttribute("aria-modal", "true");
            dialog.setAttribute("aria-labelledby", uid + (line.title_key || step.title_key ? "-title" : "-body"));
            dialog.setAttribute("aria-describedby", uid + "-body");
            const focusModal = modal && (!wasModal || wasHidden);
            if (modal && !wasModal) focusBefore = doc.activeElement;
            if (!modal && wasModal) releaseFocus();
            const character = Object.assign({}, (model.definition.speakers || {})[line.speaker || step.speaker] || {});
            if (character.faction) {
                const roster = typeof options.roster === "function" ? options.roster() : options.roster;
                const faction = (roster && roster.factions || []).find(function (item) { return item.name === character.faction; });
                if (faction) { character.name = faction.name; character.name_key = null; character.avatar = faction.avatar || "null"; }
            }
            const speakerName = character.name_key ? t(character.name_key) : character.name || "";
            const copyTitle = t(line.title_key || step.title_key), copyBody = t(line.body_key || step.body_key) || (modal ? context.hintOverride || t(step.hint_key) : "");
            const key = JSON.stringify([step.id, model.state && model.state.line, copyTitle, copyBody, speakerName, character.avatar, model.choices.map(c => [c.id, t(c.label_key), t(c.body_key), c.gold_available, c.gold_insufficient])]);
            if (renderKey !== key) {
                renderKey = key;
                conversation.scrollTop = 0;
                scrollContent.scrollTop = 0;
                actions.scrollTop = 0;
                setText(speaker, speakerName); speaker.hidden = !speakerName;
                setText(title, copyTitle); title.hidden = !copyTitle;
                setText(body, copyBody); body.hidden = !copyBody;
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
                    if (choice.body_key) { const detail = doc.createElement("small"); detail.textContent = t(choice.body_key); copy.appendChild(detail); }
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
            dismissButtons.forEach(button => { button.hidden = !options.onDismiss; button.setAttribute("aria-label", t("hud.leave_match")); });
            setText(objectiveTitle, t(step.title_key));
            objectiveSpeaker.hidden = !step.speaker || !speakerName;
            setText(objectiveSpeakerName, speakerName);
            const objectiveAvatar = character.avatar ? options.asset("gameplay/avatars/" + character.avatar + ".webp") : "";
            objectiveSpeakerImage.hidden = !objectiveAvatar;
            if (objectiveAvatar && objectiveSpeakerImage.getAttribute("src") !== objectiveAvatar) objectiveSpeakerImage.src = objectiveAvatar;
            else if (!objectiveAvatar) objectiveSpeakerImage.removeAttribute("src");
            setText(hint, context.hintOverride || t(step.hint_key || step.body_key)); hint.hidden = !hint.textContent;
            const progress = model.progress;
            objective.querySelector(".sow-story__meter").hidden = progress.target <= 1;
            meter.max = Math.max(1, progress.target); meter.value = progress.current;
            meter.setAttribute("aria-label", t(step.title_key));
            setText(amount, Math.floor(progress.current).toLocaleString() + " / " + Math.ceil(progress.target).toLocaleString());
            setText(find(".sow-story__objective-mark"), "◇");
            const focus = find("[data-story-focus]");
            const hasMapTarget = step.marker || (step.guide && step.guide.kind === "world" && step.guide.target === "target_action" && step.trigger && (step.trigger.target || Array.isArray(step.trigger.targets)));
            focus.hidden = !options.onFocus || !hasMapTarget;
            focus.setAttribute("aria-label", t("hud.center_camera"));
            objective.setAttribute("role", "status");
            if (beatChanged && !modal && !reducedMotion && objective.animate) objective.animate(
                [{ opacity: 0, transform: "translateY(-6px)" }, { opacity: 1, transform: "translateY(0)" }],
                { duration: 180, easing: "cubic-bezier(.2,.7,.2,1)" }
            );
            const anchor = context.anchor;
            const guideVisible = !modal && step.guide && anchor && Number.isFinite(anchor.x) && Number.isFinite(anchor.y);
            gesture.hidden = !guideVisible; spotlight.hidden = !guideVisible || !anchor.width;
            if (guideVisible) {
                gesture.dataset.gesture = step.guide.gesture;
                gesture.dataset.zoomMode = context.zoomMode || "pinch";
                const follow = guideWasVisible && !wasHidden;
                gesture.classList.toggle("is-following", follow);
                if (!follow || anchor.x !== guideX || anchor.y !== guideY) {
                    gesture.style.transform = "translate3d(" + anchor.x + "px, " + anchor.y + "px, 0)";
                }
                guideX = anchor.x; guideY = anchor.y; guideWasVisible = true;
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
                guideWasVisible = false;
            }
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
            else if (button.hasAttribute("data-story-focus") && options.onFocus) options.onFocus();
            else if (button.hasAttribute("data-story-dismiss") && options.onDismiss) options.onDismiss();
        }
        function outsideClick(event) {
            if (!model || root.hidden || !model.paused || root.contains(event.target)) return;
            const control = event.target.closest && event.target.closest("button, a[href], input:not([type='hidden']), select, textarea, [role='button'], [data-command], [data-map-action]");
            if (control) return;
            event.preventDefault();
            event.stopPropagation();
            if (model.step.type !== "choice" && !model.step.pause_game && event.timeStamp - lastAction >= 220) {
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
            if (event.key === "Escape" && options.onDismiss) { event.preventDefault(); options.onDismiss(); }
            else if (event.key === "Tab" && model.paused) {
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
            destroy() {
                releaseFocus();
                clearNudge();
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
