/**
 * WouID SDK loader — Shadows of War site edition.
 *
 * Single source of truth is the npm package @worldofunreal/id.
 * This file is a LOADER, not a fork. WOU_ID_VERSION is a major range on
 * purpose: the newest 2.x loads without a deploy per SDK patch.
 * No auth logic lives here on purpose.
 */
const WOU_ID_VERSION = "2";
const WOU_ID_CDNS = [
  `https://cdn.jsdelivr.net/npm/@worldofunreal/id@${WOU_ID_VERSION}/dist/index.js`,
  `https://unpkg.com/@worldofunreal/id@${WOU_ID_VERSION}/dist/index.js`,
];

let sdk = null;
for (const url of WOU_ID_CDNS) {
  try {
    sdk = await import(url);
    break;
  } catch {
    // CDN unreachable or version not published yet — try the next one.
  }
}

if (!sdk || typeof sdk.WouAuthClient !== "function") {
  console.error("[wou] identity SDK failed to load — sign-in is unavailable");
} else {
  window.wouAuth = new sdk.WouAuthClient("shadows_of_war");
  window.dispatchEvent(new CustomEvent("wou:auth-sdk-ready"));
}
