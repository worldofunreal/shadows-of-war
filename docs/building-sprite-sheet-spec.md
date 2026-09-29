# Building sprite atlas

The game loads `assets/gameplay/buildings/building_atlas.png` through Blade's
GPU image-and-text renderer. It contains one image for every building level.
The shared emoji atlas still draws text, construction rings, and the 300 ms
completion sparkle.

The checked-in art is a provisional generated sheet. Replacement art can use
the same cell order; no game logic or UI change is needed.

## Delivery format

- 8×4 grid, 128×128 pixels per cell, 1024×512 pixels overall.
- Row 1, columns 1–6: settlements; columns 7–8: empty.
- Row 2, columns 1–5: ports; columns 6–8: empty.
- Row 3, columns 1–4: factories; columns 5–8: empty.
- Row 4, columns 1–4: defenses; columns 5–7: farms; column 8: empty.
- Same camera angle, center point, scale, lighting direction, and margin in every cell.
- No text, level numbers, player colors, UI, shadows outside the cell, or baked glow.
- Use a neutral material palette. Rust adds owner rings and level badges.

## Cells

| Family | Levels |
| --- | --- |
| Settlement | Camp, Hamlet, Village, Town, City, Metropolis |
| Port | Dock, Wharf, Harbor, Port, Megaport |
| Factory | Workshop, Manufactory, Factory, Industrial Complex |
| Defense | Watchpost, Watchtower, Bastion, Citadel |
| Farm | Cultivated Plot, Farm, Irrigated Fields |

The ten empty cells have zero alpha. Keep the row and column positions when
replacing art so each level continues to use the correct sprite.
