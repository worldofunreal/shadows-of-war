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
    renderQuickMatchButton,
    renderMainNavMarkup,
} from "./main_menu.core.ts";
import { renderPublicPanel } from "./main_menu.lobbies.ts";
import { localeOptions, renderSettingsPrivacySection } from "./main_menu.shell.ts";

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
    const accountControl =
        "<div class='sow-menu__settings-account'><strong class='sow-menu__account-value'>" +
        esc(SOW_t("menu.anonymous")) +
        "</strong>" +
        "<button class='sow-menu__secondary' type='button' data-command='jest_login'>" +
        esc(SOW_t("menu.sign_in")) +
        "</button></div>";
    return (
        "" +
        "<div class='sow-menu__overlay' data-menu-overlay='settings'>" +
        "<section class='sow-menu__modal sow-menu__settings-modal'>" +
        "<div class='sow-menu__modal-head'><div><h2>" +
        esc(SOW_t("menu.settings")) +
        "</h2></div><button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='" +
        esc(SOW_t("menu.close")) +
        "'>×</button></div>" +
        "<div class='sow-menu__settings-body'>" +
        accountControl +
        "<div class='sow-menu__settings-controls'>" +
        "<label class='sow-hud__setting-row sow-hud__setting-row--slider'><div class='sow-menu__slider-label'><span>" +
        esc(SOW_t("menu.music_volume")) +
        "</span><b data-val-for='music_vol'>" +
        volPct +
        "%</b></div><input type='range' name='music_volume' min='0' max='1' step='0.05' value='" +
        esc(vol) +
        "' data-setting='music_volume'></label>" +
        "<div class='sow-hud__setting-row sow-menu__settings-dropdown-row'><span>" +
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
        "</div>" +
        renderSettingsPrivacySection("") +
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
