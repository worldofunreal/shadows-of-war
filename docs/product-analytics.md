# Product analytics

## One event contract, platform-owned reporting

Game events enter through `sow-web/shell/experience_events.js`. That module validates the shared event names and safe properties, then routes each event to exactly one destination. Partner data is never forwarded to the Shadows of War event endpoint.

| Channel | Event destination | What can be compared |
| --- | --- | --- |
| Owned website / browser game | `/api/event`, only after the website analytics choice is accepted | Shared menu, lobby, match, load, and tutorial events; first-party D1/D3/D7 for identified consenting accounts |
| Poki | Poki `measure()` | Same event moments; tutorial episode and step progress are encoded in its category / what / action fields |
| Jest | Jest `captureEvent()` | Same event names and bounded properties in Jest's custom-event report |
| CrazyGames | CrazyGames SDK lifecycle and loading methods | SOW reports loading and gameplay lifecycle. The SDK also has a whole-game completion-percentage method, but no arbitrary custom-event funnel. |
| Android / native | No first-party event capture | Android first-party analytics remains disabled; use platform-owned reporting only |

The same source action and event definition are used where the platform supports custom events. CrazyGames' SDK does not expose a general custom-event method, so its dashboard cannot provide tutorial-step or menu-event detail. Its whole-game completion-percentage method is not a substitute: SOW has no finite overall completion point, so reporting a tutorial or campaign episode as game completion would misstate progress. Do not treat an unavailable platform metric as zero, and do not combine the platforms' player records.

## Shared event groups

| Group | Events |
| --- | --- |
| Entry and loading | `landing_visit`, `shell_loaded`, `play_now_click`, `boot_start`, `boot_route_decision`, `load_stage`, `match_loading_start` |
| Menu actions | `menu_quick_match`, `menu_join_attempt`, `menu_password_join_attempt`, `menu_code_join_attempt`, `menu_custom_create`, `menu_single_player_start`, `menu_campaign_start`, `menu_campaign_open`, `menu_lobby_browser_open`, `menu_custom_create_open`, `menu_leader_confirm` |
| Lobby and match | `matchmaking_joined`, `lobby_joined`, `lobby_join_failed`, `match_started_client`, `match_ended_client`, `match_exit` |
| Tutorial | `tutorial_start`, `tutorial_step` (`start` / `complete`), `tutorial_dialog_choice`, `tutorial_exit_early`, `campaign_episode_complete` |

`tutorial_step` carries the authored episode ID, stable step ID, bounded index, and `start` or `complete`. A new explicit `tutorial_exit_early` also carries that step identity with action `fail`; older queued episode-only exits remain accepted. The site report groups those explicit exits with the step, while tab closes remain unknown. Unknown events and extra properties are rejected. `tutorial_objective_complete` remains accepted for already-queued older events but is no longer emitted by the current tutorial.

## Website retention

Retention is first-party website data, not a cross-platform estimate. It includes only accounts with an identified account ID that accepted website analytics. Each account is counted once in its first-seen cohort and at most once per D1/D3/D7 return set. D1, D3, and D7 mean activity on exactly the first, third, or seventh UTC calendar day after the cohort day; a window is reported only after that entire return day has elapsed.

The corrected retention series uses versioned cohort keys. The earlier series placed returns under the return date instead of the first-visit cohort date; it remains untouched for historical inspection and is excluded from corrected reports. The report marks immature windows as unavailable instead of showing a misleading zero rate. Tutorial reporting counts reached steps, completions, and explicit exits as events, not unique players.
