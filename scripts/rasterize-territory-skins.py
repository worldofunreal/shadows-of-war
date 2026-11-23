"""Regenerate the map renderer's R8 atlas from the Store SVGs (rsvg-convert + ImageMagick)."""
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]

# (style, svg name). Cells are style - 10; styles 13-17 (columns 3-7) are the
# new animal silhouettes in assets/art/skins/.
SOURCES = [
    ("moon_rabbit", 10),
    ("war_ravens", 11),
    ("heraldic_lions", 12),
    ("elephant", 13),
    ("wolf", 14),
    ("tiger", 15),
    ("jaguar", 16),
    ("spider", 17),
]
ATLAS_WIDTH = 512

atlas = bytearray(ATLAS_WIDTH * 64)
for name, style in SOURCES:
    column = style - 10
    svg = root / "assets" / "art" / "skins" / f"{name}.svg"
    png = subprocess.check_output(["rsvg-convert", "-w", "56", "-h", "56", svg])
    if name == "war_ravens":
        # The supplied crow is colored on an off-white background.
        mask_args = ["-background", "white", "-alpha", "remove", "-colorspace", "gray",
                     "-level", "59%,86%", "-negate"]
    else:
        mask_args = ["-alpha", "extract"]
    mask = subprocess.check_output(
        ["magick", "png:-", *mask_args, "-depth", "8", "gray:-"], input=png
    )
    assert len(mask) == 56 * 56
    for row in range(56):
        offset = (row + 4) * ATLAS_WIDTH + column * 64 + 4
        atlas[offset:offset + 56] = mask[row * 56:(row + 1) * 56]

(root / "sow-render" / "src" / "map_renderer" / "territory_skins.r8").write_bytes(atlas)
