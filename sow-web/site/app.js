(() => {
  const $ = selector => document.querySelector(selector);
  const $$ = selector => [...document.querySelectorAll(selector)];

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
      locale: (navigator.language || 'en').slice(0, 32),
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
    code: c.dataset.code,
    ability: c.dataset.ability,
    description: c.dataset.description,
    image: c.dataset.image
  }));

  const asset = (leader, mobile = false) => `/assets/shell/leaders/${leader.image}_${mobile ? 'mobile' : 'desktop'}.webp`;
  const avatar = leader => `/assets/gameplay/avatars/${leader.image}.webp`;

  function updateLeader(index) {
    const leader = leaders[index];
    if (!leader) return;

    // Trigger subtle glitch burst on hero frame
    const glitch = $('.hologram-glitch');
    if (glitch && !prefersReducedMotion.matches) {
      glitch.style.opacity = '0.5';
      setTimeout(() => { glitch.style.opacity = ''; }, 120);
    }

    const heroImage = $('[data-hero-image]');
    if (heroImage) {
      heroImage.src = asset(leader);
      heroImage.alt = `${leader.name} leader artwork`;
    }
    const heroName = $('[data-hero-name]');
    if (heroName) heroName.textContent = leader.name;
    const heroCiv = $('[data-hero-civ]');
    if (heroCiv) heroCiv.textContent = leader.civ;
    const detailImage = $('[data-detail-image]');
    if (detailImage) {
      detailImage.src = asset(leader);
      detailImage.srcset = `${asset(leader, true)} 600w, ${asset(leader)} 1200w`;
      detailImage.alt = `${leader.name} artwork`;
    }
    const detailCode = $('[data-detail-code]');
    if (detailCode) detailCode.textContent = `${leader.code} · ${String(index + 1).padStart(2, '0')}`;
    const detailName = $('[data-detail-name]');
    if (detailName) detailName.textContent = leader.name;
    const detailCiv = $('[data-detail-civ]');
    if (detailCiv) detailCiv.textContent = leader.civ;
    const detailAbility = $('[data-detail-ability]');
    if (detailAbility) detailAbility.textContent = leader.ability;
    const detailDesc = $('[data-detail-description]');
    if (detailDesc) {
      detailDesc.textContent = leader.description;
    }

    $$('.leader-chip').forEach((item, itemIndex) => {
      const active = itemIndex === index;
      item.classList.toggle('is-active', active);
      item.setAttribute('aria-pressed', String(active));
    });
    cards.forEach((item, itemIndex) => {
      const active = itemIndex === index;
      item.classList.toggle('is-active', active);
      item.setAttribute('aria-pressed', String(active));
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
      button.title = leader.name;
      button.setAttribute('aria-label', `Select ${leader.name}`);
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
  bindSiteAnalytics();
})();
