import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import {
    isValidAccountId,
    leaderArtSlug,
    leaderAvatarUrl,
    leaderById,
    leaderDisplayName,
    publicProfileUrl,
    setLeaderCatalogProvider,
} from "./sow-profile-helpers.ts";
import {
    DEFAULT_RANKED_LADDER,
    RANKED_LEAGUES,
    rankCulture,
    rankEmblem,
    rankName,
    rankProgress,
    rankTier,
    renderRankedOverview,
} from "./sow-ranked-presentation.ts";
import type { ProfileMatch, ProfileSnapshot } from "./sow-profile-view.ts";

let buildLeaderboardRequestUrl: typeof import("./sow-leaderboard-view.ts")["buildLeaderboardRequestUrl"];
let createLeaderboardController: typeof import("./sow-leaderboard-view.ts")["createLeaderboardController"];
let createProfileController: typeof import("./sow-profile-view.ts")["createProfileController"];
let filterMatchesByQueue: typeof import("./sow-profile-view.ts")["filterMatchesByQueue"];
let renderPlayerLookup: typeof import("./sow-profile-view.ts")["renderPlayerLookup"];

const initialWindow = (globalThis as { window?: unknown }).window;
const initialDocument = (globalThis as { document?: unknown }).document;

beforeAll(async () => {
    installBrowserGlobals();
    const lbMod = await import("./sow-leaderboard-view.ts");
    const pvMod = await import("./sow-profile-view.ts");
    buildLeaderboardRequestUrl = lbMod.buildLeaderboardRequestUrl;
    createLeaderboardController = lbMod.createLeaderboardController;
    createProfileController = pvMod.createProfileController;
    filterMatchesByQueue = pvMod.filterMatchesByQueue;
    renderPlayerLookup = pvMod.renderPlayerLookup;
});

afterAll(() => {
    setLeaderCatalogProvider(null);
    if (initialWindow === undefined) {
        delete (globalThis as { window?: unknown }).window;
    } else {
        (globalThis as { window?: unknown }).window = initialWindow;
    }
    if (initialDocument === undefined) {
        delete (globalThis as { document?: unknown }).document;
    } else {
        (globalThis as { document?: unknown }).document = initialDocument;
    }
});

interface MockElement {
    innerHTML: string;
    dataset: Record<string, string>;
    style: { setProperty: (name: string, val: string) => void };
    classList: {
        toggle: (cls: string, force?: boolean) => boolean;
        add: (cls: string) => void;
        remove: (cls: string) => void;
        contains: (cls: string) => boolean;
    };
    querySelector: (selector: string) => MockElement | null;
    querySelectorAll: (selector: string) => MockElement[];
    addEventListener: (type: string, fn: (event: unknown) => void) => void;
    removeEventListener: (type: string, fn: (event: unknown) => void) => void;
    contains: (node: unknown) => boolean;
}

function createMockHost(): MockElement {
    const classes = new Set<string>();
    const listeners = new Map<string, Set<(event: unknown) => void>>();
    const host: MockElement = {
        innerHTML: "",
        dataset: {},
        style: { setProperty() {} },
        classList: {
            toggle(cls, force) {
                if (force === undefined) {
                    if (classes.has(cls)) {
                        classes.delete(cls);
                        return false;
                    }
                    classes.add(cls);
                    return true;
                }
                if (force) classes.add(cls);
                else classes.delete(cls);
                return force;
            },
            add(cls) {
                classes.add(cls);
            },
            remove(cls) {
                classes.delete(cls);
            },
            contains(cls) {
                return classes.has(cls);
            },
        },
        querySelector() {
            return null;
        },
        querySelectorAll() {
            return [];
        },
        addEventListener(type, fn) {
            if (!listeners.has(type)) listeners.set(type, new Set());
            listeners.get(type)!.add(fn);
        },
        removeEventListener(type, fn) {
            listeners.get(type)?.delete(fn);
        },
        contains() {
            return false;
        },
    };
    return host;
}

function installBrowserGlobals(options?: {
    hostname?: string;
    pathname?: string;
    sitePrefix?: string;
}) {
    const host = options?.hostname ?? "shadowsofwar.io";
    const pathname = options?.pathname ?? "/leaderboards/";
    const sitePrefix = options?.sitePrefix ?? "/";
    (globalThis as { window?: unknown }).window = {
        location: {
            hostname: host,
            pathname,
            search: "",
            origin: "https://" + host,
        },
        matchMedia: () => ({ matches: true }),
    };
    (globalThis as { document?: unknown }).document = {
        documentElement: {
            dataset: { sitePrefix },
        },
        addEventListener() {},
        removeEventListener() {},
    };
}

describe("shared ranked leagues & dual-mode presentation", () => {
    test("maps all 10 leagues across the 8 cultural tiers identically for 1v1 and FFA", () => {
        expect(RANKED_LEAGUES).toHaveLength(10);
        expect(DEFAULT_RANKED_LADDER.steps.map((s) => s.tier)).toEqual([
            "Nomad",
            "Chieftain",
            "Commander",
            "Governor",
            "King",
            "Emperor",
            "Sovereign",
            "Overlord",
            "Immortal",
            "Deity",
        ]);
        expect(rankTier("Deity")).toBe("deity");
        expect(rankCulture("Deity")).toBe("grandmaster");
        expect(rankCulture("Immortal")).toBe("master");
        expect(rankCulture("Nomad")).toBe("unranked");
        expect(rankName("King", "II")).toBe("King II");
        expect(rankEmblem("Overlord", "I")).toContain("data-badge='overlord'");

        const progress = rankProgress(5400);
        expect(progress?.step.tier).toBe("Emperor");
        expect(progress?.next?.tier).toBe("Sovereign");
        expect(progress?.remaining).toBe(600);
    });

    test("renders dual-mode Ranked 1v1 and Ranked FFA overview sharing the 10 leagues", () => {
        const html = renderRankedOverview({
            activeQueue: "RankedFFA",
            showActions: false,
            rating1v1: {
                season_id: 1,
                season_name: "Season 1",
                queue: "Ranked1v1",
                mode: "FFA",
                games_played: 14,
                wins: 10,
                placements_complete: true,
                score: 4250,
                tier: "King",
                division: "II",
                peak_score: 4400,
            },
            ratingFfa: {
                season_id: 1,
                season_name: "Season 1",
                queue: "RankedFFA",
                mode: "FFA",
                games_played: 9,
                wins: 6,
                placements_complete: true,
                score: 7300,
                tier: "Overlord",
                division: "I",
                peak_score: 7450,
            },
        });
        expect(html).toContain("Ranked 1v1");
        expect(html).toContain("Ranked FFA");
        expect(html).toContain("Overlord I");
        expect(html).toContain("7450");
        expect(html).toContain("4250");
        expect(html).toContain("Separate ladders, same ten leagues");
    });
});

describe("shared public profile helpers & URL routing", () => {
    test("validates hex32 and dashed UUID account ids", () => {
        expect(isValidAccountId("0123456789abcdef0123456789abcdef")).toBe(true);
        expect(isValidAccountId("01234567-89ab-4def-8123-456789abcdef")).toBe(true);
        expect(isValidAccountId("bot-01")).toBe(false);
        expect(isValidAccountId("../etc/passwd")).toBe(false);
        expect(isValidAccountId("")).toBe(false);
    });

    test("resolves accented and English leader names to canonical art slugs without 404 or missing-key slugs", () => {
        setLeaderCatalogProvider(() => [
            { id: "bolivar", artSlug: "bolivar", name: "Simón Bolívar", enName: "Simón Bolívar" },
            { id: "kaahumanu_hawaii", artSlug: "kaahumanu_hawaii", name: "Kaʻahumanu", enName: "Kaʻahumanu" },
            { id: "boleslaw_ii", artSlug: "boleslaw_ii", name: "Bolesław II", enName: "Bolesław II" },
            { id: "cyrus_the_great", artSlug: "cyrus_the_great", name: "Ciro el Grande", enName: "Cyrus" },
            { id: "salote_tupou_iii", artSlug: "salote_tupou_iii", name: "Sālote III", enName: "Sālote III" },
        ]);
        expect(leaderById("Simón Bolívar").id).toBe("bolivar");
        expect(leaderArtSlug("Simón Bolívar")).toBe("bolivar");
        expect(leaderAvatarUrl("Kaʻahumanu")).toBe("/assets/gameplay/avatars/kaahumanu_hawaii.webp");
        expect(leaderById("Bolesław II").art_slug).toBe("boleslaw_ii");
        expect(leaderById("Cyrus").id).toBe("cyrus_the_great");
        expect(leaderDisplayName(leaderById("Sālote III"))).toBe("Sālote III");
        expect(leaderAvatarUrl("UnknownLeaderXYZ")).toBe("/assets/gameplay/avatars/random.webp");
    });

    test("formats publicProfileUrl cleanly for prod vs localhost and localized prefixes", () => {
        installBrowserGlobals({ hostname: "shadowsofwar.io", sitePrefix: "/" });
        expect(publicProfileUrl("0123456789abcdef0123456789abcdef")).toBe(
            "/players/0123456789abcdef0123456789abcdef",
        );

        installBrowserGlobals({ hostname: "shadowsofwar.io", sitePrefix: "/es/" });
        expect(publicProfileUrl("0123456789abcdef0123456789abcdef")).toBe(
            "/es/players/0123456789abcdef0123456789abcdef",
        );

        installBrowserGlobals({ hostname: "127.0.0.1", sitePrefix: "/fr/" });
        expect(publicProfileUrl("0123456789abcdef0123456789abcdef")).toBe(
            "/fr/players/?id=0123456789abcdef0123456789abcdef",
        );
    });

    test("filters profile match history by queue", () => {
        const matches: ProfileMatch[] = [
            {
                match_id: "m1",
                completed_at: 1,
                queue: "Ranked1v1",
                mode: "1v1",
                map_name: "europe",
                duration_seconds: 180,
                won: true,
                leader: "caesar",
                kills: 1,
                deaths: 0,
                assists: 0,
                verified: true,
                provisional: false,
            },
            {
                match_id: "m2",
                completed_at: 2,
                queue: "RankedFFA",
                mode: "FFA",
                map_name: "world",
                duration_seconds: 400,
                won: false,
                leader: "cleopatra",
                kills: 2,
                deaths: 1,
                assists: 1,
                verified: true,
                provisional: false,
            },
        ];
        expect(filterMatchesByQueue(matches, "all")).toHaveLength(2);
        expect(filterMatchesByQueue(matches, "Ranked1v1").map((m) => m.match_id)).toEqual(["m1"]);
        expect(filterMatchesByQueue(matches, "RankedFFA").map((m) => m.match_id)).toEqual(["m2"]);
        expect(filterMatchesByQueue(matches, "Matchmaking")).toHaveLength(0);
    });
});

describe("shared leaderboard controller", () => {
    test("builds query strings for all 3 boards and all filters", () => {
        installBrowserGlobals();
        const r1 = buildLeaderboardRequestUrl(
            {
                board: "Ranked1v1",
                tier: "Gold",
                leader: "caesar",
                q: "Diogenes",
                minGames: 10,
                sort: "peak",
            },
            25,
            25,
        );
        expect(r1).toContain("/api/seasons/current/leaderboard?");
        expect(r1).toContain("queue=Ranked1v1");
        expect(r1).toContain("tier=Gold");
        expect(r1).toContain("leader=caesar");
        expect(r1).toContain("q=Diogenes");
        expect(r1).toContain("min_games=10");
        expect(r1).toContain("sort=peak");
        expect(r1).toContain("cursor=25");

        const rFfa = buildLeaderboardRequestUrl(
            { board: "RankedFFA", tier: "all", leader: "all", q: "", minGames: 0, sort: "score" },
            0,
            25,
        );
        expect(rFfa).toContain("queue=RankedFFA");

        const vic = buildLeaderboardRequestUrl(
            { board: "victories", tier: "all", leader: "all", q: "alex", minGames: 5, sort: "win_rate" },
            0,
            25,
        );
        expect(vic).toContain("/api/seasons/current/leaderboard?kind=victories");
        expect(vic).toContain("q=alex");
        expect(vic).toContain("min_games=5");
        expect(vic).toContain("sort=win_rate");
    });

    test("renders Top-3 podium, <a href> rows in site mode, <button> in game mode, and escapes XSS", async () => {
        installBrowserGlobals({ hostname: "shadowsofwar.io", sitePrefix: "/" });
        setLeaderCatalogProvider(() => [{ id: "caesar", name: "Caesar", slug: "caesar", artSlug: "caesar" }]);

        const payload = {
            items: [
                {
                    rank: 1,
                    account_id: "0123456789abcdef0123456789abcdef",
                    handle: "Diogenes<img src=x onerror=alert(1)>",
                    score: 9400,
                    tier: "Deity",
                    division: "I",
                    games_played: 40,
                    wins: 34,
                    peak_score: 9500,
                    preferred_leader: "caesar",
                },
                {
                    rank: 2,
                    account_id: "fedcba9876543210fedcba9876543210",
                    handle: "Pericles",
                    score: 8200,
                    tier: "Immortal",
                    division: "II",
                    games_played: 25,
                    wins: 18,
                    peak_score: 8250,
                    preferred_leader: "caesar",
                },
                {
                    rank: 3,
                    account_id: "11112222333344445555666677778888",
                    handle: "Solon",
                    score: 7600,
                    tier: "Overlord",
                    division: "III",
                    games_played: 20,
                    wins: 14,
                    peak_score: 7700,
                    preferred_leader: "caesar",
                },
                {
                    rank: 4,
                    account_id: "aaaabbbbccccddddeeeeffff00001111",
                    handle: "Antisthenes",
                    score: 6800,
                    tier: "Sovereign",
                    division: "I",
                    games_played: 18,
                    wins: 12,
                    peak_score: 6900,
                    preferred_leader: "caesar",
                },
            ],
            next_cursor: null,
            total_count: 4,
            tier_counts: { Deity: 1, Immortal: 1, Overlord: 1, Sovereign: 1 },
        };

        const mockFetch = (async () =>
            new Response(JSON.stringify(payload), {
                status: 200,
                headers: { "Content-Type": "application/json" },
            })) as unknown as typeof fetch;

        const siteHost = createMockHost();
        const siteCtrl = createLeaderboardController(siteHost as unknown as HTMLElement, {
            mode: "site",
            syncUrl: false,
            fetchImpl: mockFetch,
        });
        await new Promise((r) => setTimeout(r, 0));

        expect(siteHost.innerHTML).toContain("sow-lb__podium-card");
        expect(siteHost.innerHTML).toContain("href='/players/0123456789abcdef0123456789abcdef'");
        expect(siteHost.innerHTML).not.toContain("<img src=x onerror=alert(1)>");
        expect(siteHost.innerHTML).toContain("Diogenes&#60;img src=x onerror=alert(1)&#62;");
        siteCtrl.destroy();

        const gameHost = createMockHost();
        const gameCtrl = createLeaderboardController(gameHost as unknown as HTMLElement, {
            mode: "game",
            syncUrl: false,
            fetchImpl: mockFetch,
        });
        await new Promise((r) => setTimeout(r, 0));

        expect(gameHost.innerHTML).toContain("<button type='button' class='sow-lb__row");
        expect(gameHost.innerHTML).toContain("data-account-id='0123456789abcdef0123456789abcdef'");
        gameCtrl.destroy();
    });
});

describe("WASM-free guest profile & lookup views", () => {
    test("renders guest public profile with dual-mode ranked card, achievements, and share action without menuUi", async () => {
        installBrowserGlobals({ hostname: "shadowsofwar.io", sitePrefix: "/es/" });
        const snapshot: ProfileSnapshot = {
            account_id: "0123456789abcdef0123456789abcdef",
            handle: "sinopecynic",
            display_name: "SinopeCynic",
            verified: true,
            created_at: 1700000000,
            xp: 4200,
            level: 18,
            laurels: 150,
            matches_played: 12,
            wins: 8,
            win_rate: 0.67,
            players_defeated: 20,
            empires_defeated: 12,
            tribes_defeated: 8,
            preferred_leader: "caesar",
            kills: 15,
            deaths: 4,
            assists: 9,
            ratings: [
                {
                    season_id: 1,
                    season_name: "Season 1",
                    queue: "Ranked1v1",
                    mode: "FFA",
                    games_played: 8,
                    wins: 6,
                    placements_complete: true,
                    score: 5200,
                    tier: "Emperor",
                    division: "II",
                    peak_score: 5400,
                },
                {
                    season_id: 1,
                    season_name: "Season 1",
                    queue: "RankedFFA",
                    mode: "FFA",
                    games_played: 4,
                    wins: 2,
                    placements_complete: false,
                    score: 1400,
                    tier: "Chieftain",
                    division: null,
                    peak_score: 1400,
                },
            ],
            leaders: [
                { leader: "caesar", matches_played: 12, wins: 8, win_rate: 67, xp: 1500 },
            ],
            recent_matches: [],
            achievements: [
                { id: "first_command", progress: 1, target: 1, unlocked: true, points: 25 },
            ],
        };

        const mockFetch = (async () =>
            new Response(JSON.stringify(snapshot), {
                status: 200,
                headers: { "Content-Type": "application/json" },
            })) as unknown as typeof fetch;

        const host = createMockHost();
        let loadedHandle = "";
        const ctrl = createProfileController(host as unknown as HTMLElement, {
            accountId: snapshot.account_id,
            initialTab: "achievements",
            fetchImpl: mockFetch,
            onProfileLoaded(p) {
                loadedHandle = p.handle;
            },
        });
        await new Promise((r) => setTimeout(r, 0));

        expect(loadedHandle).toBe("sinopecynic");
        expect(host.innerHTML).toContain("<h1 id='sow-profile-title' data-profile-display-name>SinopeCynic</h1>");
        expect(host.innerHTML).toContain("data-command='share_profile'");
        expect(host.innerHTML).toContain("href='/es/leaderboards/'");
        expect(host.innerHTML).toContain("sow-profile__ach-layout");
        ctrl.destroy();

        const lookupHost = createMockHost();
        const lookup = renderPlayerLookup(lookupHost as unknown as HTMLElement, {
            fetchImpl: (async () =>
                new Response(JSON.stringify({ items: [] }), {
                    status: 200,
                    headers: { "Content-Type": "application/json" },
                })) as unknown as typeof fetch,
        });
        expect(lookupHost.innerHTML).toContain("data-lookup-input");
        expect(lookupHost.innerHTML).toContain("Ranked 1v1");
        expect(lookupHost.innerHTML).toContain("Ranked FFA");
        lookup.destroy();
    });
});
