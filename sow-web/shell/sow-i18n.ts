(function () {
    "use strict";

    const STORAGE_KEY = "sow_ui_locale";
    const rtlLanguages = new Set([
        "ar",
        "arc",
        "ckb",
        "dv",
        "fa",
        "he",
        "iw",
        "nqo",
        "pnb",
        "ps",
        "sd",
        "syr",
        "ug",
        "ur",
        "yi",
    ]);
    const runtimeScript = document.currentScript;
    let inferredBase = "locales";
    if (runtimeScript && runtimeScript.src) {
        try {
            inferredBase = new URL("locales", runtimeScript.src).toString().replace(/\/$/, "");
        } catch {}
    }
    const configuredDefault = String(window.SOW_LOCALE_DEFAULT || "")
        .toLowerCase()
        .replace(/_/g, "-");
    const defaultLocale = "en";

    // Resolve only published languages so an unsupported browser tag cannot
    // masquerade as English while scanning the browser's preference list.
    function supportedLocale(value) {
        const locale = String(value || "")
            .toLowerCase()
            .replace(/_/g, "-");
        if (!locale) return "";
        if (supported[locale]) return locale;
        const aliases = { zh: "zh-cn", pt: "pt-br", tl: "fil" };
        const alias = aliases[locale];
        if (alias && supported[alias]) return alias;
        const base = locale.split("-")[0];
        if (supported[base]) return base;
        const baseAlias = aliases[base];
        if (baseAlias && supported[baseAlias]) return baseAlias;
        return "";
    }

    // Other callers need a usable locale even when their input is unknown.
    function normalizeLocale(value) {
        return supportedLocale(value) || defaultLocale;
    }

    function browserLanguages() {
        try {
            if (typeof navigator !== "undefined" && navigator && Array.isArray(navigator.languages)) {
                const languages = navigator.languages.filter(function (language) {
                    return typeof language === "string" && language.trim();
                });
                if (languages.length) return languages;
            }
        } catch {}
        try {
            if (
                typeof navigator !== "undefined" &&
                navigator &&
                typeof navigator.language === "string" &&
                navigator.language
            ) {
                return [navigator.language];
            }
        } catch {}
        return [];
    }

    function preferredLocale() {
        let stored = "";
        try {
            stored = String(window.localStorage.getItem(STORAGE_KEY) || "");
        } catch {}
        // An explicit choice (or a build-level default, e.g. a portal) always
        // wins; the browser only decides when nobody has chosen yet.
        const saved = supportedLocale(stored);
        if (saved) return saved;
        const configured = supportedLocale(configuredDefault);
        // A non-English portal locale is intentional. English is the app's
        // fallback, so it must not suppress a supported browser preference.
        if (configured && configured !== defaultLocale) return configured;
        const browser = browserLanguages();
        for (let i = 0; i < browser.length; i++) {
            const candidate = supportedLocale(browser[i]);
            if (candidate) return candidate;
        }
        return configured || defaultLocale;
    }

    let catalogVersion = Number(window.SOW_LOCALE_CATALOG_VERSION || 0);
    const configuredCodes = Array.isArray(window.SOW_LOCALE_CODES) ? window.SOW_LOCALE_CODES : [];
    const hasConfiguredCodes = configuredCodes.length > 0;
    const supported = Object.create(null);
    const supportedCodes = [];
    configuredCodes.forEach(function (code) {
        const normalized = String(code || "").toLowerCase();
        if (normalized && !supported[normalized]) {
            supported[normalized] = true;
            supportedCodes.push(normalized);
        }
    });
    if (!supported[defaultLocale]) {
        supported[defaultLocale] = true;
        supportedCodes.unshift(defaultLocale);
    }

    const cache = Object.create(null);
    const errors = Object.create(null);
    let activeLocale = defaultLocale;
    let activeStrings = {};
    let englishStrings = {};
    let englishReady = false;

    function localeTag(locale) {
        const code = String(locale || defaultLocale).toLowerCase();
        return code === "zh-cn" ? "zh-CN" : code === "pt-br" ? "pt-BR" : code;
    }

    function localeToken(locale) {
        return String(locale || "")
            .toLowerCase()
            .replace(/-/g, "_");
    }

    function localeDirection(locale) {
        return rtlLanguages.has(
            String(locale || "")
                .toLowerCase()
                .split("-")[0],
        )
            ? "rtl"
            : "ltr";
    }

    function localeScript(locale) {
        const code = String(locale || "").toLowerCase();
        if (code === "ar") return "arabic";
        if (code === "zh-cn" || code === "ja" || code === "ko") return "cjk";
        if (code === "ru") return "cyrillic";
        return "latin";
    }

    function localeVersion(key) {
        // Content hash first (server ships /locales/* hashed), published
        // catalog version as fallback, then the build stamp. Same bytes keep
        // their URL across deploys; changed bytes are instantly visible.
        const versions = window.SOW_ASSET_TS || {};
        return String(versions[key] || window.SOW_LOCALE_VERSION || window.SOW_BUILD_TS || "");
    }

    function catalogUrl(locale) {
        const base = String(window.SOW_LOCALE_BASE || inferredBase).replace(/\/$/, "");
        const version = localeVersion("/locales/" + locale);
        return base + "/" + encodeURIComponent(locale) + (version ? "?v=" + encodeURIComponent(version) : "");
    }

    function registryUrl() {
        const base = String(window.SOW_LOCALE_BASE || inferredBase).replace(/\/$/, "");
        const version = localeVersion("/locales/index.json");
        return base + "/index.json" + (version ? "?v=" + encodeURIComponent(version) : "");
    }

    function addSupportedLanguage(code) {
        const normalized = String(code || "").toLowerCase();
        if (normalized && !supported[normalized]) {
            supported[normalized] = true;
            supportedCodes.push(normalized);
        }
    }

    function fetchCatalog(url) {
        const controller = new AbortController();
        // A public page must be able to show its English HTML if a download stalls.
        const timeout = setTimeout(function () {
            controller.abort();
        }, 8000);
        return fetch(url, { credentials: "same-origin", signal: controller.signal })
            .then(function (response) {
                if (!response.ok) throw new Error("locale request returned " + response.status);
                return response.json();
            })
            .finally(function () {
                clearTimeout(timeout);
            });
    }

    function loadRegistry() {
        if (hasConfiguredCodes && catalogVersion > 0) return Promise.resolve();
        return fetchCatalog(registryUrl())
            .then(function (payload) {
                const registryVersion = Number(payload && payload.version);
                if (
                    !payload ||
                    payload.schema !== 1 ||
                    !Number.isInteger(registryVersion) ||
                    registryVersion < 1 ||
                    !Array.isArray(payload.languages)
                ) {
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
            })
            .catch(function (error) {
                console.error("[SOW i18n] locale registry", error);
                addSupportedLanguage(defaultLocale);
            });
    }

    function load(locale) {
        const code = normalizeLocale(locale);
        if (cache[code]) return cache[code];
        cache[code] = fetchCatalog(catalogUrl(code))
            .then(function (payload) {
                const payloadVersion = Number(payload && payload.version);
                if (
                    !payload ||
                    payload.schema !== 1 ||
                    !Number.isInteger(payloadVersion) ||
                    payloadVersion < 1 ||
                    (catalogVersion > 0 && payloadVersion !== catalogVersion) ||
                    payload.locale !== code
                ) {
                    throw new Error("locale " + code + " has incompatible catalog metadata");
                }
                catalogVersion = payloadVersion;
                if (Array.isArray(payload.languages)) {
                    payload.languages.forEach(function (entry) {
                        const code = typeof entry === "string" ? entry : entry && entry.code;
                        addSupportedLanguage(code);
                    });
                }
                const strings = payload.strings;
                if (!strings || typeof strings !== "object")
                    throw new Error("locale " + code + " has no strings");
                return strings;
            })
            .catch(function (error) {
                delete cache[code];
                throw error;
            });
        return cache[code];
    }

    function lookup(strings, key) {
        return String(key || "")
            .split(".")
            .reduce(function (value, part) {
                return value && typeof value === "object" ? value[part] : undefined;
            }, strings);
    }

    function report(key, error?) {
        if (errors[key]) return;
        errors[key] = true;
        console.error("[SOW i18n]", key, error || "missing translation");
    }

    function translate(key, values?) {
        let value = lookup(activeStrings, key);
        if (typeof value !== "string") {
            value = lookup(englishStrings, key);
            if (typeof value !== "string") {
                if (!englishReady) return "[" + key + "]";
                report(key);
            }
        }
        if (typeof value !== "string") return "[" + key + "]";
        return value.replace(/\{([A-Za-z0-9_]+)\}/g, function (match, name) {
            return values && values[name] != null ? String(values[name]) : match;
        });
    }

    function hasText(key) {
        let value = lookup(activeStrings, key);
        if (typeof value !== "string") value = lookup(englishStrings, key);
        return typeof value === "string" && value.trim().length > 0;
    }

    function saveLocale(locale) {
        try {
            window.localStorage.setItem(STORAGE_KEY, locale);
        } catch (error) {
            report("locale.preference", error);
        }
    }

    function announce(locale) {
        const tag = localeTag(locale);
        if (typeof window.CustomEvent === "function") {
            window.dispatchEvent(new CustomEvent("sow:locale-change", { detail: { locale: tag } }));
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
    window.SOW_hasText = hasText;
    window.SOW_getLocale = function () {
        return localeTag(activeLocale);
    };
    window.SOW_getSupportedLocales = function () {
        return supportedCodes.map(localeTag);
    };
    window.SOW_getLocaleTag = function (locale) {
        return localeTag(normalizeLocale(locale));
    };
    window.SOW_getLocaleDirection = function (locale) {
        return localeDirection(normalizeLocale(locale));
    };
    window.SOW_getLocaleScript = function (locale) {
        return localeScript(normalizeLocale(locale));
    };
    window.SOW_getLocaleLabel = function (locale) {
        return translate("menu.language_" + localeToken(normalizeLocale(locale)));
    };
    window.SOW_setLocale = function (locale) {
        const requested = normalizeLocale(locale);
        return load(requested)
            .then(function (strings) {
                return activate(requested, strings, true);
            })
            .catch(function (error) {
                report("locale." + requested, error);
                if (requested === defaultLocale) throw error;
                return load(defaultLocale).then(function (strings) {
                    return activate(defaultLocale, strings, true);
                });
            });
    };

    window.SOW_LOCALE = defaultLocale;
    let initialLocale = hasConfiguredCodes && catalogVersion > 0 ? preferredLocale() : defaultLocale;
    if (window.SOW_LOCALE_BEFORE_PAINT && document.documentElement) {
        document.documentElement.lang = localeTag(initialLocale);
        document.documentElement.dir = localeDirection(initialLocale);
        // The shared selector runs in the head. Site chrome reveals the page
        // only after applying its translations, so English cannot flash first.
        if (!hasConfiguredCodes || catalogVersion < 1 || initialLocale !== defaultLocale) {
            document.documentElement.dataset.sowLocalePending = "true";
        }
    }
    window.SOW_I18N_READY = loadRegistry()
        .then(function () {
            initialLocale = preferredLocale();
            const english = load(defaultLocale);
            const selected =
                initialLocale === defaultLocale
                    ? Promise.resolve(null)
                    : load(initialLocale).catch(function (error) {
                          report("locale." + initialLocale, error);
                          return null;
                      });
            return Promise.all([english, selected]);
        })
        .then(function (catalogs) {
            englishStrings = catalogs[0];
            englishReady = true;
            return activate(
                catalogs[1] ? initialLocale : defaultLocale,
                catalogs[1] || englishStrings,
                false,
            );
        });
})();
