//! Lady Six Sky — episodes 1–3 on the `northamerica` map (1000×516; x→east, y→south).
//!
//! History (respected where written): Wak Chanil Ajaw, daughter of B'alaj Chan
//! K'awiil of Dos Pilas, arrived in Naranjo (Sa'aal) in 682 CE to found a new
//! dynasty; gave birth to K'ak' Tiliw Chan Chaak on 682+5y (Jan 6, 688); ruled
//! as his regent from 693; commissioned stelae 3, 18, 24, 29, 31, 46; performed
//! a burning ritual three days after arrival (Stela 29) and impersonated the
//! moon goddess in 726 (Stela 47); monuments show her as a warrior standing
//! over captives (Stela 24, K'inichil Kab); died Feb 741. Gaps — her husband's
//! name, whether the "five burned cities" were literal, K'inichil Kab's
//! location — are flagged as interpretation in the episode copy.
//!
//! Rosters live in ONE place each: `assets/campaign/lady_six_sky_ep{N}.json`,
//! authored with the same generator discipline as Boudica (tile coords snapped
//! to real land in `assets/maps/northamerica/map.bin`). Embedded here as the
//! compiled-in default so web/offline always has them; see `boudica.rs` for the
//! resolution order contract.

const EP1_ROSTER: &str = include_str!("../../../assets/campaign/lady_six_sky_ep1.json");
const EP2_ROSTER: &str = include_str!("../../../assets/campaign/lady_six_sky_ep2.json");
const EP3_ROSTER: &str = include_str!("../../../assets/campaign/lady_six_sky_ep3.json");

/// Naranjo tile on `northamerica` — the last-ditch spawn if an embedded roster
/// fails to parse (tests guard that it never does).
pub const FALLBACK_SPAWN: (u32, u32) = (529, 416);

fn load(
    embedded: &str,
    file: &str,
) -> (Vec<super::Faction>, (u32, u32)) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let env_path = std::env::var("SOW_CAMPAIGN_ROSTER").ok();
        let candidates = env_path
            .as_deref()
            .into_iter()
            .chain([file]);
        for path in candidates {
            if let Some(loaded) = super::load_roster_json(path) {
                log::info!(
                    "campaign: roster from file '{}' ({} factions, spawn {:?})",
                    path,
                    loaded.0.len(),
                    loaded.1
                );
                return loaded;
            }
        }
    }
    super::parse_roster(embedded).unwrap_or_else(|| {
        log::error!("campaign: embedded {file} failed to parse — using empty roster");
        (Vec::new(), FALLBACK_SPAWN)
    })
}

/// Episode roster + player spawn. `episode` is 1–3; anything else falls back
/// to episode 1 so a bad index can never brick the launcher.
pub fn roster(episode: u8) -> (Vec<super::Faction>, (u32, u32)) {
    match episode {
        2 => load(EP2_ROSTER, "assets/campaign/lady_six_sky_ep2.json"),
        3 => load(EP3_ROSTER, "assets/campaign/lady_six_sky_ep3.json"),
        _ => load(EP1_ROSTER, "assets/campaign/lady_six_sky_ep1.json"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_rosters_parse_and_are_sane() {
        for (n, text) in [
            (1, super::EP1_ROSTER),
            (2, super::EP2_ROSTER),
            (3, super::EP3_ROSTER),
        ] {
            let (factions, spawn) = crate::campaign::parse_roster(text)
                .expect("embedded lady_six_sky roster must parse");
            assert!(
                factions.len() >= 10,
                "ep{n} roster looks too small ({} factions)",
                factions.len()
            );
            assert!(spawn.0 > 0 && spawn.1 > 0, "ep{n} player spawn unset");
        }
    }
}
