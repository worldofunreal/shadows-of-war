# Shadows of War agent rules

- Before changing match return, rewards, level celebrations, menu guidance or Campaign Studio, preserve the required order: server confirmation, rewards, every gained level, then guidance. Source presence alone is not proof of working behavior; check the behavior.

- Campaign episode translations stay locked until the owner explicitly marks that episode finished. Add its ID to APPROVED_CAMPAIGN_TRANSLATION_EPISODES only after that approval; an episode end step is not approval.
- Before map or thumbnail work, inspect assets/maps/SOURCES.toml, assets/map_sources/thumbnail_frames.json, and the map generation code. A thumbnail change must not alter playable map data.

- Before changing nameplates, player emotes, hero sheets or replay-format compatibility, keep live communication separate from betrayal and archived recordings.
