(function () {
    "use strict";

    var STORAGE_KEY = "sow_ui_locale";
    var runtimeScript = document.currentScript;
    var inferredBase = "locales";
    if (runtimeScript && runtimeScript.src) {
        try {
            inferredBase = new URL("locales", runtimeScript.src).toString().replace(/\/$/, "");
        } catch (error) {}
    }
    var defaultLocale = String(window.SOW_LOCALE_DEFAULT || "en").toLowerCase().replace(/_/g, "-");
    var catalogVersion = Number(window.SOW_LOCALE_CATALOG_VERSION || 0);
    var configuredCodes = Array.isArray(window.SOW_LOCALE_CODES) ? window.SOW_LOCALE_CODES : [];
    var hasConfiguredCodes = configuredCodes.length > 0;
    var supported = Object.create(null);
    var supportedCodes = [];
    configuredCodes.forEach(function (code) {
        var normalized = String(code || "").toLowerCase();
        if (normalized && !supported[normalized]) {
            supported[normalized] = true;
            supportedCodes.push(normalized);
        }
    });
    if (!supported[defaultLocale]) {
        supported[defaultLocale] = true;
        supportedCodes.unshift(defaultLocale);
    }

    var cache = Object.create(null);
    var errors = Object.create(null);
    var activeLocale = defaultLocale;
    var activeStrings = {};
    var englishStrings = {};

    function normalizeLocale(value) {
        var locale = String(value || "").toLowerCase().replace(/_/g, "-");
        if (supported[locale]) return locale;
        var aliases = { zh: "zh-cn", pt: "pt-br", tl: "fil" };
        var alias = aliases[locale];
        return alias && supported[alias] ? alias : defaultLocale;
    }

    function localeTag(locale) {
        var code = String(locale || defaultLocale).toLowerCase();
        return code === "zh-cn" ? "zh-CN" : code === "pt-br" ? "pt-BR" : code;
    }

    function localeToken(locale) {
        return String(locale || "").toLowerCase().replace(/-/g, "_");
    }

    function localeDirection(locale) {
        return String(locale || "").toLowerCase() === "ar" ? "rtl" : "ltr";
    }

    function localeScript(locale) {
        var code = String(locale || "").toLowerCase();
        if (code === "ar") return "arabic";
        if (code === "zh-cn" || code === "ja" || code === "ko") return "cjk";
        if (code === "ru") return "cyrillic";
        return "latin";
    }

    function storedLocale() {
        try {
            return window.localStorage.getItem(STORAGE_KEY) || defaultLocale;
        } catch (error) {
            return defaultLocale;
        }
    }

    function catalogUrl(locale) {
        var base = String(window.SOW_LOCALE_BASE || inferredBase).replace(/\/$/, "");
        var version = String(window.SOW_LOCALE_VERSION || "");
        return base + "/" + encodeURIComponent(locale) + (version ? "?v=" + encodeURIComponent(version) : "");
    }

    function registryUrl() {
        var base = String(window.SOW_LOCALE_BASE || inferredBase).replace(/\/$/, "");
        var version = String(window.SOW_LOCALE_VERSION || "");
        return base + "/index.json" + (version ? "?v=" + encodeURIComponent(version) : "");
    }

    function addSupportedLanguage(code) {
        var normalized = String(code || "").toLowerCase();
        if (normalized && !supported[normalized]) {
            supported[normalized] = true;
            supportedCodes.push(normalized);
        }
    }

    function loadRegistry() {
        if (hasConfiguredCodes && catalogVersion > 0) return Promise.resolve();
        return fetch(registryUrl(), { credentials: "same-origin" }).then(function (response) {
            if (!response.ok) throw new Error("locale registry returned " + response.status);
            return response.json();
        }).then(function (payload) {
            var registryVersion = Number(payload && payload.version);
            if (!payload || payload.schema !== 1 || !Number.isInteger(registryVersion) || registryVersion < 1 || !Array.isArray(payload.languages)) {
                throw new Error("locale registry has incompatible metadata");
            }
            if (catalogVersion > 0 && catalogVersion !== registryVersion) {
                throw new Error("locale registry version does not match the configured catalog");
            }
            catalogVersion = registryVersion;
            payload.languages.forEach(function (entry) {
                addSupportedLanguage(typeof entry === "string" ? entry : entry && entry.code);
            });
            if (!supported[defaultLocale]) addSupportedLanguage(defaultLocale);
        }).catch(function (error) {
            console.error("[SOW i18n] locale registry", error);
            addSupportedLanguage(defaultLocale);
        });
    }

    function load(locale) {
        var code = normalizeLocale(locale);
        if (cache[code]) return cache[code];
        cache[code] = fetch(catalogUrl(code), { credentials: "same-origin" }).then(function (response) {
            if (!response.ok) throw new Error("locale " + code + " returned " + response.status);
            return response.json();
        }).then(function (payload) {
            var payloadVersion = Number(payload && payload.version);
            if (!payload || payload.schema !== 1 || !Number.isInteger(payloadVersion) || payloadVersion < 1 || (catalogVersion > 0 && payloadVersion !== catalogVersion) || payload.locale !== code) {
                throw new Error("locale " + code + " has incompatible catalog metadata");
            }
            catalogVersion = payloadVersion;
            if (Array.isArray(payload.languages)) {
                payload.languages.forEach(function (entry) {
                    var code = typeof entry === "string" ? entry : entry && entry.code;
                    addSupportedLanguage(code);
                });
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

    function announce(locale) {
        var tag = localeTag(locale);
        if (typeof window.CustomEvent === "function") {
            window.dispatchEvent(new CustomEvent("sow:locale-change", { detail: { locale: tag } }));
        }
        if (typeof window.SOW_menu_locale_changed === "function") {
            window.SOW_menu_locale_changed(tag);
        }
    }

    function activate(locale, strings, notify) {
        activeLocale = locale;
        activeStrings = strings;
        window.SOW_LOCALE = localeTag(locale);
        if (document.documentElement) {
            document.documentElement.lang = localeTag(locale);
            document.documentElement.dir = localeDirection(locale);
            document.documentElement.dataset.locale = localeTag(locale);
            document.documentElement.dataset.localeScript = localeScript(locale);
        }
        saveLocale(locale);
        if (notify) announce(locale);
        return strings;
    }

    window.SOW_t = translate;
    window.SOW_getLocale = function () { return localeTag(activeLocale); };
    window.SOW_getSupportedLocales = function () { return supportedCodes.map(localeTag); };
    window.SOW_getLocaleTag = function (locale) { return localeTag(normalizeLocale(locale)); };
    window.SOW_getLocaleDirection = function (locale) { return localeDirection(normalizeLocale(locale)); };
    window.SOW_getLocaleScript = function (locale) { return localeScript(normalizeLocale(locale)); };
    window.SOW_getLocaleLabel = function (locale) {
        return translate("menu.language_" + localeToken(normalizeLocale(locale)));
    };
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

    window.SOW_LOCALE = defaultLocale;
    var initialLocale = defaultLocale;
    window.SOW_I18N_READY = loadRegistry().then(function () {
        initialLocale = normalizeLocale(storedLocale());
        return load(defaultLocale);
    }).then(function (strings) {
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
