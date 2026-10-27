/*
 * Source entry for the web menu runtime. Bundle order starts with sow-i18n.ts
 * -> sow-dropdown.ts -> sow-campaign.ts -> sow-campaign-view.ts -> main_menu.flow.ts, then menu modules.
 * Main-menu order: main_menu.core.ts -> main_menu.motion.ts -> main_menu.lobbies.ts -> main_menu.store.ts -> main_menu.heroes.ts -> main_menu.profile.ts -> main_menu.tutorial.ts -> main_menu.shell.ts -> main_menu.hud.ts
 * Poki inserts main_menu.poki.ts before main_menu.tutorial.ts so it can reuse the shared closure.
 * Jest inserts main_menu.jest.ts at the same slot instead (never both).
 */
