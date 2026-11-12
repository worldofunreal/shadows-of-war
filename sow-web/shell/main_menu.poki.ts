// Poki-only menu surface. It overrides the shared shell's account and store
// views so the Poki artifact stays anonymous, non-commercial, and external-link safe.

import {
    MAIN_NAV_ITEMS,
    esc,
    localizedText,
    menuIcon,
    menuUi,
    renderQuickMatchButton,
    renderMainNavMarkup,
} from "./main_menu.core.ts";
import { renderPublicPanel } from "./main_menu.lobbies.ts";
import { renderDirectiveCard } from "./main_menu.missions.ts";
import { renderSettingsPrivacySection } from "./main_menu.shell.ts";
import { renderLocaleSettingRow, renderMusicSpectrum } from "./main_menu.settings.ts";

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
    renderAuthModal,
    renderLeaderPurchase,
    renderPurchaseModal,
    renderSkinPickerModal,
    renderSkinsButton,
    renderStore,
    requestAuthOtp,
    resetAuthFlow,
    resolvePurchaseModal,
    resumeExternalPurchaseDelivery,
    verifyAuthOtp,
} from "./main_menu.portal-stubs.ts";

export function renderFeedback() {
    const error =
        menuUi.state && menuUi.state.error
            ? "<div class='sow-menu__status sow-menu__status--error'>" +
              esc(localizedText(menuUi.state.error)) +
              "</div>"
            : "";
    const noticeKey =
        menuUi.state && menuUi.state.notice
            ? {
                  host_left: "menu.host_left",
                  kicked: "menu.removed_from_lobby",
                  banned: "menu.banned_from_lobby",
              }[menuUi.state.notice] || "menu.join_failed"
            : "";
    const notice = noticeKey
        ? "<div class='sow-menu__status sow-menu__status--notice'>" + esc(SOW_t(noticeKey)) + "</div>"
        : "";
    return error + notice;
}

export function renderCommandPanel() {
    return (
        "" +
        "<section class='sow-menu__command'>" +
        "<div class='sow-menu__home-public'>" +
        renderPublicPanel("home") +
        "</div>" +
        "<div class='sow-menu__home-actions'>" +
        renderDirectiveCard() +
        renderQuickMatchButton() +
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

export function renderMainNav(active: string | null | undefined) {
    return renderMainNavMarkup(
        active,
        MAIN_NAV_ITEMS.filter(function (item) {
            return item[0] !== "store";
        }),
    );
}

export function renderSettings() {
    const settings = menuUi.state!.settings || {};
    const vol = settings.music_volume == null ? 0.8 : settings.music_volume;
    const volPct = Math.round(vol * 100);
    const gearSvg =
        "<svg class='sow-menu__settings-head-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><circle cx='12' cy='12' r='3'/><path d='M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z'/></svg>";
    const slidersSvg =
        "<svg class='sow-menu__settings-sec-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><line x1='4' y1='21' x2='4' y2='14'/><line x1='4' y1='10' x2='4' y2='3'/><line x1='12' y1='21' x2='12' y2='12'/><line x1='12' y1='8' x2='12' y2='3'/><line x1='20' y1='21' x2='20' y2='16'/><line x1='20' y1='12' x2='20' y2='3'/><line x1='2' y1='14' x2='6' y2='14'/><line x1='10' y1='8' x2='14' y2='8'/><line x1='18' y1='16' x2='22' y2='16'/></svg>";
    const userSvg =
        "<svg class='sow-menu__settings-sec-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2'/><circle cx='12' cy='7' r='4'/></svg>";
    const linksSvg =
        "<svg class='sow-menu__settings-sec-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71'/><path d='M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71'/></svg>";
    const musicSvg =
        "<svg class='sow-menu__settings-row-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><polygon points='11 5 6 9 2 9 2 15 6 15 11 19 11 5'/><path d='M15.54 8.46a5 5 0 0 1 0 7.07'/><path d='M19.07 4.93a10 10 0 0 1 0 14.14'/></svg>";
    const accountControl =
        "<div class='sow-menu__settings-account is-guest'><span class='sow-menu__account-dot' aria-hidden='true'></span><strong class='sow-menu__account-value'>" +
        esc(SOW_t("menu.anonymous")) +
        "</strong></div>";
    return (
        "" +
        "<div class='sow-menu__overlay' data-menu-overlay='settings'>" +
        "<section class='sow-menu__modal sow-menu__settings-modal'>" +
        "<div class='sow-menu__modal-head'><div class='sow-menu__settings-title-group'><span class='sow-menu__settings-head-badge' aria-hidden='true'>" +
        gearSvg +
        "</span><h2>" +
        esc(SOW_t("menu.settings")) +
        "</h2></div><button class='sow-menu__icon-button sow-menu__settings-close' type='button' data-command='toggle_settings' aria-label='" +
        esc(SOW_t("menu.close")) +
        "'>×</button></div>" +
        "<div class='sow-menu__settings-body'>" +
        "<div class='sow-menu__settings-col sow-menu__settings-col--primary'>" +
        "<section class='sow-menu__settings-section sow-menu__settings-section--preferences'><h3>" +
        slidersSvg +
        "<span>" +
        esc(SOW_t("menu.settings_preferences")) +
        "</span></h3><div class='sow-menu__settings-controls'>" +
        "<label class='sow-hud__setting-row sow-hud__setting-row--slider'><div class='sow-menu__slider-label'><span class='sow-menu__settings-row-label'>" +
        musicSvg +
        "<span>" +
        esc(SOW_t("menu.music_volume")) +
        "</span></span><b data-val-for='music_vol'>" +
        volPct +
        "%</b></div>" +
        renderMusicSpectrum() +
        "<input type='range' name='music_volume' min='0' max='1' step='0.01' value='" +
        esc(vol) +
        "' style='--sow-music-fill: " +
        volPct +
        "%;' data-setting='music_volume'></label>" +
        renderLocaleSettingRow() +
        "</div></section>" +
        "</div>" +
        "<div class='sow-menu__settings-col sow-menu__settings-col--secondary'>" +
        "<section class='sow-menu__settings-section sow-menu__settings-section--account'><h3>" +
        userSvg +
        "<span>" +
        esc(SOW_t("menu.account")) +
        "</span></h3><div class='sow-menu__settings-account-deck'>" +
        accountControl +
        "</div></section>" +
        "<section class='sow-menu__settings-section sow-menu__settings-section--links'><h3>" +
        linksSvg +
        "<span>" +
        esc(SOW_t("menu.game_links")) +
        "</span></h3><nav class='sow-menu__settings-links' aria-label='" +
        esc(SOW_t("menu.game_links")) +
        "'>" +
        "<button class='sow-menu__settings-link sow-menu__source-link' type='button' data-command='poki_source'>" +
        esc(SOW_t("menu.source_code")) +
        "</button>" +
        "<button class='sow-menu__settings-link sow-menu__license-link' type='button' data-command='poki_license'>" +
        esc(SOW_t("menu.license")) +
        "</button>" +
        renderSettingsPrivacySection(
            "<button class='sow-menu__settings-link' type='button' data-command='poki_privacy'>" +
                esc(SOW_t("menu.privacy")) +
                "</button>",
        ) +
        "</nav></section>" +
        "</div>" +
        "</div>" +
        "</section>" +
        "</div>"
    );
}

export function renderMenuAttribution() {
    return "<span class='sow-menu__attribution'>© OpenFront and Contributors</span>";
}

function pokiLicenseUrl() {
    const sourceUrl = typeof window.SOW_SOURCE_URL === "string" ? window.SOW_SOURCE_URL : "";
    return sourceUrl ? sourceUrl.replace("/tree/", "/blob/") + "/LICENSE" : "";
}

document.addEventListener("click", function (event) {
    const licenseTarget =
        event.target instanceof Element ? event.target.closest("[data-command='poki_license']") : null;
    if (licenseTarget) {
        event.preventDefault();
        const licenseUrl = pokiLicenseUrl();
        if (typeof window.SOW_pokiOpenExternalLink === "function" && licenseUrl) {
            window.SOW_pokiOpenExternalLink(licenseUrl);
        }
        return;
    }
    const sourceTarget =
        event.target instanceof Element ? event.target.closest("[data-command='poki_source']") : null;
    if (sourceTarget) {
        event.preventDefault();
        if (typeof window.SOW_pokiOpenExternalLink === "function" && window.SOW_SOURCE_URL) {
            window.SOW_pokiOpenExternalLink(window.SOW_SOURCE_URL);
        }
        return;
    }
    const target =
        event.target instanceof Element ? event.target.closest("[data-command='poki_privacy']") : null;
    if (target) {
        event.preventDefault();
        if (typeof window.SOW_pokiOpenExternalLink === "function") {
            window.SOW_pokiOpenExternalLink("https://shadowsofwar.io/privacy/");
        }
        return;
    }
});

export function surfaceSettingsRenderKey(_deleteBusy: boolean, _deleteError: string): string {
    const s = menuUi.state?.settings;
    return JSON.stringify([
        s?.music_volume == null ? 0.8 : s?.music_volume,
        Boolean(s?.free_zoom_out),
        Boolean(s?.show_fps_ping),
        Boolean(s?.show_dev_tools),
        Boolean(s?.show_dev_tools_available),
        typeof window.SOW_getLocale === "function" ? window.SOW_getLocale() : "",
    ]);
}
