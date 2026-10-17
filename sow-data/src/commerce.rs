use crate::leaders::Leader;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const FREE_ROTATION_SIZE: usize = 4;
pub const ROTATION_PERIOD_SECS: u64 = 7 * 24 * 60 * 60;
pub const PERMANENTLY_FREE_LEADERS: [Leader; 1] = [Leader::Boudica];
// ponytail: one balancing constant until real retention data exists; move pricing to live config then.
// Crowns are the free spendable currency (owner decision 2026-09); laurels are
// achievement points and are never a price.
pub const LEADER_UNLOCK_COST_CROWNS: u64 = 500;
pub const LEADER_UNLOCK_COST_GEMS: u64 = 450;
/// One-time starter grant for every human account, regardless of identity
/// provider. Bots never receive it.
pub const WELCOME_GEMS: u64 = 475;
/// New receipt version intentionally tops up accounts that already have v1.
pub const WELCOME_GRANT_ID: &str = "welcome_gems_v2";

// (product id, gems, price in US cents). Product ids are frozen in Google Play
// and RevenueCat; the number in an id is not the gem amount.
const GEM_BUNDLES: [(&str, u64, u64); 3] = [
    ("sow_gems_500", 500, 199),
    ("sow_gems_1200", 1_400, 499),
    ("sow_gems_2600", 3_000, 999),
];

// Original SOW cosmetic catalog. The style id is an internal renderer hint;
// the public id is the only value persisted in player ownership.
const SKINS: [(&str, &str, &str, u64, u8); 3] = [
    (
        "ember_vein",
        "Ember Vein",
        "gameplay/skins/ember_vein.svg",
        500,
        1,
    ),
    (
        "storm_grid",
        "Storm Grid",
        "gameplay/skins/storm_grid.svg",
        1_000,
        2,
    ),
    (
        "royal_lattice",
        "Royal Lattice",
        "gameplay/skins/royal_lattice.svg",
        1_500,
        3,
    ),
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LeaderOffer {
    pub id: String,
    pub name: String,
    pub civilization: String,
    pub perk: String,
    pub cost_crowns: u64,
    pub cost_gems: u64,
    pub free_rotation: bool,
    #[serde(default)]
    pub permanently_free: bool,
    pub owned: bool,
    pub available: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SkinOffer {
    pub id: String,
    pub leader_id: String,
    pub name: String,
    pub asset_path: String,
    pub cost_gems: u64,
    pub owned: bool,
    pub style: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GemBundle {
    pub id: String,
    pub gems: u64,
    pub product_id: String,
    pub asset_path: String,
    pub price_cents: u64,
    /// Extra gems per dollar versus the smallest pack, rounded.
    pub bonus_percent: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StoreCatalog {
    pub rotation_period: u64,
    pub free_leaders: Vec<String>,
    pub leaders: Vec<LeaderOffer>,
    pub skins: Vec<SkinOffer>,
    pub gem_bundles: Vec<GemBundle>,
    pub crowns: u64,
    pub gems: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeaderResolution {
    pub requested: Leader,
    pub resolved: Leader,
    pub requested_available: bool,
    pub used_fallback: bool,
}

pub fn current_rotation_period() -> u64 {
    rotation_period(
        web_time::SystemTime::now()
            .duration_since(web_time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
}

pub const fn rotation_period(unix_seconds: u64) -> u64 {
    unix_seconds / ROTATION_PERIOD_SECS
}

pub fn rotation_for_period(period: u64) -> Vec<Leader> {
    let candidates = Leader::ALL
        .into_iter()
        .filter(|leader| !PERMANENTLY_FREE_LEADERS.contains(leader))
        .collect::<Vec<_>>();
    let start = (period as usize * FREE_ROTATION_SIZE) % candidates.len();
    (0..FREE_ROTATION_SIZE)
        .map(|offset| candidates[(start + offset) % candidates.len()])
        .collect()
}

pub fn current_leader_rotation() -> Vec<Leader> {
    rotation_for_period(current_rotation_period())
}

pub fn leader_id(leader: Leader) -> &'static str {
    match leader {
        Leader::Caesar => "caesar",
        Leader::Cleopatra => "cleopatra",
        Leader::Ragnar => "ragnar",
        Leader::SunTzu => "sun_tzu",
        Leader::Alexander => "alexander",
        Leader::GenghisKhan => "genghis_khan",
        Leader::RichardTheLionheart => "richard_the_lionheart",
        Leader::Vercingetorix => "vercingetorix",
        Leader::Boudica => "boudica",
        Leader::LadySixSky => "lady_six_sky",
        Leader::Leonidas => "leonidas",
        Leader::Napoleon => "napoleon",
    }
}

pub fn leader_wire_id(leader: Leader) -> &'static str {
    match leader {
        Leader::Caesar => "Caesar",
        Leader::Cleopatra => "Cleopatra",
        Leader::Ragnar => "Ragnar",
        Leader::SunTzu => "SunTzu",
        Leader::Alexander => "Alexander",
        Leader::GenghisKhan => "GenghisKhan",
        Leader::RichardTheLionheart => "RichardTheLionheart",
        Leader::Vercingetorix => "Vercingetorix",
        Leader::Boudica => "Boudica",
        Leader::LadySixSky => "LadySixSky",
        Leader::Leonidas => "Leonidas",
        Leader::Napoleon => "Napoleon",
    }
}

pub fn leader_from_id(value: &str) -> Option<Leader> {
    let normalized: String = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect();
    Leader::ALL.into_iter().find(|leader| {
        [leader_id(*leader), leader_wire_id(*leader), leader.name()]
            .into_iter()
            .map(|candidate| {
                candidate
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .map(|character| character.to_ascii_lowercase())
                    .collect::<String>()
            })
            .any(|candidate| candidate == normalized)
    })
}

pub fn assigned_leader_for_account(account_id: &str, period: u64) -> Leader {
    let hash = account_id.bytes().fold(period, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u64)
    });
    rotation_for_period(period)[(hash as usize) % FREE_ROTATION_SIZE]
}

pub fn leader_available(leader: Leader, owned: &BTreeSet<String>, period: u64) -> bool {
    PERMANENTLY_FREE_LEADERS.contains(&leader)
        || owned.contains(leader_id(leader))
        || rotation_for_period(period).contains(&leader)
}

pub fn available_leaders(owned: &BTreeSet<String>, period: u64) -> Vec<Leader> {
    Leader::ALL
        .into_iter()
        .filter(|leader| leader_available(*leader, owned, period))
        .collect()
}

#[cfg(feature = "server")]
pub fn draw_available_leader(owned: &BTreeSet<String>, period: u64) -> Option<Leader> {
    use rand::seq::SliceRandom;
    available_leaders(owned, period)
        .choose(&mut rand::thread_rng())
        .copied()
}

pub fn gem_bundles() -> Vec<GemBundle> {
    let (_, base_gems, base_cents) = GEM_BUNDLES[0];
    GEM_BUNDLES
        .into_iter()
        .map(|(product_id, gems, price_cents)| GemBundle {
            id: product_id.to_string(),
            gems,
            product_id: product_id.to_string(),
            asset_path: format!("gameplay/store/gem_bundles/{product_id}.webp"),
            price_cents,
            bonus_percent: (gems * base_cents * 100 + base_gems * price_cents / 2)
                / (base_gems * price_cents)
                - 100,
        })
        .collect()
}

pub fn gem_bundle(product_id: &str) -> Option<GemBundle> {
    gem_bundles()
        .into_iter()
        .find(|bundle| bundle.product_id == product_id)
}

pub fn gem_amount_for_product(product_id: &str) -> Option<u64> {
    gem_bundle(product_id).map(|bundle| bundle.gems)
}


/// Real money buys gem packs only; leaders and skins cost gems or crowns.
pub fn is_store_product(product_id: &str) -> bool {
    gem_bundle(product_id).is_some()
}

pub fn skins() -> Vec<SkinOffer> {
    SKINS
        .into_iter()
        .map(|(id, name, asset_path, cost_gems, style)| SkinOffer {
            id: id.to_string(),
            leader_id: "all".to_string(),
            name: name.to_string(),
            asset_path: asset_path.to_string(),
            cost_gems,
            owned: false,
            style,
        })
        .collect()
}

pub fn skin_by_id(skin_id: &str) -> Option<SkinOffer> {
    skins().into_iter().find(|skin| skin.id == skin_id)
}

/// Canonical NFTROPOLY/WOU-ID card token for an owned skin. The public skin
/// id is the only value persisted in player ownership, so the card id is
/// derived from it (snake_case -> kebab, `sow-skin-` prefix) and registered
/// identically on WOU-ID.
pub fn skin_card_token_id(skin_id: &str) -> String {
    format!("sow-skin-{}", skin_id.replace('_', "-"))
}

pub fn skin_style_for_id(skin_id: Option<&str>) -> u8 {
    skin_id
        .and_then(skin_by_id)
        .map(|skin| skin.style)
        .unwrap_or(0)
}

pub fn skin_style_for_profile(owned_skins: &BTreeSet<String>, selected_skin: Option<&str>) -> u8 {
    match selected_skin {
        Some(id) if owned_skins.contains(id) => skin_style_for_id(Some(id)),
        _ => 0,
    }
}

pub fn resolve_leader(
    requested: Option<&str>,
    account_id: &str,
    owned: &BTreeSet<String>,
    period: u64,
) -> LeaderResolution {
    let fallback = assigned_leader_for_account(account_id, period);
    let requested = requested.and_then(leader_from_id).unwrap_or(fallback);
    let requested_available = leader_available(requested, owned, period);
    let resolved = if requested_available {
        requested
    } else {
        fallback
    };
    LeaderResolution {
        requested,
        resolved,
        requested_available,
        used_fallback: resolved != requested,
    }
}

pub fn catalog_for_profile(
    owned_leaders: &BTreeSet<String>,
    owned_skins: &BTreeSet<String>,
    crowns: u64,
    gems: u64,
    period: u64,
) -> StoreCatalog {
    let free = rotation_for_period(period);
    let leaders = Leader::ALL
        .into_iter()
        .map(|leader| {
            let owned = owned_leaders.contains(leader_id(leader));
            let free_rotation = free.contains(&leader);
            let permanently_free = PERMANENTLY_FREE_LEADERS.contains(&leader);
            LeaderOffer {
                id: leader_id(leader).to_string(),
                name: leader.name().to_string(),
                civilization: leader.civilization().name().to_string(),
                perk: leader.perk_description().to_string(),
                cost_crowns: if permanently_free { 0 } else { LEADER_UNLOCK_COST_CROWNS },
                cost_gems: if permanently_free { 0 } else { LEADER_UNLOCK_COST_GEMS },
                free_rotation,
                permanently_free,
                owned,
                available: permanently_free || free_rotation || owned,
            }
        })
        .collect();

    let skins = skins()
        .into_iter()
        .map(|mut skin| {
            skin.owned = owned_skins.contains(&skin.id);
            skin
        })
        .collect();
    StoreCatalog {
        rotation_period: period,
        free_leaders: free
            .into_iter()
            .map(|leader| leader_id(leader).to_string())
            .collect(),
        leaders,
        skins,
        gem_bundles: gem_bundles(),
        crowns,
        gems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_is_four_and_deterministic() {
        let first = rotation_for_period(7);
        assert_eq!(first.len(), FREE_ROTATION_SIZE);
        assert_eq!(first, rotation_for_period(7));
        assert_ne!(first, rotation_for_period(8));
        assert!(!first.contains(&Leader::Boudica));
        assert_eq!(first.iter().copied().collect::<std::collections::HashSet<_>>().len(), FREE_ROTATION_SIZE);
    }

    #[test]
    fn new_account_assignment_stays_inside_the_eight_leader_rotation() {
        for period in 0..16 {
            let assigned = assigned_leader_for_account("new-account", period);
            assert!(rotation_for_period(period).contains(&assigned));
            assert_ne!(assigned, Leader::Boudica);
        }
    }

    #[test]
    fn locked_leader_falls_back_to_account_assignment() {
        let owned = BTreeSet::new();
        let rotation = rotation_for_period(7);
        let locked = Leader::ALL
            .into_iter()
            .find(|leader| *leader != Leader::Boudica && !rotation.contains(leader))
            .unwrap();
        let resolution = resolve_leader(Some(leader_id(locked)), "account", &owned, 7);
        assert!(!resolution.requested_available);
        assert!(resolution.used_fallback);
        assert!(rotation.contains(&resolution.resolved));
    }

    #[test]
    fn random_pool_is_unique_and_contains_only_free_or_owned_leaders() {
        let rotation = rotation_for_period(7);
        let owned = BTreeSet::from([leader_id(Leader::Napoleon).to_string()]);
        let available = available_leaders(&owned, 7);
        assert_eq!(available.iter().copied().collect::<std::collections::HashSet<_>>().len(), available.len());
        assert!(available.contains(&Leader::Boudica));
        assert!(available.contains(&Leader::Napoleon));
        for leader in Leader::ALL {
            if !rotation.contains(&leader)
                && !owned.contains(leader_id(leader))
                && leader != Leader::Boudica
            {
                assert!(!available.contains(&leader));
            }
        }
    }

    #[test]
    fn boudica_is_permanently_free_and_has_no_purchase_product() {
        let empty = BTreeSet::new();
        let catalog = catalog_for_profile(&empty, &empty, 0, 0, 7);
        let boudica = catalog.leaders.iter().find(|leader| leader.id == "boudica").unwrap();
        assert!(boudica.permanently_free);
        assert!(boudica.available);
        assert_eq!(boudica.cost_crowns, 0);
        assert_eq!(boudica.cost_gems, 0);
        assert!(!catalog.free_leaders.contains(&"boudica".to_string()));
        assert!(!is_store_product("sow_offer_leader_boudica"));
    }

    #[test]
    fn gem_products_have_authoritative_amounts() {
        assert_eq!(gem_bundles().len(), 3);
        assert_eq!(gem_amount_for_product("sow_gems_1200"), Some(1_400));
        assert_eq!(gem_amount_for_product("unknown"), None);
    }

    #[test]
    fn bigger_gem_packs_always_give_more_gems_per_dollar() {
        let bundles = gem_bundles();
        let prices: Vec<_> = bundles.iter().map(|b| (b.price_cents, b.bonus_percent)).collect();
        assert_eq!(prices, [(199, 0), (499, 12), (999, 20)]);
        for pair in bundles.windows(2) {
            assert!(pair[1].gems * pair[0].price_cents > pair[0].gems * pair[1].price_cents);
        }
    }

    #[test]
    fn only_gem_packs_are_sold_for_money() {
        assert!(is_store_product("sow_gems_500"));
        assert!(!is_store_product("sow_offer_leader_caesar"));
        assert!(!is_store_product("sow_offer_skin_ember_vein"));
        assert!(!is_store_product("price_1UBIdc5GjRL6SWJS0cuGDATW"));
    }

    #[test]
    fn skins_are_original_and_have_stable_prices() {
        assert_eq!(skins().len(), 3);
        assert_eq!(
            skin_by_id("ember_vein").map(|skin| skin.cost_gems),
            Some(500)
        );
        assert_eq!(
            skin_by_id("storm_grid").map(|skin| skin.cost_gems),
            Some(1_000)
        );
        assert_eq!(
            skin_by_id("royal_lattice").map(|skin| skin.cost_gems),
            Some(1_500)
        );
        assert_eq!(skin_style_for_id(Some("royal_lattice")), 3);
        assert_eq!(skin_style_for_id(Some("missing")), 0);
        let mut owned = BTreeSet::new();
        owned.insert("royal_lattice".to_string());
        assert_eq!(skin_style_for_profile(&owned, Some("royal_lattice")), 3);
        assert_eq!(
            skin_style_for_profile(&BTreeSet::new(), Some("royal_lattice")),
            0
        );
    }

    #[test]
    fn leader_has_dual_currency_price() {
        assert_eq!(LEADER_UNLOCK_COST_CROWNS, 500);
        assert_eq!(LEADER_UNLOCK_COST_GEMS, 450);
        assert_eq!(WELCOME_GEMS, 475);
    }

    #[test]
    fn crowns_catalog_uses_the_canonical_balance_key() {
        let owned = BTreeSet::new();
        let catalog = catalog_for_profile(&owned, &owned, 725, 0, 0);
        let value = serde_json::to_value(&catalog).unwrap();
        assert_eq!(value["crowns"], 725);
        assert_eq!(value["leaders"][0]["cost_crowns"], 500);
    }

    #[test]
    fn catalog_distinguishes_free_owned_and_locked_leaders() {
        let empty = BTreeSet::new();
        let catalog = catalog_for_profile(&empty, &empty, 0, 475, 7);
        assert!(catalog.leaders.iter().filter(|leader| !leader.permanently_free).all(|leader| {
            leader.cost_gems == 450 && leader.cost_crowns == LEADER_UNLOCK_COST_CROWNS
        }));
        let free = catalog
            .leaders
            .iter()
            .find(|leader| leader.free_rotation)
            .unwrap();
        let locked = catalog
            .leaders
            .iter()
            .find(|leader| !leader.free_rotation && !leader.permanently_free)
            .unwrap();
        assert_eq!(catalog.free_leaders.len(), FREE_ROTATION_SIZE);
        assert!(!catalog.free_leaders.contains(&"boudica".to_string()));
        assert!(free.available);
        assert!(!free.owned);
        assert!(!locked.available);
        assert!(!locked.owned);

        let owned = BTreeSet::from([locked.id.clone()]);
        let owned_catalog = catalog_for_profile(&owned, &empty, 0, 0, 7);
        let owned_offer = owned_catalog
            .leaders
            .iter()
            .find(|leader| leader.id == locked.id)
            .unwrap();
        assert!(owned_offer.available);
        assert!(owned_offer.owned);
    }
}
