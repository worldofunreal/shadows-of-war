#[cfg(feature = "server")]
use crate::geo_entities::{Gender, Region};
use crate::leaders::Leader;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const FREE_ROTATION_SIZE: usize = 8;
pub const ROTATION_PERIOD_SECS: u64 = 7 * 24 * 60 * 60;
// Unix weeks start Thursday; this aligns rotations to Tuesday 08:00 UTC (02:00 Mexico City).
const ROTATION_RESET_OFFSET_SECS: u64 = 40 * 60 * 60;
/// One-time starter grant for every human account, regardless of identity
/// provider. Bots never receive it.
pub const WELCOME_GEMS: u64 = 25;
/// New receipt version intentionally tops up accounts that already have v1.
/// Keep this id when only the amount changes; a new id would top up every account.
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
    unix_seconds.saturating_add(ROTATION_RESET_OFFSET_SECS) / ROTATION_PERIOD_SECS
}

/// Rotation used only to freeze the week that was already live when durable
/// rotations were introduced.
#[cfg(any(feature = "server", test))]
pub(crate) fn legacy_rotation_for_period(period: u64) -> Vec<Leader> {
    let candidates = Leader::all()
        .filter(|leader| !leader.always_free())
        .collect::<Vec<_>>();
    let size = FREE_ROTATION_SIZE.min(candidates.len());
    let start = (period as usize * FREE_ROTATION_SIZE) % candidates.len().max(1);
    (0..size)
        .map(|offset| candidates[(start + offset) % candidates.len()])
        .collect()
}

pub fn leader_id(leader: Leader) -> &'static str {
    leader.id()
}

pub fn leader_wire_id(leader: Leader) -> &'static str {
    leader.wire_id()
}

pub fn leader_from_id(value: &str) -> Option<Leader> {
    let normalize = |value: &str| -> String {
        value
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .map(|character| character.to_ascii_lowercase())
            .collect()
    };
    let normalized = normalize(value);
    Leader::all().find(|leader| {
        [
            leader.id(),
            leader.wire_id(),
            leader.name(),
            leader.display_name(),
        ]
        .into_iter()
        .any(|candidate| normalize(candidate) == normalized)
            || leader
                .legacy_ids()
                .iter()
                .any(|candidate| normalize(candidate) == normalized)
    })
}

pub fn owns_leader(owned: &BTreeSet<String>, leader: Leader) -> bool {
    owned.contains(leader_id(leader))
        || leader
            .legacy_ids()
            .iter()
            .any(|legacy_id| owned.contains(legacy_id))
}

pub fn assigned_leader_for_account(
    account_id: &str,
    period: u64,
    free_leaders: &[Leader],
) -> Leader {
    let hash = account_id.bytes().fold(period, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u64)
    });
    free_leaders
        .get((hash as usize) % free_leaders.len().max(1))
        .copied()
        .unwrap_or_else(|| Leader::of("boudica"))
}

pub fn leader_available(leader: Leader, owned: &BTreeSet<String>, free_leaders: &[Leader]) -> bool {
    leader.always_free() || owns_leader(owned, leader) || free_leaders.contains(&leader)
}

pub fn available_leaders(owned: &BTreeSet<String>, free_leaders: &[Leader]) -> Vec<Leader> {
    Leader::all()
        .filter(|leader| leader_available(*leader, owned, free_leaders))
        .collect()
}

#[cfg(feature = "server")]
pub fn draw_available_leader(owned: &BTreeSet<String>, free_leaders: &[Leader]) -> Option<Leader> {
    use rand::seq::SliceRandom;
    available_leaders(owned, free_leaders)
        .choose(&mut rand::thread_rng())
        .copied()
}

#[cfg(feature = "server")]
pub(crate) fn generate_free_rotation<R: rand::Rng + ?Sized>(
    candidates: &[Leader],
    previous: &[Leader],
    queued: &[Leader],
    rng: &mut R,
) -> Option<(Vec<Leader>, Vec<Leader>)> {
    use rand::seq::SliceRandom;

    let mut pool = Vec::new();
    for leader in candidates.iter().copied() {
        if !leader.always_free() && !previous.contains(&leader) && !pool.contains(&leader) {
            pool.push(leader);
        }
    }
    pool.shuffle(rng);

    let balanced = find_rotation(&pool, &[], true).is_some();
    let mut seen_queued = std::collections::HashSet::new();
    let mut queued_candidates = queued
        .iter()
        .copied()
        .filter(|leader| pool.contains(leader) && seen_queued.insert(*leader))
        .collect::<Vec<_>>();
    queued_candidates.shuffle(rng);

    let max_queued = if balanced {
        [Gender::Male, Gender::Female]
            .into_iter()
            .map(|gender| {
                queued_candidates
                    .iter()
                    .filter(|leader| leader.character().gender == Some(gender))
                    .count()
                    .min(FREE_ROTATION_SIZE / 2)
            })
            .sum::<usize>()
    } else {
        queued_candidates.len().min(FREE_ROTATION_SIZE)
    };
    let required = (0..=max_queued)
        .rev()
        .find_map(|count| find_queued_subset(&pool, &queued_candidates, count, balanced))?;
    let mut selected = find_rotation(&pool, &required, balanced)?;
    let deferred = queued_candidates
        .into_iter()
        .filter(|leader| !selected.contains(leader))
        .collect();
    selected.shuffle(rng);
    Some((selected, deferred))
}

#[cfg(feature = "server")]
fn find_queued_subset(
    candidates: &[Leader],
    queued: &[Leader],
    target: usize,
    balanced: bool,
) -> Option<Vec<Leader>> {
    fn search(
        candidates: &[Leader],
        queued: &[Leader],
        target: usize,
        start: usize,
        selected: &mut Vec<Leader>,
        balanced: bool,
    ) -> Option<Vec<Leader>> {
        if selected.len() == target {
            return find_rotation(candidates, selected, balanced).map(|_| selected.clone());
        }
        let needed = target - selected.len();
        if queued.len().saturating_sub(start) < needed {
            return None;
        }

        let mut seen_categories = Vec::new();
        for index in start..=queued.len() - needed {
            let leader = queued[index];
            let category = (leader.character().region, leader.character().gender);
            if seen_categories.contains(&category) {
                continue;
            }
            seen_categories.push(category);

            selected.push(leader);
            if find_rotation(candidates, selected, balanced).is_some()
                && let Some(found) =
                    search(candidates, queued, target, index + 1, selected, balanced)
            {
                return Some(found);
            }
            selected.pop();
        }
        None
    }

    search(candidates, queued, target, 0, &mut Vec::new(), balanced)
}

#[cfg(feature = "server")]
fn find_rotation(
    candidates: &[Leader],
    required: &[Leader],
    balanced: bool,
) -> Option<Vec<Leader>> {
    if required.len() > FREE_ROTATION_SIZE
        || required.iter().any(|leader| !candidates.contains(leader))
    {
        return None;
    }
    let mut selected = Vec::with_capacity(FREE_ROTATION_SIZE);
    for leader in required.iter().copied() {
        if !selected.contains(&leader) {
            selected.push(leader);
        }
    }
    if selected.len() > FREE_ROTATION_SIZE {
        return None;
    }

    fn search(
        candidates: &[Leader],
        start: usize,
        selected: &mut Vec<Leader>,
        balanced: bool,
    ) -> bool {
        if selected.len() == FREE_ROTATION_SIZE {
            return has_all_regions(selected) && (!balanced || has_balanced_gender(selected));
        }

        let slots = FREE_ROTATION_SIZE - selected.len();
        if candidates.len().saturating_sub(start) < slots {
            return false;
        }
        let missing_regions = [
            Region::Africa,
            Region::Americas,
            Region::Asia,
            Region::Europe,
            Region::Oceania,
        ]
        .into_iter()
        .filter(|region| {
            !selected
                .iter()
                .any(|leader| leader.character().region == *region)
        })
        .collect::<Vec<_>>();
        if missing_regions.len() > slots
            || missing_regions.iter().any(|region| {
                !candidates[start..].iter().any(|leader| {
                    !selected.contains(leader) && leader.character().region == *region
                })
            })
        {
            return false;
        }

        if balanced {
            let male_count = selected
                .iter()
                .filter(|leader| leader.character().gender == Some(Gender::Male))
                .count();
            let female_count = selected
                .iter()
                .filter(|leader| leader.character().gender == Some(Gender::Female))
                .count();
            if male_count > 4
                || female_count > 4
                || selected
                    .iter()
                    .any(|leader| leader.character().gender.is_none())
                || candidates[start..]
                    .iter()
                    .filter(|leader| !selected.contains(leader))
                    .filter(|leader| leader.character().gender == Some(Gender::Male))
                    .count()
                    < 4 - male_count
                || candidates[start..]
                    .iter()
                    .filter(|leader| !selected.contains(leader))
                    .filter(|leader| leader.character().gender == Some(Gender::Female))
                    .count()
                    < 4 - female_count
            {
                return false;
            }
        }

        for index in start..candidates.len() {
            let leader = candidates[index];
            if selected.contains(&leader) {
                continue;
            }
            if balanced
                && !matches!(
                    leader.character().gender,
                    Some(Gender::Male | Gender::Female)
                )
            {
                continue;
            }
            selected.push(leader);
            if search(candidates, index + 1, selected, balanced) {
                return true;
            }
            selected.pop();
        }
        false
    }

    search(candidates, 0, &mut selected, balanced).then_some(selected)
}

#[cfg(feature = "server")]
fn has_all_regions(leaders: &[Leader]) -> bool {
    [
        Region::Africa,
        Region::Americas,
        Region::Asia,
        Region::Europe,
        Region::Oceania,
    ]
    .into_iter()
    .all(|region| {
        leaders
            .iter()
            .any(|leader| leader.character().region == region)
    })
}

#[cfg(feature = "server")]
fn has_balanced_gender(leaders: &[Leader]) -> bool {
    leaders
        .iter()
        .filter(|leader| leader.character().gender == Some(Gender::Male))
        .count()
        == 4
        && leaders
            .iter()
            .filter(|leader| leader.character().gender == Some(Gender::Female))
            .count()
            == 4
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
    free_leaders: &[Leader],
) -> LeaderResolution {
    let fallback = assigned_leader_for_account(account_id, period, free_leaders);
    let requested = requested.and_then(leader_from_id).unwrap_or(fallback);
    let requested_available = leader_available(requested, owned, free_leaders);
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
    free: &[Leader],
) -> StoreCatalog {
    let leaders = Leader::all()
        .map(|leader| {
            let owned = owns_leader(owned_leaders, leader);
            let free_rotation = free.contains(&leader);
            let permanently_free = leader.always_free();
            LeaderOffer {
                id: leader_id(leader).to_string(),
                name: leader.display_name().to_string(),
                civilization: leader.civilization().to_string(),
                cost_crowns: leader.price_crowns(),
                cost_gems: leader.price_gems(),
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
            .iter()
            .copied()
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
    fn rotation_period_resets_tuesday_at_0800_utc() {
        let first_reset = ROTATION_PERIOD_SECS - ROTATION_RESET_OFFSET_SECS;
        assert_eq!(rotation_period(first_reset - 1), 0);
        assert_eq!(rotation_period(first_reset), 1);
        assert_eq!(rotation_period(first_reset + ROTATION_PERIOD_SECS - 1), 1);
        assert_eq!(rotation_period(first_reset + ROTATION_PERIOD_SECS), 2);
    }

    #[test]
    fn new_account_assignment_stays_inside_the_eight_leader_rotation() {
        let rotation = legacy_rotation_for_period(7);
        let assigned = assigned_leader_for_account("new-account", 7, &rotation);
        assert!(rotation.contains(&assigned));
        assert_ne!(assigned, Leader::of("boudica"));
    }

    #[test]
    fn locked_leader_falls_back_to_account_assignment() {
        let owned = BTreeSet::new();
        let rotation = legacy_rotation_for_period(7);
        let locked = Leader::all()
            .find(|leader| *leader != Leader::of("boudica") && !rotation.contains(leader))
            .unwrap();
        let resolution = resolve_leader(Some(leader_id(locked)), "account", &owned, 7, &rotation);
        assert!(!resolution.requested_available);
        assert!(resolution.used_fallback);
        assert!(rotation.contains(&resolution.resolved));
    }

    #[test]
    fn random_pool_is_unique_and_contains_only_free_or_owned_leaders() {
        let rotation = legacy_rotation_for_period(7);
        let owned = BTreeSet::from([leader_id(Leader::of("napoleon")).to_string()]);
        let available = available_leaders(&owned, &rotation);
        assert_eq!(
            available
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            available.len()
        );
        assert!(available.contains(&Leader::of("boudica")));
        assert!(available.contains(&Leader::of("napoleon")));
        for leader in Leader::all() {
            if !rotation.contains(&leader)
                && !owned.contains(leader_id(leader))
                && leader != Leader::of("boudica")
            {
                assert!(!available.contains(&leader));
            }
        }
    }

    #[test]
    fn boudica_is_permanently_free_and_has_no_purchase_product() {
        let empty = BTreeSet::new();
        let rotation = legacy_rotation_for_period(7);
        let catalog = catalog_for_profile(&empty, &empty, 0, 0, 7, &rotation);
        let boudica = catalog
            .leaders
            .iter()
            .find(|leader| leader.id == "boudica")
            .unwrap();
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
        let prices: Vec<_> = bundles
            .iter()
            .map(|b| (b.price_cents, b.bonus_percent))
            .collect();
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
    fn welcome_grant_is_fixed() {
        assert_eq!(WELCOME_GEMS, 25);
    }

    #[test]
    fn crowns_catalog_uses_the_canonical_balance_key() {
        let owned = BTreeSet::new();
        let rotation = legacy_rotation_for_period(0);
        let catalog = catalog_for_profile(&owned, &owned, 725, 0, 0, &rotation);
        let value = serde_json::to_value(&catalog).unwrap();
        assert_eq!(value["crowns"], 725);
        let first = Leader::all().next().unwrap();
        assert_eq!(value["leaders"][0]["cost_crowns"], first.price_crowns());
    }

    #[test]
    fn catalog_distinguishes_free_owned_and_locked_leaders() {
        let empty = BTreeSet::new();
        let rotation = legacy_rotation_for_period(7);
        let catalog = catalog_for_profile(&empty, &empty, 0, 475, 7, &rotation);
        assert!(
            catalog
                .leaders
                .iter()
                .filter(|leader| !leader.permanently_free)
                .all(|leader| {
                    let hero = leader_from_id(&leader.id).unwrap();
                    leader.cost_gems == hero.price_gems() && leader.cost_crowns == hero.price_crowns()
                })
        );
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
        for offer in &catalog.leaders {
            assert_eq!(
                offer.free_rotation,
                catalog.free_leaders.contains(&offer.id),
                "catalog offer and rotation list must agree for {}",
                offer.id
            );
        }
        assert!(!catalog.free_leaders.contains(&"boudica".to_string()));
        assert!(free.available);
        assert!(!free.owned);
        assert!(!locked.available);
        assert!(!locked.owned);

        let owned = BTreeSet::from([locked.id.clone()]);
        let owned_catalog = catalog_for_profile(&owned, &empty, 0, 0, 7, &rotation);
        let owned_offer = owned_catalog
            .leaders
            .iter()
            .find(|leader| leader.id == locked.id)
            .unwrap();
        assert!(owned_offer.available);
        assert!(owned_offer.owned);
    }

    #[cfg(feature = "server")]
    #[test]
    fn generated_rotation_uses_regions_gender_balance_and_one_week_cooldown() {
        use rand::{SeedableRng, rngs::StdRng};

        let candidates = Leader::all()
            .filter(|leader| !leader.always_free())
            .collect::<Vec<_>>();
        let mut frequencies = std::collections::HashMap::<Leader, usize>::new();
        let mut gap_totals = std::collections::HashMap::<Leader, (usize, usize, usize)>::new();
        let mut closest_repeat_gap = usize::MAX;
        for seed in 0..100 {
            let mut rng = StdRng::seed_from_u64(seed);
            let mut previous = Vec::new();
            let mut last_seen = std::collections::HashMap::<Leader, usize>::new();
            for week in 0..104 {
                let (rotation, deferred) =
                    generate_free_rotation(&candidates, &previous, &[], &mut rng).unwrap();
                assert!(deferred.is_empty());
                assert_eq!(rotation.len(), FREE_ROTATION_SIZE);
                assert_eq!(
                    rotation
                        .iter()
                        .copied()
                        .collect::<std::collections::HashSet<_>>()
                        .len(),
                    FREE_ROTATION_SIZE
                );
                assert!(has_all_regions(&rotation));
                assert!(has_balanced_gender(&rotation));
                assert!(rotation.iter().all(|leader| !previous.contains(leader)));
                for leader in &rotation {
                    *frequencies.entry(*leader).or_default() += 1;
                    if let Some(last) = last_seen.insert(*leader, week) {
                        let gap = week - last;
                        closest_repeat_gap = closest_repeat_gap.min(gap);
                        let totals = gap_totals.entry(*leader).or_default();
                        totals.0 += gap;
                        totals.1 += 1;
                        totals.2 += usize::from(gap == 2);
                    }
                }
                previous = rotation;
            }
        }

        let mut cohort_ratios = Vec::new();
        let mut cohort_gap_skews = Vec::new();
        let mut cohort_short_repeat_rates = Vec::<(Region, Gender, f64, usize, usize)>::new();
        for region in [
            Region::Africa,
            Region::Americas,
            Region::Asia,
            Region::Europe,
            Region::Oceania,
        ] {
            for gender in [Gender::Female, Gender::Male] {
                let cohort = candidates
                    .iter()
                    .copied()
                    .filter(|leader| {
                        leader.character().region == region
                            && leader.character().gender == Some(gender)
                    })
                    .collect::<Vec<_>>();
                if cohort.len() < 2 {
                    continue;
                }
                let counts = cohort
                    .iter()
                    .map(|leader| frequencies.get(leader).copied().unwrap_or_default())
                    .collect::<Vec<_>>();
                let mean = counts.iter().sum::<usize>() as f64 / counts.len() as f64;
                let ratio = counts.iter().copied().max().unwrap() as f64 / mean;
                cohort_ratios.push(ratio);
                assert!(
                    ratio < 1.35,
                    "frequency skew in {region:?}/{gender:?}: {ratio:.3}"
                );
                let minimum_ratio = counts.iter().copied().min().unwrap() as f64 / mean;
                assert!(
                    minimum_ratio > 0.65,
                    "frequency underrepresentation in {region:?}/{gender:?}: {minimum_ratio:.3}"
                );

                let means = cohort
                    .iter()
                    .filter_map(|leader| gap_totals.get(leader))
                    .map(|(sum, count, _)| *sum as f64 / *count as f64)
                    .collect::<Vec<_>>();
                if let (Some(minimum), Some(maximum)) = (
                    means.iter().copied().reduce(f64::min),
                    means.iter().copied().reduce(f64::max),
                ) {
                    let gap_skew = maximum / minimum;
                    cohort_gap_skews.push(gap_skew);
                    assert!(
                        gap_skew < 1.35,
                        "repeat interval skew in {region:?}/{gender:?}: {gap_skew:.3}"
                    );
                }
                let repeats = cohort
                    .iter()
                    .filter_map(|leader| gap_totals.get(leader))
                    .map(|(_, count, _)| count)
                    .sum::<usize>();
                let short_repeats = cohort
                    .iter()
                    .filter_map(|leader| gap_totals.get(leader))
                    .map(|(_, _, count)| count)
                    .sum::<usize>();
                if repeats > 0 {
                    let short_repeat_rate = short_repeats as f64 / repeats as f64;
                    let limit = if cohort.len() <= 2 { 0.60 } else { 0.30 };
                    assert!(
                        short_repeat_rate < limit,
                        "too many two-week repeats in {region:?}/{gender:?} ({} heroes): {short_repeat_rate:.3}",
                        cohort.len()
                    );
                    cohort_short_repeat_rates.push((
                        region,
                        gender,
                        short_repeat_rate,
                        cohort.len(),
                        repeats,
                    ));
                }
            }
        }
        assert_eq!(closest_repeat_gap, 2, "a longer cooldown became accidental");

        let reversed_candidates = candidates.iter().rev().copied().collect::<Vec<_>>();
        let mut reversed_frequencies = std::collections::HashMap::<Leader, usize>::new();
        for seed in 0..100 {
            let mut rng = StdRng::seed_from_u64(seed);
            let mut previous = Vec::new();
            for _ in 0..104 {
                let (rotation, _) =
                    generate_free_rotation(&reversed_candidates, &previous, &[], &mut rng).unwrap();
                assert!(has_all_regions(&rotation));
                assert!(has_balanced_gender(&rotation));
                assert!(rotation.iter().all(|leader| !previous.contains(leader)));
                for leader in &rotation {
                    *reversed_frequencies.entry(*leader).or_default() += 1;
                }
                previous = rotation;
            }
        }
        let catalog_order_skew = candidates
            .iter()
            .map(|leader| {
                let forward = frequencies[leader] as f64;
                let reversed = reversed_frequencies[leader] as f64;
                forward.max(reversed) / forward.min(reversed)
            })
            .fold(0.0_f64, f64::max);
        assert!(
            catalog_order_skew < 1.25,
            "reversing catalog order changed individual hero frequency by {catalog_order_skew:.3}x"
        );

        let mut rng = StdRng::seed_from_u64(10_000);
        let mut previous = Vec::new();
        let mut last_seen = std::collections::HashMap::<Leader, usize>::new();
        let mut closest_long_run_gap = usize::MAX;
        for week in 0..1_000 {
            let (rotation, _) =
                generate_free_rotation(&candidates, &previous, &[], &mut rng).unwrap();
            assert_eq!(rotation.len(), FREE_ROTATION_SIZE);
            assert!(has_all_regions(&rotation));
            assert!(has_balanced_gender(&rotation));
            assert!(rotation.iter().all(|leader| !previous.contains(leader)));
            for leader in &rotation {
                if let Some(last) = last_seen.insert(*leader, week) {
                    closest_long_run_gap = closest_long_run_gap.min(week - last);
                }
            }
            previous = rotation;
        }
        assert_eq!(closest_long_run_gap, 2);
        let highest_short_repeat_share = cohort_short_repeat_rates
            .iter()
            .max_by(|left, right| left.2.total_cmp(&right.2))
            .copied()
            .unwrap();
        let lowest_short_repeat_share = cohort_short_repeat_rates
            .iter()
            .map(|(_, _, rate, _, _)| *rate)
            .reduce(f64::min)
            .unwrap_or_default();
        eprintln!(
            "rotation simulation: 100 x 104 weeks in each catalog order plus 1,000-week stress; max region/gender frequency ratio {:.3}; max repeat-interval skew {:.3}; two-week repeat share {:.3}-{:.3}, highest {:?}/{:?} with {} heroes over {} repeats; max order skew {:.3}x; shortest repeat gap 2 weeks",
            cohort_ratios.into_iter().fold(0.0_f64, f64::max),
            cohort_gap_skews.into_iter().fold(0.0_f64, f64::max),
            lowest_short_repeat_share,
            highest_short_repeat_share.2,
            highest_short_repeat_share.0,
            highest_short_repeat_share.1,
            highest_short_repeat_share.3,
            highest_short_repeat_share.4,
            catalog_order_skew,
        );
    }

    #[cfg(feature = "server")]
    #[test]
    fn queued_heroes_fill_each_valid_rotation_before_spare_candidates() {
        use rand::{SeedableRng, rngs::StdRng};

        let previous = legacy_rotation_for_period(9_200);
        let candidates = Leader::all()
            .filter(|leader| !leader.always_free() && !previous.contains(leader))
            .collect::<Vec<_>>();
        let mut queued = Vec::new();
        for region in [
            Region::Africa,
            Region::Americas,
            Region::Asia,
            Region::Europe,
            Region::Oceania,
        ] {
            queued.push(
                candidates
                    .iter()
                    .copied()
                    .find(|leader| {
                        leader.character().region == region
                            && leader.character().gender == Some(Gender::Female)
                    })
                    .unwrap(),
            );
        }
        for region in [
            Region::Africa,
            Region::Americas,
            Region::Asia,
            Region::Europe,
        ] {
            queued.push(
                candidates
                    .iter()
                    .copied()
                    .find(|leader| {
                        leader.character().region == region
                            && leader.character().gender == Some(Gender::Male)
                    })
                    .unwrap(),
            );
        }
        assert_eq!(queued.len(), 9);

        for seed in 0..100 {
            let mut rng = StdRng::seed_from_u64(seed);
            let (first, deferred) =
                generate_free_rotation(&candidates, &previous, &queued, &mut rng).unwrap();
            assert_eq!(first.len(), FREE_ROTATION_SIZE);
            assert_eq!(deferred.len(), 1, "queued overflow for seed {seed}");
            assert!(has_all_regions(&first));
            assert!(has_balanced_gender(&first));
            assert_eq!(
                queued
                    .iter()
                    .filter(|leader| first.contains(leader))
                    .count(),
                FREE_ROTATION_SIZE,
                "queued candidates should fill all eight slots for seed {seed}"
            );

            let (next, still_deferred) =
                generate_free_rotation(&candidates, &first, &deferred, &mut rng).unwrap();
            assert!(next.contains(&deferred[0]));
            assert!(still_deferred.is_empty());
            assert!(next.iter().all(|leader| !first.contains(leader)));
        }
    }
}
