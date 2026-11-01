# Shadows of War

Follow the workspace rules in `../AGENTS.md`. Keep this file limited to game and release behavior that only applies here.

## Production checks and recovery

- `./sow p` is the only Web/backend/relay deployment path. Read the current `sow-dist/src/prod.rs`; it owns stage order and recovery. `./sow help` and the launcher dispatch are the source of truth for supported commands. Do not add or remove commands to match stale documentation.
- When a task authorizes production work, that includes the pipeline's normal build, staging, activation, certificate renewal, health checks, public verification, and safe retries. Do not ask again for those steps and never replace them with manual SSH, file copies, or service restarts.
- Before starting, check whether a production run is already active. Do not stop, duplicate, or overlap it; continue when it finishes.
- On failure, use the log to find the underlying command and numbered stage. A cache message, warning, optional test skip, or the wrapper's label is not itself a failure.
- Compare the failed expectation with the accepted game behavior. Fix incorrect game code, asset source, or publishing flow at its owner. For a broken public reference, keep the asset and reference, trace where publication dropped it, repair that path, then rerun the check and the full pipeline.
- Change a check only when evidence proves its criterion is invalid; keep the behavior it was meant to protect covered. Never delete features, assets, or assertions simply to pass.
- Completion requires the pipeline's own final health and public verification to succeed. Local builds or isolated checks can diagnose a failure; they do not replace the pipeline.
- Android publishing is owner-run only. `./sow p` does not authorize Play uploads.

## Game-specific invariants

- Match return order is server confirmation, rewards, every gained level, then guidance.
- Campaign episode translations stay locked until the owner marks that episode finished. Add its ID to `APPROVED_CAMPAIGN_TRANSLATION_EPISODES` only after that approval.
- Before map or thumbnail work, inspect `assets/maps/SOURCES.toml`, `assets/map_sources/thumbnail_frames.json`, and the map generation code. Thumbnail work must not change playable map data.
- Keep live communication separate from betrayal and archived recordings when changing nameplates, emotes, hero sheets, or replay compatibility.
