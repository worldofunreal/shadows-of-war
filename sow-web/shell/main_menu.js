/*
 * Source entry for the web menu runtime.
 * sow-dist concatenates these files in this order before inlining:
 * main_menu.i18n.js -> main_menu.core.js -> main_menu.motion.js -> main_menu.lobbies.js -> main_menu.store.js -> main_menu.heroes.js -> main_menu.profile.js -> main_menu.shell.js -> main_menu.hud.js
 * Poki inserts main_menu.poki.js before main_menu.shell.js so it can reuse the shared closure.
 */
