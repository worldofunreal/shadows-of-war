(() => {
  const root = document.documentElement;
  const translate = (key) => (typeof window.SOW_t === "function" ? window.SOW_t(key) : key);
  const languageMenus = [...document.querySelectorAll(".site-language")];
  document.addEventListener("click", (event) => {
    const link = event.target.closest("[data-site-language]");
    if (link) {
      try {
        localStorage.setItem("sow_ui_locale", link.dataset.siteLanguage.toLowerCase());
      } catch {}
      // Keep a section anchor when switching the landing's language.
      if (!root.dataset.sitePage && window.location.hash) link.hash = window.location.hash;
    }
  });
  document.addEventListener("pointerdown", (event) => {
    languageMenus.forEach((menu) => {
      if (!menu.contains(event.target)) menu.open = false;
    });
  });
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    languageMenus.forEach((menu) => {
      if (menu.open) {
        event.preventDefault();
        menu.open = false;
        menu.querySelector("summary").focus();
      }
    });
  });
  // Suggest a language only on the default entry. Its URL and rendered text stay English.
  if (window.location.pathname === "/") {
    let saved = "";
    try {
      saved = localStorage.getItem("sow_ui_locale") || "";
    } catch {}
    const links = [...document.querySelectorAll(".site-language a[data-site-language]")];
    const aliases = { zh: "zh-cn", pt: "pt-br", tl: "fil" };
    const match = (value) => {
      const code = String(value || "")
        .toLowerCase()
        .replace(/_/g, "-");
      return (
        links.find((link) => link.dataset.siteLanguage.toLowerCase() === code) ||
        links.find(
          (link) =>
            link.dataset.siteLanguage.toLowerCase() ===
            (aliases[code] || aliases[code.split("-")[0]] || code.split("-")[0]),
        )
      );
    };
    const suggested = [saved, ...(navigator.languages || [navigator.language])].map(match).find(Boolean);
    if (suggested && suggested.dataset.siteLanguage !== "en") {
      const link = document.createElement("a");
      link.className = "language-suggestion";
      link.href = suggested.href;
      link.dataset.siteLanguage = suggested.dataset.siteLanguage;
      link.textContent = root.dataset.languageSuggestion.replace("{language}", suggested.textContent) + " →";
      link.lang = suggested.lang;
      document.querySelector(".hero-actions")?.append(link);
    }
  }
  const search = document.querySelector("[data-compendium-search]");
  if (search) {
    const cards = [...document.querySelectorAll("[data-compendium-card]")];
    const fold = (value) => value.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase();
    const filter = () => {
      const query = fold(search.value.trim());
      let count = 0;
      cards.forEach((card) => {
        card.hidden = !fold(card.dataset.search).includes(query);
        if (!card.hidden) count++;
      });
      document.querySelector("[data-compendium-count]").textContent = `${count} / ${cards.length}`;
      document.querySelector("[data-compendium-empty]").hidden = count !== 0;
    };
    search.form.addEventListener("submit", (event) => event.preventDefault());
    search.addEventListener("input", filter);
    document.querySelector("[data-compendium-reset]").addEventListener("click", () => {
      search.value = "";
      filter();
      search.focus();
    });
    filter();
  }
  if (window.SOW_I18N_READY) {
    window.SOW_I18N_READY.catch((error) =>
      console.error("[SOW site] interactive translations unavailable", error),
    ).finally(() => {
      delete root.dataset.sowLocalePending;
    });
  }
  const menu = document.querySelector("#mobile-menu");
  const menuToggle = document.querySelector("[data-menu-toggle]");
  if (!menu || !menuToggle) return;

  const setMenuOpen = (open) => {
    menu.classList.toggle("is-open", open);
    menuToggle.textContent = open ? "×" : "☰";
    menuToggle.setAttribute("aria-expanded", String(open));
    menuToggle.setAttribute("aria-label", translate(open ? "site.close_navigation" : "site.open_navigation"));
    menu.setAttribute("aria-hidden", String(!open));
  };

  const closeMenu = () => setMenuOpen(false);
  closeMenu();
  menuToggle.addEventListener("click", () => {
    setMenuOpen(!menu.classList.contains("is-open"));
  });
  menu.querySelectorAll("a").forEach((link) => link.addEventListener("click", closeMenu));
  document.addEventListener("pointerdown", (event) => {
    if (
      menu.classList.contains("is-open") &&
      !menu.contains(event.target) &&
      !menuToggle.contains(event.target)
    )
      closeMenu();
  });
  document.addEventListener("keydown", (event) => {
    if (!event.defaultPrevented && event.key === "Escape" && menu.classList.contains("is-open")) {
      closeMenu();
      menuToggle.focus();
    }
  });
})();
