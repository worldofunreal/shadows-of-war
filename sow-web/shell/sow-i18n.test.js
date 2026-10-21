const assert = require("node:assert/strict");
const fs = require("node:fs");
const test = require("node:test");
const vm = require("node:vm");

const runtimeSource = fs.readFileSync(__dirname + "/sow-i18n.js", "utf8");
const siteChromeSource = fs.readFileSync(__dirname + "/../site/site-chrome.js", "utf8");

function catalog(locale, settings, version = 11) {
    return {
        schema: 1,
        version,
        locale,
        languages: [
            { code: "en", name: "English" },
            { code: "es", name: "Español" },
            { code: "ar", name: "العربية" },
            { code: "bad", name: "Bad" }
        ],
        strings: { menu: { settings, missing: "" } }
    };
}

function loadRuntime(storedLocale, catalogs, settings) {
    const storage = new Map([["sow_ui_locale", storedLocale]]);
    const errors = [];
    const requests = [];
    const localeEvents = [];
    const listeners = Object.create(null);
    const document = {
        currentScript: { src: "https://example.test/play/sow-i18n.js" },
        documentElement: { lang: "", dir: "", dataset: {} },
        querySelectorAll: () => [],
        querySelector: () => null,
        addEventListener() {}
    };
    const window = {
        SOW_LOCALE_DEFAULT: "en",
        SOW_LOCALE_CATALOG_VERSION: 11,
        SOW_LOCALE_CODES: ["en", "es", "ar", "fa", "fa-ir", "he", "ur", "yi", "bad"],
        SOW_LOCALE_BASE: "locales",
        localStorage: {
            getItem: key => storage.get(key) || null,
            setItem: (key, value) => storage.set(key, value)
        },
        addEventListener: (type, handler) => {
            (listeners[type] ||= []).push(handler);
        },
        dispatchEvent: event => {
            if (event.type === "sow:locale-change") localeEvents.push(event.detail.locale);
            (listeners[event.type] || []).forEach(handler => handler(event));
        }
    };
    function CustomEvent(type, init) {
        this.type = type;
        this.detail = init && init.detail;
    }
    const fetch = async url => {
        const code = String(url).match(/\/([^/?]+)(?:\?|$)/)[1];
        requests.push(code);
        if (settings && settings.beforeResponse) await settings.beforeResponse(code);
        if (!catalogs[code]) return { ok: false, status: 404 };
        return { ok: true, json: async () => structuredClone(catalogs[code]) };
    };
    window.CustomEvent = CustomEvent;
    if (settings && settings.window) Object.assign(window, settings.window);
    const hasNavigatorOverride = settings && Object.prototype.hasOwnProperty.call(settings, "navigator");
    const context = {
        window,
        document,
        URL,
        Promise,
        fetch,
        CustomEvent,
        navigator: hasNavigatorOverride ? settings.navigator : { languages: ["en-US"], language: "en-US" },
        AbortController,
        setTimeout,
        clearTimeout,
        console: { error: (...args) => errors.push(args) }
    };
    if (settings && settings.context) Object.assign(context, settings.context);
    vm.runInNewContext(runtimeSource, context, { filename: "sow-i18n.js" });
    return { window, document, storage, errors, requests, localeEvents, context };
}

test("public site resolves its language before revealing translated text", async () => {
    let finishSpanish;
    const spanishDownload = new Promise(resolve => { finishSpanish = resolve; });
    const runtime = loadRuntime("", {
        en: catalog("en", "Settings"),
        es: catalog("es", "Configuración")
    }, {
        window: { SOW_LOCALE_BEFORE_PAINT: true, SOW_LOCALE_CODES: ["en", "es"] },
        navigator: { languages: ["es-MX", "en-US"] },
        beforeResponse: code => code === "es" ? spanishDownload : undefined
    });
    const text = { dataset: { i18n: "menu.settings" }, textContent: "Settings" };
    runtime.document.querySelectorAll = selector => selector === "[data-i18n]" ? [text] : [];
    vm.runInNewContext(siteChromeSource, runtime.context);

    assert.equal(runtime.document.documentElement.lang, "es");
    assert.equal(runtime.document.documentElement.dataset.sowLocalePending, "true");
    await new Promise(setImmediate);
    assert.deepEqual(runtime.requests, ["en", "es"]);
    assert.equal(text.textContent, "Settings");
    assert.equal(runtime.document.documentElement.dataset.sowLocalePending, "true");

    finishSpanish();
    await runtime.window.SOW_I18N_READY;
    await new Promise(setImmediate);
    assert.equal(runtime.document.documentElement.dataset.sowLocalePending, undefined);
    assert.equal(runtime.storage.get("sow_ui_locale"), "es");
});

test("public site reveals English when translations fail or time out", async () => {
    for (const failure of ["es", "en", "timeout"]) {
        const catalogs = { en: catalog("en", "Settings"), es: catalog("es", "Configuración") };
        delete catalogs[failure];
        const timeouts = [];
        const context = failure === "timeout" ? {
            setTimeout: callback => { timeouts.push(callback); return callback; },
            clearTimeout() {},
            fetch: (url, options) => new Promise((resolve, reject) => {
                options.signal.addEventListener("abort", () => reject(new Error("download timed out")));
            })
        } : {};
        const runtime = loadRuntime("", catalogs, {
            window: { SOW_LOCALE_BEFORE_PAINT: true, SOW_LOCALE_CODES: ["en", "es"] },
            navigator: { languages: ["es-MX", "en-US"] },
            context
        });
        const text = { dataset: { i18n: "menu.settings" }, textContent: "Settings" };
        runtime.document.querySelectorAll = selector => selector === "[data-i18n]" ? [text] : [];
        vm.runInNewContext(siteChromeSource, runtime.context);
        if (failure === "timeout") {
            await new Promise(setImmediate);
            assert.equal(timeouts.length, 2);
            timeouts.forEach(callback => callback());
        }
        if (failure === "es") await runtime.window.SOW_I18N_READY;
        else await assert.rejects(runtime.window.SOW_I18N_READY);
        await new Promise(setImmediate);
        assert.equal(runtime.document.documentElement.lang, "en", failure);
        assert.equal(runtime.document.documentElement.dataset.sowLocalePending, undefined, failure);
        assert.equal(text.textContent, "Settings", failure);
        assert.ok(runtime.errors.length > 0, failure);
    }
});

test("i18n runtime falls back, caches, persists, and updates direction", async () => {
    const runtime = loadRuntime("es", {
        en: catalog("en", "Settings"),
        es: catalog("es", "Configuración"),
        ar: catalog("ar", "الإعدادات")
    });
    await runtime.window.SOW_I18N_READY;

    assert.equal(runtime.window.SOW_getLocale(), "es");
    assert.equal(runtime.document.documentElement.lang, "es");
    assert.equal(runtime.window.SOW_t("menu.settings"), "Configuración");
    assert.deepEqual(runtime.requests, ["en", "es"]);

    await runtime.window.SOW_setLocale("ar");
    assert.equal(runtime.window.SOW_getLocale(), "ar");
    assert.equal(runtime.document.documentElement.dir, "rtl");
    for (const locale of ["ar", "fa", "fa-ir", "he", "ur", "yi"]) assert.equal(runtime.window.SOW_getLocaleDirection(locale), "rtl");
    assert.equal(runtime.window.SOW_getLocaleDirection("en"), "ltr");
    assert.equal(runtime.storage.get("sow_ui_locale"), "ar");
    assert.deepEqual(runtime.localeEvents, ["ar"]);

    await runtime.window.SOW_setLocale("ar");
    assert.deepEqual(runtime.requests, ["en", "es", "ar"]);
});

test("i18n only warns when the active and English catalogs both miss a key", async () => {
    const runtime = loadRuntime("es", {
        en: catalog("en", "Settings"),
        es: catalog("es", "Settings")
    });
    delete runtime.window.SOW_LOCALE_CODES;
    await runtime.window.SOW_I18N_READY;

    assert.equal(runtime.window.SOW_t("menu.settings"), "Settings");
    assert.equal(runtime.errors.length, 0);
    assert.equal(runtime.window.SOW_t("menu.not_in_catalog"), "[menu.not_in_catalog]");
    assert.equal(runtime.errors.length, 1);
});

test("i18n rejects an incompatible catalog and returns to English", async () => {
    const runtime = loadRuntime("en", {
        en: catalog("en", "Settings"),
        bad: catalog("bad", "Bad", 9)
    });
    await runtime.window.SOW_I18N_READY;
    await runtime.window.SOW_setLocale("bad");

    assert.equal(runtime.window.SOW_getLocale(), "en");
    assert.equal(runtime.document.documentElement.dir, "ltr");
    assert.equal(runtime.localeEvents.length, 1);
    assert.ok(runtime.errors.length >= 1);
});

test("i18n resolves a usable locale from choice, portal default, then browser", async () => {
    const codes = ["en", "es", "pt-br", "ja"];
    const catalogs = {
        en: catalog("en", "Settings"),
        es: catalog("es", "Configuración"),
        "pt-br": catalog("pt-br", "Configurações"),
        ja: catalog("ja", "設定")
    };

    // A regional browser tag falls back to the published base code.
    const regional = loadRuntime("", catalogs, {
        window: { SOW_LOCALE_CODES: codes, SOW_LOCALE_DEFAULT: "" },
        navigator: { languages: ["es-MX"], language: "es-MX" }
    });
    await regional.window.SOW_I18N_READY;
    assert.equal(regional.window.SOW_getLocale(), "es");

    // ...even when the base tag only exists as an alias (pt-PT -> pt-br).
    const portugal = loadRuntime("pt-PT", catalogs, {
        window: { SOW_LOCALE_CODES: codes, SOW_LOCALE_DEFAULT: "" },
        navigator: { languages: ["ja-JP"], language: "ja-JP" }
    });
    await portugal.window.SOW_I18N_READY;
    assert.equal(portugal.window.SOW_getLocale(), "pt-BR");

    // An explicit choice always beats the browser.
    const chosen = loadRuntime("es", catalogs, {
        window: { SOW_LOCALE_CODES: codes, SOW_LOCALE_DEFAULT: "" },
        navigator: { languages: ["ja-JP"], language: "ja-JP" }
    });
    await chosen.window.SOW_I18N_READY;
    assert.equal(chosen.window.SOW_getLocale(), "es");

    // A build-level default (portal) beats the browser until someone chooses.
    const portal = loadRuntime("", catalogs, {
        window: { SOW_LOCALE_CODES: codes, SOW_LOCALE_DEFAULT: "pt-BR" },
        navigator: { languages: ["ja-JP"], language: "ja-JP" }
    });
    await portal.window.SOW_I18N_READY;
    assert.equal(portal.window.SOW_getLocale(), "pt-BR");
});

test("browser detection keeps English, chooses the first supported preference, and caches it", async () => {
    const codes = ["en", "es", "pt-br", "ja"];
    const catalogs = {
        en: catalog("en", "Settings"),
        es: catalog("es", "Configuración"),
        "pt-br": catalog("pt-br", "Configurações"),
        ja: catalog("ja", "設定")
    };

    // The game shell's English default is a fallback, not a reason to ignore
    // English or another supported browser preference.
    const englishFirst = loadRuntime("", catalogs, {
        window: { SOW_LOCALE_CODES: codes },
        navigator: { languages: ["en-US", "es-MX"], language: "en-US" }
    });
    await englishFirst.window.SOW_I18N_READY;
    assert.equal(englishFirst.window.SOW_getLocale(), "en");
    assert.equal(englishFirst.storage.get("sow_ui_locale"), "en");

    const spanishFirst = loadRuntime("", catalogs, {
        window: { SOW_LOCALE_CODES: codes },
        navigator: { languages: ["es-MX", "en-US"], language: "es-MX" }
    });
    await spanishFirst.window.SOW_I18N_READY;
    assert.equal(spanishFirst.window.SOW_getLocale(), "es");
    assert.equal(spanishFirst.storage.get("sow_ui_locale"), "es");

    const unsupportedThenEnglish = loadRuntime("", catalogs, {
        window: { SOW_LOCALE_CODES: codes },
        navigator: { languages: ["nl-NL", "en-US"], language: "nl-NL" }
    });
    await unsupportedThenEnglish.window.SOW_I18N_READY;
    assert.equal(unsupportedThenEnglish.window.SOW_getLocale(), "en");

    const unsupportedOnly = loadRuntime("", catalogs, {
        window: { SOW_LOCALE_CODES: codes },
        navigator: { languages: ["nl-NL"], language: "nl-NL" }
    });
    await unsupportedOnly.window.SOW_I18N_READY;
    assert.equal(unsupportedOnly.window.SOW_getLocale(), "en");

    // An empty preference list uses navigator.language when available.
    const languageFallback = loadRuntime("", catalogs, {
        window: { SOW_LOCALE_CODES: codes },
        navigator: { languages: [], language: "es-MX" }
    });
    await languageFallback.window.SOW_I18N_READY;
    assert.equal(languageFallback.window.SOW_getLocale(), "es");

    const noNavigator = loadRuntime("", catalogs, {
        window: { SOW_LOCALE_CODES: codes },
        navigator: null
    });
    await noNavigator.window.SOW_I18N_READY;
    assert.equal(noNavigator.window.SOW_getLocale(), "en");

    const inaccessibleNavigator = {};
    Object.defineProperty(inaccessibleNavigator, "languages", { get() { throw new Error("unavailable"); } });
    Object.defineProperty(inaccessibleNavigator, "language", { get() { throw new Error("unavailable"); } });
    const unreadableNavigator = loadRuntime("", catalogs, {
        window: { SOW_LOCALE_CODES: codes },
        navigator: inaccessibleNavigator
    });
    await unreadableNavigator.window.SOW_I18N_READY;
    assert.equal(unreadableNavigator.window.SOW_getLocale(), "en");
});
