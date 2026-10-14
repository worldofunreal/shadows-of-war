# Tutorial cinematic streaming — Fase 1 (handoff de estado)

Owner asked: cut the tutorial cinematic's boot cost and stop it blocking the instant-web loader.
Phase 1 = shrink + streamable file, poster, dual sources, loader gate, pipeline lock. Phase 2 (HLS chunks + 3 qualities) deliberately NOT started.

## Verified diagnosis (measured, not assumed)

- Old asset: `assets/campaign/boudica/intro.webm` 9.521.555 B, VP9 1280x720 30fps, Opus stereo, 1.718 kb/s, 44,3 s.
- Its `Cues` index sat at byte 9.521.341 (100% into the file) → browser needed a second range request to the tail; first cluster 400.304 B → 392 KB before first frame.
- `sow-campaign-view.js` used `<video preload="auto">`; boot payload for a new player: WASM 10,9 MB + shell HTML 1,55 MB + menu music 3,2 MB + video 9,5 MB ≈ 25 MB on one origin.
- Regression landed in `bece7b46` (2026-10-10): same commit added the 9,1 MB video AND `preload="auto"`.
- Network itself was fine: CF `HIT`, TTFB 0,18 s, `Accept-Ranges` OK, CORS `*` on `/assets/`. `cf-cache-status` alternated MISS by POP. `cache-control: max-age=14400` came from Cloudflare's zone default browser TTL (origin sends none).

## Phase 1 — DONE and LIVE in production (release `0.1.3-aed843c8ac49`, `./sow p` completed 8/8)

- New files in `assets/campaign/boudica/` (old 9,5 MB file deleted):
  - `intro.720p.555d0500.webm` 5.563.674 B (VP9 800k/max1400k/buf2400k, GOP 60, `-cluster_time_limit 2000 -cues_to_front 1`, Opus mono 48k)
  - `intro.720p.555d0500.mp4` 5.412.982 B (H264 High 900k/max1500k/buf3000k, GOP 60, `+faststart`, AAC mono 64k)
  - `intro.720p.555d0500.poster.webp` 34.948 B (frame 0; YAVG 62,5 → not black)
  - hash stem = first 8 of sha256 of the source master.
- `assets/campaign/boudica.triggers.json`: entry step `boudica_rise_of_the_iceni_intro` `video_src` now points at the new `.webm`.
- Verified structure: WebM `Cues` at byte 697 (0% of file), first cluster 193.719 B → ~190 KB to start; MP4 `moov`@32 before `mdat`@49808. First 256 KB decodes standalone.
- Live headers confirmed: `cache-control: public, max-age=31536000, immutable` + `access-control-allow-origin: *` + `cf-cache-status: HIT`; head-194 KB `total 0,91 s`; full file `3,44 s`; poster `0,66 s`.
- `sow-web/shell/sow-campaign-view.js`: `<video playsinline preload="none" fetchpriority="low">`; poster+sources derived from the authored stem (WebM first, MP4 fallback); loader gate that holds the cinematic until `sow:loader-ready` / `sow:loader-cycle-ready` when `#web-loader` is visible.
- `sow-dist/deploy/freebsd/conf.d/shadowsofwar.io.conf`: nested `location ~* ^/assets/campaign/.+\.(webm|mp4|poster\.webp)$` inside `^~ /assets/` with immutable cache + own ACAO (a sibling regex location would have been dead config, `^~` stops the regex phase). `nginx -t` passed on host.
- `sow-dist/src/main.rs` `validate_campaign_assets` → new `validate_cinematic_media`: every cinematic needs `.webm` + `.mp4` + `.poster.webp`, ≤6 MB per rendition, ≤128 KB poster, and index-at-front checked by reading the first 1 MB (WebM `Cues` magic within first 16 KB; MP4 `moov` before `mdat`).
- Tests: `main_menu.interaction.test.js` 206/206 (updated the `preload="auto"` assertion, extended the Boudica asset test to require siblings+size, added the loader-gate/sources test). Rust `campaign_cinematic_*` 4/4.
- No Cloudflare purge needed (new hashed filenames = new cache key; old name is gone).

## Phase 1 — written, locally verified, NOT yet deployed (the last piece)

Chromium (system `/usr/bin/chromium`, `--autoplay-policy=document-user-activation-required`) against live prod proved the v1 leak:
`play()` was correctly held until ready@10831, but the `.webm` request STARTED at 10112 and pulled the whole 5.5 MB — Chrome's preload heuristics fetch as soon as `<source>` children exist, even with `preload="none"`.

Fix in the working tree (tested locally): the cinematic step only sets the POSTER; the `<source>` children are armed inside `attachCinematicMedia` only at the moment playback is really allowed. See `showCinematicPoster`, `attachCinematicMedia` (`cinematicMediaArmed`), `startCinematic` in `sow-campaign-view.js`. Test asserts `video.children` is empty while gated.

## Shared nameplate contract

The current nameplate, emote and recording contracts are documented in
`docs/nameplates-and-emotes.md`. The former core compilation and reference-test
failures are resolved: production log `production-20261013-233010.log` passed
all 247 core tests and completed the official publication through stage 8.

## Next steps for whoever picks this up

1. For subsequent cinematic changes, run `./sow p` (mandatory gate).
2. Re-trace in Chromium against production: assert the `.webm` request start timestamp is ≥ the `loader-ready` timestamp and that `#web-loader` visibility is the only precondition; poster still early (~8 s), video bytes only on play.
3. Phase 2 (not authorized yet): HLS fMP4 2-4 s chunks in 3 renditions under `/assets/campaign/boudica/`, `.m3u8` uncached + chunks immutable, player with hls.js/`canPlayType` fallback to the two existing renditions.

## Rules followed during this work

No manual deploy, no ssh activation, no scp, no symlink: everything went through `./sow p`. Manual ssh usage was read-only (`systemctl is-active`, `journalctl`, `readlink`, `cat override.conf`) to diagnose a failed pipeline stage. Trace scripts live in `/tmp/opencode/` (`fine.py`, `win.py`, `cineboot2.py`) — throwaway, not part of the repo.

## Boot-window follow-up (deployed 2026-10-14, release 0.1.3-cd7e9ef38ef2)

Re-audit replaced the planned "HLS phase 2" with the measured levers still on the
boot window (HLS rejected: one 44 s video already range-streamable with a
front-loaded index; it would need ffmpeg on a builder that has none, a ~100 KB
player lib in a zero-dependency shell, and new validators/cache rules).

1. `optimize_webp` (was Poki-only) + `compress_web_leaders` (400 KB cap) now run in
   every web and native-dev packaging: boot leader art 600-945 KB → 166-243 KB,
   release assets 136 MB → 75 MB. Poki ships only the MP4 (`copy_campaign_for_poki`,
   trigger `video_src` rewritten) — portal package −5.6 MB.
2. Menu music (3.2 MB) is `preload="none"`; fetching starts on `sow:wasm-bytes-ready`
   dispatched in the boot template right after the WASM response resolves.
3. The inline menu bundle is now `play/main_menu.js?v=<build>` with `<link
   rel=preload>` (self-hosted/native only; Poki/Jest/CrazyGames stay inline).
   HTML 1.55 MB → 822 KB; repeat visits pull 754 KB (162 KB over the wire, HIT).
4. `snippets/security-headers.conf` included in every asset/font `add_header`
   location (nginx drops inherited headers when a block declares its own).

Live Chromium trace: wasm res 90 ms → bundle 1.3 s → music request 2.3 s →
loader ready 9.4 s → cinematic bytes only at play. The CF edge may still show
pre-change cached copies of unchanged files (same bytes) until entries age out.

## Boot-window part 2 (deployed 2026-10-14, release 0.1.3-94d8bbd60f42)

1. Engine/glue/menu now serve the pipeline's own brotli files at the SAME URLs
   (`map $http_accept_encoding` + `try_files $uri.br $uri`; the .br body needs
   `types {}` + explicit `default_type`). WASM wire 4.75 MB -> 4.17 MB, all three
   `Cache-Control: public, max-age=31536000, immutable` at origin. The menu rule
   is nested inside `^~ /play/` (a top-level sibling is dead config).
2. `window.SOW_ASSET_TS` carries per-file content hashes for leaders/loader/
   mobile-nav/music; `assetUrl` and the music URL prefer them over `?v=<build>`,
   so unrelated deploys no longer re-download the 3.2 MB theme. nginx: `map $arg_v`
   -> immutable when a `?v=` is present, 10 min without (marketing pages keep
   refreshing). Test: asset_version_keys_match_the_urls_the_shell_builds.
3. Loader splash recompressed q72 (175/157 -> 143/127 KB); boot HTML 822 -> 742 KB.
4. Boot leader prefetch switched to `fetchPriority=low` (no WASM competition);
   match transitions keep `high`. Service-worker PRECACHE reduced to sow.svg (the
   webp copies were double-downloaded and are immutable at the edge anyway).

Verified live: br+immutable at origin for wasm/glue/menu, music key 2e301e77,
Chromium boot: wasm res 282 ms, music req 2.2 s (after sow:wasm-bytes-ready),
poster 7.3 s, loader ready 8.9 s, cinematic bytes 9.9 s.

Deferred (measured, not shipped): gzip_static rejected (4.78 MB vs own .br 4.07);
Rocket Loader verified harmless (4 KB, injected last, no script rewrites); the
4.5 MB of atlases embedded in the wasm (msdf 3.1 MB + emoji 0.7 + arabic 0.2 +
nameplate 0.13, ~2.06 MB of the .br) are only used by TextRenderer::new at
EnterGame (loader/engine.rs) -> extraction to lazy network loads is the next
-2.2 MB boot lever, parked because another session owns sow-render/text.
wasm-opt -Oz vs -O3 untested on the code half (~2.1 MB br).
