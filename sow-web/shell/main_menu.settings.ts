import { esc, menuUi } from "./main_menu.core.ts";

export function renderMusicSpectrum() {
    return "<canvas class='sow-menu__music-spectrum' data-menu-music-spectrum aria-hidden='true' hidden></canvas>";
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
