(() => {
  const $ = selector => document.querySelector(selector);
  const $$ = selector => [...document.querySelectorAll(selector)];
  const siteText = (key, fallback, values) => {
    if (typeof window.SOW_t !== 'function') return fallback;
    const value = window.SOW_t(key, values);
    return value === `[${key}]` ? fallback : value;
  };

  // First-party, vendor-free landing funnel telemetry. The game shell owns
  // gameplay events; this page only measures entry and the Play now CTA.
  const siteSessionId = (() => {
    try {
      const key = 'sow_site_session_id';
      const existing = sessionStorage.getItem(key);
      if (existing) return existing;
      const created = globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(16).slice(2)}`;
      sessionStorage.setItem(key, created);
      return created;
    } catch (_) {
      return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
    }
  })();

  function siteTrack(name, props) {
    if (location.hostname === 'localhost' || location.hostname === '127.0.0.1' || location.hostname === '::1') return;
    const event = {
      v: 1,
      name,
      ts_ms: Date.now(),
      session_id: siteSessionId,
      portal: 'site',
      platform: 'web',
      build: 'site',
      locale: (typeof window.SOW_getLocale === 'function' ? window.SOW_getLocale() : 'en').slice(0, 32),
    };
    if (props && typeof props === 'object') event.props = props;
    fetch('/api/event', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ events: [event] }),
      keepalive: true,
    }).catch(() => {});
  }

  const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');

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
  const avatar = leader => `/assets/gameplay/avatars/${leader.image}.webp`;
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

    // Trigger subtle glitch burst on hero frame
    const glitch = $('.hologram-glitch');
    if (glitch && !prefersReducedMotion.matches) {
      glitch.style.opacity = '0.5';
      setTimeout(() => { glitch.style.opacity = ''; }, 120);
    }

    const heroImage = $('[data-hero-image]');
    if (heroImage) {
      heroImage.src = asset(leader);
      heroImage.alt = siteText('site.leader_artwork', `${name} leader artwork`, { name });
    }
    const heroName = $('[data-hero-name]');
    if (heroName) heroName.textContent = name;
    const heroCiv = $('[data-hero-civ]');
    if (heroCiv) heroCiv.textContent = civ;
    const detailImage = $('[data-detail-image]');
    if (detailImage) {
      detailImage.src = asset(leader);
      detailImage.srcset = `${asset(leader, true)} 600w, ${asset(leader)} 1200w`;
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

    $$('.leader-chip').forEach((item, itemIndex) => {
      const active = itemIndex === index;
      item.classList.toggle('is-active', active);
      item.setAttribute('aria-pressed', String(active));
      item.setAttribute('aria-label', siteText('site.inspect_leader', `Inspect ${leaderName(leaders[itemIndex])}`, { name: leaderName(leaders[itemIndex]) }));
    });
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

  function renderLeaderRail() {
    const rail = $('[data-leader-rail]');
    if (!rail || !leaders.length) return;
    rail.innerHTML = '';
    leaders.forEach((leader, index) => {
      const button = document.createElement('button');
      button.className = `leader-chip${index === 0 ? ' is-active' : ''}`;
      button.type = 'button';
      button.setAttribute('aria-pressed', index === 0 ? 'true' : 'false');
      button.title = leaderName(leader);
      button.setAttribute('aria-label', siteText('site.select_leader', `Select ${leaderName(leader)}`, { name: leaderName(leader) }));
      button.innerHTML = `<span class="sheen" aria-hidden="true"></span><img src="${avatar(leader)}" alt="" width="256" height="256" decoding="async">`;
      button.addEventListener('click', () => updateLeader(index));
      rail.appendChild(button);
    });
  }

  function bindLeaderGrid() {
    cards.forEach((card, index) => {
      const sheen = document.createElement('span');
      sheen.className = 'sheen';
      sheen.setAttribute('aria-hidden', 'true');
      card.appendChild(sheen);

      card.addEventListener('click', () => {
        updateLeader(index);
        $('[data-leader-detail]')?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      });
    });
  }

  function bindSiteAnalytics() {
    siteTrack('landing_visit');
    $$('a[href="/play/"]').forEach(link => {
      link.addEventListener('click', () => siteTrack('play_now_click'));
    });
  }

  renderLeaderRail();
  bindLeaderGrid();
  if (leaders.length) {
    updateLeader(0);
  }
  window.addEventListener('sow:locale-change', () => updateLeader(activeLeaderIndex));
  if (window.SOW_I18N_READY && typeof window.SOW_I18N_READY.then === 'function') {
    window.SOW_I18N_READY.then(() => updateLeader(activeLeaderIndex)).catch(() => {});
  }
  bindSiteAnalytics();
})();
