import test from "node:test";
import { avatarBackgroundCss, avatarImageAttribute, setAvatarBackground, setAvatarImage } from "./avatar-presentation.ts";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const shell = import.meta.dirname;

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
        SOW_leaderArtUrl: (slug: string, variant?: string) =>
            `/assets/leaders/${slug}_${variant || "desktop"}.webp`,
    };
    (globalThis as any).SOW_t = (k: string) => k;
    (globalThis as any).HTMLInputElement = class HTMLInputElement {};
    (globalThis as any).HTMLButtonElement = class HTMLButtonElement {};
    (globalThis as any).HTMLSelectElement = class HTMLSelectElement {};
    (globalThis as any).HTMLTextAreaElement = class HTMLTextAreaElement {};
    (globalThis as any).Element = class Element {};
    (globalThis as any).HTMLElement = class HTMLElement {};
}

function loadCampaignHarness() {
    const lobbiesSource = fs.readFileSync(path.join(shell, "main_menu.lobbies.ts"), "utf8");
    const coreSource = fs.readFileSync(path.join(shell, "main_menu.core.ts"), "utf8");

    const strings = {
        "lobbies.single_player": "Single Player",
        "lobbies.campaign_chronicles": "Campaign Chronicles",
        "lobbies.the_saga": "The Saga",
        "lobbies.episodes": "Episodes",
        "lobbies.continue_campaign": "Continue Campaign",
        "lobbies.saga_complete": "Saga Complete",
        "lobbies.completed": "Completed",
        "lobbies.available": "Available",
        "lobbies.next": "Next",
        "lobbies.locked": "Locked",
        "lobbies.play": "Play",
        "lobbies.replay": "Replay",
        "lobbies.continue": "Continue",
        "lobbies.back": "Back",
        "lobbies.map": "Map",
        "menu.heroes": "Heroes",
        "lobbies.campaign_boudica_title": "Rise of the Iceni",
        "lobbies.campaign_boudica_subtitle": "Tutorial · Boudica",
        "leaders.lady_six_sky": "Lady Six Sky",
    };

    const mockMenuUi = {
        state: {
            phase: "MainMenu",
            campaign: {
                tutorial_completed: false,
                continue_episode: "boudica",
                episodes: [
                    {
                        id: "boudica",
                        title: { key: "lobbies.campaign_boudica_title" },
                        subtitle: { key: "lobbies.campaign_boudica_subtitle" },
                        completed: true,
                        unlocked: true,
                        replayable: true,
                    },
                    {
                        id: "six_sky_ep1",
                        title: "Arrival",
                        subtitle: "Lady Six Sky · 682 CE",
                        completed: false,
                        unlocked: true,
                        replayable: false,
                    },
                    {
                        id: "six_sky_ep2",
                        title: "Regent's Fire",
                        subtitle: "Lady Six Sky · 693 CE",
                        completed: false,
                        unlocked: false,
                        replayable: false,
                    },
                    {
                        id: "six_sky_ep3",
                        title: "Moon Goddess",
                        subtitle: "Lady Six Sky · 726–741 CE",
                        completed: false,
                        unlocked: false,
                        replayable: false,
                    },
                ],
            },
        },
        campaignOpen: true,
        selectedCampaignEpisode: null as string | null,
        selectedCampaignSaga: "all" as string | null,
    };

    const sandbox = {
        window: {
            innerWidth: 1200,
            innerHeight: 800,
            matchMedia: () => ({ matches: false }),
        },
        SOW_t: (key: string) => (strings as Record<string, string>)[key] || key,
        menuUi: mockMenuUi,
        asset: (p: string) => "/assets/" + p,
        leaderArtUrl: (slug: string, variant: string = "desktop") =>
            `/assets/leaders/${slug}_${variant}.webp`,
        leaderArtSlug: (leader: any) => (leader && leader.slug) || "boudica",
        leaderById: (id: string) => ({ id, slug: id, name: id }),
        leaderDisplayName: (leader: any) => (leader && leader.name) || "Hero",
        formatMapName: (name: string) => name,
        lobbyThumb: (opt: any) => `/maps/${opt.map_name}/thumbnail.webp`,
        localizedText: (val: any) =>
            val && val.key ? (strings as Record<string, string>)[val.key] || val.key : String(val || ""),
        esc: (s: any) =>
            String(s || "")
                .replace(/&/g, "&amp;")
                .replace(/</g, "&lt;")
                .replace(/'/g, "&#39;"),
        render: () => {},
        activeScreenPanel: () => null,
        root: { dataset: {} },
    };

    return { sandbox, mockMenuUi };
}

test("campaign metadata correctly maps heroes, maps, and sagas", async () => {
    const lobbies = await import("./main_menu.lobbies.ts");

    // Hero slug mapping
    assert.equal(lobbies.campaignEpisodeHeroSlug("boudica"), "boudica");
    assert.equal(lobbies.campaignEpisodeHeroSlug("six_sky_ep1"), "lady_six_sky");
    assert.equal(lobbies.campaignEpisodeHeroSlug("six_sky_ep2"), "lady_six_sky");
    assert.equal(lobbies.campaignEpisodeHeroSlug("six_sky_ep3"), "lady_six_sky");
    assert.equal(lobbies.campaignEpisodeHeroSlug("caesar_ep1"), "caesar");

    // Map name mapping
    assert.equal(lobbies.campaignEpisodeMapName("boudica"), "eastanglia");
    assert.equal(lobbies.campaignEpisodeMapName("six_sky_ep1"), "northamerica");
    assert.equal(lobbies.campaignEpisodeMapName("six_sky_ep2"), "northamerica");
    assert.equal(lobbies.campaignEpisodeMapName("six_sky_ep3"), "northamerica");

    // Saga ID grouping
    assert.equal(lobbies.campaignEpisodeSagaId("boudica"), "boudica");
    assert.equal(lobbies.campaignEpisodeSagaId("six_sky_ep1"), "six_sky");
    assert.equal(lobbies.campaignEpisodeSagaId("six_sky_ep2"), "six_sky");
    assert.equal(lobbies.campaignEpisodeSagaId("six_sky_ep3"), "six_sky");
    assert.equal(lobbies.campaignEpisodeSagaId("alexander_ep1"), "alexander");
});

test("campaign render generates featured hero showcase, responsive picture, and dynamic REPLAY/PLAY action", async () => {
    const lobbies = await import("./main_menu.lobbies.ts");
    const { menuUi } = await import("./main_menu.core.ts");

    // Setup mock campaign state
    menuUi.state = {
        phase: "MainMenu",
        campaign: {
            tutorial_completed: true,
            continue_episode: "six_sky_ep1",
            episodes: [
                {
                    id: "boudica",
                    title: "Rise of the Iceni",
                    subtitle: "Tutorial · Boudica",
                    completed: true,
                    unlocked: true,
                    replayable: true,
                },
                {
                    id: "six_sky_ep1",
                    title: "Arrival",
                    subtitle: "Lady Six Sky · 682 CE",
                    completed: false,
                    unlocked: true,
                    replayable: false,
                },
                {
                    id: "six_sky_ep2",
                    title: "Regent's Fire",
                    subtitle: "Lady Six Sky · 693 CE",
                    completed: false,
                    unlocked: false,
                    replayable: false,
                },
            ],
        },
    } as any;
    menuUi.campaignOpen = true;

    // Test 1: Selecting completed Boudica episode shows REPLAY and Boudica hero art
    menuUi.selectedCampaignEpisode = "boudica";
    menuUi.selectedCampaignSaga = "all";
    const htmlBoudica = lobbies.renderCampaign();

    assert.match(htmlBoudica, /class=['"][^'"]*sow-campaign__featured/);
    assert.match(htmlBoudica, /data-hero-slug=['"]boudica['"]/);
    assert.match(htmlBoudica, /class=['"][^'"]*sow-campaign__action-play/);
    assert.match(htmlBoudica, /data-episode-id=['"]boudica['"]/);
    // When completed, label is Replay
    assert.match(htmlBoudica, /REPLAY|Replay|Repetir/i);

    // Test 2: Selecting incompleted Six Sky episode shows PLAY/CONTINUE and Lady Six Sky hero art
    menuUi.selectedCampaignEpisode = "six_sky_ep1";
    const htmlSixSky = lobbies.renderCampaign();

    assert.match(htmlSixSky, /data-hero-slug=['"]lady_six_sky['"]/);
    assert.match(htmlSixSky, /data-episode-id=['"]six_sky_ep1['"]/);
    // When not completed, label is Play or Continue
    assert.match(htmlSixSky, /PLAY|CONTINUE|Play|Continue|Jugar|Continuar/i);

    // Test 3: Sagas navigation rail renders All, Boudica, Six Sky tabs
    assert.match(htmlSixSky, /class=['"][^'"]*sow-campaign__sagas-nav/);
    assert.match(htmlSixSky, /data-saga-id=['"]all['"]/);
    assert.match(htmlSixSky, /data-saga-id=['"]boudica['"]/);
    assert.match(htmlSixSky, /data-saga-id=['"]six_sky['"]/);

    // Test 4: Filtering by Saga "six_sky" hides Boudica card
    menuUi.selectedCampaignSaga = "six_sky";
    const htmlFiltered = lobbies.renderCampaign();
    assert.match(htmlFiltered, /data-episode-id=['"]six_sky_ep1['"]/);
    assert.doesNotMatch(
        htmlFiltered,
        /class=['"][^'"]*sow-campaign__episode[^'"]*data-episode-id=['"]boudica['"]/,
    );
});

test("campaign expandability: handles 40+ episodes across multiple sagas with correct grouping and hero art", async () => {
    const lobbies = await import("./main_menu.lobbies.ts");
    const { menuUi } = await import("./main_menu.core.ts");

    // Generate 40 episodes across 5 different sagas/heroes
    const leaders = ["boudica", "six_sky", "caesar", "alexander", "genghis_khan"];
    const episodes: any[] = [];
    for (const leader of leaders) {
        for (let i = 1; i <= 8; i++) {
            const epId = leader === "boudica" && i === 1 ? "boudica" : `${leader}_ep${i}`;
            episodes.push({
                id: epId,
                title: `${leader} Mission ${i}`,
                subtitle: `${leader} Campaign ${i}`,
                completed: i <= 3, // first 3 completed
                unlocked: i <= 4, // 4th unlocked, rest locked
                replayable: i <= 3,
            });
        }
    }
    assert.equal(episodes.length, 40);

    menuUi.state = {
        phase: "MainMenu",
        campaign: {
            tutorial_completed: true,
            continue_episode: "caesar_ep4",
            episodes,
        },
    } as any;
    menuUi.campaignOpen = true;

    // 1. Check all sagas are present in the tabs
    menuUi.selectedCampaignSaga = "all";
    menuUi.selectedCampaignEpisode = "caesar_ep4";
    const htmlAll = lobbies.renderCampaign();

    assert.match(htmlAll, /data-saga-id=['"]all['"]/);
    assert.match(htmlAll, /data-saga-id=['"]boudica['"]/);
    assert.match(htmlAll, /data-saga-id=['"]six_sky['"]/);
    assert.match(htmlAll, /data-saga-id=['"]caesar['"]/);
    assert.match(htmlAll, /data-saga-id=['"]alexander['"]/);
    assert.match(htmlAll, /data-saga-id=['"]genghis_khan['"]/);

    // Caesar episode 4 is unlocked & not completed -> action is PLAY/CONTINUE
    assert.match(htmlAll, /data-hero-slug=['"]caesar['"]/);
    assert.match(htmlAll, /data-episode-id=['"]caesar_ep4['"]/);
    assert.match(htmlAll, /PLAY|CONTINUE|Play|Continue|Jugar|Continuar/i);

    // 2. Select completed episode Caesar episode 1 -> action is REPLAY
    menuUi.selectedCampaignEpisode = "caesar_ep1";
    const htmlCaesar1 = lobbies.renderCampaign();
    assert.match(htmlCaesar1, /data-episode-id=['"]caesar_ep1['"]/);
    assert.match(htmlCaesar1, /REPLAY|Replay|Repetir/i);

    // 3. Filter by Alexander saga -> shows 8 Alexander episodes, hides Caesar
    menuUi.selectedCampaignSaga = "alexander";
    menuUi.selectedCampaignEpisode = "alexander_ep1";
    const htmlAlexander = lobbies.renderCampaign();
    assert.match(htmlAlexander, /data-hero-slug=['"]alexander['"]/);
    assert.match(htmlAlexander, /data-episode-id=['"]alexander_ep1['"]/);
    assert.match(htmlAlexander, /data-episode-id=['"]alexander_ep8['"]/);
    assert.doesNotMatch(
        htmlAlexander,
        /class=['"][^'"]*sow-campaign__episode[^'"]*data-episode-id=['"]caesar_ep1['"]/,
    );
});

test("campaign CSS includes full responsive rules for desktop, mobile vertical, and mobile compact horizontal", () => {
    const cssPath = path.join(shell, "main_menu.campaign.css");
    const css = fs.readFileSync(cssPath, "utf8");

    // Tactical card tokens
    assert.match(css, /\.sow-campaign__episode\s*\{[^}]*background:\s*var\(--sow-tactical-surface-card-bg\)/);

    // Hero backdrop and animations
    assert.match(css, /\.sow-campaign__hero-backdrop/);
    assert.match(css, /\.sow-campaign__hero-vignette/);
    assert.match(css, /@keyframes\s+sow-campaign-hero-enter/);
    assert.match(css, /@keyframes\s+sow-campaign-pulse/);

    // Selected state indicator
    assert.match(css, /\.sow-campaign__episode\.is-selected/);

    // Mobile vertical media query (portrait / <= 680px)
    assert.match(css, /@media\s*\(max-width:\s*680px\),\s*\(orientation:\s*portrait\)/);
    const mobilePortraitIdx = css.indexOf("@media (max-width: 680px), (orientation: portrait)");
    const mobilePortraitBlock = css.slice(mobilePortraitIdx, mobilePortraitIdx + 1200);
    assert.match(mobilePortraitBlock, /\.sow-campaign__featured/);
    assert.match(mobilePortraitBlock, /\.sow-campaign__action-play/);

    // Mobile horizontal compact media query (landscape and max-height: 560px)
    assert.match(css, /@media\s*\(orientation:\s*landscape\)\s*and\s*\(max-height:\s*560px\)/);
    const compactIdx = css.indexOf("@media (orientation: landscape) and (max-height: 560px)");
    const compactBlock = css.slice(compactIdx, compactIdx + 1500);
    assert.match(compactBlock, /\.sow-campaign__featured/);
    assert.match(compactBlock, /\.sow-campaign__workspace/);
    assert.match(compactBlock, /\.sow-campaign__action-play/);

    // Reduced motion support
    assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
});

test("custom game war room renders visual map theatre, in-art arrows, thumbnail strip, and icon-driven controls", async () => {
    (globalThis as any).SOW_renderDropdown = (cfg: any) =>
        `<div class="sow-control-dropdown" data-control-dropdown data-dropdown-key="${cfg.key}"><input data-dropdown-input name="${cfg.name || ""}" value="${cfg.value || ""}"></div>`;
    const lobbies = await import("./main_menu.lobbies.ts");
    const { menuUi } = await import("./main_menu.core.ts");

    menuUi.state = {
        phase: "MainMenu",
        map_catalog: [
            {
                key: "world",
                name: "World",
                width: 2000,
                height: 1000,
                default_roster: "classic",
                roster_presets: [{ id: "classic", name: "Classic" }],
            },
            {
                key: "europe",
                name: "Europe",
                width: 1600,
                height: 1200,
                default_roster: null,
                roster_presets: [],
            },
            {
                key: "eastasia",
                name: "East Asia",
                width: 1500,
                height: 1400,
                default_roster: null,
                roster_presets: [],
            },
        ],
        custom_game_config: {
            map_name: "world",
            map_width: 2000,
            map_height: 1000,
            game_mode: "FFA",
            bot_difficulty: "Vanilla",
            random_spawn: true,
            max_players: 8,
            bot_count: 128,
            nation_count: 32,
            seed: 42,
        },
        bot_limits: { tribes: 400, nations: 200 },
    } as any;
    menuUi.createDraft = null;
    menuUi.createOffline = false;
    menuUi.createPrivate = false;

    const htmlOnline = lobbies.renderCreate();
    assert.match(htmlOnline, /data-create-map-stage/);
    assert.match(htmlOnline, /data-command=['"]cycle_create_map['"][^>]*data-dir=['"]-1['"]/);
    assert.match(htmlOnline, /data-command=['"]cycle_create_map['"][^>]*data-dir=['"]1['"]/);
    assert.match(htmlOnline, /class=['"][^'"]*sow-create__map-strip/);
    assert.match(htmlOnline, /data-command=['"]select_create_map['"][^>]*data-map-key=['"]europe['"]/);
    assert.match(htmlOnline, /class=['"][^'"]*sow-create__mode-grid/);
    assert.match(htmlOnline, /data-command=['"]step_create_slider['"][^>]*data-field=['"]max_players['"]/);
    assert.match(htmlOnline, /data-command=['"]open_campaign['"]/);

    // Cycle to next map triggers directional entrance animation
    lobbies.cycleCreateMap(1);
    assert.equal(menuUi.createDraft?.map_name, "europe");
    const htmlNext = lobbies.renderCreate();
    assert.match(htmlNext, /is-animating-enter-next/);

    // Direct map selection triggers directional entrance animation
    lobbies.selectCreateMap("world");
    assert.equal(menuUi.createDraft?.map_name, "world");
    const htmlPrev = lobbies.renderCreate();
    assert.match(htmlPrev, /is-animating-enter-prev/);

    // Step slider helper clamps and increments accurately
    lobbies.stepCreateSlider("max_players", 2);
    assert.equal(menuUi.createDraft?.max_players, 10);
    lobbies.stepCreateSlider("bot_count", -16);
    assert.equal(menuUi.createDraft?.bot_count, 112);

    // Browser screen renders compact war room header and toolbar
    const browserHtml = lobbies.renderBrowser();
    assert.match(browserHtml, /class=['"][^'"]*sow-browser__header/);
    assert.match(browserHtml, /class=['"][^'"]*sow-browser__toolbar/);
    assert.match(browserHtml, /data-command=['"]open_create['"]/);
});

test("custom game and lobby browser CSS support desktop, mobile vertical, and mobile compact horizontal", () => {
    const createCss = fs.readFileSync(path.join(shell, "main_menu.create.css"), "utf8");
    const lobbiesCss = fs.readFileSync(path.join(shell, "main_menu.lobbies.css"), "utf8");

    assert.match(createCss, /@keyframes\s+sow-create-map-enter-next/);
    assert.match(createCss, /@keyframes\s+sow-create-map-enter-prev/);
    assert.match(createCss, /@media\s*\(max-width:\s*680px\),\s*\(orientation:\s*portrait\)/);
    assert.match(createCss, /@media\s*\(orientation:\s*landscape\)\s*and\s*\(max-height:\s*560px\)/);

    assert.match(lobbiesCss, /\.sow-browser__header/);
    assert.match(lobbiesCss, /@media\s*\(max-width:\s*680px\),\s*\(orientation:\s*portrait\)/);
    assert.match(lobbiesCss, /@media\s*\(orientation:\s*landscape\)\s*and\s*\(max-height:\s*560px\)/);
});
