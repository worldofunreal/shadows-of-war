// Shared menu chrome, overlays, lifecycle, events, and state bridge.

    // ── Conduct & privacy (Terms/Privacy enforcement in-client) ──
    var REPORT_REASONS = [
        ["cheating", "profile.cheating"],
        ["harassment", "profile.harassment"],
        ["hate_speech", "profile.hate_speech"],
        ["threats", "profile.threats"],
        ["spam", "profile.spam"],
        ["inappropriate_name", "profile.inappropriate_name"],
        ["exploiting", "profile.exploiting"],
        ["other", "profile.other_explain"]
    ];
    var reportOpen = false;
    var reportTarget = null;
    var reportSent = false;
    var reportBusy = false;
    var exitMenuAssetsPending = false;
    var exitMenuAssetsReady = false;
    var exitMenuAssetsToken = 0;
    var deleteArmed = false;
    var deleteBusy = false;
    var activeMenuPointerId = null;
    var renderPending = false;
    var renderFlushTimer = null;
    var rewardAnimationTimer = null;
    var rewardAckRetryTimer = null;
    var rewardAnimationRunning = false;
    var rewardAnimationAccount = null;
    var rewardAnimationShown = new Set();
    var rewardAckTimes = Object.create(null);
    var rewardAnimationToken = 0;
    var activeRewardStage = null;
    var rewardOptimisticPreview = null;
    var rewardPresentationReady = false;
    var displayedProgression = null;
    var progressionAnimationFrame = 0;
    var progressionAnimationTarget = null;

    function renderTopbar() {
        var leader = leaderById(state.selected_leader);
        var name = state.player_name || SOW_t("menu.anonymous");
        var auth = {};
        /* POKI_SHARED_AUTH_LOOKUP_BEGIN */
        auth = typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() : { linked: false, pending: false };
        /* POKI_SHARED_AUTH_LOOKUP_END */
        var accountXp = Math.max(0, Number(state.xp) || 0);
        var crowns = state.crowns || 0;
        var laurels = state.laurels || 0;
        var gems = state.gems || 0;
        var showSignIn = window.SOW_PORTAL !== "poki" && window.SOW_PORTAL !== "jest" && !(auth.linked || auth.pending);
        var signInMarkup = "";
        /* POKI_SHARED_SIGNIN_MARKUP_BEGIN */
        signInMarkup = "<button class='sow-menu__signin' type='button' data-command='sign_in'>" + esc(SOW_t("menu.sign_in")) + "</button>";
        /* POKI_SHARED_SIGNIN_MARKUP_END */
        return "" +
            "<header class='sow-menu__topbar'>" +
                "<div class='sow-menu__identity'>" +
                    "<button class='sow-menu__avatar' type='button' data-command='open_leader_picker' " +
                        "aria-label='" + esc(SOW_t("menu.select_leader")) + "' style=\"background-image:url('" + esc(avatarImage()) + "')\"></button>" +
                    "<div class='sow-menu__profile'>" +
                        "<input data-role='display-name' name='display_name' value=\"" + esc(name) + "\" maxlength='16' aria-label='" + esc(SOW_t("menu.display_name")) + "'>" +
                        "<button class='sow-menu__profile-link' type='button' data-command='open_profile'>" + esc(leaderDisplayName(leader)) + " · " + esc(leaderCivilization(leader)) + "</button>" +
                    "</div>" +
                "</div>" +
                "<div class='sow-menu__top-actions'>" +
                    "<div class='sow-menu__progress' data-progression data-command='open_profile' role='button' tabindex='0' title='" + esc(SOW_t("menu.open_profile")) + "' aria-label='" + esc(SOW_t("menu.open_profile")) + "'>" +
                        "<span class='sow-menu__progress-cell sow-menu__level'><small>" + esc(SOW_t("menu.level_short")) + "</small><strong data-progression-level-value>" + esc(state.level) + "</strong></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__xp'><span class='sow-menu__xp-value' data-progression-xp-value>" + esc(Math.floor(accountXp)) + " " + esc(SOW_t("menu.xp")) + "</span><span class='sow-menu__xp-track' aria-hidden='true'><i data-progression-xp-fill style='width:" + (accountXp % 100) + "%'></i></span></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__crowns'><img class='sow-menu__currency-icon' src='" + esc(currencyAsset("crown")) + "' alt='' aria-hidden='true'><strong data-progression-crowns-value>" + esc(crowns) + "</strong></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__laurels' aria-label='" + esc(SOW_t("profile.laurels")) + "'><img class='sow-menu__currency-icon' src='" + esc(currencyAsset("laurel")) + "' alt='' aria-hidden='true'><strong data-progression-laurels-value>" + esc(laurels) + "</strong></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__gems'><img class='sow-menu__currency-icon' src='" + esc(currencyAsset("gem")) + "' alt='' aria-hidden='true'><strong data-progression-gems-value>" + esc(gems) + "</strong></span>" +
                    "</div>" +
                    (showSignIn ? signInMarkup : ((window.SOW_PORTAL === "poki" || window.SOW_PORTAL === "jest") ? "<span class='sow-menu__account-label'>" + esc(SOW_t("menu.anonymous")) + "</span>" : "")) +
                    "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='" + esc(SOW_t("menu.settings")) + "'>⚙</button>" +
                "</div>" +
            "</header>";
    }

    /* POKI_RENDER_REPLACEMENT_BEGIN */
    function renderCommandPanel() {
        return "" +
            "<section class='sow-menu__command'>" +
                "<div class='sow-menu__home-public'>" + renderPublicPanel("home") + "</div>" +
                "<div class='sow-menu__home-actions'>" +
                    "<button class='sow-menu__primary' type='button' data-command='quick_match'>" + esc(SOW_t("menu.quick_match")) + " <span>↗</span></button>" +
                    "<button class='sow-menu__secondary' type='button' data-command='open_campaign'>" + esc(SOW_t("menu.campaign")) + " <span>⚔</span></button>" +
                    "<button class='sow-menu__secondary' type='button' data-command='open_browser'>" + esc(SOW_t("menu.lobby_browser")) + " <span>→</span></button>" +
                    "<form class='sow-menu__join' data-form='join'>" +
                        "<input name='code' inputmode='numeric' autocomplete='off' placeholder='" + esc(SOW_t("menu.lobby_code")) + "' aria-label='" + esc(SOW_t("menu.lobby_code")) + "'>" +
                        "<button type='submit'>" + esc(SOW_t("menu.join")) + "</button>" +
                    "</form>" +
                    "<button class='sow-menu__secondary' type='button' data-command='open_create'>" + esc(SOW_t("menu.create_custom_game")) + " <span>+</span></button>" +
                    renderFeedback() +
                "</div>" +
            "</section>";
    }

    function renderFeedback() {
        var error = state.error ? "<div class='sow-menu__status sow-menu__status--error'>" + esc(localizedText(state.error)) + "</div>" : "";
        var notice = state.notice ? "<div class='sow-menu__status sow-menu__status--notice'>" +
            esc(SOW_t(({ host_left: "menu.host_left", kicked: "menu.removed_from_lobby", banned: "menu.banned_from_lobby", connection_lost: "menu.connection_lost" }[state.notice] || "menu.connection_lost"))) +
            "</div>" : "";
        return error + notice;
    }

    function renderFooter(label) {
        var externalAttrs = isAndroidTwa() ? "" : " target='_blank' rel='noopener noreferrer'";
        return "<footer class='sow-menu__footer'>" + (label ? "<span data-menu-footer-label>" + esc(label) + "</span>" : "") + "<nav class='sow-menu__footer-links' aria-label='" + esc(SOW_t("menu.game_links")) + "'>" +
            "<a href='/how-to-play/'>" + esc(SOW_t("menu.how_to_play")) + "</a><a href='/support/'>" + esc(SOW_t("menu.support")) + "</a><a href='/terms/'>" + esc(SOW_t("menu.terms")) + "</a><a href='/privacy/'>" + esc(SOW_t("menu.privacy")) + "</a><a href='/cookies/'>" + esc(SOW_t("menu.cookies")) + "</a>" +
            "<a href='https://discord.gg/d6ZDeChSE'" + externalAttrs + ">" + esc(SOW_t("menu.discord")) + "</a><a href='https://t.me/shadowsofwario'" + externalAttrs + ">" + esc(SOW_t("menu.telegram")) + "</a><a href='https://github.com/worldofunreal/shadows-of-war'" + externalAttrs + ">" + esc(SOW_t("menu.github")) + "</a>" +
            "</nav><span>" + esc(SOW_t("menu.brand")) + "</span></footer>";
    }

    function renderMainNav(active) {
        return renderMainNavMarkup(active, MAIN_NAV_ITEMS);
    }

    function localeOptions() {
        var codes = typeof window.SOW_getSupportedLocales === "function" ? window.SOW_getSupportedLocales() : [];
        return codes.map(function (code) {
            return { value: code, label: typeof window.SOW_getLocaleLabel === "function" ? window.SOW_getLocaleLabel(code) : SOW_t("menu.language_" + String(code).replace(/-/g, "_")) };
        });
    }

    function renderSettings() {
        var settings = state.settings || {};
        var vol = settings.music_volume == null ? 0.8 : settings.music_volume;
        var volPct = Math.round(vol * 100);
        var auth = typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() : { platform: isAndroidTwa() ? "twa" : "web", linked: false, pending: false };
        var session = typeof window.SOW_getWouSession === "function" ? window.SOW_getWouSession() : authSession();
        var accountEmail = session && session.user && session.user.email ? String(session.user.email).trim() : "";
        var providers = (auth.linkedProviders || []).map(function (provider) {
            return String(provider).replace(/_/g, " ").toUpperCase();
        });
        var providerLabel = auth.platform === "twa" ? SOW_t("menu.google_play_games") :
            auth.provider === "crazygames" ? "CRAZYGAMES" :
            providers.join(" · ") || (auth.provider ? String(auth.provider).replace(/_/g, " ").toUpperCase() : "");
        var accountDetail = auth.linked ? (accountEmail || providerLabel || SOW_t("menu.anonymous")) : SOW_t("menu.anonymous");
        var accountControl = auth.pending
            ? "<div class='sow-menu__settings-account sow-menu__settings-account--pending'><strong class='sow-menu__account-pending'>" + esc(SOW_t("menu.connecting")) + "</strong></div>"
            : "<div class='sow-menu__settings-account'><strong class='sow-menu__account-value'>" + esc(accountDetail) + "</strong></div>";
        var signOutControl = "";
        if (auth.canSignOut) {
            signOutControl = signOutConfirmOpen
                ? "<section class='sow-menu__settings-confirm' role='alertdialog' aria-label='" + esc(SOW_t("menu.sign_out")) + "'>" +
                    "<strong>" + esc(SOW_t("menu.sign_out")) + "?</strong>" +
                    "<div class='sow-menu__settings-confirm-actions'>" +
                        "<button class='sow-menu__secondary' type='button' data-command='cancel_sign_out'>" + esc(SOW_t("lobbies.cancel")) + "</button>" +
                        "<button class='sow-menu__danger' type='button' data-command='confirm_sign_out'>" + esc(SOW_t("menu.sign_out")) + "</button>" +
                    "</div>" +
                "</section>"
                : "<button class='sow-menu__danger sow-menu__settings-signout' type='button' data-command='sign_out'>" + esc(SOW_t("menu.sign_out")) + "</button>";
        }
        return "" +
            "<div class='sow-menu__overlay' data-menu-overlay='settings'>" +
                "<section class='sow-menu__modal sow-menu__settings-modal'>" +
                    "<div class='sow-menu__modal-head'>" +
                        "<div>" +
                            "<h2>" + esc(SOW_t("menu.settings")) + "</h2>" +
                        "</div>" +
                        "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='" + esc(SOW_t("menu.close")) + "'>×</button>" +
                    "</div>" +
                    "<div class='sow-menu__settings-body'>" +
                        accountControl +
                        "<div class='sow-menu__settings-controls'>" +
                            "<label class='sow-menu__form-field'>" +
                                "<div class='sow-menu__slider-label'><span>" + esc(SOW_t("menu.music_volume")) + "</span><b data-val-for='music_vol'>" + volPct + "%</b></div>" +
                                "<input class='sow-menu__field' type='range' name='music_volume' min='0' max='1' step='0.05' value='" + esc(vol) + "' data-setting='music_volume'>" +
                            "</label>" +
                            "<label class='sow-menu__form-field'>" +
                                "<span>" + esc(SOW_t("menu.motion_animation")) + "</span>" +
                                SOW_renderDropdown({ key: "settings-motion", name: "reduced_motion", setting: "reduced_motion", value: settings.reduced_motion ? "reduced" : "full", options: [
                                    { value: "full", label: SOW_t("menu.full") },
                                    { value: "reduced", label: SOW_t("menu.reduced_motion") }
                                ] }) +
                            "</label>" +
                            "<label class='sow-menu__form-field'><span>" + esc(SOW_t("menu.language")) + "</span>" +
                                SOW_renderDropdown({ key: "settings-language", name: "locale", setting: "locale", value: typeof window.SOW_getLocale === "function" ? window.SOW_getLocale() : "en", options: localeOptions() }) +
                            "</label>" +
                        "</div>" +
                        (signOutControl ? "<div class='sow-menu__settings-actions'>" + signOutControl + "</div>" : "") +
                    "</div>" +
                "</section>" +
            "</div>";
    }

    function authApi(path) {
        return "https://id.worldofunreal.com" + path;
    }

    function authSession() {
        if (typeof window.SOW_getWouSession === "function") return window.SOW_getWouSession();
        try {
            var token = window.localStorage.getItem("wou_session_token") || "";
            var user = JSON.parse(window.localStorage.getItem("wou_user_data") || "null");
            return { token: token, accountId: user && user.account_id || "", user: user };
        } catch (e) {
            return { token: "", accountId: "", user: null };
        }
    }

    function authHeaders() {
        var headers = { "Content-Type": "application/json", "Accept": "application/json" };
        var session = authSession();
        if (session.token) headers.Authorization = "Bearer " + session.token;
        return headers;
    }

    function ensureAuthSession() {
        if (typeof window.SOW_ensureWouAnonymousSession === "function") {
            return window.SOW_ensureWouAnonymousSession();
        }
        return Promise.resolve(authSession());
    }

    function authIcon(provider) {
        if (provider === "google") return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='#4285F4' d='M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z'/><path fill='#34A853' d='M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z'/><path fill='#FBBC05' d='M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.06H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.94l2.85-2.22z'/><path fill='#EA4335' d='M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.06l3.66 2.84C6.71 7.3 9.14 5.38 12 5.38z'/></svg>";
        if (provider === "discord") return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='currentColor' d='M20.3 4.37a19.8 19.8 0 0 0-4.88-1.52.08.08 0 0 0-.08.04c-.21.38-.45.87-.61 1.25a18.27 18.27 0 0 0-5.49 0c-.17-.38-.41-.87-.62-1.25a.08.08 0 0 0-.08-.04A19.74 19.74 0 0 0 3.68 4.37a.07.07 0 0 0-.03.03C.53 9.05-.32 13.58.1 18.06a.08.08 0 0 0 .03.06 19.9 19.9 0 0 0 5.99 3.03.08.08 0 0 0 .08-.03c.46-.63.87-1.3 1.23-1.99a.08.08 0 0 0-.04-.11 12.3 12.3 0 0 1-1.87-.89.08.08 0 0 1-.01-.13 10.2 10.2 0 0 0 .37-.29.07.07 0 0 1 .08-.01c3.93 1.79 8.18 1.79 12.06 0a.07.07 0 0 1 .08.01c.12.1.25.2.37.29a.08.08 0 0 1-.01.13c-.59.35-1.21.65-1.87.89a.08.08 0 0 0-.04.11c.36.7.77 1.36 1.23 1.99a.08.08 0 0 0 .08.03 19.84 19.84 0 0 0 6-3.03.08.08 0 0 0 .03-.05c.5-5.18-.84-9.67-3.55-13.66a.06.06 0 0 0-.03-.03zM8.02 15.33c-1.18 0-2.16-1.09-2.16-2.42s.96-2.42 2.16-2.42 2.18 1.1 2.16 2.42-.96 2.42-2.16 2.42zm7.98 0c-1.18 0-2.16-1.09-2.16-2.42s.96-2.42 2.16-2.42 2.18 1.1 2.16 2.42-.95 2.42-2.16 2.42z'/></svg>";
        if (provider === "twitter") return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='currentColor' d='M18.24 2.25h3.31l-7.23 8.26 8.5 11.24h-6.66l-5.21-6.82-5.97 6.82H1.68l7.73-8.84L1.25 2.25h6.82l4.71 6.23 5.46-6.23zm-1.16 17.52h1.83L7.08 4.13H5.12l11.96 15.64z'/></svg>";
        return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='currentColor' d='M24 12.07a12 12 0 1 0-13.88 11.86v-8.38H7.08v-3.48h3.04V9.43c0-3.01 1.79-4.67 4.53-4.67 1.31 0 2.69.24 2.69.24v2.95h-1.51c-1.49 0-1.96.93-1.96 1.88v2.25h3.33l-.53 3.47h-2.8v8.38A12 12 0 0 0 24 12.07z'/></svg>";
    }

    function renderAuthSocial() {
        return [["google", "auth.google"], ["discord", "auth.discord_provider"], ["twitter", "auth.x_provider"], ["meta", "auth.meta"]].map(function (provider) {
            return "<button class='sow-auth__provider' type='button' data-command='wou_provider' data-provider='" + provider[0] + "'><span class='sow-auth__provider-icon sow-auth__provider-icon--" + provider[0] + "'>" + authIcon(provider[0]) + "</span><span>" + esc(SOW_t("auth.continue_with", { provider: SOW_t(provider[1]) })) + "</span><i>↗</i></button>";
        }).join("");
    }

    function finishWouLogin(data) {
        if (!data || !data.session_token || !data.account) throw new Error(SOW_t("auth.identity_incomplete"));
        try {
            window.localStorage.setItem("wou_session_token", data.session_token);
            window.localStorage.setItem("wou_user_data", JSON.stringify(data.account));
        } catch (e) {
            throw new Error(SOW_t("auth.account_save_failed"));
        }
        authBusy = false;
        authModalOpen = false;
        authOtpSent = false;
        authError = "";
        authNotice = "";
        window.dispatchEvent(new CustomEvent("wou:auth-state-change", {
            detail: { authenticated: true, isAuthenticated: true, user: data.account, token: data.session_token }
        }));
        window.location.reload();
    }

    function authJson(response) {
        return response.json().catch(function () { return {}; }).then(function (data) {
            if (!response.ok) throw new Error(data.error || SOW_t("auth.sign_in_unavailable"));
            return data;
        });
    }

    function requestAuthOtp() {
        if (authBusy) return;
        var email = String(authEmail || "").trim();
        if (!/^\S+@\S+\.\S+$/.test(email)) {
            authError = SOW_t("auth.valid_email");
            authNotice = "";
            render();
            return;
        }
        authEmail = email;
        authBusy = true;
        authError = "";
        authNotice = "";
        render();
        ensureAuthSession().then(function (session) {
            return fetch(authApi("/api/v1/auth/otp/request"), {
            method: "POST",
            headers: authHeaders(),
            body: JSON.stringify({
                email: authEmail,
                account_id: session.accountId || null,
                context: "shadows_of_war",
                newsletter_opt_in: false
            })
            }).then(authJson);
        }).then(function (data) {
            authOtpSent = true;
            authBusy = false;
            authNotice = data.message || SOW_t("auth.verification_sent");
            render();
        }).catch(function (error) {
            authBusy = false;
            authError = error && error.message ? error.message : SOW_t("auth.send_code_failed");
            render();
        });
    }

    function verifyAuthOtp() {
        if (authBusy) return;
        var code = String(authCode || "").trim();
        if (!/^\d{6}$/.test(code)) {
            authError = SOW_t("auth.six_digit_code");
            authNotice = "";
            render();
            return;
        }
        authBusy = true;
        authError = "";
        authNotice = "";
        render();
        ensureAuthSession().then(function (session) {
            return fetch(authApi("/api/v1/auth/otp/verify"), {
            method: "POST",
            headers: authHeaders(),
            body: JSON.stringify({
                email: authEmail,
                code: code,
                account_id: session.accountId || null,
                context: "shadows_of_war"
            })
            }).then(authJson);
        }).then(finishWouLogin).catch(function (error) {
            authBusy = false;
            authError = error && error.message ? error.message : SOW_t("auth.invalid_code");
            render();
        });
    }

    function resetAuthFlow() {
        authEmail = "";
        authCode = "";
        authOtpSent = false;
        authBusy = false;
        authError = "";
        authNotice = "";
    }

    function renderAuthModal() {
        var emailPanel = authOtpSent
            ? "<form class='sow-auth__form' data-auth-form='verify'><label>" + esc(SOW_t("auth.code_sent_to")) + " <strong>" + esc(authEmail) + "</strong></label><input class='sow-auth__code' data-auth-field='code' inputmode='numeric' autocomplete='one-time-code' maxlength='6' value='" + esc(authCode) + "' placeholder='000000' aria-label='" + esc(SOW_t("auth.verification_code")) + "' required><button class='sow-auth__submit sow-auth__submit--cyan' type='submit'" + (authBusy ? " disabled" : "") + ">" + esc(authBusy ? SOW_t("auth.checking") : SOW_t("auth.verify_code")) + "</button><div class='sow-auth__form-links'><button type='button' data-command='auth_change_email'>" + esc(SOW_t("auth.change_email")) + "</button><button type='button' data-command='auth_resend'" + (authBusy ? " disabled" : "") + ">" + esc(SOW_t("auth.resend")) + "</button></div></form>"
            : "<form class='sow-auth__form' data-auth-form='request'><label for='sow-auth-email'>" + esc(SOW_t("auth.email_address")) + "</label><input id='sow-auth-email' class='sow-auth__input' data-auth-field='email' type='email' autocomplete='email' value='" + esc(authEmail) + "' placeholder='" + esc(SOW_t("auth.email_placeholder")) + "' required><button class='sow-auth__submit' type='submit'" + (authBusy ? " disabled" : "") + ">" + esc(authBusy ? SOW_t("auth.sending") : SOW_t("auth.send_code")) + "</button></form>";
        var error = authError ? "<div class='sow-auth__message sow-auth__message--error' role='alert'>" + esc(authError) + "</div>" : "";
        var notice = authNotice ? "<div class='sow-auth__message sow-auth__message--notice' role='status'>" + esc(authNotice) + "</div>" : "";
        return "<div class='sow-menu__overlay' data-menu-overlay='auth' data-auth-overlay><section class='sow-menu__modal sow-auth' role='dialog' aria-modal='true' aria-label='" + esc(SOW_t("auth.account")) + "'>" +
            "<div class='sow-auth__glow sow-auth__glow--cyan'></div><div class='sow-auth__head'><div class='sow-auth__logos'><img class='sow-auth__game-logo' src='/sow-long.svg' alt='" + esc(SOW_t("menu.brand")) + "'><span class='sow-auth__logo-divider' aria-hidden='true'></span><img class='sow-auth__wou-logo' src='https://worldofunreal.com/wouid.svg' alt='WouID'></div><button class='sow-menu__icon-button' type='button' data-command='close_auth' aria-label='" + esc(SOW_t("auth.close")) + "'>×</button></div>" +
            error + notice + "<div class='sow-auth__body'>" + emailPanel + "<div class='sow-auth__social-list'>" + renderAuthSocial() + "</div></div>" +
            "<a class='sow-auth__terms' href='/terms/'>" + esc(SOW_t("auth.terms")) + "</a></section></div>";
    }

    /* POKI_RENDER_REPLACEMENT_END */
    function renderScreenPanel(screen) {
        if (screen === "home") return renderHome();
        if (screen === "campaign") return renderCampaign();
        if (screen === "browser") return renderBrowser();
        if (screen === "create") return renderCreate();
        if (screen === "queue") return renderQueue();
        if (screen === "profile") return renderProfile();
        if (screen === "heroes") return renderHeroes();
        if (screen === "store") return renderStore();
        return "";
    }

    function screenNav(screen) {
        return screen === "store" ? "store" : screen === "heroes" ? "heroes" : screen === "profile" ? "profile" : "battle";
    }

    function screenShellClass(screen) {
        return "sow-menu__shell" + (screen === "store" ? " sow-menu__store" : screen === "heroes" ? " sow-menu__heroes" : screen === "profile" ? " sow-profile" : "");
    }

    function screenBackdropClass(screen) {
        return "sow-menu__backdrop" + (screen === "store" ? " sow-store__backdrop" : screen === "heroes" ? " sow-heroes__backdrop" : "");
    }

    function screenFooterLabel(screen) {
        var key = ({ campaign: "menu.campaign", browser: "menu.lobby_browser", create: "menu.create_game", queue: "menu.lobby", store: "menu.shop", heroes: "menu.heroes", profile: "menu.profile" })[screen];
        return key ? SOW_t(key) : "";
    }

    function renderFrame(screen) {
        return "<div class='" + screenBackdropClass(screen) + "' data-menu-backdrop></div>" +
            "<div class='" + screenShellClass(screen) + "'>" +
                renderTopbar() +
                "<div class='sow-menu__screen-stage' data-screen-stage>" + renderScreenPanel(screen) + "</div>" +
                renderMainNav(screenNav(screen)) + renderFooter(screenFooterLabel(screen)) +
            "</div>" + renderPasswordModal() + renderProfileDetail();
    }

    function panelFromMarkup(markup) {
        var template = document.createElement("template");
        template.innerHTML = String(markup || "").trim();
        return template.content.firstElementChild;
    }

    function syncOverlay(key, markup) {
        var selector = "[data-menu-overlay='" + key + "']";
        var current = root.querySelector(selector);
        if (!markup) {
            if (current) current.remove();
            return;
        }
        var next = panelFromMarkup(markup);
        if (!next) return;
        if (!current) root.appendChild(next);
        else if (current.outerHTML !== next.outerHTML) current.replaceWith(next);
    }

    function updateTopbar() {
        var topbar = root.querySelector(".sow-menu__topbar");
        if (!topbar || !state) return;
        var leader = leaderById(state.selected_leader);
        var nameInput = topbar.querySelector("[data-role='display-name']");
        var name = state.player_name || SOW_t("menu.anonymous");
        if (nameInput && document.activeElement !== nameInput) nameInput.value = name;
        var avatar = topbar.querySelector(".sow-menu__avatar");
        var avatarUrl = avatarImage();
        if (avatar && avatar.dataset.avatarUrl !== avatarUrl) {
            avatar.style.backgroundImage = "url(" + JSON.stringify(avatarUrl) + ")";
            avatar.dataset.avatarUrl = avatarUrl;
        }
        var leaderLink = topbar.querySelector(".sow-menu__profile-link");
        if (leaderLink) leaderLink.textContent = leaderDisplayName(leader) + " · " + leaderCivilization(leader);
        var auth = {};
        /* POKI_SHARED_AUTH_LOOKUP_BEGIN */
        auth = typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() || {} : {};
        /* POKI_SHARED_AUTH_LOOKUP_END */
        var showSignIn = window.SOW_PORTAL !== "poki" && window.SOW_PORTAL !== "jest" && !(auth.linked || auth.pending);
        var actions = topbar.querySelector(".sow-menu__top-actions");
        var settingsButton = actions && actions.querySelector("[data-command='toggle_settings']");
        var signIn = actions && actions.querySelector(".sow-menu__signin");
        if (showSignIn) {
            if (!signIn && actions) {
                signIn = document.createElement("button");
                signIn.className = "sow-menu__signin";
                signIn.type = "button";
                signIn.dataset.command = "sign_in";
                actions.insertBefore(signIn, settingsButton);
            }
            if (signIn) signIn.textContent = SOW_t("menu.sign_in");
        } else if (signIn) {
            signIn.remove();
        }
    }

    function updateFrameChrome(screen) {
        var backdrop = root.querySelector("[data-menu-backdrop]");
        var shell = root.querySelector(".sow-menu__shell");
        var backdropClass = screenBackdropClass(screen);
        var shellClass = screenShellClass(screen);
        if (backdrop && backdrop.className !== backdropClass) backdrop.className = backdropClass;
        if (shell && shell.className !== shellClass) shell.className = shellClass;
        root.querySelectorAll("[data-nav-screen]").forEach(function (item) {
            var active = item.dataset.navScreen === screenNav(screen);
            var navItem = MAIN_NAV_ITEMS.find(function (candidate) {
                return candidate[0] === item.dataset.navScreen;
            });
            var labelText = navItem ? SOW_t(navItem[2]) : "";
            var labelNode = item.querySelector("small");
            if (labelNode && labelText) labelNode.textContent = labelText;
            if (labelText) item.setAttribute("aria-label", labelText);
            item.classList.toggle("is-active", active);
            if (active) item.setAttribute("aria-current", "page");
            else item.removeAttribute("aria-current");
        });
        var footer = root.querySelector(".sow-menu__footer");
        var label = footer && footer.querySelector("[data-menu-footer-label]");
        var nextLabel = screenFooterLabel(screen);
        if (footer && nextLabel && label) label.textContent = nextLabel;
        else if (footer && nextLabel && !label) {
            label = document.createElement("span");
            label.dataset.menuFooterLabel = "";
            label.textContent = nextLabel;
            footer.insertBefore(label, footer.firstElementChild);
        } else if (label && !nextLabel) label.remove();
        updateTopbar();
    }

    function activeScreenPanel() {
        var stage = root.querySelector("[data-screen-stage]");
        return stage && (stage.querySelector("[data-screen-panel].is-active") || stage.lastElementChild);
    }

    function syncScreenPanel(screen, screenChanged) {
        var stage = root.querySelector("[data-screen-stage]");
        var next = panelFromMarkup(renderScreenPanel(screen));
        if (!stage || !next) return;
        if (screenChanged) {
            var settings = state && state.settings || {};
            sowScreenMotion.show(stage, next, !!settings.reduced_motion);
            return;
        }
        var current = activeScreenPanel();
        if (current) current.replaceWith(next);
        else stage.appendChild(next);
        next.classList.add("is-active");
    }

    function render() {
        if (!state) return;
        if (activeMenuPointerId !== null) {
            renderPending = true;
            return;
        }
        renderPending = false;
        var screen = currentScreen();
        if (screen !== "heroes") skinPickerOpen = false;
        var screenChanged = previousScreen !== screen;
        var sameScreen = !screenChanged;
        if (screenChanged) dropdownOpenKey = null;
        if (screen === "create" && previousScreen !== "create") {
            createDraft = cloneConfig();
            createOffline = !!state.custom_game_is_sp;
            createPrivate = !!state.custom_game_is_private;
            createPassword = "";
        }
        if (screen !== "create") {
            createDraft = null;
            createOffline = false;
            createPrivate = false;
            createPassword = "";
        }
        previousScreen = screen;
        var nextHero = heroImage();
        if (root.dataset.hero !== nextHero) {
            root.style.setProperty("--sow-hero", "url(\"" + nextHero + "\")");
            root.dataset.hero = nextHero;
        }
        root.dataset.screen = screen;
        root.dataset.ready = typeof window.SOW_menu_command === "function" ? "true" : "false";
        root.hidden = state.phase !== "MainMenu";

        var activeEl = document.activeElement;
        var isTyping = activeEl && root.contains(activeEl) && (activeEl.tagName === "INPUT" || activeEl.tagName === "TEXTAREA" || activeEl.tagName === "SELECT");
        var activeRole = activeEl && activeEl.dataset ? activeEl.dataset.role : null;
        var activeName = activeEl ? activeEl.name : null;
        var activeVal = isTyping ? activeEl.value : null;
        var selStart = isTyping && typeof activeEl.selectionStart === "number" ? activeEl.selectionStart : null;
        var selEnd = isTyping && typeof activeEl.selectionEnd === "number" ? activeEl.selectionEnd : null;
        var currentPanel = activeScreenPanel();
        var scrollTop = null;
        if (sameScreen) {
            var currentScrollOwner = screen === "create"
                ? currentPanel && currentPanel.querySelector(".sow-create__scroll")
                : currentPanel;
            if (currentScrollOwner) scrollTop = currentScrollOwner.scrollTop;
        }

        if (!root.querySelector("[data-screen-stage]")) {
            root.innerHTML = renderFrame(screen);
            var initialPanel = activeScreenPanel();
            if (initialPanel) initialPanel.classList.add("is-active");
        } else {
            updateFrameChrome(screen);
            syncScreenPanel(screen, screenChanged);
        }
        syncOverlay("password", renderPasswordModal());
        syncOverlay("settings", settingsOpen ? renderSettings() : "");
        syncOverlay("auth", authModalOpen ? renderAuthModal() : "");
        syncOverlay("profile-detail", profileOpen ? renderProfileDetail() : "");
        syncOverlay("skin-picker", typeof renderSkinPickerModal === "function" ? renderSkinPickerModal() : "");
        syncOverlay("purchase", typeof renderPurchaseModal === "function" ? renderPurchaseModal() : "");

        if (sameScreen && isTyping) {
            var restored = null;
            if (activeRole) {
                restored = root.querySelector("[data-role='" + activeRole + "']");
            } else if (activeName) {
                restored = root.querySelector("[name='" + activeName + "']");
            }
            if (restored) {
                if (activeVal != null) restored.value = activeVal;
                try {
                    restored.focus();
                    if (selStart != null && selEnd != null) {
                        restored.setSelectionRange(selStart, selEnd);
                    }
                } catch (e) {}
            }
        }
        updateDynamic();
        if (screen === "store") loadStoreCatalog();
        if (sameScreen && scrollTop !== null) {
            var nextScrollOwner = screen === "create"
                ? activeScreenPanel() && activeScreenPanel().querySelector(".sow-create__scroll")
                : activeScreenPanel();
            if (nextScrollOwner) nextScrollOwner.scrollTop = scrollTop;
        }
        syncDropdowns();
        lastRenderKey = renderKey();
        updateLobbyViews();
    }

    function progressionFromState() {
        var xp = Math.max(0, Number(state && state.xp) || 0);
        return {
            xp: xp,
            level: Math.max(1, Number(state && state.level) || Math.floor(xp / 100) + 1),
            crowns: Math.max(0, Number(state && state.crowns) || 0),
            laurels: Math.max(0, Number(state && state.laurels) || 0),
            gems: Math.max(0, Number(state && state.gems) || 0)
        };
    }

    function pendingRewardReceipts() {
        return (state && state.reward_receipts || []).filter(function (receipt) {
            return receipt && receipt.id && receipt.status !== "presented" && !receipt.presented_at;
        });
    }

    function findRewardReceipt(id) {
        return (state && state.reward_receipts || []).find(function (receipt) { return receipt && receipt.id === id; });
    }

    function reducedRewardMotion() {
        return !!(state && state.settings && state.settings.reduced_motion) ||
            !!(window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
    }

    function writeProgression(values) {
        if (!values) return;
        displayedProgression = {
            xp: Math.max(0, Number(values.xp) || 0),
            level: Math.max(1, Number(values.level) || Math.floor((Number(values.xp) || 0) / 100) + 1),
            crowns: Math.max(0, Number(values.crowns) || 0),
            laurels: Math.max(0, Number(values.laurels) || 0),
            gems: Math.max(0, Number(values.gems) || 0)
        };
        var progression = root.querySelector("[data-progression]");
        if (!progression) return;
        var levelValue = progression.querySelector("[data-progression-level-value]");
        var xpValue = progression.querySelector("[data-progression-xp-value]");
        var xpFill = progression.querySelector("[data-progression-xp-fill]");
        var crownsValue = progression.querySelector("[data-progression-crowns-value]");
        var laurelsValue = progression.querySelector("[data-progression-laurels-value]");
        var gemsValue = progression.querySelector("[data-progression-gems-value]");
        if (levelValue) levelValue.textContent = displayedProgression.level;
        if (xpValue) xpValue.textContent = Math.floor(displayedProgression.xp) + " " + SOW_t("menu.xp");
        if (xpFill) xpFill.style.width = (displayedProgression.xp % 100) + "%";
        if (crownsValue) crownsValue.textContent = Math.round(displayedProgression.crowns);
        if (laurelsValue) laurelsValue.textContent = Math.round(displayedProgression.laurels);
        if (gemsValue) gemsValue.textContent = Math.round(displayedProgression.gems);
    }

    function ensureProgressionDisplay() {
        if (displayedProgression) return;
        var canonical = progressionFromState();
        var pending = pendingRewardReceipts().filter(function (receipt) { return !rewardAnimationShown.has(receipt.id); });
        if (pending.length) {
            var totals = pending.reduce(function (sum, receipt) {
                sum.xp += Math.max(0, Number(receipt.xp) || 0);
                sum.crowns += Math.max(0, Number(receipt.crowns) || 0);
                sum.laurels += Math.max(0, Number(receipt.laurels) || 0);
                return sum;
            }, { xp: 0, crowns: 0, laurels: 0 });
            canonical.xp = Math.max(0, canonical.xp - totals.xp);
            canonical.crowns = Math.max(0, canonical.crowns - totals.crowns);
            canonical.laurels = Math.max(0, canonical.laurels - totals.laurels);
            canonical.level = Math.floor(canonical.xp / 100) + 1;
        } else {
            var preview = state && state.exit_reward_preview;
            if (preview && preview.account_id === state.account_id && !rewardAnimationShown.has(preview.receipt_id)) {
                canonical.xp = Math.max(0, Number(preview.base_xp) || 0);
                canonical.level = Math.max(1, Number(preview.base_level) || 1);
                canonical.crowns = Math.max(0, Number(preview.base_crowns) || 0);
                canonical.laurels = Math.max(0, Number(preview.base_laurels) || 0);
            }
        }
        writeProgression(canonical);
    }

    function animateProgressionTo(target, duration, token) {
        var from = Object.assign({}, displayedProgression || progressionFromState());
        if (reducedRewardMotion() || duration <= 0) {
            writeProgression(target);
            if (token === rewardAnimationToken) progressionAnimationTarget = null;
            return Promise.resolve(true);
        }
        progressionAnimationTarget = Object.assign({}, target);
        var start = performance.now();
        return new Promise(function (resolve) {
            var frameId = 0;
            function frame(now) {
                if (token !== rewardAnimationToken) {
                    if (progressionAnimationFrame === frameId) progressionAnimationFrame = 0;
                    resolve(false);
                    return;
                }
                if (root.hidden || document.visibilityState !== "visible") {
                    progressionAnimationFrame = 0;
                    progressionAnimationTarget = null;
                    writeProgression(progressionFromState());
                    resolve(false);
                    return;
                }
                var t = Math.min(1, (now - start) / duration);
                var eased = 1 - Math.pow(1 - t, 3);
                var currentXp = from.xp + (target.xp - from.xp) * eased;
                writeProgression({
                    xp: currentXp,
                    level: Math.floor(currentXp / 100) + 1,
                    crowns: from.crowns + (target.crowns - from.crowns) * eased,
                    laurels: from.laurels + (target.laurels - from.laurels) * eased,
                    gems: from.gems + (target.gems - from.gems) * eased
                });
                if (t < 1) {
                    frameId = requestAnimationFrame(frame);
                    progressionAnimationFrame = frameId;
                }
                else {
                    if (progressionAnimationFrame === frameId) progressionAnimationFrame = 0;
                    writeProgression(target);
                    progressionAnimationTarget = null;
                    if (!activeRewardStage && duration >= 900) {
                        if (from.xp !== target.xp) pulseRewardCounter("[data-progression-xp-value]", false);
                        if (from.crowns !== target.crowns) pulseRewardCounter("[data-progression-crowns-value]", false);
                        if (from.laurels !== target.laurels) pulseRewardCounter("[data-progression-laurels-value]", false);
                        if (from.gems !== target.gems) pulseRewardCounter("[data-progression-gems-value]", false);
                        if (target.level > from.level) pulseRewardCounter("[data-progression-level-value]", true);
                    }
                    resolve(true);
                }
            }
            frameId = requestAnimationFrame(frame);
            progressionAnimationFrame = frameId;
        });
    }

    function removeRewardStage() {
        var layer = activeRewardStage;
        activeRewardStage = null;
        if (!layer) return;
        if (typeof layer.getAnimations === "function") layer.getAnimations({ subtree: true }).forEach(function (motion) { motion.cancel(); });
        layer.remove();
        if (layer._shell && "inert" in layer._shell) layer._shell.inert = layer._shellWasInert;
        if (layer._restoreFocus && layer._restoreFocus.isConnected && !root.hidden) layer._restoreFocus.focus({ preventScroll: true });
    }

    function cancelRewardAnimation() {
        rewardAnimationToken += 1;
        rewardAnimationRunning = false;
        progressionAnimationTarget = null;
        removeRewardStage();
    }

    function createRewardStage(stages, skip) {
        var layer = document.createElement("section");
        layer.className = "sow-menu__reward-stage";
        layer.setAttribute("role", "dialog");
        layer.setAttribute("aria-modal", "true");
        layer.setAttribute("aria-label", SOW_t("endgame.match_result"));
        var cards = document.createElement("div");
        cards.className = "sow-menu__reward-cards";
        var items = stages.map(function (stage, index) {
            var card = document.createElement("div");
            card.className = "sow-menu__reward-card sow-menu__reward-card--" + stage.kind;
            card.style.animationDelay = (120 + index * 140) + "ms";
            card.setAttribute("role", "group");
            card.setAttribute("aria-label", stage.amount + " " + SOW_t(stage.kind === "xp" ? "menu.xp" : stage.kind === "crowns" ? "store.crowns" : "store.laurels"));
            var medallion = document.createElement("span");
            medallion.className = "sow-menu__reward-medallion";
            medallion.setAttribute("aria-hidden", "true");
            var emblem = document.createElement(stage.kind === "xp" ? "span" : "img");
            emblem.className = "sow-menu__reward-emblem";
            if (stage.kind === "xp") emblem.textContent = "XP";
            else emblem.src = currencyAsset(stage.kind === "crowns" ? "crown" : "laurel");
            medallion.appendChild(emblem);
            var amount = document.createElement("strong");
            amount.className = "sow-menu__reward-amount";
            amount.setAttribute("aria-hidden", "true");
            amount.textContent = "+0";
            card.appendChild(medallion);
            card.appendChild(amount);
            cards.appendChild(card);
            return { stage: stage, card: card, amount: amount };
        });
        var close = document.createElement("button");
        close.className = "sow-menu__reward-skip sow-menu__icon-button";
        close.type = "button";
        close.textContent = "×";
        close.setAttribute("aria-label", SOW_t("menu.close"));
        close.addEventListener("click", skip);
        layer.addEventListener("keydown", function (event) {
            if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); skip(); }
        });
        layer.appendChild(cards);
        layer.appendChild(close);
        layer._shell = root.querySelector(".sow-menu__shell");
        layer._shellWasInert = layer._shell && layer._shell.inert;
        layer._restoreFocus = document.activeElement;
        root.appendChild(layer);
        activeRewardStage = layer;
        if (layer._shell && "inert" in layer._shell) layer._shell.inert = true;
        close.focus({ preventScroll: true });
        return items;
    }

    function revealRewardAmounts(items, token) {
        var start = performance.now();
        return new Promise(function (resolve) {
            function frame(now) {
                if (token !== rewardAnimationToken || root.hidden || document.visibilityState !== "visible") { resolve(false); return; }
                var elapsed = now - start;
                items.forEach(function (item, index) {
                    var t = Math.max(0, Math.min(1, (elapsed - 300 - index * 140) / 650));
                    var value = "+" + Math.round(item.stage.amount * (1 - Math.pow(1 - t, 3)));
                    if (item.amount.textContent !== value) item.amount.textContent = value;
                });
                if (elapsed < 1450) requestAnimationFrame(frame);
                else resolve(true);
            }
            requestAnimationFrame(frame);
        });
    }

    function flyRewardCard(item, token) {
        var target = root.querySelector(item.stage.selector);
        if (!target || typeof item.card.animate !== "function") { item.card.style.visibility = "hidden"; return Promise.resolve(true); }
        var from = item.card.getBoundingClientRect();
        var to = target.getBoundingClientRect();
        var dx = to.left + to.width / 2 - from.left - from.width / 2;
        var dy = to.top + to.height / 2 - from.top - from.height / 2;
        var bend = Math.min(160, Math.max(70, Math.hypot(dx, dy) * .2));
        var controlX = dx / 2 + (dx < 0 ? -bend * .6 : bend * .6);
        var controlY = dy / 2 - bend;
        var path = [];
        for (var i = 0; i <= 12; i++) {
            var t = i / 12;
            var inverse = 1 - t;
            var x = 2 * inverse * t * controlX + t * t * dx;
            var y = 2 * inverse * t * controlY + t * t * dy;
            path.push({
                offset: t,
                transform: "translate3d(" + x + "px," + y + "px,0) scale(" + (1 - .82 * t) + ") rotate(" + (Math.sin(Math.PI * t) * 8) + "deg)",
                opacity: 1 - .4 * Math.max(0, (t - .8) / .2)
            });
        }
        return new Promise(function (resolve) {
            var motion = item.card.animate(path, { duration: 520, easing: "cubic-bezier(.45,0,.55,1)", fill: "forwards" });
            motion.onfinish = function () { item.card.style.visibility = "hidden"; resolve(token === rewardAnimationToken); };
            motion.oncancel = function () { resolve(false); };
        });
    }

    function pulseRewardCounter(selector, levelUp) {
        if (reducedRewardMotion()) return;
        var counter = root.querySelector(selector);
        if (!counter) return;
        var className = levelUp ? "is-reward-level-up" : "is-reward-value-hit";
        counter.classList.remove(className);
        void counter.offsetWidth;
        counter.classList.add(className);
        window.setTimeout(function () { counter.classList.remove(className); }, levelUp ? 900 : 640);
    }

    function acknowledgeShownReceipts(accountId) {
        if (!state || !accountId || state.account_id !== accountId) return;
        var now = Date.now();
        var ids = pendingRewardReceipts().filter(function (receipt) {
            return rewardAnimationShown.has(receipt.id) && now - (rewardAckTimes[receipt.id] || 0) >= 30000;
        }).map(function (receipt) { return receipt.id; });
        if (ids.length) {
            ids.forEach(function (id) { rewardAckTimes[id] = now; });
            send("acknowledge_reward_receipts", { account_id: accountId, receipt_ids: ids });
            if (rewardAckRetryTimer === null) {
                rewardAckRetryTimer = window.setTimeout(function () {
                    rewardAckRetryTimer = null;
                    if (document.visibilityState === "visible" && state && state.phase === "MainMenu") acknowledgeShownReceipts(accountId);
                }, 30000);
            }
        }
    }

    function runRewardPresentation(receipts, preview) {
        if (rewardAnimationRunning) return;
        rewardAnimationRunning = true;
        var token = ++rewardAnimationToken;
        var accountId = state && state.account_id;
        var ids = receipts.map(function (receipt) { return receipt.id; });
        var totals = receipts.reduce(function (sum, receipt) {
            sum.xp += Math.max(0, Number(receipt.xp) || 0);
            sum.crowns += Math.max(0, Number(receipt.crowns) || 0);
            sum.laurels += Math.max(0, Number(receipt.laurels) || 0);
            return sum;
        }, { xp: 0, crowns: 0, laurels: 0 });
        var canonical = progressionFromState();
        var target = receipts.length ? canonical : {
            xp: Math.max(0, Number(preview.base_xp) || 0) + Math.max(0, Number(preview.xp) || 0),
            level: 1,
            crowns: Math.max(0, Number(preview.base_crowns) || 0) + Math.max(0, Number(preview.crowns) || 0),
            laurels: Math.max(0, Number(preview.base_laurels) || 0) + Math.max(0, Number(preview.laurels) || 0),
            gems: canonical.gems
        };
        target.level = Math.floor(target.xp / 100) + 1;
        if (!receipts.length) {
            totals = { xp: Number(preview.xp) || 0, crowns: Number(preview.crowns) || 0, laurels: Number(preview.laurels) || 0 };
        }
        var stages = [
            { kind: "xp", amount: totals.xp, selector: "[data-progression-xp-value]" },
            { kind: "crowns", amount: totals.crowns, selector: "[data-progression-crowns-value]" },
            { kind: "laurels", amount: totals.laurels, selector: "[data-progression-laurels-value]" }
        ].filter(function (stage) { return stage.amount > 0; });

        function finish(shown) {
            if (token !== rewardAnimationToken || !state || state.account_id !== accountId) return;
            rewardAnimationToken += 1;
            rewardAnimationRunning = false;
            removeRewardStage();
            if (!shown) {
                writeProgression(progressionFromState());
                if (document.visibilityState === "visible") maybePresentRewards();
                return;
            }
            if (receipts.length) writeProgression(progressionFromState());
            else {
                target.gems = progressionFromState().gems;
                writeProgression(target);
            }
            if (receipts.length) {
                ids.forEach(function (id) { rewardAnimationShown.add(id); });
                acknowledgeShownReceipts(accountId);
            } else {
                rewardAnimationShown.add(preview.receipt_id);
                rewardOptimisticPreview = { receipt_id: preview.receipt_id, target: target };
                send("acknowledge_reward_presentation", { account_id: accountId, receipt_id: preview.receipt_id });
            }
            maybePresentRewards();
        }

        function complete(shown) {
            if (!shown || !activeRewardStage) { finish(shown); return; }
            activeRewardStage.classList.add("is-exiting");
            window.setTimeout(function () { finish(true); }, 240);
        }

        function next(index) {
            if (token !== rewardAnimationToken || root.hidden || document.visibilityState !== "visible") {
                finish(false);
                return;
            }
            if (index >= stages.length) {
                var latest = receipts.length ? progressionFromState() : Object.assign({}, target, { gems: progressionFromState().gems });
                var current = displayedProgression || progressionFromState();
                var changed = latest.xp !== current.xp || latest.level !== current.level || latest.crowns !== current.crowns ||
                    latest.laurels !== current.laurels || latest.gems !== current.gems;
                if (changed) animateProgressionTo(latest, 900, token).then(complete);
                else complete(true);
                return;
            }
            var item = items[index];
            var stage = item.stage;
            var nextValues = Object.assign({}, displayedProgression || progressionFromState());
            var oldLevel = Math.floor(nextValues.xp / 100) + 1;
            nextValues[stage.kind] = target[stage.kind];
            nextValues.level = Math.floor(nextValues.xp / 100) + 1;
            flyRewardCard(item, token).then(function (arrived) {
                if (!arrived) { finish(false); return; }
                animateProgressionTo(nextValues, 380, token).then(function (shown) {
                    if (!shown) { finish(false); return; }
                    pulseRewardCounter(stage.selector, false);
                    if (stage.kind === "xp" && nextValues.level > oldLevel) {
                        pulseRewardCounter("[data-progression-level-value]", true);
                        window.setTimeout(function () { next(index + 1); }, 250);
                    } else next(index + 1);
                });
            });
        }
        if (reducedRewardMotion() || !stages.length) { finish(true); return; }
        var items = createRewardStage(stages, function () { finish(true); });
        revealRewardAmounts(items, token).then(function (shown) { if (shown) next(0); else finish(false); });
    }

    function maybePresentRewards() {
        if (!state || state.phase !== "MainMenu" || root.hidden || document.visibilityState !== "visible" ||
            !rewardPresentationReady || rewardAnimationRunning) return;
        ensureProgressionDisplay();
        var preview = state.exit_reward_preview;
            if (rewardOptimisticPreview) {
            var receipt = findRewardReceipt(rewardOptimisticPreview.receipt_id);
            if (receipt) {
                var accountId = state.account_id;
                rewardAnimationShown.add(receipt.id);
                rewardOptimisticPreview = null;
                rewardAnimationRunning = true;
                var reconcileToken = ++rewardAnimationToken;
                animateProgressionTo(progressionFromState(), 900, reconcileToken).then(function (shown) {
                    if (reconcileToken !== rewardAnimationToken || !state || state.account_id !== accountId) return;
                    rewardAnimationRunning = false;
                    if (shown) acknowledgeShownReceipts(accountId);
                    maybePresentRewards();
                });
                return;
            }
        }
        if (preview && preview.account_id === state.account_id && !rewardAnimationShown.has(preview.receipt_id)) {
            runRewardPresentation([], preview);
            return;
        }
        var pending = pendingRewardReceipts().filter(function (receipt) { return !rewardAnimationShown.has(receipt.id); });
        if (pending.length) {
            runRewardPresentation(pending, null);
            return;
        }
        acknowledgeShownReceipts(state.account_id);
    }

    function updateDynamic() {
        if (!state || root.hidden) return;
        ensureProgressionDisplay();
        if (reducedRewardMotion() && (rewardAnimationRunning || progressionAnimationTarget)) {
            cancelRewardAnimation();
            writeProgression(progressionFromState());
        }
        var pendingRewards = pendingRewardReceipts().filter(function (receipt) { return !rewardAnimationShown.has(receipt.id); });
        var activePreview = state.exit_reward_preview &&
            state.exit_reward_preview.account_id === state.account_id &&
            !rewardAnimationShown.has(state.exit_reward_preview.receipt_id);
        var holdBundleGems = purchaseModal && purchaseModal.type === "bundle" && !purchaseModal.gemPresented;
        if (!rewardAnimationRunning && !pendingRewards.length && !rewardOptimisticPreview && !activePreview) {
            var canonical = progressionFromState();
            var changed = canonical.xp !== displayedProgression.xp || canonical.level !== displayedProgression.level ||
                canonical.crowns !== displayedProgression.crowns || canonical.laurels !== displayedProgression.laurels || canonical.gems !== displayedProgression.gems;
            var sameTarget = progressionAnimationTarget && canonical.xp === progressionAnimationTarget.xp &&
                canonical.level === progressionAnimationTarget.level && canonical.crowns === progressionAnimationTarget.crowns &&
                canonical.laurels === progressionAnimationTarget.laurels && canonical.gems === progressionAnimationTarget.gems;
            if (changed && !sameTarget && !holdBundleGems) {
                if (reducedRewardMotion() || root.hidden || document.visibilityState !== "visible") writeProgression(canonical);
                else animateProgressionTo(canonical, 900, ++rewardAnimationToken);
            }
        }
        var panel = activeScreenPanel() || root;
        var settings = state.settings || {};
        var musicInput = root.querySelector("[data-setting='music_volume']");
        var motionInput = root.querySelector("[data-setting='reduced_motion']");
        if (musicInput && document.activeElement !== musicInput) musicInput.value = settings.music_volume == null ? 0.8 : settings.music_volume;
        if (motionInput && document.activeElement !== motionInput) motionInput.value = settings.reduced_motion ? "reduced" : "full";
        var musicValBadge = root.querySelector("[data-val-for='music_vol']");
        if (musicValBadge && musicInput) musicValBadge.textContent = Math.round(Number(musicInput.value) * 100) + "%";
        var timer = panel.querySelector("[data-live-countdown]");
        var lobby = joinedLobby();
        if (timer && lobby) timer.textContent = lobby.is_counting_down ? SOW_t("lobbies.starting_in", { seconds: Math.ceil(lobby.timer_secs) }) : SOW_t("lobbies.waiting_for_players");
        updateQueueRoster(lobby);
        var cardStatuses = panel.querySelectorAll("[data-lobby-status-for]");
        for (var i = 0; i < cardStatuses.length; i++) {
            var status = cardStatuses[i];
            var cardLobby = findLobby(Number(status.dataset.lobbyStatusFor));
            status.textContent = cardLobby ? lobbyStatusText(cardLobby) : "";
            var card = status.closest("[data-lobby-card]");
            var count = card && card.querySelector("[data-lobby-count]");
            if (count) count.textContent = cardLobby ? lobbyPlayerCount(cardLobby) : "";
            if (cardLobby && card) card.setAttribute("aria-label", lobbyLabel(cardLobby));
        }
    }

    root.addEventListener("click", function (event) {
        var purchaseOverlay = event.target.closest("[data-menu-overlay='purchase']");
        if (purchaseOverlay && event.target === purchaseOverlay) {
            if (typeof canDismissPurchaseModal === "function" && !canDismissPurchaseModal()) return;
            if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
            else {
                purchaseModal = null;
                purchaseIntent = null;
            }
            render();
            return;
        }
        var skinPickerOverlay = event.target.closest("[data-menu-overlay='skin-picker']");
        if (skinPickerOverlay && event.target === skinPickerOverlay) {
            skinPickerOpen = false;
            render();
            return;
        }
        var settingsOverlay = event.target.closest("[data-menu-overlay='settings']");
        if (settingsOverlay && event.target === settingsOverlay) {
            settingsOpen = false;
            signOutConfirmOpen = false;
            dropdownOpenKey = null;
            render();
            return;
        }
        var target = event.target.closest("[data-command]");
        if (!target || !root.contains(target)) return;
        var command = target.dataset.command;
        if (command === "open_campaign") {
            profileOpen = false;
            profileAccountId = null;
            profileMatchDetail = null;
            heroesOpen = false;
            storeOpen = false;
            campaignOpen = true;
            render();
            return;
        }
        if (command === "close_campaign") {
            campaignOpen = false;
            render();
            return;
        }
        if (command === "start_campaign_episode") {
            var episodeId = target.dataset.episodeId;
            if (!episodeId) return;
            campaignOpen = false;
            if (typeof window.SOW_startCampaignEpisode === "function") {
                window.SOW_startCampaignEpisode(episodeId);
            }
            render();
            return;
        }
        /* POKI_SHARED_STORE_ACTIONS_BEGIN */
        if (command === "open_product_purchase") {
            openProductPurchase(target.dataset.productId, target.dataset.leaderId, target.dataset.priceLabel, target.dataset.skinId);
            return;
        }
        if (command === "buy_product") {
            beginStorePurchase(target.dataset.productId);
            return;
        }
        if (command === "restore_purchases") {
            beginStoreRestore();
            return;
        }
        if (command === "close_store_checkout") {
            closeStoreCheckout();
            return;
        }
        /* POKI_SHARED_STORE_ACTIONS_END */
        if (command === "main_nav") {
            var navScreen = target.dataset.navScreen || "battle";
            if (storeCheckoutInstance && typeof storeCheckoutInstance.destroy === "function") {
                storeCheckoutInstance.destroy();
            }
            storeCheckoutInstance = null;
            storeCheckoutProduct = null;
            storeCheckoutRequestId = null;
            storeCheckoutBusy = false;
            purchaseModal = null;
            purchaseIntent = null;
            settingsOpen = false;
            signOutConfirmOpen = false;
            campaignOpen = false;
            passwordLobbyId = null;
            passwordDraft = "";
            tempSelectedLeader = null;
            if (navScreen === "heroes") {
                profileOpen = false;
                profileAccountId = null;
                profileMatchDetail = null;
                profileDetailLoading = false;
                profileDetailRequestKey = null;
                storeOpen = false;
                heroesOpen = true;
                heroesSearchQuery = "";
                heroesRegionFilter = "all";
                dropdownOpenKey = null;
                tempSelectedLeader = state ? state.selected_leader : "Caesar";
                render();
                return;
            }
            if (navScreen === "profile") {
                storeOpen = false;
                if (!profileOpen || profileAccountId !== (state && state.account_id)) {
                    openProfile(null);
                } else {
                    render();
                }
                return;
            }
            if (navScreen === "store") {
                profileOpen = false;
                heroesOpen = false;
                storeOpen = true;
                render();
                return;
            }
            profileOpen = false;
            profileAccountId = null;
            profileMatchDetail = null;
            profileDetailLoading = false;
            profileDetailRequestKey = null;
            heroesOpen = false;
            storeOpen = false;
            if (state.show_browser || state.show_create) {
                send("close_overlay");
            } else {
                render();
            }
            return;
        }
        if (command === "open_profile" || command === "open_public_profile") {
            storeOpen = false;
            heroesOpen = false;
            campaignOpen = false;
            openProfile(target.dataset.accountId || null);
            return;
        }
        if (command === "close_profile") {
            profileOpen = false;
            storeOpen = false;
            heroesOpen = false;
            profileAccountId = null;
            profileData = null;
            profileSearchResults = [];
            profileRatings = null;
            profileMatchDetail = null;
            profileDetailLoading = false;
            profileDetailRequestKey = null;
            render();
            return;
        }
        if (command === "profile_tab") {
            profileTab = target.dataset.profileTab || "overview";
            render();
            if (profileTab === "history") {
                var historyEntry = activeProfileEntry();
                if (historyEntry && (historyEntry.historyHasMore || historyEntry.historyError || !historyEntry.historyLoaded || historyEntry.stale)) loadMoreProfileHistory();
            } else if (profileTab === "ranked") {
                loadProfileRatings();
            } else if (profileTab === "victories") {
                loadVictoryLeaderboard();
            }
            return;
        }
        if (command === "open_play_games") {
            if (typeof window.SOW_openAndroidPlayGames === "function") {
                window.SOW_openAndroidPlayGames(target.dataset.playGamesSection || "");
            }
            return;
        }
        if (command === "load_profile_more") {
            loadMoreProfileHistory();
            return;
        }
        if (command === "retry_profile") {
            invalidateProfileCache(profileAccountId);
            profileSnapshotLoading = false;
            profileHistoryLoading = false;
            profileRatingsLoading = false;
            loadProfile(profileAccountId);
            syncProfileDom();
            return;
        }
        if (command === "open_match") {
            var matchId = target.dataset.matchId;
            var detailEntry = activeProfileEntry();
            if (!matchId || !detailEntry || profileDetailLoading) return;
            if (detailEntry.details[matchId]) {
                profileDetailId = matchId;
                profileDetailError = "";
                profileDetailRequestKey = null;
                profileMatchDetail = detailEntry.details[matchId];
                render();
                return;
            }
            var detailProfileId = profileAccountId;
            var detailRequestKey = detailProfileId + ":" + matchId;
            if (detailEntry.detailRequests[matchId]) {
                profileDetailId = matchId;
                profileDetailError = "";
                profileDetailLoading = true;
                profileDetailRequestKey = detailRequestKey;
                return;
            }
            profileDetailId = matchId;
            profileDetailError = "";
            profileMatchDetail = null;
            profileDetailLoading = true;
            profileDetailRequestKey = detailRequestKey;
            detailEntry.detailRequests[matchId] = fetch(profileApi("/matches/" + encodeURIComponent(matchId)), {
                headers: { "Accept": "application/json" }
            }).then(function (response) {
                if (!response.ok) throw new Error("match detail failed");
                return response.json();
            }).then(function (detail) {
                detailEntry.details[matchId] = detail;
                if (profileOpen && profileAccountId === detailProfileId) {
                    profileDetailError = "";
                    profileMatchDetail = detail;
                }
            }).catch(function () {
                if (profileOpen && profileAccountId === detailProfileId) profileDetailError = SOW_t("profile.match_details_unavailable");
            }).finally(function () {
                delete detailEntry.detailRequests[matchId];
                if (profileDetailRequestKey === detailRequestKey) {
                    profileDetailLoading = false;
                    profileDetailRequestKey = null;
                }
                if (profileOpen && profileAccountId === detailProfileId && profileDetailId === matchId) render();
            });
            return;
        }
        if (command === "close_match") {
            profileMatchDetail = null;
            profileDetailError = "";
            profileDetailId = null;
            profileDetailLoading = false;
            profileDetailRequestKey = null;
            render();
            return;
        }
        if (command === "open_report") {
            reportOpen = true;
            reportTarget = target.dataset.accountId || profileAccountId;
            reportSent = false;
            render();
            return;
        }
        if (command === "cancel_report") {
            reportOpen = false;
            reportTarget = null;
            render();
            return;
        }
        if (command === "request_delete") {
            deleteArmed = true;
            render();
            return;
        }
        if (command === "cancel_delete") {
            deleteArmed = false;
            render();
            return;
        }
        if (command === "confirm_delete") {
            var delCreds = selfCreds();
            if (!delCreds || deleteBusy) {
                deleteArmed = false;
                render();
                return;
            }
            deleteBusy = true;
            render();
            fetch(profileApi("/profile/anonymous/delete"), {
                method: "POST",
                headers: { "Content-Type": "application/json", "Accept": "application/json" },
                body: JSON.stringify({ account_id: delCreds.account_id, auth_secret: delCreds.auth_secret })
            }).then(function (response) {
                if (!response.ok) throw new Error("delete failed");
                try {
                    ["sow_account_id", "sow_account_secret", "sow_player_progress", "poki_ignore_sow_pending_display_name"].forEach(function (k) {
                        window.localStorage.removeItem(k);
                    });
                } catch (e) {}
                window.location.reload();
            }).catch(function () {
                deleteBusy = false;
                deleteArmed = false;
                    state.error = SOW_t("menu.account_deletion_unavailable");
                render();
            });
            return;
        }
        if (command === "open_leader_picker") {
            profileOpen = false;
            profileAccountId = null;
            profileMatchDetail = null;
            profileSearchResults = [];
            storeOpen = false;
            campaignOpen = false;
            heroesOpen = true;
            heroesSearchQuery = "";
            heroesRegionFilter = "all";
            dropdownOpenKey = null;
            tempSelectedLeader = state ? state.selected_leader : "Caesar";
            settingsOpen = false;
            signOutConfirmOpen = false;
            render();
            return;
        }
        if (command === "toggle_dropdown") {
            var dropdownKey = target.dataset.dropdownKey;
            setDropdownOpen(dropdownKey, dropdownOpenKey !== dropdownKey, target);
            return;
        }
        if (command === "select_dropdown") {
            selectDropdown(target.dataset.dropdownKey, target.dataset.dropdownOptionValue);
            return;
        }
        if (command === "preview_leader") {
            tempSelectedLeader = target.dataset.leaderId;
            if (!updateHeroesPreview()) render();
            return;
        }
        if (command === "confirm_leader") {
            var leaderId = target.dataset.leaderId || tempSelectedLeader;
            var leader = leaderById(leaderId);
            if (leaderId && leader.available !== false) {
                send("set_leader", { leader_id: leaderId });
                heroesOpen = false;
                heroesSearchQuery = "";
                heroesRegionFilter = "all";
                tempSelectedLeader = null;
                render();
            }
            return;
        }
        /* POKI_SHARED_STORE_UNLOCK_BEGIN */
        if (command === "unlock_leader") {
            var unlockLeaderId = target.dataset.leaderId;
            var unlockCurrency = target.dataset.currency || "crowns";
            if (unlockLeaderId) openLeaderPurchase(unlockLeaderId, unlockCurrency);
            return;
        }
        if (command === "open_skin_purchase") {
            var purchaseSkinId = target.dataset.skinId;
            if (purchaseSkinId) openSkinPurchase(purchaseSkinId);
            return;
        }
        if (command === "open_skin_picker") {
            skinPickerOpen = true;
            loadStoreCatalog();
            render();
            return;
        }
        if (command === "close_skin_picker") {
            skinPickerOpen = false;
            render();
            return;
        }
        if (command === "confirm_purchase") {
            if (!purchaseModal || purchaseModal.submitted) return;
            purchaseModal.submitted = true;
            if (purchaseModal.type === "leader") {
                if (!(state && state.store_busy)) send("unlock_leader", { leader_id: purchaseModal.leaderId, currency: purchaseModal.currency });
            } else if (purchaseModal.type === "skin") {
                if (!(state && state.store_busy)) send("unlock_skin", { skin_id: purchaseModal.skinId });
            } else if (purchaseModal.type === "product") {
                if (!beginStorePurchase(purchaseModal.productId)) purchaseModal.submitted = false;
            }
            render();
            return;
        }
        if (command === "cancel_purchase") {
            if (storeCheckoutProduct || storeCheckoutInstance) closeStoreCheckout();
            else {
                if (typeof canDismissPurchaseModal === "function" && !canDismissPurchaseModal()) return;
                if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
                else {
                    purchaseModal = null;
                    purchaseIntent = null;
                }
                render();
            }
            return;
        }
        if (command === "unlock_skin" || command === "equip_skin") {
            var skinId = target.dataset.skinId;
            if (skinId && !(state && state.store_busy)) {
                send(command, { skin_id: skinId });
            }
            return;
        }
        /* POKI_SHARED_STORE_UNLOCK_END */
        if (command === "set_session_mode") {
            createOffline = target.dataset.mode === "offline";
            render();
            return;
        }
        if (command === "set_create_mode") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.game_mode = target.dataset.mode;
            render();
            return;
        }
        if (command === "set_create_diff") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.bot_difficulty = target.dataset.diff;
            render();
            return;
        }
        if (command === "set_create_spawn") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.random_spawn = target.dataset.spawn === "true";
            render();
            return;
        }
        if (command === "set_create_private") {
            createPrivate = target.dataset.private === "true";
            render();
            return;
        }
        if (command === "randomize_seed") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.seed = Math.floor(Math.random() * 9999) + 1;
            render();
            return;
        }
        if (command === "toggle_settings") {
            settingsOpen = !settingsOpen;
            if (!settingsOpen) signOutConfirmOpen = false;
            authModalOpen = false;
            render();
            return;
        }
        /* POKI_SHARED_AUTH_ACTIONS_BEGIN */
        if (command === "close_auth") {
            authModalOpen = false;
            render();
            return;
        }
        if (command === "auth_change_email") {
            authOtpSent = false;
            authCode = "";
            authError = "";
            authNotice = "";
            render();
            return;
        }
        if (command === "auth_resend") {
            requestAuthOtp();
            return;
        }
        if (command === "wou_provider") {
            if (typeof window.SOW_startWouOAuth === "function") window.SOW_startWouOAuth(target.dataset.provider);
            return;
        }
        if (command === "sign_in") {
            if (isAndroidTwa()) {
                send(command);
            } else {
                settingsOpen = false;
                signOutConfirmOpen = false;
                resetAuthFlow();
                authModalOpen = true;
                render();
            }
            return;
        }
        if (command === "sign_out") {
            signOutConfirmOpen = true;
            render();
            return;
        }
        if (command === "cancel_sign_out") {
            signOutConfirmOpen = false;
            render();
            return;
        }
        if (command === "confirm_sign_out") {
            signOutConfirmOpen = false;
            if (isAndroidTwa()) send("sign_out");
            else if (typeof window.SOW_signOutWou === "function") window.SOW_signOutWou();
            return;
        }
        /* POKI_SHARED_AUTH_ACTIONS_END */
        if (command === "close_password") {
            passwordLobbyId = null;
            passwordDraft = "";
            render();
            return;
        }
        if (command === "copy_lobby_code") {
            var lobbyCode = String(target.dataset.lobbyId || "");
            if (navigator.clipboard && lobbyCode) {
                navigator.clipboard.writeText(lobbyCode).then(function () {
                    target.textContent = SOW_t("menu.copied");
                }).catch(function () {
                    target.textContent = SOW_t("menu.copy_failed");
                });
            } else if (lobbyCode) {
                target.textContent = SOW_t("menu.copy_unavailable");
            }
            return;
        }
        if (command === "ban_player" && !window.confirm(SOW_t("auth.ban_player_confirm"))) {
            return;
        }
        if (command === "join_lobby") {
            var selectedLobby = findLobby(Number(target.dataset.lobbyId));
            if (selectedLobby && selectedLobby.has_password) {
                passwordLobbyId = selectedLobby.id;
                passwordDraft = "";
                render();
            } else {
                send("join_lobby", { lobby_id: Number(target.dataset.lobbyId) });
            }
            return;
        }
        if (command === "set_leader") {
            if (send("set_leader", { leader_id: target.dataset.leaderId })) {
                heroesOpen = false;
            }
            return;
        }
        var payload = {};
        if (target.dataset.lobbyId) payload.lobby_id = Number(target.dataset.lobbyId);
        if (target.dataset.playerId) payload.target_player_id = Number(target.dataset.playerId);
        send(command, payload);
    });

    document.addEventListener("pointerdown", function (event) {
        var target = event.target;
        var interactive = target && target.closest ? target.closest("[data-command], button, input, textarea, a, [role='button']") : null;
        if (interactive && root.contains(interactive)) activeMenuPointerId = event.pointerId;
        var dropdown = target && target.closest ? target.closest("[data-control-dropdown]") : null;
        if (dropdownOpenKey && (!dropdown || dropdown.dataset.dropdownKey !== dropdownOpenKey)) {
            setDropdownOpen(dropdownOpenKey, false);
        }
    });

    function finishMenuPointer(event) {
        if (activeMenuPointerId !== event.pointerId || renderFlushTimer !== null) return;
        renderFlushTimer = setTimeout(function () {
            renderFlushTimer = null;
            activeMenuPointerId = null;
            if (renderPending) render();
        }, 0);
    }

    document.addEventListener("pointerup", finishMenuPointer);
    document.addEventListener("pointercancel", finishMenuPointer);

    /* POKI_SHARED_AUTH_EVENTS_BEGIN */
    window.addEventListener("wou:auth-state-change", function () {
        if (!state || root.hidden) return;
        var auth = typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() || {} : {};
        if (auth.linked && authModalOpen) {
            authModalOpen = false;
            resetAuthFlow();
        }
        if (!root.querySelector("[data-screen-stage]")) {
            render();
            return;
        }
        updateFrameChrome(currentScreen());
        updateDynamic();
        if (settingsOpen) syncOverlay("settings", renderSettings());
        if (authModalOpen) syncOverlay("auth", renderAuthModal());
        syncProfileDom();
    });

    window.addEventListener("wou:auth-error", function (event) {
        authBusy = false;
        authModalOpen = true;
        authError = event.detail && event.detail.message ? event.detail.message : SOW_t("auth.sign_in_unavailable");
        render();
    });
    /* POKI_SHARED_AUTH_EVENTS_END */

    /* POKI_SHARED_STORE_EVENTS_BEGIN */
    window.addEventListener("sow:android-purchase-bridge-ready", function () {
        if (state && !root.hidden && currentScreen() === "store") render();
    });

    window.addEventListener("sow:android-purchase-result", function (event) {
        var result = event.detail || {};
        if (result.request_id && storeCheckoutRequestId &&
            result.request_id !== storeCheckoutRequestId) return;
        storeCheckoutBusy = false;
        if (!state) return;
        storeCheckoutRequestId = null;
        if (result.status === "success") {
            state.error = null;
            var attempt = typeof currentExternalPurchaseAttempt === "function" ? currentExternalPurchaseAttempt() : null;
            if (attempt && (!result.product_id || attempt.product_id === result.product_id)) {
                completeExternalPurchase(attempt.product_id);
            } else {
                purchaseIntent = null;
                send("refresh_profile");
            }
        } else if (result.status === "restored") {
            purchaseIntent = null;
            send("refresh_profile");
        } else if (result.status === "cancelled") {
            if (typeof abandonExternalPurchaseAttempt === "function") abandonExternalPurchaseAttempt(result.product_id);
            if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
            else {
                purchaseModal = null;
                purchaseIntent = null;
            }
            state.error = SOW_t("menu.purchase_cancelled");
        } else {
            if (typeof abandonExternalPurchaseAttempt === "function") abandonExternalPurchaseAttempt(result.product_id);
            if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
            else {
                purchaseModal = null;
                purchaseIntent = null;
            }
            state.error = SOW_t("menu.purchase_failed");
        }
        render();
    });
    /* POKI_SHARED_STORE_EVENTS_END */

    root.addEventListener("keydown", function (event) {
        var dropdownTrigger = event.target.closest("[data-role='dropdown-trigger']");
        var dropdownOption = event.target.closest("[data-role='dropdown-option'], [data-command='select_dropdown']");
        if (dropdownTrigger) {
            if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                var triggerKey = dropdownTrigger.dataset.dropdownKey;
                setDropdownOpen(triggerKey, true);
                var triggerOptions = dropdownTrigger.parentElement.querySelectorAll("[data-command='select_dropdown']");
                if (triggerOptions.length) triggerOptions[event.key === "ArrowUp" ? triggerOptions.length - 1 : 0].focus();
                return;
            }
        }
        if (dropdownOption) {
            var dropdown = dropdownOption.closest("[data-control-dropdown]");
            var options = dropdown ? Array.prototype.slice.call(dropdown.querySelectorAll("[data-command='select_dropdown']")) : [];
            var optionIndex = options.indexOf(dropdownOption);
            var nextIndex = optionIndex;
            if (event.key === "ArrowDown") nextIndex = Math.min(options.length - 1, optionIndex + 1);
            if (event.key === "ArrowUp") nextIndex = Math.max(0, optionIndex - 1);
            if (event.key === "Home") nextIndex = 0;
            if (event.key === "End") nextIndex = options.length - 1;
            if (nextIndex !== optionIndex) {
                event.preventDefault();
                options[nextIndex].focus();
                return;
            }
            if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                dropdownOption.click();
                return;
            }
        }
        if (event.key === "Escape" && dropdownOpenKey) {
            event.preventDefault();
            var openKey = dropdownOpenKey;
            setDropdownOpen(openKey, false, root.querySelector("[data-control-dropdown][data-dropdown-key='" + openKey + "'] [data-role='dropdown-trigger']"));
            return;
        }
        if (event.key === "Escape" && settingsOpen) {
            event.preventDefault();
            if (signOutConfirmOpen) {
                signOutConfirmOpen = false;
                render();
                return;
            }
            settingsOpen = false;
            signOutConfirmOpen = false;
            dropdownOpenKey = null;
            render();
            return;
        }
        if (event.key === "Escape" && purchaseModal) {
            event.preventDefault();
            if (storeCheckoutProduct || storeCheckoutInstance) closeStoreCheckout();
            else {
                if (typeof canDismissPurchaseModal === "function" && !canDismissPurchaseModal()) return;
                if (typeof dismissPurchaseModal === "function") dismissPurchaseModal();
                else {
                    purchaseModal = null;
                    purchaseIntent = null;
                }
                render();
            }
            return;
        }
        if (event.key === "Escape" && skinPickerOpen) {
            event.preventDefault();
            skinPickerOpen = false;
            render();
            return;
        }
        if (event.key === "Escape" && profileMatchDetail) {
            profileMatchDetail = null;
            render();
            return;
        }
        if (event.key === "Enter") {
            var input = event.target;
            if (input && input.dataset && input.dataset.role === "display-name") {
                event.preventDefault();
                input.blur();
                return;
            }
        }
        if (event.key !== "Enter" && event.key !== " ") return;
        var target = event.target.closest("[data-command='join_lobby']");
        if (!target || !root.contains(target)) return;
        event.preventDefault();
        target.click();
    });

    root.addEventListener("submit", function (event) {
        var form = event.target;
        /* POKI_SHARED_AUTH_SUBMIT_BEGIN */
        if (form.dataset.authForm === "request") {
            event.preventDefault();
            requestAuthOtp();
            return;
        }
        if (form.dataset.authForm === "verify") {
            event.preventDefault();
            verifyAuthOtp();
            return;
        }
        /* POKI_SHARED_AUTH_SUBMIT_END */
        if (form.dataset.form === "profile-search") {
            event.preventDefault();
            var query = form.elements.q.value.trim();
            fetch(profileApi("/profiles/search?q=" + encodeURIComponent(query) + "&limit=20"), {
                headers: { "Accept": "application/json" }
            }).then(function (response) {
                if (!response.ok) throw new Error("profile search failed");
                return response.json();
            }).then(function (data) {
                profileSearchResults = Array.isArray(data.items) ? data.items : [];
                profileSearchResults = profileSearchResults.filter(function (s) {
                    return !isBlockedAccountId(s && s.account_id);
                });
                profileSearchResults.forEach(function (summary) {
                    cacheProfileSeed(summary.account_id, profileSeedFromSummary(summary));
                });
            }).catch(function () {
                profileSearchResults = [];
            }).finally(function () {
                render();
            });
            return;
        }
        if (form.dataset.form === "report") {
            event.preventDefault();
            if (reportBusy) return;
            var repCreds = selfCreds();
            var reason = form.elements.reason ? form.elements.reason.value : "";
            var details = form.elements.details ? form.elements.details.value.trim() : "";
            if (!repCreds || !reportTarget) {
                reportOpen = false;
                render();
                return;
            }
            if (reason === "other" && !details) {
                render();
                return;
            }
            reportBusy = true;
            render();
            fetch(profileApi("/profile/anonymous/report"), {
                method: "POST",
                headers: { "Content-Type": "application/json", "Accept": "application/json" },
                body: JSON.stringify({
                    account_id: repCreds.account_id,
                    auth_secret: repCreds.auth_secret,
                    reported_account_id: reportTarget,
                    reason: reason,
                    details: details || null
                })
            }).then(function (response) {
                if (!response.ok) throw new Error("report failed");
                return response.json();
            }).then(function () {
                reportSent = true;
                reportOpen = false;
                try {
                    var list = window.SOW_BLOCKED_IDS || [];
                    if (list.indexOf(reportTarget) === -1) list.push(reportTarget);
                    window.SOW_BLOCKED_IDS = list;
                } catch (e) {}
            }).catch(function () {
            }).finally(function () {
                reportBusy = false;
                render();
            });
            return;
        }
        if (form.dataset.form === "join") {
            event.preventDefault();
            var code = form.elements.code.value.trim();
            send("join_code", { code: code });
        }
        if (form.dataset.form === "password") {
            event.preventDefault();
            passwordDraft = form.elements.password.value;
            if (passwordLobbyId != null && passwordDraft) {
                send("join_with_password", { lobby_id: passwordLobbyId, password: passwordDraft });
            }
        }
        if (form.dataset.form === "create") {
            event.preventDefault();
            syncCreateDraft(form);
            var config = createDraft || cloneConfig();
            if (createOffline) {
                send("start_single_player", { config: config });
            } else {
                send("create_game", { config: config, is_private: createPrivate, password: createPassword || null });
            }
        }
    });

    root.addEventListener("input", function (event) {
        var input = event.target;
        if (input.dataset && input.dataset.authField === "email") authEmail = input.value;
        if (input.dataset && input.dataset.authField === "code") authCode = input.value.replace(/\D/g, "").slice(0, 6);
        if (input.dataset && input.dataset.role === "browser-search") {
            browserSearchQuery = input.value;
            var publicPanel = root.querySelector(".sow-menu__public");
            if (publicPanel) {
                updateLobbyViews();
            }
        }
        if (input.dataset && input.dataset.role === "heroes-search") {
            heroesSearchQuery = input.value;
            refreshHeroesRoster(true);
        }
        var createForm = input.closest("form[data-form='create']");
        if (createForm) {
            syncCreateDraft(createForm);
            var valBadge = createForm.querySelector("[data-val-for='" + input.name + "']");
            if (valBadge) valBadge.textContent = input.value;
        }
        if (input.dataset && input.dataset.setting === "music_volume") {
            var musicValBadge = root.querySelector("[data-val-for='music_vol']");
            if (musicValBadge) musicValBadge.textContent = Math.round(Number(input.value) * 100) + "%";
        }
        if (input.name === "password" && input.closest("form[data-form='password']")) {
            passwordDraft = input.value;
        }
    });

    root.addEventListener("change", function (event) {
        var input = event.target;
        if (input.dataset && input.dataset.role === "display-name") {
            var name = input.value.trim();
            if (name && name !== state.player_name) send("save_display_name", { name: name });
            return;
        }
        var createForm = input.closest("form[data-form='create']");
        if (createForm) {
            syncCreateDraft(createForm);
            render();
            return;
        }
        if (input.dataset.setting === "locale") {
            if (typeof window.SOW_setLocale === "function") window.SOW_setLocale(input.value);
            return;
        }
        if (!input.dataset.setting) return;
        if (input.dataset.setting === "music_volume") send("set_music_volume", { value: Number(input.value) });
        if (input.dataset.setting === "reduced_motion") send("set_reduced_motion", { value: input.value === "reduced" });
    });

    function waitForMenuImage(url) {
        return new Promise(function (resolve) {
            var image = new Image();
            image.onload = function () {
                if (typeof image.decode !== "function") return resolve();
                try {
                    image.decode().then(resolve, resolve);
                } catch (error) {
                    resolve();
                }
            };
            image.onerror = resolve;
            image.src = url;
        });
    }

    function waitForExitMenuArt() {
        var waits = [waitForMenuImage(heroImage())];
        publicLobbies(true).forEach(function (lobby) {
            waits.push(preloadLobbyThumbnail(lobby).catch(function () {}));
        });
        return Promise.all(waits);
    }

    function syncWebLoaderForState(nextState) {
        if (typeof window.SOW_syncWebLoader !== "function") return;
        if (nextState.phase !== "MainMenu" || nextState.loader_job !== "ExitGame") {
            exitMenuAssetsToken += 1;
            exitMenuAssetsPending = false;
            exitMenuAssetsReady = false;
            window.SOW_syncWebLoader(nextState);
            return;
        }
        if (exitMenuAssetsReady) {
            window.SOW_syncWebLoader(nextState);
            return;
        }
        if (exitMenuAssetsPending) return;

        exitMenuAssetsPending = true;
        var token = ++exitMenuAssetsToken;
        waitForExitMenuArt().then(function () {
            if (token !== exitMenuAssetsToken) return;
            exitMenuAssetsPending = false;
            if (!state || state.phase !== "MainMenu" || state.loader_job !== "ExitGame") return;
            exitMenuAssetsReady = true;
            window.SOW_syncWebLoader(state);
        });
    }

    function scheduleRewardPresentation(delay) {
        if (!state || state.phase !== "MainMenu" || pendingExitScreenIntro || rewardPresentationReady || rewardAnimationTimer !== null) return;
        rewardAnimationTimer = window.setTimeout(function () {
            rewardAnimationTimer = null;
            if (!state || state.phase !== "MainMenu") return;
            rewardPresentationReady = true;
            maybePresentRewards();
        }, delay);
    }

    window.addEventListener("sow:loader-cycle-ready", function () {
        if (!pendingExitScreenIntro || !state || state.phase !== "MainMenu" || state.loader_job !== "ExitGame") return;
        pendingExitScreenIntro = false;
        previousScreen = null;
        render();
        scheduleRewardPresentation(250);
    });

    document.addEventListener("visibilitychange", function () {
        if (document.visibilityState !== "visible") {
            if (state && (rewardAnimationRunning || progressionAnimationTarget)) {
                cancelRewardAnimation();
                writeProgression(progressionFromState());
                var preview = state.exit_reward_preview;
                var unpresentedReceipt = pendingRewardReceipts().some(function (receipt) { return !rewardAnimationShown.has(receipt.id); });
                if (unpresentedReceipt || (preview && !rewardAnimationShown.has(preview.receipt_id))) displayedProgression = null;
            }
            return;
        }
        if (!state || state.phase !== "MainMenu") return;
        if (typeof resumeExternalPurchaseDelivery === "function") resumeExternalPurchaseDelivery(true);
        if (!rewardPresentationReady) scheduleRewardPresentation(0);
        else maybePresentRewards();
    });

    function handleMenuStateUpdate(raw) {
        if (typeof raw !== "string" || raw === lastRaw) {
            updateDynamic();
            return;
        }
        lastRaw = raw;
        try {
            state = JSON.parse(raw);
        } catch (error) {
            console.warn("[WEB MENU] invalid state:", error);
            return;
        }
        var accountId = state.account_id || "";
        if (rewardAnimationAccount !== accountId) {
            rewardAnimationAccount = accountId;
            rewardAnimationShown.clear();
            rewardAckTimes = Object.create(null);
            if (rewardAckRetryTimer !== null) window.clearTimeout(rewardAckRetryTimer);
            rewardAckRetryTimer = null;
            rewardOptimisticPreview = null;
            displayedProgression = null;
            rewardPresentationReady = false;
            cancelRewardAnimation();
            if (rewardAnimationTimer !== null) window.clearTimeout(rewardAnimationTimer);
            rewardAnimationTimer = null;
        }
        if (typeof resumeExternalPurchaseDelivery === "function") resumeExternalPurchaseDelivery(false);
        if (typeof resolvePurchaseModal === "function") resolvePurchaseModal();
        var returnedFromGame = lastMenuPhase !== null &&
            lastMenuPhase !== "MainMenu" &&
            state.phase === "MainMenu" &&
            state.loader_job === "ExitGame";
        lastMenuPhase = state.phase;
        if (state.phase !== "MainMenu") {
            rewardPresentationReady = false;
            if (rewardAnimationRunning) {
                cancelRewardAnimation();
                writeProgression(progressionFromState());
            }
            if (rewardAnimationTimer !== null) window.clearTimeout(rewardAnimationTimer);
            rewardAnimationTimer = null;
        }
        if (returnedFromGame) {
            profileOpen = false;
            heroesOpen = false;
            campaignOpen = false;
            storeOpen = false;
            pendingExitScreenIntro = true;
        }
        if (typeof window.SOW_tutorial_menu_state_update === "function") {
            window.SOW_tutorial_menu_state_update(state);
        }
        syncProfilePreload(state);
        if (state.phase === "MainMenu" && window.SOW_open_store_after_match) {
            window.SOW_open_store_after_match = false;
            storeOpen = true;
        }
        if (state.waiting && passwordLobbyId != null) {
            passwordLobbyId = null;
            passwordDraft = "";
        }
        var key = renderKey();
        if (key !== lastRenderKey) {
            render();
        } else {
            root.hidden = state.phase !== "MainMenu";
            updateLobbyViews();
            updateDynamic();
        }
        syncWebLoaderForState(state);
        if (state.phase === "MainMenu") {
            scheduleRewardPresentation(250);
            maybePresentRewards();
        }
    }

    window.addEventListener("sow:locale-change", function () {
        if (state && !root.hidden) render();
    });

    window.SOW_menu_state_update = handleMenuStateUpdate;
    root.hidden = true;
})();
