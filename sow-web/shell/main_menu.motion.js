    var sowScreenMotion = (function () {
        function reducedMotion() {
            return typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
        }

        function setInactive(panel) {
            panel.classList.remove("is-active", "sow-screen-panel--entering");
            panel.classList.add("sow-screen-panel--leaving");
            panel.setAttribute("aria-hidden", "true");
            if ("inert" in panel) panel.inert = true;
        }

        function setActive(panel) {
            panel.classList.add("is-active");
            panel.classList.remove("sow-screen-panel--leaving");
            panel.removeAttribute("aria-hidden");
            if ("inert" in panel) panel.inert = false;
        }

        function finish(event) {
            var panel = event.target;
            if (!panel || panel.parentNode !== event.currentTarget || !panel.matches("[data-screen-panel]")) return;
            if (event.animationName === "sow-screen-panel-leave") panel.remove();
            if (event.animationName === "sow-screen-panel-enter") panel.classList.remove("sow-screen-panel--entering");
        }

        function bind(stage) {
            if (stage.dataset.motionBound) return;
            stage.dataset.motionBound = "true";
            stage.addEventListener("animationend", finish);
            stage.addEventListener("animationcancel", finish);
        }

        function show(stage, next, disableMotion) {
            bind(stage);
            var current = stage.querySelector("[data-screen-panel].is-active") || stage.lastElementChild;
            var panels = stage.querySelectorAll("[data-screen-panel]");
            for (var i = 0; i < panels.length; i++) {
                if (panels[i] !== current) panels[i].remove();
            }
            if (current) setInactive(current);
            setActive(next);
            if (disableMotion || reducedMotion()) {
                if (current) current.remove();
                stage.appendChild(next);
                return;
            }
            next.classList.add("sow-screen-panel--entering");
            stage.appendChild(next);
        }

        return { show: show };
    })();
