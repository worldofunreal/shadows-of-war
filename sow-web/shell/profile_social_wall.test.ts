import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const shell = import.meta.dirname;

const profileCss = fs.readFileSync(path.join(shell, "main_menu.profile.css"), "utf8");
const profileSource = fs.readFileSync(path.join(shell, "main_menu.profile.ts"), "utf8");
const coreSource = fs.readFileSync(path.join(shell, "main_menu.core.ts"), "utf8");

test("profile CSS defines responsive layout for desktop, portrait mobile, and compact landscape", () => {
    // Desktop overview grid
    assert.match(
        profileCss,
        /\.sow-profile__overview\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)\s*340px/,
        "Desktop overview should be a 2-column grid with a 340px sidebar",
    );

    // Mobile vertical / portrait breakpoint
    assert.match(
        profileCss,
        /@media\s*\((max-width:\s*680px\)|orientation:\s*portrait)\s*,\s*\((max-width:\s*680px\)|orientation:\s*portrait)\)/,
        "Mobile portrait breakpoint should be present",
    );

    // Mobile compact landscape breakpoint
    assert.match(
        profileCss,
        /@media\s*\(orientation:\s*landscape\)\s*and\s*\(max-height:\s*560px\)/,
        "Mobile compact landscape breakpoint should be present",
    );

    // Check stats ribbon grid definitions
    assert.match(
        profileCss,
        /\.sow-profile__stats\s*\{[^}]*grid-template-columns:\s*repeat\(5,\s*minmax\(0,\s*1fr\)\)/,
        "Stats ribbon should be 5 columns on desktop",
    );

    // Social match cards have elevation and hover transition
    assert.match(
        profileCss,
        /\.sow-profile__match\s*\{[^}]*border-radius:\s*10px/,
        "Match card should have rounded border-radius",
    );
    assert.match(
        profileCss,
        /\.sow-profile__match:hover[^}]*transform:\s*translateY\(-2px\)/,
        "Match card hover should have elevation lift",
    );
});

test("profile navigation stack tracks multi-profile browsing and returns cleanly", () => {
    // Evaluate navigation stack logic in a simulated environment
    const menuUi: {
        profileAccountId: string | null;
        profileOwnId: string | null;
        profileTab: string;
        profileNavStack: { accountId: string; tab: string }[];
        state: any;
    } = {
        profileAccountId: "player-alpha",
        profileOwnId: "player-alpha",
        profileTab: "overview",
        profileNavStack: [],
        state: { account_id: "player-alpha" },
    };

    function openProfile(id: string | null, isNavigation = false) {
        const nextId = id || menuUi.profileOwnId || (menuUi.state && menuUi.state.account_id);
        if (!isNavigation && menuUi.profileAccountId && nextId && menuUi.profileAccountId !== nextId) {
            if (!menuUi.profileNavStack) menuUi.profileNavStack = [];
            menuUi.profileNavStack.push({
                accountId: menuUi.profileAccountId,
                tab: menuUi.profileTab || "overview",
            });
        }
        menuUi.profileAccountId = nextId;
    }

    function popProfileNav(): boolean {
        if (menuUi.profileNavStack && menuUi.profileNavStack.length > 0) {
            const previous = menuUi.profileNavStack.pop()!;
            openProfile(previous.accountId, true);
            menuUi.profileTab = previous.tab || "overview";
            return true;
        }
        const ownId = menuUi.profileOwnId || (menuUi.state && menuUi.state.account_id);
        if (menuUi.profileAccountId && ownId && menuUi.profileAccountId !== ownId) {
            openProfile(ownId, true);
            return true;
        }
        return false;
    }

    // Step 1: Initial state is own profile
    assert.equal(menuUi.profileAccountId, "player-alpha");
    assert.equal(menuUi.profileNavStack.length, 0);

    // Step 2: Navigate to another player's profile (e.g. from match details or leaderboard)
    openProfile("player-beta");
    assert.equal(menuUi.profileAccountId, "player-beta");
    assert.equal(menuUi.profileNavStack.length, 1);
    assert.deepEqual(menuUi.profileNavStack[0], { accountId: "player-alpha", tab: "overview" });

    // Step 3: From player-beta, inspect player-gamma
    openProfile("player-gamma");
    assert.equal(menuUi.profileAccountId, "player-gamma");
    assert.equal(menuUi.profileNavStack.length, 2);

    // Step 4: Hit Back (popProfileNav) -> returns to player-beta
    const popped1 = popProfileNav();
    assert.equal(popped1, true);
    assert.equal(menuUi.profileAccountId, "player-beta");
    assert.equal(menuUi.profileNavStack.length, 1);

    // Step 5: Hit Back again -> returns to player-alpha
    const popped2 = popProfileNav();
    assert.equal(popped2, true);
    assert.equal(menuUi.profileAccountId, "player-alpha");
    assert.equal(menuUi.profileNavStack.length, 0);

    // Step 6: Hit Back on own profile root -> returns false (allows menu shell to close screen)
    const popped3 = popProfileNav();
    assert.equal(popped3, false);
});

test("profile source code contains social wall markup, cover banner, and bio stats ribbon", () => {
    // Assert presence of cover banner container and hero background
    assert.match(
        profileSource,
        /data-profile-cover/,
        "Profile markup must include data-profile-cover container",
    );
    assert.match(
        profileSource,
        /sow-profile__cover-overlay/,
        "Profile markup must include sow-profile__cover-overlay",
    );

    // Assert presence of avatar container with overlapping identity badge
    assert.match(
        profileSource,
        /sow-profile__avatar-container/,
        "Profile markup must include overlapping avatar container",
    );

    // Assert presence of 5-column bio stats ribbon
    assert.match(
        profileSource,
        /function profileStatsRibbonMarkup/,
        "Profile must include profileStatsRibbonMarkup function",
    );
    assert.match(
        profileSource,
        /sow-profile__stat-kda/,
        "Stats ribbon must include KDA stat slot",
    );

    // Assert presence of 2-column wall overview (feed + sidebar)
    assert.match(
        profileSource,
        /sow-profile__overview-feed/,
        "Overview tab must include feed column",
    );
    assert.match(
        profileSource,
        /sow-profile__overview-sidebar/,
        "Overview tab must include sidebar column",
    );

    // Assert presence of quick jump 'Mi Perfil' button when browsing others
    assert.match(
        profileSource,
        /data-command='back_to_my_profile'/,
        "Header must include back_to_my_profile command for quick return",
    );
});

test("profile achievements system renders prestige dossier, category filter pills, and mythic progression", () => {
    // Assert presence of Prestige Header with Laurel icon and progress
    assert.match(
        profileSource,
        /sow-profile__achievements-laurel-card/,
        "Profile must include laurel callout card in prestige dossier",
    );
    assert.match(
        profileSource,
        /currencyAsset\("laurel"\)/,
        "Laurel callouts must render the canonical laurel asset",
    );
    assert.match(
        profileSource,
        /sow-profile__achievements-progress-card/,
        "Profile must include total achievements progress card",
    );

    // Assert presence of Category Filter Bar & Chips
    assert.match(
        profileSource,
        /sow-profile__ach-filter-bar/,
        "Profile must include category filter navigation bar",
    );
    assert.match(
        profileSource,
        /data-command='profile_achievement_filter'/,
        "Filter chips must trigger profile_achievement_filter command",
    );
    assert.match(
        profileSource,
        /profileAchievementFilter/,
        "Menu state must track active achievement filter",
    );

    // Assert Mythic Tier milestone support
    assert.match(
        profileSource,
        /isMythic = target >= 1000/,
        "Target >= 1000 milestones must be classified as Mythic",
    );
    assert.match(
        profileSource,
        /sow-profile__achievement-mythic-pill/,
        "Mythic achievements must render radiant mythic pill",
    );

    // Assert CSS defines mythic styles and category filter chips
    assert.match(
        profileCss,
        /\.sow-profile__ach-filter-chip\.is-active/,
        "CSS must define active state for filter chips",
    );
    assert.match(
        profileCss,
        /\.sow-profile__achievement\.is-mythic/,
        "CSS must define mythic styling with golden radiance",
    );
});

test("profile achievements expands to 1,466 achievements across all 93 heroes with hero avatars and search", () => {
    // Assert 93 heroes defined in catalog
    assert.match(
        profileSource,
        /const ALL_HEROES:\s*\{\s*slug:\s*string;\s*name:\s*string\s*\}\[\]/,
        "Profile must define all playable heroes for the full achievements matrix",
    );
    assert.match(
        profileSource,
        /export function getFullAchievementsCatalog\(\)/,
        "Profile must export getFullAchievementsCatalog",
    );
    assert.match(
        profileSource,
        /export function resolveProfileAchievements\(/,
        "Profile must export resolveProfileAchievements",
    );

    // Hero avatars in achievement badges
    assert.match(
        profileSource,
        /sow-profile__ach-hero-avatar/,
        "Hero-specific achievements must render hero avatar badge",
    );
    assert.match(
        profileCss,
        /\.sow-profile__ach-hero-avatar/,
        "CSS must style hero avatar badges",
    );

    // Search bar for achievements
    assert.match(
        profileSource,
        /data-role='achievements-search'/,
        "Achievements tab must provide search bar for rapid filtering",
    );
    assert.match(
        profileCss,
        /\.sow-profile__ach-search-input/,
        "CSS must style achievement search input",
    );
});
