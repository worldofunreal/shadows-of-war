// Jest-only menu surface. It overrides the shared shell's account and store
// views so the Jest artifact stays anonymous, non-commercial, and
// external-link safe. Registration goes through the Jest platform popup
// (window.SOW_jestLogin); there is no first-party auth UI here.

import {
    MAIN_NAV_ITEMS,
    esc,
    localizedText,
    menuIcon,
    menuUi,
    renderLobbyBrowserButton,
    renderQuickMatchButton,
    renderMainNavMarkup,
} from "./main_menu.core.ts";
import { renderPublicPanel } from "./main_menu.lobbies.ts";
import { renderDirectiveCard } from "./main_menu.missions.ts";
import { renderSettingsPrivacySection } from "./main_menu.shell.ts";
import {
    renderAdvisorTipsSettingRow,
    renderMatchClockSettingRow,
    renderLocaleSettingRow,
    renderMusicSpectrum,
    renderSfxVolumeSettingRow,
} from "./main_menu.settings.ts";

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
    openOfferPurchase,
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
        "<section class='sow-menu__command sow-home__stage'>" +
        "<div class='sow-menu__home-public'>" +
        renderPublicPanel("home") +
        "</div>" +
        "<div class='sow-home__directive'>" +
        renderDirectiveCard() +
        "</div>" +
        "<div class='sow-menu__home-actions sow-home__dock'>" +
        renderFeedback() +
        "<div class='sow-home__secondary-modes'>" +
        "<button class='sow-menu__secondary sow-home__mode-btn sow-home__campaign-btn' type='button' data-command='open_campaign'>" +
        "<span class='sow-home__mode-icon' aria-hidden='true'>" +
        menuIcon("attack") +
        "</span>" +
        "<span class='sow-home__mode-label'>" +
        esc(SOW_t("menu.campaign")) +
        "</span>" +
        "</button>" +
        renderLobbyBrowserButton() +
        "</div>" +
        renderQuickMatchButton() +
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

function renderAccountControl() {
    const identity = window.SOW_PLATFORM_IDENTITY;
    const registered = identity?.provider === "jest" && identity.registered === true;
    const accountName = registered ? identity?.displayName || "Player" : SOW_t("menu.anonymous");
    return (
        "<div class='sow-menu__settings-account " +
        (registered ? "is-registered" : "is-guest") +
        "'><span class='sow-menu__account-dot' aria-hidden='true'></span><strong class='sow-menu__account-value'>" +
        esc(accountName) +
        "</strong></div>" +
        (registered
            ? ""
            : "<button class='sow-menu__secondary sow-menu__settings-signin' type='button' data-command='jest_login'>" +
              esc(SOW_t("menu.sign_in")) +
              "</button>")
    );
}

export function renderSettings() {
    const settings = menuUi.state!.settings || {};
    const vol = settings.music_volume == null ? 0.8 : settings.music_volume;
    const volPct = Math.round(vol * 100);
    const gearSvg =
        menuIcon("options", "sow-menu__settings-head-icon");
    const slidersSvg =
        "<svg class='sow-menu__settings-sec-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><line x1='4' y1='21' x2='4' y2='14'/><line x1='4' y1='10' x2='4' y2='3'/><line x1='12' y1='21' x2='12' y2='12'/><line x1='12' y1='8' x2='12' y2='3'/><line x1='20' y1='21' x2='20' y2='16'/><line x1='20' y1='12' x2='20' y2='3'/><line x1='2' y1='14' x2='6' y2='14'/><line x1='10' y1='8' x2='14' y2='8'/><line x1='18' y1='16' x2='22' y2='16'/></svg>";
    const userSvg =
        "<svg class='sow-menu__settings-sec-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2'/><circle cx='12' cy='7' r='4'/></svg>";
    const musicSvg =
        "<svg class='sow-menu__settings-row-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><polygon points='11 5 6 9 2 9 2 15 6 15 11 19 11 5'/><path d='M15.54 8.46a5 5 0 0 1 0 7.07'/><path d='M19.07 4.93a10 10 0 0 1 0 14.14'/></svg>";
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
        "' style='--sow-slider-fill: " +
        volPct +
        "%; --sow-music-fill: " +
        volPct +
        "%;' data-setting='music_volume'></label>" +
        renderSfxVolumeSettingRow() +
        renderLocaleSettingRow() +
        renderAdvisorTipsSettingRow() +
        renderMatchClockSettingRow() +
        "</div></section>" +
        "</div>" +
        "<div class='sow-menu__settings-col sow-menu__settings-col--secondary'>" +
        "<section class='sow-menu__settings-section sow-menu__settings-section--account'><h3>" +
        userSvg +
        "<span>" +
        esc(SOW_t("menu.account")) +
        "</span></h3><div class='sow-menu__settings-account-deck'>" +
        renderAccountControl() +
        "</div></section>" +
        renderSettingsPrivacySection("") +
        "</div>" +
        "</div>" +
        "</section>" +
        "</div>"
    );
}

export function renderMenuAttribution() {
    return "<span class='sow-menu__attribution'>© OpenFront and Contributors</span>";
}

document.addEventListener("click", function (event) {
    const target =
        event.target instanceof Element ? event.target.closest("[data-command='jest_login']") : null;
    if (!target) return;
    event.preventDefault();
    if (typeof window.SOW_jestLogin === "function") {
        window.SOW_jestLogin();
    }
});

window.addEventListener("sow:platform-identity-changed", function () {
    const accountDeck = document.querySelector(".sow-menu__settings-account-deck");
    if (accountDeck) accountDeck.innerHTML = renderAccountControl();
});

export function surfaceSettingsRenderKey(_deleteBusy: boolean, _deleteError: string): string {
    const s = menuUi.state?.settings;
    return JSON.stringify([
        s?.music_volume == null ? 0.8 : s?.music_volume,
        s?.sfx_volume == null ? 0.8 : s?.sfx_volume,
        Boolean(s?.free_zoom_out),
        Boolean(s?.show_fps_ping),
        Boolean(s?.show_match_clock),
        Boolean(s?.show_dev_tools),
        Boolean(s?.show_dev_tools_available),
        Boolean(menuUi.state?.menu_tips_disabled),
        typeof window.SOW_getLocale === "function" ? window.SOW_getLocale() : "",
    ]);
}
