// Shared menu chrome, overlays, lifecycle, events, and state bridge.

    // ── Conduct & privacy (Terms/Privacy enforcement in-client) ──
    var REPORT_REASONS = [
        ["cheating", "Cheating / automation"],
        ["harassment", "Harassment"],
        ["hate_speech", "Hate speech"],
        ["threats", "Threats"],
        ["spam", "Spam"],
        ["inappropriate_name", "Inappropriate name"],
        ["exploiting", "Exploiting bugs"],
        ["other", "Other (explain below)"]
    ];
    var reportOpen = false;
    var reportTarget = null;
    var reportSent = false;
    var reportBusy = false;
    var deleteArmed = false;
    var deleteBusy = false;

    /* POKI_RENDER_REPLACEMENT_BEGIN */
    function renderTopbar() {
        var leader = leaderById(state.selected_leader);
        var name = displayNameDraft != null ? displayNameDraft : (state.player_name || "ANONYMOUS");
        var auth = typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() : { linked: false, pending: false };
        var accountXp = Math.max(0, Number(state.xp) || 0);
        var crowns = state.crowns == null ? state.laurels : state.crowns;
        return "" +
            "<header class='sow-menu__topbar'>" +
                "<div class='sow-menu__identity'>" +
                    "<button class='sow-menu__avatar' type='button' data-command='open_leader_picker' " +
                        "aria-label='Select leader' style=\"background-image:url('" + esc(avatarImage()) + "')\"></button>" +
                    "<div class='sow-menu__profile'>" +
                        "<input data-role='display-name' name='display_name' value=\"" + esc(name) + "\" maxlength='20' " +
                            (state.name_locked ? "readonly" : "") + " aria-label='Display name'>" +
                        "<button class='sow-menu__profile-link' type='button' data-command='open_profile'>" + esc(leader.name) + " · " + esc(leader.civilization) + "</button>" +
                    "</div>" +
                "</div>" +
                "<div class='sow-menu__top-actions'>" +
                    "<div class='sow-menu__progress' data-progression data-command='open_profile' role='button' tabindex='0' title='Open profile' aria-label='Open profile'>" +
                        "<span class='sow-menu__progress-cell sow-menu__level'><small>LV</small><strong data-progression-level-value>" + esc(state.level) + "</strong></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__xp'><span class='sow-menu__xp-value' data-progression-xp-value>" + esc(Math.floor(accountXp)) + " XP</span><span class='sow-menu__xp-track' aria-hidden='true'><i data-progression-xp-fill style='width:" + (accountXp % 100) + "%'></i></span></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__crowns'><img class='sow-menu__currency-icon' src='" + esc(currencyAsset("crown")) + "' alt='' aria-hidden='true'><strong data-progression-crowns-value>" + esc(crowns) + "</strong></span>" +
                    "</div>" +
                    (auth.linked || auth.pending ? "" : "<button class='sow-menu__signin' type='button' data-command='sign_in'>SIGN IN</button>") +
                    "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='Settings'>⚙</button>" +
                "</div>" +
            "</header>";
    }

    function renderCommandPanel() {
        return "" +
            "<section class='sow-menu__command'>" +
                "<div class='sow-menu__home-public'>" + renderPublicPanel("home") + "</div>" +
                "<div class='sow-menu__home-actions'>" +
                    "<button class='sow-menu__primary' type='button' data-command='quick_match'>QUICK MATCH <span>↗</span></button>" +
                    "<button class='sow-menu__secondary' type='button' data-command='open_campaign'>CAMPAIGN <span>⚔</span></button>" +
                    "<button class='sow-menu__secondary' type='button' data-command='open_browser'>LOBBY BROWSER <span>→</span></button>" +
                    "<form class='sow-menu__join' data-form='join'>" +
                        "<input name='code' inputmode='numeric' autocomplete='off' placeholder='LOBBY CODE' aria-label='Lobby code'>" +
                        "<button type='submit'>JOIN</button>" +
                    "</form>" +
                    "<button class='sow-menu__secondary' type='button' data-command='open_create'>CREATE CUSTOM GAME <span>+</span></button>" +
                    "<button class='sow-menu__secondary' type='button' data-command='main_nav' data-nav-screen='store'>SHOP <span>↗</span></button>" +
                    renderFeedback() +
                "</div>" +
            "</section>";
    }

    function renderFeedback() {
        var purchaseStatus = "";
        try {
            var purchase = new URLSearchParams(window.location.search).get("purchase");
            var purchaseMessages = {
                success: "Purchase received. Your gems may take a moment to appear.",
                restored: "Purchases restored.",
                cancelled: "Purchase cancelled.",
                error: "Purchase could not be completed."
            };
            if (purchaseMessages[purchase]) {
                purchaseStatus = "<div class='sow-menu__status sow-menu__status--notice'>" + esc(purchaseMessages[purchase]) + "</div>";
            }
        } catch (e) {}
        var error = state.error ? "<div class='sow-menu__status sow-menu__status--error'>" + esc(state.error) + "</div>" : "";
        var notice = state.notice ? "<div class='sow-menu__status sow-menu__status--notice'>" +
            esc({ host_left: "Host left the lobby", kicked: "You were removed from the lobby", banned: "You are banned from this lobby", connection_lost: "Connection lost" }[state.notice] || state.notice) +
            "</div>" : "";
        return purchaseStatus + error + notice;
    }

    function renderFooter(label) {
        var externalAttrs = isAndroidTwa() ? "" : " target='_blank' rel='noopener noreferrer'";
        return "<footer class='sow-menu__footer'>" + (label ? "<span data-menu-footer-label>" + esc(label) + "</span>" : "") + "<nav class='sow-menu__footer-links' aria-label='Game links'>" +
            "<a href='/how-to-play/'>HOW TO PLAY</a><a href='/support/'>SUPPORT</a><a href='/terms/'>TERMS</a><a href='/privacy/'>PRIVACY</a><a href='/cookies/'>COOKIES</a>" +
            "<a href='https://discord.gg/d6ZDeChSE'" + externalAttrs + ">DISCORD</a><a href='https://t.me/shadowsofwario'" + externalAttrs + ">TELEGRAM</a><a href='https://github.com/worldofunreal/shadows-of-war'" + externalAttrs + ">GITHUB</a>" +
            "</nav><span>SHADOWSOFWAR.IO</span></footer>";
    }

    function renderMainNav(active) {
        var items = [
            ["store", "shell/mobile-nav/store.webp", "Shop"],
            ["heroes", "shell/mobile-nav/heroes.webp", "Heroes"],
            ["battle", "shell/mobile-nav/battle.webp", "Battle"],
            ["profile", "shell/mobile-nav/profile.webp", "Profile"]
        ];
        return "<nav class='sow-menu__main-nav' aria-label='Main menu navigation'>" + items.map(function (item) {
            var selected = active === item[0];
            return "<button type='button' class='sow-menu__main-nav-item" + (selected ? " is-active" : "") + "' data-command='main_nav' data-nav-screen='" + item[0] + "'" +
                (selected ? " aria-current='page'" : "") + " aria-label='" + esc(item[2]) + "'><span aria-hidden='true'><img src='" + esc(asset(item[1])) + "' alt='' width='128' height='128' decoding='async' draggable='false'></span><small>" + item[2] + "</small></button>";
        }).join("") + "</nav>";
    }

    function renderSettings() {
        var settings = state.settings || {};
        var vol = settings.music_volume == null ? 0.8 : settings.music_volume;
        var volPct = Math.round(vol * 100);
        var auth = typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() : { platform: isAndroidTwa() ? "twa" : "web", linked: false, pending: false };
        var providers = (auth.linkedProviders || []).map(function (provider) {
            return String(provider).replace(/_/g, " ").toUpperCase();
        });
        var providerLabel = auth.platform === "twa" ? "GOOGLE PLAY GAMES" :
            auth.provider === "crazygames" ? "CRAZYGAMES" :
            providers.length ? "WOU-ID · " + providers[0] + (providers.length > 1 ? " +" + (providers.length - 1) : "") : "WOU-ID ACCOUNT";
        var accountControl = auth.pending
            ? "<section class='sow-menu__form-field sow-menu__form-field--wide sow-menu__account-row'><span>GOOGLE PLAY GAMES</span><strong class='sow-menu__account-pending'>CONNECTING…</strong></section>"
            : auth.linked
            ? "<section class='sow-menu__form-field sow-menu__form-field--wide sow-menu__account-row'><span>ACCOUNT · " + providerLabel + "</span>" + (auth.canSignOut ? "<button class='sow-menu__danger' type='button' data-command='sign_out'>SIGN OUT</button>" : "") + "</section>"
            : "";
        return "" +
            "<div class='sow-menu__overlay' data-menu-overlay='settings'>" +
                "<section class='sow-menu__modal sow-menu__settings-modal'>" +
                    "<div class='sow-menu__modal-head'>" +
                        "<div>" +
                            "<p class='sow-menu__panel-label'>SYSTEM CONFIGURATION</p>" +
                            "<h2>SETTINGS</h2>" +
                        "</div>" +
                        "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='Close'>×</button>" +
                    "</div>" +
                    "<div class='sow-menu__form-grid'>" +
                        accountControl +
                        "<label class='sow-menu__form-field sow-menu__form-field--wide'>" +
                            "<span>MASTER AUDIO</span>" +
                            renderDropdown({ key: "settings-mute", name: "mute_all", setting: "mute", value: settings.mute_all ? "off" : "on", options: [
                                { value: "on", label: "AUDIO ENABLED (ON)" },
                                { value: "off", label: "MUTED (OFF)" }
                            ] }) +
                        "</label>" +
                        "<label class='sow-menu__form-field sow-menu__form-field--wide'>" +
                            "<div class='sow-menu__slider-label'><span>MUSIC VOLUME</span><b data-val-for='music_vol'>" + volPct + "%</b></div>" +
                            "<input class='sow-menu__field' type='range' name='music_volume' min='0' max='1' step='0.05' value='" + esc(vol) + "' data-setting='music_volume'>" +
                        "</label>" +
                        "<label class='sow-menu__form-field sow-menu__form-field--wide'>" +
                            "<span>MOTION &amp; ANIMATION</span>" +
                            renderDropdown({ key: "settings-motion", name: "reduced_motion", setting: "reduced_motion", value: settings.reduced_motion ? "reduced" : "full", options: [
                                { value: "full", label: "FULL" },
                                { value: "reduced", label: "REDUCED MOTION" }
                            ] }) +
                        "</label>" +
                    "</div>" +
                    "<div class='sow-menu__modal-actions'>" +
                        "<button class='sow-menu__primary' type='button' data-command='toggle_settings'>DONE <span>✓</span></button>" +
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
            return { token: token, accountId: user && (user.account_id || user.id) || "", user: user };
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
        return [["google", "Google"], ["discord", "Discord"], ["twitter", "X"], ["meta", "Meta"]].map(function (provider) {
            return "<button class='sow-auth__provider' type='button' data-command='wou_provider' data-provider='" + provider[0] + "'><span class='sow-auth__provider-icon sow-auth__provider-icon--" + provider[0] + "'>" + authIcon(provider[0]) + "</span><span>Continue with " + provider[1] + "</span><i>↗</i></button>";
        }).join("");
    }

    function finishWouLogin(data) {
        if (!data || !data.session_token || !data.account) throw new Error("Identity service returned an incomplete session.");
        try {
            window.localStorage.setItem("wou_session_token", data.session_token);
            window.localStorage.setItem("wou_user_data", JSON.stringify(data.account));
        } catch (e) {
            throw new Error("Could not save the account on this device.");
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
            if (!response.ok) throw new Error(data.error || "Sign-in is unavailable right now.");
            return data;
        });
    }

    function requestAuthOtp() {
        if (authBusy) return;
        var email = String(authEmail || "").trim();
        if (!/^\S+@\S+\.\S+$/.test(email)) {
            authError = "Enter a valid email address.";
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
            authNotice = data.message || "Verification code sent.";
            render();
        }).catch(function (error) {
            authBusy = false;
            authError = error && error.message ? error.message : "Could not send the verification code.";
            render();
        });
    }

    function verifyAuthOtp() {
        if (authBusy) return;
        var code = String(authCode || "").trim();
        if (!/^\d{6}$/.test(code)) {
            authError = "Enter the six-digit code.";
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
            authError = error && error.message ? error.message : "That code is invalid or expired.";
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
            ? "<form class='sow-auth__form' data-auth-form='verify'><label>CODE SENT TO <strong>" + esc(authEmail) + "</strong></label><input class='sow-auth__code' data-auth-field='code' inputmode='numeric' autocomplete='one-time-code' maxlength='6' value='" + esc(authCode) + "' placeholder='000000' aria-label='Verification code' required><button class='sow-auth__submit sow-auth__submit--cyan' type='submit'" + (authBusy ? " disabled" : "") + ">" + (authBusy ? "CHECKING…" : "VERIFY CODE") + "</button><div class='sow-auth__form-links'><button type='button' data-command='auth_change_email'>CHANGE EMAIL</button><button type='button' data-command='auth_resend'" + (authBusy ? " disabled" : "") + ">RESEND</button></div></form>"
            : "<form class='sow-auth__form' data-auth-form='request'><label for='sow-auth-email'>EMAIL ADDRESS</label><input id='sow-auth-email' class='sow-auth__input' data-auth-field='email' type='email' autocomplete='email' value='" + esc(authEmail) + "' placeholder='you@example.com' required><button class='sow-auth__submit' type='submit'" + (authBusy ? " disabled" : "") + ">" + (authBusy ? "SENDING…" : "SEND CODE") + "</button></form>";
        var error = authError ? "<div class='sow-auth__message sow-auth__message--error' role='alert'>" + esc(authError) + "</div>" : "";
        var notice = authNotice ? "<div class='sow-auth__message sow-auth__message--notice' role='status'>" + esc(authNotice) + "</div>" : "";
        return "<div class='sow-menu__overlay' data-menu-overlay='auth' data-auth-overlay><section class='sow-menu__modal sow-menu__auth-modal sow-auth' role='dialog' aria-modal='true' aria-label='Account'>" +
            "<div class='sow-auth__glow sow-auth__glow--cyan'></div><div class='sow-auth__head'><div class='sow-auth__logos'><img class='sow-auth__game-logo' src='/sow-long.svg' alt='Shadows of War'><span class='sow-auth__logo-divider' aria-hidden='true'></span><img class='sow-auth__wou-logo' src='https://worldofunreal.com/wouid.svg' alt='WouID'></div><button class='sow-menu__icon-button' type='button' data-command='close_auth' aria-label='Close'>×</button></div>" +
            error + notice + "<div class='sow-auth__body'>" + emailPanel + "<div class='sow-auth__social-list'>" + renderAuthSocial() + "</div></div>" +
            "<a class='sow-auth__terms' href='/terms/'>Terms</a></section></div>";
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
        return ({ campaign: "CAMPAIGN", browser: "LOBBY BROWSER", create: "CREATE GAME", queue: "LOBBY", store: "SHOP", heroes: "HEROES", profile: "PROFILE" })[screen] || "";
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
        var name = displayNameDraft != null ? displayNameDraft : (state.player_name || "ANONYMOUS");
        if (nameInput && document.activeElement !== nameInput) nameInput.value = name;
        if (nameInput) nameInput.readOnly = !!state.name_locked;
        var avatar = topbar.querySelector(".sow-menu__avatar");
        var avatarUrl = avatarImage();
        if (avatar && avatar.dataset.avatarUrl !== avatarUrl) {
            avatar.style.backgroundImage = "url(" + JSON.stringify(avatarUrl) + ")";
            avatar.dataset.avatarUrl = avatarUrl;
        }
        var leaderLink = topbar.querySelector(".sow-menu__profile-link");
        if (leaderLink) leaderLink.textContent = leader.name + " · " + leader.civilization;
        var auth = typeof window.SOW_getAuthState === "function" ? window.SOW_getAuthState() || {} : {};
        var showSignIn = !(auth.linked || auth.pending);
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
            if (signIn) signIn.textContent = "SIGN IN";
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
        var screen = currentScreen();
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

    function updateDynamic() {
        if (!state || root.hidden) return;
        var panel = activeScreenPanel() || root;
        var progression = root.querySelector("[data-progression]");
        if (progression) {
            var progressionXp = Math.max(0, Number(state.xp) || 0);
            var progressionLevel = Math.max(1, Number(state.level) || 1);
            var levelValue = progression.querySelector("[data-progression-level-value]");
            var xpValue = progression.querySelector("[data-progression-xp-value]");
            var xpFill = progression.querySelector("[data-progression-xp-fill]");
            var crownsValue = progression.querySelector("[data-progression-crowns-value]");
            if (levelValue) levelValue.textContent = progressionLevel;
            if (xpValue) xpValue.textContent = Math.floor(progressionXp) + " XP";
            if (xpFill) xpFill.style.width = (progressionXp % 100) + "%";
            var crowns = state.crowns == null ? state.laurels : state.crowns;
            if (crownsValue) crownsValue.textContent = Math.max(0, Number(crowns) || 0);
        }
        var settings = state.settings || {};
        var muteInput = root.querySelector("[data-setting='mute']");
        var musicInput = root.querySelector("[data-setting='music_volume']");
        var motionInput = root.querySelector("[data-setting='reduced_motion']");
        if (muteInput && document.activeElement !== muteInput) muteInput.value = settings.mute_all ? "off" : "on";
        if (musicInput && document.activeElement !== musicInput) musicInput.value = settings.music_volume == null ? 0.8 : settings.music_volume;
        if (motionInput && document.activeElement !== motionInput) motionInput.value = settings.reduced_motion ? "reduced" : "full";
        var musicValBadge = root.querySelector("[data-val-for='music_vol']");
        if (musicValBadge && musicInput) musicValBadge.textContent = Math.round(Number(musicInput.value) * 100) + "%";
        var timer = panel.querySelector("[data-live-countdown]");
        var lobby = joinedLobby();
        if (timer && lobby) timer.textContent = lobby.is_counting_down ? "STARTING IN " + Math.ceil(lobby.timer_secs) + "s" : "WAITING FOR PLAYERS";
        updateQueueRoster(lobby);
        var cardTimers = panel.querySelectorAll("[data-timer-for]");
        for (var i = 0; i < cardTimers.length; i++) {
            var cardLobby = findLobby(Number(cardTimers[i].dataset.timerFor));
            cardTimers[i].textContent = cardLobby ? lobbyTimerText(cardLobby) : "";
        }
    }

    root.addEventListener("click", function (event) {
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
            send("start_campaign_episode", { episode_id: episodeId });
            render();
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
        if (command === "main_nav") {
            var navScreen = target.dataset.navScreen || "battle";
            if (storeCheckoutInstance && typeof storeCheckoutInstance.destroy === "function") {
                storeCheckoutInstance.destroy();
            }
            storeCheckoutInstance = null;
            storeCheckoutProduct = null;
            storeCheckoutRequestId = null;
            storeCheckoutBusy = false;
            settingsOpen = false;
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
                if (profileOpen && profileAccountId === detailProfileId) profileDetailError = "Match details unavailable.";
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
                    ["sow_account_id", "sow_account_secret", "sow_player_progress"].forEach(function (k) {
                        window.localStorage.removeItem(k);
                    });
                } catch (e) {}
                window.location.reload();
            }).catch(function () {
                deleteBusy = false;
                deleteArmed = false;
                state.error = "Account deletion unavailable. Try again or email hello@shadowsofwar.io.";
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
            if (leaderId) {
                send("set_leader", { leader_id: leaderId });
                heroesOpen = false;
                heroesSearchQuery = "";
                heroesRegionFilter = "all";
                tempSelectedLeader = null;
                render();
            }
            return;
        }
        if (command === "unlock_leader") {
            var unlockLeaderId = target.dataset.leaderId;
            var unlockCurrency = target.dataset.currency || "crowns";
            var unlockAccountId = state && state.account_id;
            var unlockAuth = storeAuth();
            if (!unlockLeaderId || !unlockAccountId || !unlockAuth.available) {
                state.error = "Account setup is required before unlocking a leader.";
                render();
                return;
            }
            target.disabled = true;
            fetch(profileApi("/store/leaders/unlock"), {
                method: "POST",
                headers: unlockAuth.headers,
                body: JSON.stringify({
                    account_id: unlockAccountId,
                    auth_secret: unlockAuth.authSecret,
                    leader_id: unlockLeaderId,
                    currency: unlockCurrency
                })
            }).then(function (response) {
                if (!response.ok) throw new Error("unlock failed");
                window.location.reload();
            }).catch(function () {
                state.error = "Leader unlock unavailable.";
                render();
            });
            return;
        }
        if (command === "unlock_skin" || command === "equip_skin") {
            var skinId = target.dataset.skinId;
            var skinAccountId = state && state.account_id;
            var skinAuth = storeAuth();
            if (!skinId || !skinAccountId || !skinAuth.available) {
                state.error = "Account setup is required before changing skins.";
                render();
                return;
            }
            target.disabled = true;
            fetch(profileApi(command === "unlock_skin" ? "/store/skins/unlock" : "/store/skins/equip"), {
                method: "POST",
                headers: skinAuth.headers,
                body: JSON.stringify({
                    account_id: skinAccountId,
                    auth_secret: skinAuth.authSecret,
                    skin_id: skinId
                })
            }).then(function (response) {
                if (!response.ok) throw new Error("skin action failed");
                window.location.reload();
            }).catch(function () {
                state.error = command === "unlock_skin" ? "Skin unlock unavailable." : "Skin equip unavailable.";
                render();
            });
            return;
        }
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
            authModalOpen = false;
            render();
            return;
        }
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
                resetAuthFlow();
                authModalOpen = true;
                render();
            }
            return;
        }
        if (command === "sign_out") {
            if (isAndroidTwa()) send(command);
            else if (typeof window.SOW_signOutWou === "function") window.SOW_signOutWou();
            return;
        }
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
                    target.textContent = "COPIED";
                }).catch(function () {
                    target.textContent = "COPY FAILED";
                });
            } else if (lobbyCode) {
                target.textContent = "COPY UNAVAILABLE";
            }
            return;
        }
        if (command === "ban_player" && !window.confirm("Ban this player from the lobby?")) {
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
        var dropdown = event.target.closest ? event.target.closest("[data-control-dropdown]") : null;
        if (dropdownOpenKey && (!dropdown || dropdown.dataset.dropdownKey !== dropdownOpenKey)) {
            setDropdownOpen(dropdownOpenKey, false);
        }
    });

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
        authError = event.detail && event.detail.message ? event.detail.message : "Sign-in is unavailable right now.";
        render();
    });

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
        if (result.status === "success" || result.status === "restored") {
            state.error = null;
            send("refresh_profile");
        } else if (result.status === "cancelled") {
            state.error = "Purchase cancelled.";
        } else {
            state.error = "Purchase failed.";
        }
        render();
    });

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
        if (input.dataset && input.dataset.role === "display-name") {
            displayNameDraft = input.value;
        }
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

    root.addEventListener("focusout", function (event) {
        var input = event.target;
        if (input.dataset.role !== "display-name" || state.name_locked) return;
        var name = input.value.trim();
        displayNameDraft = null;
        if (name && name !== state.player_name) send("save_display_name", { name: name });
    });

    root.addEventListener("change", function (event) {
        var input = event.target;
        var createForm = input.closest("form[data-form='create']");
        if (createForm) {
            syncCreateDraft(createForm);
            render();
            return;
        }
        if (!input.dataset.setting) return;
        if (input.dataset.setting === "mute") send("set_mute", { value: input.value === "off" });
        if (input.dataset.setting === "music_volume") send("set_music_volume", { value: Number(input.value) });
        if (input.dataset.setting === "reduced_motion") send("set_reduced_motion", { value: input.value === "reduced" });
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
        syncProfilePreload(state);
        if (state.phase === "MainMenu" && window.SOW_open_store_after_match) {
            window.SOW_open_store_after_match = false;
            storeOpen = true;
        }
        if (typeof window.SOW_syncWebLoader === "function") {
            window.SOW_syncWebLoader(state);
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
    }

    window.SOW_menu_state_update = handleMenuStateUpdate;
    root.hidden = true;
})();
