/* One event contract for the owned web build and partner SDK adapters. */
(function () {
    "use strict";

    type EventProps = Record<string, unknown>;

    const EVENTS = new Set([
        "landing_visit",
        "shell_loaded",
        "play_now_click",
        "boot_start",
        "boot_route_decision",
        "load_stage",
        "menu_quick_match",
        "menu_join_attempt",
        "menu_password_join_attempt",
        "menu_code_join_attempt",
        "menu_custom_create",
        "menu_single_player_start",
        "menu_campaign_start",
        "menu_campaign_open",
        "menu_lobby_browser_open",
        "menu_custom_create_open",
        "menu_leader_confirm",
        "matchmaking_joined",
        "lobby_joined",
        "lobby_join_failed",
        "match_exit",
        "match_loading_start",
        "match_started_client",
        "match_ended_client",
        "tutorial_start",
        "tutorial_step",
        "tutorial_objective_complete",
        "tutorial_dialog_choice",
        "tutorial_exit_early",
        "campaign_episode_complete",
        "store_open",
        "store_buy_tap",
        "store_checkout_open",
        "store_purchase_complete",
    ]);
    const STORE_ITEM_EVENTS = ["store_buy_tap", "store_checkout_open", "store_purchase_complete"];
    const POKI_EVENTS: Record<string, string[]> = {
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
        tutorial_exit_early: ["tutorial", "episode", "exit"],
    };
    const LOAD_STAGES = new Set<unknown>([
        "relay_connect_start",
        "relay_connect_complete",
        "engine_init_complete",
        "gpu_upload_complete",
        "snapshot_available",
        "ready_sent",
    ]);

    function isId(value: unknown): value is string {
        return typeof value === "string" && /^[a-z][a-z0-9_]{0,63}$/.test(value);
    }

    function isStepId(value: unknown): value is string {
        return (
            typeof value === "string" &&
            /^[a-z][a-z0-9_-]{0,95}$/.test(value) &&
            !["constructor", "prototype", "__proto__"].includes(value)
        );
    }

    function isIndex(value: unknown): value is number {
        return typeof value === "number" && Number.isInteger(value) && value >= 0 && value < 512;
    }

    function isOneOf(value: unknown, options: readonly string[]): value is string {
        return typeof value === "string" && options.includes(value);
    }

    function exactKeys(value: unknown, keys: string[]): boolean {
        if (!value || typeof value !== "object" || Array.isArray(value)) return false;
        const actual = Object.keys(value).sort();
        const expected = keys.slice().sort();
        return (
            actual.length === expected.length &&
            actual.every(function (key, index) {
                return key === expected[index];
            })
        );
    }

    function safeProps(name: string, props: EventProps): EventProps | null {
        if (name === "tutorial_step") {
            if (exactKeys(props, ["step_index"]) && isIndex(props.step_index)) {
                return { step_index: props.step_index };
            }
            if (
                exactKeys(props, ["episode_id", "step_id", "step_index", "action"]) &&
                isId(props.episode_id) &&
                isStepId(props.step_id) &&
                isIndex(props.step_index) &&
                isOneOf(props.action, ["start", "complete"])
            ) {
                return {
                    episode_id: props.episode_id,
                    step_id: props.step_id,
                    step_index: props.step_index,
                    action: props.action,
                };
            }
            return null;
        }
        if (name === "tutorial_exit_early") {
            if (exactKeys(props, ["episode_id"]) && isId(props.episode_id)) {
                return { episode_id: props.episode_id };
            }
            if (
                exactKeys(props, ["episode_id", "step_id", "step_index", "action"]) &&
                isId(props.episode_id) &&
                isStepId(props.step_id) &&
                isIndex(props.step_index) &&
                props.action === "fail"
            ) {
                return {
                    episode_id: props.episode_id,
                    step_id: props.step_id,
                    step_index: props.step_index,
                    action: props.action,
                };
            }
            return null;
        }
        if (
            ["tutorial_start", "campaign_episode_complete"].includes(name) &&
            exactKeys(props, ["episode_id"]) &&
            isId(props.episode_id)
        ) {
            return { episode_id: props.episode_id };
        }
        if (
            STORE_ITEM_EVENTS.includes(name) &&
            exactKeys(props, ["kind", "item_id", "currency"]) &&
            isOneOf(props.kind, ["gem_pack", "leader", "skin"]) &&
            isId(props.item_id) &&
            isOneOf(props.currency, ["usd", "gems", "crowns"])
        ) {
            return { kind: props.kind, item_id: props.item_id, currency: props.currency };
        }
        if (name === "load_stage" && exactKeys(props, ["stage"]) && LOAD_STAGES.has(props.stage)) {
            return { stage: props.stage };
        }
        if (
            name === "boot_route_decision" &&
            exactKeys(props, ["route"]) &&
            isOneOf(props.route, ["menu", "intro"])
        ) {
            return { route: props.route };
        }
        return null;
    }

    function parseEvent(payload: unknown) {
        let event;
        try {
            event = typeof payload === "string" ? JSON.parse(payload) : payload;
        } catch {
            return null;
        }
        if (!event || typeof event.name !== "string" || !EVENTS.has(event.name)) return null;
        let props = event.props;
        if (
            event.name === "tutorial_exit_early" &&
            exactKeys(props, ["episode_id"]) &&
            typeof window.SOW_tutorial_exit_context === "function"
        ) {
            let context = null;
            try {
                context = window.SOW_tutorial_exit_context(props.episode_id);
            } catch {}
            if (context === false) return null;
            if (context && typeof context === "object" && !Array.isArray(context)) {
                props = Object.assign({}, props, context);
            }
        }
        return { name: event.name, props: safeProps(event.name, props) };
    }

    function pokiMeasure(event: { name: string; props: EventProps | null }) {
        let mapped: string[] | undefined = POKI_EVENTS[event.name];
        if (event.name === "tutorial_exit_early" && event.props && event.props.step_id) {
            mapped = ["tutorial", event.props.episode_id + "_" + event.props.step_id, "fail"];
        } else if (
            ["tutorial_start", "tutorial_exit_early", "campaign_episode_complete"].includes(event.name)
        ) {
            if (event.props && event.props.episode_id) {
                const action =
                    event.name === "tutorial_start"
                        ? "start"
                        : event.name === "tutorial_exit_early"
                          ? "exit"
                          : "complete";
                mapped = ["tutorial", String(event.props.episode_id), action];
            }
        } else if (event.name === "tutorial_step") {
            const props = event.props || {};
            const step =
                props.step_id ||
                (Number.isInteger(props.step_index) ? "step_" + props.step_index : "step_unknown");
            mapped = [
                "tutorial",
                (props.episode_id || "legacy") + "_" + step,
                String(props.action || "start"),
            ];
        } else if (event.name === "load_stage" && event.props) {
            const stage = String(event.props.stage);
            mapped = [
                "loading",
                stage.replace(/_(start|complete)$/, ""),
                stage.endsWith("_start") ? "start" : "complete",
            ];
        } else if (event.name === "boot_route_decision" && event.props) {
            mapped = ["boot", "route", String(event.props.route)];
        }
        if (mapped && typeof window.SOW_pokiMeasure === "function") {
            window.SOW_pokiMeasure(mapped[0], mapped[1], mapped[2]);
        }
    }

    window.SOW_trackExperienceEvent = function (payload) {
        const event = parseEvent(payload);
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
