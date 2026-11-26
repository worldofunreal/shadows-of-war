// Canonical icon registry and shared HTML renderers used by both the in-game
// HUD and the public /buildings/ Codex. No DOM dependencies so SSR & browser share one source.

export const HUD_ICONS: Record<string, string> = {
  troops: "gameplay/icons/battle_icon_cutout.webp",
  gold: "gameplay/currency/gold.webp",
  surrender: "gameplay/icons/surrender_flag.webp",
};

// Cell order of the badge strip packed from assets/art/icons (assets/generated/badges-atlas.json).
export const STATUS_CELLS: string[] = [
  "request",
  "allied",
  "traitor",
  "rank-1",
  "rank-2",
  "rank-3",
  "you",
  "disconnected",
  "upgrade-sparkle",
  "wilderness",
  "troops",
  "death",
  "radiation",
  "arrow-up",
  "gold",
  "boat",
  "clock",
  "shield",
  "range",
  "speed",
  "lod-city",
  "lod-tower",
  "lod-trading-post",
  "lod-port",
  "lod-military-camp",
  "lod-sam",
  "lod-silo",
];

export const BUILDING_GLYPHS: Record<string, string> = {
  city: "city",
  farm: "farm",
  tradingpost: "trading_post",
  port: "anchor",
  tower: "defense",
  militarycamp: "military_camp",
  sam: "sam-missile",
};

export const RING_GLYPH_OPEN =
  '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">';

export const HUD_GLYPHS: Record<string, string> = {
  plus:
    '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
    '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
    '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
    '<path d="M7 10l6 0"/>' +
    '<path d="M10 7l0 6"/>' +
    '<path d="M21 21l-6 -6"/>' +
    "</svg>",
  minus:
    '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
    '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
    '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
    '<path d="M7 10l6 0"/>' +
    '<path d="M21 21l-6 -6"/>' +
    "</svg>",
  home:
    '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">' +
    '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
    '<path d="M12.707 2.293l9 9c.63 .63 .184 1.707 -.707 1.707h-1v6a3 3 0 0 1 -3 3h-10a3 3 0 0 1 -3 -3v-6h-1c-.89 0 -1.337 -1.077 -.707 -1.707l9 -9a1 1 0 0 1 1.414 0m.793 8.707h-3a1.5 1.5 0 0 0 -1.5 1.5v3a1.5 1.5 0 0 0 1.5 1.5h3a1.5 1.5 0 0 0 1.5 -1.5v-3a1.5 1.5 0 0 0 -1.5 -1.5"/>' +
    "</svg>",
  zoom_in:
    '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
    '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
    '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
    '<path d="M7 10l6 0"/>' +
    '<path d="M10 7l0 6"/>' +
    '<path d="M21 21l-6 -6"/>' +
    "</svg>",
  zoom_out:
    '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
    '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
    '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
    '<path d="M7 10l6 0"/>' +
    '<path d="M21 21l-6 -6"/>' +
    "</svg>",
  close: '<path d="M18 6 6 18M6 6l12 12"/>',
  inbox:
    '<svg class="sow-hud__action-icon" viewBox="0 0 36 39" fill="currentColor" aria-hidden="true">' +
    '<path d="M26.9474 14.6637C27.3732 15.5695 27.9073 16.266 28.5497 16.746C29.1885 17.226 29.7911 17.5183 30.3505 17.6266C30.9099 17.7348 31.4981 18.0416 32.1116 18.5468C32.7251 19.052 33.1906 19.7882 33.5118 20.7482C34.1 22.401 33.1112 24.5518 30.549 27.1935C27.9867 29.8351 24.6269 31.9282 20.4624 33.4764C16.0849 35.0246 12.126 35.6236 8.57499 35.2772C5.02753 34.9271 2.95969 33.9275 2.37146 32.2747C1.83735 30.8348 1.99614 29.3515 2.85143 27.8322C3.70671 26.3129 3.94489 24.8297 3.57319 23.3898C2.07914 18.2653 1.45121 14.2631 1.693 11.3833C1.93479 8.5035 3.43966 5.94125 6.21483 3.69658C6.90772 3.10835 7.29386 2.42989 7.37686 1.654C7.45626 0.881715 7.8424 0.358439 8.5389 0.091388C9.17766 -0.121531 9.79115 0.037256 10.3794 0.571359C10.9676 1.10546 11.7146 1.34364 12.6205 1.29312C16.1426 1.23899 18.7843 2.11953 20.5454 3.93476C22.3029 5.7536 24.4393 9.32631 26.9474 14.6637ZM19.5024 30.8348C21.8518 29.9795 23.9701 28.8463 25.8648 27.4317C27.7594 26.017 29.0946 24.8045 29.8669 23.7904C30.6392 22.7763 30.9748 22.1087 30.8666 21.7875C30.6536 21.1993 30.0004 20.759 28.907 20.4667C27.8135 20.1744 26.1571 20.1599 23.9449 20.427C21.7327 20.694 19.2354 21.3328 16.4602 22.3469C13.7392 23.3609 11.4295 24.523 9.53493 25.8294C7.64031 27.1357 6.38805 28.2581 5.77456 29.1928C5.16106 30.1274 4.93371 30.8348 5.09249 31.3147C5.20076 31.6359 5.86478 31.9282 7.09538 32.1953C8.32237 32.4623 10.1087 32.5165 12.4581 32.3541C14.8074 32.1917 17.1531 31.6864 19.5024 30.8348ZM17.0196 23.8698C17.2325 23.8156 17.514 23.7218 17.8604 23.5883C18.2069 23.4548 18.4342 23.3609 18.5425 23.3068L18.6219 23.3862C18.9972 24.4544 18.7698 25.5623 17.9398 26.7063C17.1134 27.8539 15.9261 28.6947 14.3779 29.2288C11.8157 30.1888 9.78754 30.0011 8.2935 28.6695C10.3758 26.8579 13.2845 25.2556 17.0196 23.8698Z"/>' +
    "</svg>",
  emote:
    '<path d="M20 11.5a7.5 7.5 0 0 1-8 7.5 8.7 8.7 0 0 1-3.5-.7L4 20l1.4-3.6A7 7 0 0 1 4 12c0-4.1 3.6-7.5 8-7.5s8 3.1 8 7Z"/><path d="M9 11h.01M15 11h.01M9 14.5s1 1.5 3 1.5 3-1.5 3-1.5"/>',
  rankings:
    '<svg class="sow-hud__action-icon" viewBox="0 0 48 41" fill="currentColor" aria-hidden="true">' +
    '<path d="M3.43115 14.2054L5.09646 35.0217H15.0883V14.2054H3.43115Z"/>' +
    '<path d="M15.6436 1.71558C16.4762 12.2625 18.1415 33.6895 18.1415 35.0217H31.464L33.1293 1.71558H15.6436Z"/>' +
    '<path d="M44.2317 20.034H33.4072V34.1891H42.5664L44.2317 20.034Z"/>' +
    "</svg>",
  leaderboard:
    '<svg class="sow-hud__action-icon" viewBox="0 0 48 41" fill="currentColor" aria-hidden="true">' +
    '<path d="M3.43115 14.2054L5.09646 35.0217H15.0883V14.2054H3.43115Z"/>' +
    '<path d="M15.6436 1.71558C16.4762 12.2625 18.1415 33.6895 18.1415 35.0217H31.464L33.1293 1.71558H15.6436Z"/>' +
    '<path d="M44.2317 20.034H33.4072V34.1891H42.5664L44.2317 20.034Z"/>' +
    "</svg>",
  cup:
    '<svg class="sow-hud__action-icon" viewBox="0 0 36 34" fill="currentColor" aria-hidden="true">' +
    '<path fill-rule="evenodd" clip-rule="evenodd" d="M28.8428 0H7.44094V2.48287L1.02324 2.48287C0.45812 2.48287 0 2.94099 0 3.50611V14.129C0 18.63 5.02309 22.3338 9.95271 22.7191C11.571 24.453 14.4175 25.7125 16.951 26.2496V29.2936C14.5283 30.0377 10.2008 31.786 9.95271 33.6909H26.5792C26.3287 31.786 21.5603 30.0353 19.1328 29.2936V26.2496C21.6759 25.7125 24.6501 24.453 26.2708 22.7191C31.198 22.3338 36 18.63 36 14.129V3.50611C36 2.94099 35.5419 2.48287 34.9768 2.48287H28.8428V0ZM2.28495 4.68973H7.26755V14.7214C7.26755 16.701 7.77809 18.5842 8.67636 20.2796C4.94844 19.0924 2.28495 16.0339 2.28495 12.4553V4.68973ZM21.8016 16.925L18.0825 14.8756L14.3593 16.925L15.0702 12.5902L12.0599 9.52209L16.2189 8.88873L18.0825 4.94889L19.942 8.88873L24.101 9.52209L21.0927 12.5902L21.8016 16.925ZM27.328 20.5999C28.2311 18.8274 28.7489 16.8672 28.7489 14.7937V4.75441H33.7435V12.4264C33.7435 16.1664 31.068 19.3597 27.328 20.5999Z"/>' +
    "</svg>",
  search: '<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/>',
  options:
    '<svg class="sow-hud__action-icon" viewBox="70 135 860 775" fill="currentColor" aria-hidden="true">' +
    '<path d="M341,205c22.61-2.97,45.75-.43,68,4c5.93-17.06,20.41-35.48,30-51c30.77,6.9,63.06,23.94,88.06,42.94c9.99,7.59,18.86,16.75,28.94,24.06c-2.72,13.76-24.17,40.64-32,54c6.35,9.77,13.98,18.47,19.98,28.53C557.54,330.24,564.77,353.8,572,379c8.03,1.11,55.3,6.03,58.86,9.19c3.64,3.22,3.43,16.65,3.46,21.41c.21,39.07-5.77,74.39-18.32,111.4c-22.08-1.42-44.14-5.63-66-9c-8.44,21.72-26.54,52.27-48,63c1.83,11.12,25.68,53.57,23,60c-.82,1.97-11.84,8.03-14.18,9.49C478.2,664.88,439.36,684.31,400,684c-4.9-11.59-22-44.37-22-55c-23.77,.37-45.47-1.93-69-4c-9.82,17.51-19.87,35.3-31,52c-43.13-8.12-85.06-41.51-115-72c4-10.54,31.11-47.38,31-53c-.04-2.19-17.99-25.01-20.55-29.44C160.3,499.79,151.48,475.94,147,450c-8.18-1.7-58.86-8.15-61.62-10.63c-1.35-1.21-1.87-11.53-2.05-13.79C80.34,387.45,88.66,341.76,109,309c8.84,.39,57.25,10.02,61,7c3.88-3.13,8.56-14.9,11.67-19.63C191.1,282.04,202.55,268.7,216,258c-2.51-8.07-23.64-52.49-22-57c.65-1.79,8.44-6.25,10.43-7.63C231.08,174.97,284.22,147.01,317,148c4.84,11.37,24,47.28,24,57ZM271,323c-15.23,6.47-25.38,27.19-31.26,41.66c-54.92,135.4,112.99,242.21,215.25,137.59c15.77-16.14,26.19-37.45,30.8-59.43C507.44,339.5,400.97,255.7,305.16,298.4C298.57,301.33,271,316.01,271,323Z"/>' +
    '<path d="M549,715c-6.33-29.98-2.8-58.17,7-87c-11.93-9.74-24.22-19.09-36-29c8.75-30.05,40.88-63.94,67-80c8.2,6.7,24.51,23.56,33.74,26.75c.66,.22,43.57-18.73,56.26-20.75c.69-6.78,2.03-40.4,5.31-44.01c12.66-13.95,87.33,.43,103.69,9.01c-1.03,15.87-4,32.17-4,48c5.6,2.84,40.32,26.85,40.59,26.87c7.85,.64,34.03-16.81,42.41-20.87c20.65,19.28,52,60.93,52,90c-14.45,7.44-28.85,14.99-43,23c5.29,27.32,4.64,60.72-8,86c5.2,2.3,35.43,23.89,37,28c4.16,10.88-48.89,79.17-61.73,82.75c-3.33,.93-10.04-5.5-12.53-7.62c-5.73-4.87-19.87-19.01-26.47-20.86c-2.81-.78-10.77,4.98-13.47,6.54C775.83,839.27,760.29,847,745,847c-.36,6.01-3.06,46.71-6,49c-1.36,1.06-9.82,1.43-11.9,1.62c-30.36,2.78-64.84-2.67-92.1-16.62c1.76-15.65,3.69-31.31,5-47c-11.59-3.48-39.29-26.62-42-27c-6.81-.95-32.33,19.93-40.66,21.78c-11.7,2.61-42.65-64.85-45.99-76c-1.18-3.94-5.16-14.14-2.35-17.78c1.6-2.07,38.19-20,40-20Zm237-94c-11.04-14.65-26.67-23.97-44.02-29.4c-97.34-30.43-169.37,78.97-106.65,159.16c9.87,12.61,26.41,22.85,41.28,28.32C772.65,814.43,851.38,698.48,786,621Z"/>' +
    "</svg>",
  leave:
    '<path d="M13 4h6a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-6M11 16l4-4-4-4M15 12H3"/>',
  camera:
    '<circle cx="12" cy="12" r="7"/><circle cx="12" cy="12" r="2"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3"/>',
  tools:
    '<path d="M15 5a4.5 4.5 0 0 0-6 6.2L3.5 16.7a1.8 1.8 0 0 0 2.5 2.5L11.5 13.7A4.5 4.5 0 0 0 17.7 8L15 10.7l-2-2L15.7 6A4.5 4.5 0 0 0 15 5Z"/>',
  retreat:
    RING_GLYPH_OPEN +
    '<path d="M9 3 2 10l7 7v-5h5a4 4 0 0 1 0 8h-3v3h3a7 7 0 0 0 0-14H9Z"/>' +
    "</svg>",
  spawn: '<path d="M5 21V4m1 1h12l-3 4 3 4H6"/>',
  attack:
    '<path d="m5 4 14 16M19 4 5 20M3 6l3-3 3 3M15 18l3 3 3-3M3 18l3 3 3-3M15 6l3-3 3 3"/>',
  fleet:
    '<path d="M3 15h18l-3 5H7l-4-5ZM6 12l6-9 6 9M12 3v12M2 21c2 1 3 1 5 0s3-1 5 0 3 1 5 0 3-1 5 0"/>',
  transfer: '<path d="M4 7h15l-3-3M20 17H5l3 3M19 7l-3 3M5 17l3-3"/>',
  // Filled send glyph: Wikimedia Commons "Send icon.svg" (CC0 1.0, by Lycopene579), recolored to currentColor.
  send:
    '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">' +
    '<path fill-rule="evenodd" d="M16.6915026,12.4744748 L3.50612381,13.2599618 C3.19218622,13.2599618 3.03521743,13.4170592 3.03521743,13.5741566 L1.15159189,20.0151496 C0.8376543,20.8006365 0.99,21.89 1.77946707,22.52 C2.41,22.99 3.50612381,23.1 4.13399899,22.8429026 L21.714504,14.0454487 C22.6563168,13.5741566 23.1272231,12.6315722 22.9702544,11.6889879 C22.8132856,11.0605983 22.3423792,10.4322088 21.714504,10.118014 L4.13399899,1.16346272 C3.34915502,0.9 2.40734225,1.00636533 1.77946707,1.4776575 C0.994623095,2.10604706 0.8376543,3.0486314 1.15159189,3.99121575 L3.03521743,10.4322088 C3.03521743,10.5893061 3.34915502,10.7464035 3.50612381,10.7464035 L16.6915026,11.5318905 C16.6915026,11.5318905 17.1624089,11.5318905 17.1624089,12.0031827 C17.1624089,12.4744748 16.6915026,12.4744748 16.6915026,12.4744748 Z"/>' +
    "</svg>",
  // Question bubble for requests, drawn as a line glyph like the other actions.
  request:
    '<path d="M5 3h14a3 3 0 0 1 3 3v8a3 3 0 0 1-3 3h-7l-4.5 4v-4H5a3 3 0 0 1-3-3V6a3 3 0 0 1 3-3z"/>' +
    '<path d="M10 7.8a2 2 0 1 1 3 1.7c-.6.3-1 .7-1 1.4v.4M12 13.9h.01"/>',
  city: '<path d="M3 21h18M5 21V10l7-6 7 6v11M9 21v-6h6v6"/>',
  farm: '<path d="M3 20h18M4 20v-8l4-5 4 5v8M4 13h8M15 20V8a2 2 0 0 1 4 0v12M14 14h6"/>',
  nuke: '<path d="M12 22V9M9 12l3-3 3 3M12 9l5-5M17 4h4M17 4V1M7 21h10M9 18h6"/>',
  upgrade:
    '<path d="m12 3 2.2 5.2L20 10l-4.4 3.5L17 19l-5-3-5 3 1.4-5.5L4 10l5.8-1.8L12 3Z"/>',
  anchor:
    '<path d="M12 3v12M8 7a4 4 0 1 0 8 0M4 13a8 8 0 0 0 16 0M2 13h4M18 13h4"/>',
  trading_post:
    '<path d="M3 21V9l6 3V8l6 4V5h6v16H3ZM17 8h1M17 11h1M7 17h2M12 17h2M17 17h1"/>',
  military_camp:
    '<path d="M12 21V8M12 12c-4-1-6-3-6-6 4 0 6 2 6 6ZM12 15c4-1 6-3 6-6-4 0-6 2-6 6Z"/>',
  defense: '<path d="M12 22s8-4 8-11V5l-8-3-8 3v6c0 7 8 11 8 11Z"/>',
  range:
    '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="2"/><path d="M12 2v3M22 12h-3M12 22v-3M2 12h3"/>',
  speed: '<path d="m13 2-3 8h7l-6 12 1-9H5l8-11Z"/>',
  discount:
    '<circle cx="7" cy="7" r="2"/><circle cx="17" cy="17" r="2"/><path d="m19 5-14 14"/>',
  clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
  construction:
    '<path d="M14.5 6.5a5 5 0 0 0-6.7 6.7L3 18l3 3 4.8-4.8a5 5 0 0 0 6.7-6.7L14 13l-3-3 3.5-3.5Z"/>',
  lock: '<rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 1 1 8 0v3"/>',
  dice: '<rect x="4" y="4" width="16" height="16" rx="3"/><path d="M9 9h.01M15 9h.01M12 12h.01M9 15h.01M15 15h.01"/>',
  globe:
    '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
  solo: '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
  // Filled silhouettes for the command ring.
  ring_attack:
    RING_GLYPH_OPEN +
    '<path fill-opacity=".72" transform="rotate(-45 12 12)" d="M12 1.5l2.5 4v3h-5v-3L12 1.5ZM8 15.5h8V18h-2.5v2h.5v2h-4v-2h.5v-2H8Z"/>' +
    '<path transform="rotate(45 12 12)" d="M12 1.5l2.5 4v8H16V16h-2.5v4h.5v2h-4v-2h.5v-4H8v-2.5h1.5v-8L12 1.5Z"/>' +
    "</svg>",
  ring_fleet:
    RING_GLYPH_OPEN +
    '<path d="M12 2.5v10H4L12 2.5ZM2 14h20l-4 4.5H6L2 14Z"/>' +
    '<path fill-opacity=".72" d="m14 5.5 5.5 7H14Z"/>' +
    '<path d="M4 21c2.7 1.3 5.3-1.3 8 0s5.3-1.3 8 0" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>' +
    "</svg>",
  ring_nuke:
    RING_GLYPH_OPEN +
    '<path d="M16.75 20.23a9.5 9.5 0 0 1-9.5 0L10 15.46a4 4 0 0 0 4 0l2.75 4.77ZM2.5 12a9.5 9.5 0 0 1 4.75-8.23L10 8.54A4 4 0 0 0 8 12H2.5ZM16.75 3.77A9.5 9.5 0 0 1 21.5 12H16a4 4 0 0 0-2-3.46l2.75-4.77Z"/>' +
    '<circle cx="12" cy="12" r="2.4"/>' +
    "</svg>",
  ring_alliance:
    RING_GLYPH_OPEN +
    '<path fill-opacity=".72" d="m1.75 9.75 3.5-5L8 6.75 4.5 12l-2.75-2.25Zm20.5 0-3.5-5L16 6.75 19.5 12l2.75-2.25Z"/>' +
    '<path d="M8.8 7.65h2.1l-1.5 1.5a2.05 2.05 0 0 0 2.9 2.9l1.3-1.3 4.25 4.15-3.05 3.1a2 2 0 0 1-2.8 0L6 12.6l2.8-4.95Z"/>' +
    '<path d="m10.45 10.1 2.2-2.25a1.6 1.6 0 0 1 1.4-.45l1.35.25 3.1 4.75-1.1.95-3.8-3.7-2.05 1.55a.8.8 0 0 1-1.1-1.1Z"/>' +
    "</svg>",
  ring_pending:
    RING_GLYPH_OPEN +
    '<path fill-rule="evenodd" d="M12 2.5a9.5 9.5 0 1 0 0 19 9.5 9.5 0 0 0 0-19ZM11 6h2v5.4l3.6 2.5-1.2 1.7L11 12.5V6Z"/>' +
    "</svg>",
  ring_send:
    RING_GLYPH_OPEN +
    '<path d="m12 2.5 6 6h-4.5v7h-3v-7H6l6-6Z"/>' +
    '<path fill-opacity=".72" d="M3 15h2.5v4h13v-4H21v6.5H3Z"/>' +
    "</svg>",
  ring_request:
    RING_GLYPH_OPEN +
    '<path d="M10.5 2.5h3v7H18l-6 6-6-6h4.5v-7Z"/>' +
    '<path fill-opacity=".72" d="M3 15h2.5v4h13v-4H21v6.5H3Z"/>' +
    "</svg>",
  ring_spawn:
    RING_GLYPH_OPEN +
    '<path d="M5 3h2.5v1h13L17 8.5l3.5 4.5h-13v6H12v2H3v-2h2V3Z"/>' +
    "</svg>",
  ring_build:
    RING_GLYPH_OPEN +
    '<path transform="rotate(35 12 12)" d="M4 5h11l4 3v3h-5l-2-2h-1v11a1.5 1.5 0 0 1-3 0V9H4V5Z"/>' +
    "</svg>",
  ring_upgrade:
    RING_GLYPH_OPEN +
    '<path d="m12 3 8 6v3.5l-8-6-8 6V9l8-6Zm0 9 8 6v3.5l-8-6-8 6V18l8-6Z"/>' +
    "</svg>",
  ring_emote:
    RING_GLYPH_OPEN +
    '<path fill-rule="evenodd" d="M8 3.5h8a6 6 0 0 1 6 6v3a6 6 0 0 1-6 6h-5l-5 3v-3.35a6 6 0 0 1-4-5.65v-3a6 6 0 0 1 6-6ZM8.5 8.25a1.25 1.25 0 1 0 0 2.5 1.25 1.25 0 0 0 0-2.5Zm7 0a1.25 1.25 0 1 0 0 2.5 1.25 1.25 0 0 0 0-2.5Zm-8 4.25h9a4.75 4.75 0 0 1-9 0Z"/>' +
    "</svg>",
  ring_pin:
    RING_GLYPH_OPEN +
    '<path transform="rotate(35 12 12)" d="M7 3h10v2.5h-1.5v5L18 13v2h-4.5v5L12 22l-1.5-2v-5H6v-2l2.5-2.5v-5H7V3Z"/>' +
    "</svg>",
};

export interface BuildingAtlasManifest {
  atlas?: string;
  cell_size: number;
  atlas_width: number;
  atlas_height: number;
  sprites: Record<string, Record<string, { x: number; y: number }>>;
}

export type AssetResolver = (relativePath: string) => string;

const defaultAssetResolver: AssetResolver = (relativePath) =>
  "/assets/" + relativePath.replace(/^\/+/, "");

export function escapeAttr(value: unknown): string {
  return String(value ?? "").replace(/[&<>"']/g, (character) => {
    return (
      { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[
        character
      ] || character
    );
  });
}

export function renderHudIcon(
  name: string,
  cls = "sow-hud__action-icon",
  resolveAsset: AssetResolver = defaultAssetResolver,
): string {
  const glyph = HUD_GLYPHS[BUILDING_GLYPHS[name] || name];
  if (glyph) {
    if (glyph.startsWith("<svg")) {
      return glyph.replace('class="sow-hud__action-icon"', 'class="' + cls + '"');
    }
    return (
      '<svg class="' +
      cls +
      '" viewBox="0 0 24 24" fill="none" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><g class="sow-hud__glyph-ink">' +
      glyph +
      '</g><g class="sow-hud__glyph-face">' +
      glyph +
      "</g></svg>"
    );
  }
  if (!HUD_ICONS[name]) return "";
  return (
    '<img class="' +
    cls +
    '" src="' +
    escapeAttr(resolveAsset(HUD_ICONS[name])) +
    '" alt="" aria-hidden="true">'
  );
}

export function renderStatusIcon(
  kind: string,
  cls = "sow-hud__action-icon",
  resolveAsset: AssetResolver = defaultAssetResolver,
  badgeAtlasUrl?: string,
): string {
  const resolvedUrl = badgeAtlasUrl ?? resolveAsset("gameplay/icons/nameplate_status_atlas.webp");
  const idx = Math.max(0, STATUS_CELLS.indexOf(kind));
  const pos = (idx * 100) / Math.max(1, STATUS_CELLS.length - 1);
  const style =
    "background-image:url(&quot;" +
    escapeAttr(resolvedUrl) +
    "&quot;);" +
    "background-size:" +
    STATUS_CELLS.length * 100 +
    "% 100%;" +
    "background-position:" +
    pos +
    "% 0;";
  return (
    '<span class="' +
    cls +
    ' sow-hud__status-icon" style="' +
    style +
    '" aria-hidden="true"></span>'
  );
}

export function renderUnitIcon(
  name: string,
  cls = "sow-hud__building-icon",
  resolveAsset: AssetResolver = defaultAssetResolver,
): string {
  return (
    '<span class="' +
    cls +
    ' is-pixel-art" style="background-image:url(&quot;' +
    escapeAttr(resolveAsset("gameplay/units/" + name + ".webp")) +
    '&quot;);background-size:contain;background-repeat:no-repeat;background-position:center;" aria-hidden="true"></span>'
  );
}

export function renderBuildingAtlasSprite(
  kind: string,
  level: number,
  atlas: BuildingAtlasManifest | null | undefined,
  atlasUrl: string,
  cls: string,
): string {
  if (atlas && atlas.cell_size > 0 && atlas.atlas_width > atlas.cell_size && atlas.atlas_height > atlas.cell_size) {
    const clamped = Math.max(1, Math.trunc(level || 1));
    let sprite: { x: number; y: number } | undefined;
    if (kind === "Farm") {
      sprite = atlas.sprites["Farm"]?.[String(clamped)]
        ?? atlas.sprites["TradingPost"]?.[String(clamped)]
        ?? atlas.sprites["City"]?.["1"];
    } else if (kind === "Tower" && clamped >= 5) {
      sprite = atlas.sprites["Tower"]?.[String(clamped)]
        ?? atlas.sprites["SAM"]?.[String(clamped - 4)]
        ?? atlas.sprites["Tower"]?.["4"];
    } else {
      const group = atlas.sprites[kind];
      sprite = group?.[String(clamped)] ?? (group ? group[String(Object.keys(group).length)] : undefined);
    }
    if (sprite) {
      const bgW = ((atlas.atlas_width * 100) / atlas.cell_size).toFixed(4);
      const bgH = ((atlas.atlas_height * 100) / atlas.cell_size).toFixed(4);
      const posX = ((sprite.x * 100) / (atlas.atlas_width - atlas.cell_size)).toFixed(4);
      const posY = ((sprite.y * 100) / (atlas.atlas_height - atlas.cell_size)).toFixed(4);
      const style =
        "background-image:url(&quot;" +
        escapeAttr(atlasUrl) +
        "&quot;);" +
        "background-size:" +
        bgW +
        "% " +
        bgH +
        "%;" +
        "background-position:" +
        posX +
        "% " +
        posY +
        "%;";
      return '<span class="' + cls + ' is-pixel-art" style="' + style + '" aria-hidden="true"></span>';
    }
  }
  return renderHudIcon(kind.toLowerCase(), cls);
}
