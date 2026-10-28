// Default render surfaces. Portal builds (Poki, Jest) replace this module with
// main_menu.poki.ts or main_menu.jest.ts, which export the same names.

import {
    MAIN_NAV_ITEMS,
    esc,
    localizedText,
    menuIcon,
    menuUi,
    renderMainNavMarkup,
} from "./main_menu.core.ts";
import { renderPublicPanel } from "./main_menu.lobbies.ts";
import { localeOptions, render, renderSettingsPrivacySection } from "./main_menu.shell.ts";
import { isAndroidTwa } from "./main_menu.store.ts";

export function renderCommandPanel() {
    const ranked = menuUi.rankedRating;
    const rankedLabel = ranked
        ? ranked.placements_complete
            ? ranked.tier + (ranked.division ? " " + ranked.division : "")
            : SOW_t("profile.unranked")
        : "—";
    return (
        "" +
        "<section class='sow-menu__command'>" +
        "<div class='sow-menu__home-public'>" +
        renderPublicPanel("home") +
        "</div>" +
        "<div class='sow-menu__home-actions'>" +
        "<button class='sow-menu__primary sow-menu__quick-match' type='button' data-command='quick_match'><div><strong>" +
        esc(SOW_t("menu.quick_match")) +
        "</strong><small>" +
        esc(SOW_t("lobbies.ranked_1v1")) +
        " · <b data-ranked-current>" +
        esc(rankedLabel) +
        "</b></small></div><span aria-hidden='true'>↗</span></button>" +
        "<button class='sow-menu__secondary' type='button' data-command='open_campaign'>" +
        esc(SOW_t("menu.campaign")) +
        " " +
        menuIcon("attack") +
        "</button>" +
        "<button class='sow-menu__secondary' type='button' data-command='open_browser'>" +
        esc(SOW_t("menu.lobby_browser")) +
        " <span>→</span></button>" +
        "<form class='sow-menu__join' data-form='join'>" +
        "<input name='code' inputmode='numeric' autocomplete='off' placeholder='" +
        esc(SOW_t("menu.lobby_code")) +
        "' aria-label='" +
        esc(SOW_t("menu.lobby_code")) +
        "'>" +
        "<button type='submit'>" +
        esc(SOW_t("menu.join")) +
        "</button>" +
        "</form>" +
        "<button class='sow-menu__secondary' type='button' data-command='open_create'>" +
        esc(SOW_t("menu.create_custom_game")) +
        " <span>+</span></button>" +
        renderFeedback() +
        "</div>" +
        "</section>"
    );
}

export function renderFeedback() {
    const error = menuUi.state!.error
        ? "<div class='sow-menu__status sow-menu__status--error'>" +
          esc(localizedText(menuUi.state!.error)) +
          "</div>"
        : "";
    const notice = menuUi.state!.notice
        ? "<div class='sow-menu__status sow-menu__status--notice'>" +
          esc(
              SOW_t(
                  {
                      host_left: "menu.host_left",
                      kicked: "menu.removed_from_lobby",
                      banned: "menu.banned_from_lobby",
                  }[menuUi.state!.notice] || "menu.join_failed",
              ),
          ) +
          "</div>"
        : "";
    return error + notice;
}

function renderSiteLink(path: string, key: string, className?: string) {
    const crazyGames = window.SOW_PORTAL === "crazygames";
    const href = crazyGames ? "https://shadowsofwar.io" + path : path;
    const classAttr = className ? " class='" + esc(className) + "'" : "";
    const targetAttr = crazyGames ? " target='_blank' rel='noopener noreferrer'" : "";
    return "<a" + classAttr + " href='" + esc(href) + "'" + targetAttr + ">" + esc(SOW_t(key)) + "</a>";
}

export function renderMenuAttribution() {
    return "<span class='sow-menu__attribution'>© OpenFront and Contributors</span>";
}

function renderSettingsLink(url: string | null | undefined, key: string, className?: string) {
    if (!url) return "";
    if (isAndroidTwa()) {
        return (
            "<button class='sow-menu__settings-link" +
            (className ? " " + esc(className) : "") +
            "' type='button' data-command='open_android_external_link' data-url='" +
            esc(url) +
            "'>" +
            esc(SOW_t(key)) +
            "</button>"
        );
    }
    return (
        "<a class='sow-menu__settings-link" +
        (className ? " " + esc(className) : "") +
        "' href='" +
        esc(url) +
        "' target='_blank' rel='noopener noreferrer'>" +
        esc(SOW_t(key)) +
        "</a>"
    );
}

function renderSettingsLinks() {
    const sourceUrl = typeof window.SOW_SOURCE_URL === "string" ? window.SOW_SOURCE_URL : "";
    const licenseUrl = sourceUrl ? sourceUrl.replace("/tree/", "/blob/") + "/LICENSE" : "";
    const androidOssLicenses =
        isAndroidTwa() &&
        typeof window.SOW_androidOssLicensesAvailable === "function" &&
        window.SOW_androidOssLicensesAvailable()
            ? "<button class='sow-menu__settings-link' type='button' data-command='open_android_oss_licenses'>" +
              esc(SOW_t("menu.third_party_licenses")) +
              "</button>"
            : "";
    const cookieSettings =
        typeof window.SOW_analyticsManage === "function"
            ? "<button class='sow-menu__settings-link' type='button' data-sow-cookie-settings>" +
              esc(SOW_t("menu.cookie_settings")) +
              "</button>"
            : "";
    return (
        renderSettingsLink("https://shadowsofwar.io/#faq", "site.faq") +
        renderSettingsLink("https://shadowsofwar.io/support/", "menu.support") +
        renderSettingsLink("https://shadowsofwar.io/terms/", "menu.terms") +
        renderSettingsLink("https://shadowsofwar.io/cookies/", "menu.cookies") +
        cookieSettings +
        renderSettingsLink("https://discord.gg/d6ZDeChSE", "menu.discord") +
        renderSettingsLink("https://t.me/shadowsofwario", "menu.telegram") +
        renderSettingsLink(sourceUrl, "menu.source_code", "sow-menu__source-link") +
        renderSettingsLink(licenseUrl, "menu.license", "sow-menu__license-link") +
        androidOssLicenses
    );
}

export function renderMainNav(active: string | null | undefined) {
    return renderMainNavMarkup(active, MAIN_NAV_ITEMS);
}

export function renderSettings() {
    const settings = menuUi.state!.settings || {};
    const vol = settings.music_volume == null ? 0.8 : settings.music_volume;
    const volPct = Math.round(vol * 100);
    const auth =
        typeof window.SOW_getAuthState === "function"
            ? window.SOW_getAuthState()
            : { platform: isAndroidTwa() ? "twa" : "web", linked: false, pending: false };
    const session =
        typeof window.SOW_getWouSession === "function" ? window.SOW_getWouSession() : authSession();
    const accountEmail =
        session && session.user && session.user.email ? String(session.user.email).trim() : "";
    const providers = (auth.linkedProviders || []).map(function (provider) {
        return String(provider).replace(/_/g, " ").toUpperCase();
    });
    const providerLabel =
        auth.platform === "twa"
            ? SOW_t("menu.google_play_games")
            : auth.provider === "crazygames"
              ? "CRAZYGAMES"
              : providers.join(" · ") ||
                (auth.provider ? String(auth.provider).replace(/_/g, " ").toUpperCase() : "");
    const accountDetail = auth.linked
        ? accountEmail || providerLabel || SOW_t("menu.anonymous")
        : SOW_t("menu.anonymous");
    const accountControl = auth.pending
        ? "<div class='sow-menu__settings-account sow-menu__settings-account--pending'><strong class='sow-menu__account-pending'>" +
          esc(SOW_t("menu.connecting")) +
          "</strong></div>"
        : "<div class='sow-menu__settings-account'><strong class='sow-menu__account-value'>" +
          esc(accountDetail) +
          "</strong></div>";
    let signOutControl = "";
    if (auth.canSignOut) {
        signOutControl = menuUi.signOutConfirmOpen
            ? "<section class='sow-menu__settings-confirm' role='alertdialog' aria-label='" +
              esc(SOW_t("menu.sign_out")) +
              "'>" +
              "<strong>" +
              esc(SOW_t("menu.sign_out")) +
              "?</strong>" +
              "<div class='sow-menu__settings-confirm-actions'>" +
              "<button class='sow-menu__secondary' type='button' data-command='cancel_sign_out'>" +
              esc(SOW_t("lobbies.cancel")) +
              "</button>" +
              "<button class='sow-menu__danger' type='button' data-command='confirm_sign_out'>" +
              esc(SOW_t("menu.sign_out")) +
              "</button>" +
              "</div>" +
              "</section>"
            : "<button class='sow-menu__danger sow-menu__settings-signout' type='button' data-command='sign_out'>" +
              esc(SOW_t("menu.sign_out")) +
              "</button>";
    }
    return (
        "" +
        "<div class='sow-menu__overlay' data-menu-overlay='settings'>" +
        "<section class='sow-menu__modal sow-menu__settings-modal'>" +
        "<div class='sow-menu__modal-head'>" +
        "<div>" +
        "<h2>" +
        esc(SOW_t("menu.settings")) +
        "</h2>" +
        "</div>" +
        "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='" +
        esc(SOW_t("menu.close")) +
        "'>×</button>" +
        "</div>" +
        "<div class='sow-menu__settings-body'>" +
        "<section class='sow-menu__settings-section'><h3>" +
        esc(SOW_t("menu.account")) +
        "</h3>" +
        accountControl +
        (signOutControl ? "<div class='sow-menu__settings-actions'>" + signOutControl + "</div>" : "") +
        "</section>" +
        "<section class='sow-menu__settings-section'><h3>" +
        esc(SOW_t("menu.settings_preferences")) +
        "</h3><div class='sow-menu__settings-controls'>" +
        "<label class='sow-hud__setting-row sow-hud__setting-row--slider'>" +
        "<div class='sow-menu__slider-label'><span>" +
        esc(SOW_t("menu.music_volume")) +
        "</span><b data-val-for='music_vol'>" +
        volPct +
        "%</b></div>" +
        "<input type='range' name='music_volume' min='0' max='1' step='0.05' value='" +
        esc(vol) +
        "' data-setting='music_volume'>" +
        "</label>" +
        "<div class='sow-hud__setting-row sow-menu__settings-dropdown-row'>" +
        "<span>" +
        esc(SOW_t("menu.motion_animation")) +
        "</span>" +
        SOW_renderDropdown({
            key: "settings-motion",
            className: "sow-menu__settings-dropdown",
            label: SOW_t("menu.motion_animation"),
            name: "reduced_motion",
            setting: "reduced_motion",
            value: settings.reduced_motion ? "reduced" : "full",
            options: [
                { value: "full", label: SOW_t("menu.full") },
                { value: "reduced", label: SOW_t("menu.reduced_motion") },
            ],
        }) +
        "</div>" +
        "<label class='sow-hud__setting-row'>" +
        "<span>" +
        esc(SOW_t("menu.free_zoom_out")) +
        "</span>" +
        "<input class='sow-hud__switch' type='checkbox' role='switch' name='free_zoom_out' data-setting='free_zoom_out'" +
        (settings.free_zoom_out ? " checked" : "") +
        ">" +
        "</label>" +
        "<label class='sow-hud__setting-row'>" +
        "<span>" +
        esc(SOW_t("menu.show_fps_ping")) +
        "</span>" +
        "<input class='sow-hud__switch' type='checkbox' role='switch' name='show_fps_ping' data-setting='show_fps_ping'" +
        (settings.show_fps_ping === true ? " checked" : "") +
        ">" +
        "</label>" +
        (settings.show_dev_tools_available
            ? "<label class='sow-hud__setting-row'>" +
              "<span>" +
              esc(SOW_t("menu.show_dev_tools")) +
              "</span>" +
              "<input class='sow-hud__switch' type='checkbox' role='switch' name='show_dev_tools' data-setting='show_dev_tools'" +
              (settings.show_dev_tools ? " checked" : "") +
              ">" +
              "</label>"
            : "") +
        "<div class='sow-hud__setting-row sow-menu__settings-dropdown-row'><span>" +
        esc(SOW_t("menu.language")) +
        "</span>" +
        SOW_renderDropdown({
            key: "settings-language",
            className: "sow-menu__settings-dropdown",
            label: SOW_t("menu.language"),
            name: "locale",
            setting: "locale",
            value: typeof window.SOW_getLocale === "function" ? window.SOW_getLocale() : "en",
            options: localeOptions(),
        }) +
        "</div>" +
        "</div></section>" +
        renderSettingsPrivacySection(renderSettingsLink("https://shadowsofwar.io/privacy/", "menu.privacy")) +
        "<section class='sow-menu__settings-section'><h3>" +
        esc(SOW_t("menu.game_links")) +
        "</h3><nav class='sow-menu__settings-links' aria-label='" +
        esc(SOW_t("menu.game_links")) +
        "'>" +
        renderSettingsLinks() +
        "</nav></section>" +
        "</div>" +
        "</section>" +
        "</div>"
    );
}

function authApi(path: string) {
    return "https://id.worldofunreal.com" + path;
}

interface WouSession {
    token: string;
    accountId: string;
    user: unknown;
}

function authSession(): WouSession {
    if (typeof window.SOW_getWouSession === "function") return window.SOW_getWouSession();
    try {
        const token = window.localStorage.getItem("wou_session_token") || "";
        const user = JSON.parse(window.localStorage.getItem("wou_user_data") || "null");
        return { token: token, accountId: (user && user.account_id) || "", user: user };
    } catch {
        return { token: "", accountId: "", user: null };
    }
}

function authHeaders() {
    const headers: Record<string, string> = {
        "Content-Type": "application/json",
        Accept: "application/json",
    };
    const session = authSession();
    if (session.token) headers.Authorization = "Bearer " + session.token;
    return headers;
}

function ensureAuthSession(): Promise<WouSession> {
    if (typeof window.SOW_ensureWouAnonymousSession === "function") {
        return window.SOW_ensureWouAnonymousSession();
    }
    return Promise.resolve(authSession());
}

function authIcon(provider: string) {
    if (provider === "google")
        return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='#4285F4' d='M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z'/><path fill='#34A853' d='M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z'/><path fill='#FBBC05' d='M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.06H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.94l2.85-2.22z'/><path fill='#EA4335' d='M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.06l3.66 2.84C6.71 7.3 9.14 5.38 12 5.38z'/></svg>";
    if (provider === "discord")
        return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='currentColor' d='M20.3 4.37a19.8 19.8 0 0 0-4.88-1.52.08.08 0 0 0-.08.04c-.21.38-.45.87-.61 1.25a18.27 18.27 0 0 0-5.49 0c-.17-.38-.41-.87-.62-1.25a.08.08 0 0 0-.08-.04A19.74 19.74 0 0 0 3.68 4.37a.07.07 0 0 0-.03.03C.53 9.05-.32 13.58.1 18.06a.08.08 0 0 0 .03.06 19.9 19.9 0 0 0 5.99 3.03.08.08 0 0 0 .08-.03c.46-.63.87-1.3 1.23-1.99a.08.08 0 0 0-.04-.11 12.3 12.3 0 0 1-1.87-.89.08.08 0 0 1-.01-.13 10.2 10.2 0 0 0 .37-.29.07.07 0 0 1 .08-.01c3.93 1.79 8.18 1.79 12.06 0a.07.07 0 0 1 .08.01c.12.1.25.2.37.29a.08.08 0 0 1-.01.13c-.59.35-1.21.65-1.87.89a.08.08 0 0 0-.04.11c.36.7.77 1.36 1.23 1.99a.08.08 0 0 0 .08.03 19.84 19.84 0 0 0 6-3.03.08.08 0 0 0 .03-.05c.5-5.18-.84-9.67-3.55-13.66a.06.06 0 0 0-.03-.03zM8.02 15.33c-1.18 0-2.16-1.09-2.16-2.42s.96-2.42 2.16-2.42 2.18 1.1 2.16 2.42-.96 2.42-2.16 2.42zm7.98 0c-1.18 0-2.16-1.09-2.16-2.42s.96-2.42 2.16-2.42 2.18 1.1 2.16 2.42-.95 2.42-2.16 2.42z'/></svg>";
    if (provider === "twitter")
        return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='currentColor' d='M18.24 2.25h3.31l-7.23 8.26 8.5 11.24h-6.66l-5.21-6.82-5.97 6.82H1.68l7.73-8.84L1.25 2.25h6.82l4.71 6.23 5.46-6.23zm-1.16 17.52h1.83L7.08 4.13H5.12l11.96 15.64z'/></svg>";
    return "<svg viewBox='0 0 24 24' aria-hidden='true'><path fill='currentColor' d='M24 12.07a12 12 0 1 0-13.88 11.86v-8.38H7.08v-3.48h3.04V9.43c0-3.01 1.79-4.67 4.53-4.67 1.31 0 2.69.24 2.69.24v2.95h-1.51c-1.49 0-1.96.93-1.96 1.88v2.25h3.33l-.53 3.47h-2.8v8.38A12 12 0 0 0 24 12.07z'/></svg>";
}

function renderAuthSocial() {
    return [
        ["google", "auth.google"],
        ["discord", "auth.discord_provider"],
        ["twitter", "auth.x_provider"],
        ["meta", "auth.meta"],
    ]
        .map(function (provider) {
            return (
                "<button class='sow-auth__provider' type='button' data-command='wou_provider' data-provider='" +
                provider[0] +
                "'><span class='sow-auth__provider-icon sow-auth__provider-icon--" +
                provider[0] +
                "'>" +
                authIcon(provider[0]) +
                "</span><span>" +
                esc(SOW_t("auth.continue_with", { provider: SOW_t(provider[1]) })) +
                "</span><i>↗</i></button>"
            );
        })
        .join("");
}

interface WouLoginData {
    session_token?: string;
    account?: unknown;
}

function finishWouLogin(data: WouLoginData) {
    if (!data || !data.session_token || !data.account) throw new Error(SOW_t("auth.identity_incomplete"));
    try {
        window.localStorage.setItem("wou_session_token", data.session_token);
        window.localStorage.setItem("wou_user_data", JSON.stringify(data.account));
    } catch (error) {
        throw new Error(SOW_t("auth.account_save_failed"), { cause: error });
    }
    menuUi.authBusy = false;
    menuUi.authModalOpen = false;
    menuUi.authOtpSent = false;
    menuUi.authError = "";
    menuUi.authNotice = "";
    window.dispatchEvent(
        new CustomEvent("wou:auth-state-change", {
            detail: {
                authenticated: true,
                isAuthenticated: true,
                user: data.account,
                token: data.session_token,
            },
        }),
    );
    window.location.reload();
}

function authJson(response: Response) {
    return response
        .json()
        .catch(function () {
            return {};
        })
        .then(function (data) {
            if (!response.ok) throw new Error(data.error || SOW_t("auth.sign_in_unavailable"));
            return data;
        });
}

export function requestAuthOtp() {
    if (menuUi.authBusy) return;
    const email = String(menuUi.authEmail || "").trim();
    if (!/^\S+@\S+\.\S+$/.test(email)) {
        menuUi.authError = SOW_t("auth.valid_email");
        menuUi.authNotice = "";
        render();
        return;
    }
    menuUi.authEmail = email;
    menuUi.authBusy = true;
    menuUi.authError = "";
    menuUi.authNotice = "";
    render();
    ensureAuthSession()
        .then(function (session) {
            return fetch(authApi("/api/v1/auth/otp/request"), {
                method: "POST",
                headers: authHeaders(),
                body: JSON.stringify({
                    email: menuUi.authEmail,
                    account_id: session.accountId || null,
                    context: "shadows_of_war",
                    newsletter_opt_in: false,
                }),
            }).then(authJson);
        })
        .then(function (data) {
            menuUi.authOtpSent = true;
            menuUi.authBusy = false;
            menuUi.authNotice = data.message || SOW_t("auth.verification_sent");
            render();
        })
        .catch(function (error: unknown) {
            menuUi.authBusy = false;
            menuUi.authError =
                error instanceof Error && error.message ? error.message : SOW_t("auth.send_code_failed");
            render();
        });
}

export function verifyAuthOtp() {
    if (menuUi.authBusy) return;
    const code = String(menuUi.authCode || "").trim();
    if (!/^\d{6}$/.test(code)) {
        menuUi.authError = SOW_t("auth.six_digit_code");
        menuUi.authNotice = "";
        render();
        return;
    }
    menuUi.authBusy = true;
    menuUi.authError = "";
    menuUi.authNotice = "";
    render();
    ensureAuthSession()
        .then(function (session) {
            return fetch(authApi("/api/v1/auth/otp/verify"), {
                method: "POST",
                headers: authHeaders(),
                body: JSON.stringify({
                    email: menuUi.authEmail,
                    code: code,
                    account_id: session.accountId || null,
                    context: "shadows_of_war",
                }),
            }).then(authJson);
        })
        .then(finishWouLogin)
        .catch(function (error: unknown) {
            menuUi.authBusy = false;
            menuUi.authError =
                error instanceof Error && error.message ? error.message : SOW_t("auth.invalid_code");
            render();
        });
}

export function resetAuthFlow() {
    menuUi.authEmail = "";
    menuUi.authCode = "";
    menuUi.authOtpSent = false;
    menuUi.authBusy = false;
    menuUi.authError = "";
    menuUi.authNotice = "";
}

export function renderAuthModal() {
    const emailPanel = menuUi.authOtpSent
        ? "<form class='sow-auth__form' data-auth-form='verify'><label>" +
          esc(SOW_t("auth.code_sent_to")) +
          " <strong>" +
          esc(menuUi.authEmail) +
          "</strong></label><input class='sow-auth__code' data-auth-field='code' inputmode='numeric' autocomplete='one-time-code' maxlength='6' value='" +
          esc(menuUi.authCode) +
          "' placeholder='000000' aria-label='" +
          esc(SOW_t("auth.verification_code")) +
          "' required><button class='sow-auth__submit sow-auth__submit--cyan' type='submit'" +
          (menuUi.authBusy ? " disabled" : "") +
          ">" +
          esc(menuUi.authBusy ? SOW_t("auth.checking") : SOW_t("auth.verify_code")) +
          "</button><div class='sow-auth__form-links'><button type='button' data-command='auth_change_email'>" +
          esc(SOW_t("auth.change_email")) +
          "</button><button type='button' data-command='auth_resend'" +
          (menuUi.authBusy ? " disabled" : "") +
          ">" +
          esc(SOW_t("auth.resend")) +
          "</button></div></form>"
        : "<form class='sow-auth__form' data-auth-form='request'><label for='sow-auth-email'>" +
          esc(SOW_t("auth.email_address")) +
          "</label><input id='sow-auth-email' class='sow-auth__input' data-auth-field='email' type='email' autocomplete='email' value='" +
          esc(menuUi.authEmail) +
          "' placeholder='" +
          esc(SOW_t("auth.email_placeholder")) +
          "' required><button class='sow-auth__submit' type='submit'" +
          (menuUi.authBusy ? " disabled" : "") +
          ">" +
          esc(menuUi.authBusy ? SOW_t("auth.sending") : SOW_t("auth.send_code")) +
          "</button></form>";
    const error = menuUi.authError
        ? "<div class='sow-auth__message sow-auth__message--error' role='alert'>" +
          esc(menuUi.authError) +
          "</div>"
        : "";
    const notice = menuUi.authNotice
        ? "<div class='sow-auth__message sow-auth__message--notice' role='status'>" +
          esc(menuUi.authNotice) +
          "</div>"
        : "";
    return (
        "<div class='sow-menu__overlay' data-menu-overlay='auth' data-auth-overlay><section class='sow-menu__modal sow-auth' role='dialog' aria-modal='true' aria-label='" +
        esc(SOW_t("auth.account")) +
        "'>" +
        "<div class='sow-auth__glow sow-auth__glow--cyan'></div><div class='sow-auth__head'><div class='sow-auth__logos'><img class='sow-auth__game-logo' src='/sow-long.svg' alt='" +
        esc(SOW_t("menu.brand")) +
        "'><span class='sow-auth__logo-divider' aria-hidden='true'></span><img class='sow-auth__wou-logo' src='https://worldofunreal.com/wouid.svg' alt='WouID'></div><button class='sow-menu__icon-button' type='button' data-command='close_auth' aria-label='" +
        esc(SOW_t("auth.close")) +
        "'>×</button></div>" +
        error +
        notice +
        "<div class='sow-auth__body'>" +
        emailPanel +
        "<div class='sow-auth__social-list'>" +
        renderAuthSocial() +
        "</div></div>" +
        renderSiteLink("/terms/", "auth.terms", "sow-auth__terms") +
        "</section></div>"
    );
}

export {
    abandonExternalPurchaseAttempt,
    beginStorePurchase,
    beginStoreRestore,
    canDismissPurchaseModal,
    completeExternalPurchase,
    currentExternalPurchaseAttempt,
    dismissPurchaseModal,
    isAndroidTwa,
    loadStoreCatalog,
    openLeaderPurchase,
    openSkinPurchase,
    renderLeaderPurchase,
    renderPurchaseModal,
    renderSkinPickerModal,
    renderStore,
    resolvePurchaseModal,
    resumeExternalPurchaseDelivery,
} from "./main_menu.store.ts";
