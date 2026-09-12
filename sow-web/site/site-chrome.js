(() => {
  const root = document.documentElement;
  const toggles = [...document.querySelectorAll('[data-theme-toggle]')];

  function setTheme(theme) {
    root.dataset.theme = theme;
    localStorage.setItem('sow-theme', theme);
    const light = theme === 'light';
    toggles.forEach(toggle => {
      const icon = toggle.querySelector('.theme-icon');
      const label = toggle.querySelector('.theme-label');
      if (icon) icon.textContent = light ? '☾' : '☼';
      if (label) label.textContent = light ? 'Dark mode' : 'Light mode';
      toggle.setAttribute('aria-label', light ? 'Switch to dark mode' : 'Switch to light mode');
    });
    const themeColor = document.querySelector('meta[name="theme-color"]');
    if (themeColor) themeColor.content = light ? '#f5f0e6' : '#0a0a0e';
  }

  const saved = localStorage.getItem('sow-theme');
  const preferred = window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
  setTheme(saved || preferred);
  toggles.forEach(toggle => toggle.addEventListener('click', () => {
    setTheme(root.dataset.theme === 'light' ? 'dark' : 'light');
  }));

  const menu = document.querySelector('#mobile-menu');
  const menuToggle = document.querySelector('[data-menu-toggle]');
  if (!menu || !menuToggle) return;

  const setMenuOpen = open => {
    menu.classList.toggle('is-open', open);
    menuToggle.textContent = open ? '×' : '☰';
    menuToggle.setAttribute('aria-expanded', String(open));
    menuToggle.setAttribute('aria-label', open ? 'Close navigation' : 'Open navigation');
    menu.setAttribute('aria-hidden', String(!open));
  };

  const closeMenu = () => setMenuOpen(false);
  closeMenu();
  menuToggle.addEventListener('click', () => {
    setMenuOpen(!menu.classList.contains('is-open'));
  });
  menu.querySelectorAll('a').forEach(link => link.addEventListener('click', closeMenu));
  document.addEventListener('pointerdown', event => {
    if (menu.classList.contains('is-open') && !menu.contains(event.target) && !menuToggle.contains(event.target)) closeMenu();
  });
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape' && menu.classList.contains('is-open')) closeMenu();
  });
})();
