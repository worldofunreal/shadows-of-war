// Owns the handoff between server settlement, reward presentation and menu guides.
// See docs/player-journeys.md. Guide definitions cannot release this gate.
(function (scope) {
    "use strict";

    function create() {
        var phase = "waiting";
        var key = null;
        var requestedScreen = null;
        var destinationDelivered = false;
        var destinationHeld = false;

        function update(state, presentation) {
            presentation = presentation || {};
            var context = state && state.menu_return;
            var nextKey = state && [state.account_id || "", context && context.cycle_id,
                context && context.source, context && context.match_id].join(":");
            if (key !== nextKey) {
                key = nextKey;
                requestedScreen = context && context.requested_screen || null;
                destinationDelivered = false;
                destinationHeld = false;
            }
            if (!destinationDelivered && context && context.requested_screen) requestedScreen = context.requested_screen;
            var confirmed = context
                ? context.account_id === state.account_id && context.cycle_id === state.loader_cycle_id &&
                    (context.status === "confirmed" || context.status === "not_applicable")
                : state && state.profile_ready === true;
            if (confirmed && context && context.status === "confirmed") {
                var expected = context.receipt_ids || [];
                confirmed = expected.length > 0 && expected.every(function (id) {
                    return (state.reward_receipts || []).some(function (receipt) {
                        return receipt.id === id && receipt.match_id === context.match_id &&
                            (receipt.status === "applied" || receipt.status === "presented");
                    });
                });
            }
            if (!state || state.phase !== "MainMenu" || !confirmed || !presentation.visible || !presentation.ready) {
                phase = "waiting";
            } else if (presentation.running || (presentation.receipts || []).length) {
                phase = "rewards";
            } else if (requestedScreen && !destinationDelivered) {
                phase = "destination";
            } else {
                if (destinationHeld && presentation.screen === "home") destinationHeld = false;
                phase = destinationHeld ? "destination" : "guide";
            }
            return phase;
        }

        return {
            update: update,
            canGuide: function () { return phase === "guide"; },
            canPresent: function () { return phase === "rewards"; },
            takeDestination: function () {
                if (phase !== "destination" || destinationDelivered) return null;
                destinationDelivered = true;
                destinationHeld = true;
                return requestedScreen;
            },
            phase: function () { return phase; }
        };
    }

    var api = { create: create };
    if (typeof module !== "undefined" && module.exports) module.exports = api;
    scope.SOWMenuFlow = api;
})(typeof window !== "undefined" ? window : globalThis);
