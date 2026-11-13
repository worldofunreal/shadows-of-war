# Shadows of War

Workspace rules: `../AGENTS.md`. This file holds only what is specific to this game.

## Release

- `./sow p` (production) and `./sow ptr` (`ptr.shadowsofwar.io`, same IONOS host) are the only deploy paths for web, backend and relay. `sow-dist/src/prod.rs` and `sow-dist/src/prod_relay.rs` own stage order and recovery; `sow-dist/src/site.rs` owns what differs between the two sites. Both hold the production lock, so they never overlap. `./sow help` lists the supported commands.
- A run builds from a snapshot of the working tree taken at start (`dist/.source`), uncommitted changes included. Edits made during the run ship in the next one. `release.json` records HEAD as `git`, plus `dirty` and `dirty_files`. Run from the checkout that holds the code to ship; a worktree behind it ships older code.
- Check for an active run before starting one. Never stop, duplicate or overlap it.
- A deploy is done only when the pipeline's own stage 8 public verification passes.
- `sow-dist` and the test suites are deletion-only: a new check, stage or test needs the owner's approval first.
- Android publishing (`./sow a`) is owner-run only and refuses a dirty tree. `./sow p` does not authorize Play uploads.

## Game invariants

- Match return order: server confirmation, rewards, each gained level, then guidance.
- Do not translate a campaign episode until the owner marks it finished.
- Before map or thumbnail work, read `assets/maps/SOURCES.toml`, `assets/map_sources/thumbnail_frames.json` and the map generation code. Thumbnail work must not change playable map data.
- Nameplates, emotes and hero sheets: keep live communication separate from betrayal and archived recordings.
