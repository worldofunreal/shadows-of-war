/*
 * Entry for the web menu runtime. Bundling this file pulls in every menu module
 * in the order listed here. Portal builds (Poki, Jest) replace
 * main_menu.surfaces.ts with their own surface module; see sow-dist.
 */
import "./sow-i18n.ts";
import "./sow-dropdown.ts";
import "./sow-campaign.ts";
import "./sow-campaign-view.ts";
import "./main_menu.core.ts";
import "./main_menu.motion.ts";
import "./main_menu.lobbies.ts";
import "./main_menu.heroes.ts";
import "./main_menu.profile.ts";
import "./main_menu.surfaces.ts";
import "./main_menu.tutorial.ts";
import "./main_menu.shell.ts";
import "./main_menu.hud.ts";
