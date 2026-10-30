export function resetDropdownPosition(menu: HTMLElement) {
    menu.style.position = "";
    menu.style.insetInlineEnd = "";
    menu.style.left = "";
    menu.style.right = "";
    menu.style.top = "";
    menu.style.width = "";
    menu.style.minWidth = "";
    menu.style.maxHeight = "";
}

function positionDropdown(trigger: Element, menu: HTMLElement) {
    const rect = trigger.getBoundingClientRect();
    const viewportWidth = document.documentElement.clientWidth || window.innerWidth;
    const viewportHeight = document.documentElement.clientHeight || window.innerHeight;
    const edge = 8;
    const gap = 6;
    const maxHeight = Math.min(240, Math.max(40, viewportHeight - edge * 2));
    const width = Math.max(0, Math.min(viewportWidth - edge * 2, rect.width));
    const rtl = document.documentElement.dir === "rtl";
    let left = rtl ? rect.right - width : rect.left;
    left = Math.max(edge, Math.min(left, viewportWidth - width - edge));

    menu.style.position = "fixed";
    menu.style.insetInlineEnd = "auto";
    menu.style.left = left + "px";
    menu.style.right = "auto";
    menu.style.width = width + "px";
    menu.style.minWidth = width + "px";

    const desiredHeight = Math.min(menu.scrollHeight, maxHeight);
    const below = viewportHeight - rect.bottom - gap - edge;
    const above = rect.top - gap - edge;
    const opensBelow = below >= desiredHeight || below >= above;
    const available = opensBelow ? below : above;
    const menuMaxHeight = Math.max(40, Math.min(maxHeight, available));
    menu.style.maxHeight = menuMaxHeight + "px";

    const height = Math.min(menu.scrollHeight, menuMaxHeight);
    const top = opensBelow ? rect.bottom + gap : rect.top - gap - height;
    menu.style.top = Math.max(edge, Math.min(top, viewportHeight - height - edge)) + "px";
}

export function syncDropdownScope(
    scope: ParentNode,
    openKey: string | null,
    focusTarget?: HTMLElement | null,
) {
    const dropdowns = scope.querySelectorAll<HTMLElement>("[data-control-dropdown]");
    for (let i = 0; i < dropdowns.length; i++) {
        const dropdown = dropdowns[i];
        const key = dropdown.dataset.dropdownKey;
        const open = openKey === key;
        const trigger = dropdown.querySelector<HTMLElement>("[data-role='dropdown-trigger']");
        const menu = dropdown.querySelector<HTMLElement>("[role='listbox']");
        if (!trigger || !menu) continue;
        trigger.setAttribute("aria-expanded", String(open));
        menu.hidden = !open;
        if (open) positionDropdown(trigger, menu);
        else resetDropdownPosition(menu);
        const input = dropdown.querySelector<HTMLInputElement>("[data-dropdown-value]");
        const value = input ? input.value : "";
        const label = dropdown.querySelector<HTMLElement>("[data-dropdown-label]");
        const options = dropdown.querySelectorAll<HTMLElement>("[role='option']");
        for (let j = 0; j < options.length; j++) {
            const selected = options[j].dataset.dropdownOptionValue === value;
            options[j].setAttribute("aria-selected", String(selected));
            if (label && selected) label.textContent = options[j].textContent;
        }
    }
    if (focusTarget) focusTarget.focus();
}

(function () {
    "use strict";

    function escapeHtml(value: unknown) {
        return String(value == null ? "" : value)
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#39;");
    }

    window.SOW_renderDropdown = function (config: Parameters<typeof SOW_renderDropdown>[0]) {
        const value = config.value == null ? "" : String(config.value);
        const options = config.options || [];
        const key = String(config.key);
        const inputAttrs =
            " data-dropdown-value" +
            (config.setting ? " data-setting='" + escapeHtml(config.setting) + "'" : "");
        const selected = options.find(function (option) {
            return String(option.value) === value;
        });
        return (
            "<div class='sow-control-dropdown" +
            (config.className ? " " + escapeHtml(config.className) : "") +
            "' data-control-dropdown data-dropdown-key='" +
            escapeHtml(key) +
            "'>" +
            "<button class='sow-control-dropdown__trigger' type='button' data-command='toggle_dropdown' data-dropdown-key='" +
            escapeHtml(key) +
            "' data-role='dropdown-trigger' aria-label='" +
            escapeHtml(config.label || "") +
            "' aria-haspopup='listbox' aria-expanded='false' aria-controls='sow-dropdown-" +
            escapeHtml(key) +
            "'>" +
            "<span data-dropdown-label>" +
            escapeHtml(selected ? selected.label : value) +
            "</span><span class='sow-control-dropdown__chevron' aria-hidden='true'>⌄</span>" +
            "</button>" +
            "<div class='sow-control-dropdown__menu' id='sow-dropdown-" +
            escapeHtml(key) +
            "' role='listbox' aria-label='" +
            escapeHtml(config.label || "") +
            "' hidden>" +
            options
                .map(function (option) {
                    const isSelected = String(option.value) === value;
                    return (
                        "<button class='sow-control-dropdown__option' type='button' role='option' data-command='select_dropdown' data-dropdown-key='" +
                        escapeHtml(key) +
                        "' data-dropdown-option-value='" +
                        escapeHtml(option.value) +
                        "' aria-selected='" +
                        (isSelected ? "true" : "false") +
                        "'>" +
                        escapeHtml(option.label) +
                        "</button>"
                    );
                })
                .join("") +
            "</div>" +
            "<input type='hidden' name='" +
            escapeHtml(config.name || key) +
            "' value='" +
            escapeHtml(value) +
            "' data-dropdown-input" +
            inputAttrs +
            ">" +
            "</div>"
        );
    };
})();
