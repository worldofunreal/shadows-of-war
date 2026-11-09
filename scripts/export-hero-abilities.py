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

    heroes_strings = strings["heroes"]
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
        source = ability.get("source")
        district = source.get("district") if isinstance(source, dict) else source
        extra = " / ".join(part for part in (district, ability.get("resource")) if part)
        kind = ability["kind"] + (f" ({extra})" if extra else "")
        icon = f"gameplay/icons/abilities/{hero['hero'].get('art_slug') or hero['id']}.webp"
        hero_name = heroes_strings.get(f"leader_{hero_id}_name", hero["hero"]["name"])
        number = (
            f"{ability['percent']}%" if 'percent' in ability else
            f"{ability['gold']} gold" if 'gold' in ability else
            f"up to {ability['troops']} troops/match" if ability['kind'] == 'streltsy_muster' else
            f"{ability['troops']} troops" if 'troops' in ability else
            f"up to {ability['max_troops']} troops/match" if 'max_troops' in ability else
            f"{ability['troops_per_level']} troops/factory level" if 'troops_per_level' in ability else
            f"{ability['troops_per_gold']} troops/gold" if 'troops_per_gold' in ability else
            f"{ability['gold_per_second']} gold/s/level" if 'gold_per_second' in ability else
            "1 port/match" if ability['kind'] == 'coastal_founding' else
            "1 farm/level-2 city upgrade" if ability['kind'] == 'city_farm_founding' else
            "−1 level/city" if ability['kind'] == 'city_denial' else
            "1 farm level/home port" if ability['kind'] == 'punt_gardens' else
            "1 city/match" if ability['kind'] == 'last_city_relocation' else
            "1 tower/captured tower" if ability['kind'] == 'siege_breach' else
            "1 city level/lost farm" if ability['kind'] == 'cookham_exchange' else
            "1 completed allied city/match" if ability['kind'] == 'second_crown' else
            "50% of remaining upgrade time" if ability['kind'] == 'kitchen_philosophy' else
            "1 recalled army/city" if ability['kind'] == 'aksum_rout' else
            "1 tower/captured city" if ability['kind'] == 'akkad_garrison' else
            "25% each, up to 500 troops" if ability['kind'] == 'shared_feast' else
            "2 timed alliances restored/pair" if ability['kind'] == 'great_law_of_peace' else
            "1 tower level/city upgrade"
        )
        lines.append(
            f"| {hero_name} | {name} | {description} | {kind} | "
            f"{number} | `{icon}` |"
        )

    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)} ({len(heroes)} heroes)")


if __name__ == "__main__":
    main()
