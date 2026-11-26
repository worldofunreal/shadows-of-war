(() => {
  const codex = document.querySelector<HTMLElement>("[data-sow-buildings-codex]");
  if (!codex) return;

  const buttons = Array.from(codex.querySelectorAll<HTMLButtonElement>("[data-codex-filter]"));
  const sections = Array.from(codex.querySelectorAll<HTMLElement>("[data-codex-section]"));
  if (!buttons.length || !sections.length) return;

  const prefersReducedMotion =
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  let activeFilter = "all";

  function applyFilter(target: string, syncHash: boolean): void {
    const valid = target === "all" || sections.some((s) => s.dataset.codexSection === target);
    activeFilter = valid ? target : "all";

    buttons.forEach((btn) => {
      const pressed = btn.dataset.codexFilter === activeFilter;
      btn.setAttribute("aria-pressed", pressed ? "true" : "false");
    });

    sections.forEach((section) => {
      section.hidden =
        activeFilter !== "all" && section.dataset.codexSection !== activeFilter;
    });

    if (syncHash) {
      const nextUrl =
        activeFilter === "all"
          ? window.location.pathname + window.location.search
          : "#" + activeFilter;
      try {
        window.history.replaceState(null, "", nextUrl);
      } catch {
        // Ignore history restrictions in sandboxed environments.
      }
    }
  }

  function scrollToSection(id: string): void {
    const section = sections.find((s) => s.id === id);
    if (!section) return;
    section.scrollIntoView({
      behavior: prefersReducedMotion ? "auto" : "smooth",
      block: "start",
    });
  }

  buttons.forEach((button, index) => {
    button.addEventListener("click", () => {
      const target = button.dataset.codexFilter || "all";
      applyFilter(target, true);
      if (target !== "all") {
        scrollToSection(target);
      }
    });

    button.addEventListener("keydown", (event) => {
      let next = index;
      if (event.key === "ArrowRight") next = (index + 1) % buttons.length;
      else if (event.key === "ArrowLeft") next = (index - 1 + buttons.length) % buttons.length;
      else if (event.key === "Home") next = 0;
      else if (event.key === "End") next = buttons.length - 1;
      else return;
      event.preventDefault();
      buttons[next]?.focus();
    });
  });

  codex.addEventListener("click", (event) => {
    const link = (event.target as Element | null)?.closest<HTMLAnchorElement>('a[href^="#"]');
    if (!link) return;
    const hashId = link.getAttribute("href")?.slice(1);
    if (!hashId) return;
    if (activeFilter !== "all" && activeFilter !== hashId) {
      applyFilter("all", false);
    }
    const target = sections.find((s) => s.id === hashId);
    if (!target) return;
    event.preventDefault();
    try {
      window.history.replaceState(null, "", "#" + hashId);
    } catch {}
    scrollToSection(hashId);
  });

  const initialHash = window.location.hash.replace(/^#/, "").toLowerCase();
  if (initialHash && sections.some((s) => s.id === initialHash)) {
    window.requestAnimationFrame(() => scrollToSection(initialHash));
  }
})();
