# Building upgrades

**Status:** approved rules for this implementation.  
**Scope:** normal matches. Starting resources, base income, territory income, and game speed stay as they are.

## The simple rule

Tap one of your buildings to see its current benefit, the next benefit, its price, and how long the work takes. Tap **Upgrade** to improve that exact building. A new building is still placed separately.

During construction, the last finished level keeps working. The new benefit starts when the timer ends. Only one upgrade can run on a building at a time.

Foundation prices and their existing growth stay the same. An upgrade costs 85% of what the next new building of that kind would cost at the same owned-building count. Finished Factory perks can lower that upgrade price by 5% each, up to 25%; this discount does not lower Factory upgrade prices.

Factories at Manufactory level or higher also shorten construction by 5% each, up to 25% total.

## Benefits by building

Only building benefits are halved. Starting resources and income remain unchanged.

| Building | Benefit from each finished level | Special levels |
| --- | --- | --- |
| City | +1,250 troop capacity, +6.25 troops/second, +1 gold/second | Camp gives 1 farm plot; Hamlet 2; Village 4 and unlocks Factories; Town 8 and, with a Harbor, unlocks Trade Ships; City 16 and, with a Port, unlocks Warships; Metropolis 32 and unlocks nuclear launches. |
| Port | +1 boat slot, +1% boat speed, +12.5 troops/second, +1 gold/second | Harbor unlocks Trade Ships with a Town; Port unlocks Warships with a City. Speed from all finished Port levels stops at +30%. |
| Factory | +2 gold/second for every finished Factory level | Manufactory speeds construction; Factory lowers other building upgrade prices; Industrial Complex increases Trade Ship income. Each factory at each milestone adds 5%; those three bonuses stop at 25% total. |
| Bunker | +5% enemy attack losses near this bunker for each finished level | Range starts at 14 and grows by 2 at each level, stopping at 20. Citadel can intercept nuclear bombs. The local attack-loss bonus from all bunkers stops at 50%. |
| Farm | +6.25 troops/second for each finished Farm level | Three levels: Cultivated Plot, Farm, Irrigated Fields. |

City farm plots are a limit, not an order to fill every tile. Farms can be placed only on empty lowland tiles you own. The game stores a record for each farm that is actually built; it does not create a second data entry for every tile on the map.

### Boat example

- With no Port, you can have one boat active or being built.
- Each finished Port level adds one more boat slot. A Dock adds one; upgrading it to a Wharf adds one more.
- Each finished Port level adds 1% speed. Four hundred Port levels give 400 extra boat slots, but speed still stops at 30%.
- Each finished Port level also adds 12.5 troops/s and 1 gold/s.
- The Port card shows boats in use, total slots, and the current speed bonus.

## Names and order

| Building | Upgrade order |
| --- | --- |
| City | Camp → Hamlet → Village → Town → City → Metropolis |
| Port | Dock → Wharf → Harbor → Port → Megaport |
| Factory | Workshop → Manufactory → Factory → Industrial Complex |
| Bunker | Watchpost → Watchtower → Bastion → Citadel |
| Farm | Cultivated Plot → Farm → Irrigated Fields |

## Touch and visuals

The browser JavaScript HUD owns the building card and buttons. The map and building markers remain in Rust. Blade's GPU text-and-emoji renderer draws building icons and level labels from the existing emoji atlas. A completed upgrade gets a brief 300 ms sparkle; the effect adds no new sprite sheet or per-tile map storage.

The card uses a large touch target and can scroll on a small screen. It shows the active level, what the next level adds, the price, and the remaining or expected time. It does not put a permanent explanation beneath the building buttons.

## Manual play guide

Use a normal match and try the same actions on a phone-sized screen if available.

1. Build a City. Tap it and check that the card shows its name, level, benefit, next name, price, and time. Upgrade it; its name and new benefit should change only when the work finishes.
2. Build a Farm on an empty lowland tile you own. It should add troops over time. A highland, mountain, water, occupied, or unowned tile should not offer a Farm action. Try to exceed the number of farm plots from your Cities; the extra Farm should not be accepted.
3. Send one transport without a Port. It should work. While that boat is active, a second launch should be unavailable. Finish a Dock; one more boat should become available. Upgrade the Dock; capacity should gain one more slot and boat speed should rise by another 1%.
4. Select a Port and check the card's boat count and current speed. With many finished Port levels, speed should never exceed 30%.
5. Upgrade a Factory to each level. Confirm the displayed gold income, shorter construction, lower prices for other upgrades, and increased Trade Ship income appear only after each level finishes. Each percentage bonus should stop growing at 25%.
6. Upgrade a Bunker. Confirm its range grows by two per level, its nearby defense benefit stacks only up to 50%, and a finished Citadel can intercept an incoming nuclear bomb.
7. Upgrade a City to Metropolis. Nuclear launches should remain unavailable before Metropolis and become available after it finishes.

### What the pace should feel like

The first minutes still use the current starting resources and base income. Buildings add less income than before, so gold and troops should not explode just because several low-level buildings were placed. Early choices should be clear: grow a City for capacity and unlocks, add Farms for troops, add a Factory for money, or invest in a Port or Bunker. Higher tiers should feel like a visible commitment during a match, while each tap and upgrade stays quick to understand.

Check the same save near minutes 2, 6, and 10. Write down gold, troop income, completed building levels, active boats, and how often a fight ends. This distinguishes a slow opening from an upgrade that simply costs too much.
