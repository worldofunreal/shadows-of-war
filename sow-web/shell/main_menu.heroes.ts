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
    leaderPerk,
    menuUi,
    prepareLeaderArt,
    renderHeroBriefing,
    root,
} from "./main_menu.core.ts";
import { activeScreenPanel } from "./main_menu.shell.ts";
import { syncDropdownScope } from "./sow-dropdown.ts";
import type { MenuLeader } from "./main_menu.state.ts";
import { renderLeaderPurchase } from "./main_menu.surfaces.ts";

// Hero selection, filtering, dropdowns, and preview updates.

let heroesRotationCountdownInterval: number | null = null;
const HEROES_ROTATION_COUNTDOWN_SECS = 4 * 24 * 60 * 60;

function isWeeklyFreeRotation(leader: MenuLeader | null) {
    return !!leader && leader.available === true && leader.free_rotation === true && leader.owned === false;
}

function heroesStatusLabel(leader: MenuLeader) {
    if (!leader || leader.id === "Boudica") return "";
    if (leader.permanently_free) return SOW_t("heroes.always_free");
    return "";
}

function rotationCountdownText(leader: MenuLeader | null, rotationEndsAt: number | null, nowSeconds: number) {
    if (!isWeeklyFreeRotation(leader)) return "";
    const remaining = Number(rotationEndsAt) - Number(nowSeconds);
    if (!Number.isFinite(remaining) || remaining <= 0 || remaining > HEROES_ROTATION_COUNTDOWN_SECS)
        return "";
    const total = Math.ceil(remaining);
    const days = Math.floor(total / 86400);
    const hours = Math.floor(total / 3600) % 24;
    const minutes = Math.floor(total / 60) % 60;
    const seconds = total % 60;
    return SOW_t("heroes.rotation_available_for", {
        days: days,
        hours: String(hours).padStart(2, "0"),
        minutes: String(minutes).padStart(2, "0"),
        seconds: String(seconds).padStart(2, "0"),
    });
}

function updateHeroesRotationCountdown() {
    const countdown = root.querySelector<HTMLElement>("[data-hero-rotation-countdown]");
    if (!countdown) return;
    const randomSelected =
        menuUi.tempSelectedRandom ||
        (!menuUi.tempSelectedLeader && menuUi.state && menuUi.state.prefer_random_leader);
    const activeLeader = randomSelected
        ? null
        : leaderById(menuUi.tempSelectedLeader || (menuUi.state && menuUi.state.selected_leader) || "Caesar");
    const endsAt = menuUi.state && menuUi.state.store && menuUi.state.store.rotation_ends_at;
    const value =
        menuUi.state && menuUi.state.phase === "MainMenu"
            ? rotationCountdownText(activeLeader, endsAt, Date.now() / 1000)
            : "";
    countdown.textContent = value;
    countdown.hidden = !value;
    const status = root.querySelector<HTMLElement>("[data-hero-status]");
    const statusLabel = status && status.querySelector<HTMLElement>("[data-hero-status-label]");
    if (statusLabel) statusLabel.hidden = !!value;
    if (status) status.hidden = !value && (!statusLabel || !statusLabel.textContent.trim());
}

export function syncHeroesRotationCountdown() {
    const shouldRun =
        !!menuUi.state &&
        menuUi.state.phase === "MainMenu" &&
        !root.hidden &&
        currentScreen() === "heroes" &&
        document.visibilityState === "visible";
    if (!shouldRun) {
        if (heroesRotationCountdownInterval !== null) {
            window.clearInterval(heroesRotationCountdownInterval);
            heroesRotationCountdownInterval = null;
        }
        return;
    }
    if (heroesRotationCountdownInterval === null) {
        heroesRotationCountdownInterval = window.setInterval(updateHeroesRotationCountdown, 1000);
    }
    updateHeroesRotationCountdown();
}

// OWNER DECISION — DO NOT "FIX" THIS MAPPING.
//
// The heroes featured art is chosen by the SHAPE OF THE PANEL, not by the
// device and not by the file name:
//   - Desktop / landscape screen: the panel is a tall narrow column
//     (".sow-heroes__workspace { grid-template-columns: minmax(320px, .8fr)
//     minmax(0, 1.2fr) }" in main_menu.profile.css), so it takes the
//     PORTRAIT art *_mobile.webp (1080x1920).
//   - Phone / portrait screen: the panel collapses into a short wide band
//     ("#sow-menu[data-screen="heroes"] .sow-heroes__featured {
//     flex: 0 0 clamp(192px, 29dvh, 236px) }"), so it takes the LANDSCAPE
//     art *_desktop.webp (1920x1080).
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
    const avatarUrl = asset("gameplay/avatars/" + leaderArtSlug(leader) + ".webp");
    return (
        "<button class='sow-heroes__card" +
        (selected ? " is-selected" : "") +
        (locked ? " is-locked" : "") +
        "' type='button' data-command='preview_leader' data-leader-id='" +
        esc(leader.id) +
        "' aria-pressed='" +
        selected +
        "'>" +
        "<span class='sow-heroes__card-art' style=\"background-image:url('" +
        esc(avatarUrl) +
        "')\"></span>" +
        "<span class='sow-heroes__card-copy'><strong>" +
        esc(leaderDisplayName(leader)) +
        "</strong><small>" +
        esc(leaderCivilization(leader)) +
        "</small><em>" +
        esc(leaderPerk(leader)) +
        "</em></span>" +
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
        "<span class='sow-heroes__card-art' style=\"background-image:url('" +
        esc(avatarUrl) +
        "')\"></span>" +
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
                return renderHeroesCard(leader, activeId);
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
    const featured = panel.querySelector(".sow-heroes__featured");
    if (!activeLeader || !featured) return false;
    const picture = featured.querySelector("picture");
    const source = picture && picture.querySelector("source");
    const image = picture && picture.querySelector("img");
    const briefing = featured.querySelector("[data-hero-briefing]");
    const status = featured.querySelector<HTMLElement>("[data-hero-status]");
    const confirm = featured.querySelector<HTMLButtonElement>(
        "[data-command='confirm_leader'], [data-command='confirm_random_leader']",
    );
    const locked = activeLeader.available === false;
    featured.classList.toggle("is-locked", locked);
    // prepareLeaderArt() warms the loader splash art (loader.js), not this
    // panel art. Leave it alone; the <picture> fetches its own file.
    prepareLeaderArt(activeLeader);
    // Panel-shape mapping — see heroesFeaturedArt(). Do not invert it.
    const featuredArt = heroesFeaturedArt(activeLeader.slug);
    if (source) source.srcset = featuredArt.band;
    if (image) {
        image.src = featuredArt.column;
        image.alt = randomSelected ? randomLeaderName() : leaderDisplayName(activeLeader);
    }
    if (briefing)
        briefing.innerHTML = renderHeroBriefing(
            randomSelected ? "random" : activeLeader,
            "selection",
            "sow-heroes-selected",
        );
    if (status) {
        const statusLabel = randomSelected ? "" : heroesStatusLabel(activeLeader);
        const statusLabelNode = status.querySelector("[data-hero-status-label]");
        if (statusLabelNode) statusLabelNode.textContent = statusLabel;
        else status.textContent = statusLabel;
        status.hidden = !statusLabel;
    }
    updateHeroesRotationCountdown();
    const purchase = featured.querySelector("[data-hero-purchase]");
    if (purchase) purchase.innerHTML = randomSelected ? "" : renderLeaderPurchase(activeLeader);
    if (confirm) {
        confirm.disabled = locked;
        confirm.setAttribute("aria-disabled", locked ? "true" : "false");
        confirm.setAttribute("data-command", randomSelected ? "confirm_random_leader" : "confirm_leader");
        confirm.dataset.leaderId = randomSelected ? "" : activeLeader.id;
        confirm.firstChild!.nodeValue =
            SOW_t(randomSelected ? "heroes.confirm_random_leader" : "heroes.confirm_leader", { name: "" }) +
            " ";
    }
    const cards = panel.querySelectorAll<HTMLElement>("[data-command='preview_leader']");
    for (let i = 0; i < cards.length; i++) {
        const selected = cards[i].dataset.leaderId === activeLeader.id;
        cards[i].classList.toggle("is-selected", selected);
        cards[i].setAttribute("aria-pressed", selected ? "true" : "false");
    }
    const randomCard = panel.querySelector("[data-command='preview_random_leader']");
    if (randomCard) {
        randomCard.classList.toggle("is-selected", randomSelected);
        randomCard.setAttribute("aria-pressed", randomSelected ? "true" : "false");
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

export function renderHeroes() {
    const activeId = menuUi.tempSelectedLeader || (menuUi.state && menuUi.state.selected_leader) || "Caesar";
    const activeRandom =
        menuUi.tempSelectedRandom ||
        (!menuUi.tempSelectedLeader && !!(menuUi.state && menuUi.state.prefer_random_leader));
    const activeLeader = activeRandom ? randomLeaderOption() : leaderById(activeId);
    const locked = activeLeader.available === false;
    // prepareLeaderArt() warms the loader splash art (loader.js), not this
    // panel art. Leave it alone; the <picture> fetches its own file.
    prepareLeaderArt(activeLeader);
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
        "<section class='sow-heroes__featured" +
        (locked ? " is-locked" : "") +
        "' aria-labelledby='sow-heroes-selected'><button class='sow-menu__icon-button sow-heroes__info' type='button' data-command='open_hero_info' aria-label='" +
        esc(displayName) +
        "'" +
        (activeRandom ? " hidden" : "") +
        ">i</button><picture><source media='(max-width: 680px), (orientation: portrait)' srcset='" +
        esc(featuredArt.band) +
        "'><img src='" +
        esc(featuredArt.column) +
        "' alt='" +
        esc(displayName) +
        "' width='1080' height='1920' fetchpriority='high'></picture><div class='sow-heroes__featured-copy'><p class='sow-heroes__rotation' data-hero-status" +
        (!statusLabel ? " hidden" : "") +
        "><span data-hero-status-label>" +
        esc(statusLabel) +
        "</span><span class='sow-heroes__rotation-countdown' data-hero-rotation-countdown hidden></span></p><div data-hero-briefing>" +
        renderHeroBriefing(activeRandom ? "random" : activeLeader, "selection", "sow-heroes-selected") +
        "</div><div class='sow-heroes__actions'><div data-hero-purchase>" +
        (activeRandom ? "" : renderLeaderPurchase(activeLeader)) +
        "</div><button class='sow-menu__primary sow-heroes__confirm' type='button' data-command='" +
        (activeRandom ? "confirm_random_leader" : "confirm_leader") +
        "' data-leader-id='" +
        (activeRandom ? "" : esc(activeLeader.id)) +
        "'" +
        (locked ? " disabled aria-disabled='true'" : "") +
        ">" +
        esc(SOW_t(activeRandom ? "heroes.confirm_random_leader" : "heroes.confirm_leader", { name: "" })) +
        " <span>✓</span></button></div></div></section>" +
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
        "</div></div><div class='sow-heroes__grid' data-heroes-roster aria-live='polite'>" +
        renderHeroesRoster(activeId, activeRandom) +
        "</div></section>" +
        "</div>" +
        "</section></main>"
    );
}
