(function () {
    "use strict";

    var hudRoot = document.getElementById("sow-hud");
    var runtime = {
        episodeId: null,
        definition: null,
        roster: null,
        active: false,
        stepIndex: 0,
        baselineTiles: null,
        dialogOpen: true,
        paused: null,
        finalReady: false,
        completionSent: false,
        enteredSteps: Object.create(null),
        completedSteps: Object.create(null),
        flags: Object.create(null),
        loading: Object.create(null),
        startingEpisode: null,
        bootEpisode: null,
        generation: 0,
        lastPhase: null
    };

    var allowedTriggers = {
        territory: true,
        kills: true,
        defeated: true,
        contact: true,
        attack: true,
        troops: true,
        building: true,
        fleet: true,
        nuke: true,
        elapsed: true
    };
    var allowedActions = {
        show_dialog: true,
        set_objective: true,
        emote: true,
        pause: true,
        resume: true,
        set_flag: true
    };

    function esc(value) {
        return String(value == null ? "" : value)
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#39;");
    }

    function asset(path) {
        var base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
        return base + "/" + path.split("/").map(encodeURIComponent).join("/");
    }

    function leaderSlug(value) {
        return String(value || "boudica")
            .replace(/([a-z])([A-Z])/g, "$1_$2")
            .replace(/\s+/g, "_")
            .toLowerCase();
    }

    function send(type, extra) {
        if (typeof window.SOW_menu_command !== "function") return false;
        window.SOW_menu_command(JSON.stringify(Object.assign({ type: type }, extra || {})));
        return true;
    }

    function translation(key) {
        var value = typeof window.SOW_t === "function" ? window.SOW_t(key) : "[" + key + "]";
        if (!value || value === "[" + key + "]") throw new Error("missing translation " + key);
        return value;
    }

    function ensureI18n() {
        return Promise.resolve(window.SOW_I18N_READY).catch(function () {});
    }

    function fetchJson(path) {
        return fetch(asset(path), { cache: "no-store", credentials: "same-origin" }).then(function (response) {
            if (!response.ok) throw new Error(path + " returned " + response.status);
            return response.json();
        });
    }

    function validateEpisode(episodeId, roster, definition) {
        if (!roster || typeof roster !== "object" || typeof roster.map !== "string" || !Array.isArray(roster.factions)) {
            throw new Error("roster shape");
        }
        if (!Array.isArray(roster.player_spawn) || roster.player_spawn.length !== 2) {
            throw new Error("player spawn");
        }
        var names = Object.create(null);
        roster.factions.forEach(function (faction) {
            if (!faction || typeof faction.name !== "string" || !faction.name.trim()) throw new Error("faction name");
            if (names[faction.name]) throw new Error("duplicate faction " + faction.name);
            names[faction.name] = true;
        });
        if (!definition || definition.version !== 1 || definition.episode_id !== episodeId || !Array.isArray(definition.steps) || !definition.steps.length) {
            throw new Error("trigger shape");
        }
        var ids = Object.create(null);
        definition.steps.forEach(function (step) {
            if (!step || typeof step.id !== "string" || !step.id || ids[step.id]) throw new Error("duplicate step id");
            ids[step.id] = true;
            if (!step.trigger || !allowedTriggers[step.trigger.type]) throw new Error("unknown trigger");
            [step.title_key, step.body_key, step.hint_key].forEach(function (key) {
                if (typeof key !== "string") throw new Error("missing translation key");
                translation(key);
            });
            var trigger = step.trigger;
            if ((trigger.type === "defeated" || trigger.type === "contact") && !names[trigger.target]) throw new Error("unknown faction target " + trigger.target);
            if (["territory", "kills", "contact", "attack", "troops", "building", "fleet", "nuke", "elapsed"].indexOf(trigger.type) >= 0 && (!Number.isFinite(Number(trigger.value)) || Number(trigger.value) < 0)) {
                throw new Error("invalid trigger value");
            }
            [step.on_enter || [], step.on_complete || []].forEach(function (actions) {
                if (!Array.isArray(actions)) throw new Error("invalid actions");
                actions.forEach(function (action) {
                    if (!action || !allowedActions[action.type]) throw new Error("unknown action");
                });
            });
            if (step.marker && step.marker.target !== "player" && !names[step.marker.target]) throw new Error("unknown marker target " + step.marker.target);
        });
        var settings = definition.settings || {};
        if (typeof settings.buildings_enabled !== "boolean" || !Number.isFinite(Number(settings.starting_troops))) throw new Error("invalid episode settings");
    }

    function episodeFiles(episodeId) {
        return {
            roster: "campaign/" + episodeId + ".json",
            triggers: "campaign/" + episodeId + ".triggers.json"
        };
    }

    function loadEpisode(episodeId) {
        if (runtime.loading[episodeId]) return runtime.loading[episodeId];
        var files = episodeFiles(episodeId);
        runtime.loading[episodeId] = Promise.all([fetchJson(files.roster), fetchJson(files.triggers)])
            .then(function (payload) {
                return ensureI18n().then(function () {
                    validateEpisode(episodeId, payload[0], payload[1]);
                    return { roster: payload[0], definition: payload[1] };
                });
            })
            .catch(function (error) {
                delete runtime.loading[episodeId];
                throw error;
            });
        return runtime.loading[episodeId];
    }

    function removeError() {
        var node = document.getElementById("sow-tutorial-error");
        if (node) node.remove();
    }

    function resetRuntime() {
        runtime.generation += 1;
        runtime.episodeId = null;
        runtime.definition = null;
        runtime.roster = null;
        runtime.active = false;
        runtime.stepIndex = 0;
        runtime.baselineTiles = null;
        runtime.dialogOpen = true;
        runtime.paused = null;
        runtime.finalReady = false;
        runtime.completionSent = false;
        runtime.enteredSteps = Object.create(null);
        runtime.completedSteps = Object.create(null);
        runtime.flags = Object.create(null);
        runtime.startingEpisode = null;
        runtime.bootEpisode = null;
        removeError();
        if (hudRoot) {
            var stale = hudRoot.querySelector("[data-tutorial-overlay]");
            if (stale) stale.remove();
        }
    }

    function showError(error, episodeId) {
        console.error("[SOW TUTORIAL]", error);
        removeError();
        var node = document.createElement("div");
        node.id = "sow-tutorial-error";
        node.className = "sow-tutorial__error";
        node.setAttribute("role", "alert");
        node.innerHTML = "<span>" + esc(translation("tutorial.unavailable")) + "</span><button type='button' data-tutorial-retry>" + esc(translation("tutorial.retry")) + "</button>";
        document.body.appendChild(node);
        node.querySelector("[data-tutorial-retry]").addEventListener("click", function () {
            removeError();
            if (episodeId) startEpisode(episodeId, false);
        });
    }

    function startEpisode(episodeId, fromBoot) {
        if (!episodeId || runtime.startingEpisode === episodeId || (runtime.active && runtime.episodeId === episodeId)) return;
        var generation = runtime.generation;
        runtime.startingEpisode = episodeId;
        runtime.bootEpisode = fromBoot ? episodeId : runtime.bootEpisode;
        removeError();
        loadEpisode(episodeId).then(function (data) {
            if (generation !== runtime.generation) return;
            runtime.episodeId = episodeId;
            runtime.roster = data.roster;
            runtime.definition = data.definition;
            runtime.stepIndex = 0;
            runtime.baselineTiles = null;
            runtime.dialogOpen = true;
            runtime.paused = null;
            runtime.finalReady = false;
            runtime.completionSent = false;
            runtime.enteredSteps = Object.create(null);
            runtime.completedSteps = Object.create(null);
            runtime.flags = Object.create(null);
            if (!send("start_campaign_episode", {
                episode_id: episodeId,
                roster: data.roster,
                match: data.definition.settings
            })) {
                throw new Error("menu bridge unavailable");
            }
        }).catch(function (error) {
            if (generation !== runtime.generation) return;
            showError(error, episodeId);
        }).then(function () {
            if (generation === runtime.generation) runtime.startingEpisode = null;
        });
    }

    function tutorialFacts(hud) {
        var tutorial = hud.tutorial || {};
        var facts = tutorial.facts || {};
        if (runtime.baselineTiles == null && Number.isFinite(Number(facts.tiles)) && Number(facts.tiles) > 0) {
            runtime.baselineTiles = Number(facts.tiles);
        }
        facts.tiles_gained = Math.max(0, Number(facts.tiles || 0) - Number(runtime.baselineTiles || 0));
        return facts;
    }

    function triggerProgress(trigger, facts) {
        var type = trigger.type;
        if (type === "defeated") return { current: facts.defeated_names && facts.defeated_names.indexOf(trigger.target) >= 0 ? 1 : 0, target: 1 };
        var current = type === "territory" && trigger.mode === "gained" ? facts.tiles_gained : facts[type + "s"];
        if (type === "kills") current = facts.kills;
        if (type === "troops") current = facts.troops;
        if (type === "elapsed") current = facts.elapsed_ticks;
        current = Number(current || 0);
        return { current: Math.min(current, Number(trigger.value)), target: Number(trigger.value) };
    }

    function runActions(actions) {
        (actions || []).forEach(function (action) {
            if (!action || typeof action.type !== "string") return;
            if (action.type === "pause") setPaused(true);
            else if (action.type === "resume") setPaused(false);
            else if (action.type === "show_dialog") { runtime.dialogOpen = true; setPaused(true); }
            else if (action.type === "set_flag" && action.name) runtime.flags[action.name] = true;
            else if (action.type === "emote" && action.emoji) send("express_emoji", { emoji: action.emoji, pinned: false });
        });
    }

    function enterStep(step) {
        if (!step || runtime.enteredSteps[step.id]) return;
        runtime.enteredSteps[step.id] = true;
        runActions(step.on_enter);
    }

    function completeStep(step) {
        if (!step || runtime.completedSteps[step.id]) return;
        runtime.completedSteps[step.id] = true;
        runActions(step.on_complete);
    }

    function currentStep(hud) {
        if (!runtime.definition) return null;
        var facts = tutorialFacts(hud);
        var steps = runtime.definition.steps;
        while (runtime.stepIndex < steps.length - 1) {
            var progress = triggerProgress(steps[runtime.stepIndex].trigger, facts);
            if (progress.current < progress.target) break;
            completeStep(steps[runtime.stepIndex]);
            runtime.stepIndex += 1;
            runtime.dialogOpen = true;
            runtime.finalReady = false;
            setPaused(true);
        }
        var step = steps[runtime.stepIndex];
        enterStep(step);
        var progress = triggerProgress(step.trigger, facts);
        runtime.finalReady = runtime.stepIndex === steps.length - 1 && progress.current >= progress.target;
        return { step: step, progress: progress, facts: facts };
    }

    function setPaused(paused) {
        if (!runtime.active || runtime.paused === paused) return;
        runtime.paused = paused;
        send("set_tutorial_paused", { paused: paused });
    }

    function render(hud) {
        if (!hudRoot) return;
        var tutorial = hud && hud.tutorial;
        if (!tutorial || !tutorial.active || !runtime.definition || runtime.episodeId !== tutorial.episode_id) {
            var stale = hudRoot.querySelector("[data-tutorial-overlay]");
            if (stale) stale.remove();
            runtime.active = false;
            runtime.paused = null;
            return;
        }
        runtime.active = true;
        var info = currentStep(hud);
        if (!info) return;
        var step = info.step;
        var progress = info.progress;
        var overlay = hudRoot.querySelector("[data-tutorial-overlay]");
        if (!overlay) {
            overlay = document.createElement("section");
            overlay.className = "sow-hud__tutorial-overlay";
            overlay.dataset.tutorialOverlay = "true";
            overlay.innerHTML = "<article class='sow-hud__tutorial-card' data-tutorial-card role='dialog' aria-modal='true'><div class='sow-hud__tutorial-copy'><p class='sow-hud__tutorial-step' data-tutorial-step></p><h2 data-tutorial-title></h2><p data-tutorial-body></p><strong data-tutorial-hint></strong><div class='sow-hud__tutorial-progress' data-tutorial-progress></div><button type='button' class='sow-hud__tutorial-action' data-tutorial-continue></button></div><img class='sow-hud__tutorial-avatar' data-tutorial-avatar alt=''></article>";
            hudRoot.appendChild(overlay);
            overlay.addEventListener("click", function (event) {
                event.stopPropagation();
                var button = event.target && event.target.closest
                    ? event.target.closest("[data-tutorial-continue]")
                    : null;
                if (!button) return;
                event.preventDefault();
                if (runtime.finalReady) {
                    if (runtime.completionSent) return;
                    completeStep(runtime.definition.steps[runtime.stepIndex]);
                    runtime.completionSent = true;
                    send("complete_campaign_episode", { episode_id: runtime.episodeId });
                    return;
                }
                runtime.dialogOpen = false;
                setPaused(false);
                render(hud);
            }, true);
            overlay.addEventListener("pointerdown", function (event) {
                if (runtime.dialogOpen) event.stopPropagation();
            }, true);
            overlay.addEventListener("pointerup", function (event) {
                if (runtime.dialogOpen) event.stopPropagation();
            }, true);
        }
        overlay.classList.toggle("is-open", runtime.dialogOpen);
        var card = overlay.querySelector("[data-tutorial-card]");
        card.hidden = !runtime.dialogOpen;
        overlay.querySelector("[data-tutorial-step]").textContent = (runtime.stepIndex + 1) + " / " + runtime.definition.steps.length;
        overlay.querySelector("[data-tutorial-title]").textContent = translation(step.title_key);
        overlay.querySelector("[data-tutorial-body]").textContent = translation(step.body_key);
        overlay.querySelector("[data-tutorial-hint]").textContent = translation(step.hint_key);
        overlay.querySelector("[data-tutorial-progress]").textContent = Math.floor(progress.current) + " / " + Math.floor(progress.target);
        overlay.querySelector("[data-tutorial-continue]").textContent = translation(runtime.finalReady ? "tutorial.complete" : "tutorial.continue");
        var leader = leaderSlug(hud.player_leader || "boudica");
        var avatar = overlay.querySelector("[data-tutorial-avatar]");
        avatar.src = asset("gameplay/avatars/" + leader + ".webp");
        avatar.alt = leader;
        if (runtime.paused == null) setPaused(true);
    }

    window.SOW_startCampaignEpisode = function (episodeId) {
        startEpisode(String(episodeId || ""), false);
    };

    window.SOW_tutorial_menu_state_update = function (state) {
        var phase = state && state.phase;
        var enteringMenu = phase === "MainMenu" && runtime.lastPhase !== "MainMenu";
        runtime.lastPhase = phase;
        if (enteringMenu && (!state || !state.boot_campaign)) resetRuntime();
        if (!state || !state.boot_campaign || runtime.bootEpisode === state.boot_campaign) return;
        startEpisode(String(state.boot_campaign), true);
    };

    window.SOW_tutorial_state_update = function (state) {
        var hud = state && state.phase === "Playing" ? state.hud : null;
        if (!hud || !hud.tutorial || !hud.tutorial.active) {
            render(null);
            return;
        }
        if (runtime.episodeId !== hud.tutorial.episode_id) {
            var generation = runtime.generation;
            loadEpisode(hud.tutorial.episode_id).then(function (data) {
                if (generation !== runtime.generation) return;
                runtime.episodeId = hud.tutorial.episode_id;
                runtime.roster = data.roster;
                runtime.definition = data.definition;
                runtime.stepIndex = 0;
                runtime.baselineTiles = null;
                runtime.finalReady = false;
                runtime.completionSent = false;
                runtime.enteredSteps = Object.create(null);
                runtime.completedSteps = Object.create(null);
                runtime.flags = Object.create(null);
                runtime.dialogOpen = true;
                render(hud);
            }).catch(function (error) {
                if (generation !== runtime.generation) return;
                showError(error, hud.tutorial.episode_id);
            });
            return;
        }
        render(hud);
    };
})();
