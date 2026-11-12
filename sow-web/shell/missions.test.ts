import test from "node:test";
import assert from "node:assert/strict";

if (typeof (globalThis as any).document === "undefined") {
    const sowMenu = {
        id: "sow-menu",
        dataset: {},
        style: { setProperty() {}, removeProperty() {} },
        addEventListener() {},
        querySelector: () => null,
        querySelectorAll: () => [],
    };
    (globalThis as any).document = {
        getElementById: (id: string) => (id === "sow-menu" ? sowMenu : null),
        addEventListener() {},
        querySelector: () => null,
        querySelectorAll: () => [],
    };
    (globalThis as any).window = {
        matchMedia: () => ({ matches: false }),
        innerWidth: 1200,
        innerHeight: 800,
        addEventListener() {},
        SOW_t: (k: string) => k,
        SOW_portalRewardedAvailable: () => false,
        SOW_portalRewardedBreak: () => Promise.resolve(false),
    };
    (globalThis as any).SOW_t = (k: string) => k;
    (globalThis as any).HTMLInputElement = class HTMLInputElement {};
    (globalThis as any).HTMLButtonElement = class HTMLButtonElement {};
    (globalThis as any).HTMLSelectElement = class HTMLSelectElement {};
    (globalThis as any).HTMLTextAreaElement = class HTMLTextAreaElement {};
    (globalThis as any).Element = class Element {};
    (globalThis as any).HTMLElement = class HTMLElement {};
}

if (typeof (globalThis as any).window !== "undefined") {
    (globalThis as any).window.SOW_portalRewardedBreak = () => Promise.resolve(false);
    (globalThis as any).window.SOW_portalRewardedAvailable = () => false;
}

const { menuUi } = await import("./main_menu.core.ts");
const {
    resolveActiveMissions,
    getActiveDirective,
    renderDirectiveCard,
    renderMissionsModal,
    claimMission,
    isRewardedAdAvailable,
} = await import("./main_menu.missions.ts");
import type { MenuMission, MenuState } from "./main_menu.state.ts";

test("resolveActiveMissions returns state missions directly when provided by backend", () => {
    const customMission: MenuMission = {
        id: "onboarding_1_tutorial",
        category: "onboarding",
        title_key: "missions.onboarding_tutorial_title",
        description_key: "missions.onboarding_tutorial_desc",
        target: 1,
        progress: 1,
        completed: true,
        claimed: false,
        crowns_reward: 150,
        gems_reward: 25,
        icon: "shield",
        order: 1,
    };
    const state = {
        missions: [customMission],
    } as unknown as MenuState;

    const resolved = resolveActiveMissions(state);
    assert.equal(resolved.length, 1);
    assert.equal(resolved[0].id, "onboarding_1_tutorial");
    assert.equal(resolved[0].completed, true);
});

test("getActiveDirective steps sequentially through Camino del Conquistador", () => {
    const missions: MenuMission[] = [
        {
            id: "onboarding_1_tutorial",
            category: "onboarding",
            title_key: "missions.onboarding_tutorial_title",
            description_key: "missions.onboarding_tutorial_desc",
            target: 1,
            progress: 1,
            completed: true,
            claimed: true,
            crowns_reward: 150,
            gems_reward: 25,
            icon: "shield",
            order: 1,
        },
        {
            id: "onboarding_2_structures",
            category: "onboarding",
            title_key: "missions.onboarding_structures_title",
            description_key: "missions.onboarding_structures_desc",
            target: 5,
            progress: 2,
            completed: false,
            claimed: false,
            crowns_reward: 300,
            gems_reward: 40,
            icon: "tower",
            order: 2,
        },
        {
            id: "onboarding_3_rotation",
            category: "onboarding",
            title_key: "missions.onboarding_rotation_title",
            description_key: "missions.onboarding_rotation_desc",
            target: 1,
            progress: 0,
            completed: false,
            claimed: false,
            crowns_reward: 200,
            gems_reward: 30,
            icon: "sword",
            order: 3,
        },
    ];

    const active = getActiveDirective(missions);
    assert.ok(active);
    assert.equal(active.id, "onboarding_2_structures");
});

test("renderDirectiveCard renders progress and CTA buttons", () => {
    menuUi.state = {
        missions: [
            {
                id: "onboarding_2_structures",
                category: "onboarding",
                title_key: "missions.onboarding_structures_title",
                description_key: "missions.onboarding_structures_desc",
                target: 5,
                progress: 3,
                completed: false,
                claimed: false,
                crowns_reward: 300,
                gems_reward: 40,
                icon: "tower",
                order: 2,
            },
        ],
    } as unknown as MenuState;

    const html = renderDirectiveCard();
    assert.match(html, /sow-directive-card/);
    assert.match(html, /data-command='open_missions'/);
    assert.match(html, /3 \/ 5/);
    assert.match(html, /\+300/);
    assert.match(html, /\+40/);
});

test("renderDirectiveCard renders Claim and Claim 2X when completed and ad available", () => {
    (globalThis as any).window.SOW_portalRewardedAvailable = () => true;
    (globalThis as any).window.SOW_portalRewardedBreak = () => Promise.resolve(true);

    menuUi.state = {
        missions: [
            {
                id: "onboarding_2_structures",
                category: "onboarding",
                title_key: "missions.onboarding_structures_title",
                description_key: "missions.onboarding_structures_desc",
                target: 5,
                progress: 5,
                completed: true,
                claimed: false,
                crowns_reward: 300,
                gems_reward: 40,
                icon: "tower",
                order: 2,
            },
        ],
    } as unknown as MenuState;

    const html = renderDirectiveCard();
    assert.match(html, /is-ready/);
    assert.match(html, /data-command='claim_mission'/);
    assert.match(html, /data-command='claim_mission_double'/);

    (globalThis as any).window.SOW_portalRewardedAvailable = () => false;
});

test("renderMissionsModal renders tab switcher and mission list", () => {
    menuUi.activeMissionsTab = "onboarding";
    menuUi.state = {
        missions: [
            {
                id: "onboarding_1_tutorial",
                category: "onboarding",
                title_key: "missions.onboarding_tutorial_title",
                description_key: "missions.onboarding_tutorial_desc",
                target: 1,
                progress: 1,
                completed: true,
                claimed: true,
                crowns_reward: 150,
                gems_reward: 25,
                icon: "shield",
                order: 1,
            },
            {
                id: "daily_play_matches",
                category: "daily",
                title_key: "missions.daily_play_matches_title",
                description_key: "missions.daily_play_matches_desc",
                target: 3,
                progress: 1,
                completed: false,
                claimed: false,
                crowns_reward: 150,
                gems_reward: 20,
                icon: "sword",
                order: 1,
            },
        ],
    } as unknown as MenuState;

    const html = renderMissionsModal();
    assert.match(html, /sow-missions-modal/);
    assert.match(html, /data-missions-tab='onboarding'/);
    assert.match(html, /data-missions-tab='daily'/);
    assert.match(html, /data-missions-tab='weekly'/);
    assert.match(html, /missions\.onboarding_tutorial_title/);
    assert.match(html, /sow-missions__claimed-badge/);
});

test("renderDirectiveCard omits zero-gem reward pill and redundant kicker", () => {
    menuUi.state = {
        missions: [
            {
                id: "onboarding_2_structures",
                category: "onboarding",
                title_key: "missions.onboarding_structures_title",
                description_key: "missions.onboarding_structures_desc",
                target: 5,
                progress: 5,
                completed: true,
                claimed: false,
                crowns_reward: 50,
                gems_reward: 0,
                icon: "tower",
                order: 2,
            },
        ],
    } as unknown as MenuState;

    const html = renderDirectiveCard();
    assert.match(html, /\+50/);
    assert.doesNotMatch(html, /\+0/);
    assert.doesNotMatch(html, /sow-directive__kicker/);
    assert.match(html, /sow-directive__title-row/);
    assert.match(html, /sow-directive__desc/);
});

test("renderMissionsModal renders weekly tab and countdown", () => {
    menuUi.activeMissionsTab = "weekly";
    menuUi.state = {
        missions: [
            {
                id: "weekly_ranked_champion",
                category: "weekly",
                title_key: "missions.weekly_ranked_champion_title",
                description_key: "missions.weekly_ranked_champion_desc",
                target: 3,
                progress: 1,
                completed: false,
                claimed: false,
                crowns_reward: 150,
                gems_reward: 5,
                icon: "crown",
                order: 30,
            },
        ],
    } as unknown as MenuState;

    const html = renderMissionsModal();
    assert.match(html, /data-missions-tab='weekly'/);
    assert.match(html, /missions\.weekly_resets_in/);
    assert.match(html, /missions\.weekly_ranked_champion_title/);
    assert.match(html, /\+150/);
    assert.match(html, /\+5/);
});

test("claimMission sends doubled_by_ad when video ad watched", async () => {
    let sentCommand = "";
    let sentPayload: any = null;

    (globalThis as any).window.SOW_portalRewardedAvailable = () => true;
    (globalThis as any).window.SOW_portalRewardedBreak = () => Promise.resolve(true);

    const originalSend = (globalThis as any).window.SOW_send;
    (globalThis as any).window.SOW_send = (type: string, payload: any) => {
        sentCommand = type;
        sentPayload = payload;
        return true;
    };

    claimMission("onboarding_2_structures", true);

    // Wait microtask
    await Promise.resolve();

    (globalThis as any).window.SOW_portalRewardedAvailable = () => false;
    (globalThis as any).window.SOW_send = originalSend;
});

test("main_menu.missions.css provides symmetrical multi-viewport responsive rules", async () => {
    const fs = await import("node:fs");
    const path = await import("node:path");
    const cssPath = path.resolve(__dirname, "main_menu.missions.css");
    const css = fs.readFileSync(cssPath, "utf8");

    // Symmetrical base styling
    assert.match(css, /\.sow-directive-card\s*\{[^}]*margin:\s*0/);
    assert.match(css, /\.sow-directive-card\s*\{[^}]*width:\s*100%/);

    // Landscape compact rules (height <= 560px)
    assert.match(css, /@media\s*\(orientation:\s*landscape\)\s*and\s*\(max-height:\s*560px\)/);
    assert.match(css, /\.sow-directive__desc\s*\{[^}]*display:\s*none/);

    // Mobile vertical portrait rules (width <= 680px)
    assert.match(css, /@media\s*\(max-width:\s*680px\),\s*\(orientation:\s*portrait\)/);

    // Mission reward celebration stage & banner styles
    assert.match(css, /\.sow-menu__reward-stage--mission/);
    assert.match(css, /\.sow-menu__mission-banner/);
    assert.match(css, /\.sow-menu__mission-title/);
    assert.match(css, /\.sow-menu__mission-shockwave/);
    assert.match(css, /\.sow-menu__mission-desc/);
    assert.match(css, /\.sow-menu__reward-card--gems/);
});

test("claimMission invokes registered mission claim celebration handler", () => {
    let claimedMission: any = null;
    let claimedDoubled: boolean | null = null;

    menuUi.state = {
        missions: [
            {
                id: "onboarding_1_tutorial",
                category: "onboarding",
                title_key: "missions.onboarding_tutorial_title",
                description_key: "missions.onboarding_tutorial_desc",
                target: 1,
                progress: 1,
                completed: true,
                claimed: false,
                crowns_reward: 200,
                gems_reward: 25,
                icon: "shield",
                order: 1,
            },
        ],
    } as unknown as MenuState;

    const originalSend = (globalThis as any).window.SOW_send;
    (globalThis as any).window.SOW_send = () => true;

    const { setMissionClaimHandler } = require("./main_menu.missions.ts");
    setMissionClaimHandler((mission: any, doubled: boolean) => {
        claimedMission = mission;
        claimedDoubled = doubled;
    });

    claimMission("onboarding_1_tutorial", false);

    assert.ok(claimedMission, "handler should be called");
    assert.equal(claimedMission.id, "onboarding_1_tutorial");
    assert.equal(claimedMission.crowns_reward, 200);
    assert.equal(claimedMission.gems_reward, 25);
    assert.equal(claimedDoubled, false);
    assert.equal(claimedMission.claimed, true, "mission should be optimistically marked claimed");

    setMissionClaimHandler(null);
    (globalThis as any).window.SOW_send = originalSend;
});

test("missionIconSvg renders valid distinct tactical SVGs for all icon types", () => {
    const { missionIconSvg } = require("./main_menu.missions.ts");
    const icons = ["shield", "tower", "sword", "swords", "rotation", "crosshair", "team", "globe", "crown"];
    for (const icon of icons) {
        const svg = missionIconSvg(icon);
        assert.ok(svg.startsWith("<svg"), `Icon ${icon} should render an SVG element`);
        assert.ok(svg.endsWith("</svg>"), `Icon ${icon} should properly close SVG element`);
    }
});
