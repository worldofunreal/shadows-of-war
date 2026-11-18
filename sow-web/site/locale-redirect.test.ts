import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { buildSiteLocaleRedirectScript } from "./generate.ts";

const PUBLISHED_CODES = [
  "ar",
  "de",
  "en",
  "es",
  "fil",
  "fr",
  "id",
  "it",
  "ja",
  "ko",
  "pt-BR",
  "ru",
  "tr",
  "vi",
  "zh-CN",
];

function runRedirectScript(options: {
  route?: string;
  pathname?: string;
  search?: string;
  hash?: string;
  protocol?: string;
  storedLocale?: string | null;
  languages?: string[];
  language?: string;
  userAgent?: string;
}) {
  const redirects: string[] = [];
  const dataset: Record<string, string> = {};
  const storage = new Map<string, string>();
  if (options.storedLocale) storage.set("sow_ui_locale", options.storedLocale);

  const windowObj = {
    location: {
      protocol: options.protocol ?? "https:",
      pathname: options.pathname ?? "/",
      search: options.search ?? "",
      hash: options.hash ?? "",
      replace(url: string) {
        redirects.push(url);
      },
    },
    localStorage: {
      getItem: (key: string) => storage.get(key) ?? null,
    },
  };

  const script = buildSiteLocaleRedirectScript(options.route ?? "", PUBLISHED_CODES);
  vm.runInNewContext(script, {
    window: windowObj,
    document: { documentElement: { dataset } },
    navigator: {
      languages: options.languages,
      language: options.language,
      userAgent: options.userAgent ?? "Mozilla/5.0",
    },
  });

  return { redirects, dataset };
}

describe("buildSiteLocaleRedirectScript", () => {
  test("redirects first-time non-English visitors based on browser preference list", () => {
    const res = runRedirectScript({
      languages: ["ca-ES", "es-MX", "en-US"],
      search: "?ref=hero",
      hash: "#leaders",
    });
    expect(res.redirects).toEqual(["/es/?ref=hero#leaders"]);
    expect(res.dataset.sowLocalePending).toBe("true");
  });

  test("resolves language aliases and regional tags to lowercase route prefixes", () => {
    expect(runRedirectScript({ languages: ["pt-PT"] }).redirects).toEqual(["/pt-br/"]);
    expect(runRedirectScript({ languages: ["zh-Hans"] }).redirects).toEqual(["/zh-cn/"]);
    expect(runRedirectScript({ language: "tl-PH" }).redirects).toEqual(["/fil/"]);
    expect(runRedirectScript({ route: "leaders/Caesar/", language: "fr-CA" }).redirects).toEqual([
      "/fr/leaders/Caesar/",
    ]);
  });

  test("stays on English when first resolved browser language is English", () => {
    const res = runRedirectScript({ languages: ["nl-NL", "en-US", "es-ES"] });
    expect(res.redirects).toEqual([]);
    expect(res.dataset.sowLocalePending).toBeUndefined();
  });

  test("respects explicit saved localStorage preference in both directions", () => {
    const forcedEnglish = runRedirectScript({
      storedLocale: "en",
      languages: ["es-MX", "es"],
    });
    expect(forcedEnglish.redirects).toEqual([]);
    expect(forcedEnglish.dataset.sowLocalePending).toBeUndefined();

    const forcedFrench = runRedirectScript({
      route: "maps/",
      storedLocale: "fr",
      languages: ["en-US"],
    });
    expect(forcedFrench.redirects).toEqual(["/fr/maps/"]);
  });

  test("skips search/preview crawlers and non-http protocols", () => {
    expect(
      runRedirectScript({
        languages: ["es"],
        userAgent: "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
      }).redirects,
    ).toEqual([]);
    expect(
      runRedirectScript({
        protocol: "file:",
        languages: ["es"],
      }).redirects,
    ).toEqual([]);
  });
});

describe("sow-i18n game shell vs landing locale persistence", () => {
  test("game boot follows browser language without writing a sticky override until user switches explicitly", async () => {
    const tsSource = fs.readFileSync(path.resolve(import.meta.dir, "../shell/sow-i18n.ts"), "utf8");
    const transpiler = new Bun.Transpiler({ loader: "ts" });
    const jsSource = transpiler.transformSync(tsSource);

    const storage = new Map<string, string>();
    const documentElement = { lang: "en", dir: "ltr", dataset: {} as Record<string, string> };
    const windowObj: Record<string, any> = {
      SOW_LOCALE_CODES: PUBLISHED_CODES,
      SOW_LOCALE_CATALOG_VERSION: 19,
      localStorage: {
        getItem: (k: string) => storage.get(k) ?? null,
        setItem: (k: string, v: string) => storage.set(k, String(v)),
      },
      CustomEvent: class {
        type: string;
        detail: unknown;
        constructor(type: string, init?: { detail?: unknown }) {
          this.type = type;
          this.detail = init?.detail;
        }
      },
      dispatchEvent: () => true,
    };

    const fetchImpl = async (url: string) => {
      const code = String(url).split("?")[0].split("/").pop() || "en";
      return {
        ok: true,
        status: 200,
        json: async () => ({
          schema: 1,
          version: 19,
          locale: code,
          languages: PUBLISHED_CODES,
          strings: { menu: { language_en: "English", language_es: "Español" } },
        }),
      };
    };

    vm.runInNewContext(jsSource, {
      window: windowObj,
      CustomEvent: windowObj.CustomEvent,
      document: { documentElement, currentScript: null },
      navigator: { languages: ["es-MX", "es", "en-US"], language: "es-MX" },
      fetch: fetchImpl,
      AbortController,
      setTimeout,
      clearTimeout,
      console,
    });

    await windowObj.SOW_I18N_READY;
    expect(windowObj.SOW_getLocale()).toBe("es");
    expect(documentElement.lang).toBe("es");
    expect(storage.has("sow_ui_locale")).toBe(false);

    await windowObj.SOW_setLocale("en");
    expect(windowObj.SOW_getLocale()).toBe("en");
    expect(storage.get("sow_ui_locale")).toBe("en");
  });
});
