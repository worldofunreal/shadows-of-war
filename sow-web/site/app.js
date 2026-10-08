(() => {
  const $ = selector => document.querySelector(selector);
  const $$ = selector => [...document.querySelectorAll(selector)];
  const siteText = (key, fallback, values) => {
    if (typeof window.SOW_t !== 'function') return fallback;
    const value = window.SOW_t(key, values);
    return value === `[${key}]` ? fallback : value;
  };

  const heroVideo = $('.hero-video');
  if (heroVideo) {
    const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
    const heroSection = heroVideo.closest('.hero');
    let heroVisible = true;
    function syncHeroVideo() {
      if (prefersReducedMotion.matches || document.visibilityState === 'hidden' || !heroVisible) {
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
    prefersReducedMotion.addEventListener?.('change', syncHeroVideo);
    document.addEventListener('visibilitychange', syncHeroVideo);
    syncHeroVideo();
  }

  const cards = $$('.leader-card[data-leader-id]');
  const leaders = cards.map(c => ({
    id: c.dataset.leaderId,
    name: c.dataset.name,
    civ: c.dataset.civ,
    nameKey: `heroes.leader_${c.dataset.leaderId}_name`,
    historicalKey: `heroes.leader_${c.dataset.leaderId}_historical`,
    civKey: c.dataset.civKey,
    code: c.dataset.code,
    abilityKey: c.dataset.abilityKey,
    descriptionKey: c.dataset.descriptionKey,
    image: c.dataset.image
  }));

  const asset = (leader, mobile = false) => `/assets/shell/leaders/${leader.image}_${mobile ? 'mobile' : 'desktop'}.webp`;
  let activeLeaderIndex = 0;
  const leaderName = leader => siteText(leader.nameKey, leader.name);
  const leaderCiv = leader => siteText(leader.civKey, leader.civ);
  const leaderHistorical = leader => {
    const value = siteText(leader.historicalKey, '');
    return value === leaderName(leader) ? '' : value;
  };

  function updateLeader(index) {
    const leader = leaders[index];
    if (!leader) return;
    activeLeaderIndex = index;
    const name = leaderName(leader);
    const civ = leaderCiv(leader);
    const historical = leaderHistorical(leader);

    const detailImage = $('[data-detail-image]');
    if (detailImage) {
      detailImage.src = asset(leader);
      detailImage.alt = siteText('site.leader_artwork', `${name} artwork`, { name });
    }
    const detailCode = $('[data-detail-code]');
    if (detailCode) detailCode.textContent = `${leader.code} · ${String(index + 1).padStart(2, '0')}`;
    const detailName = $('[data-detail-name]');
    if (detailName) detailName.textContent = name;
    const detailCiv = $('[data-detail-civ]');
    if (detailCiv) detailCiv.textContent = civ;
    const detailHistorical = $('[data-detail-historical]');
    if (detailHistorical) {
      detailHistorical.textContent = historical;
      detailHistorical.hidden = !historical;
    }
    const detailAbility = $('[data-detail-ability]');
    if (detailAbility) detailAbility.textContent = siteText(leader.abilityKey, '', {});
    const detailDesc = $('[data-detail-description]');
    if (detailDesc) {
      detailDesc.textContent = siteText(leader.descriptionKey, '', {});
    }

    cards.forEach((item, itemIndex) => {
      const active = itemIndex === index;
      const itemLeader = leaders[itemIndex];
      item.classList.toggle('is-active', active);
      item.setAttribute('aria-pressed', String(active));
      item.setAttribute('aria-label', siteText('site.inspect_leader', `Inspect ${leaderName(itemLeader)}`, { name: leaderName(itemLeader) }));
      const cardName = item.querySelector('.leader-card-info b');
      const cardCiv = item.querySelector('.leader-card-info span');
      const cardImage = item.querySelector('img');
      if (cardName) cardName.textContent = leaderName(itemLeader);
      if (cardCiv) cardCiv.textContent = leaderCiv(itemLeader);
      if (cardImage) cardImage.alt = siteText('site.leader_artwork', `${leaderName(itemLeader)} artwork`, { name: leaderName(itemLeader) });
    });
  }

  function bindLeaderGrid() {
    cards.forEach((card, index) => {
      card.addEventListener('click', () => {
        updateLeader(index);
        $('[data-leader-detail]')?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      });
    });
  }

  bindLeaderGrid();
  if (leaders.length) {
    updateLeader(0);
  }
  window.addEventListener('sow:locale-change', () => updateLeader(activeLeaderIndex));
  if (window.SOW_I18N_READY && typeof window.SOW_I18N_READY.then === 'function') {
    window.SOW_I18N_READY.then(() => updateLeader(activeLeaderIndex)).catch(() => {});
  }
})();
