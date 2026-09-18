(function () {
    "use strict";

    function escapeHtml(value) {
        return String(value == null ? "" : value)
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/\"/g, "&quot;")
            .replace(/'/g, "&#39;");
    }

    window.SOW_renderDropdown = function (config) {
        var value = config.value == null ? "" : String(config.value);
        var options = config.options || [];
        var key = String(config.key);
        var inputAttrs = " data-dropdown-value" + (config.setting ? " data-setting='" + escapeHtml(config.setting) + "'" : "");
        var selected = options.find(function (option) { return String(option.value) === value; });
        return "<div class='sow-control-dropdown" + (config.className ? " " + escapeHtml(config.className) : "") + "' data-control-dropdown data-dropdown-key='" + escapeHtml(key) + "'>" +
            "<button class='sow-control-dropdown__trigger' type='button' data-command='toggle_dropdown' data-dropdown-key='" + escapeHtml(key) + "' data-role='dropdown-trigger' aria-label='" + escapeHtml(config.label || "") + "' aria-haspopup='listbox' aria-expanded='false' aria-controls='sow-dropdown-" + escapeHtml(key) + "'>" +
                "<span data-dropdown-label>" + escapeHtml(selected ? selected.label : value) + "</span><span class='sow-control-dropdown__chevron' aria-hidden='true'>⌄</span>" +
            "</button>" +
            "<div class='sow-control-dropdown__menu' id='sow-dropdown-" + escapeHtml(key) + "' role='listbox' aria-label='" + escapeHtml(config.label || "") + "' hidden>" +
                options.map(function (option) {
                    var isSelected = String(option.value) === value;
                    return "<button class='sow-control-dropdown__option' type='button' role='option' data-command='select_dropdown' data-dropdown-key='" + escapeHtml(key) + "' data-dropdown-option-value='" + escapeHtml(option.value) + "' aria-selected='" + (isSelected ? "true" : "false") + "'>" + escapeHtml(option.label) + "</button>";
                }).join("") +
            "</div>" +
            "<input type='hidden' name='" + escapeHtml(config.name || key) + "' value='" + escapeHtml(value) + "' data-dropdown-input" + inputAttrs + ">" +
        "</div>";
    };
})();
