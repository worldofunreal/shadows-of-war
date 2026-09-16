(function () {
    "use strict";

    var STORAGE_KEY = "sow_ui_locale";
    var defaultLocale = String(window.SOW_LOCALE_DEFAULT || "en").toLowerCase();
    var catalogVersion = Number(window.SOW_LOCALE_CATALOG_VERSION || 1);
    var configuredCodes = Array.isArray(window.SOW_LOCALE_CODES) ? window.SOW_LOCALE_CODES : [defaultLocale];
    var supported = Object.create(null);
    configuredCodes.forEach(function (code) {
        var normalized = String(code || "").toLowerCase();
        if (normalized) supported[normalized] = true;
    });
    supported[defaultLocale] = true;

    var cache = Object.create(null);
    var errors = Object.create(null);
    var activeLocale = defaultLocale;
    var activeStrings = {};
    var englishStrings = {};

    function normalizeLocale(value) {
        var locale = String(value || "").toLowerCase().replace(/_/g, "-").split("-")[0];
        return supported[locale] ? locale : defaultLocale;
    }

    function storedLocale() {
        try {
            return normalizeLocale(window.localStorage.getItem(STORAGE_KEY));
        } catch (error) {
            return defaultLocale;
        }
    }

    function catalogUrl(locale) {
        var base = String(window.SOW_LOCALE_BASE || "locales").replace(/\/$/, "");
        var version = String(window.SOW_LOCALE_VERSION || "");
        return base + "/" + encodeURIComponent(locale) + (version ? "?v=" + encodeURIComponent(version) : "");
    }

    function load(locale) {
        var code = normalizeLocale(locale);
        if (cache[code]) return cache[code];
        cache[code] = fetch(catalogUrl(code), { credentials: "same-origin" }).then(function (response) {
            if (!response.ok) throw new Error("locale " + code + " returned " + response.status);
            return response.json();
        }).then(function (payload) {
            if (!payload || payload.schema !== 1 || Number(payload.version) !== catalogVersion || payload.locale !== code) {
                throw new Error("locale " + code + " has incompatible catalog metadata");
            }
            var strings = payload.strings;
            if (!strings || typeof strings !== "object") throw new Error("locale " + code + " has no strings");
            return strings;
        }).catch(function (error) {
            delete cache[code];
            throw error;
        });
        return cache[code];
    }

    function lookup(strings, key) {
        return String(key || "").split(".").reduce(function (value, part) {
            return value && typeof value === "object" ? value[part] : undefined;
        }, strings);
    }

    function report(key, error) {
        if (errors[key]) return;
        errors[key] = true;
        console.error("[SOW i18n]", key, error || "missing translation");
    }

    function translate(key, values) {
        var value = lookup(activeStrings, key);
        if (typeof value !== "string") {
            value = lookup(englishStrings, key);
            report(key);
        }
        if (typeof value !== "string") return "[" + key + "]";
        return value.replace(/\{([A-Za-z0-9_]+)\}/g, function (match, name) {
            return values && values[name] != null ? String(values[name]) : match;
        });
    }

    function saveLocale(locale) {
        try {
            window.localStorage.setItem(STORAGE_KEY, locale);
        } catch (error) {
            report("locale.preference", error);
        }
    }

    function activate(locale, strings, notify) {
        activeLocale = locale;
        activeStrings = strings;
        window.SOW_LOCALE = locale;
        saveLocale(locale);
        if (notify && typeof window.SOW_menu_locale_changed === "function") {
            window.SOW_menu_locale_changed(locale);
        }
        return strings;
    }

    window.SOW_t = translate;
    window.SOW_getLocale = function () { return activeLocale; };
    window.SOW_getSupportedLocales = function () { return Object.keys(supported); };
    window.SOW_setLocale = function (locale) {
        var requested = normalizeLocale(locale);
        return load(requested).then(function (strings) {
            return activate(requested, strings, true);
        }).catch(function (error) {
            report("locale." + requested, error);
            if (requested === defaultLocale) throw error;
            return load(defaultLocale).then(function (strings) {
                return activate(defaultLocale, strings, true);
            });
        });
    };

    var initialLocale = storedLocale();
    window.SOW_LOCALE = defaultLocale;
    window.SOW_I18N_READY = load(defaultLocale).then(function (strings) {
        englishStrings = strings;
        if (initialLocale === defaultLocale) return activate(defaultLocale, strings, false);
        return load(initialLocale).then(function (selected) {
            return activate(initialLocale, selected, false);
        }).catch(function (error) {
            report("locale." + initialLocale, error);
            return activate(defaultLocale, strings, false);
        });
    });
})();
