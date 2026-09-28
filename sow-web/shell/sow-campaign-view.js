/* The editor previews this very same DOM/CSS presentation used over the battlefield. */
(function (host) {
    "use strict";
    let instance = 0;
    const hand = '<svg viewBox="0 0 64 64" aria-hidden="true"><path d="M25 5c-3 0-5 2-5 5v23l-6-5c-3-2-6-1-8 1s-1 6 1 8l13 12c3 3 7 5 11 5h10c8 0 14-7 14-15V25c0-3-2-5-5-5-2 0-3 1-4 2v-3c0-3-2-5-5-5-2 0-3 1-4 2v-3c0-3-2-5-5-5-1 0-2 0-3 1-1-3-2-4-4-4Z"/></svg>';
    function mount(root, options) {
        const doc = root.ownerDocument;
        const uid = "sow-story-" + (++instance);
        let model = null, renderKey = "", lastBeatKey = "", focusBefore = null, wasModal = false, lastAction = -Infinity;
        root.classList.add("sow-story");
        root.innerHTML = '<div class="sow-story__shade" hidden></div>' +
            '<article class="sow-story__dialog" tabindex="-1" hidden>' +
                '<div class="sow-story__portrait" hidden><img alt="" draggable="false"><span class="sow-story__portrait-line" aria-hidden="true"></span></div>' +
                '<div class="sow-story__conversation"><header class="sow-story__heading"><p class="sow-story__speaker"></p><button class="sow-story__close" type="button" data-story-dismiss>×</button></header>' +
                '<h2 class="sow-story__title" id="' + uid + '-title"></h2><p class="sow-story__body" id="' + uid + '-body" aria-live="polite" aria-atomic="true"></p>' +
                '<div class="sow-story__choices"></div><footer class="sow-story__footer"><span class="sow-story__lines" aria-hidden="true"></span>' +
                '<button class="sow-story__continue" type="button" data-story-continue><span></span><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m8 5 7 7-7 7"/></svg></button></footer></div>' +
            '</article>' +
            '<aside class="sow-story__objective" hidden><span class="sow-story__objective-mark" aria-hidden="true">◇</span>' +
                '<div class="sow-story__objective-copy"><div class="sow-story__objective-speaker" hidden><img alt="" draggable="false"><span></span></div><h3></h3><p></p><div class="sow-story__meter"><progress></progress><output></output></div></div>' +
                '<button class="sow-story__locate" type="button" data-story-focus>⌖</button></aside>' +
            '<div class="sow-story__spotlight" aria-hidden="true" hidden></div>' +
            '<div class="sow-story__gesture" data-tutorial-hand aria-hidden="true" hidden><span class="sow-story__ripple"></span><span class="sow-story__hand">' + hand + '</span></div>';
        const find = selector => root.querySelector(selector);
        const dialog = find(".sow-story__dialog"), shade = find(".sow-story__shade");
        const portrait = find(".sow-story__portrait"), image = portrait.querySelector("img");
        const speaker = find(".sow-story__speaker"), title = find(".sow-story__title"), body = find(".sow-story__body");
        const conversation = find(".sow-story__conversation"), choices = find(".sow-story__choices"), footer = find(".sow-story__footer");
        const continueButton = find("[data-story-continue]"), lines = find(".sow-story__lines");
        const objective = find(".sow-story__objective"), objectiveTitle = objective.querySelector("h3"), hint = objective.querySelector("p");
        const objectiveSpeaker = find(".sow-story__objective-speaker"), objectiveSpeakerImage = objectiveSpeaker.querySelector("img"), objectiveSpeakerName = objectiveSpeaker.querySelector("span");
        const meter = objective.querySelector("progress"), amount = objective.querySelector("output");
        const gesture = find(".sow-story__gesture"), spotlight = find(".sow-story__spotlight");
        const dismissButtons = Array.from(root.querySelectorAll("[data-story-dismiss]"));
        const t = key => key ? options.translate(key) : "";
        function setText(node, value) { if (node.textContent !== value) node.textContent = value; }
        function releaseFocus() {
            if (focusBefore && focusBefore.isConnected && typeof focusBefore.focus === "function") focusBefore.focus({ preventScroll: true });
            focusBefore = null;
        }
        function focusAction() {
            const target = model.step.type === "choice" ? choices.querySelector("button") : continueButton;
            (target || dialog).focus({ preventScroll: true });
        }
        function render(next, context) {
            context = context || {};
            model = next;
            const wasHidden = root.hidden;
            root.hidden = !model || model.done;
            if (root.hidden) { if (wasModal) releaseFocus(); wasModal = false; return; }
            const step = model.step, line = model.line || step;
            footer.hidden = step.type === "choice";
            const beatKey = JSON.stringify([step.id, model.state && model.state.line]);
            const beatChanged = Boolean(lastBeatKey && beatKey !== lastBeatKey);
            lastBeatKey = beatKey;
            const reducedMotion = Boolean(context.reducedMotion || (doc.defaultView && doc.defaultView.matchMedia && doc.defaultView.matchMedia("(prefers-reduced-motion: reduce)").matches));
            root.dir = context.direction || doc.documentElement.dir || "ltr";
            root.dataset.localeScript = context.localeScript || doc.documentElement.dataset.localeScript || "latin";
            const modal = Boolean(model.paused);
            root.classList.toggle("is-modal", modal);
            root.classList.toggle("is-chapter", step.presentation === "chapter");
            root.classList.toggle("is-ready", Boolean(model.ready));
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
            const character = (model.definition.speakers || {})[line.speaker || step.speaker] || {};
            const speakerName = character.name_key ? t(character.name_key) : character.name || "";
            const copyTitle = t(line.title_key || step.title_key), copyBody = t(line.body_key || step.body_key);
            const key = JSON.stringify([step.id, model.state && model.state.line, copyTitle, copyBody, speakerName, character.avatar, model.choices.map(c => [c.id, t(c.label_key), t(c.body_key)])]);
            if (renderKey !== key) {
                renderKey = key;
                conversation.scrollTop = 0;
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
                    const copy = doc.createElement("span"), label = doc.createElement("strong");
                    label.textContent = t(choice.label_key); copy.appendChild(label);
                    if (choice.body_key) { const detail = doc.createElement("small"); detail.textContent = t(choice.body_key); copy.appendChild(detail); }
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
            setText(hint, t(step.hint_key || step.body_key)); hint.hidden = !hint.textContent;
            const progress = model.progress;
            objective.querySelector(".sow-story__meter").hidden = progress.target <= 1;
            meter.max = Math.max(1, progress.target); meter.value = progress.current;
            meter.setAttribute("aria-label", t(step.title_key));
            setText(amount, Math.floor(progress.current).toLocaleString() + " / " + Math.ceil(progress.target).toLocaleString());
            setText(find(".sow-story__objective-mark"), model.ready ? "✓" : "◇");
            const focus = find("[data-story-focus]");
            const hasMapTarget = step.marker || (step.guide && step.guide.kind === "world" && step.guide.target === "target_action" && step.trigger && step.trigger.target);
            focus.hidden = !options.onFocus || !hasMapTarget;
            focus.setAttribute("aria-label", t("hud.center_camera"));
            objective.setAttribute("role", "status");
            if (beatChanged && !modal && !reducedMotion && objective.animate) objective.animate(
                [{ opacity: 0, transform: "translateY(-6px)" }, { opacity: 1, transform: "translateY(0)" }],
                { duration: 180, easing: "cubic-bezier(.2,.7,.2,1)" }
            );
            const anchor = context.anchor;
            const guideVisible = !modal && !model.ready && step.guide && anchor && Number.isFinite(anchor.x) && Number.isFinite(anchor.y);
            gesture.hidden = !guideVisible; spotlight.hidden = !guideVisible || !anchor.width;
            if (guideVisible) {
                gesture.dataset.gesture = step.guide.gesture;
                gesture.style.left = anchor.x + "px"; gesture.style.top = anchor.y + "px";
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
            }
        }
        function click(event) {
            const button = event.target.closest("button");
            event.stopPropagation();
            if (!button || !root.contains(button) || button.disabled || !model) return;
            event.preventDefault();
            if (event.timeStamp - lastAction < 220) return;
            lastAction = event.timeStamp;
            if (button.hasAttribute("data-story-choice")) options.onChoice(button.dataset.storyChoice);
            else if (button.hasAttribute("data-story-continue")) options.onContinue();
            else if (button.hasAttribute("data-story-focus") && options.onFocus) options.onFocus();
            else if (button.hasAttribute("data-story-dismiss") && options.onDismiss) options.onDismiss();
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
                if (buttons.length) { event.preventDefault(); buttons[next].focus(); }
            } else if ((event.key === "Enter" || event.key === " ") && doc.activeElement === dialog && model.step.type !== "choice") {
                event.preventDefault(); continueButton.click();
            }
        }
        image.addEventListener("error", () => { portrait.hidden = true; root.classList.remove("has-portrait"); });
        root.addEventListener("click", click);
        ["pointerdown", "pointerup", "touchstart", "touchend", "wheel"].forEach(type => root.addEventListener(type, stop, { passive: true }));
        root.addEventListener("keydown", keys);
        return {
            render,
            destroy() {
                releaseFocus();
                root.removeEventListener("click", click); root.removeEventListener("keydown", keys);
                ["pointerdown", "pointerup", "touchstart", "touchend", "wheel"].forEach(type => root.removeEventListener(type, stop));
                root.replaceChildren(); root.classList.remove("sow-story", "is-modal"); root.hidden = true;
            }
        };
    }
    host.SOWCampaignView = { mount };
})(typeof globalThis !== "undefined" ? globalThis : this);
