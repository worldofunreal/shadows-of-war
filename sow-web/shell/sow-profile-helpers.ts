import type { MenuLeader } from "./main_menu.state.ts";

export interface PublicLeaderCatalogEntry {
    id: string;
    artSlug: string;
    legacyIds?: string[];
    region?: string;
    name?: string;
    enName?: string;
    civilization?: string;
    slug?: string;
    art_slug?: string;
}

const LEADER_FALLBACK: MenuLeader = {
    id: "Caesar",
    name: "Caesar",
    civilization_key: "heroes.leader_caesar_civilization",
    perk_key: "site.leader_caesar_description",
    slug: "caesar",
    region: "europe",
};

const MAP_DISPLAY_NAMES: Record<string, string> = {
    africa: "Africa",
    asia: "Asia",
    bajacalifornia: "Baja California",
    eastanglia: "East Anglia",
    eastasia: "East Asia",
    europe: "Europe",
    indiansubcontinent: "Indian Subcontinent",
    mena: "MENA",
    middleeast: "Middle East",
    northamerica: "North America",
    oceania: "Oceania",
    pangaea: "Pangaea",
    southamerica: "South America",
    southeastasia: "Southeast Asia",
    world: "World",
};

let leaderCatalogProvider: (() => Array<MenuLeader | PublicLeaderCatalogEntry>) | null = null;
let mapInfoProvider: ((key: string) => { display_name?: string }) | null = null;

/** Registers a runtime leader catalog supplier (used by the in-game menu shell). */
export function setLeaderCatalogProvider(
    provider: (() => Array<MenuLeader | PublicLeaderCatalogEntry>) | null,
): void {
    leaderCatalogProvider = provider;
}

/** Registers an optional map metadata supplier for custom map display names. */
export function setMapInfoProvider(
    provider: ((key: string) => { display_name?: string }) | null,
): void {
    mapInfoProvider = provider;
}

/** Escapes HTML special characters for safe string-template rendering. */
export function esc(value: unknown): string {
    return String(value == null ? "" : value).replace(/[&<>"']/g, function (c) {
        return "&#" + c.charCodeAt(0) + ";";
    });
}

/** Resolves a static asset URL while respecting portal asset overrides and manifest hashes. */
export function assetUrl(path: string): string {
    const clean = String(path || "").replace(/^\/+/, "");
    const win = (typeof window !== "undefined" ? window : {}) as {
        SOW_ASSETS_URL?: string;
        SOW_ASSET_TS?: Record<string, string>;
        SOW_assetUrl?: (url: string) => string;
    };
    const base = String(win.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
    const raw = base + "/" + clean.split("/").map(encodeURIComponent).join("/");
    if (typeof win.SOW_assetUrl === "function") {
        return win.SOW_assetUrl(raw);
    }
    const manifestHash = win.SOW_ASSET_TS && win.SOW_ASSET_TS["/assets/" + clean];
    return manifestHash ? raw + "?v=" + encodeURIComponent(manifestHash) : raw;
}

/** Resolves a currency emblem image URL (crowns, gems, laurel). */
export function currencyAssetUrl(kind: string): string {
    return assetUrl("gameplay/currency/" + kind + ".webp");
}

/** Resolves an API endpoint against SOW_DATABASE_URL or same-origin /api. */
export function profileApiUrl(path: string): string {
    const win = (typeof window !== "undefined" ? window : {}) as { SOW_DATABASE_URL?: string };
    const base = String(win.SOW_DATABASE_URL || "/api").replace(/\/$/, "");
    return base + (path.startsWith("/") ? path : "/" + path);
}

/** XP curve threshold per level (canonical shared formula with main_menu.shell.ts). */
export function xpForLevel(level: number): number {
    if (level <= 1) return 100;
    return Math.round((100 + 11 * Math.pow(level - 1, 1.05)) / 5) * 5;
}

/** Computes level from cumulative XP. */
export function levelFromXp(xp: number): number {
    let level = 1;
    let remaining = Math.max(0, Math.floor(Number(xp) || 0));
    while (true) {
        const req = xpForLevel(level);
        if (remaining < req) return level;
        remaining -= req;
        level += 1;
        if (level > 1000) return level;
    }
}

/** Safe localization lookup that falls back cleanly when SOW_t is unavailable or missing a key. */
export function tSafe(key: string, fallback: string, values?: Record<string, unknown>): string {
    const fn = typeof SOW_t === "function" ? SOW_t : null;
    if (fn) {
        const translated = fn(key, values);
        if (translated && translated !== "[" + key + "]") {
            return translated;
        }
    }
    return fallback.replace(/\{(\w+)\}/g, function (match, name) {
        return values && name in values ? String(values[name]) : match;
    });
}

/** Normalizes raw catalog entries (either in-game MenuLeader or site SOW_PUBLIC_LEADERS) to MenuLeader. */
function toMenuLeader(entry: MenuLeader | PublicLeaderCatalogEntry): MenuLeader {
    const artSlug = (entry as MenuLeader).art_slug || (entry as PublicLeaderCatalogEntry).artSlug || (entry as MenuLeader).slug || "";
    const slug = (entry as MenuLeader).slug || artSlug || String(entry.id || "").toLowerCase();
    return {
        ...(entry as MenuLeader),
        id: String(entry.id || slug),
        slug,
        art_slug: artSlug || slug,
    };
}

/** Returns the active leader catalog from the game runtime or public site global. */
export function getLeaderCatalog(): MenuLeader[] {
    if (leaderCatalogProvider) {
        const items = leaderCatalogProvider();
        if (Array.isArray(items) && items.length) {
            return items.map(toMenuLeader);
        }
    }
    const win = (typeof window !== "undefined" ? window : {}) as {
        SOW_PUBLIC_LEADERS?: PublicLeaderCatalogEntry[];
    };
    if (Array.isArray(win.SOW_PUBLIC_LEADERS) && win.SOW_PUBLIC_LEADERS.length) {
        return win.SOW_PUBLIC_LEADERS.map(toMenuLeader);
    }
    return [];
}

function normalizeLeaderKey(value: unknown): string {
    return String(value || "")
        .normalize("NFD")
        .replace(/[\u0300-\u036f]/g, "")
        .replace(/\u0142/gi, "l")
        .replace(/[^a-z0-9]/gi, "")
        .toLowerCase();
}

/** Resolves a leader entry by id, legacy id, slug, art slug, or localized/English name. */
export function leaderById(id: unknown): MenuLeader {
    const leaders = getLeaderCatalog();
    const wanted = normalizeLeaderKey(id);
    if (wanted) {
        const found = leaders.find(function (leader) {
            const raw = leader as MenuLeader & { legacyIds?: string[]; artSlug?: string; enName?: string };
            if (normalizeLeaderKey(leader.id) === wanted) return true;
            if (normalizeLeaderKey(leader.slug) === wanted) return true;
            if (normalizeLeaderKey(leader.art_slug) === wanted) return true;
            if (normalizeLeaderKey(leader.name) === wanted) return true;
            if (normalizeLeaderKey(raw.enName) === wanted) return true;
            if (Array.isArray(raw.legacyIds) && raw.legacyIds.some((leg) => normalizeLeaderKey(leg) === wanted)) {
                return true;
            }
            return false;
        });
        if (found) return found;
        const rawLabel = String(id || "").trim();
        if (rawLabel) {
            return {
                id: rawLabel,
                slug: "",
                art_slug: "random",
                name: rawLabel,
                civilization_key: "",
                perk_key: "",
            };
        }
    }
    return leaders[0] || LEADER_FALLBACK;
}

/** Extracts the portrait/art slug for a leader object or string identifier. */
export function leaderArtSlug(leader: MenuLeader | string | null | undefined): string {
    if (typeof leader === "string") {
        return leaderById(leader).art_slug || "random";
    }
    return String((leader && (leader.art_slug || leader.slug)) || "random");
}

/** Returns the localized display name for a commander. */
export function leaderDisplayName(leader: MenuLeader | null | undefined): string {
    const slug = leader && (leader.slug || leader.art_slug) ? String(leader.slug || leader.art_slug) : "";
    if (slug) {
        const key = "heroes.leader_" + slug + "_name";
        const translated = tSafe(key, "");
        if (translated) return translated;
    }
    return (leader && leader.name) || slug || "";
}

/** Returns the localized civilization subtitle for a commander. */
export function leaderCivilization(leader: MenuLeader | null | undefined): string {
    const key = leader && leader.civilization_key;
    if (key) {
        const translated = tSafe(key, "");
        if (translated) return translated;
    }
    return (leader && leader.civilization) || "";
}

/** Resolves a commander avatar image URL, falling back to the default random emblem. */
export function leaderAvatarUrl(leaderId?: unknown): string {
    if (!leaderId) return assetUrl("gameplay/avatars/random.webp");
    const slug = leaderArtSlug(leaderById(leaderId)) || "caesar";
    return assetUrl("gameplay/avatars/" + slug + ".webp");
}

/** Resolves a hero cover art image URL (always desktop landscape framing for profile headers). */
export function profileHeroBackdropUrl(leaderId: unknown): string {
    const slug = leaderArtSlug(leaderById(leaderId || "Caesar")) || "caesar";
    const win = (typeof window !== "undefined" ? window : {}) as {
        SOW_leaderArtUrl?: (slug: string, variant?: string) => string;
    };
    if (typeof win.SOW_leaderArtUrl === "function") {
        return win.SOW_leaderArtUrl(slug, "desktop");
    }
    return assetUrl("shell/leaders/" + slug + "_desktop.webp");
}

/** Formats a server map key or object into a localized display label. */
export function formatMapName(
    value: string | { map_name?: string; key?: string; display_name?: string } | null | undefined,
): string {
    let key: string;
    let displayName = "";
    if (value && typeof value === "object") {
        key = value.map_name || value.key || "";
        displayName = value.display_name || "";
    } else {
        key = value || "";
    }
    key = String(key || "world")
        .trim()
        .toLowerCase();
    if (MAP_DISPLAY_NAMES[key]) return MAP_DISPLAY_NAMES[key];
    const extraInfo = mapInfoProvider ? mapInfoProvider(key) : null;
    const label = String(displayName || (extraInfo && extraInfo.display_name) || "").trim();
    if (label && label.toLowerCase() !== key) return label;
    return (
        key.replace(/[-_]+/g, " ").replace(/\b[a-z]/g, function (letter) {
            return letter.toUpperCase();
        }) || "World"
    );
}

/** Checks supported account ID formats (32 hex chars or 36-char UUID). */
export function isValidAccountId(value: unknown): boolean {
    const raw = String(value || "").trim();
    return /^[0-9a-f]{32}$/i.test(raw) || /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(raw);
}

/** Returns the active public site prefix ("/" or "/es/", "/zh-cn/", etc.). */
export function siteLocalePrefix(): string {
    if (typeof document !== "undefined" && document.documentElement) {
        const raw = document.documentElement.dataset.sitePrefix;
        if (raw && raw.startsWith("/")) {
            return raw.endsWith("/") ? raw : raw + "/";
        }
        if (typeof window !== "undefined" && window.location) {
            const m = window.location.pathname.match(/^\/(ar|de|es|fil|fr|id|it|ja|ko|pt-br|ru|tr|vi|zh-cn)\//i);
            if (m) return "/" + m[1].toLowerCase() + "/";
        }
    }
    return "/";
}

/** Builds a public URL for a player profile, using ?id= on local static HTTP preview servers. */
export function publicProfileUrl(accountId: string): string {
    const prefix = siteLocalePrefix();
    const id = encodeURIComponent(String(accountId || "").trim());
    if (!id) return prefix + "players/";
    const host =
        typeof window !== "undefined" && window.location ? String(window.location.hostname || "") : "";
    if (host === "localhost" || host === "127.0.0.1") {
        return prefix + "players/?id=" + id;
    }
    return prefix + "players/" + id;
}

/** Builds a public URL for the localized leaderboards directory. */
export function publicLeaderboardsUrl(): string {
    return siteLocalePrefix() + "leaderboards/";
}

/** Copies text to the clipboard with a safe DOM textarea fallback (portal-safe, no navigator share). */
export async function copyTextToClipboard(text: string): Promise<boolean> {
    const value = String(text || "");
    if (!value) return false;
    try {
        if (typeof navigator !== "undefined" && navigator.clipboard && typeof navigator.clipboard.writeText === "function") {
            await navigator.clipboard.writeText(value);
            return true;
        }
    } catch {}
    try {
        if (typeof document !== "undefined" && document.body) {
            const ta = document.createElement("textarea");
            ta.value = value;
            ta.setAttribute("readonly", "");
            ta.style.position = "fixed";
            ta.style.opacity = "0";
            ta.style.pointerEvents = "none";
            document.body.appendChild(ta);
            ta.select();
            const ok = typeof document.execCommand === "function" ? document.execCommand("copy") : false;
            document.body.removeChild(ta);
            return ok;
        }
    } catch {}
    return false;
}

/** Wires subtle pointer-tracked specular light (--mx / --my) onto showcase surfaces. */
export function bindPointerSpotlight(el: HTMLElement | null | undefined): void {
    if (!el || el.dataset.spotlightBound === "1") return;
    if (
        typeof window !== "undefined" &&
        typeof window.matchMedia === "function" &&
        window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
        return;
    }
    el.dataset.spotlightBound = "1";
    el.addEventListener("pointermove", function (event) {
        const rect = el.getBoundingClientRect();
        if (!rect.width || !rect.height) return;
        const x = Math.max(0, Math.min(100, ((event.clientX - rect.left) / rect.width) * 100));
        const y = Math.max(0, Math.min(100, ((event.clientY - rect.top) / rect.height) * 100));
        el.style.setProperty("--mx", x.toFixed(1) + "%");
        el.style.setProperty("--my", y.toFixed(1) + "%");
    });
}
