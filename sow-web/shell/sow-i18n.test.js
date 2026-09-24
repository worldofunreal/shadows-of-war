const assert = require("node:assert/strict");
const fs = require("node:fs");
const test = require("node:test");
const vm = require("node:vm");

const runtimeSource = fs.readFileSync(__dirname + "/sow-i18n.js", "utf8");

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

function loadRuntime(storedLocale, catalogs) {
    const storage = new Map([["sow_ui_locale", storedLocale]]);
    const errors = [];
    const requests = [];
    const localeEvents = [];
    const listeners = Object.create(null);
    const document = {
        currentScript: { src: "https://example.test/play/sow-i18n.js" },
        documentElement: { lang: "", dir: "", dataset: {} }
    };
    const window = {
        SOW_LOCALE_DEFAULT: "en",
        SOW_LOCALE_CATALOG_VERSION: 11,
        SOW_LOCALE_CODES: ["en", "es", "ar", "bad"],
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
        if (!catalogs[code]) return { ok: false, status: 404 };
        return { ok: true, json: async () => structuredClone(catalogs[code]) };
    };
    window.CustomEvent = CustomEvent;
    const context = {
        window,
        document,
        URL,
        Promise,
        fetch,
        CustomEvent,
        console: { error: (...args) => errors.push(args) }
    };
    vm.runInNewContext(runtimeSource, context, { filename: "sow-i18n.js" });
    return { window, document, storage, errors, requests, localeEvents };
}

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
