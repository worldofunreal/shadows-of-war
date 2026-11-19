import {
    asset,
    avatarImage,
    currencyAsset,
    esc,
    leaderArtUrl,
    leaderCivilization,
    leaderArtSlug,
    leaderById,
    leaderDisplayName,
    localizedText,
    menuUi,
    root,
    selfCreds,
    send,
    stageLeaderSelection,
} from "./main_menu.core.ts";
import type { PurchaseIntent } from "./main_menu.core.ts";
import { profileApi } from "./main_menu.profile.ts";
import {
    animateProgressionTo,
    flyRewardCard,
    progressionFromState,
    pulseRewardCounter,
    reducedRewardMotion,
    render,
    rewardAnimationToken,
    writeProgression,
} from "./main_menu.shell.ts";
import type {
    MenuGemBundle,
    MenuLeader,
    MenuLeaderOffer,
    MenuSkinOffer,
    MenuStore,
    MenuStoreBundleOffer,
    MenuStoreOffer,
} from "./main_menu.state.ts";
import { renderFeedback } from "./main_menu.surfaces.ts";

// Store state, rendering, catalog, and purchase actions.
let loadedRotationPeriod = -1;

// Store funnel: store_open -> store_buy_tap -> store_checkout_open (real
// money only) -> store_purchase_complete. Taxonomy lives in
// experience_events.js and sow-data/src/events.rs.
function trackStoreEvent(name: string, kind: string, itemId: string | undefined, currency: string) {
    if (typeof window.SOW_trackExperienceEvent !== "function") return;
    window.SOW_trackExperienceEvent({
        name: name,
        props: { kind: kind, item_id: String(itemId || ""), currency: currency },
    });
}

function storeAuth() {
    const creds = selfCreds();
    const headers: Record<string, string> = {
        "Content-Type": "application/json",
        Accept: "application/json",
    };
    let token = null;
    const identity = window.SOW_PLATFORM_IDENTITY;
    if (identity && identity.token) token = identity.token;
    if (!token) {
        try {
            token = window.localStorage.getItem("sow_session_token");
        } catch {}
    }
    if (token) {
        headers["X-Platform-Auth"] = token;
        headers["X-Platform-Provider"] = identity && identity.provider ? identity.provider : "sow";
    }
    return { available: !!creds || !!token, headers: headers, authSecret: creds ? creds.auth_secret : "" };
}

export function isAndroidTwa() {
    const referrer = String(document.referrer || "");
    if (/^android-app:\/\/com\.shadowsofwar(?:\/|$)/i.test(referrer)) return true;
    try {
        return (
            new URLSearchParams(window.location.search).get("sow_platform") === "android" &&
            /Android/i.test(navigator.userAgent || "")
        );
    } catch {
        return false;
    }
}

function renderCurrencyAmount(amount: number, kind: "gem" | "crown", amountClass?: string) {
    const className = amountClass ? " class='" + esc(amountClass) + "'" : "";
    return (
        "<span" +
        className +
        ">" +
        esc(amount) +
        "</span> <img class='sow-store__currency-icon' src='" +
        esc(currencyAsset(kind)) +
        "' alt='' aria-hidden='true'>"
    );
}

function storeBalance(currency: "gems" | "crowns") {
    return Math.max(0, Number(menuUi.state && menuUi.state[currency]) || 0);
}

// A real-money purchase waiting for delivery, saved per account in localStorage.
interface ExternalPurchaseAttempt {
    account_id: string;
    product_id: string;
    kind: "bundle" | "leader" | "skin";
    item_id: string;
    gems: number;
    base_gems: number;
    started_at: number;
    deadline_at: number;
    provider_complete: boolean;
    delivery_id: string;
    timed_out: boolean;
    dismissed: boolean;
}

const EXTERNAL_PURCHASE_POLL_MS = 3000;
const EXTERNAL_PURCHASE_WAIT_MS = 120000;
let externalPurchaseAttempt: ExternalPurchaseAttempt | null = null;
let externalPurchasePollTimer: number | null = null;
let externalPurchasePollInFlight = false;
let externalPurchaseRevealTimer: number | null = null;
let purchaseSuccessTimer: number | null = null;
let bundleFlightToken: HTMLElement | null = null;

function storeBundleByProductId(productId: string | undefined) {
    return (
        ((menuUi.state && menuUi.state.store && menuUi.state.store.gem_bundles) || []).find(
            function (bundle) {
                return String(bundle.product_id) === String(productId);
            },
        ) || null
    );
}

function externalPurchaseStorageKey(accountId: string) {
    return "sow_pending_store_purchase_v1:" + accountId;
}

function clearExternalPurchasePoll() {
    if (externalPurchasePollTimer !== null) window.clearTimeout(externalPurchasePollTimer);
    externalPurchasePollTimer = null;
    externalPurchasePollInFlight = false;
}

function saveExternalPurchaseAttempt() {
    if (!externalPurchaseAttempt) return;
    try {
        window.localStorage.setItem(
            externalPurchaseStorageKey(externalPurchaseAttempt.account_id),
            JSON.stringify(externalPurchaseAttempt),
        );
    } catch {}
}

export function currentExternalPurchaseAttempt() {
    if (!menuUi.state || !menuUi.state.account_id) return null;
    if (externalPurchaseAttempt && externalPurchaseAttempt.account_id === menuUi.state.account_id)
        return externalPurchaseAttempt;
    clearExternalPurchasePoll();
    externalPurchaseAttempt = null;
    try {
        const saved = JSON.parse(
            window.localStorage.getItem(externalPurchaseStorageKey(menuUi.state.account_id)) || "null",
        );
        if (
            saved &&
            saved.account_id === menuUi.state.account_id &&
            saved.product_id &&
            (saved.kind === "bundle" || saved.kind === "leader" || saved.kind === "skin")
        ) {
            externalPurchaseAttempt = saved;
        }
    } catch {}
    return externalPurchaseAttempt;
}

function clearExternalPurchaseAttempt() {
    const attempt = currentExternalPurchaseAttempt();
    clearExternalPurchasePoll();
    if (attempt) {
        try {
            window.localStorage.removeItem(externalPurchaseStorageKey(attempt.account_id));
        } catch {}
    }
    externalPurchaseAttempt = null;
}

function externalPurchaseDetails(
    productId: string,
): { kind: ExternalPurchaseAttempt["kind"]; itemId: string; gems: number } | null {
    const bundle = storeBundleByProductId(productId);
    if (bundle) return { kind: "bundle", itemId: bundle.id, gems: Number(bundle.gems) || 0 };
    const intent: Partial<PurchaseIntent> = menuUi.purchaseIntent || {};
    if (intent.productId !== productId) return null;
    if (intent.leaderId) return { kind: "leader", itemId: intent.leaderId, gems: 0 };
    if (intent.skinId) return { kind: "skin", itemId: intent.skinId, gems: 0 };
    return null;
}

function beginExternalPurchaseAttempt(productId: string) {
    if (!menuUi.state || !menuUi.state.account_id) return null;
    const details = externalPurchaseDetails(productId);
    if (!details) return null;
    externalPurchaseAttempt = {
        account_id: menuUi.state.account_id,
        product_id: productId,
        kind: details.kind,
        item_id: details.itemId,
        gems: details.gems,
        base_gems: Math.max(0, Number(menuUi.state.gems) || 0),
        started_at: Date.now(),
        deadline_at: Date.now() + EXTERNAL_PURCHASE_WAIT_MS,
        provider_complete: false,
        delivery_id: "",
        timed_out: false,
        dismissed: false,
    };
    saveExternalPurchaseAttempt();
    return externalPurchaseAttempt;
}

function requestExternalPurchaseProfile(attempt: ExternalPurchaseAttempt) {
    if (!attempt || !menuUi.state || menuUi.state.account_id !== attempt.account_id) return;
    send("refresh_profile");
}

function profileHasExternalPurchase(attempt: ExternalPurchaseAttempt) {
    if (!attempt || !menuUi.state || menuUi.state.account_id !== attempt.account_id) return false;
    if (attempt.kind === "bundle") return Number(menuUi.state.gems) > Number(attempt.base_gems || 0);
    if (attempt.kind === "leader") {
        const leader = storeLeaderById(attempt.item_id);
        return !!leader && !!leader.owned;
    }
    const skin = storeSkinById(attempt.item_id);
    return !!skin && !!skin.owned;
}

function purchaseIntentForAttempt(attempt: ExternalPurchaseAttempt): PurchaseIntent | null {
    if (!attempt) return null;
    if (attempt.kind === "leader")
        return { type: "leader", leaderId: attempt.item_id, productId: attempt.product_id };
    if (attempt.kind === "skin")
        return { type: "skin", skinId: attempt.item_id, productId: attempt.product_id };
    return { type: "bundle", productId: attempt.product_id };
}

function showExternalPurchaseProcessing(attempt: ExternalPurchaseAttempt) {
    if (!attempt || attempt.dismissed || !attempt.provider_complete) return;
    menuUi.purchaseIntent = purchaseIntentForAttempt(attempt);
    menuUi.purchaseModal = {
        type: attempt.kind === "bundle" ? "bundle" : "product",
        productId: attempt.product_id,
        leaderId: attempt.kind === "leader" ? attempt.item_id : "",
        skinId: attempt.kind === "skin" ? attempt.item_id : "",
        bundleId: attempt.kind === "bundle" ? attempt.item_id : "",
        phase: "processing",
        submitted: true,
        timedOut: !!attempt.timed_out,
    };
}

// One row of POST /store/purchases (sow-data PurchaseRecord); only the fields read here.
interface StorePurchaseRecord {
    id: string;
    product_id: string;
    status: string;
    updated_at: number;
}

function purchaseHistory(): Promise<StorePurchaseRecord[]> {
    const auth = storeAuth();
    if (!auth.available || !menuUi.state || !menuUi.state.account_id)
        return Promise.reject(new Error("store unavailable"));
    return fetch(profileApi("/store/purchases"), {
        method: "POST",
        headers: auth.headers,
        body: JSON.stringify({ account_id: menuUi.state.account_id, auth_secret: auth.authSecret }),
    }).then(function (response) {
        if (!response.ok) throw new Error("purchase history unavailable");
        return response.json();
    });
}

function deliveredPurchaseRecord(records: StorePurchaseRecord[], attempt: ExternalPurchaseAttempt) {
    const startedAt = Number(attempt && attempt.started_at) || 0;
    return (
        (Array.isArray(records) ? records : []).find(function (record) {
            return (
                record &&
                record.status === "granted" &&
                record.product_id === attempt.product_id &&
                Number(record.updated_at || 0) * 1000 >= startedAt - 1000
            );
        }) || null
    );
}

function scheduleExternalPurchasePoll(delay?: number) {
    const attempt = currentExternalPurchaseAttempt();
    if (!attempt || externalPurchasePollTimer !== null || attempt.timed_out) return;
    externalPurchasePollTimer = window.setTimeout(
        function () {
            externalPurchasePollTimer = null;
            pollExternalPurchaseDelivery();
        },
        delay == null ? EXTERNAL_PURCHASE_POLL_MS : delay,
    );
}

function timeOutExternalPurchase(attempt: ExternalPurchaseAttempt) {
    if (!attempt || attempt.timed_out) return;
    attempt.timed_out = true;
    saveExternalPurchaseAttempt();
    clearExternalPurchasePoll();
    if (attempt.provider_complete && !attempt.dismissed) {
        showExternalPurchaseProcessing(attempt);
        render();
    }
}

function pollExternalPurchaseDelivery() {
    const attempt = currentExternalPurchaseAttempt();
    if (
        !attempt ||
        externalPurchasePollInFlight ||
        !menuUi.state ||
        menuUi.state.account_id !== attempt.account_id
    )
        return;
    if (Date.now() >= Number(attempt.deadline_at || 0)) {
        timeOutExternalPurchase(attempt);
        return;
    }
    externalPurchasePollInFlight = true;
    purchaseHistory()
        .then(function (records) {
            const delivered = deliveredPurchaseRecord(records, attempt);
            if (delivered) {
                attempt.delivery_id = delivered.id;
                attempt.provider_complete = true;
                attempt.dismissed = false;
                saveExternalPurchaseAttempt();
                showExternalPurchaseProcessing(attempt);
                requestExternalPurchaseProfile(attempt);
                render();
            }
        })
        .catch(function () {})
        .finally(function () {
            externalPurchasePollInFlight = false;
            const latest = currentExternalPurchaseAttempt();
            if (!latest || latest !== attempt) return;
            if (Date.now() >= Number(latest.deadline_at || 0)) timeOutExternalPurchase(latest);
            else scheduleExternalPurchasePoll();
        });
}

function beginBundlePurchasePresentation(attempt: ExternalPurchaseAttempt) {
    if (!attempt || !menuUi.state || !profileHasExternalPurchase(attempt)) return;
    clearExternalPurchaseAttempt();
    trackStoreEvent("store_purchase_complete", "gem_pack", attempt.product_id, "usd");
    menuUi.purchaseIntent = null;
    menuUi.purchaseModal = {
        type: "bundle",
        productId: attempt.product_id,
        bundleId: attempt.item_id,
        gems: attempt.gems,
        phase: "success",
        submitted: false,
        gemBaseline: Math.max(0, Number(attempt.base_gems) || 0),
        gemPresented: false,
        gemFlightStarted: false,
    };
    render();
    if (externalPurchaseRevealTimer !== null) window.clearTimeout(externalPurchaseRevealTimer);
    externalPurchaseRevealTimer = window.setTimeout(function () {
        externalPurchaseRevealTimer = null;
        if (
            !menuUi.purchaseModal ||
            menuUi.purchaseModal.type !== "bundle" ||
            menuUi.purchaseModal.phase !== "success" ||
            menuUi.purchaseModal.gemFlightStarted
        )
            return;
        menuUi.purchaseModal.gemFlightStarted = true;
        const finalValues = progressionFromState();
        const startValues = Object.assign({}, finalValues, { gems: menuUi.purchaseModal.gemBaseline });
        writeProgression(startValues);
        if (reducedRewardMotion()) {
            menuUi.purchaseModal.gemPresented = true;
            writeProgression(finalValues);
            return;
        }
        const token = root.querySelector<HTMLElement>(".sow-purchase-modal__bundle-token");
        const animationToken = rewardAnimationToken;
        if (!token) {
            menuUi.purchaseModal.gemPresented = true;
            animateProgressionTo(finalValues, 640, animationToken).then(function () {
                pulseRewardCounter("[data-progression-gems-value]", false);
            });
            return;
        }
        flyBundleToken(token, animationToken).then(function (arrived) {
            if (!arrived || !menuUi.purchaseModal || menuUi.purchaseModal.type !== "bundle") return;
            animateProgressionTo(finalValues, 640, animationToken).then(function (shown) {
                if (!shown || !menuUi.purchaseModal || menuUi.purchaseModal.type !== "bundle") return;
                menuUi.purchaseModal.gemPresented = true;
                pulseRewardCounter("[data-progression-gems-value]", false);
            });
        });
    }, 760);
}

function settleBundlePurchasePresentation() {
    if (externalPurchaseRevealTimer !== null) window.clearTimeout(externalPurchaseRevealTimer);
    externalPurchaseRevealTimer = null;
    if (bundleFlightToken) {
        if (typeof bundleFlightToken.getAnimations === "function")
            bundleFlightToken.getAnimations().forEach(function (animation) {
                animation.cancel();
            });
        bundleFlightToken.remove();
        bundleFlightToken = null;
    }
    if (menuUi.purchaseModal && menuUi.purchaseModal.type === "bundle")
        menuUi.purchaseModal.gemPresented = true;
    if (menuUi.state) writeProgression(progressionFromState());
}

function flyBundleToken(token: HTMLElement, animationToken: number) {
    const rect = token.getBoundingClientRect();
    const flyer = token.cloneNode(true) as HTMLElement;
    flyer.className += " sow-purchase-modal__bundle-flyer";
    flyer.style.cssText =
        "position:fixed;z-index:30;left:" +
        rect.left +
        "px;top:" +
        rect.top +
        "px;width:" +
        rect.width +
        "px;height:" +
        rect.height +
        "px;margin:0;pointer-events:none;";
    document.body.appendChild(flyer);
    bundleFlightToken = flyer;
    return flyRewardCard(
        { card: flyer, stage: { selector: "[data-progression-gems-value]" } },
        animationToken,
    ).then(function (arrived) {
        if (bundleFlightToken === flyer) bundleFlightToken = null;
        flyer.remove();
        return arrived;
    });
}

function beginPurchaseSuccessReveal() {
    if (!menuUi.purchaseModal || menuUi.purchaseModal.phase !== "spending") return;
    if (reducedRewardMotion()) {
        menuUi.purchaseModal.phase = "success";
        render();
        return;
    }
    if (purchaseSuccessTimer !== null) window.clearTimeout(purchaseSuccessTimer);
    purchaseSuccessTimer = window.setTimeout(function () {
        purchaseSuccessTimer = null;
        if (!menuUi.purchaseModal || menuUi.purchaseModal.phase !== "spending") return;
        menuUi.purchaseModal.phase = "success";
        render();
    }, 460);
}

export function dismissPurchaseModal() {
    if (menuUi.purchaseModal && menuUi.purchaseModal.type === "bundle") settleBundlePurchasePresentation();
    if (menuUi.purchaseModal && menuUi.purchaseModal.phase === "processing") {
        const attempt = currentExternalPurchaseAttempt();
        if (attempt && attempt.timed_out) {
            attempt.dismissed = true;
            saveExternalPurchaseAttempt();
        }
    }
    if (purchaseSuccessTimer !== null) window.clearTimeout(purchaseSuccessTimer);
    purchaseSuccessTimer = null;
    menuUi.purchaseModal = null;
    menuUi.purchaseIntent = null;
}

export function canDismissPurchaseModal() {
    return (
        !menuUi.purchaseModal || menuUi.purchaseModal.phase !== "processing" || menuUi.purchaseModal.timedOut
    );
}

export function abandonExternalPurchaseAttempt(productId: string | undefined) {
    const attempt = currentExternalPurchaseAttempt();
    if (productId && (!attempt || attempt.product_id !== productId)) return;
    clearExternalPurchaseAttempt();
}

export function completeExternalPurchase(productId: string) {
    if (
        menuUi.purchaseModal &&
        menuUi.purchaseModal.productId === productId &&
        (menuUi.purchaseModal.phase === "spending" || menuUi.purchaseModal.phase === "success")
    )
        return;
    let attempt = currentExternalPurchaseAttempt();
    if (!attempt || attempt.product_id !== productId) attempt = beginExternalPurchaseAttempt(productId);
    if (!attempt) return;
    attempt.provider_complete = true;
    attempt.dismissed = false;
    attempt.timed_out = false;
    attempt.deadline_at = Date.now() + EXTERNAL_PURCHASE_WAIT_MS;
    saveExternalPurchaseAttempt();
    showExternalPurchaseProcessing(attempt);
    requestExternalPurchaseProfile(attempt);
    render();
    pollExternalPurchaseDelivery();
}

export function resumeExternalPurchaseDelivery(fromReturn: boolean) {
    const attempt = currentExternalPurchaseAttempt();
    if (!attempt) return;
    if (fromReturn && attempt.timed_out) {
        attempt.timed_out = false;
        attempt.dismissed = false;
        attempt.deadline_at = Date.now() + EXTERNAL_PURCHASE_WAIT_MS;
        saveExternalPurchaseAttempt();
    }
    if (attempt.delivery_id && profileHasExternalPurchase(attempt)) {
        if (attempt.kind === "bundle") beginBundlePurchasePresentation(attempt);
        else {
            showExternalPurchaseProcessing(attempt);
            resolvePurchaseModal();
        }
        return;
    }
    if ((attempt.provider_complete || attempt.delivery_id) && !attempt.dismissed)
        showExternalPurchaseProcessing(attempt);
    scheduleExternalPurchasePoll(0);
}

function storeProductButton(productId: string, label: string) {
    return (
        "<button class='sow-store__buy sow-store__buy--primary' type='button' data-command='buy_product' data-product-id='" +
        esc(productId) +
        "'>" +
        esc(label || SOW_t("store.buy")) +
        "</button>"
    );
}

// Real-money gem packs are sold only through the Android app (Google Play).
function canRenderProductPurchase(productId: string) {
    return (
        isAndroidTwa() &&
        typeof window.SOW_androidPurchaseSupports === "function" &&
        window.SOW_androidPurchaseSupports(productId)
    );
}

function storeLeaderById(id: string | undefined) {
    const normalize = function (value: string | undefined) {
        return String(value || "")
            .replace(/[^a-z0-9]/gi, "")
            .toLowerCase();
    };
    const target = normalize(id);
    return (
        ((menuUi.state!.store && menuUi.state!.store.leaders) || []).find(function (leader) {
            return normalize(leader.id) === target;
        }) || null
    );
}

function storeSkinById(id: string) {
    return (
        ((menuUi.state!.store && menuUi.state!.store.skins) || []).find(function (skin) {
            return String(skin.id) === String(id);
        }) || null
    );
}

function storeOffers(): MenuStoreOffer[] {
    const offers = menuUi.state!.store && menuUi.state!.store.offers;
    if (!Array.isArray(offers)) return [];
    const now = Date.now() / 1000;
    return offers.filter(function (offer) {
        return Number(offer.ends_at) > now;
    });
}

function storeOfferById(id: string | undefined) {
    if (!id) return null;
    return (
        storeOffers().find(function (offer) {
            return offer.id === id;
        }) || null
    );
}

function storeBundleOffers(): MenuStoreBundleOffer[] {
    const bundles = menuUi.state!.store && menuUi.state!.store.bundles;
    if (!Array.isArray(bundles)) return [];
    const now = Date.now() / 1000;
    return bundles.filter(function (bundle) {
        return Number(bundle.ends_at) > now;
    });
}

function storeBundleOfferById(id: string | undefined): MenuStoreBundleOffer | null {
    if (!id) return null;
    return (
        storeBundleOffers().find(function (bundle) {
            return bundle.id === id;
        }) || null
    );
}

function discountedStoreAmount(base: number, percent: number, currency: "gems" | "crowns") {
    const step = currency === "gems" ? 5 : 25;
    const raw = Math.floor((base * (100 - percent)) / 100);
    return Math.min(base, Math.max(1, Math.floor(raw / step) * step));
}

function activeOfferForItem(
    kind: "leader" | "skin",
    itemId: string | undefined,
    currency: "gems" | "crowns",
): MenuStoreOffer | null {
    if (!itemId) return null;
    const norm = function (v: string) {
        return String(v || "")
            .replace(/[^a-z0-9]/gi, "")
            .toLowerCase();
    };
    const target = norm(itemId);
    return (
        storeOffers().find(function (offer) {
            return offer.kind === kind && offer.currency === currency && norm(offer.item_id) === target;
        }) || null
    );
}

interface BundleQuoteItem {
    id: string;
    name: string;
    art: string;
    owned: boolean;
    baseCrowns: number;
    baseGems: number;
}

function computeBundleQuote(bundle: MenuStoreBundleOffer) {
    const isSkin = bundle.kind === "skin";
    const items: BundleQuoteItem[] = [];
    let unownedCount = 0;
    let ownedCount = 0;
    let baseCrowns = 0;
    let baseGems = 0;

    for (const itemId of bundle.item_ids || []) {
        if (isSkin) {
            const skin = storeSkinById(itemId);
            if (!skin) continue;
            const owned = !!skin.owned;
            if (owned) ownedCount += 1;
            else {
                unownedCount += 1;
                baseCrowns += Number(skin.cost_crowns) || 0;
                baseGems += Number(skin.cost_gems) || 0;
            }
            items.push({
                id: skin.id,
                name: skin.name,
                art: asset(skin.asset_path),
                owned: owned,
                baseCrowns: Number(skin.cost_crowns) || 0,
                baseGems: Number(skin.cost_gems) || 0,
            });
        } else {
            const offer = storeLeaderById(itemId);
            if (!offer) continue;
            const leader = leaderById(offer.id);
            const owned = !!offer.owned || !!offer.permanently_free;
            if (owned) ownedCount += 1;
            else {
                unownedCount += 1;
                baseCrowns += Number(offer.cost_crowns) || 0;
                baseGems += Number(offer.cost_gems) || 0;
            }
            items.push({
                id: offer.id,
                name: leaderDisplayName(leader),
                art: avatarImage(offer.id),
                owned: owned,
                baseCrowns: Number(offer.cost_crowns) || 0,
                baseGems: Number(offer.cost_gems) || 0,
            });
        }
    }

    const allOwned = items.length > 0 && unownedCount === 0;
    const costCrowns =
        unownedCount > 0 ? discountedStoreAmount(baseCrowns, bundle.discount_percent, "crowns") : 0;
    const costGems =
        unownedCount > 0 ? discountedStoreAmount(baseGems, bundle.discount_percent, "gems") : 0;

    return {
        items,
        unownedCount,
        ownedCount,
        allOwned,
        baseCrowns,
        costCrowns,
        baseGems,
        costGems,
    };
}

function offerItemOwned(offer: MenuStoreOffer) {
    if (offer.kind === "skin") {
        const skin = storeSkinById(offer.item_id);
        return !skin || !!skin.owned;
    }
    const leader = storeLeaderById(offer.item_id);
    return !leader || !!leader.owned || !!leader.permanently_free;
}

// Remaining time as "2d 5h", "5h 12m" or "12m"; the catalog reloads at expiry.
function offerTimeLeft(endsAt: number) {
    const seconds = Math.max(0, Math.floor(Number(endsAt) - Date.now() / 1000));
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.max(1, Math.floor((seconds % 3600) / 60));
    if (days > 0) return days + "d " + hours + "h";
    if (hours > 0) return hours + "h " + minutes + "m";
    return minutes + "m";
}

function renderLeaderCurrencyButton(offer: MenuLeaderOffer, currency: "gems" | "crowns") {
    const isGems = currency === "gems";
    const liveOffer = activeOfferForItem("leader", offer.id, currency);
    const baseCost = Math.max(0, Number(isGems ? offer.cost_gems : offer.cost_crowns) || 0);
    const cost = liveOffer ? Number(liveOffer.cost) : baseCost;
    const insufficient = storeBalance(currency) < cost;
    const disabled = insufficient || (menuUi.state && menuUi.state.store_busy) ? " disabled" : "";
    const amountClass = insufficient ? "sow-store__currency-amount--insufficient" : "";
    const strikeHtml = liveOffer
        ? "<s class='sow-store__list-price'>" + esc(baseCost) + "</s> "
        : "";
    return (
        "<button class='sow-store__buy" +
        (liveOffer ? " sow-store__buy--on-sale" : "") +
        (insufficient ? " sow-store__buy--insufficient" : "") +
        "' type='button' data-command='unlock_leader' data-currency='" +
        currency +
        "' data-leader-id='" +
        esc(offer.id) +
        "'" +
        disabled +
        ">" +
        strikeHtml +
        renderCurrencyAmount(cost, isGems ? "gem" : "crown", amountClass) +
        "</button>"
    );
}

export function renderLeaderPurchase(leader: MenuLeader) {
    const offer = storeLeaderById(leader.id);
    const leaderActions =
        !offer || offer.owned || offer.permanently_free
            ? ""
            : renderLeaderCurrencyButton(offer, "crowns") + renderLeaderCurrencyButton(offer, "gems");
    return leaderActions ? "<div class='sow-heroes__purchase-actions'>" + leaderActions + "</div>" : "";
}

// Entry button to open the global skin picker modal from the featured hero card.
export function renderSkinsButton() {
    return (
        "<button class='sow-heroes__skins sow-store__buy' type='button' data-command='open_skin_picker'>" +
        esc(SOW_t("store.skins")) +
        "</button>"
    );
}

export function renderSkinPickerModal() {
    if (!menuUi.skinPickerOpen) return "";
    const skins = Array.isArray(menuUi.state!.store && menuUi.state!.store.skins)
        ? menuUi.state!.store.skins
        : [];
    return (
        "<div class='sow-menu__overlay' data-menu-overlay='skin-picker'><section class='sow-menu__modal sow-skin-picker' role='dialog' aria-modal='true' aria-label='" +
        esc(SOW_t("store.skins")) +
        "'><div class='sow-menu__modal-head'><h2>" +
        esc(SOW_t("store.skins")) +
        "</h2><button class='sow-menu__icon-button' type='button' data-command='close_skin_picker' aria-label='" +
        esc(SOW_t("menu.close")) +
        "'>×</button></div><div class='sow-store__skin-grid'>" +
        (skins.map(renderStoreSkinPromo).join("") ||
            "<p class='sow-menu__empty'>" + esc(SOW_t("store.offers_unavailable")) + "</p>") +
        "</div></section></div>"
    );
}

export function openLeaderPurchase(leaderId: string, currency: string, storeOffer?: MenuStoreOffer | null) {
    const offer = storeLeaderById(leaderId);
    const isGems = currency === "gems";
    const resolvedOffer =
        storeOffer || (offer ? activeOfferForItem("leader", offer.id, isGems ? "gems" : "crowns") : null);
    const cost = resolvedOffer
        ? resolvedOffer.cost
        : offer && (isGems ? offer.cost_gems : offer.cost_crowns);
    if (
        !offer ||
        offer.owned ||
        (menuUi.state && menuUi.state.store_busy) ||
        storeBalance(isGems ? "gems" : "crowns") < Number(cost || 0)
    )
        return;
    menuUi.state!.error = null;
    const offerId = resolvedOffer ? resolvedOffer.id : undefined;
    menuUi.purchaseIntent = { type: "leader", leaderId: offer.id, offerId: offerId };
    menuUi.purchaseModal = {
        type: "leader",
        leaderId: offer.id,
        currency: currency,
        offerId: offerId,
        phase: "confirm",
        submitted: false,
    };
    trackStoreEvent("store_buy_tap", "leader", offer.id, isGems ? "gems" : "crowns");
    render();
}

export function openSkinPurchase(skinId: string, currency: string, storeOffer?: MenuStoreOffer | null) {
    const skin = storeSkinById(skinId);
    const isGems = currency !== "crowns";
    const resolvedOffer =
        storeOffer || (skin ? activeOfferForItem("skin", skin.id, isGems ? "gems" : "crowns") : null);
    const cost = resolvedOffer ? resolvedOffer.cost : skin && (isGems ? skin.cost_gems : skin.cost_crowns);
    if (
        !skin ||
        skin.owned ||
        (menuUi.state && menuUi.state.store_busy) ||
        storeBalance(isGems ? "gems" : "crowns") < Number(cost || 0)
    )
        return;
    menuUi.state!.error = null;
    const offerId = resolvedOffer ? resolvedOffer.id : undefined;
    menuUi.purchaseIntent = { type: "skin", skinId: skin.id, offerId: offerId };
    menuUi.purchaseModal = {
        type: "skin",
        skinId: skin.id,
        currency: isGems ? "gems" : "crowns",
        offerId: offerId,
        phase: "confirm",
        submitted: false,
    };
    trackStoreEvent("store_buy_tap", "skin", skin.id, isGems ? "gems" : "crowns");
    render();
}

export function openOfferPurchase(offerId: string, currency?: string) {
    const bundleOffer = storeBundleOfferById(offerId);
    if (bundleOffer) {
        const quote = computeBundleQuote(bundleOffer);
        const isGems = currency === "gems";
        const cost = isGems ? quote.costGems : quote.costCrowns;
        if (
            quote.allOwned ||
            (menuUi.state && menuUi.state.store_busy) ||
            storeBalance(isGems ? "gems" : "crowns") < cost
        )
            return;
        menuUi.state!.error = null;
        menuUi.purchaseIntent = {
            type: "store_bundle",
            bundleOfferId: bundleOffer.id,
        };
        menuUi.purchaseModal = {
            type: "store_bundle",
            bundleOfferId: bundleOffer.id,
            currency: isGems ? "gems" : "crowns",
            phase: "confirm",
            submitted: false,
        };
        trackStoreEvent("store_buy_tap", "bundle", bundleOffer.id, isGems ? "gems" : "crowns");
        render();
        return;
    }
    const offer = storeOfferById(offerId);
    if (!offer || offerItemOwned(offer)) return;
    if (offer.kind === "skin") openSkinPurchase(offer.item_id, offer.currency, offer);
    else openLeaderPurchase(offer.item_id, offer.currency, offer);
}

function purchaseModalCost() {
    const modal = menuUi.purchaseModal!;
    if (modal.type === "store_bundle") {
        const bundle = storeBundleOfferById(modal.bundleOfferId);
        if (!bundle) return 0;
        const quote = computeBundleQuote(bundle);
        return modal.currency === "gems" ? quote.costGems : quote.costCrowns;
    }
    const offer = storeOfferById(modal.offerId);
    if (offer) return offer.cost;
    const isGems = modal.currency === "gems";
    if (modal.skinId) {
        const skin = storeSkinById(modal.skinId);
        return skin ? (isGems ? skin.cost_gems : skin.cost_crowns) : 0;
    }
    const leader = storeLeaderById(modal.leaderId);
    return leader ? (isGems ? leader.cost_gems : leader.cost_crowns) : 0;
}

function bundleBuyButton(bundle: MenuGemBundle) {
    return canRenderProductPurchase(bundle.product_id)
        ? storeProductButton(bundle.product_id, SOW_t("store.buy"))
        : "";
}

function bundleBadge(bundle: MenuGemBundle, bundles: MenuGemBundle[]) {
    const bonus = Number(bundle.bonus_percent) || 0;
    if (!bonus) return "";
    const best = bundles.every(function (other) {
        return (Number(other.bonus_percent) || 0) <= bonus;
    });
    return (
        "<small class='sow-store__bonus'>" +
        esc(best ? SOW_t("store.best_value") : SOW_t("store.bonus_percent", { amount: bonus })) +
        "</small>"
    );
}

function renderStoreBundle(bundle: MenuGemBundle, _index: number, bundles: MenuGemBundle[]) {
    const art = bundle.asset_path
        ? asset(bundle.asset_path)
        : asset("gameplay/store/gem_bundles/" + bundle.id + ".webp");
    return (
        "<article class='sow-store__bundle'><img class='sow-store__bundle-icon' src='" +
        esc(art) +
        "' alt='' aria-hidden='true' width='56' height='56' loading='lazy'><div><strong class='sow-store__bundle-price' aria-label='" +
        esc(SOW_t("store.gems_count", { amount: bundle.gems })) +
        "'>" +
        renderCurrencyAmount(bundle.gems, "gem") +
        "</strong>" +
        bundleBadge(bundle, bundles) +
        "</div>" +
        bundleBuyButton(bundle) +
        "</article>"
    );
}

function renderSkinCurrencyButton(skin: MenuSkinOffer, currency: "gems" | "crowns") {
    const isGems = currency === "gems";
    const liveOffer = activeOfferForItem("skin", skin.id, currency);
    const baseCost = Math.max(0, Number(isGems ? skin.cost_gems : skin.cost_crowns) || 0);
    const cost = liveOffer ? Number(liveOffer.cost) : baseCost;
    const insufficient = storeBalance(currency) < cost;
    const disabled = insufficient || (menuUi.state && menuUi.state.store_busy) ? " disabled" : "";
    const amountClass = insufficient ? "sow-store__currency-amount--insufficient" : "";
    const strikeHtml = liveOffer
        ? "<s class='sow-store__list-price'>" + esc(baseCost) + "</s> "
        : "";
    return (
        "<button class='sow-store__buy" +
        (liveOffer ? " sow-store__buy--on-sale" : "") +
        (insufficient ? " sow-store__buy--insufficient" : "") +
        "' type='button' data-command='open_skin_purchase' data-currency='" +
        currency +
        "' data-skin-id='" +
        esc(skin.id) +
        "'" +
        disabled +
        ">" +
        strikeHtml +
        renderCurrencyAmount(cost, isGems ? "gem" : "crown", amountClass) +
        "</button>"
    );
}

function renderStoreSkinPromo(skin: MenuSkinOffer) {
    const liveGemOffer = activeOfferForItem("skin", skin.id, "gems");
    const liveCrownOffer = activeOfferForItem("skin", skin.id, "crowns");
    const discountPct = liveGemOffer
        ? liveGemOffer.discount_percent
        : liveCrownOffer
          ? liveCrownOffer.discount_percent
          : 0;
    let action;
    const busyDisabled = menuUi.state && menuUi.state.store_busy ? " disabled" : "";
    const isEquipped = Boolean(skin.owned && menuUi.state!.selected_skin === skin.id);
    if (skin.owned) {
        action = isEquipped
            ? "<button class='sow-store__buy sow-store__buy--equipped' type='button' data-command='unequip_skin' data-skin-id='" +
              esc(skin.id) +
              "' aria-pressed='true'" +
              busyDisabled +
              "><span>" +
              esc(SOW_t("store.equipped")) +
              "</span><span aria-hidden='true'>×</span></button>"
            : "<button class='sow-store__buy' type='button' data-command='equip_skin' data-skin-id='" +
              esc(skin.id) +
              "' aria-pressed='false'" +
              busyDisabled +
              ">" +
              esc(SOW_t("store.equip")) +
              "</button>";
    } else if (Number(skin.cost_gems) <= 0 && Number(skin.cost_crowns) <= 0) {
        action =
            "<div class='sow-store__buy-row'>" +
            "<button class='sow-store__buy' type='button' data-command='open_campaign' data-saga-id='advanced_tactics'>" +
            esc(SOW_t("lobbies.campaign_advanced_tactics_title") || "Advanced Tactics") +
            "</button></div>";
    } else {
        action =
            "<div class='sow-store__buy-row'>" +
            renderSkinCurrencyButton(skin, "gems") +
            (Number(skin.cost_crowns) > 0 ? renderSkinCurrencyButton(skin, "crowns") : "") +
            "</div>";
    }
    return (
        "<article class='sow-store__skin" +
        (isEquipped ? " is-equipped" : "") +
        (discountPct > 0 && !skin.owned ? " is-on-sale" : "") +
        "'><div class='sow-store__skin-art'><img src='" +
        esc(asset(skin.asset_path)) +
        "' alt='' width='96' height='96' loading='lazy'>" +
        (discountPct > 0 && !skin.owned
            ? "<b class='sow-store__discount'>-" + esc(discountPct) + "%</b>"
            : "") +
        "</div><div class='sow-store__skin-body'><h3>" +
        esc(skin.name) +
        "</h3><p>" +
        esc(SOW_t("store.territory_pattern")) +
        "</p>" +
        action +
        "</div></article>"
    );
}

function renderBundleCurrencyButton(
    bundle: MenuStoreBundleOffer,
    currency: "crowns" | "gems",
    baseCost: number,
    cost: number,
) {
    const isGems = currency === "gems";
    const insufficient = storeBalance(currency) < cost;
    const disabled = insufficient || (menuUi.state && menuUi.state.store_busy) ? " disabled" : "";
    const amountClass = insufficient ? "sow-store__currency-amount--insufficient" : "";
    return (
        "<button class='sow-store__buy sow-store__bundle-buy" +
        (insufficient ? " sow-store__buy--insufficient" : "") +
        "' type='button' data-command='open_offer_purchase' data-offer-id='" +
        esc(bundle.id) +
        "' data-currency='" +
        currency +
        "'" +
        disabled +
        ">" +
        "<s class='sow-store__list-price'>" +
        esc(baseCost) +
        "</s> " +
        renderCurrencyAmount(cost, isGems ? "gem" : "crown", amountClass) +
        "</button>"
    );
}

function renderStoreBundleOfferCard(bundle: MenuStoreBundleOffer) {
    const quote = computeBundleQuote(bundle);
    if (!quote.items.length) return "";
    const isSkin = bundle.kind === "skin";

    const portraitsHtml = quote.items
        .map(function (item) {
            return (
                "<span class='sow-store__bundle-portrait" +
                (isSkin ? " is-skin" : "") +
                (item.owned ? " is-owned" : "") +
                "'><img src='" +
                esc(item.art) +
                "' alt='" +
                esc(item.name) +
                "' width='72' height='72' loading='lazy'>" +
                (item.owned ? "<i class='sow-store__bundle-owned-check'>✓</i>" : "") +
                "</span>"
            );
        })
        .join("");

    const rosterChipsHtml = quote.items
        .map(function (item) {
            if (isSkin) {
                return (
                    "<span class='sow-store__bundle-chip" +
                    (item.owned ? " is-owned" : "") +
                    "'>" +
                    (item.owned ? "✓ " : "") +
                    esc(item.name) +
                    "</span>"
                );
            }
            return (
                "<button type='button' class='sow-store__bundle-chip" +
                (item.owned ? " is-owned" : "") +
                "' data-command='open_hero_info' data-leader-id='" +
                esc(item.id) +
                "' data-hero-info-tab='overview'>" +
                (item.owned ? "✓ " : "") +
                esc(item.name) +
                "</button>"
            );
        })
        .join("");

    const proratedNote =
        quote.ownedCount > 0 && !quote.allOwned
            ? "<span class='sow-store__bundle-prorated'>✓ " +
              esc(quote.ownedCount + "/" + quote.items.length) +
              " " +
              esc(SOW_t("store.unlocked")) +
              "</span>"
            : "";

    const actionsHtml = quote.allOwned
        ? "<div class='sow-store__bundle-complete'><span class='sow-store__offer-state'>✓ " +
          esc(SOW_t("store.unlocked")) +
          "</span></div>"
        : "<div class='sow-store__buy-row sow-store__bundle-actions'>" +
          renderBundleCurrencyButton(bundle, "crowns", quote.baseCrowns, quote.costCrowns) +
          renderBundleCurrencyButton(bundle, "gems", quote.baseGems, quote.costGems) +
          "</div>";

    return (
        "<article class='sow-store__bundle-card is-theme-" +
        esc(bundle.theme_key) +
        (quote.allOwned ? " is-owned" : "") +
        "'>" +
        "<div class='sow-store__bundle-top'>" +
        "<span class='sow-store__bundle-discount'>-" +
        esc(bundle.discount_percent) +
        "% OFF</span>" +
        "<span class='sow-store__bundle-timer'>" +
        esc(offerTimeLeft(bundle.ends_at)) +
        "</span>" +
        "</div>" +
        "<div class='sow-store__bundle-visual'>" +
        "<div class='sow-store__bundle-portraits'>" +
        portraitsHtml +
        "</div>" +
        "<div class='sow-store__bundle-copy'>" +
        "<h3>" +
        esc(bundle.title) +
        "</h3>" +
        "<p>" +
        esc(bundle.subtitle) +
        "</p>" +
        proratedNote +
        "</div>" +
        "</div>" +
        "<div class='sow-store__bundle-chips'>" +
        rosterChipsHtml +
        "</div>" +
        actionsHtml +
        "</article>"
    );
}

function renderBundlesShowcaseSection(kindFilter?: "leader" | "skin") {
    const bundles = storeBundleOffers().filter(function (bundle) {
        return !kindFilter || bundle.kind === kindFilter;
    });
    if (!bundles.length) return "";
    const cards = bundles.map(renderStoreBundleOfferCard).join("");
    if (!cards) return "";
    return (
        "<section class='sow-store__section sow-store__section--bundles' aria-labelledby='sow-store-weekly-bundles'>" +
        "<div class='sow-store__section-head'>" +
        "<h2 id='sow-store-weekly-bundles'>Weekly Value Bundles</h2>" +
        "<span>" +
        esc(SOW_t("store.ends_in", { time: offerTimeLeft(bundles[0].ends_at) })) +
        "</span>" +
        "</div>" +
        "<div class='sow-store__bundle-showcase'>" +
        cards +
        "</div>" +
        "</section>"
    );
}

function renderStoreOffer(offer: MenuStoreOffer) {
    const isSkin = offer.kind === "skin";
    const skin = isSkin ? storeSkinById(offer.item_id) : null;
    const leaderOffer = isSkin ? null : storeLeaderById(offer.item_id);
    if (isSkin ? !skin : !leaderOffer) return "";
    const leader = leaderOffer ? leaderById(leaderOffer.id) : null;
    const name = skin ? skin.name : leaderDisplayName(leader);
    const sublabel = skin ? SOW_t("store.territory_pattern") : leaderCivilization(leader);
    const art = skin ? asset(skin.asset_path) : avatarImage(leaderOffer!.id);
    const kind = offer.currency === "gems" ? "gem" : "crown";
    const insufficient = storeBalance(offer.currency) < Number(offer.cost);
    const disabled = insufficient || (menuUi.state && menuUi.state.store_busy) ? " disabled" : "";
    const action = offerItemOwned(offer)
        ? "<span class='sow-store__offer-state'>✓ " + esc(SOW_t("store.unlocked")) + "</span>"
        : "<button class='sow-store__buy sow-store__buy--deal" +
          (insufficient ? " sow-store__buy--insufficient" : "") +
          "' type='button' data-command='open_offer_purchase' data-offer-id='" +
          esc(offer.id) +
          "'" +
          disabled +
          ">" +
          "<s class='sow-store__list-price'>" +
          esc(offer.base_cost) +
          "</s> " +
          renderCurrencyAmount(
              offer.cost,
              kind,
              insufficient ? "sow-store__currency-amount--insufficient" : "",
          ) +
          "</button>";
    const inspectAttr = leaderOffer
        ? " data-command='open_hero_info' data-leader-id='" +
          esc(leaderOffer.id) +
          "' data-hero-info-tab='overview'"
        : "";
    const artTag = leaderOffer ? "button type='button'" : "div";
    const artClose = leaderOffer ? "button" : "div";
    return (
        "<article class='sow-store__skin sow-store__offer sow-store__deal-card'>" +
        "<" +
        artTag +
        " class='sow-store__skin-art" +
        (isSkin ? "" : " sow-store__skin-art--leader sow-store__hero-inspect") +
        "'" +
        inspectAttr +
        "><img src='" +
        esc(art) +
        "' alt='' width='96' height='96' loading='lazy'><b class='sow-store__discount'>-" +
        esc(offer.discount_percent) +
        "%</b></" +
        artClose +
        "><div class='sow-store__skin-body'>" +
        "<div class='sow-store__deal-meta'>" +
        "<span class='sow-store__deal-cadence'>" +
        esc(offer.cadence === "weekly" ? SOW_t("store.weekly_offers") : SOW_t("store.daily_offers")) +
        "</span>" +
        "<span class='sow-store__deal-timer'>" +
        esc(offerTimeLeft(offer.ends_at)) +
        "</span>" +
        "</div>" +
        "<h3>" +
        esc(name) +
        "</h3><p>" +
        esc(sublabel) +
        "</p>" +
        action +
        "</div></article>"
    );
}

function renderFlashDealsSection(kindFilter?: "leader" | "skin") {
    const offers = storeOffers().filter(function (offer) {
        return !kindFilter || offer.kind === kindFilter;
    });
    if (!offers.length) return "";
    const cards = offers.map(renderStoreOffer).join("");
    if (!cards) return "";
    return (
        "<section class='sow-store__section' aria-labelledby='sow-store-flash-deals'>" +
        "<div class='sow-store__section-head'>" +
        "<h2 id='sow-store-flash-deals'>" +
        esc(SOW_t("store.featured_offers")) +
        "</h2>" +
        "<span>" +
        esc(SOW_t("store.ends_in", { time: offerTimeLeft(offers[0].ends_at) })) +
        "</span>" +
        "</div>" +
        "<div class='sow-store__skin-grid sow-store__deal-grid'>" +
        cards +
        "</div></section>"
    );
}

function renderStoreHero(offer: MenuLeaderOffer) {
    const leader = leaderById(offer.id);
    const displayName = leaderDisplayName(leader);
    const liveGemOffer = activeOfferForItem("leader", offer.id, "gems");
    const liveCrownOffer = activeOfferForItem("leader", offer.id, "crowns");
    const discountPct = liveGemOffer
        ? liveGemOffer.discount_percent
        : liveCrownOffer
          ? liveCrownOffer.discount_percent
          : 0;
    return (
        "<article class='sow-store__skin sow-store__hero" +
        (discountPct > 0 ? " is-on-sale" : "") +
        "'>" +
        "<button type='button' class='sow-store__skin-art sow-store__skin-art--leader sow-store__hero-inspect' data-command='open_hero_info' data-leader-id='" +
        esc(offer.id) +
        "' data-hero-info-tab='overview' aria-label='" +
        esc(displayName) +
        "'><img src='" +
        esc(avatarImage(offer.id)) +
        "' alt='' width='96' height='96' loading='lazy'>" +
        (discountPct > 0 ? "<b class='sow-store__discount'>-" + esc(discountPct) + "%</b>" : "") +
        "</button>" +
        "<div class='sow-store__skin-body'><div class='sow-store__hero-head'><h3>" +
        esc(displayName) +
        "</h3><button type='button' class='sow-store__hero-bio-btn' data-command='open_hero_info' data-leader-id='" +
        esc(offer.id) +
        "' data-hero-info-tab='overview'>" +
        esc(SOW_t("heroes.bio_view_dossier")) +
        "</button></div><p>" +
        esc(leaderCivilization(leader)) +
        "</p><div class='sow-store__buy-row'>" +
        renderLeaderCurrencyButton(offer, "crowns") +
        renderLeaderCurrencyButton(offer, "gems") +
        "</div></div></article>"
    );
}

function renderHeroSection(leaders: MenuLeaderOffer[]) {
    const locked = leaders
        .filter(function (offer) {
            return !offer.owned && !offer.permanently_free && Number(offer.cost_crowns) > 0;
        })
        .sort(function (left, right) {
            const leftSale =
                activeOfferForItem("leader", left.id, "gems") ||
                activeOfferForItem("leader", left.id, "crowns")
                    ? 0
                    : 1;
            const rightSale =
                activeOfferForItem("leader", right.id, "gems") ||
                activeOfferForItem("leader", right.id, "crowns")
                    ? 0
                    : 1;
            if (leftSale !== rightSale) return leftSale - rightSale;
            return Number(left.cost_crowns) - Number(right.cost_crowns);
        });
    if (!locked.length) return "";
    return (
        "<section class='sow-store__section' aria-labelledby='sow-store-heroes'><div class='sow-store__section-head'><h2 id='sow-store-heroes'>" +
        esc(SOW_t("store.leaders")) +
        "</h2><button class='sow-store__link' type='button' data-command='main_nav' data-nav-screen='heroes'>" +
        esc(SOW_t("store.view_heroes")) +
        "</button></div><div class='sow-store__skin-grid'>" +
        locked.map(renderStoreHero).join("") +
        "</div></section>"
    );
}

function renderSkinsSection(skins: MenuSkinOffer[]) {
    const sorted = skins.slice().sort(function (left, right) {
        const leftOwned = left.owned ? 1 : 0;
        const rightOwned = right.owned ? 1 : 0;
        if (leftOwned !== rightOwned) return leftOwned - rightOwned;
        const leftSale =
            activeOfferForItem("skin", left.id, "gems") ||
            activeOfferForItem("skin", left.id, "crowns")
                ? 0
                : 1;
        const rightSale =
            activeOfferForItem("skin", right.id, "gems") ||
            activeOfferForItem("skin", right.id, "crowns")
                ? 0
                : 1;
        if (leftSale !== rightSale) return leftSale - rightSale;
        return Number(left.cost_gems || 0) - Number(right.cost_gems || 0);
    });
    return (
        "<section class='sow-store__section' aria-labelledby='sow-store-skins'><div class='sow-store__section-head'><h2 id='sow-store-skins'>" +
        esc(SOW_t("store.skins")) +
        "</h2><span>" +
        esc(SOW_t("store.territory_patterns")) +
        "</span></div><div class='sow-store__skin-grid'>" +
        (sorted.map(renderStoreSkinPromo).join("") ||
            "<p class='sow-menu__empty'>" + esc(SOW_t("store.offers_unavailable")) + "</p>") +
        "</div></section>"
    );
}

function purchaseItemOwned() {
    if (!menuUi.purchaseIntent) return false;
    if (menuUi.purchaseIntent.type === "store_bundle") {
        const bundle = storeBundleOfferById(menuUi.purchaseIntent.bundleOfferId);
        return !!bundle && computeBundleQuote(bundle).allOwned;
    }
    if (menuUi.purchaseIntent.skinId) {
        const skin = storeSkinById(menuUi.purchaseIntent.skinId);
        return !!skin && !!skin.owned;
    }
    const offer = storeLeaderById(menuUi.purchaseIntent.leaderId);
    return !!offer && !!offer.owned;
}

function renderPurchaseArt(
    leader: MenuLeader | null,
    skin: MenuSkinOffer | null,
    bundle: MenuGemBundle | null,
    storeBundle: MenuStoreBundleOffer | null,
    success: boolean,
) {
    let visualClass = "";
    let art;
    if (storeBundle) {
        const quote = computeBundleQuote(storeBundle);
        visualClass = " sow-purchase-modal__visual--bundle sow-purchase-modal__visual--store-bundle";
        art =
            "<div class='sow-store__bundle-portraits sow-store__bundle-portraits--modal'>" +
            quote.items
                .map(function (item) {
                    return (
                        "<span class='sow-store__bundle-portrait" +
                        (storeBundle.kind === "skin" ? " is-skin" : "") +
                        "'><img src='" +
                        esc(item.art) +
                        "' alt='" +
                        esc(item.name) +
                        "' width='88' height='88'></span>"
                    );
                })
                .join("") +
            "</div>";
    } else if (bundle) {
        const bundleArt = bundle.asset_path
            ? asset(bundle.asset_path)
            : asset("gameplay/store/gem_bundles/" + bundle.id + ".webp");
        visualClass = " sow-purchase-modal__visual--bundle";
        art =
            "<img class='sow-purchase-modal__art-image sow-purchase-modal__art-image--bundle sow-purchase-modal__bundle-token' src='" +
            esc(bundleArt) +
            "' alt='' width='128' height='128'>";
    } else if (skin) {
        visualClass = " sow-purchase-modal__visual--skin";
        art =
            "<img class='sow-purchase-modal__art-image sow-purchase-modal__art-image--skin' src='" +
            esc(asset(skin.asset_path)) +
            "' alt='" +
            esc(skin.name) +
            "' width='256' height='256'>";
    } else {
        const artSlug = leaderArtSlug(leader);
        const portrait =
            typeof window.matchMedia === "function"
                ? window.matchMedia("(orientation: portrait)").matches
                : window.innerHeight >= window.innerWidth;
        const mobileArt = leaderArtUrl(artSlug, "mobile");
        const desktopArt = leaderArtUrl(artSlug, "desktop");
        art =
            "<picture><source media='(orientation: portrait)' srcset='" +
            esc(mobileArt) +
            "'><source media='(orientation: landscape)' srcset='" +
            esc(desktopArt) +
            "'><img class='sow-purchase-modal__art-image' src='" +
            esc(portrait ? mobileArt : desktopArt) +
            "' alt='" +
            esc(leaderDisplayName(leader)) +
            "'></picture>";
    }
    const effects = success
        ? "<div class='sow-purchase-modal__effects' aria-hidden='true'><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i></div>"
        : "";
    return "<div class='sow-purchase-modal__visual" + visualClass + "'>" + art + "</div>" + effects;
}

export function renderPurchaseModal() {
    if (!menuUi.purchaseModal) return "";
    const leader = menuUi.purchaseModal.leaderId ? leaderById(menuUi.purchaseModal.leaderId) : null;
    const offer = menuUi.purchaseModal.leaderId ? storeLeaderById(menuUi.purchaseModal.leaderId) : null;
    const skin = menuUi.purchaseModal.skinId ? storeSkinById(menuUi.purchaseModal.skinId) : null;
    const bundle =
        menuUi.purchaseModal.type === "bundle"
            ? storeBundleByProductId(menuUi.purchaseModal.productId)
            : null;
    const storeBundle =
        menuUi.purchaseModal.type === "store_bundle"
            ? storeBundleOfferById(menuUi.purchaseModal.bundleOfferId)
            : null;
    if (
        (menuUi.purchaseModal.leaderId && (!leader || !offer)) ||
        (menuUi.purchaseModal.skinId && !skin) ||
        (menuUi.purchaseModal.type === "bundle" && !bundle) ||
        (menuUi.purchaseModal.type === "store_bundle" && !storeBundle)
    )
        return "";
    const isProduct = menuUi.purchaseModal.type === "product";
    const success = menuUi.purchaseModal.phase === "success";
    const spending = menuUi.purchaseModal.phase === "spending";
    const processing = menuUi.purchaseModal.phase === "processing";
    const busy =
        menuUi.purchaseModal.submitted ||
        menuUi.storeCheckoutBusy ||
        (menuUi.state && menuUi.state.store_busy);
    const itemName = bundle
        ? SOW_t("store.gems_count", { amount: bundle.gems })
        : storeBundle
          ? storeBundle.title
          : skin
            ? skin.name
            : leaderDisplayName(leader);
    const heading = bundle
        ? "<span class='sow-purchase-modal__bundle-amount sow-purchase-modal__price--gem'>" +
          renderCurrencyAmount(bundle.gems, "gem") +
          "</span>"
        : esc(itemName);
    // "product" modals only show delivery of an already paid leader/skin.
    const storeOffer = storeOfferById(menuUi.purchaseModal.offerId);
    const storeBundleQuote = storeBundle ? computeBundleQuote(storeBundle) : null;
    const currencyKind = menuUi.purchaseModal.currency === "gems" ? "gem" : "crown";
    const baseCost = storeBundleQuote
        ? menuUi.purchaseModal.currency === "gems"
            ? storeBundleQuote.baseGems
            : storeBundleQuote.baseCrowns
        : storeOffer
          ? storeOffer.base_cost
          : 0;
    const summary =
        bundle || isProduct
            ? ""
            : (baseCost > 0
                  ? "<s class='sow-store__list-price'>" +
                    renderCurrencyAmount(baseCost, currencyKind) +
                    "</s> "
                  : "") + renderCurrencyAmount(purchaseModalCost(), currencyKind);
    const error =
        !success && !spending && !processing && menuUi.state && menuUi.state.error
            ? "<div class='sow-menu__status sow-menu__status--error' role='alert'>" +
              esc(localizedText(menuUi.state.error)) +
              "</div>"
            : "";
    const body = success
        ? "<div class='sow-menu__modal-actions'><button class='sow-menu__primary' type='button' data-command='cancel_purchase'>" +
          esc(SOW_t("tutorial.continue")) +
          "</button></div>"
        : processing
          ? menuUi.purchaseModal.timedOut
              ? "<div class='sow-menu__modal-actions'><button class='sow-menu__primary' type='button' data-command='cancel_purchase'>" +
                esc(SOW_t("tutorial.continue")) +
                "</button></div>"
              : "<div class='sow-purchase-modal__pending' aria-busy='true' aria-hidden='true'><i></i></div>"
          : spending
            ? "<div class='sow-purchase-modal__spend' aria-hidden='true'><i></i><i></i><i></i></div>"
            : "<div class='sow-menu__modal-actions'><button class='sow-menu__ghost-button' type='button' data-command='cancel_purchase'>" +
              esc(SOW_t("lobbies.cancel")) +
              "</button><button class='sow-menu__primary' type='button' data-command='confirm_purchase'" +
              (busy ? " disabled" : "") +
              ">" +
              esc(SOW_t("store.buy")) +
              "</button></div>";
    const eyebrow = success ? SOW_t("store.unlocked") : SOW_t("store.purchase");
    const price =
        success || spending || processing || !summary
            ? ""
            : "<strong class='sow-purchase-modal__price sow-purchase-modal__price--" +
              currencyKind +
              "'>" +
              summary +
              "</strong>";
    const successClass = success ? " is-success" : "";
    const itemClass = menuUi.purchaseModal.leaderId ? " is-leader" : "";
    const phaseClass =
        (spending ? " is-spending" : processing ? " is-processing" : "") +
        (reducedRewardMotion() ? " is-reduced-motion" : "");
    const head = spending
        ? ""
        : "<div class='sow-menu__modal-head'><div><p class='sow-purchase-modal__eyebrow'>" +
          esc(eyebrow) +
          "</p><h2>" +
          heading +
          "</h2></div></div>";
    return (
        "<div class='sow-menu__overlay' data-menu-overlay='purchase'><section class='sow-menu__modal sow-purchase-modal" +
        successClass +
        itemClass +
        phaseClass +
        "' role='dialog' aria-modal='true' aria-live='polite' aria-busy='" +
        (spending || processing ? "true" : "false") +
        "' aria-label='" +
        esc(itemName) +
        "'><div class='sow-purchase-modal__layout'>" +
        renderPurchaseArt(leader, skin, bundle, storeBundle, success) +
        "<div class='sow-purchase-modal__copy'>" +
        head +
        price +
        error +
        body +
        "</div></div></section></div>"
    );
}

export function resolvePurchaseModal() {
    if (
        !menuUi.purchaseModal ||
        menuUi.purchaseModal.phase === "success" ||
        !menuUi.purchaseModal.submitted ||
        !menuUi.purchaseIntent
    )
        return;
    if (menuUi.purchaseModal.type === "bundle") {
        const bundleAttempt = currentExternalPurchaseAttempt();
        if (bundleAttempt && bundleAttempt.delivery_id && profileHasExternalPurchase(bundleAttempt)) {
            beginBundlePurchasePresentation(bundleAttempt);
        }
        return;
    }
    if (menuUi.purchaseModal.type === "product") {
        const attempt = currentExternalPurchaseAttempt();
        if (!attempt || !attempt.delivery_id || !profileHasExternalPurchase(attempt)) return;
        clearExternalPurchaseAttempt();
        menuUi.purchaseModal.phase = "spending";
        menuUi.purchaseModal.submitted = false;
        menuUi.purchaseIntent = null;
        if (menuUi.state) menuUi.state.error = null;
        beginPurchaseSuccessReveal();
        return;
    }
    if (purchaseItemOwned()) {
        if (menuUi.purchaseModal.type === "leader" && menuUi.purchaseModal.leaderId) {
            menuUi.tempSelectedLeader = menuUi.purchaseModal.leaderId;
            menuUi.tempSelectedRandom = false;
            stageLeaderSelection(false, menuUi.purchaseModal.leaderId);
            send("set_leader", {
                leader_id: menuUi.purchaseModal.leaderId,
                prefer_random: false,
            });
        }
        menuUi.storeCheckoutRequestId = null;
        menuUi.storeCheckoutBusy = false;
        trackStoreEvent(
            "store_purchase_complete",
            menuUi.purchaseModal.type,
            menuUi.purchaseModal.type === "skin"
                ? menuUi.purchaseModal.skinId
                : menuUi.purchaseModal.type === "store_bundle"
                  ? menuUi.purchaseModal.bundleOfferId
                  : menuUi.purchaseModal.leaderId,
            menuUi.purchaseModal.currency === "gems" ? "gems" : "crowns",
        );
        menuUi.purchaseModal.phase = "spending";
        menuUi.purchaseModal.submitted = false;
        menuUi.purchaseIntent = null;
        if (menuUi.state) menuUi.state.error = null;
        beginPurchaseSuccessReveal();
    } else if (menuUi.state && !menuUi.state.store_busy && menuUi.state.error) {
        menuUi.purchaseModal.submitted = false;
        menuUi.purchaseIntent = null;
    }
}

export function renderStore() {
    const store = menuUi.state!.store || {};
    const bundles = Array.isArray(store.gem_bundles) ? store.gem_bundles : [];
    const leaders = Array.isArray(store.leaders) ? store.leaders : [];
    const skins = Array.isArray(store.skins) ? store.skins : [];
    const crowns = store.crowns || 0;
    const gemsForSale = isAndroidTwa();
    const activeTab = menuUi.storeTab || "featured";
    const lockedLeadersCount = leaders.filter(function (offer) {
        return !offer.owned && !offer.permanently_free && Number(offer.cost_crowns) > 0;
    }).length;
    const unownedSkinsCount = skins.filter(function (skin) {
        return !skin.owned && Number(skin.cost_gems || skin.cost_crowns || 0) > 0;
    }).length;
    const featured = gemsForSale
        ? bundles.find(function (bundle) {
              return bundle.product_id === "sow_gems_2600";
          }) || bundles[bundles.length - 1]
        : null;
    const featuredAction = featured ? bundleBuyButton(featured) : "";
    const restoreAction =
        isAndroidTwa() &&
        menuUi.state!.account_id &&
        typeof window.SOW_requestAndroidRestore === "function" &&
        typeof window.SOW_isAndroidPurchaseBridgeReady === "function" &&
        window.SOW_isAndroidPurchaseBridgeReady()
            ? "<button class='sow-store__link' type='button' data-command='restore_purchases'>" +
              esc(SOW_t("store.restore_purchases")) +
              "</button>"
            : "";
    const tabsHtml =
        "<nav class='sow-store__tabs' aria-label='" +
        esc(SOW_t("store.shop")) +
        "'>" +
        "<button type='button' class='sow-store__tab" +
        (activeTab === "featured" ? " is-active" : "") +
        "' data-command='switch_store_tab' data-store-tab='featured'>" +
        esc(SOW_t("store.featured_offers")) +
        "</button>" +
        "<button type='button' class='sow-store__tab" +
        (activeTab === "leaders" ? " is-active" : "") +
        "' data-command='switch_store_tab' data-store-tab='leaders'>" +
        esc(SOW_t("store.leaders")) +
        (lockedLeadersCount > 0 ? " <b>" + esc(lockedLeadersCount) + "</b>" : "") +
        "</button>" +
        "<button type='button' class='sow-store__tab" +
        (activeTab === "skins" ? " is-active" : "") +
        "' data-command='switch_store_tab' data-store-tab='skins'>" +
        esc(SOW_t("store.skins")) +
        (unownedSkinsCount > 0 ? " <b>" + esc(unownedSkinsCount) + "</b>" : "") +
        "</button>" +
        "</nav>";

    const featuredGemsHtml =
        activeTab === "featured"
            ? (featured
                  ? "<article class='sow-store__featured'><div><p class='sow-store__eyebrow'>" +
                    esc(SOW_t("store.kingdom_vault")) +
                    "</p><h2>" +
                    renderCurrencyAmount(featured.gems, "gem") +
                    " " +
                    esc(SOW_t("store.gems")) +
                    "</h2><p>" +
                    esc(SOW_t("store.build_next_army")) +
                    "</p></div>" +
                    featuredAction +
                    "</article>"
                  : "") +
              (gemsForSale
                  ? "<section class='sow-store__section' aria-labelledby='sow-store-gems'><div class='sow-store__section-head'><h2 id='sow-store-gems'>" +
                    esc(SOW_t("store.gems")) +
                    "</h2><button class='sow-store__link' type='button' data-command='main_nav' data-nav-screen='heroes'>" +
                    esc(SOW_t("store.view_heroes")) +
                    "</button></div><div class='sow-store__bundle-grid'>" +
                    (bundles.map(renderStoreBundle).join("") ||
                        "<p class='sow-menu__empty'>" + esc(SOW_t("store.offers_unavailable")) + "</p>") +
                    "</div></section>"
                  : "")
            : "";

    const tabBodyHtml =
        activeTab === "leaders"
            ? renderBundlesShowcaseSection("leader") +
              renderFlashDealsSection("leader") +
              renderHeroSection(leaders)
            : activeTab === "skins"
              ? renderBundlesShowcaseSection("skin") +
                renderFlashDealsSection("skin") +
                renderSkinsSection(skins)
              : renderBundlesShowcaseSection() +
                renderFlashDealsSection() +
                featuredGemsHtml +
                renderHeroSection(leaders) +
                renderSkinsSection(skins);

    return (
        "<main class='sow-menu__main sow-menu__main--store' data-screen-panel='store'><section class='sow-menu__store-slot' data-store-slot aria-label='" +
        esc(SOW_t("store.shop")) +
        "'>" +
        "<header class='sow-store__heading'><div class='sow-store__heading-copy'><p class='sow-store__eyebrow'>" +
        esc(SOW_t("store.shop")) +
        "</p><h1>" +
        esc(SOW_t("store.featured_offers")) +
        "</h1></div><div class='sow-store__balances'><span class='sow-store__balance--gems' aria-label='" +
        esc(SOW_t("store.gems_count", { amount: store.gems || 0 })) +
        "'>" +
        renderCurrencyAmount(store.gems || 0, "gem") +
        "</span><span class='sow-store__balance--crowns' aria-label='" +
        esc(SOW_t("store.crowns_count", { amount: crowns })) +
        "'>" +
        renderCurrencyAmount(crowns, "crown") +
        "</span></div></header>" +
        tabsHtml +
        restoreAction +
        renderFeedback() +
        tabBodyHtml +
        "</section></main>"
    );
}

export function beginStorePurchase(productId: string | undefined) {
    if (!productId || menuUi.storeCheckoutBusy) return false;
    trackStoreEvent("store_buy_tap", "gem_pack", productId, "usd");
    if (!beginExternalPurchaseAttempt(productId)) return false;
    if (isAndroidTwa()) {
        if (!menuUi.state!.account_id || typeof window.SOW_requestAndroidPurchase !== "function") {
            abandonExternalPurchaseAttempt(productId);
            menuUi.state!.error = SOW_t("store.store_unavailable");
            render();
            return false;
        }
        menuUi.storeCheckoutBusy = true;
        const requestId = window.SOW_requestAndroidPurchase(productId, menuUi.state!.account_id);
        if (!requestId) {
            menuUi.storeCheckoutBusy = false;
            abandonExternalPurchaseAttempt(productId);
            menuUi.state!.error = SOW_t("store.android_checkout_unavailable");
            render();
        } else {
            menuUi.storeCheckoutRequestId = requestId;
            trackStoreEvent("store_checkout_open", "gem_pack", productId, "usd");
            render();
        }
        return !!requestId;
    }
    // Only the Android app sells gem packs; there is no web checkout.
    abandonExternalPurchaseAttempt(productId);
    menuUi.state!.error = SOW_t("store.store_unavailable");
    render();
    return false;
}

export function beginStoreRestore() {
    if (
        !isAndroidTwa() ||
        menuUi.storeCheckoutBusy ||
        !menuUi.state!.account_id ||
        typeof window.SOW_requestAndroidRestore !== "function"
    )
        return;
    menuUi.storeCheckoutBusy = true;
    const requestId = window.SOW_requestAndroidRestore(menuUi.state!.account_id);
    if (!requestId) {
        menuUi.storeCheckoutBusy = false;
        menuUi.state!.error = SOW_t("store.android_restore_unavailable");
    } else {
        menuUi.storeCheckoutRequestId = requestId;
    }
    render();
}

export function loadStoreCatalog() {
    const state = menuUi.state;
    if (!state) return;
    const expectedPeriod = Number(state.store && state.store.rotation_period) || 0;
    const nowSecs = Date.now() / 1000;
    const offersExpired = (Array.isArray(state.store && state.store.offers) ? state.store.offers! : []).some(
        function (offer) {
            return Number(offer.ends_at) <= nowSecs;
        },
    );
    const bundlesExpired = (Array.isArray(state.store && state.store.bundles) ? state.store.bundles! : []).some(
        function (bundle) {
            return Number(bundle.ends_at) <= nowSecs;
        },
    );
    if (
        menuUi.storeCatalogLoading ||
        (menuUi.storeCatalogLoaded &&
            loadedRotationPeriod >= expectedPeriod &&
            !offersExpired &&
            !bundlesExpired) ||
        Date.now() < menuUi.storeCatalogRetryAt
    )
        return;
    menuUi.storeCatalogLoading = true;
    fetch(profileApi("/store/catalog"), { headers: { Accept: "application/json" } })
        .then(function (response) {
            if (!response.ok) throw new Error("catalog unavailable");
            return response.json();
        })
        .then(function (catalog: MenuStore) {
            if (Number(catalog.rotation_period) < expectedPeriod || catalog.free_leaders.length !== 8)
                throw new Error("stale or invalid hero rotation");
            const current = menuUi.state!.store || {};
            const localLeaders = current.leaders || [];
            const localSkins = current.skins || [];
            catalog.leaders = (catalog.leaders || []).map(function (leader) {
                const local = localLeaders.find(function (item) {
                    return (
                        String(item.id)
                            .replace(/[^a-z0-9]/gi, "")
                            .toLowerCase() ===
                        String(leader.id)
                            .replace(/[^a-z0-9]/gi, "")
                            .toLowerCase()
                    );
                });
                const owned = local ? local.owned : leader.owned;
                return Object.assign({}, leader, {
                    owned: owned,
                    available: leader.permanently_free || leader.free_rotation || owned,
                });
            });
            catalog.skins = (catalog.skins || []).map(function (skin) {
                const local = localSkins.find(function (item) {
                    return item.id === skin.id;
                });
                return Object.assign({}, skin, local && { owned: local.owned });
            });
            menuUi.state!.store = Object.assign({}, current, catalog, {
                gems: current.gems == null ? 0 : current.gems,
                crowns: current.crowns == null ? 0 : current.crowns,
            });
            const offers = new Map(
                catalog.leaders.map(function (leader) {
                    return [
                        String(leader.id).replace(/[^a-z0-9]/gi, "").toLowerCase(),
                        leader,
                    ] as const;
                }),
            );
            menuUi.state!.leaders = menuUi.state!.leaders.map(function (leader) {
                const offer = offers.get(
                    String(leader.id).replace(/[^a-z0-9]/gi, "").toLowerCase(),
                );
                return offer
                    ? Object.assign({}, leader, {
                          free_rotation: offer.free_rotation,
                          permanently_free: offer.permanently_free,
                          owned: offer.owned,
                          available: offer.available,
                      })
                    : leader;
            });
            loadedRotationPeriod = Number(catalog.rotation_period);
            send("sync_free_rotation", {
                rotation_period: loadedRotationPeriod,
                free_leaders: catalog.free_leaders,
            });
            menuUi.storeCatalogLoaded = true;
        })
        .catch(function (error) {
            menuUi.storeCatalogLoaded = false;
            menuUi.storeCatalogRetryAt = Date.now() + 2000;
            console.warn("[store] catalog unavailable", error);
        })
        .finally(function () {
            menuUi.storeCatalogLoading = false;
            render();
        });
}
