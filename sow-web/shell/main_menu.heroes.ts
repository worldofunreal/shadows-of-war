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
import { activeScreenPanel } from "./main_menu.shell.ts";
import { syncDropdownScope } from "./sow-dropdown.ts";
import { avatarAtlasCell, avatarImageAttribute, loadAvatarImage } from "./avatar-presentation.ts";
import type { MenuLeader } from "./main_menu.state.ts";
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
        }).catch(() => undefined);
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

function setHeroesCardAvatar(card: HTMLElement, expression: HeroesAvatarExpression) {
    setHeroesAvatar(card.querySelector<HTMLImageElement>("img"), card.dataset.avatarSlug || "", expression);
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

function heroesStatusLabel(leader: MenuLeader) {
    if (!leader) return "";
    const rarity = heroesRarityLabel(leader);
    if (leader.id === "Boudica") return rarity;
    const status = leader.permanently_free
        ? SOW_t("heroes.always_free")
        : isWeeklyFreeRotation(leader)
          ? SOW_t("heroes.weekly_free_rotation")
          : "";
    return [rarity, status].filter(Boolean).join(" · ");
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
// This function is the ONLY owner of the mapping. Guarded by the test
// "heroes featured art follows the panel shape, not the device name"
// (menu_flow.test.ts, runs in the ./sow p preflight) and by
// AGENTS.md -> "Heroes featured art (owner decision — do not flip)".
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
        (rarity ? "<em>" + esc(rarity) + "</em>" : "") +
        "</span>" +
        "</button>"
    );
}

function randomLeaderName() {
    return SOW_t("heroes.random_leader");
}

function randomLeaderOption() {
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
    if (!matchesSearch || menuUi.heroesRegionFilter !== "all") return "";
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
        return matchesQuery && matchesRegion;
    });
    return filtered
        .filter(function (leader) {
            return leader.available !== false;
        })
        .concat(
            filtered.filter(function (leader) {
                return leader.available === false;
            }),
        );
}

function renderHeroesRegionDropdown() {
    return SOW_renderDropdown({
        key: "heroes-region",
        label: SOW_t("heroes.filter_leaders_region"),
        name: "region",
        value: menuUi.heroesRegionFilter,
        className: "sow-heroes__region",
        options: [
            { value: "all", label: SOW_t("heroes.all_regions") },
            { value: "europe", label: SOW_t("heroes.europe") },
            { value: "africa", label: SOW_t("heroes.africa") },
            { value: "asia", label: SOW_t("heroes.asia") },
            { value: "americas", label: SOW_t("heroes.americas") },
        ],
    });
}

function renderHeroesRoster(activeId: string, randomSelected: boolean) {
    const leaders = menuUi.state && Array.isArray(menuUi.state.leaders) ? menuUi.state.leaders : [];
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
    const status = featured.querySelector<HTMLElement>("[data-hero-status]");
    const locked = activeLeader.available === false;
    featured.classList.toggle("is-locked", locked);
    featured.classList.toggle("is-motion-reduced", heroesMotionReduced());
    // The selected scene is requested by the picture; keep next-game art low priority.
    prepareLeaderArt(activeLeader, "low");
    // Panel-shape mapping — see heroesFeaturedArt(). Do not invert it.
    const featuredArt = heroesFeaturedArt(activeLeader.slug);
    const artSlug = leaderArtSlug(activeLeader);
    if (briefing)
        briefing.innerHTML = renderHeroBriefing(
            randomSelected ? "random" : activeLeader,
            "selection",
            "sow-heroes-selected",
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
    if (status) {
        const statusLabel = randomSelected ? "" : heroesStatusLabel(activeLeader);
        const statusLabelNode = status.querySelector("[data-hero-status-label]");
        if (statusLabelNode) statusLabelNode.textContent = statusLabel;
        else status.textContent = statusLabel;
        status.hidden = !statusLabel;
    }
    const purchase = featured.querySelector("[data-hero-purchase]");
    if (purchase) purchase.innerHTML = randomSelected ? "" : renderLeaderPurchase(activeLeader);
    const cards = panel.querySelectorAll<HTMLElement>("[data-command='preview_leader']");
    for (let i = 0; i < cards.length; i++) {
        const cardLeaderId = cards[i].dataset.leaderId;
        const selected = cardLeaderId === activeLeader.id;
        cards[i].classList.toggle("is-selected", selected);
        cards[i].setAttribute("aria-pressed", selected ? "true" : "false");
        setHeroesCardAvatar(cards[i], selected ? "angry" : "calm");
    }
    const randomCard = panel.querySelector("[data-command='preview_random_leader']");
    if (randomCard) {
        const selected = randomSelected;
        randomCard.classList.toggle("is-selected", selected);
        randomCard.setAttribute("aria-pressed", selected ? "true" : "false");
    }
    const info = featured.querySelector<HTMLElement>("[data-command='open_hero_info']");
    if (info) {
        info.hidden = randomSelected;
        info.setAttribute(
            "aria-label",
            randomSelected ? randomLeaderName() : leaderDisplayName(activeLeader),
        );
    }
    return true;
}

export function renderHeroInfoModal() {
    if (!menuUi.heroInfoOpen) return "";
    if (
        menuUi.tempSelectedRandom ||
        (!menuUi.tempSelectedLeader && menuUi.state && menuUi.state.prefer_random_leader)
    )
        return "";
    const activeLeader = leaderById(
        menuUi.tempSelectedLeader || (menuUi.state && menuUi.state.selected_leader) || "Caesar",
    );
    if (!activeLeader) return "";
    // The info dialog is one tall portrait card on every device, so it
    // always uses the portrait art. Not part of the panel-shape swap.
    const artUrl = leaderArtUrl(leaderArtSlug(activeLeader), "mobile");
    return (
        "<div class='sow-menu__overlay' data-menu-overlay='hero-info'><section class='sow-menu__modal sow-hero-info' role='dialog' aria-modal='true' aria-label='" +
        esc(leaderDisplayName(activeLeader)) +
        "'>" +
        "<div class='sow-hero-info__art' style=\"background-image:url('" +
        esc(artUrl) +
        "')\">" +
        "<button class='sow-menu__icon-button' type='button' data-command='close_hero_info' aria-label='" +
        esc(SOW_t("menu.close")) +
        "'>×</button></div>" +
        "<div class='sow-hero-info__foot'>" +
        renderHeroBriefing(activeLeader, "modal", "sow-hero-info-selected") +
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
        syncDropdowns();
        refreshHeroesRoster(true);
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
    const statusLabel = activeRandom ? "" : heroesStatusLabel(activeLeader);
    return (
        "<main class='sow-menu__main sow-menu__main--heroes' data-screen-panel='heroes'><section class='sow-menu__heroes-slot' aria-label='" +
        esc(SOW_t("menu.heroes")) +
        "'>" +
        "<div class='sow-heroes__workspace'>" +
        "<section class='sow-heroes__featured is-loading" +
        (locked ? " is-locked" : "") +
        (motionReduced ? " is-motion-reduced" : "") +
        "' aria-labelledby='sow-heroes-selected'><button class='sow-menu__icon-button sow-heroes__info' type='button' data-command='open_hero_info' aria-label='" +
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
        "<div class='sow-heroes__featured-copy'><p class='sow-heroes__rotation' data-hero-status" +
        (!statusLabel ? " hidden" : "") +
        "><span data-hero-status-label>" +
        esc(statusLabel) +
        "</span></p><div data-hero-briefing>" +
        renderHeroBriefing(activeRandom ? "random" : activeLeader, "selection", "sow-heroes-selected") +
        "</div><div class='sow-heroes__actions'><div data-hero-purchase>" +
        (activeRandom ? "" : renderLeaderPurchase(activeLeader)) +
        "</div>" +
        "</div></div></section>" +
        "<section class='sow-heroes__roster' aria-label='" +
        esc(SOW_t("heroes.leader_list")) +
        "'><div class='sow-heroes__section-head'><div class='sow-heroes__filters'><label class='sow-heroes__search'><span class='sow-heroes__sr-only'>" +
        esc(SOW_t("heroes.search_leaders")) +
        "</span><input data-role='heroes-search' type='search' placeholder='" +
        esc(SOW_t("heroes.search_leader_civilization")) +
        "' value=\"" +
        esc(menuUi.heroesSearchQuery) +
        "\" autocomplete='off' spellcheck='false'></label>" +
        renderHeroesRegionDropdown() +
        renderSkinsButton() +
        "</div></div><div class='sow-heroes__grid' data-heroes-roster aria-live='polite'>" +
        renderHeroesRoster(activeId, activeRandom) +
        "</div></section>" +
        "</div>" +
        "</section></main>"
    );
}
