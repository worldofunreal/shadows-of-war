import { esc } from "./main_menu.core.ts";

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
        "<div class='sow-hud__setting-row sow-hud__setting-card sow-menu__settings-dropdown-row'><span>" +
        esc(label) +
        "</span>" +
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
