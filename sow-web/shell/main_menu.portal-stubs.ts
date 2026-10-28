// Stand-ins for the store and sign-in views. Poki and Jest builds leave those
// modules out, so the portal surfaces re-export these names from here and the
// shared shell still finds them. Each one does nothing where it cannot be shown.

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
export function openSkinPurchase() {}
export function renderLeaderPurchase() {
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
