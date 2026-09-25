// Store state, rendering, catalog, and purchase actions.

    function storeAuth() {
        var creds = selfCreds();
        var headers = { "Content-Type": "application/json", "Accept": "application/json" };
        var token = null;
        var identity = window.SOW_PLATFORM_IDENTITY;
        if (identity && identity.token) token = identity.token;
        if (!token) {
            try { token = window.localStorage.getItem("wou_session_token"); } catch (e) {}
        }
        if (token) {
            headers["X-Platform-Auth"] = token;
            headers["X-Platform-Provider"] = identity && identity.provider ? identity.provider : "wou";
        }
        return { available: !!creds || !!token, headers: headers, authSecret: creds ? creds.auth_secret : "" };
    }

    function isAndroidTwa() {
        var referrer = String(document.referrer || "");
        if (/^android-app:\/\/com\.shadowsofwar(?:\/|$)/i.test(referrer)) return true;
        try {
            return new URLSearchParams(window.location.search).get("sow_platform") === "android" &&
                /Android/i.test(navigator.userAgent || "");
        } catch (e) {
            return false;
        }
    }

    function renderCurrencyAmount(amount, kind, amountClass) {
        var className = amountClass ? " class='" + esc(amountClass) + "'" : "";
        return "<span" + className + ">" + esc(amount) + "</span> <img class='sow-store__currency-icon' src='" + esc(currencyAsset(kind)) + "' alt='' aria-hidden='true'>";
    }

    function storeBalance(currency) {
        return Math.max(0, Number(state && state[currency]) || 0);
    }

    var EXTERNAL_PURCHASE_POLL_MS = 3000;
    var EXTERNAL_PURCHASE_WAIT_MS = 120000;
    var externalPurchaseAttempt = null;
    var externalPurchasePollTimer = null;
    var externalPurchasePollInFlight = false;
    var externalPurchaseRevealTimer = null;
    var purchaseSuccessTimer = null;
    var bundleFlightToken = null;

    function storeBundleByProductId(productId) {
        return (state && state.store && state.store.gem_bundles || []).find(function (bundle) {
            return String(bundle.product_id) === String(productId);
        }) || null;
    }

    function externalPurchaseStorageKey(accountId) {
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
                JSON.stringify(externalPurchaseAttempt)
            );
        } catch (error) {}
    }

    function currentExternalPurchaseAttempt() {
        if (!state || !state.account_id) return null;
        if (externalPurchaseAttempt && externalPurchaseAttempt.account_id === state.account_id) return externalPurchaseAttempt;
        clearExternalPurchasePoll();
        externalPurchaseAttempt = null;
        try {
            var saved = JSON.parse(window.localStorage.getItem(externalPurchaseStorageKey(state.account_id)) || "null");
            if (saved && saved.account_id === state.account_id && saved.product_id &&
                (saved.kind === "bundle" || saved.kind === "leader" || saved.kind === "skin")) {
                externalPurchaseAttempt = saved;
            }
        } catch (error) {}
        return externalPurchaseAttempt;
    }

    function clearExternalPurchaseAttempt() {
        var attempt = currentExternalPurchaseAttempt();
        clearExternalPurchasePoll();
        if (attempt) {
            try { window.localStorage.removeItem(externalPurchaseStorageKey(attempt.account_id)); } catch (error) {}
        }
        externalPurchaseAttempt = null;
    }

    function externalPurchaseDetails(productId) {
        var bundle = storeBundleByProductId(productId);
        if (bundle) return { kind: "bundle", itemId: bundle.id, gems: Number(bundle.gems) || 0 };
        var intent = purchaseIntent || {};
        if (intent.productId !== productId) return null;
        if (intent.leaderId) return { kind: "leader", itemId: intent.leaderId, gems: 0 };
        if (intent.skinId) return { kind: "skin", itemId: intent.skinId, gems: 0 };
        return null;
    }

    function beginExternalPurchaseAttempt(productId) {
        if (!state || !state.account_id) return null;
        var details = externalPurchaseDetails(productId);
        if (!details) return null;
        externalPurchaseAttempt = {
            account_id: state.account_id,
            product_id: productId,
            kind: details.kind,
            item_id: details.itemId,
            gems: details.gems,
            base_gems: Math.max(0, Number(state.gems) || 0),
            started_at: Date.now(),
            deadline_at: Date.now() + EXTERNAL_PURCHASE_WAIT_MS,
            provider_complete: false,
            delivery_id: "",
            timed_out: false,
            dismissed: false
        };
        saveExternalPurchaseAttempt();
        return externalPurchaseAttempt;
    }

    function requestExternalPurchaseProfile(attempt) {
        if (!attempt || !state || state.account_id !== attempt.account_id) return;
        send("refresh_profile");
    }

    function profileHasExternalPurchase(attempt) {
        if (!attempt || !state || state.account_id !== attempt.account_id) return false;
        if (attempt.kind === "bundle") return Number(state.gems) > Number(attempt.base_gems || 0);
        if (attempt.kind === "leader") {
            var leader = storeLeaderById(attempt.item_id);
            return !!leader && !!leader.owned;
        }
        var skin = storeSkinById(attempt.item_id);
        return !!skin && !!skin.owned;
    }

    function purchaseIntentForAttempt(attempt) {
        if (!attempt) return null;
        if (attempt.kind === "leader") return { type: "leader", leaderId: attempt.item_id, productId: attempt.product_id };
        if (attempt.kind === "skin") return { type: "skin", skinId: attempt.item_id, productId: attempt.product_id };
        return { type: "bundle", productId: attempt.product_id };
    }

    function showExternalPurchaseProcessing(attempt) {
        if (!attempt || attempt.dismissed || !attempt.provider_complete) return;
        purchaseIntent = purchaseIntentForAttempt(attempt);
        purchaseModal = {
            type: attempt.kind === "bundle" ? "bundle" : "product",
            productId: attempt.product_id,
            leaderId: attempt.kind === "leader" ? attempt.item_id : "",
            skinId: attempt.kind === "skin" ? attempt.item_id : "",
            bundleId: attempt.kind === "bundle" ? attempt.item_id : "",
            phase: "processing",
            submitted: true,
            timedOut: !!attempt.timed_out
        };
    }

    function purchaseHistory() {
        var auth = storeAuth();
        if (!auth.available || !state || !state.account_id) return Promise.reject(new Error("store unavailable"));
        return fetch(profileApi("/store/purchases"), {
            method: "POST",
            headers: auth.headers,
            body: JSON.stringify({ account_id: state.account_id, auth_secret: auth.authSecret })
        }).then(function (response) {
            if (!response.ok) throw new Error("purchase history unavailable");
            return response.json();
        });
    }

    function deliveredPurchaseRecord(records, attempt) {
        var startedAt = Number(attempt && attempt.started_at) || 0;
        return (Array.isArray(records) ? records : []).find(function (record) {
            return record && record.status === "granted" && record.product_id === attempt.product_id &&
                Number(record.updated_at || 0) * 1000 >= startedAt - 1000;
        }) || null;
    }

    function scheduleExternalPurchasePoll(delay) {
        var attempt = currentExternalPurchaseAttempt();
        if (!attempt || externalPurchasePollTimer !== null || attempt.timed_out) return;
        externalPurchasePollTimer = window.setTimeout(function () {
            externalPurchasePollTimer = null;
            pollExternalPurchaseDelivery();
        }, delay == null ? EXTERNAL_PURCHASE_POLL_MS : delay);
    }

    function timeOutExternalPurchase(attempt) {
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
        var attempt = currentExternalPurchaseAttempt();
        if (!attempt || externalPurchasePollInFlight || !state || state.account_id !== attempt.account_id) return;
        if (Date.now() >= Number(attempt.deadline_at || 0)) {
            timeOutExternalPurchase(attempt);
            return;
        }
        externalPurchasePollInFlight = true;
        purchaseHistory().then(function (records) {
            var delivered = deliveredPurchaseRecord(records, attempt);
            if (delivered) {
                attempt.delivery_id = delivered.id;
                attempt.provider_complete = true;
                attempt.dismissed = false;
                saveExternalPurchaseAttempt();
                showExternalPurchaseProcessing(attempt);
                requestExternalPurchaseProfile(attempt);
                render();
            }
        }).catch(function () {}).finally(function () {
            externalPurchasePollInFlight = false;
            var latest = currentExternalPurchaseAttempt();
            if (!latest || latest !== attempt) return;
            if (Date.now() >= Number(latest.deadline_at || 0)) timeOutExternalPurchase(latest);
            else scheduleExternalPurchasePoll();
        });
    }

    function beginBundlePurchasePresentation(attempt) {
        if (!attempt || !state || !profileHasExternalPurchase(attempt)) return;
        clearExternalPurchaseAttempt();
        purchaseIntent = null;
        purchaseModal = {
            type: "bundle",
            productId: attempt.product_id,
            bundleId: attempt.item_id,
            gems: attempt.gems,
            phase: "success",
            submitted: false,
            gemBaseline: Math.max(0, Number(attempt.base_gems) || 0),
            gemPresented: false,
            gemFlightStarted: false
        };
        render();
        if (externalPurchaseRevealTimer !== null) window.clearTimeout(externalPurchaseRevealTimer);
        externalPurchaseRevealTimer = window.setTimeout(function () {
            externalPurchaseRevealTimer = null;
            if (!purchaseModal || purchaseModal.type !== "bundle" || purchaseModal.phase !== "success" || purchaseModal.gemFlightStarted) return;
            purchaseModal.gemFlightStarted = true;
            var finalValues = progressionFromState();
            var startValues = Object.assign({}, finalValues, { gems: purchaseModal.gemBaseline });
            writeProgression(startValues);
            if (reducedRewardMotion()) {
                purchaseModal.gemPresented = true;
                writeProgression(finalValues);
                return;
            }
            var token = root.querySelector(".sow-purchase-modal__bundle-token");
            var animationToken = rewardAnimationToken;
            if (!token) {
                purchaseModal.gemPresented = true;
                animateProgressionTo(finalValues, 640, animationToken).then(function () {
                    pulseRewardCounter("[data-progression-gems-value]", false);
                });
                return;
            }
            flyBundleToken(token, animationToken).then(function (arrived) {
                if (!arrived || !purchaseModal || purchaseModal.type !== "bundle") return;
                animateProgressionTo(finalValues, 640, animationToken).then(function (shown) {
                    if (!shown || !purchaseModal || purchaseModal.type !== "bundle") return;
                    purchaseModal.gemPresented = true;
                    pulseRewardCounter("[data-progression-gems-value]", false);
                });
            });
        }, 760);
    }

    function settleBundlePurchasePresentation() {
        if (externalPurchaseRevealTimer !== null) window.clearTimeout(externalPurchaseRevealTimer);
        externalPurchaseRevealTimer = null;
        if (bundleFlightToken) {
            if (typeof bundleFlightToken.getAnimations === "function") bundleFlightToken.getAnimations().forEach(function (animation) { animation.cancel(); });
            bundleFlightToken.remove();
            bundleFlightToken = null;
        }
        if (purchaseModal && purchaseModal.type === "bundle") purchaseModal.gemPresented = true;
        if (state) writeProgression(progressionFromState());
    }

    function flyBundleToken(token, animationToken) {
        var rect = token.getBoundingClientRect();
        var flyer = token.cloneNode(true);
        flyer.className += " sow-purchase-modal__bundle-flyer";
        flyer.style.cssText = "position:fixed;z-index:30;left:" + rect.left + "px;top:" + rect.top + "px;width:" + rect.width + "px;height:" + rect.height + "px;margin:0;pointer-events:none;";
        document.body.appendChild(flyer);
        bundleFlightToken = flyer;
        return flyRewardCard({ card: flyer, stage: { selector: "[data-progression-gems-value]" } }, animationToken).then(function (arrived) {
            if (bundleFlightToken === flyer) bundleFlightToken = null;
            flyer.remove();
            return arrived;
        });
    }

    function beginPurchaseSuccessReveal() {
        if (!purchaseModal || purchaseModal.phase !== "spending") return;
        if (reducedRewardMotion()) {
            purchaseModal.phase = "success";
            render();
            return;
        }
        if (purchaseSuccessTimer !== null) window.clearTimeout(purchaseSuccessTimer);
        purchaseSuccessTimer = window.setTimeout(function () {
            purchaseSuccessTimer = null;
            if (!purchaseModal || purchaseModal.phase !== "spending") return;
            purchaseModal.phase = "success";
            render();
        }, 460);
    }

    function dismissPurchaseModal() {
        if (purchaseModal && purchaseModal.type === "bundle") settleBundlePurchasePresentation();
        if (purchaseModal && purchaseModal.phase === "processing") {
            var attempt = currentExternalPurchaseAttempt();
            if (attempt && attempt.timed_out) {
                attempt.dismissed = true;
                saveExternalPurchaseAttempt();
            }
        }
        if (purchaseSuccessTimer !== null) window.clearTimeout(purchaseSuccessTimer);
        purchaseSuccessTimer = null;
        purchaseModal = null;
        purchaseIntent = null;
    }

    function canDismissPurchaseModal() {
        return !purchaseModal || purchaseModal.phase !== "processing" || purchaseModal.timedOut;
    }

    function abandonExternalPurchaseAttempt(productId) {
        var attempt = currentExternalPurchaseAttempt();
        if (productId && (!attempt || attempt.product_id !== productId)) return;
        clearExternalPurchaseAttempt();
    }

    function completeExternalPurchase(productId) {
        if (purchaseModal && purchaseModal.productId === productId &&
            (purchaseModal.phase === "spending" || purchaseModal.phase === "success")) return;
        var attempt = currentExternalPurchaseAttempt();
        if (!attempt || attempt.product_id !== productId) attempt = beginExternalPurchaseAttempt(productId);
        if (!attempt) return;
        if (storeCheckoutProduct === productId) {
            if (storeCheckoutInstance && typeof storeCheckoutInstance.destroy === "function") storeCheckoutInstance.destroy();
            storeCheckoutInstance = null;
            storeCheckoutProduct = null;
            storeCheckoutRequestId = null;
            storeCheckoutBusy = false;
        }
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

    function resumeExternalPurchaseDelivery(fromReturn) {
        var attempt = currentExternalPurchaseAttempt();
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
        if ((attempt.provider_complete || attempt.delivery_id) && !attempt.dismissed) showExternalPurchaseProcessing(attempt);
        scheduleExternalPurchasePoll(0);
    }

    function storeProductButton(productId, label, confirm, priceLabel, leaderId, skinId) {
        var command = confirm ? "open_product_purchase" : "buy_product";
        var price = priceLabel ? " data-price-label='" + esc(priceLabel) + "'" : "";
        var leader = leaderId ? " data-leader-id='" + esc(leaderId) + "'" : "";
        var skin = skinId ? " data-skin-id='" + esc(skinId) + "'" : "";
        return "<button class='sow-store__buy sow-store__buy--primary' type='button' data-command='" + command + "' data-product-id='" + esc(productId) + "'" + price + leader + skin + ">" + esc(label || SOW_t("store.buy")) + "</button>";
    }

    function canRenderDirectPurchase(productId) {
        if (!productId) return false;
        return !isAndroidTwa() || (typeof window.SOW_androidPurchaseSupports === "function" && window.SOW_androidPurchaseSupports(productId));
    }

    function canRenderProductPurchase(productId) {
        if (!isAndroidTwa()) return !!(state.store && state.store.web_checkout_available);
        return canRenderDirectPurchase(productId);
    }

    function storeLeaderById(id) {
        var normalize = function (value) { return String(value || "").replace(/[^a-z0-9]/gi, "").toLowerCase(); };
        var target = normalize(id);
        return (state.store && state.store.leaders || []).find(function (leader) { return normalize(leader.id) === target; }) || null;
    }

    function storeSkinById(id) {
        return (state.store && state.store.skins || []).find(function (skin) { return String(skin.id) === String(id); }) || null;
    }

    function renderLeaderCurrencyButton(offer, currency) {
        var isGems = currency === "gems";
        var cost = Math.max(0, Number(isGems ? offer.cost_gems : offer.cost_crowns) || 0);
        var insufficient = storeBalance(currency) < cost;
        var disabled = insufficient || (state && state.store_busy) ? " disabled" : "";
        var amountClass = insufficient ? "sow-store__currency-amount--insufficient" : "";
        return "<button class='sow-store__buy" + (insufficient ? " sow-store__buy--insufficient" : "") + "' type='button' data-command='unlock_leader' data-currency='" + currency + "' data-leader-id='" + esc(offer.id) + "'" + disabled + ">" + esc(SOW_t("store.buy")) + " " + renderCurrencyAmount(cost, isGems ? "gem" : "crown", amountClass) + "</button>";
    }

    function renderLeaderPurchase(leader) {
        var offer = storeLeaderById(leader.id);
        var leaderActions = !offer || offer.owned ? "" :
            renderLeaderCurrencyButton(offer, "crowns") +
            renderLeaderCurrencyButton(offer, "gems") +
            (canRenderDirectPurchase(offer.direct_product_id) ? storeProductButton(offer.direct_product_id, SOW_t("store.buy"), true, offer.direct_price_label, offer.id) : "");
        return "<div class='sow-heroes__purchase-actions'>" +
            leaderActions +
            "<button class='sow-store__buy sow-store__buy--primary' type='button' data-command='open_skin_picker'>" + esc(SOW_t("store.skins")) + "</button>" +
            "</div>";
    }

    function renderSkinPickerModal() {
        if (!skinPickerOpen) return "";
        var skins = Array.isArray(state.store && state.store.skins) ? state.store.skins : [];
        return "<div class='sow-menu__overlay' data-menu-overlay='skin-picker'><section class='sow-menu__modal sow-skin-picker' role='dialog' aria-modal='true' aria-label='" + esc(SOW_t("store.skins")) + "'><div class='sow-menu__modal-head'><h2>" + esc(SOW_t("store.skins")) + "</h2><button class='sow-menu__icon-button' type='button' data-command='close_skin_picker' aria-label='" + esc(SOW_t("menu.close")) + "'>×</button></div><div class='sow-store__skin-grid'>" + (skins.map(renderStoreSkinPromo).join("") || "<p class='sow-menu__empty'>" + esc(SOW_t("store.offers_unavailable")) + "</p>") + "</div></section></div>";
    }

    function openLeaderPurchase(leaderId, currency) {
        var offer = storeLeaderById(leaderId);
        var isGems = currency === "gems";
        var cost = offer && (isGems ? offer.cost_gems : offer.cost_crowns);
        if (!offer || offer.owned || (state && state.store_busy) ||
            storeBalance(currency) < Number(cost || 0)) return;
        state.error = null;
        purchaseIntent = { type: "leader", leaderId: offer.id };
        purchaseModal = { type: "leader", leaderId: offer.id, currency: currency, phase: "confirm", submitted: false };
        render();
    }

    function openSkinPurchase(skinId) {
        var skin = storeSkinById(skinId);
        if (!skin || skin.owned || (state && state.store_busy) ||
            storeBalance("gems") < Number(skin.cost_gems || 0)) return;
        state.error = null;
        purchaseIntent = { type: "skin", skinId: skin.id };
        purchaseModal = { type: "skin", skinId: skin.id, currency: "gems", phase: "confirm", submitted: false };
        render();
    }

    function openProductPurchase(productId, leaderId, priceLabel, skinId) {
        if (!productId || storeCheckoutBusy || (state && state.store_busy)) return;
        state.error = null;
        purchaseIntent = leaderId ? { type: "leader", leaderId: leaderId, productId: productId } : { type: "skin", skinId: skinId, productId: productId };
        purchaseModal = { type: "product", productId: productId, leaderId: leaderId, skinId: skinId, priceLabel: priceLabel || "", phase: "confirm", submitted: false };
        render();
    }

    function renderStoreBundle(bundle) {
        var action = canRenderProductPurchase(bundle.product_id) ? storeProductButton(bundle.product_id, SOW_t("store.buy")) : "";
        var art = bundle.asset_path ? asset(bundle.asset_path) : asset("gameplay/store/gem_bundles/" + bundle.id + ".webp");
        return "<article class='sow-store__bundle'><img class='sow-store__bundle-icon' src='" + esc(art) + "' alt='' aria-hidden='true' width='56' height='56' loading='lazy'><div><strong class='sow-store__bundle-price' aria-label='" + esc(SOW_t("store.gems_count", { amount: bundle.gems })) + "'>" + renderCurrencyAmount(bundle.gems, "gem") + "</strong></div>" + action + "</article>";
    }

    function renderStoreSkinPromo(skin) {
        var action;
        var cost = Math.max(0, Number(skin.cost_gems) || 0);
        var insufficient = storeBalance("gems") < cost;
        var disabled = insufficient || (state && state.store_busy) ? " disabled" : "";
        var busyDisabled = state && state.store_busy ? " disabled" : "";
        if (skin.owned) {
            action = state.selected_skin === skin.id
                ? "<span class='sow-store__offer-state'>" + esc(SOW_t("store.equipped")) + "</span>"
                : "<button class='sow-store__buy' type='button' data-command='equip_skin' data-skin-id='" + esc(skin.id) + "'" + busyDisabled + ">" + esc(SOW_t("store.equip")) + "</button>";
        } else {
            var amountClass = insufficient ? "sow-store__currency-amount--insufficient" : "";
            action = "<div class='sow-store__buy-row'><button class='sow-store__buy" + (insufficient ? " sow-store__buy--insufficient" : "") + "' type='button' data-command='open_skin_purchase' data-skin-id='" + esc(skin.id) + "'" + disabled + ">" + esc(SOW_t("store.buy")) + " " + renderCurrencyAmount(cost, "gem", amountClass) + "</button>" +
                (canRenderDirectPurchase(skin.direct_product_id) ? storeProductButton(skin.direct_product_id, SOW_t("store.buy"), true, skin.direct_price_label, null, skin.id) : "") + "</div>";
        }
        return "<article class='sow-store__skin'><div class='sow-store__skin-art'><img src='" + esc(asset(skin.asset_path)) + "' alt='' width='96' height='96' loading='lazy'></div><div class='sow-store__skin-body'><h3>" + esc(skin.name) + "</h3><p>" + esc(SOW_t("store.territory_pattern")) + "</p>" + action + "</div></article>";
    }

    function renderStoreCheckout() {
        return storeCheckoutProduct
            ? "<section class='sow-store__checkout' data-store-checkout><div class='sow-store__section-head'><h2>" + esc(SOW_t("store.checkout")) + "</h2><button class='sow-store__close' type='button' data-command='close_store_checkout' aria-label='" + esc(SOW_t("menu.close")) + "'>×</button></div><div class='sow-store__checkout-host' data-store-checkout-host><p>" + esc(SOW_t("store.loading_checkout")) + "</p></div></section>"
            : "";
    }

    function purchaseItemOwned() {
        if (!purchaseIntent) return false;
        if (purchaseIntent.skinId) {
            var skin = storeSkinById(purchaseIntent.skinId);
            return !!skin && !!skin.owned;
        }
        var offer = storeLeaderById(purchaseIntent.leaderId);
        return !!offer && !!offer.owned;
    }

    function renderPurchaseArt(leader, skin, bundle, success) {
        var visualClass = "";
        var art;
        if (bundle) {
            var bundleArt = bundle.asset_path ? asset(bundle.asset_path) : asset("gameplay/store/gem_bundles/" + bundle.id + ".webp");
            visualClass = " sow-purchase-modal__visual--bundle";
            art = "<img class='sow-purchase-modal__art-image sow-purchase-modal__art-image--bundle sow-purchase-modal__bundle-token' src='" + esc(bundleArt) + "' alt='' width='128' height='128'>";
        } else if (skin) {
            visualClass = " sow-purchase-modal__visual--skin";
            art = "<img class='sow-purchase-modal__art-image sow-purchase-modal__art-image--skin' src='" + esc(asset(skin.asset_path)) + "' alt='" + esc(skin.name) + "' width='256' height='256'>";
        } else {
            art = "<picture><source media='(max-width: 700px) and (orientation: portrait)' srcset='" + esc(asset("shell/leaders/" + leader.slug + "_mobile.webp")) + "'><img class='sow-purchase-modal__art-image' src='" + esc(asset("shell/leaders/" + leader.slug + "_desktop.webp")) + "' alt='" + esc(leaderDisplayName(leader)) + "' width='1080' height='1920'></picture>";
        }
        var effects = success
            ? "<div class='sow-purchase-modal__effects' aria-hidden='true'><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i></div>"
            : "";
        return "<div class='sow-purchase-modal__visual" + visualClass + "'>" + art + "</div>" + effects;
    }

    function renderPurchaseModal() {
        if (!purchaseModal) return "";
        var leader = purchaseModal.leaderId ? leaderById(purchaseModal.leaderId) : null;
        var offer = purchaseModal.leaderId ? storeLeaderById(purchaseModal.leaderId) : null;
        var skin = purchaseModal.skinId ? storeSkinById(purchaseModal.skinId) : null;
        var bundle = purchaseModal.type === "bundle" ? storeBundleByProductId(purchaseModal.productId) : null;
        if ((purchaseModal.leaderId && (!leader || !offer)) || (purchaseModal.skinId && !skin) || (purchaseModal.type === "bundle" && !bundle)) return "";
        var isProduct = purchaseModal.type === "product";
        var success = purchaseModal.phase === "success";
        var spending = purchaseModal.phase === "spending";
        var processing = purchaseModal.phase === "processing";
        var checkout = purchaseModal.phase === "confirm" && isProduct && storeCheckoutProduct === purchaseModal.productId;
        var busy = purchaseModal.submitted || storeCheckoutBusy || (state && state.store_busy);
        var itemName = bundle ? SOW_t("store.gems_count", { amount: bundle.gems }) : (skin ? skin.name : leaderDisplayName(leader));
        var heading = bundle
            ? "<span class='sow-purchase-modal__bundle-amount'>" + renderCurrencyAmount(bundle.gems, "gem") + "</span>"
            : esc(itemName);
        var summary = bundle ? "" : isProduct
            ? (purchaseModal.priceLabel || SOW_t("store.buy"))
            : renderCurrencyAmount(
                purchaseModal.currency === "gems" ? (skin ? skin.cost_gems : offer.cost_gems) : offer.cost_crowns,
                purchaseModal.currency === "gems" ? "gem" : "crown"
            );
        var error = !success && !spending && !processing && state && state.error ? "<div class='sow-menu__status sow-menu__status--error' role='alert'>" + esc(localizedText(state.error)) + "</div>" : "";
        var body = success
            ? "<div class='sow-menu__modal-actions'><button class='sow-menu__primary' type='button' data-command='cancel_purchase'>" + esc(SOW_t("tutorial.continue")) + "</button></div>"
            : processing
                ? (purchaseModal.timedOut
                    ? "<div class='sow-menu__modal-actions'><button class='sow-menu__primary' type='button' data-command='cancel_purchase'>" + esc(SOW_t("tutorial.continue")) + "</button></div>"
                    : "<div class='sow-purchase-modal__pending' aria-busy='true' aria-hidden='true'><i></i></div>")
                : spending
                    ? "<div class='sow-purchase-modal__spend' aria-hidden='true'><i></i><i></i><i></i></div>"
            : checkout
                ? renderStoreCheckout()
                : "<div class='sow-menu__modal-actions'><button class='sow-menu__ghost-button' type='button' data-command='cancel_purchase'>" + esc(SOW_t("lobbies.cancel")) + "</button><button class='sow-menu__primary' type='button' data-command='confirm_purchase'" + (busy ? " disabled" : "") + ">" + esc(SOW_t("store.buy")) + "</button></div>";
        var eyebrow = success ? SOW_t("store.unlocked") : SOW_t("store.purchase");
        var price = success || spending || processing || !summary ? "" : "<strong class='sow-purchase-modal__price'>" + summary + "</strong>";
        var successClass = success ? " is-success" : "";
        var phaseClass = (spending ? " is-spending" : (processing ? " is-processing" : "")) + (reducedRewardMotion() ? " is-reduced-motion" : "");
        var head = spending ? "" : "<div class='sow-menu__modal-head'><div><p class='sow-purchase-modal__eyebrow'>" + esc(eyebrow) + "</p><h2>" + heading + "</h2></div></div>";
        return "<div class='sow-menu__overlay' data-menu-overlay='purchase'><section class='sow-menu__modal sow-purchase-modal" + successClass + phaseClass + "' role='dialog' aria-modal='true' aria-live='polite' aria-busy='" + (spending || processing ? "true" : "false") + "' aria-label='" + esc(itemName) + "'><div class='sow-purchase-modal__layout'>" + renderPurchaseArt(leader, skin, bundle, success) + "<div class='sow-purchase-modal__copy'>" + head + price + error + body + "</div></div></section></div>";
    }

    function resolvePurchaseModal() {
        if (!purchaseModal || purchaseModal.phase === "success" || !purchaseModal.submitted || !purchaseIntent) return;
        if (purchaseModal.type === "bundle") {
            var bundleAttempt = currentExternalPurchaseAttempt();
            if (bundleAttempt && bundleAttempt.delivery_id && profileHasExternalPurchase(bundleAttempt)) {
                beginBundlePurchasePresentation(bundleAttempt);
            }
            return;
        }
        if (purchaseModal.type === "product") {
            var attempt = currentExternalPurchaseAttempt();
            if (!attempt || !attempt.delivery_id || !profileHasExternalPurchase(attempt)) return;
            clearExternalPurchaseAttempt();
            purchaseModal.phase = "spending";
            purchaseModal.submitted = false;
            purchaseIntent = null;
            if (state) state.error = null;
            beginPurchaseSuccessReveal();
            return;
        }
        if (purchaseItemOwned()) {
            if (storeCheckoutInstance && typeof storeCheckoutInstance.destroy === "function") storeCheckoutInstance.destroy();
            storeCheckoutInstance = null;
            storeCheckoutProduct = null;
            storeCheckoutRequestId = null;
            storeCheckoutBusy = false;
            purchaseModal.phase = "spending";
            purchaseModal.submitted = false;
            purchaseIntent = null;
            if (state) state.error = null;
            beginPurchaseSuccessReveal();
        } else if (state && !state.store_busy && state.error && purchaseModal.type !== "product") {
            purchaseModal.submitted = false;
            purchaseIntent = null;
        }
    }

    function renderStore() {
        var store = state.store || {};
        var bundles = Array.isArray(store.gem_bundles) ? store.gem_bundles : [];
        var skins = Array.isArray(store.skins) ? store.skins : [];
        var crowns = store.crowns || 0;
        var featured = bundles.find(function (bundle) { return bundle.product_id === "sow_gems_2600"; }) || bundles[bundles.length - 1];
        var featuredAction = featured && canRenderProductPurchase(featured.product_id) ? storeProductButton(featured.product_id, SOW_t("store.buy")) : "";
        var restoreAction = isAndroidTwa() && state.account_id &&
            typeof window.SOW_requestAndroidRestore === "function" &&
            typeof window.SOW_isAndroidPurchaseBridgeReady === "function" &&
            window.SOW_isAndroidPurchaseBridgeReady()
            ? "<button class='sow-store__link' type='button' data-command='restore_purchases'>" + esc(SOW_t("store.restore_purchases")) + "</button>"
            : "";
        var checkout = renderStoreCheckout();
        return "<main class='sow-menu__main sow-menu__main--store' data-screen-panel='store'><section class='sow-menu__store-slot' data-store-slot aria-label='" + esc(SOW_t("store.shop")) + "'>" +
                    "<header class='sow-store__heading'><div><p class='sow-store__eyebrow'>" + esc(SOW_t("store.shop")) + "</p><h1>" + esc(SOW_t("store.featured_offers")) + "</h1></div><div class='sow-store__balances'><span class='sow-store__balance--gems' aria-label='" + esc(SOW_t("store.gems_count", { amount: store.gems || 0 })) + "'>" + renderCurrencyAmount(store.gems || 0, "gem") + "</span><span class='sow-store__balance--crowns' aria-label='" + esc(SOW_t("store.crowns_count", { amount: crowns })) + "'>" + renderCurrencyAmount(crowns, "crown") + "</span></div></header>" +
                    restoreAction +
                    renderFeedback() +
                    (featured ? "<article class='sow-store__featured'><div><p class='sow-store__eyebrow'>" + esc(SOW_t("store.kingdom_vault")) + "</p><h2>" + renderCurrencyAmount(featured.gems, "gem") + " " + esc(SOW_t("store.gems")) + "</h2><p>" + esc(SOW_t("store.build_next_army")) + "</p></div>" + featuredAction + "</article>" : "") +
                    "<section class='sow-store__section' aria-labelledby='sow-store-gems'><div class='sow-store__section-head'><h2 id='sow-store-gems'>" + esc(SOW_t("store.gems")) + "</h2><button class='sow-store__link' type='button' data-command='main_nav' data-nav-screen='heroes'>" + esc(SOW_t("store.view_heroes")) + "</button></div><div class='sow-store__bundle-grid'>" + (bundles.map(renderStoreBundle).join("") || "<p class='sow-menu__empty'>" + esc(SOW_t("store.offers_unavailable")) + "</p>") + "</div></section>" +
                    "<section class='sow-store__section' aria-labelledby='sow-store-skins'><div class='sow-store__section-head'><h2 id='sow-store-skins'>" + esc(SOW_t("store.skins")) + "</h2><span>" + esc(SOW_t("store.territory_patterns")) + "</span></div><div class='sow-store__skin-grid'>" + (skins.map(renderStoreSkinPromo).join("") || "<p class='sow-menu__empty'>" + esc(SOW_t("store.offers_unavailable")) + "</p>") + "</div></section>" +
                    "<section class='sow-store__promos'><button type='button' data-command='main_nav' data-nav-screen='heroes'><strong>" + esc(SOW_t("store.leaders")) + "</strong><span>" + esc(SOW_t("store.choose_commander")) + "</span><b>" + esc(SOW_t("store.view")) + "</b></button><button type='button' data-command='main_nav' data-nav-screen='heroes'><strong>" + esc(SOW_t("store.skins")) + "</strong><span>" + esc(SOW_t("store.change_territory_style")) + "</span><b>" + esc(SOW_t("store.view")) + "</b></button></section>" +
                    checkout +
        "</section></main>";
    }

    function loadStripeJs() {
        if (window.Stripe) return Promise.resolve(window.Stripe);
        if (stripePromise) return stripePromise;
        stripePromise = new Promise(function (resolve, reject) {
            var script = document.createElement("script");
            script.src = "https://js.stripe.com/v3/";
            script.async = true;
            script.onload = function () { window.Stripe ? resolve(window.Stripe) : reject(new Error("Stripe.js unavailable")); };
            script.onerror = function () { reject(new Error("Stripe.js failed to load")); };
            document.head.appendChild(script);
        });
        return stripePromise;
    }

    function beginStorePurchase(productId) {
        if (!productId || storeCheckoutBusy) return false;
        if (!beginExternalPurchaseAttempt(productId)) return false;
        if (isAndroidTwa()) {
            if (!state.account_id || typeof window.SOW_requestAndroidPurchase !== "function") {
                abandonExternalPurchaseAttempt(productId);
                state.error = SOW_t("store.store_unavailable");
                render();
                return false;
            }
            storeCheckoutBusy = true;
            var requestId = window.SOW_requestAndroidPurchase(productId, state.account_id);
            if (!requestId) {
                storeCheckoutBusy = false;
                abandonExternalPurchaseAttempt(productId);
                state.error = SOW_t("store.android_checkout_unavailable");
                render();
            } else {
                storeCheckoutRequestId = requestId;
                render();
            }
            return !!requestId;
        }
        var auth = storeAuth();
        if (!auth.available || !state.account_id) {
            abandonExternalPurchaseAttempt(productId);
            state.error = SOW_t("store.store_unavailable");
            render();
            return false;
        }
        storeCheckoutProduct = productId;
        storeCheckoutBusy = true;
        render();
        fetch(profileApi("/store/checkout"), {
            method: "POST",
            headers: auth.headers,
            body: JSON.stringify({ account_id: state.account_id, auth_secret: auth.authSecret, product_id: productId })
        }).then(function (response) {
            return response.json().catch(function () { return {}; }).then(function (data) {
                if (!response.ok) throw new Error(data.error || SOW_t("store.checkout_unavailable"));
                return data;
            });
        }).then(function (data) {
            return loadStripeJs().then(function (Stripe) {
                if (!data.publishable_key || !data.client_secret) throw new Error("Checkout is not configured");
                var stripe = Stripe(data.publishable_key);
                return stripe.initEmbeddedCheckout({
                    clientSecret: data.client_secret,
                    onComplete: function () {
                        completeExternalPurchase(productId);
                    }
                });
            });
        }).then(function (checkout) {
            if (!storeCheckoutProduct || storeCheckoutProduct !== productId) {
                checkout.destroy();
                return;
            }
            storeCheckoutInstance = checkout;
            var host = root.querySelector("[data-store-checkout-host]");
            if (host) {
                host.textContent = "";
                checkout.mount(host);
            }
        }).catch(function (error) {
            abandonExternalPurchaseAttempt(productId);
            state.error = error.message === "Checkout is not configured" ? SOW_t("store.checkout_not_configured") : SOW_t("store.checkout_unavailable");
            storeCheckoutProduct = null;
            if (purchaseModal && purchaseModal.type === "product" && purchaseModal.productId === productId) {
                purchaseModal.submitted = false;
                purchaseIntent = null;
            }
            render();
        }).finally(function () {
            storeCheckoutBusy = false;
            if (purchaseModal && purchaseModal.type === "product" && !storeCheckoutProduct) render();
        });
        return true;
    }

    function beginStoreRestore() {
        if (!isAndroidTwa() || storeCheckoutBusy || !state.account_id ||
            typeof window.SOW_requestAndroidRestore !== "function") return;
        storeCheckoutBusy = true;
        var requestId = window.SOW_requestAndroidRestore(state.account_id);
        if (!requestId) {
            storeCheckoutBusy = false;
            state.error = SOW_t("store.android_restore_unavailable");
        } else {
            storeCheckoutRequestId = requestId;
        }
        render();
    }

    function closeStoreCheckout() {
        if (storeCheckoutInstance && typeof storeCheckoutInstance.destroy === "function") {
            storeCheckoutInstance.destroy();
        }
        storeCheckoutInstance = null;
        storeCheckoutProduct = null;
        storeCheckoutRequestId = null;
        storeCheckoutBusy = false;
        abandonExternalPurchaseAttempt();
        dismissPurchaseModal();
        render();
    }

    function loadStoreCatalog() {
        if (storeCatalogLoading || storeCatalogLoaded) return;
        storeCatalogLoading = true;
        fetch(profileApi("/store/catalog"), { headers: { "Accept": "application/json" } })
            .then(function (response) {
                if (!response.ok) throw new Error("catalog unavailable");
                return response.json();
            })
            .then(function (catalog) {
                var current = state.store || {};
                var localLeaders = current.leaders || [];
                var localSkins = current.skins || [];
                catalog.leaders = (catalog.leaders || []).map(function (leader) {
                    var local = localLeaders.find(function (item) {
                        return String(item.id).replace(/[^a-z0-9]/gi, "").toLowerCase() === String(leader.id).replace(/[^a-z0-9]/gi, "").toLowerCase();
                    });
                    return Object.assign({}, leader, local && {
                        owned: local.owned,
                        free_rotation: local.free_rotation,
                        available: local.available
                    });
                });
                catalog.skins = (catalog.skins || []).map(function (skin) {
                    var local = localSkins.find(function (item) { return item.id === skin.id; });
                    return Object.assign({}, skin, local && { owned: local.owned });
                });
                state.store = Object.assign({}, current, catalog, {
                    gems: current.gems == null ? 0 : current.gems,
                    crowns: current.crowns == null ? 0 : current.crowns
                });
                storeCatalogLoaded = true;
                render();
            })
            .catch(function () { storeCatalogLoaded = true; })
            .finally(function () { storeCatalogLoading = false; });
    }
