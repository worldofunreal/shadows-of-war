(() => {
  const $ = (selector) => document.querySelector(selector);
  const $$ = (selector) => [...document.querySelectorAll(selector)];
  const siteText = (key, fallback, values?) => {
    if (typeof window.SOW_t !== "function") return fallback;
    const value = window.SOW_t(key, values);
    return value === `[${key}]` ? fallback : value;
  };
  const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  const mobile = window.matchMedia("(max-width: 960px)");
  const motionButton = $("[data-motion-toggle]");
  const heroVideo = $(".hero-video");
  const stepVideos = $$("[data-step-video]");
  let motionPaused = false;
  let heroVisible = false;
  let videoFailed = false;
  function syncMotion() {
    const paused = motionPaused || prefersReducedMotion.matches;
    document.documentElement.classList.toggle("landing-motion-paused", paused);
    stepVideos.forEach((video) => {
      if (paused || document.visibilityState === "hidden" || video.dataset.inView === "false" || video.dataset.failed) {
        video.pause();
      } else {
        video.play()?.catch(() => {});
      }
    });
    if (!heroVideo) return;
    if (paused || document.visibilityState === "hidden" || !heroVisible || videoFailed) {
      heroVideo.pause();
      heroVideo.hidden = true;
    } else {
      heroVideo.play()?.catch(() => {
        // Autoplay is optional: the opening picture remains visible underneath.
        heroVideo.hidden = true;
      });
    }
    if (motionButton) {
      motionButton.hidden = videoFailed;
      motionButton.disabled = prefersReducedMotion.matches;
      motionButton.setAttribute("aria-pressed", String(paused));
      $("[data-motion-label]").textContent = paused
        ? siteText("site.resume_motion", "Resume motion")
        : siteText("site.pause_motion", "Pause motion");
    }
  }
  motionButton?.addEventListener("click", () => {
    motionPaused = !(motionPaused || prefersReducedMotion.matches);
    // A reduced-motion preference always wins over an automatic animation.
    syncMotion();
  });
  if (heroVideo) {
    heroVideo.addEventListener("playing", () => {
      heroVideo.hidden =
        motionPaused || prefersReducedMotion.matches || !heroVisible || document.visibilityState === "hidden";
    });
    heroVideo.addEventListener("error", () => {
      videoFailed = true;
      heroVideo.hidden = true;
      syncMotion();
    });
    if (window.IntersectionObserver) {
      new window.IntersectionObserver(([entry]) => {
        heroVisible = entry.isIntersecting;
        syncMotion();
      }).observe(heroVideo.closest(".hero"));
    } else {
      heroVisible = true;
    }
    document.addEventListener("visibilitychange", syncMotion);
  }
  prefersReducedMotion.addEventListener?.("change", syncMotion);
  syncMotion();

  // The publisher renders this single roster from the playable character catalog.
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
  let activeLeaderIndex = Math.max(
    0,
    cards.findIndex((card) => card.classList.contains("is-active")),
  );
  let activeArtSlug = "";
  const image = $("[data-detail-image]");
  const mobileImage = $("[data-detail-image-mobile]");
  const portrait = $("[data-detail-portrait]");
  const art = $("[data-detail-art]");
  const search = $("[data-leader-search]");
  const browser = $("[data-leader-browser]");
  const browserHome = $("[data-leader-browser-home]");
  const picker = $("[data-leader-picker]");
  const browse = $("[data-leader-browse]");
  let pickerScrollY = 0;
  let pickerReturnFocus = null;
  const normalize = (text) => String(text).normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase();

  function showText(element, leader, field) {
    if (!element) return "";
    element.setAttribute("data-i18n", textKey(leader, field));
    element.textContent = leaderText(leader, field);
    return element.textContent;
  }
  function filterLeaders() {
    const query = normalize(search?.value || "").trim();
    let shown = 0;
    cards.forEach((card, index) => {
      const leader = leaders[index];
      card.hidden = !normalize(
        `${leaderText(leader, "name")} ${leaderText(leader, "civilization")}`,
      ).includes(query);
      if (!card.hidden) shown++;
      card.setAttribute(
        "aria-label",
        siteText("site.inspect_leader", `Inspect ${leaderText(leader, "name")}`, {
          name: leaderText(leader, "name"),
        }),
      );
    });
    if ($("[data-roster-count]")) $("[data-roster-count]").textContent = `${shown} / ${leaders.length}`;
    if ($("[data-roster-empty]")) $("[data-roster-empty]").hidden = shown !== 0;
  }
  const isCurrentArtwork = () =>
    ["desktop", "mobile"].some((variant) =>
      image.currentSrc.endsWith(`/assets/shell/leaders/${activeArtSlug}_${variant}.webp`),
    );
  image?.addEventListener("load", () => {
    if (isCurrentArtwork()) art.classList.remove("is-loading", "is-unavailable");
  });
  image?.addEventListener("error", () => {
    if (!isCurrentArtwork()) return;
    art.classList.remove("is-loading");
    art.classList.add("is-unavailable");
  });
  portrait?.addEventListener("error", () => {
    portrait.hidden = true;
  });

  function updateLeader(index, animate = false) {
    const leader = leaders[index];
    if (!leader) return;
    activeLeaderIndex = index;
    const name = showText($("[data-detail-name]"), leader, "name");
    if (image) {
      image.alt = name;
      if (activeArtSlug !== leader.artSlug) {
        activeArtSlug = leader.artSlug;
        art.classList.add("is-loading");
        art.classList.remove("is-unavailable");
        portrait.hidden = false;
        portrait.src = `/assets/gameplay/avatars/${leader.artSlug}.webp`;
        // Change the responsive source first, so the browser never requests
        // the desktop art for an intermediate leader on a phone.
        if (mobileImage) mobileImage.srcset = `/assets/shell/leaders/${leader.artSlug}_mobile.webp`;
        image.src = `/assets/shell/leaders/${leader.artSlug}_desktop.webp`;
        if (image.complete && image.naturalWidth && isCurrentArtwork()) art.classList.remove("is-loading");
        if (animate && !prefersReducedMotion.matches) {
          art.animate?.([{ opacity: 0.65 }, { opacity: 1 }], { duration: 220, easing: "ease-out" });
        }
      }
    }
    showText($("[data-detail-civ]"), leader, "civilization");
    const historical = $("[data-detail-historical]");
    if (historical) historical.hidden = showText(historical, leader, "historical") === name;
    showText($("[data-detail-ability]"), leader, "ability");
    showText($("[data-detail-description]"), leader, "description");
    if ($("[data-leader-position]"))
      $("[data-leader-position]").textContent = `${name} · ${index + 1} / ${leaders.length}`;
    cards.forEach((card, cardIndex) => {
      const active = cardIndex === index;
      card.classList.toggle("is-active", active);
      card.setAttribute("aria-pressed", String(active));
    });
    // Reveal selection inside the roster without scrolling the page or preview.
    if (animate && !cards[index].hidden) {
      const scroll = $("[data-roster-scroll]");
      const cardBounds = cards[index].getBoundingClientRect();
      const scrollBounds = scroll.getBoundingClientRect();
      if (cardBounds.top < scrollBounds.top) scroll.scrollTop += cardBounds.top - scrollBounds.top;
      else if (cardBounds.bottom > scrollBounds.bottom)
        scroll.scrollTop += cardBounds.bottom - scrollBounds.bottom;
      if (cardBounds.left < scrollBounds.left) scroll.scrollLeft += cardBounds.left - scrollBounds.left;
      else if (cardBounds.right > scrollBounds.right)
        scroll.scrollLeft += cardBounds.right - scrollBounds.right;
    }
  }

  function restoreBrowser() {
    browserHome?.append(browser);
    document.documentElement.classList.remove("leader-picker-open");
    browse?.setAttribute("aria-expanded", "false");
    pickerReturnFocus?.focus({ preventScroll: true });
    if (window.scrollY !== pickerScrollY) window.scrollTo({ top: pickerScrollY, behavior: "instant" });
  }
  browse?.addEventListener("click", () => {
    if (!picker || !browser || picker.open) return;
    pickerScrollY = window.scrollY;
    pickerReturnFocus = document.activeElement;
    picker.append(browser);
    document.documentElement.classList.add("leader-picker-open");
    picker.showModal();
    browse.setAttribute("aria-expanded", "true");
    search?.focus({ preventScroll: true });
  });
  picker?.addEventListener("close", restoreBrowser);
  $("[data-picker-close]")?.addEventListener("click", () => picker.close());
  mobile.addEventListener?.("change", () => {
    if (!mobile.matches && picker?.open) picker.close();
  });
  cards.forEach((card, index) =>
    card.addEventListener("click", () => {
      updateLeader(index, true);
      if (picker?.open) picker.close();
    }),
  );
  $("[data-leader-prev]")?.addEventListener("click", () =>
    updateLeader((activeLeaderIndex - 1 + leaders.length) % leaders.length, true),
  );
  $("[data-leader-next]")?.addEventListener("click", () =>
    updateLeader((activeLeaderIndex + 1) % leaders.length, true),
  );
  search?.addEventListener("input", () => {
    filterLeaders();
    if ($("[data-roster-scroll]")) $("[data-roster-scroll]").scrollTop = 0;
  });
  $("[data-clear-search]")?.addEventListener("click", () => {
    search.value = "";
    filterLeaders();
    search.focus({ preventScroll: true });
  });
  const roster = $("[data-leaders-grid]");
  roster?.addEventListener("keydown", (event) => {
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) return;
    const visible = cards.filter((card) => !card.hidden);
    const position = visible.indexOf(document.activeElement);
    if (position < 0) return;
    event.preventDefault();
    const columns = getComputedStyle(roster).gridTemplateColumns.split(" ").length;
    const rtl = document.documentElement.dir === "rtl" ? -1 : 1;
    const step = { ArrowLeft: -rtl, ArrowRight: rtl, ArrowUp: -columns, ArrowDown: columns }[event.key];
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? visible.length - 1
          : Math.max(0, Math.min(visible.length - 1, position + step));
    visible[next]?.focus();
  });

  const mapCards = $$("[data-map-select]");
  const mapImage = $("[data-map-image]");
  const mapTitle = $("[data-map-name]");
  const mapFrame = $("[data-map-frame]");
  let activeMapImage = mapImage?.getAttribute("src") || "";
  mapImage?.addEventListener("load", () => {
    if (mapImage.currentSrc.endsWith(activeMapImage))
      mapFrame.classList.remove("is-loading", "is-unavailable");
  });
  mapImage?.addEventListener("error", () => {
    if (!mapImage.currentSrc.endsWith(activeMapImage)) return;
    mapFrame.classList.remove("is-loading");
    mapFrame.classList.add("is-unavailable");
  });
  mapCards.forEach((card) =>
    card.addEventListener("click", () => {
      if (card.getAttribute("aria-pressed") === "true") return;
      mapFrame.classList.add("is-loading");
      mapFrame.classList.remove("is-unavailable");
      mapImage.alt = card.dataset.mapName;
      mapTitle.textContent = card.dataset.mapName;
      activeMapImage = card.dataset.mapImage;
      mapImage.src = activeMapImage;
      if (mapImage.complete && mapImage.naturalWidth && mapImage.currentSrc.endsWith(activeMapImage))
        mapFrame.classList.remove("is-loading");
      mapCards.forEach((item) => item.setAttribute("aria-pressed", String(item === card)));
    }),
  );

  function refreshLocale() {
    updateLeader(activeLeaderIndex);
    filterLeaders();
    syncMotion();
  }
  $$("[data-leader-total]").forEach((element) => {
    element.textContent = String(leaders.length);
  });
  if (leaders.length) refreshLocale();
  window.addEventListener("sow:locale-change", refreshLocale);
  if (window.SOW_I18N_READY && typeof window.SOW_I18N_READY.then === "function") {
    window.SOW_I18N_READY.then(refreshLocale).catch(() => {});
  }
})();
