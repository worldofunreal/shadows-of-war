// Poki-only menu surface. It overrides the shared shell's account and store
// views so the Poki artifact stays anonymous, non-commercial, and external-link safe.

function renderFeedback() {
    var error = state && state.error ? "<div class='sow-menu__status sow-menu__status--error'>" + esc(state.error) + "</div>" : "";
    var notice = state && state.notice ? "<div class='sow-menu__status sow-menu__status--notice'>" + esc(state.notice) + "</div>" : "";
    return error + notice;
}

function authSession() { return { token: "", accountId: "", user: null }; }
function authHeaders() { return { "Content-Type": "application/json", "Accept": "application/json" }; }
function ensureAuthSession() { return Promise.resolve(authSession()); }
function isAndroidTwa() { return false; }
function requestAuthOtp() {}
function verifyAuthOtp() {}
function resetAuthFlow() {
    authModalOpen = false;
    authEmail = "";
    authCode = "";
    authOtpSent = false;
    authBusy = false;
    authError = "";
    authNotice = "";
}
function storeAuth() { return { available: false, headers: {}, authSecret: "" }; }

function renderTopbar() {
    var leader = leaderById(state.selected_leader);
    var name = displayNameDraft != null ? displayNameDraft : (state.player_name || "ANONYMOUS");
    var accountXp = Math.max(0, Number(state.xp) || 0);
    var crowns = state.crowns == null ? state.laurels : state.crowns;
    return "" +
        "<header class='sow-menu__topbar'>" +
            "<div class='sow-menu__identity'>" +
                "<button class='sow-menu__avatar' type='button' data-command='open_leader_picker' " +
                    "aria-label='Select leader' style=\"background-image:url('" + esc(avatarImage()) + "')\"></button>" +
                "<div class='sow-menu__profile'>" +
                    "<input data-role='display-name' name='display_name' value=\"" + esc(name) + "\" maxlength='16' " +
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
                "<span class='sow-menu__account-label'>ANONYMOUS</span>" +
                "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='Settings'>⚙</button>" +
            "</div>" +
            "</header>";
}

// The shared shell's updater can add a sign-in button after every state tick.
// Keep the Poki top bar anonymous on updates as well as on the first render.
function updateTopbar() {
    var topbar = root && root.querySelector ? root.querySelector(".sow-menu__topbar") : null;
    if (!topbar || !state) return;
    var leader = leaderById(state.selected_leader);
    var nameInput = topbar.querySelector("[data-role='display-name']");
    var name = displayNameDraft != null ? displayNameDraft : (state.player_name || "ANONYMOUS");
    if (nameInput && document.activeElement !== nameInput) nameInput.value = name;
    if (nameInput) nameInput.readOnly = !!state.name_locked;
    var avatar = topbar.querySelector(".sow-menu__avatar");
    if (avatar) avatar.style.backgroundImage = "url(" + JSON.stringify(avatarImage()) + ")";
    var leaderLink = topbar.querySelector(".sow-menu__profile-link");
    if (leaderLink) leaderLink.textContent = leader.name + " · " + leader.civilization;
    var level = topbar.querySelector("[data-progression-level-value]");
    if (level) level.textContent = String(state.level == null ? 1 : state.level);
    var xp = topbar.querySelector("[data-progression-xp-value]");
    if (xp) xp.textContent = Math.floor(Number(state.xp) || 0) + " XP";
    var crowns = topbar.querySelector("[data-progression-crowns-value]");
    if (crowns) crowns.textContent = String(state.crowns == null ? state.laurels : state.crowns);
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
                renderFeedback() +
            "</div>" +
        "</section>";
}

function renderMainNav(active) {
    var items = [
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
    return "" +
        "<div class='sow-menu__overlay' data-menu-overlay='settings'>" +
            "<section class='sow-menu__modal sow-menu__settings-modal'>" +
                "<div class='sow-menu__modal-head'><div><p class='sow-menu__panel-label'>SYSTEM CONFIGURATION</p><h2>SETTINGS</h2></div><button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='Close'>×</button></div>" +
                "<div class='sow-menu__form-grid'>" +
                    "<section class='sow-menu__form-field sow-menu__form-field--wide sow-menu__account-row'><span>PLAYER MODE</span><strong>ANONYMOUS</strong></section>" +
                    "<label class='sow-menu__form-field sow-menu__form-field--wide'><span>MASTER AUDIO</span>" +
                        renderDropdown({ key: "settings-mute", name: "mute_all", setting: "mute", value: settings.mute_all ? "off" : "on", options: [{ value: "on", label: "AUDIO ENABLED (ON)" }, { value: "off", label: "MUTED (OFF)" }] }) +
                    "</label>" +
                    "<label class='sow-menu__form-field sow-menu__form-field--wide'><div class='sow-menu__slider-label'><span>MUSIC VOLUME</span><b data-val-for='music_vol'>" + volPct + "%</b></div><input class='sow-menu__field' type='range' name='music_volume' min='0' max='1' step='0.05' value='" + esc(vol) + "' data-setting='music_volume'></label>" +
                    "<label class='sow-menu__form-field sow-menu__form-field--wide'><span>MOTION &amp; ANIMATION</span>" +
                        renderDropdown({ key: "settings-motion", name: "reduced_motion", setting: "reduced_motion", value: settings.reduced_motion ? "reduced" : "full", options: [{ value: "full", label: "FULL" }, { value: "reduced", label: "REDUCED MOTION" }] }) +
                    "</label>" +
                "</div>" +
                "<div class='sow-menu__modal-actions'><button class='sow-menu__primary' type='button' data-command='toggle_settings'>DONE <span>✓</span></button></div>" +
            "</section>" +
        "</div>";
}

function renderFooter(label) {
    return "<footer class='sow-menu__footer'>" +
        (label ? "<span data-menu-footer-label>" + esc(label) + "</span>" : "") +
        "<nav class='sow-menu__footer-links' aria-label='Game links'><button type='button' data-command='poki_privacy'>PRIVACY</button></nav>" +
        "<span>SHADOWS OF WAR</span></footer>";
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
