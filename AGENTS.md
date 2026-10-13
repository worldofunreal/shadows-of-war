# Shadows of War agent rules

- Before changing match return, rewards, level celebrations, menu guidance or Campaign Studio, read `docs/player-journeys.md`. Reuse its named owners and preserve the required order: server confirmation, rewards, every gained level, then guidance. Read its acceptance evidence separately from the contract; source presence alone is not proof of working behavior.

- Campaign episode translations stay locked until the owner explicitly marks that episode finished. Add its ID to APPROVED_CAMPAIGN_TRANSLATION_EPISODES only after that approval; an episode end step is not approval.
- Before map or thumbnail work, inspect assets/maps/SOURCES.toml, assets/map_sources/thumbnail_frames.json, and the map generation code. A thumbnail change must not alter playable map data.
