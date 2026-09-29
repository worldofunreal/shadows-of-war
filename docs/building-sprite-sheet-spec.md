# Building sprite sheet

The first renderer uses one static transparent atlas. No animation frames are
needed for construction or upgrades; Blade draws the progress ring and the
completion pulse.

## Delivery format

- 22 cells in an 8×4 grid.
- Each cell is 128×128 RGBA with transparent corners.
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

The remaining ten atlas cells stay transparent and reserved. A supplied atlas
can replace the current emoji fallback without changing gameplay or the HUD.
