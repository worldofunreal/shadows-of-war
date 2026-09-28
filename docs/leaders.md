# Leaders roster

**Shipped in-game today: 12 leaders / 12 civilizations**, one-to-one
(`Leader::ALL` and `Civilization::ALL` in `sow-data/src/leaders.rs`; the
in-game roster is the source of truth, not this folder).

[leaders/README.md](leaders/README.md) is the **production roadmap dossier**
(~1,270 historical leader entries in 12 regional files, chronological order)
used to source art and copy as the roster grows. Entries there are *planned
candidates*, not playable leaders. Portrait batches land in the art pipeline
before being wired into `sow-data`.

Regenerate the regional files from `leaders/roster-source.md`:
`python3 scripts/split_leaders_roster.py`
