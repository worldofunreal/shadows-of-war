import {
    asset,
    currentScreen,
    esc,
    leaderArtUrl,
    leaderArtSlug,
    leaderById,
    leaderCivilization,
    leaderDisplayName,
    leaderHistoricalName,
    menuUi,
    prepareLeaderArt,
    renderHeroBriefing,
    renderShowcaseLoader,
    root,
} from "./main_menu.core.ts";
import { activeScreenPanel, render } from "./main_menu.shell.ts";
import { syncDropdownScope } from "./sow-dropdown.ts";
import { avatarAtlasCell, avatarImageAttribute, deferAvatarImage, loadAvatarImage } from "./avatar-presentation.ts";
import {
    computeHeroAchievementSummary,
    renderHeroDossierAchievementsTab,
    renderHeroSelectionAchievementStrip,
    tText,
    type HeroAchievementSummary,
} from "./main_menu.achievements.ts";
import type { HeroBioEntry, MenuLeader } from "./main_menu.state.ts";
import { renderLeaderPurchase, renderSkinsButton } from "./main_menu.surfaces.ts";

// Hero selection, filtering, dropdowns, and preview updates.

function isWeeklyFreeRotation(leader: MenuLeader | null) {
    return !!leader && leader.available === true && leader.free_rotation === true && leader.owned === false;
}

type HeroesAvatarExpression = "calm" | "happy" | "angry";

const heroesAvatarAnimations = new WeakMap<HTMLImageElement, Animation>();
const heroesAvatarRevisions = new WeakMap<HTMLImageElement, number>();
const heroesAvatarSources = new WeakMap<HTMLImageElement, string>();
let heroesFeaturedEnterTimer: number | null = null;
let heroesFeaturedPreloadUrl = "";
let heroesFeaturedPreloadImage: HTMLImageElement | null = null;

function heroesAvatarUrl(slug: string, expression: HeroesAvatarExpression) {
    const path =
        expression === "calm"
            ? "gameplay/avatars/" + slug + ".webp"
            : "gameplay/avatars/expressions/" + slug + "_" + expression + ".webp";
    return asset(path);
}

function setHeroesFeaturedPlaceholder(featured: HTMLElement, slug: string) {
    const calm = heroesAvatarUrl(slug, "calm");
    if (!avatarAtlasCell(calm)) {
        featured.style.setProperty("--sow-hero-featured-placeholder", "url(" + JSON.stringify(calm) + ")");
        return;
    }
    featured.dataset.heroPlaceholder = slug;
    void loadAvatarImage(calm)
        .then((portrait) => {
            if (featured.dataset.heroPlaceholder !== slug) return;
            featured.style.setProperty("--sow-hero-featured-placeholder", "url(" + JSON.stringify(portrait.src) + ")");
        })
        .catch(() => undefined);
}

function preloadHeroesFeaturedArt(leader: MenuLeader) {
    const art = heroesFeaturedArt(leader.slug);
    const compact =
        typeof window.matchMedia === "function"
            ? window.matchMedia("(max-width: 900px), (orientation: portrait)").matches
            : window.innerWidth <= 900 || window.innerHeight >= window.innerWidth;
    const url = compact ? art.band : art.column;
    if (!url || (url === heroesFeaturedPreloadUrl && heroesFeaturedPreloadImage)) return;
    const image = new Image();
    image.decoding = "async";
    image.fetchPriority = "low";
    image.src = url;
    heroesFeaturedPreloadUrl = url;
    heroesFeaturedPreloadImage = image;
}

function heroesMotionReduced() {
    return (
        (typeof window.matchMedia === "function" &&
            window.matchMedia("(prefers-reduced-motion: reduce)").matches) ||
        !!(menuUi.state && menuUi.state.settings && menuUi.state.settings.reduced_motion)
    );
}

function setHeroesAvatar(image: HTMLImageElement | null, slug: string, expression: HeroesAvatarExpression) {
    if (!image || !slug) return;
    const source = heroesAvatarUrl(slug, expression);
    const packed = avatarAtlasCell(source) !== null;
    const previousSource = image.getAttribute("src") || image.src || "";
    if (heroesAvatarSources.get(image) === source) return;
    if (!packed && (previousSource === source || (previousSource && previousSource.endsWith(source)))) return;
    heroesAvatarSources.set(image, source);
    const revision = (heroesAvatarRevisions.get(image) || 0) + 1;
    heroesAvatarRevisions.set(image, revision);
    const running = heroesAvatarAnimations.get(image);
    if (running) running.cancel();
    if (!packed) image.src = source;

    const animateLoadedAvatar = function () {
        if (
            heroesAvatarRevisions.get(image) !== revision ||
            !image.naturalWidth ||
            heroesMotionReduced() ||
            typeof image.animate !== "function"
        )
            return;
        const animation = image.animate(
            [
                { opacity: 0.35, transform: "scale(0.86) translateY(2px)" },
                { opacity: 1, transform: "scale(1) translateY(0)" },
            ],
            {
                duration: expression === "happy" ? 340 : 250,
                easing: "cubic-bezier(0.2, 0.8, 0.2, 1)",
                fill: "both",
            },
        );
        heroesAvatarAnimations.set(image, animation);
    };
    if (packed) {
        void loadAvatarImage(source).then((portrait) => {
            if (heroesAvatarRevisions.get(image) !== revision) return;
            image.src = portrait.src;
            if (heroesMotionReduced() || typeof image.animate !== "function") return;
            if (image.complete) {
                if (typeof window.requestAnimationFrame === "function")
                    window.requestAnimationFrame(animateLoadedAvatar);
                else animateLoadedAvatar();
            } else {
                image.addEventListener("load", animateLoadedAvatar, { once: true });
            }
        }).catch(() => {
            if (heroesAvatarRevisions.get(image) === revision) heroesAvatarSources.delete(image);
        });
        return;
    }
    if (heroesMotionReduced() || typeof image.animate !== "function") return;
    if (image.complete) {
        if (typeof window.requestAnimationFrame === "function")
            window.requestAnimationFrame(animateLoadedAvatar);
        else animateLoadedAvatar();
    } else {
        image.addEventListener("load", animateLoadedAvatar, { once: true });
    }
}

function setHeroesCardAvatar(card: HTMLElement, expression: HeroesAvatarExpression, eager = true) {
    const image = card.querySelector<HTMLImageElement>("img");
    const slug = card.dataset.avatarSlug || "";
    if (!image || !slug) return;
    if (eager) {
        setHeroesAvatar(image, slug, expression);
    } else {
        deferAvatarImage(image, heroesAvatarUrl(slug, expression));
    }
}

function heroesAvatarCard(event: Event) {
    const target = event.target;
    return target instanceof Element
        ? target.closest<HTMLElement>(".sow-heroes__card[data-avatar-slug]")
        : null;
}

function clearHeroesFeaturedAnimation(featured: HTMLElement) {
    if (heroesFeaturedEnterTimer !== null) window.clearTimeout(heroesFeaturedEnterTimer);
    heroesFeaturedEnterTimer = null;
    featured.classList.remove(
        "is-transitioning",
        "is-animating-enter-next",
        "is-animating-enter-prev",
        "is-copy-entering",
    );
}

function heroesLeaderDirection(previousId: string, nextId: string) {
    if (!previousId) return 1;
    const leaders = menuUi.state && Array.isArray(menuUi.state.leaders) ? menuUi.state.leaders : [];
    const previousIndex =
        previousId === "random" ? -1 : leaders.findIndex((leader) => leader.id === previousId);
    const nextIndex = nextId === "random" ? -1 : leaders.findIndex((leader) => leader.id === nextId);
    if (previousIndex < 0 && previousId !== "random") return 1;
    if (nextIndex < 0 && nextId !== "random") return 1;
    return nextIndex >= previousIndex ? 1 : -1;
}

function triggerHeroesFeaturedEnter(featured: HTMLElement, direction: number) {
    if (heroesMotionReduced()) return;
    const enterClass = direction === 1 ? "is-animating-enter-next" : "is-animating-enter-prev";
    featured.classList.add(enterClass, "is-copy-entering");
    heroesFeaturedEnterTimer = window.setTimeout(function () {
        featured.classList.remove(enterClass, "is-copy-entering");
        heroesFeaturedEnterTimer = null;
    }, 380);
}

function setFeaturedArtAvailability(image: HTMLImageElement, unavailable: boolean) {
    const slug = image.dataset.heroArtSlug;
    const source = (image.currentSrc || image.src || "").split(/[?#]/, 1)[0];
    if (
        !slug ||
        (!source.endsWith("/leaders/" + slug + "_desktop.webp") &&
            !source.endsWith("/leaders/" + slug + "_mobile.webp"))
    )
        return;
    const featured = image.closest<HTMLElement>(".sow-heroes__featured");
    if (!featured) return;
    const skillIcon = featured.querySelector<HTMLImageElement>("[data-hero-skill-icon]");
    if (!image.complete || (skillIcon && !skillIcon.complete)) return;
    const animateEntrance = featured.classList.contains("is-transitioning");
    featured.classList.remove("is-loading", "is-unavailable", "is-transitioning");
    if (unavailable) {
        clearHeroesFeaturedAnimation(featured);
        featured.classList.add("is-unavailable");
    } else {
        featured.style.removeProperty("--sow-hero-featured-placeholder");
        if (animateEntrance) {
            const direction = featured.dataset.heroDirection === "-1" ? -1 : 1;
            triggerHeroesFeaturedEnter(featured, direction);
        }
    }
}

export function settleHeroesFeaturedArt() {
    const panel = activeScreenPanel() || root;
    const image = panel.querySelector<HTMLImageElement>("[data-hero-featured-art]");
    if (!image) return;
    const featured = image.closest<HTMLElement>(".sow-heroes__featured");
    if (featured && image.dataset.heroArtSlug)
        setHeroesFeaturedPlaceholder(featured, image.dataset.heroArtSlug);
    if (image.complete) setFeaturedArtAvailability(image, image.naturalWidth === 0);
}

root.addEventListener(
    "load",
    function (event) {
        const image = event.target instanceof HTMLImageElement ? event.target : null;
        if (!image) return;
        if (image.hasAttribute("data-hero-featured-art")) setFeaturedArtAvailability(image, false);
        if (image.hasAttribute("data-hero-skill-icon")) {
            const featured = image.closest<HTMLElement>(".sow-heroes__featured");
            const art = featured?.querySelector<HTMLImageElement>("[data-hero-featured-art]");
            if (art && image.dataset.heroSkillArtSlug === art.dataset.heroArtSlug)
                setFeaturedArtAvailability(art, art.naturalWidth === 0);
        }
    },
    true,
);
root.addEventListener(
    "error",
    function (event) {
        const image = event.target instanceof HTMLImageElement ? event.target : null;
        if (!image) return;
        if (image.hasAttribute("data-hero-featured-art")) setFeaturedArtAvailability(image, true);
        if (image.hasAttribute("data-hero-skill-icon")) {
            const featured = image.closest<HTMLElement>(".sow-heroes__featured");
            const art = featured?.querySelector<HTMLImageElement>("[data-hero-featured-art]");
            if (art && image.dataset.heroSkillArtSlug === art.dataset.heroArtSlug)
                setFeaturedArtAvailability(art, art.naturalWidth === 0);
            image.closest(".sow-hero-briefing__icon-slot")?.remove();
        }
        if (image.hasAttribute("data-hero-art-avatar")) image.hidden = true;
    },
    true,
);
root.addEventListener("pointerover", function (event) {
    const card = heroesAvatarCard(event);
    if (!card || card.classList.contains("is-selected")) return;
    if (event.relatedTarget instanceof Node && card.contains(event.relatedTarget)) return;
    setHeroesCardAvatar(card, "happy");
    const leader = leaderById(card.dataset.leaderId);
    if (leader) preloadHeroesFeaturedArt(leader);
});
root.addEventListener("pointerout", function (event) {
    const card = heroesAvatarCard(event);
    if (!card) return;
    if (event.relatedTarget instanceof Node && card.contains(event.relatedTarget)) return;
    setHeroesCardAvatar(card, card.classList.contains("is-selected") ? "angry" : "calm");
});
root.addEventListener("focusin", function (event) {
    const card = heroesAvatarCard(event);
    if (card && !card.classList.contains("is-selected")) {
        setHeroesCardAvatar(card, "happy");
        const leader = leaderById(card.dataset.leaderId);
        if (leader) preloadHeroesFeaturedArt(leader);
    }
});
root.addEventListener("focusout", function (event) {
    const card = heroesAvatarCard(event);
    if (!card || (event.relatedTarget instanceof Node && card.contains(event.relatedTarget))) return;
    setHeroesCardAvatar(card, card.classList.contains("is-selected") ? "angry" : "calm");
});

function heroesRarityLabel(leader: MenuLeader) {
    return leader.rarity ? SOW_t("heroes.rarity_" + leader.rarity) : "";
}

function heroesStatusLabel(leader: MenuLeader | null) {
    if (!leader || leader.id === "Boudica") return "";
    if (leader.permanently_free) return SOW_t("heroes.always_free");
    if (isWeeklyFreeRotation(leader)) return SOW_t("heroes.weekly_free_rotation");
    return "";
}

function renderHeroStatusTags(leader: MenuLeader | null, live: boolean) {
    const rarityLabel = leader ? heroesRarityLabel(leader) : "";
    const raritySlug = leader && leader.rarity ? leader.rarity : "";
    const statusLabel = heroesStatusLabel(leader);
    const hasAny = Boolean(rarityLabel || statusLabel);
    if (!live && !hasAny) return "";
    return (
        "<div class='sow-heroes__tags'" +
        (live ? " data-hero-tags" : "") +
        (!hasAny ? " hidden" : "") +
        ">" +
        (live || rarityLabel
            ? "<span class='sow-heroes__rarity-tag'" +
              (live ? " data-hero-rarity" : "") +
              " data-rarity='" +
              esc(raritySlug) +
              "'" +
              (!rarityLabel ? " hidden" : "") +
              ">" +
              esc(rarityLabel) +
              "</span>"
            : "") +
        (live || statusLabel
            ? "<span class='sow-heroes__rotation'" +
              (live ? " data-hero-status" : "") +
              (!statusLabel ? " hidden" : "") +
              "><span" +
              (live ? " data-hero-status-label" : "") +
              ">" +
              esc(statusLabel) +
              "</span></span>"
            : "") +
        "</div>"
    );
}

// OWNER DECISION — DO NOT "FIX" THIS MAPPING.
//
// The heroes featured art is chosen by the SHAPE OF THE PANEL, not by the
// device and not by the file name:
//   - Wide landscape layout: the panel is a tall narrow column, so it takes
//     the PORTRAIT art *_mobile.webp (1080x1920).
//   - Compact layout (portrait or landscape up to 900px): the panel becomes a
//     short wide band, so it takes the LANDSCAPE art *_desktop.webp (1920x1080).
//
// The file names are backwards, which is exactly why this line keeps
// getting "corrected" without authorization. Incident history:
//   - 80c7043d (2026-09-24) flipped it to the device-name mapping.
//   - b945feb3 (2026-09-24) flipped it back the same day.
//   - 12486980 (2026-10-01) flipped it again and shipped broken.
// Every flip was wrong: each one read "_mobile.webp on desktop" as a bug.
// It is not a bug. The owner explicitly asked for desktop = *_mobile.webp
// and phone = *_desktop.webp.
//
// This function is the ONLY owner of the mapping.
function heroesFeaturedArt(slug: string) {
    const artSlug = leaderArtSlug(leaderById(slug));
    return {
        band: leaderArtUrl(artSlug, "desktop"),
        column: leaderArtUrl(artSlug, "mobile"),
    };
}

function renderHeroesCard(leader: MenuLeader, activeId: string) {
    const selected = leader.id === activeId;
    const locked = leader.available === false;
    const avatarSlug = leaderArtSlug(leader);
    const avatarUrl = heroesAvatarUrl(avatarSlug, selected ? "angry" : "calm");
    const rarity = heroesRarityLabel(leader);
    const rarityClass = leader.rarity ? " sow-heroes__rarity sow-heroes__rarity--" + esc(leader.rarity) : "";
    const rarityAttr = leader.rarity ? " data-rarity='" + esc(leader.rarity) + "'" : "";
    const weeklyFree = !leader.permanently_free && leader.id !== "Boudica" && isWeeklyFreeRotation(leader);
    const achSummary = computeHeroAchievementSummary(leader);
    const achBadge =
        achSummary.unlockedCount > 0 || achSummary.matchesPlayed > 0
            ? "<span class='sow-heroes__card-ach" +
              (achSummary.unlockedCount === achSummary.totalCount ? " is-complete" : "") +
              "' title='" +
              esc(tText("heroes.achievements_progress", "{unlocked}/{total} Achievements", {
                  unlocked: achSummary.unlockedCount,
                  total: achSummary.totalCount,
              })) +
              "'>★ " +
              esc(achSummary.unlockedCount + "/" + achSummary.totalCount) +
              "</span>"
            : "";
    const meta =
        (rarity
            ? "<em class='" +
              (rarityClass ? rarityClass.trim() : "sow-heroes__rarity") +
              "'" +
              rarityAttr +
              ">" +
              esc(rarity) +
              "</em>"
            : "") +
        (weeklyFree ? "<span class='sow-heroes__card-free'>" + esc(SOW_t("heroes.free_week_badge")) + "</span>" : "") +
        achBadge;
    const progressLine =
        achSummary.percent > 0
            ? "<span class='sow-heroes__card-ach-bar' aria-hidden='true'><i style='width:" +
              esc(achSummary.percent) +
              "%'></i></span>"
            : "";
    return (
        "<button class='sow-heroes__card" +
        (selected ? " is-selected" : "") +
        (locked ? " is-locked" : "") +
        "' type='button' data-command='preview_leader' data-leader-id='" +
        esc(leader.id) +
        "' data-avatar-slug='" +
        esc(avatarSlug) +
        "' aria-pressed='" +
        selected +
        "'>" +
        "<span class='sow-heroes__card-art'><img " +
        avatarImageAttribute(avatarUrl) +
        " alt='' width='68' height='68' loading='lazy' decoding='async'></span>" +
        "<span class='sow-heroes__card-copy'><strong>" +
        esc(leaderDisplayName(leader)) +
        "</strong><small>" +
        esc(leaderCivilization(leader)) +
        "</small>" +
        (meta ? "<span class='sow-heroes__card-meta'>" + meta + "</span>" : "") +
        "</span>" +
        progressLine +
        "</button>"
    );
}

function randomLeaderName() {
    return SOW_t("heroes.random_leader");
}

function randomLeaderOption(): MenuLeader {
    return {
        id: "random",
        slug: "random",
        name: randomLeaderName(),
        civilization: "",
        available: true,
        permanently_free: true,
    };
}

function renderRandomLeaderCard(selected: boolean) {
    const matchesSearch =
        !menuUi.heroesSearchQuery.trim() ||
        randomLeaderName().toLowerCase().indexOf(menuUi.heroesSearchQuery.trim().toLowerCase()) !== -1;
    if (
        !matchesSearch ||
        menuUi.heroesRegionFilter !== "all" ||
        menuUi.heroesRarityFilter !== "all" ||
        menuUi.heroesStatusFilter !== "all"
    )
        return "";
    const avatarUrl = asset("gameplay/avatars/random.webp");
    return (
        "<button class='sow-heroes__card" +
        (selected ? " is-selected" : "") +
        "' type='button' data-command='preview_random_leader' aria-pressed='" +
        selected +
        "'>" +
        "<span class='sow-heroes__card-art'><img " +
        avatarImageAttribute(avatarUrl) +
        " alt='' width='68' height='68' loading='lazy' decoding='async'></span>" +
        "<span class='sow-heroes__card-copy'><strong>" +
        esc(randomLeaderName()) +
        "</strong><em>" +
        esc(SOW_t("heroes.random_leader_description")) +
        "</em></span></button>"
    );
}

function heroesRoster(leaders: MenuLeader[]) {
    const query = menuUi.heroesSearchQuery.trim().toLowerCase();
    const statusFilter = menuUi.heroesStatusFilter || "all";
    const summaryCache = new Map<string, HeroAchievementSummary>();
    const summaryFor = (leader: MenuLeader): HeroAchievementSummary => {
        const key = String(leader.slug || leader.id);
        let entry = summaryCache.get(key);
        if (!entry) {
            entry = computeHeroAchievementSummary(leader);
            summaryCache.set(key, entry);
        }
        return entry;
    };

    const filtered = leaders.filter(function (leader) {
        const matchesQuery =
            !query ||
            String(leaderDisplayName(leader) || "")
                .toLowerCase()
                .indexOf(query) !== -1 ||
            String(leaderHistoricalName(leader) || "")
                .toLowerCase()
                .indexOf(query) !== -1 ||
            String(leader.name || "")
                .toLowerCase()
                .indexOf(query) !== -1 ||
            String(leaderCivilization(leader) || "")
                .toLowerCase()
                .indexOf(query) !== -1;
        const matchesRegion =
            menuUi.heroesRegionFilter === "all" || leader.region === menuUi.heroesRegionFilter;
        const matchesRarity =
            menuUi.heroesRarityFilter === "all" || leader.rarity === menuUi.heroesRarityFilter;
        const matchesPlayable = !menuUi.heroesPlayableOnly || leader.available !== false;
        let matchesStatus = true;
        if (statusFilter !== "all") {
            const sum = summaryFor(leader);
            if (statusFilter === "owned") {
                matchesStatus = sum.ownershipStatus === "owned" || sum.ownershipStatus === "starter";
            } else if (statusFilter === "weekly_free") {
                matchesStatus = sum.ownershipStatus === "weekly_free";
            } else if (statusFilter === "locked") {
                matchesStatus = leader.available === false;
            } else if (statusFilter === "in_progress") {
                matchesStatus = sum.unlockedCount > 0 || sum.matchesPlayed > 0;
            } else if (statusFilter === "almost_milestone") {
                matchesStatus = sum.unlockedCount < sum.totalCount && sum.nextPercent >= 25;
            }
        }
        return matchesQuery && matchesRegion && matchesRarity && matchesPlayable && matchesStatus;
    });

    function leaderCost(leader: MenuLeader): number {
        if (typeof leader.cost_crowns === "number") return leader.cost_crowns;
        if (typeof leader.cost_gems === "number") return leader.cost_gems * 4;
        return 0;
    }

    function heroMatchesPlayed(leader: MenuLeader): number {
        if (leader.stats && typeof leader.stats.matches_played === "number") return leader.stats.matches_played;
        const map = menuUi.state?.profile_stats?.leader_stats;
        if (map && leader.name && map[leader.name]) return Number(map[leader.name].matches_played || 0);
        return 0;
    }

    function heroXp(leader: MenuLeader): number {
        if (typeof leader.xp === "number") return leader.xp;
        const map = menuUi.state?.profile_stats?.leader_xp;
        if (map && typeof map === "object" && leader.name) return Number(map[leader.name] || 0);
        return 0;
    }

    function sortLeaders(list: MenuLeader[], sort: string): MenuLeader[] {
        const copy = list.slice();
        if (sort === "achievements_desc") {
            copy.sort(function (a, b) {
                const sa = summaryFor(a);
                const sb = summaryFor(b);
                if (sb.percent !== sa.percent) return sb.percent - sa.percent;
                if (sb.unlockedCount !== sa.unlockedCount) return sb.unlockedCount - sa.unlockedCount;
                if (sb.nextPercent !== sa.nextPercent) return sb.nextPercent - sa.nextPercent;
                const diffMatches = sa.matchesPlayed !== sb.matchesPlayed ? sb.matchesPlayed - sa.matchesPlayed : 0;
                if (diffMatches !== 0) return diffMatches;
                return String(leaderDisplayName(a) || "").localeCompare(String(leaderDisplayName(b) || ""));
            });
        } else if (sort === "most_played") {
            copy.sort(function (a, b) {
                const diffMatches = heroMatchesPlayed(b) - heroMatchesPlayed(a);
                if (diffMatches !== 0) return diffMatches;
                const diffXp = heroXp(b) - heroXp(a);
                if (diffXp !== 0) return diffXp;
                return String(leaderDisplayName(a) || "").localeCompare(String(leaderDisplayName(b) || ""));
            });
        } else if (sort === "cost_desc") {
            copy.sort(function (a, b) {
                const diff = leaderCost(b) - leaderCost(a);
                if (diff !== 0) return diff;
                return String(leaderDisplayName(a) || "").localeCompare(String(leaderDisplayName(b) || ""));
            });
        } else if (sort === "cost_asc") {
            copy.sort(function (a, b) {
                const diff = leaderCost(a) - leaderCost(b);
                if (diff !== 0) return diff;
                return String(leaderDisplayName(a) || "").localeCompare(String(leaderDisplayName(b) || ""));
            });
        } else if (sort === "alpha_asc") {
            copy.sort(function (a, b) {
                return String(leaderDisplayName(a) || "").localeCompare(String(leaderDisplayName(b) || ""));
            });
        } else if (sort === "alpha_desc") {
            copy.sort(function (a, b) {
                return String(leaderDisplayName(b) || "").localeCompare(String(leaderDisplayName(a) || ""));
            });
        }
        return copy;
    }

    const sort = (menuUi && menuUi.heroesSort) || "default";
    const active = sortLeaders(
        filtered.filter(function (leader) {
            return leader.available !== false;
        }),
        sort,
    );
    const locked = sortLeaders(
        filtered.filter(function (leader) {
            return leader.available === false;
        }),
        sort,
    );
    return active.concat(locked);
}

const HEROES_FILTER_ICON =
    "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><polygon points='22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3'/></svg>";

const HEROES_SORT_ICON =
    "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m3 16 4 4 4-4'/><path d='M7 20V4'/><path d='m21 8-4-4-4 4'/><path d='M17 4v16'/></svg>";

function renderHeroesRegionDropdown() {
    return SOW_renderDropdown({
        key: "heroes-region",
        label: SOW_t("heroes.filter_leaders_region"),
        name: "region",
        icon: HEROES_FILTER_ICON,
        value: menuUi.heroesRegionFilter,
        className: "sow-heroes__region" + (menuUi.heroesRegionFilter !== "all" ? " is-filtered" : ""),
        options: [
            { value: "all", label: SOW_t("heroes.all_regions"), shortLabel: SOW_t("heroes.region_filter") },
            { value: "europe", label: SOW_t("heroes.europe") },
            { value: "africa", label: SOW_t("heroes.africa") },
            { value: "asia", label: SOW_t("heroes.asia") },
            { value: "americas", label: SOW_t("heroes.americas") },
            { value: "oceania", label: SOW_t("heroes.oceania") },
        ],
    });
}

function renderHeroesRarityDropdown() {
    return SOW_renderDropdown({
        key: "heroes-rarity",
        label: SOW_t("heroes.rarity_filter"),
        name: "rarity",
        icon: HEROES_FILTER_ICON,
        value: menuUi.heroesRarityFilter || "all",
        className: "sow-heroes__rarity-filter" + (menuUi.heroesRarityFilter !== "all" ? " is-filtered" : ""),
        options: [
            { value: "all", label: SOW_t("heroes.all_rarities"), shortLabel: SOW_t("heroes.rarity_filter") },
            { value: "common", label: SOW_t("heroes.rarity_common") },
            { value: "rare", label: SOW_t("heroes.rarity_rare") },
            { value: "epic", label: SOW_t("heroes.rarity_epic") },
            { value: "legendary", label: SOW_t("heroes.rarity_legendary") },
        ],
    });
}

function renderHeroesStatusDropdown() {
    const val = menuUi.heroesStatusFilter || "all";
    return SOW_renderDropdown({
        key: "heroes-status",
        label: tText("heroes.filter_status", "Filter by status"),
        name: "status",
        icon: HEROES_FILTER_ICON,
        value: val,
        className: "sow-heroes__status-filter" + (val !== "all" ? " is-filtered" : ""),
        options: [
            { value: "all", label: tText("heroes.all_statuses", "All status"), shortLabel: tText("heroes.status_filter", "Status") },
            { value: "owned", label: tText("heroes.status_owned", "Owned") },
            { value: "weekly_free", label: tText("heroes.status_weekly_free", "Weekly Free") },
            { value: "locked", label: tText("heroes.status_locked", "Locked") },
            { value: "in_progress", label: tText("heroes.status_with_progress", "With progress") },
            { value: "almost_milestone", label: tText("heroes.status_almost_milestone", "Near milestone") },
        ],
    });
}

function renderHeroesSortDropdown() {
    return SOW_renderDropdown({
        key: "heroes-sort",
        label: SOW_t("heroes.sort_leaders"),
        name: "sort",
        icon: HEROES_SORT_ICON,
        value: menuUi.heroesSort || "alpha_asc",
        className: "sow-heroes__sort",
        options: [
            { value: "achievements_desc", label: tText("heroes.sort_achievements_desc", "Achievements (%)") },
            { value: "most_played", label: SOW_t("heroes.sort_most_played") },
            { value: "cost_desc", label: SOW_t("heroes.sort_cost_desc") },
            { value: "cost_asc", label: SOW_t("heroes.sort_cost_asc") },
            { value: "alpha_asc", label: SOW_t("heroes.sort_alpha_asc"), shortLabel: "A-Z" },
            { value: "alpha_desc", label: SOW_t("heroes.sort_alpha_desc"), shortLabel: "Z-A" },
        ],
    });
}

let lastRenderedAvailableKey: string | null = null;

function renderHeroesRoster(activeId: string, randomSelected: boolean) {
    const leaders = menuUi.state && Array.isArray(menuUi.state.leaders) ? menuUi.state.leaders : [];
    lastRenderedAvailableKey = leaders
        .filter((l) => l.available !== false)
        .map((l) => l.id)
        .sort()
        .join(",");
    const filtered = heroesRoster(leaders);
    const cards =
        renderRandomLeaderCard(randomSelected) +
        filtered
            .map(function (leader) {
                return renderHeroesCard(leader, randomSelected ? "" : activeId);
            })
            .join("");
    return (
        cards || "<p class='sow-menu__empty sow-heroes__empty'>" + esc(SOW_t("heroes.no_leaders")) + "</p>"
    );
}

export function refreshHeroesRoster(resetScroll: boolean) {
    const panel = activeScreenPanel() || root;
    const heroesRosterContainer = panel.querySelector("[data-heroes-roster]");
    if (!heroesRosterContainer) return;
    const activeHeroesId =
        menuUi.tempSelectedLeader || (menuUi.state && menuUi.state.selected_leader) || "Caesar";
    const randomSelected =
        menuUi.tempSelectedRandom ||
        (!menuUi.tempSelectedLeader && !!(menuUi.state && menuUi.state.prefer_random_leader));
    heroesRosterContainer.innerHTML = renderHeroesRoster(activeHeroesId, randomSelected);
    if (resetScroll) {
        const heroesRosterPanel = panel.querySelector(".sow-heroes__roster");
        if (heroesRosterPanel) heroesRosterPanel.scrollTop = 0;
    }
}

export function updateHeroesPreview() {
    if (currentScreen() !== "heroes") return false;
    const randomSelected =
        menuUi.tempSelectedRandom ||
        (!menuUi.tempSelectedLeader && !!(menuUi.state && menuUi.state.prefer_random_leader));
    const activeLeader = randomSelected
        ? randomLeaderOption()
        : leaderById(menuUi.tempSelectedLeader || (menuUi.state && menuUi.state.selected_leader) || "Caesar");
    const panel = activeScreenPanel() || root;
    const featured = panel.querySelector<HTMLElement>(".sow-heroes__featured");
    if (!activeLeader || !featured) return false;
    const picture = featured.querySelector("picture");
    const source = picture && picture.querySelector("source");
    const image = picture && picture.querySelector("img");
    const briefing = featured.querySelector("[data-hero-briefing]");
    const locked = activeLeader.available === false;
    featured.classList.toggle("is-locked", locked);
    featured.classList.toggle("is-motion-reduced", heroesMotionReduced());
    // The selected scene is requested by the picture; keep next-game art low priority.
    prepareLeaderArt(activeLeader, "low");
    // Panel-shape mapping — see heroesFeaturedArt(). Do not invert it.
    const featuredArt = heroesFeaturedArt(activeLeader.slug);
    const artSlug = leaderArtSlug(activeLeader);
    const targetLeader = randomSelected ? null : activeLeader;
    if (briefing)
        briefing.innerHTML = renderHeroBriefing(
            randomSelected ? "random" : activeLeader,
            "selection",
            "sow-heroes-selected",
            renderHeroStatusTags(targetLeader, true),
        );
    if (image) {
        image.alt = randomSelected ? randomLeaderName() : leaderDisplayName(activeLeader);
        const leaderId = randomSelected ? "random" : activeLeader.id;
        if (image.dataset.heroArtSlug !== artSlug) {
            const previousLeaderId = image.dataset.heroLeaderId || "";
            const direction = heroesLeaderDirection(previousLeaderId, leaderId);
            clearHeroesFeaturedAnimation(featured);
            const pictureElement = image.closest("picture");
            if (pictureElement) void pictureElement.offsetWidth;
            setHeroesFeaturedPlaceholder(featured, artSlug);
            image.dataset.heroLeaderId = leaderId;
            image.dataset.heroArtSlug = artSlug;
            image.dataset.heroDirection = String(direction);
            const loadArt = function () {
                if (!featured.isConnected) return;
                featured.classList.remove("is-unavailable");
                featured.classList.add("is-loading");
                if (source) source.srcset = featuredArt.band;
                image.src = featuredArt.column;
                const fallbackAvatar = featured.querySelector<HTMLImageElement>("[data-hero-art-avatar]");
                if (fallbackAvatar) fallbackAvatar.hidden = false;
                setHeroesAvatar(fallbackAvatar, artSlug, "calm");
                if (image.complete) {
                    const settle = function () {
                        setFeaturedArtAvailability(image, image.naturalWidth === 0);
                    };
                    if (typeof window.requestAnimationFrame === "function")
                        window.requestAnimationFrame(settle);
                    else settle();
                }
            };
            if (previousLeaderId && !heroesMotionReduced()) {
                featured.classList.add("is-transitioning");
                featured.dataset.heroDirection = String(direction);
            } else {
                featured.classList.remove("is-transitioning");
            }
            loadArt();
        } else {
            image.dataset.heroLeaderId = randomSelected ? "random" : activeLeader.id;
        }
    }
    const tags = featured.querySelector<HTMLElement>("[data-hero-tags]");
    const rarityNode = featured.querySelector<HTMLElement>("[data-hero-rarity]");
    const status = featured.querySelector<HTMLElement>("[data-hero-status]");
    const rarityLabel = targetLeader ? heroesRarityLabel(targetLeader) : "";
    const statusLabel = heroesStatusLabel(targetLeader);
    if (rarityNode) {
        rarityNode.textContent = rarityLabel;
        rarityNode.hidden = !rarityLabel;
        rarityNode.dataset.rarity = targetLeader && targetLeader.rarity ? targetLeader.rarity : "";
    }
    if (status) {
        const statusLabelNode = status.querySelector("[data-hero-status-label]");
        if (statusLabelNode) statusLabelNode.textContent = statusLabel;
        else status.textContent = statusLabel;
        status.hidden = !statusLabel;
    }
    if (tags) tags.hidden = !rarityLabel && !statusLabel;
    const achStrip = featured.querySelector<HTMLElement>("[data-hero-achievement-strip]");
    if (achStrip) {
        achStrip.innerHTML = randomSelected ? "" : renderHeroSelectionAchievementStrip(activeLeader);
        achStrip.hidden = randomSelected;
    }
    const purchase = featured.querySelector("[data-hero-purchase]");
    if (purchase) purchase.innerHTML = randomSelected ? "" : renderLeaderPurchase(activeLeader);
    const skinsContainer = featured.querySelector<HTMLElement>("[data-hero-skins]");
    if (skinsContainer) skinsContainer.innerHTML = randomSelected ? "" : renderFeaturedTopControls();
    const leaders = menuUi.state && Array.isArray(menuUi.state.leaders) ? menuUi.state.leaders : [];
    const currentAvailableKey = leaders
        .filter((l) => l.available !== false)
        .map((l) => l.id)
        .sort()
        .join(",");
    if (lastRenderedAvailableKey !== null && lastRenderedAvailableKey !== currentAvailableKey) {
        refreshHeroesRoster(false);
    }
    const cards = panel.querySelectorAll<HTMLElement>("[data-command='preview_leader']");
    for (let i = 0; i < cards.length; i++) {
        const cardLeaderId = cards[i].dataset.leaderId;
        const cardLeader = cardLeaderId ? leaderById(cardLeaderId) : null;
        const selected = cardLeaderId === activeLeader.id;
        cards[i].classList.toggle("is-selected", selected);
        if (cardLeader) {
            cards[i].classList.toggle("is-locked", cardLeader.available === false);
        }
        cards[i].setAttribute("aria-pressed", selected ? "true" : "false");
        const isHovered = typeof cards[i].matches === "function" && cards[i].matches(":hover");
        setHeroesCardAvatar(cards[i], selected ? "angry" : isHovered ? "happy" : "calm", selected || isHovered);
    }
    const randomCard = panel.querySelector("[data-command='preview_random_leader']");
    if (randomCard) {
        const selected = randomSelected;
        randomCard.classList.toggle("is-selected", selected);
        randomCard.setAttribute("aria-pressed", selected ? "true" : "false");
    }
    const info = featured.querySelector<HTMLElement>(".sow-heroes__info[data-command='open_hero_info']");
    if (info) {
        info.hidden = randomSelected;
        if (!randomSelected) info.dataset.leaderId = activeLeader.id;
        info.setAttribute(
            "aria-label",
            randomSelected ? randomLeaderName() : leaderDisplayName(activeLeader),
        );
    }
    return true;
}

// Bundles are published as gameplay/bios/<code>.json under the registry codes
// (pt-BR, zh-CN), so the lookup returns those exact codes.
const SUPPORTED_BIO_LOCALES = new Map(
    [
        "en",
        "es",
        "fr",
        "de",
        "it",
        "tr",
        "zh-CN",
        "ja",
        "ko",
        "ar",
        "ru",
        "vi",
        "id",
        "fil",
        "pt-BR",
    ].map((code) => [code.toLowerCase(), code] as const),
);

function normalizeBioLocale(locale: unknown): string {
    const raw = String(locale || "en")
        .trim()
        .toLowerCase()
        .replace(/_/g, "-");
    return SUPPORTED_BIO_LOCALES.get(raw) || "en";
}

const bioFetches = new Map<string, Promise<void>>();

export function ensureHeroBiosLoaded(localeOverride?: string): Promise<void> {
    const lang = normalizeBioLocale(
        localeOverride !== undefined
            ? localeOverride
            : typeof window.SOW_getLocale === "function"
              ? window.SOW_getLocale()
              : "en",
    );
    if (menuUi.heroBioCache[lang]) return Promise.resolve();
    const pending = bioFetches.get(lang);
    if (pending) return pending;
    menuUi.heroBioLocaleLoading = lang;
    menuUi.heroBioError = null;
    const req = fetch(asset("gameplay/bios/" + lang + ".json"))
        .then(function (response) {
            if (!response.ok) throw new Error("http_" + response.status);
            return response.json();
        })
        .then(function (data) {
            if (data && typeof data === "object") {
                menuUi.heroBioCache[lang] = data as Record<string, HeroBioEntry>;
            }
            if (menuUi.heroBioLocaleLoading === lang) menuUi.heroBioLocaleLoading = null;
            bioFetches.delete(lang);
            render();
        })
        .catch(function (err) {
            bioFetches.delete(lang);
            if (menuUi.heroBioLocaleLoading === lang) menuUi.heroBioLocaleLoading = null;
            if (lang !== "en") {
                return ensureHeroBiosLoaded("en");
            }
            menuUi.heroBioError = String(err || "error");
            render();
        });
    bioFetches.set(lang, req);
    return req;
}

function renderFeaturedTopControls() {
    return renderSkinsButton();
}

function renderBioSourceList(
    sources: { title: string; url: string }[] | undefined,
    isPoki: boolean,
): string {
    if (!Array.isArray(sources) || !sources.length) return "";
    return sources
        .map(function (source) {
            const title = esc(source.title || source.url || "");
            if (isPoki || !source.url) {
                return "<span class='sow-hero-info__bio-source-text'>" + title + "</span>";
            }
            return (
                "<a href='" +
                esc(source.url) +
                "' target='_blank' rel='noopener noreferrer'>" +
                title +
                "</a>"
            );
        })
        .join(" · ");
}

function renderHeroCareerRecord(leader: MenuLeader): string {
    const summary = computeHeroAchievementSummary(leader);
    const matches = summary.matchesPlayed;
    const wins = summary.wins;
    const winRate = matches > 0 ? Math.round((wins / matches) * 100) + "%" : "0%";
    const items = [
        [SOW_t("heroes.stat_matches"), matches],
        [SOW_t("heroes.stat_wins"), wins],
        [tText("heroes.stat_win_rate", "Win Rate"), winRate],
        [SOW_t("heroes.stat_kills"), summary.kills],
        [SOW_t("heroes.stat_deaths"), summary.deaths],
        [SOW_t("heroes.stat_xp"), summary.xp],
    ];
    const next = summary.nextAchievement;
    const nextLabel = next
        ? next.localizedTitle + " (" + next.progress + "/" + next.target + " · +" + next.points + ")"
        : tText("heroes.all_hero_achievements_done", "All {total} hero achievements completed!", {
              total: summary.totalCount,
          });
    const nextPct = next ? next.percent : 100;

    return (
        "<section class='sow-hero-info__career' aria-label='" +
        esc(SOW_t("heroes.career_record")) +
        "'><h3>" +
        esc(SOW_t("heroes.career_record")) +
        "</h3><div class='sow-hero-info__stats-grid'>" +
        items
            .map(function (entry) {
                return (
                    "<div class='sow-hero-info__stat'><span>" +
                    esc(entry[0]) +
                    "</span><strong>" +
                    esc(entry[1]) +
                    "</strong></div>"
                );
            })
            .join("") +
        "</div>" +
        "<div class='sow-hero-info__ach-preview'>" +
        "<div class='sow-hero-info__ach-preview-top'>" +
        "<div>" +
        "<span class='sow-hero-info__ach-preview-kicker'>" +
        esc(tText("heroes.hero_achievements", "Hero Achievements")) +
        " · " +
        esc(summary.unlockedCount + "/" + summary.totalCount) +
        "</span>" +
        "<strong>" +
        esc(nextLabel) +
        "</strong>" +
        "</div>" +
        "<button type='button' class='sow-hero-info__ach-jump' data-command='switch_hero_info_tab' data-hero-info-tab='achievements'>" +
        esc(
            tText("heroes.view_hero_achievements", "View all {total} achievements", {
                total: summary.totalCount,
            }),
        ) +
        "</button>" +
        "</div>" +
        "<div class='sow-profile__achievement-track'><i style='width:" +
        esc(nextPct) +
        "%'></i></div>" +
        "</div>" +
        "</section>"
    );
}

function renderHeroBiographyTab(leader: MenuLeader): string {
    const lang = normalizeBioLocale(
        typeof window.SOW_getLocale === "function" ? window.SOW_getLocale() : "en",
    );
    // Bundles are keyed by catalog id (`slug`, catherine_the_great); `id` is the
    // wire id (CatherineTheGreat) that messages and profiles use.
    const bio =
        (menuUi.heroBioCache[lang] && menuUi.heroBioCache[lang][leader.slug]) ||
        (menuUi.heroBioCache.en && menuUi.heroBioCache.en[leader.slug]);
    if (!bio) {
        if (menuUi.heroBioLocaleLoading === lang || bioFetches.has(lang)) {
            return (
                "<div class='sow-hero-info__bio-status'>" + esc(SOW_t("heroes.bio_loading")) + "</div>"
            );
        }
        // A bundle that already loaded has no entry to wait for.
        if (menuUi.heroBioError || menuUi.heroBioCache[lang]) {
            return (
                "<div class='sow-hero-info__bio-status is-error'>" +
                esc(SOW_t("heroes.bio_unavailable")) +
                "</div>"
            );
        }
        void ensureHeroBiosLoaded(lang);
        return "<div class='sow-hero-info__bio-status'>" + esc(SOW_t("heroes.bio_loading")) + "</div>";
    }
    const isPoki = window.SOW_PORTAL === "poki";
    const bioLang = bio.lang || lang;
    let html = "<div class='sow-hero-info__bio' lang='" + esc(bioLang) + "' dir='auto'>";
    if (bio.isSaga && bio.highlight) {
        html +=
            "<section class='sow-hero-info__bio-highlight'><h4>" +
            esc(bio.highlightLabel || SOW_t("heroes.bio_highlights")) +
            "</h4><p>" +
            esc(bio.highlight) +
            "</p></section>";
    } else if (Array.isArray(bio.intro) && bio.intro.length) {
        html +=
            "<section class='sow-hero-info__bio-intro'>" +
            bio.intro
                .map(function (p) {
                    return "<p>" + esc(p) + "</p>";
                })
                .join("") +
            "</section>";
    }
    if (Array.isArray(bio.sections)) {
        html += bio.sections
            .map(function (sec) {
                const body = (sec.paragraphs || [])
                    .map(function (p) {
                        return "<p>" + esc(p) + "</p>";
                    })
                    .join("");
                const evidence = sec.evidence
                    ? "<p class='sow-hero-info__bio-evidence'><strong>" +
                      esc(bio.evidenceLabel || "") +
                      "</strong> " +
                      esc(sec.evidence) +
                      "</p>"
                    : "";
                const chSources =
                    sec.sources && sec.sources.length
                        ? "<p class='sow-hero-info__bio-cite'>" +
                          esc(bio.chapterSourcesLabel || SOW_t("heroes.bio_sources")) +
                          ": " +
                          renderBioSourceList(sec.sources, isPoki) +
                          "</p>"
                        : "";
                return (
                    "<section class='sow-hero-info__bio-chapter'><h4>" +
                    esc(sec.title) +
                    "</h4>" +
                    body +
                    evidence +
                    chSources +
                    "</section>"
                );
            })
            .join("");
    }
    if (bio.isSaga && bio.sourceNote) {
        html += "<p class='sow-hero-info__bio-sources'>" + esc(bio.sourceNote) + "</p>";
    } else if (bio.sources && bio.sources.length) {
        html +=
            "<p class='sow-hero-info__bio-sources'><strong>" +
            esc(SOW_t("heroes.bio_sources")) +
            ":</strong> " +
            renderBioSourceList(bio.sources, isPoki) +
            "</p>";
    } else if (bio.url || bio.title) {
        html +=
            "<p class='sow-hero-info__bio-sources'><strong>" +
            esc(SOW_t("heroes.bio_sources")) +
            ":</strong> " +
            (isPoki || !bio.url
                ? "<span class='sow-hero-info__bio-source-text'>" +
                  esc(bio.title || bio.url || "") +
                  " (CC BY-SA 4.0)</span>"
                : "<a href='" +
                  esc(bio.url) +
                  "' target='_blank' rel='noopener noreferrer'>" +
                  esc(bio.title || bio.url) +
                  "</a> · <a href='https://creativecommons.org/licenses/by-sa/4.0/' target='_blank' rel='noopener noreferrer'>CC BY-SA 4.0</a>") +
            "</p>";
    }
    html += "</div>";
    return html;
}

export function renderHeroInfoModal() {
    if (!menuUi.heroInfoOpen) return "";
    if (
        !menuUi.heroInfoLeaderId &&
        (menuUi.tempSelectedRandom ||
            (!menuUi.tempSelectedLeader && menuUi.state && menuUi.state.prefer_random_leader))
    )
        return "";
    const activeLeader = leaderById(
        menuUi.heroInfoLeaderId ||
            menuUi.tempSelectedLeader ||
            (menuUi.state && menuUi.state.selected_leader) ||
            "Caesar",
    );
    if (!activeLeader) return "";
    const tab =
        menuUi.heroInfoTab === "biography" || menuUi.heroInfoTab === "achievements"
            ? menuUi.heroInfoTab
            : "overview";
    const achSummary = computeHeroAchievementSummary(activeLeader);
    // The info dialog is one tall portrait card on every device, so it
    // always uses the portrait art. Not part of the panel-shape swap.
    const artUrl = leaderArtUrl(leaderArtSlug(activeLeader), "mobile");
    return (
        "<div class='sow-menu__overlay' data-menu-overlay='hero-info'><section class='sow-menu__modal sow-hero-info sow-hero-info--dossier' role='dialog' aria-modal='true' aria-label='" +
        esc(leaderDisplayName(activeLeader)) +
        "'>" +
        "<div class='sow-hero-info__banner'>" +
        "<div class='sow-hero-info__art' style=\"background-image:url('" +
        esc(artUrl) +
        "')\"></div>" +
        "<button class='sow-menu__icon-button sow-hero-info__close' type='button' data-command='close_hero_info' aria-label='" +
        esc(SOW_t("menu.close")) +
        "'>×</button>" +
        "<div class='sow-hero-info__banner-copy'>" +
        renderHeroStatusTags(activeLeader, false) +
        "<h2>" +
        esc(leaderDisplayName(activeLeader)) +
        "</h2><p>" +
        esc(leaderCivilization(activeLeader)) +
        "</p></div></div>" +
        "<div class='sow-hero-info__tabs' role='tablist'>" +
        "<button type='button' role='tab' aria-selected='" +
        (tab === "overview") +
        "' class='sow-hero-info__tab" +
        (tab === "overview" ? " is-active" : "") +
        "' data-command='switch_hero_info_tab' data-hero-info-tab='overview'>" +
        esc(SOW_t("heroes.tab_overview")) +
        "</button>" +
        "<button type='button' role='tab' aria-selected='" +
        (tab === "biography") +
        "' class='sow-hero-info__tab" +
        (tab === "biography" ? " is-active" : "") +
        "' data-command='switch_hero_info_tab' data-hero-info-tab='biography'>" +
        esc(SOW_t("heroes.tab_biography")) +
        "</button>" +
        "<button type='button' role='tab' aria-selected='" +
        (tab === "achievements") +
        "' class='sow-hero-info__tab" +
        (tab === "achievements" ? " is-active" : "") +
        "' data-command='switch_hero_info_tab' data-hero-info-tab='achievements'>" +
        esc(tText("heroes.tab_achievements", "Achievements")) +
        "<b class='sow-hero-info__tab-count'>" +
        esc(achSummary.unlockedCount + "/" + achSummary.totalCount) +
        "</b>" +
        "</button>" +
        "</div>" +
        "<div class='sow-hero-info__body'>" +
        (tab === "biography"
            ? renderHeroBiographyTab(activeLeader)
            : tab === "achievements"
              ? renderHeroDossierAchievementsTab(activeLeader, {
                    category: menuUi.heroInfoAchCategory,
                    status: menuUi.heroInfoAchStatus,
                    sort: menuUi.heroInfoAchSort,
                })
              : "<div class='sow-hero-info__overview'>" +
                renderHeroBriefing(activeLeader, "modal", "sow-hero-info-selected") +
                renderHeroCareerRecord(activeLeader) +
                "</div>") +
        "</div>" +
        "</section></div>"
    );
}

export function syncDropdowns(focusTarget?: HTMLElement | null) {
    const scopes = [];
    const panel = activeScreenPanel();
    if (panel) scopes.push(panel);
    root.querySelectorAll("[data-menu-overlay]").forEach(function (overlay) {
        scopes.push(overlay);
    });
    if (!scopes.length) scopes.push(root);
    for (let s = 0; s < scopes.length; s++) syncDropdownScope(scopes[s], menuUi.dropdownOpenKey);
    if (focusTarget) focusTarget.focus();
}

export function setDropdownOpen(key: string, open: boolean, focusTarget?: HTMLElement | null) {
    menuUi.dropdownOpenKey = open ? key : null;
    syncDropdowns(focusTarget);
}

window.addEventListener("resize", function () {
    if (menuUi.dropdownOpenKey) syncDropdowns();
});

document.addEventListener(
    "scroll",
    function () {
        if (menuUi.dropdownOpenKey) syncDropdowns();
    },
    true,
);

export function selectDropdown(key: string, value: string) {
    const panel = activeScreenPanel();
    let dropdown = panel && panel.querySelector("[data-control-dropdown][data-dropdown-key='" + key + "']");
    if (!dropdown)
        dropdown = root.querySelector(
            "[data-menu-overlay] [data-control-dropdown][data-dropdown-key='" + key + "']",
        );
    if (!dropdown) return;
    const input = dropdown.querySelector<HTMLInputElement>("[data-dropdown-input]");
    if (!input) return;
    input.value = value;
    menuUi.dropdownOpenKey = null;
    if (key === "heroes-region") {
        menuUi.heroesRegionFilter = value || "all";
        dropdown.classList.toggle("is-filtered", menuUi.heroesRegionFilter !== "all");
        syncDropdowns();
        refreshHeroesRoster(true);
        return;
    }
    if (key === "heroes-rarity") {
        menuUi.heroesRarityFilter = value || "all";
        dropdown.classList.toggle("is-filtered", menuUi.heroesRarityFilter !== "all");
        syncDropdowns();
        refreshHeroesRoster(true);
        return;
    }
    if (key === "heroes-status") {
        menuUi.heroesStatusFilter = value || "all";
        if (menuUi.heroesStatusFilter === "locked" && menuUi.heroesPlayableOnly) {
            menuUi.heroesPlayableOnly = false;
            const playableBtn = panel?.querySelector<HTMLElement>("[data-command='toggle_heroes_playable']");
            if (playableBtn) {
                playableBtn.classList.remove("is-active");
                playableBtn.setAttribute("aria-pressed", "false");
            }
        }
        dropdown.classList.toggle("is-filtered", menuUi.heroesStatusFilter !== "all");
        syncDropdowns();
        refreshHeroesRoster(true);
        return;
    }
    if (key === "heroes-sort") {
        menuUi.heroesSort = value || "alpha_asc";
        syncDropdowns();
        refreshHeroesRoster(true);
        return;
    }
    if (key === "ach-category") {
        menuUi.profileAchievementFilter = value || "summary";
        menuUi.profileAchievementSubcategory = "all";
        syncDropdowns();
        input.dispatchEvent(new Event("change", { bubbles: true }));
        return;
    }
    if (key === "ach-status") {
        menuUi.profileAchievementStatus = value || "all";
        dropdown.classList.toggle("is-filtered", menuUi.profileAchievementStatus !== "all");
        syncDropdowns();
        input.dispatchEvent(new Event("change", { bubbles: true }));
        return;
    }
    if (key === "ach-tier") {
        menuUi.profileAchievementTier = value || "all";
        dropdown.classList.toggle("is-filtered", menuUi.profileAchievementTier !== "all");
        syncDropdowns();
        input.dispatchEvent(new Event("change", { bubbles: true }));
        return;
    }
    if (key === "ach-sort") {
        menuUi.profileAchievementSort = value || "smart";
        syncDropdowns();
        input.dispatchEvent(new Event("change", { bubbles: true }));
        return;
    }
    syncDropdowns();
    input.dispatchEvent(new Event("change", { bubbles: true }));
}

function renderHeroesArtLoader(avatarSlug: string) {
    return renderShowcaseLoader("showcase-loader-arc-art")
        .replace("class='showcase-loader'", "class='showcase-loader' data-hero-art-loader")
        .replace(
            /<\/div>$/,
            "<img class='showcase-portrait' data-hero-art-avatar " +
                avatarImageAttribute(heroesAvatarUrl(avatarSlug, "calm")) +
                " alt='' width='256' height='256' decoding='async'></div>",
        );
}

export function renderHeroes() {
    const activeId = menuUi.tempSelectedLeader || (menuUi.state && menuUi.state.selected_leader) || "Caesar";
    const activeRandom =
        menuUi.tempSelectedRandom ||
        (!menuUi.tempSelectedLeader && !!(menuUi.state && menuUi.state.prefer_random_leader));
    const activeLeader = activeRandom ? randomLeaderOption() : leaderById(activeId);
    const locked = activeLeader.available === false;
    const artSlug = leaderArtSlug(activeLeader);
    const motionReduced = heroesMotionReduced();
    // Keep next-game art available without competing with the featured image.
    prepareLeaderArt(activeLeader, "low");
    // Panel-shape mapping — see heroesFeaturedArt(). Do not invert it.
    // The <img> fallback is the tall column art, so width='1080'
    // height='1920' below are correct on purpose.
    const featuredArt = heroesFeaturedArt(activeLeader.slug);
    const displayName = activeRandom ? randomLeaderName() : leaderDisplayName(activeLeader);
    const allLeaders = menuUi.state && Array.isArray(menuUi.state.leaders) ? menuUi.state.leaders : [];
    const ownedCount = allLeaders.filter(function (l) {
        return l.owned === true;
    }).length;
    return (
        "<main class='sow-menu__main sow-menu__main--heroes' data-screen-panel='heroes'><section class='sow-menu__heroes-slot' aria-label='" +
        esc(SOW_t("menu.heroes")) +
        "'>" +
        "<div class='sow-heroes__workspace'>" +
        "<section class='sow-heroes__featured is-loading" +
        (locked ? " is-locked" : "") +
        (motionReduced ? " is-motion-reduced" : "") +
        "' aria-labelledby='sow-heroes-selected'>" +
        "<div class='sow-heroes__featured-skins' data-hero-skins>" +
        (activeRandom ? "" : renderFeaturedTopControls()) +
        "</div>" +
        "<button class='sow-menu__icon-button sow-heroes__info' type='button' data-command='open_hero_info' data-leader-id='" +
        esc(activeRandom ? "" : activeLeader.id) +
        "' aria-label='" +
        esc(displayName) +
        "'" +
        (activeRandom ? " hidden" : "") +
        ">i</button><picture><source media='(max-width: 900px), (orientation: portrait)' srcset='" +
        esc(featuredArt.band) +
        "'><img src='" +
        esc(featuredArt.column) +
        "' alt='" +
        esc(displayName) +
        "' width='1080' height='1920' data-hero-featured-art data-hero-leader-id='" +
        esc(activeRandom ? "random" : activeLeader.id) +
        "' data-hero-art-slug='" +
        esc(artSlug) +
        "' fetchpriority='high'></picture>" +
        renderHeroesArtLoader(artSlug) +
        "<div class='sow-heroes__featured-copy'>" +
        "<div data-hero-briefing>" +
        renderHeroBriefing(
            activeRandom ? "random" : activeLeader,
            "selection",
            "sow-heroes-selected",
            renderHeroStatusTags(activeRandom ? null : activeLeader, true),
        ) +
        "</div><div data-hero-achievement-strip" +
        (activeRandom ? " hidden" : "") +
        ">" +
        (activeRandom ? "" : renderHeroSelectionAchievementStrip(activeLeader)) +
        "</div><div class='sow-heroes__actions'><div data-hero-purchase>" +
        (activeRandom ? "" : renderLeaderPurchase(activeLeader)) +
        "</div>" +
        "</div></div></section>" +
        "<section class='sow-heroes__roster' aria-label='" +
        esc(SOW_t("heroes.leader_list")) +
        "'><div class='sow-heroes__section-head'><div class='sow-heroes__roster-meta'>" +
        "<span class='sow-heroes__owned-badge' data-heroes-owned-count>" +
        esc(SOW_t("heroes.owned_progress", { owned: ownedCount, total: allLeaders.length })) +
        "</span>" +
        "<button type='button' class='sow-heroes__playable-btn" +
        (menuUi.heroesPlayableOnly ? " is-active" : "") +
        "' data-command='toggle_heroes_playable' aria-pressed='" +
        Boolean(menuUi.heroesPlayableOnly) +
        "'>" +
        esc(SOW_t("heroes.filter_playable")) +
        "</button></div><div class='sow-heroes__filters'><label class='sow-heroes__search'><span class='sow-heroes__sr-only'>" +
        esc(SOW_t("heroes.search_leaders")) +
        "</span><span class='sow-heroes__search-icon' aria-hidden='true'><svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='11' cy='11' r='8'/><line x1='21' y1='21' x2='16.65' y2='16.65'/></svg></span><input data-role='heroes-search' type='search' placeholder='" +
        esc(SOW_t("heroes.search_leader_civilization")) +
        "' value=\"" +
        esc(menuUi.heroesSearchQuery) +
        "\" autocomplete='off' spellcheck='false'></label>" +
        renderHeroesRegionDropdown() +
        renderHeroesRarityDropdown() +
        renderHeroesStatusDropdown() +
        renderHeroesSortDropdown() +
        "</div></div><div class='sow-heroes__grid' data-heroes-roster aria-live='polite'>" +
        renderHeroesRoster(activeId, activeRandom) +
        "</div></section>" +
        "</div>" +
        "</section></main>"
    );
}
