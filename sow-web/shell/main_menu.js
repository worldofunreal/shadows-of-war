/*
 * Source entry for the web menu runtime. Bundle order starts with sow-i18n.js
 * -> sow-dropdown.js -> sow-campaign.js -> sow-campaign-view.js, then menu modules.
 * Main-menu order: main_menu.core.js -> main_menu.motion.js -> main_menu.lobbies.js -> main_menu.store.js -> main_menu.heroes.js -> main_menu.profile.js -> main_menu.tutorial.js -> main_menu.shell.js -> main_menu.hud.js
 * Poki inserts main_menu.poki.js before main_menu.tutorial.js so it can reuse the shared closure.
 * Jest inserts main_menu.jest.js at the same slot instead (never both).
 */
