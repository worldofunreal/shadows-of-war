(() => {
  const $ = (selector) => document.querySelector(selector);
  const $$ = (selector) => [...document.querySelectorAll(selector)];
  const siteText = (key, fallback, values) => {
    if (typeof window.SOW_t !== "function") return fallback;
    const value = window.SOW_t(key, values);
    return value === `[${key}]` ? fallback : value;
  };
  const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

  const heroVideo = $(".hero-video");
  if (heroVideo) {
    const heroSection = heroVideo.closest(".hero");
    let heroVisible = true;
    function syncHeroVideo() {
      if (prefersReducedMotion.matches || document.visibilityState === "hidden" || !heroVisible) {
        heroVideo.pause();
        return;
      }
      heroVideo.play()?.catch(() => {});
    }
    if (heroSection && window.IntersectionObserver) {
      heroVisible = false;
      new IntersectionObserver(([entry]) => {
        heroVisible = entry.isIntersecting;
        syncHeroVideo();
      }).observe(heroSection);
    }
    prefersReducedMotion.addEventListener?.("change", syncHeroVideo);
    document.addEventListener("visibilitychange", syncHeroVideo);
    syncHeroVideo();
  }

  // The roster is built from the character catalog when the site is published
  // (sow-dist `landing_leaders`); each card carries its English texts as fallback.
  const cards = $$(".roster-card[data-leader-id]");
  const leaders = cards.map((card) => ({
    id: card.dataset.leaderId,
    artSlug: card.dataset.artSlug || card.dataset.leaderId,
    name: card.dataset.name,
    civilization: card.dataset.civ,
    historical: card.dataset.historical,
    ability: card.dataset.ability,
    description: card.dataset.description,
  }));
  const textKey = (leader, field) =>
    field === "ability" || field === "description"
      ? `site.leader_${leader.id}_${field}`
      : `heroes.leader_${leader.id}_${field}`;
  const leaderText = (leader, field) => siteText(textKey(leader, field), leader[field]);
  let activeLeaderIndex = 0;

  // The key moves with the hero so a later language switch translates the right one.
  function showText(element, leader, field) {
    if (!element) return "";
    element.setAttribute("data-i18n", textKey(leader, field));
    element.textContent = leaderText(leader, field);
    return element.textContent;
  }

  function updateLeader(index, animate = false) {
    const leader = leaders[index];
    if (!leader) return;
    activeLeaderIndex = index;
    const name = showText($("[data-detail-name]"), leader, "name");
    const image = $("[data-detail-image]");
    if (image) {
      image.src = `/assets/shell/leaders/${leader.artSlug}_desktop.webp`;
      image.alt = name;
      if (animate && !prefersReducedMotion.matches) {
        image.animate?.([{ opacity: 0.2 }, { opacity: 1 }], { duration: 320, easing: "ease-out" });
      }
    }
    const mobileImage = $("[data-detail-image-mobile]");
    if (mobileImage) mobileImage.srcset = `/assets/shell/leaders/${leader.artSlug}_mobile.webp`;
    showText($("[data-detail-civ]"), leader, "civilization");
    const historical = $("[data-detail-historical]");
    if (historical) historical.hidden = showText(historical, leader, "historical") === name;
    showText($("[data-detail-ability]"), leader, "ability");
    showText($("[data-detail-description]"), leader, "description");
    cards.forEach((card, cardIndex) => {
      const active = cardIndex === index;
      card.classList.toggle("is-active", active);
      card.setAttribute("aria-pressed", String(active));
    });
  }

  cards.forEach((card, index) => card.addEventListener("click", () => updateLeader(index, true)));
  if (leaders.length) updateLeader(0);
  window.addEventListener("sow:locale-change", () => updateLeader(activeLeaderIndex));
  if (window.SOW_I18N_READY && typeof window.SOW_I18N_READY.then === "function") {
    window.SOW_I18N_READY.then(() => updateLeader(activeLeaderIndex)).catch(() => {});
  }
})();
