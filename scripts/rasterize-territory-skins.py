"""Regenerate the map renderer's R8 atlas from the Store SVGs (rsvg-convert + ImageMagick)."""
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
atlas = bytearray(256 * 64)
for column, name in enumerate(("moon_rabbit", "war_ravens", "heraldic_lions")):
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
        offset = (row + 4) * 256 + column * 64 + 4
        atlas[offset:offset + 56] = mask[row * 56:(row + 1) * 56]

(root / "sow-render" / "src" / "map_renderer" / "territory_skins.r8").write_bytes(atlas)
