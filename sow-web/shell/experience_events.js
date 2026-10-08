/* One event contract for the owned web build and partner SDK adapters. */
(function () {
    "use strict";

    var EVENTS = new Set([
        "landing_visit", "shell_loaded", "play_now_click", "boot_start", "boot_route_decision", "load_stage",
        "menu_quick_match", "menu_join_attempt", "menu_password_join_attempt", "menu_code_join_attempt",
        "menu_custom_create", "menu_single_player_start", "menu_campaign_start", "menu_campaign_open",
        "menu_lobby_browser_open", "menu_custom_create_open", "menu_leader_confirm", "matchmaking_joined",
        "lobby_joined", "lobby_join_failed", "match_exit", "match_loading_start", "match_started_client",
        "match_ended_client", "tutorial_start", "tutorial_step", "tutorial_objective_complete",
        "tutorial_dialog_choice", "tutorial_exit_early", "campaign_episode_complete"
    ]);
    var POKI_EVENTS = {
        landing_visit: ["site", "landing", "visit"],
        shell_loaded: ["loading", "shell", "complete"],
        play_now_click: ["menu", "play_now", "interact"],
        boot_start: ["boot", "session", "start"],
        menu_quick_match: ["menu", "quick_match", "interact"],
        menu_join_attempt: ["menu", "join", "interact"],
        menu_password_join_attempt: ["menu", "password_join", "interact"],
        menu_code_join_attempt: ["menu", "code_join", "interact"],
        menu_custom_create: ["menu", "custom_create", "interact"],
        menu_single_player_start: ["menu", "single_player", "start"],
        menu_campaign_start: ["menu", "campaign", "start"],
        menu_campaign_open: ["menu", "campaign", "open"],
        menu_lobby_browser_open: ["menu", "lobby_browser", "open"],
        menu_custom_create_open: ["menu", "custom_create", "open"],
        menu_leader_confirm: ["menu", "leader", "confirm"],
        matchmaking_joined: ["matchmaking", "queue", "complete"],
        lobby_joined: ["lobby", "join", "complete"],
        lobby_join_failed: ["lobby", "join", "fail"],
        match_exit: ["match", "round", "exit"],
        match_loading_start: ["match", "loading", "start"],
        match_started_client: ["match", "round", "start"],
        match_ended_client: ["match", "round", "complete"],
        tutorial_objective_complete: ["tutorial", "objective", "complete"],
        tutorial_dialog_choice: ["tutorial", "dialog", "choice"],
        tutorial_exit_early: ["tutorial", "episode", "exit"]
    };
    var LOAD_STAGES = new Set([
        "relay_connect_start", "relay_connect_complete", "engine_init_complete",
        "gpu_upload_complete", "snapshot_available", "ready_sent"
    ]);

    function isId(value) {
        return typeof value === "string" && /^[a-z][a-z0-9_]{0,63}$/.test(value);
    }

    function isStepId(value) {
        return typeof value === "string" && /^[a-z][a-z0-9_-]{0,95}$/.test(value)
            && !["constructor", "prototype", "__proto__"].includes(value);
    }

    function exactKeys(value, keys) {
        if (!value || typeof value !== "object" || Array.isArray(value)) return false;
        var actual = Object.keys(value).sort();
        var expected = keys.slice().sort();
        return actual.length === expected.length && actual.every(function (key, index) {
            return key === expected[index];
        });
    }

    function safeProps(name, props) {
        if (name === "tutorial_step") {
            if (exactKeys(props, ["step_index"]) && Number.isInteger(props.step_index)
                && props.step_index >= 0 && props.step_index < 512) {
                return { step_index: props.step_index };
            }
            if (exactKeys(props, ["episode_id", "step_id", "step_index", "action"])
                && isId(props.episode_id) && isStepId(props.step_id)
                && Number.isInteger(props.step_index) && props.step_index >= 0 && props.step_index < 512
                && ["start", "complete"].includes(props.action)) {
                return {
                    episode_id: props.episode_id,
                    step_id: props.step_id,
                    step_index: props.step_index,
                    action: props.action
                };
            }
            return null;
        }
        if (name === "tutorial_exit_early") {
            if (exactKeys(props, ["episode_id"]) && isId(props.episode_id)) {
                return { episode_id: props.episode_id };
            }
            if (exactKeys(props, ["episode_id", "step_id", "step_index", "action"])
                && isId(props.episode_id) && isStepId(props.step_id)
                && Number.isInteger(props.step_index) && props.step_index >= 0 && props.step_index < 512
                && props.action === "fail") {
                return {
                    episode_id: props.episode_id,
                    step_id: props.step_id,
                    step_index: props.step_index,
                    action: props.action
                };
            }
            return null;
        }
        if (["tutorial_start", "campaign_episode_complete"].includes(name)
            && exactKeys(props, ["episode_id"]) && isId(props.episode_id)) {
            return { episode_id: props.episode_id };
        }
        if (name === "load_stage" && exactKeys(props, ["stage"]) && LOAD_STAGES.has(props.stage)) {
            return { stage: props.stage };
        }
        if (name === "boot_route_decision" && exactKeys(props, ["route"])
            && ["menu", "intro"].includes(props.route)) {
            return { route: props.route };
        }
        return null;
    }

    function parseEvent(payload) {
        var event;
        try { event = typeof payload === "string" ? JSON.parse(payload) : payload; } catch (_) { return null; }
        if (!event || typeof event.name !== "string" || !EVENTS.has(event.name)) return null;
        var props = event.props;
        if (event.name === "tutorial_exit_early" && exactKeys(props, ["episode_id"])
            && typeof window.SOW_tutorial_exit_context === "function") {
            var context = null;
            try { context = window.SOW_tutorial_exit_context(props.episode_id); } catch (_) {}
            if (context === false) return null;
            if (context && typeof context === "object" && !Array.isArray(context)) {
                props = Object.assign({}, props, context);
            }
        }
        return { name: event.name, props: safeProps(event.name, props) };
    }

    function pokiMeasure(event) {
        var mapped = POKI_EVENTS[event.name];
        if (event.name === "tutorial_exit_early" && event.props && event.props.step_id) {
            mapped = ["tutorial", event.props.episode_id + "_" + event.props.step_id, "fail"];
        } else if (["tutorial_start", "tutorial_exit_early", "campaign_episode_complete"].includes(event.name)) {
            if (event.props && event.props.episode_id) {
                var action = event.name === "tutorial_start" ? "start"
                    : event.name === "tutorial_exit_early" ? "exit" : "complete";
                mapped = ["tutorial", event.props.episode_id, action];
            }
        } else if (event.name === "tutorial_step") {
            var props = event.props || {};
            var step = props.step_id || (Number.isInteger(props.step_index) ? "step_" + props.step_index : "step_unknown");
            mapped = ["tutorial", (props.episode_id || "legacy") + "_" + step, props.action || "start"];
        } else if (event.name === "load_stage" && event.props) {
            var stage = event.props.stage;
            mapped = ["loading", stage.replace(/_(start|complete)$/, ""), stage.endsWith("_start") ? "start" : "complete"];
        } else if (event.name === "boot_route_decision" && event.props) {
            mapped = ["boot", "route", event.props.route];
        }
        if (mapped && typeof window.SOW_pokiMeasure === "function") {
            window.SOW_pokiMeasure(mapped[0], mapped[1], mapped[2]);
        }
    }

    window.SOW_trackExperienceEvent = function (payload) {
        var event = parseEvent(payload);
        if (!event || (typeof window.SOW_isAndroidTwa === "function" && window.SOW_isAndroidTwa())) return;
        if (window.SOW_PORTAL === "poki") {
            pokiMeasure(event);
            return;
        }
        if (window.SOW_PORTAL === "jest") {
            if (typeof window.SOW_portalCaptureExperienceEvent === "function") {
                window.SOW_portalCaptureExperienceEvent(event);
            }
            return;
        }
        // CrazyGames has lifecycle/loading plus whole-game progress, but no
        // arbitrary event funnel; SOW has no finite completion percentage to report.
        if (window.SOW_PORTAL === "crazygames") return;
        if (typeof window.SOW_analyticsTrack === "function") {
            window.SOW_analyticsTrack(event.name, event.props);
        }
    };
})();
