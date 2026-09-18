(() => {
  const root = document.documentElement;
  const toggles = [...document.querySelectorAll('[data-theme-toggle]')];
  const localeControls = [...document.querySelectorAll('[data-locale-select]')];
  let openLocaleKey = null;

  const translate = (key, values) => typeof window.SOW_t === 'function' ? window.SOW_t(key, values) : key;
  const translated = (key, fallback) => {
    const value = translate(key);
    return value === key || value === `[${key}]` ? fallback : value;
  };

  const localeOptions = codes => codes.map(code => ({
    value: code,
    label: translate(`menu.language_${code}`)
  }));

  function syncLocaleDropdown(dropdown, open) {
    const trigger = dropdown.querySelector('[data-role="dropdown-trigger"]');
    const menu = dropdown.querySelector('[role="listbox"]');
    if (!trigger || !menu) return;
    trigger.setAttribute('aria-expanded', String(open));
    menu.hidden = !open;
    const input = dropdown.querySelector('[data-dropdown-input]');
    const value = input ? input.value : '';
    const label = dropdown.querySelector('[data-dropdown-label]');
    dropdown.querySelectorAll('[role="option"]').forEach(option => {
      const selected = option.dataset.dropdownOptionValue === value;
      option.setAttribute('aria-selected', String(selected));
      if (selected && label) label.textContent = option.textContent;
    });
  }

  function syncLocaleDropdowns(focusTarget) {
    localeControls.forEach(control => {
      const dropdown = control.querySelector('[data-control-dropdown]');
      if (dropdown) syncLocaleDropdown(dropdown, dropdown.dataset.dropdownKey === openLocaleKey);
    });
    if (focusTarget) focusTarget.focus();
  }

  function renderLocaleControls(locale, codes) {
    if (typeof window.SOW_renderDropdown !== 'function') return;
    openLocaleKey = null;
    localeControls.forEach((control, index) => {
      const key = `site-locale-${index}`;
      control.className = 'site-locale';
      control.innerHTML = window.SOW_renderDropdown({
        key,
        name: 'locale',
        label: translate('site.select_language'),
        value: locale,
        className: 'site-locale-dropdown',
        options: localeOptions(codes)
      });
      const dropdown = control.querySelector('[data-control-dropdown]');
      const trigger = dropdown && dropdown.querySelector('[data-role="dropdown-trigger"]');
      const menu = dropdown && dropdown.querySelector('[role="listbox"]');
      if (trigger) trigger.setAttribute('aria-label', translate('site.select_language'));
      if (menu) menu.setAttribute('aria-label', translate('site.select_language'));
    });
    syncLocaleDropdowns();
  }

  function applyLocale() {
    if (typeof window.SOW_getLocale !== 'function' || typeof window.SOW_t !== 'function') return;
    const locale = window.SOW_getLocale();
    root.lang = locale;
    document.querySelectorAll('[data-i18n]').forEach(element => {
      const key = element.dataset.i18n;
      const value = translate(key);
      const attribute = element.dataset.i18nAttr;
      if (attribute) element.setAttribute(attribute, value);
      else element.textContent = value;
      if (element.dataset.i18nPlaceholder) {
        element.setAttribute('placeholder', translate(element.dataset.i18nPlaceholder));
      }
    });
    const codes = typeof window.SOW_getSupportedLocales === 'function' ? window.SOW_getSupportedLocales() : [];
    renderLocaleControls(locale, codes);
    setTheme(root.dataset.theme || 'dark');
  }

  function setTheme(theme) {
    root.dataset.theme = theme;
    localStorage.setItem('sow-theme', theme);
    const light = theme === 'light';
    toggles.forEach(toggle => {
      const icon = toggle.querySelector('.theme-icon');
      const label = toggle.querySelector('.theme-label');
      if (icon) icon.textContent = light ? '☾' : '☼';
      if (label) label.textContent = translated(light ? 'site.dark_mode' : 'site.light_mode', label.textContent);
      const ariaLabel = translated(light ? 'site.switch_to_dark' : 'site.switch_to_light', toggle.getAttribute('aria-label'));
      if (ariaLabel) toggle.setAttribute('aria-label', ariaLabel);
    });
    const themeColor = document.querySelector('meta[name="theme-color"]');
    if (themeColor) themeColor.content = light ? '#f5f0e6' : '#0a0a0e';
  }

  function closeLocaleDropdown(focusTarget) {
    openLocaleKey = null;
    syncLocaleDropdowns(focusTarget);
  }

  const saved = localStorage.getItem('sow-theme');
  const preferred = window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
  setTheme(saved || preferred);
  toggles.forEach(toggle => toggle.addEventListener('click', () => {
    setTheme(root.dataset.theme === 'light' ? 'dark' : 'light');
  }));

  document.addEventListener('click', event => {
    const option = event.target.closest('[data-locale-select] [data-command="select_dropdown"]');
    if (option) {
      const value = option.dataset.dropdownOptionValue;
      closeLocaleDropdown();
      if (typeof window.SOW_setLocale === 'function') window.SOW_setLocale(value).catch(() => {});
      return;
    }
    const trigger = event.target.closest('[data-locale-select] [data-role="dropdown-trigger"]');
    if (trigger) {
      const key = trigger.dataset.dropdownKey;
      openLocaleKey = openLocaleKey === key ? null : key;
      syncLocaleDropdowns();
    }
  });

  document.addEventListener('pointerdown', event => {
    if (openLocaleKey && !event.target.closest('[data-locale-select]')) closeLocaleDropdown();
  });

  document.addEventListener('keydown', event => {
    const trigger = event.target.closest('[data-locale-select] [data-role="dropdown-trigger"]');
    const option = event.target.closest('[data-locale-select] [data-command="select_dropdown"]');
    if (trigger && ['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)) {
      event.preventDefault();
      openLocaleKey = trigger.dataset.dropdownKey;
      syncLocaleDropdowns();
      const options = trigger.parentElement.querySelectorAll('[data-command="select_dropdown"]');
      if (options.length) options[event.key === 'ArrowUp' ? options.length - 1 : 0].focus();
      return;
    }
    if (option) {
      const options = [...option.closest('[data-control-dropdown]').querySelectorAll('[data-command="select_dropdown"]')];
      const index = options.indexOf(option);
      const next = event.key === 'ArrowDown' ? Math.min(options.length - 1, index + 1)
        : event.key === 'ArrowUp' ? Math.max(0, index - 1)
          : event.key === 'Home' ? 0
            : event.key === 'End' ? options.length - 1 : index;
      if (next !== index) {
        event.preventDefault();
        options[next].focus();
      } else if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        option.click();
      }
      return;
    }
    if (event.key === 'Escape' && openLocaleKey) {
      const triggerToRestore = document.querySelector(`[data-locale-select] [data-dropdown-key="${CSS.escape(openLocaleKey)}"] [data-role="dropdown-trigger"]`);
      event.preventDefault();
      closeLocaleDropdown(triggerToRestore);
    }
  });

  window.addEventListener('sow:locale-change', applyLocale);
  if (window.SOW_I18N_READY && typeof window.SOW_I18N_READY.then === 'function') {
    window.SOW_I18N_READY.then(applyLocale).catch(() => {});
  }

  const menu = document.querySelector('#mobile-menu');
  const menuToggle = document.querySelector('[data-menu-toggle]');
  if (!menu || !menuToggle) return;

  const setMenuOpen = open => {
    menu.classList.toggle('is-open', open);
    menuToggle.textContent = open ? '×' : '☰';
    menuToggle.setAttribute('aria-expanded', String(open));
    menuToggle.setAttribute('aria-label', translate(open ? 'site.close_navigation' : 'site.open_navigation'));
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
