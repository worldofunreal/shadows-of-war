import { esc, menuUi } from "./main_menu.core.ts";

export function renderMusicSpectrum() {
    return "<canvas class='sow-menu__music-spectrum' data-menu-music-spectrum aria-hidden='true' hidden></canvas>";
}

const SFX_SVG =
    "<svg class='sow-menu__settings-row-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><line x1='4' y1='10' x2='4' y2='14'/><line x1='8' y1='6' x2='8' y2='18'/><line x1='12' y1='3' x2='12' y2='21'/><line x1='16' y1='7' x2='16' y2='17'/><line x1='20' y1='10' x2='20' y2='14'/></svg>";

export function renderSfxVolumeSettingRow() {
    const settings = menuUi.state?.settings;
    const raw = settings?.sfx_volume == null ? 0.8 : Number(settings.sfx_volume);
    const vol = Number.isFinite(raw) ? Math.max(0, Math.min(1, raw)) : 0.8;
    const volPct = Math.round(vol * 100);
    return (
        "<label class='sow-hud__setting-row sow-hud__setting-row--slider'>" +
        "<div class='sow-menu__slider-label'><span class='sow-menu__settings-row-label'>" +
        SFX_SVG +
        "<span>" +
        esc(SOW_t("menu.sfx_volume")) +
        "</span></span><b data-val-for='sfx_vol'>" +
        volPct +
        "%</b></div>" +
        "<input type='range' name='sfx_volume' min='0' max='1' step='0.01' value='" +
        esc(vol) +
        "' style='--sow-slider-fill: " +
        volPct +
        "%;' data-setting='sfx_volume'>" +
        "</label>"
    );
}

const GLOBE_SVG =
    "<svg class='sow-menu__settings-row-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><circle cx='12' cy='12' r='9'/><path d='M3 12h18'/><path d='M12 3a14 14 0 0 1 0 18'/><path d='M12 3a14 14 0 0 0 0 18'/></svg>";

const ADVISOR_SVG =
    "<svg class='sow-menu__settings-row-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M12 2a7 7 0 0 0-4 12.74V17a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1v-2.26A7 7 0 0 0 12 2Z'/><path d='M9 21h6'/></svg>";

export function localeOptions() {
    const codes =
        typeof window.SOW_getSupportedLocales === "function" ? window.SOW_getSupportedLocales() : [];
    return codes.map(function (code) {
        return {
            value: code,
            label:
                typeof window.SOW_getLocaleLabel === "function"
                    ? window.SOW_getLocaleLabel(code)
                    : SOW_t("menu.language_" + String(code).replace(/-/g, "_")),
        };
    });
}

export function renderLocaleSettingRow(dropdownKey = "settings-language") {
    const label = SOW_t("menu.language");
    return (
        "<div class='sow-hud__setting-row sow-hud__setting-card sow-menu__settings-dropdown-row'>" +
        "<span class='sow-menu__settings-row-label'>" +
        GLOBE_SVG +
        "<span>" +
        esc(label) +
        "</span></span>" +
        SOW_renderDropdown({
            key: dropdownKey,
            className: "sow-menu__settings-dropdown",
            label: label,
            name: "locale",
            setting: "locale",
            value: typeof window.SOW_getLocale === "function" ? window.SOW_getLocale() : "en",
            options: localeOptions(),
        }) +
        "</div>"
    );
}

export function renderAdvisorTipsSettingRow() {
    const enabled = !Boolean(menuUi.state && menuUi.state.menu_tips_disabled);
    return (
        "<label class='sow-hud__setting-row sow-menu__settings-switch-row'>" +
        "<span class='sow-menu__settings-row-label'>" +
        ADVISOR_SVG +
        "<span>" +
        esc(SOW_t("menu.advisor_tips")) +
        "</span></span>" +
        "<input class='sow-hud__switch' type='checkbox' role='switch' name='menu_tips' data-setting='menu_tips'" +
        (enabled ? " checked" : "") +
        ">" +
        "</label>"
    );
}
