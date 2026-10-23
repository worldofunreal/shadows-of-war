#!/usr/bin/env python3
"""Write docs/habilidades-heroes.md from the hero catalog and the English strings.

Source of truth: assets/geo_entities.json (ability kind and number) and
sow-i18n/strings/en/web.toml (name and text, as keys). Other languages are
translated later from the same keys, so this table always reads in English.
"""
import json
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOG = ROOT / "assets" / "geo_entities.json"
STRINGS = ROOT / "sow-i18n" / "strings" / "en" / "web.toml"
OUT = ROOT / "docs" / "habilidades-heroes.md"


def main():
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    strings = tomllib.loads(STRINGS.read_text(encoding="utf-8"))
    site = strings["site"]
    heroes = [entity for entity in catalog["entities"] if entity.get("hero")]

    lines = [
        "# Habilidades de héroes",
        "",
        "Generado por `scripts/export-hero-abilities.py`. No editar a mano.",
        "",
        "| Héroe | Habilidad | Descripción | Tipo | Número | Ícono |",
        "|---|---|---|---|---|---|",
    ]
    for hero in heroes:
        hero_id = hero["id"]
        ability = hero["hero"]["ability"]
        name = site.get(f"leader_{hero_id}_ability", "(sin texto)")
        description = site.get(f"leader_{hero_id}_description", "(sin texto)")
        extra = ability.get("district") or ""
        if ability.get("resource"):
            extra = f"{extra} / {ability['resource']}".strip(" /")
        kind = ability["kind"] + (f" ({extra})" if extra else "")
        icon = f"gameplay/icons/abilities/{hero_id}.webp"
        lines.append(
            f"| {hero['hero']['name']} | {name} | {description} | {kind} | "
            f"{ability['percent']}% | `{icon}` |"
        )

    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)} ({len(heroes)} heroes)")


if __name__ == "__main__":
    main()
