// Poki-only menu surface. It overrides the shared shell's account and store
// views so the Poki artifact stays anonymous, non-commercial, and external-link safe.

function renderFeedback() {
    var error = state && state.error ? "<div class='sow-menu__status sow-menu__status--error'>" + esc(localizedText(state.error)) + "</div>" : "";
    var noticeKey = state && state.notice ? ({
        host_left: "menu.host_left",
        kicked: "menu.removed_from_lobby",
        banned: "menu.banned_from_lobby",
        connection_lost: "menu.connection_lost"
    }[state.notice] || "menu.connection_lost") : "";
    var notice = noticeKey ? "<div class='sow-menu__status sow-menu__status--notice'>" + esc(SOW_t(noticeKey)) + "</div>" : "";
    return error + notice;
}

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

function renderMainNav(active) {
    return renderMainNavMarkup(active, MAIN_NAV_ITEMS.filter(function (item) { return item[0] !== "store"; }));
}

function renderSettings() {
    var settings = state.settings || {};
    var vol = settings.music_volume == null ? 0.8 : settings.music_volume;
    var volPct = Math.round(vol * 100);
    var accountControl = "<div class='sow-menu__settings-account'><strong class='sow-menu__account-value'>" + esc(SOW_t("menu.anonymous")) + "</strong></div>";
    return "" +
        "<div class='sow-menu__overlay' data-menu-overlay='settings'>" +
            "<section class='sow-menu__modal sow-menu__settings-modal'>" +
                "<div class='sow-menu__modal-head'><div><h2>" + esc(SOW_t("menu.settings")) + "</h2></div><button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='" + esc(SOW_t("menu.close")) + "'>×</button></div>" +
                "<div class='sow-menu__settings-body'>" +
                    accountControl +
                    "<div class='sow-menu__settings-controls'>" +
                        "<label class='sow-menu__form-field'><div class='sow-menu__slider-label'><span>" + esc(SOW_t("menu.music_volume")) + "</span><b data-val-for='music_vol'>" + volPct + "%</b></div><input class='sow-menu__field' type='range' name='music_volume' min='0' max='1' step='0.05' value='" + esc(vol) + "' data-setting='music_volume'></label>" +
                    "<label class='sow-menu__form-field'><span>" + esc(SOW_t("menu.motion_animation")) + "</span>" +
                        SOW_renderDropdown({ key: "settings-motion", name: "reduced_motion", setting: "reduced_motion", value: settings.reduced_motion ? "reduced" : "full", options: [{ value: "full", label: SOW_t("menu.full") }, { value: "reduced", label: SOW_t("menu.reduced_motion") }] }) +
                    "</label>" +
                    "<label class='sow-menu__form-field'><span>" + esc(SOW_t("menu.language")) + "</span>" +
                        SOW_renderDropdown({ key: "settings-language", name: "locale", setting: "locale", value: typeof window.SOW_getLocale === "function" ? window.SOW_getLocale() : "en", options: localeOptions() }) +
                    "</label>" +
                    "</div>" +
                "</div>" +
            "</section>" +
        "</div>";
}

function renderFooter(label) {
    return "<footer class='sow-menu__footer'>" +
        (label ? "<span data-menu-footer-label>" + esc(label) + "</span>" : "") +
        "<nav class='sow-menu__footer-links' aria-label='" + esc(SOW_t("menu.game_links")) + "'><button type='button' data-command='poki_privacy'>" + esc(SOW_t("menu.privacy")) + "</button></nav>" +
        "<span>" + esc(SOW_t("menu.brand")) + "</span></footer>";
}

function renderAuthModal() { return ""; }
function renderStore() { return ""; }
function renderLeaderPurchase() { return ""; }
function loadStoreCatalog() {}
function beginStorePurchase() {}
function beginStoreRestore() {}
function closeStoreCheckout() {}

document.addEventListener("click", function (event) {
    var target = event.target && event.target.closest ? event.target.closest("[data-command='poki_privacy']") : null;
    if (target) {
        event.preventDefault();
        if (typeof window.SOW_pokiOpenExternalLink === "function") {
            window.SOW_pokiOpenExternalLink("https://shadowsofwar.io/privacy/");
        }
        return;
    }
    var commandTarget = event.target && event.target.closest ? event.target.closest("[data-command]") : null;
    if (commandTarget && typeof window.SOW_pokiMeasure === "function") {
        var command = String(commandTarget.dataset.command || "");
        if (command === "quick_match" || command === "open_campaign" || command === "open_browser" || command === "open_create" || command === "join_lobby" || command === "confirm_leader") {
            window.SOW_pokiMeasure("menu", command, "interact");
        }
    }
});
