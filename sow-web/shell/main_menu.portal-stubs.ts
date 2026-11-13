// Stand-ins for the store and sign-in views. Poki and Jest builds leave those
// modules out, so the portal surfaces re-export these names from here and the
// shared shell still finds them. Each one does nothing where it cannot be shown.
// Heroes stay unlockable with earned crowns and gems: that spends no money.

import { currencyAsset, esc, menuUi } from "./main_menu.core.ts";
import type { MenuLeader } from "./main_menu.state.ts";

export function abandonExternalPurchaseAttempt() {}
export function beginStorePurchase() {}
export function beginStoreRestore() {}
export function canDismissPurchaseModal() {
    return true;
}
export function completeExternalPurchase() {}
export function currentExternalPurchaseAttempt() {
    return null;
}
export function dismissPurchaseModal() {}
export function isAndroidTwa() {
    return false;
}
export function loadStoreCatalog() {}
export function openLeaderPurchase() {}
export function openOfferPurchase() {}
export function openSkinPurchase() {}
function portalUnlockButton(leader: MenuLeader, currency: "crowns" | "gems") {
    const cost = Math.max(0, Number(currency === "gems" ? leader.cost_gems : leader.cost_crowns) || 0);
    if (!cost) return "";
    const balance = Math.max(0, Number(menuUi.state && menuUi.state[currency]) || 0);
    const insufficient = balance < cost;
    const confirming = menuUi.portalUnlockConfirm === leader.id + ":" + currency;
    const disabled = insufficient || (menuUi.state && menuUi.state.store_busy) ? " disabled" : "";
    return (
        "<button class='sow-store__buy" +
        (insufficient ? " sow-store__buy--insufficient" : "") +
        "' type='button' data-command='portal_unlock_leader' data-currency='" +
        currency +
        "' data-leader-id='" +
        esc(leader.id) +
        "'" +
        disabled +
        ">" +
        esc(SOW_t(confirming ? "store.confirm_unlock" : "store.unlock")) +
        " <span" +
        (insufficient ? " class='sow-store__currency-amount--insufficient'" : "") +
        ">" +
        esc(cost) +
        "</span> <img class='sow-store__currency-icon' src='" +
        esc(currencyAsset(currency === "gems" ? "gem" : "crown")) +
        "' alt='' aria-hidden='true'></button>"
    );
}

export function renderLeaderPurchase(leader?: MenuLeader | null) {
    if (!leader || leader.owned || leader.permanently_free) return "";
    const actions = portalUnlockButton(leader, "crowns") + portalUnlockButton(leader, "gems");
    return actions ? "<div class='sow-heroes__purchase-actions'>" + actions + "</div>" : "";
}
export function renderSkinsButton() {
    return "";
}
export function renderPurchaseModal() {
    return "";
}
export function renderSkinPickerModal() {
    return "";
}
export function renderStore() {
    return "";
}
export function resolvePurchaseModal() {}
export function resumeExternalPurchaseDelivery() {}
export function renderAuthModal() {
    return "";
}
export function requestAuthOtp() {}
export function verifyAuthOtp() {}
export function resetAuthFlow() {}
