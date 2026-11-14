# Shadows of War

Workspace rules: `../AGENTS.md`. This file holds only what is specific to this game.

## Release

- `./sow p` (production) and `./sow ptr` (`ptr.shadowsofwar.io`, same IONOS host) are the only deploy paths for web, backend and relay. `sow-dist/src/prod.rs` and `sow-dist/src/prod_relay.rs` own stage order and recovery; `sow-dist/src/site.rs` owns what differs between the two sites. Both hold the production lock, so they never overlap. `./sow help` lists the supported commands.
- A run builds from a snapshot of the working tree taken at start (`dist/.source`), uncommitted changes included. Edits made during the run ship in the next one. `release.json` records HEAD as `git`, plus `dirty` and `dirty_files`. Run from the checkout that holds the code to ship; a worktree behind it ships older code.
- Check for an active run before starting one. Never stop, duplicate or overlap it.
- A deploy is done only when the pipeline's own stage 8 public verification passes.
- A live match is never a generic reason to stop work, ask the owner to wait, or kill a relay. Each match runs in an isolated child process and survives a relay-control restart. Relay-only deployment should pause new admissions and continue existing matches.
- Server/rules releases have a separate compatibility requirement: each match records its `simulation_rules_id`, and a server built for a different ID cannot verify that match's replay/rewards. Do not misstate this as the relay dying, and do not remove the drain while it is the only protection. The target fix is version-aware replay verification that keeps the matching immutable server verifier available until its matches settle, so backend releases also proceed during live matches without losing rewards. Do not ask the owner to wait; continue with that compatibility fix when this limitation blocks a requested release.
- `sow-dist` and the test suites are deletion-only: a new check, stage or test needs the owner's approval first.
- Android publishing (`./sow a`) is owner-run only and refuses a dirty tree. `./sow p` does not authorize Play uploads.

## Focus and delegation

- The primary agent spends its work on the user's current product goal and its hardest necessary engineering decisions. Do not start or continue secondary work that does not materially advance that goal; drop it.
- Delegate necessary repetitive execution, such as running established checks, collecting their output, and monitoring an already-authorized build or deployment, to one subagent with a bounded task and clear result. Do not delegate product decisions, root-cause analysis, the core fix, or final review.
- The primary agent owns the diagnosis, design, core implementation, review and integration, safe recovery, and final verification. Delegation never transfers responsibility to the user or a subagent.
- Preserve the current branch and all concurrent work. Do not create branches, parallel systems, or extra infrastructure for convenience.
- Do not repeat a successful check without a code, input, or environment change that makes the result relevant again. Do not present a checklist or audit as completion when the requested behavior remains unfinished.
- When a command fails, inspect its actual failure, correct the underlying cause, and retry through the owning workflow. Keep progressing through independent work; ask the user only for a product decision or external authority that cannot be inferred or recovered safely.
- Report completion cleanly and digestibly: behavior changed, player-facing effect, next options, and a brief pass confirmation without dumping raw test logs or crate-by-crate counts unless something failed. Distinguish measured outcomes from expected retention impact.

## Game invariants

- Match return order: server confirmation, rewards, each gained level, then guidance.
- Do not translate a campaign episode until the owner marks it finished.
- Before map or thumbnail work, read `assets/maps/SOURCES.toml`, `assets/map_sources/thumbnail_frames.json` and the map generation code. Thumbnail work must not change playable map data.
- Nameplates, emotes and hero sheets: keep live communication separate from betrayal and archived recordings.
