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

    function renderCurrencyAmount(amount, kind) {
        return esc(amount) + " <img class='sow-store__currency-icon' src='" + esc(currencyAsset(kind)) + "' alt='' aria-hidden='true'>";
    }

    function storeProductButton(productId, label) {
        return "<button class='sow-store__buy sow-store__buy--primary' type='button' data-command='buy_product' data-product-id='" + esc(productId) + "'>" + esc(label || SOW_t("store.buy")) + "</button>";
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

    function renderLeaderPurchase(leader) {
        var offer = storeLeaderById(leader.id);
        if (!offer || offer.owned) return "";
        var crowns = offer.cost_crowns == null ? offer.cost_laurels : offer.cost_crowns;
        return "<div class='sow-heroes__purchase-actions'><span>" + esc(SOW_t("store.unlock")) + "</span>" +
            "<button class='sow-store__buy' type='button' data-command='unlock_leader' data-currency='crowns' data-leader-id='" + esc(offer.id) + "'>" + renderCurrencyAmount(crowns, "crown") + "</button>" +
            "<button class='sow-store__buy' type='button' data-command='unlock_leader' data-currency='gems' data-leader-id='" + esc(offer.id) + "'>" + renderCurrencyAmount(offer.cost_gems, "gem") + "</button>" +
            (canRenderDirectPurchase(offer.direct_product_id) ? storeProductButton(offer.direct_product_id, SOW_t("store.buy")) : "") +
            "</div>";
    }

    function renderStoreBundle(bundle) {
        var action = canRenderProductPurchase(bundle.product_id) ? storeProductButton(bundle.product_id, SOW_t("store.buy")) : "";
        var art = bundle.asset_path ? asset(bundle.asset_path) : asset("gameplay/store/gem_bundles/" + bundle.id + ".webp");
        return "<article class='sow-store__bundle'><img class='sow-store__bundle-icon' src='" + esc(art) + "' alt='' aria-hidden='true' width='56' height='56' loading='lazy'><div><strong class='sow-store__bundle-price' aria-label='" + esc(SOW_t("store.gems_count", { amount: bundle.gems })) + "'>" + renderCurrencyAmount(bundle.gems, "gem") + "</strong></div>" + action + "</article>";
    }

    function renderStoreSkinPromo(skin) {
        var action;
        if (skin.owned) {
            action = state.selected_skin === skin.id
                ? "<span class='sow-store__offer-state'>" + esc(SOW_t("store.equipped")) + "</span>"
                : "<button class='sow-store__buy' type='button' data-command='equip_skin' data-skin-id='" + esc(skin.id) + "'>" + esc(SOW_t("store.equip")) + "</button>";
        } else {
            action = "<div class='sow-store__buy-row'><button class='sow-store__buy' type='button' data-command='unlock_skin' data-skin-id='" + esc(skin.id) + "'>" + renderCurrencyAmount(skin.cost_gems, "gem") + "</button>" +
                (canRenderDirectPurchase(skin.direct_product_id) ? storeProductButton(skin.direct_product_id, SOW_t("store.buy")) : "") + "</div>";
        }
        return "<article class='sow-store__skin'><div class='sow-store__skin-art'><img src='" + esc(asset(skin.asset_path)) + "' alt='' width='96' height='96' loading='lazy'></div><div class='sow-store__skin-body'><h3>" + esc(skin.name) + "</h3><p>" + esc(SOW_t("store.territory_pattern")) + "</p>" + action + "</div></article>";
    }

    function renderStore() {
        var store = state.store || {};
        var bundles = Array.isArray(store.gem_bundles) ? store.gem_bundles : [];
        var skins = Array.isArray(store.skins) ? store.skins : [];
        var crowns = store.crowns == null ? store.laurels : store.crowns;
        var featured = bundles.find(function (bundle) { return bundle.product_id === "sow_gems_2600"; }) || bundles[bundles.length - 1];
        var featuredAction = featured && canRenderProductPurchase(featured.product_id) ? storeProductButton(featured.product_id, SOW_t("store.buy")) : "";
        var restoreAction = isAndroidTwa() && state.account_id &&
            typeof window.SOW_requestAndroidRestore === "function" &&
            typeof window.SOW_isAndroidPurchaseBridgeReady === "function" &&
            window.SOW_isAndroidPurchaseBridgeReady()
            ? "<button class='sow-store__link' type='button' data-command='restore_purchases'>" + esc(SOW_t("store.restore_purchases")) + "</button>"
            : "";
        var checkout = storeCheckoutProduct
            ? "<section class='sow-store__checkout' data-store-checkout><div class='sow-store__section-head'><h2>" + esc(SOW_t("store.checkout")) + "</h2><button class='sow-store__close' type='button' data-command='close_store_checkout' aria-label='" + esc(SOW_t("menu.close")) + "'>×</button></div><div class='sow-store__checkout-host' data-store-checkout-host><p>" + esc(SOW_t("store.loading_checkout")) + "</p></div></section>"
            : "";
        return "<main class='sow-menu__main sow-menu__main--store' data-screen-panel='store'><section class='sow-menu__store-slot' data-store-slot aria-label='" + esc(SOW_t("store.shop")) + "'>" +
                    "<header class='sow-store__heading'><div><p class='sow-store__eyebrow'>" + esc(SOW_t("store.shop")) + "</p><h1>" + esc(SOW_t("store.featured_offers")) + "</h1></div><div class='sow-store__balances'><span class='sow-store__balance--gems' aria-label='" + esc(SOW_t("store.gems_count", { amount: store.gems || 0 })) + "'>" + renderCurrencyAmount(store.gems || 0, "gem") + "</span><span class='sow-store__balance--crowns' aria-label='" + esc(SOW_t("store.crowns_count", { amount: crowns || 0 })) + "'>" + renderCurrencyAmount(crowns || 0, "crown") + "</span></div></header>" +
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
        if (!productId || storeCheckoutBusy) return;
        if (isAndroidTwa()) {
            if (!state.account_id || typeof window.SOW_requestAndroidPurchase !== "function") {
                state.error = SOW_t("store.store_unavailable");
                render();
                return;
            }
            storeCheckoutBusy = true;
            var requestId = window.SOW_requestAndroidPurchase(productId, state.account_id);
            if (!requestId) {
                storeCheckoutBusy = false;
                state.error = SOW_t("store.android_checkout_unavailable");
                render();
            } else {
                storeCheckoutRequestId = requestId;
                render();
            }
            return;
        }
        var auth = storeAuth();
        if (!auth.available || !state.account_id) {
            state.error = SOW_t("store.store_unavailable");
            render();
            return;
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
                return stripe.initEmbeddedCheckout({ clientSecret: data.client_secret });
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
            state.error = error.message === "Checkout is not configured" ? SOW_t("store.checkout_not_configured") : SOW_t("store.checkout_unavailable");
            storeCheckoutProduct = null;
            render();
        }).finally(function () {
            storeCheckoutBusy = false;
        });
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
                    return Object.assign({}, leader, local && { owned: local.owned, free_rotation: local.free_rotation });
                });
                catalog.skins = (catalog.skins || []).map(function (skin) {
                    var local = localSkins.find(function (item) { return item.id === skin.id; });
                    return Object.assign({}, skin, local && { owned: local.owned });
                });
                state.store = Object.assign({}, current, catalog, {
                    gems: current.gems == null ? 0 : current.gems,
                    crowns: current.crowns == null ? current.laurels : current.crowns
                });
                storeCatalogLoaded = true;
                render();
            })
            .catch(function () { storeCatalogLoaded = true; })
            .finally(function () { storeCatalogLoading = false; });
    }
