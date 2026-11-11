import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import type { HudMarket } from "./main_menu.state.ts";

const source = readFileSync(new URL("./main_menu.hud.ts", import.meta.url), "utf8");
const css = readFileSync(new URL("./main_menu.hud.css", import.meta.url), "utf8");
const start = source.indexOf("    let marketSide:");
const end = source.indexOf("    function updateHudSafeArea()", start);
const logic = new Bun.Transpiler({ loader: "ts" }).transformSync(source.slice(start, end));

function harness() {
    const hidden = new Set(["hidden"]);
    const hudRefs = {
        market: {
            classList: {
                contains: (name: string) => hidden.has(name),
                toggle: (name: string, hide: boolean) => hide ? hidden.add(name) : hidden.delete(name),
            },
            contains: () => false,
            querySelectorAll: () => [],
        },
        marketButton: { hidden: false, setAttribute() {} },
        marketAmount: { value: "200", min: "", step: "" },
        marketPrice: { value: "" },
        marketTrade: { disabled: false, textContent: "" },
        marketPost: { disabled: false, textContent: "" },
        marketCancel: { disabled: false },
        marketStatus: { textContent: "" },
        marketStock: { textContent: "" },
        marketOrder: { hidden: true },
        marketOrderText: { textContent: "" },
        marketLimitLabel: { textContent: "" },
    };
    const market: HudMarket = {
        side: "buy", lots: 2, troops_per_lot: 100, available_lots: 0,
        best_price: null, quote_gold: null, quote_failure: "insufficient_stock", order: null, feedback: null,
    };
    const hudState = { hud: { market_available: true, market } };
    const commands: { type: string; fields: Record<string, unknown> }[] = [];
    const actions = runInNewContext(logic + "\n({ renderMarket, sendMarketAction })", {
        hudRefs, hudState,
        document: { activeElement: null, getElementById: () => null },
        window: { SOW_LOCALE: "en" },
        restoreHudFocus() {},
        SOW_t: (key: string, values?: object) => key + (values ? JSON.stringify(values) : ""),
        send: (type: string, fields: Record<string, unknown>) => {
            commands.push({ type, fields });
            return true;
        },
    });
    return { hudRefs, hudState, market, commands, ...actions };
}

test("market renders empty stock and requires the visible quote before charging", () => {
    const { hudRefs, hudState, market, commands, renderMarket, sendMarketAction } = harness();
    renderMarket(hudState.hud);
    assert.equal(hudRefs.marketTrade.disabled, true);
    assert.match(hudRefs.marketStatus.textContent, /insufficient_stock/);
    sendMarketAction("market_trade");
    assert.equal(commands.length, 0);
    Object.assign(market, { available_lots: 4, quote_gold: 38, best_price: 18, quote_failure: null });
    renderMarket(hudState.hud);
    assert.match(hudRefs.marketTrade.textContent, /38/);
    assert.equal(hudRefs.marketTrade.disabled, false);
    hudRefs.marketAmount.value = "300";
    sendMarketAction("market_trade");
    assert.equal(commands.length, 0);
    hudRefs.marketAmount.value = "200";
    sendMarketAction("market_trade");
    assert.equal(commands.length, 1);
    assert.equal(commands[0].type, "market_trade");
    assert.equal(commands[0].fields.limit_gold, 38);
    assert.equal(hudRefs.marketTrade.disabled, true);
    market.quote_gold = 40;
    market.feedback = {
        request_id: Number(commands[0].fields.request_id),
        outcome: { kind: "rejected", reason: "price_changed" },
    };
    renderMarket(hudState.hud);
    assert.match(hudRefs.marketStatus.textContent, /price_changed/);
    assert.match(hudRefs.marketTrade.textContent, /40/);
    assert.equal(commands.length, 1);
});

test("market cancellation removes the open order after acknowledgement", () => {
    const { hudRefs, hudState, market, commands, renderMarket, sendMarketAction } = harness();
    market.order = { side: "sell", lots: 3, gold_per_lot: 22 };
    renderMarket(hudState.hud);
    assert.equal(hudRefs.marketOrder.hidden, false);
    sendMarketAction("cancel_market_order");
    assert.equal(hudRefs.marketCancel.disabled, true);
    market.order = null;
    market.feedback = { request_id: Number(commands[0].fields.request_id), outcome: { kind: "cancelled" } };
    renderMarket(hudState.hud);
    assert.equal(hudRefs.marketOrder.hidden, true);
    assert.match(hudRefs.marketStatus.textContent, /cancelled/);
});

test("market uses the shared compact transfer dialog and a single content column", () => {
    assert.match(source, /sow-hud__dialog sow-hud__transfer sow-hud__market hidden/);
    assert.match(css, /\[data-compact="true"\] \.sow-hud__transfer\s*\{/);
    assert.match(css, /\.sow-hud__market \.sow-hud__dialog-body\s*\{\s*grid-template-columns: minmax\(0, 1fr\)/);
});
