/**
 * WouID SDK loader — Shadows of War site edition.
 *
 * Single source of truth is the npm package @worldofunreal/id.
 * This file is a PINNED LOADER, not a fork: to upgrade, bump
 * WOU_ID_VERSION below (must be a published version) and deploy.
 * No auth logic lives here on purpose.
 */
const WOU_ID_VERSION = '2.0.0';
const WOU_ID_CDNS = [
  `https://cdn.jsdelivr.net/npm/@worldofunreal/id@${WOU_ID_VERSION}/dist/index.js`,
  `https://unpkg.com/@worldofunreal/id@${WOU_ID_VERSION}/dist/index.js`,
];

let sdk = null;
for (const url of WOU_ID_CDNS) {
  try {
    sdk = await import(url);
    break;
  } catch (_) {
    // CDN unreachable or version not published yet — try the next one.
  }
}

if (!sdk || typeof sdk.WouAuthClient !== 'function') {
  console.error('[wou] identity SDK failed to load — sign-in is unavailable');
} else {
  window.wouAuth = new sdk.WouAuthClient('shadows_of_war');
  window.dispatchEvent(new CustomEvent('wou:auth-sdk-ready'));
}
