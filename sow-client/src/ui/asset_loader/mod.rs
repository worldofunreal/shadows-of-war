//! Shared portrait and hero-expression loading; see docs/nameplates-and-emotes.md.
use sow_core::player::Leader;
use std::collections::{HashMap, HashSet};
use web_time::{Duration, Instant};

pub const MAX_AVATAR_FETCHES_IN_FLIGHT: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AvatarFetchKey {
    Fallback,
    Leader(Leader),
    LeaderPortrait(Leader),
    Campaign { slug: String, slot: usize },
}

#[derive(Clone, Debug)]
struct Retry {
    attempts: u32,
    next_retry_at: Instant,
    permanent: bool,
}

pub struct AssetLoader {
    pub map_catalog: Option<Vec<sow_core::maps::MapCatalogEntry>>,
    pub catalog_in_flight: bool,
    pub maps: HashMap<String, Vec<u8>>,
    pub maps_in_flight: HashSet<String>,
    pub avatars: HashMap<Leader, Vec<u8>>,
    pub campaign_avatar_slots: HashMap<String, usize>,
    pub campaign_avatars: HashMap<String, Vec<u8>>,
    pub avatar_fallback: Option<Vec<u8>>,
    pub avatars_fetch_pending: Vec<AvatarFetchKey>,
    pub avatars_in_flight: HashSet<AvatarFetchKey>,
    avatar_retry_state: HashMap<AvatarFetchKey, Retry>,
    pub gpu_avatar_cells: Vec<(usize, Vec<u8>)>,
    pub portal_avatar: Option<Vec<u8>>,
    pub portal_avatar_request: Option<String>,
    pub portal_avatar_in_flight: bool,
}

impl Default for AssetLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetLoader {
    pub fn new() -> Self {
        Self {
            map_catalog: None,
            catalog_in_flight: false,
            maps: HashMap::new(),
            maps_in_flight: HashSet::new(),
            avatars: HashMap::new(),
            campaign_avatar_slots: HashMap::new(),
            campaign_avatars: HashMap::new(),
            avatar_fallback: None,
            avatars_fetch_pending: Vec::new(),
            avatars_in_flight: HashSet::new(),
            avatar_retry_state: HashMap::new(),
            gpu_avatar_cells: Vec::new(),
            portal_avatar: None,
            portal_avatar_request: None,
            portal_avatar_in_flight: false,
        }
    }

    pub fn map_key(name: &str) -> String {
        sow_core::maps::map_key(name)
    }

    pub fn avatar_filename(key: &AvatarFetchKey) -> String {
        match key {
            AvatarFetchKey::Fallback => "null.webp".into(),
            AvatarFetchKey::Leader(leader) => format!(
                "spritesheets/{}.webp",
                sow_data::commerce::leader_id(*leader)
            ),
            AvatarFetchKey::LeaderPortrait(leader) => {
                format!("{}.webp", sow_data::commerce::leader_id(*leader))
            }
            AvatarFetchKey::Campaign { slug, .. } => format!("{slug}.webp"),
        }
    }

    pub fn queue_leader_avatar(&mut self, leader: Leader, priority: bool) {
        self.queue_avatar_fetch(AvatarFetchKey::Fallback, true);
        let sheet = AvatarFetchKey::Leader(leader);
        let key = if self
            .avatar_retry_state
            .get(&sheet)
            .is_some_and(|retry| retry.permanent)
        {
            AvatarFetchKey::LeaderPortrait(leader)
        } else {
            sheet
        };
        self.queue_avatar_fetch(key, priority);
    }

    pub fn queue_campaign_avatar(&mut self, slug: &str) {
        if slug == "null" || !valid_avatar_id(slug) {
            return;
        }
        if let Some(&leader) = Leader::ALL
            .iter()
            .find(|leader| sow_data::commerce::leader_id(**leader) == slug)
        {
            self.campaign_avatar_slots.insert(
                slug.to_owned(),
                sow_render::text::leader_avatar_slot(
                    leader,
                    sow_core::emote::AvatarExpression::Calm,
                ),
            );
            self.queue_leader_avatar(leader, false);
            return;
        }
        let slot = if let Some(slot) = self.campaign_avatar_slots.get(slug) {
            *slot
        } else {
            let static_count = self
                .campaign_avatar_slots
                .values()
                .filter(|slot| **slot >= sow_render::text::AVATAR_CAMPAIGN_FIRST_SLOT)
                .count();
            let slot = sow_render::text::AVATAR_CAMPAIGN_FIRST_SLOT + static_count;
            if slot >= sow_render::text::AVATAR_SLOT_COUNT {
                log::error!("Avatar atlas exhausted while allocating {slug}");
                return;
            }
            self.campaign_avatar_slots.insert(slug.to_string(), slot);
            slot
        };
        self.queue_avatar_fetch(
            AvatarFetchKey::Campaign {
                slug: slug.to_string(),
                slot,
            },
            false,
        );
    }

    pub fn campaign_avatar_slot(&self, slug: &str) -> Option<usize> {
        if slug == "null" {
            Some(sow_render::text::AVATAR_FALLBACK_SLOT)
        } else {
            self.campaign_avatar_slots.get(slug).copied()
        }
    }

    pub fn has_map(&self, name: &str) -> bool {
        let key = Self::map_key(name);
        self.maps.contains_key(&key) || sow_core::maps::map_payload_available(&key)
    }

    pub fn take_map(&mut self, name: &str) -> Option<Vec<u8>> {
        let key = Self::map_key(name);
        sow_core::maps::load_map_br_payload(&key, self.maps.remove(&key))
    }

    fn avatar_loaded(&self, key: &AvatarFetchKey) -> bool {
        match key {
            AvatarFetchKey::Fallback => self.avatar_fallback.is_some(),
            AvatarFetchKey::Leader(leader) | AvatarFetchKey::LeaderPortrait(leader) => {
                self.avatars.contains_key(&leader)
            }
            AvatarFetchKey::Campaign { slug, .. } => self.campaign_avatars.contains_key(slug),
        }
    }

    fn retry_ready(&self, key: &AvatarFetchKey) -> bool {
        self.avatar_retry_state
            .get(key)
            .map(|retry| !retry.permanent && Instant::now() >= retry.next_retry_at)
            .unwrap_or(true)
    }

    fn queue_avatar_fetch(&mut self, key: AvatarFetchKey, front: bool) {
        if self.avatar_loaded(&key)
            || self.avatars_in_flight.contains(&key)
            || !self.retry_ready(&key)
            || self.avatars_fetch_pending.contains(&key)
        {
            return;
        }
        if front {
            self.avatars_fetch_pending.insert(0, key);
        } else {
            self.avatars_fetch_pending.push(key);
        }
    }

    pub fn take_next_avatar_fetch_pending(
        &mut self,
        priority: &AvatarFetchKey,
    ) -> Option<AvatarFetchKey> {
        if self.avatars_in_flight.len() >= MAX_AVATAR_FETCHES_IN_FLIGHT {
            return None;
        }
        let index = self
            .avatars_fetch_pending
            .iter()
            .position(|key| key == priority)
            .or((!self.avatars_fetch_pending.is_empty()).then_some(0))?;
        let key = self.avatars_fetch_pending.remove(index);
        if self.avatar_loaded(&key) {
            return self.take_next_avatar_fetch_pending(priority);
        }
        self.avatars_in_flight.insert(key.clone());
        Some(key)
    }

    pub fn note_avatar_fetch_failed(&mut self, key: AvatarFetchKey, reason: impl Into<String>) {
        self.avatars_in_flight.remove(&key);
        let reason = reason.into();
        let permanent_error = reason.contains("404")
            || reason.contains("decode avatar")
            || reason.contains("invalid hero sheet");
        let attempts = self
            .avatar_retry_state
            .get(&key)
            .map(|retry| retry.attempts + 1)
            .unwrap_or(1);
        let permanent = permanent_error || attempts >= 3;
        if permanent && let AvatarFetchKey::Leader(leader) = key {
            log::warn!(
                "Hero sheet unavailable for {}: {reason}; using static portrait",
                sow_data::commerce::leader_id(leader)
            );
            self.queue_avatar_fetch(AvatarFetchKey::LeaderPortrait(leader), true);
        }
        self.avatar_retry_state.insert(
            key,
            Retry {
                attempts,
                next_retry_at: Instant::now()
                    + Duration::from_millis(500 * (1 << attempts.saturating_sub(1).min(4))),
                permanent,
            },
        );
    }

    pub fn ingest_avatar_webp_bytes(
        &mut self,
        key: AvatarFetchKey,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.avatars_in_flight.remove(&key);
        self.avatar_retry_state.remove(&key);
        let image = image::load_from_memory(bytes)
            .map_err(|error| format!("decode avatar: {error}"))?
            .to_rgba8();
        let cells = match key {
            AvatarFetchKey::Leader(leader) => hero_avatar_cells(&image)?
                .into_iter()
                .enumerate()
                .map(|(index, cell)| {
                    (
                        sow_render::text::leader_avatar_slot(
                            leader,
                            sow_core::emote::AvatarExpression::ALL[index],
                        ),
                        cell,
                    )
                })
                .collect::<Vec<_>>(),
            AvatarFetchKey::LeaderPortrait(leader) => {
                let cell = square_avatar_cell(&image);
                sow_core::emote::AvatarExpression::ALL
                    .into_iter()
                    .map(|expression| {
                        (
                            sow_render::text::leader_avatar_slot(leader, expression),
                            cell.clone(),
                        )
                    })
                    .collect()
            }
            AvatarFetchKey::Fallback => vec![(
                sow_render::text::AVATAR_FALLBACK_SLOT,
                square_avatar_cell(&image),
            )],
            AvatarFetchKey::Campaign { slot, .. } => vec![(slot, square_avatar_cell(&image))],
        };
        for (slot, cell) in cells {
            self.gpu_avatar_cells
                .retain(|(loaded_slot, _)| *loaded_slot != slot);
            self.gpu_avatar_cells.push((slot, cell));
        }
        match key {
            AvatarFetchKey::Fallback => self.avatar_fallback = Some(bytes.to_vec()),
            AvatarFetchKey::Leader(leader) | AvatarFetchKey::LeaderPortrait(leader) => {
                self.avatars.insert(leader, bytes.to_vec());
            }
            AvatarFetchKey::Campaign { slug, .. } => {
                self.campaign_avatars.insert(slug, bytes.to_vec());
            }
        }
        Ok(())
    }

    pub fn queue_portal_avatar(&mut self, url: String) {
        if self.portal_avatar.is_none() && !self.portal_avatar_in_flight {
            self.portal_avatar_request = Some(url);
        }
    }

    pub fn note_portal_avatar_failed(&mut self, _reason: impl Into<String>) {
        self.portal_avatar_in_flight = false;
        self.portal_avatar_request = None;
    }

    pub fn ingest_portal_avatar_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        image::load_from_memory(bytes).map_err(|error| format!("decode avatar: {error}"))?;
        self.portal_avatar = Some(bytes.to_vec());
        self.portal_avatar_in_flight = false;
        self.portal_avatar_request = None;
        Ok(())
    }

    pub fn get_assets_to_fetch(
        &mut self,
        lobbies: &[sow_core::protocol::LobbyInfo],
    ) -> Vec<String> {
        lobbies
            .iter()
            .filter_map(|lobby| {
                let key = Self::map_key(&lobby.map_name);
                (!self.has_map(&key) && self.maps_in_flight.insert(key.clone())).then_some(key)
            })
            .collect()
    }
}

fn square_avatar_cell(image: &image::RgbaImage) -> Vec<u8> {
    let size = image.width().min(image.height()).max(1);
    let square = image::imageops::crop_imm(
        image,
        (image.width() - size) / 2,
        (image.height() - size) / 2,
        size,
        size,
    )
    .to_image();
    image::imageops::resize(
        &square,
        sow_render::text::AVATAR_CELL,
        sow_render::text::AVATAR_CELL,
        image::imageops::FilterType::Triangle,
    )
    .into_raw()
}

fn hero_avatar_cells(image: &image::RgbaImage) -> Result<Vec<Vec<u8>>, String> {
    if image.width() < 4 || image.height() < 4 {
        return Err("invalid hero sheet dimensions".into());
    }
    let half_w = image.width() / 2;
    let half_h = image.height() / 2;
    // Trim the source separators before square cropping; odd and non-square sheets are valid.
    let inset = ((half_w.min(half_h) as f32 * 0.01).round() as u32).max(1);
    Ok(sow_core::emote::AvatarExpression::ALL
        .into_iter()
        .map(|expression| {
            let index = expression as u32;
            let quadrant = image::imageops::crop_imm(
                image,
                (index % 2) * half_w + inset,
                (index / 2) * half_h + inset,
                half_w.saturating_sub(2 * inset).max(1),
                half_h.saturating_sub(2 * inset).max(1),
            )
            .to_image();
            square_avatar_cell(&quadrant)
        })
        .collect())
}

fn valid_avatar_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hero_quadrants_are_square_and_odd_sheets_do_not_distort() {
        let mut source = image::RgbaImage::new(976, 1087);
        let colors = [
            [240, 10, 10, 255],
            [10, 240, 10, 255],
            [10, 10, 240, 255],
            [240, 240, 10, 255],
        ];
        for (x, y, pixel) in source.enumerate_pixels_mut() {
            let column = usize::from(x >= 488);
            let row = usize::from(y >= 543);
            *pixel = image::Rgba(colors[row * 2 + column]);
        }
        let cells = hero_avatar_cells(&source).unwrap();
        for (index, cell) in cells.iter().enumerate() {
            assert_eq!(cell.len(), 256 * 256 * 4);
            assert!(cell.chunks_exact(4).all(|pixel| pixel == colors[index]));
        }
    }

    #[test]
    fn all_playable_hero_sheets_exist_and_have_four_distinct_frames() {
        for leader in Leader::ALL {
            let key = AvatarFetchKey::Leader(leader);
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../assets/gameplay/avatars")
                .join(AssetLoader::avatar_filename(&key));
            let bytes =
                std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let mut loader = AssetLoader::new();
            loader.ingest_avatar_webp_bytes(key, &bytes).unwrap();
            assert_eq!(loader.gpu_avatar_cells.len(), 4);
            for (index, (slot, cell)) in loader.gpu_avatar_cells.iter().enumerate() {
                assert_eq!(
                    *slot,
                    sow_render::text::leader_avatar_slot(
                        leader,
                        sow_core::emote::AvatarExpression::ALL[index]
                    )
                );
                assert_eq!(cell.len(), 256 * 256 * 4);
                assert!(
                    loader
                        .gpu_avatar_cells
                        .iter()
                        .take(index)
                        .all(|(_, other)| other != cell)
                );
            }
        }
    }

    #[test]
    fn hero_campaign_identity_shares_slots_and_failed_sheet_queues_static_fallback() {
        let mut loader = AssetLoader::new();
        loader.queue_campaign_avatar("boudica");
        assert_eq!(
            loader.campaign_avatar_slot("boudica"),
            Some(sow_render::text::leader_avatar_slot(
                Leader::Boudica,
                sow_core::emote::AvatarExpression::Calm
            ))
        );
        loader.note_avatar_fetch_failed(AvatarFetchKey::Leader(Leader::Boudica), "HTTP 404");
        assert!(
            loader
                .avatars_fetch_pending
                .contains(&AvatarFetchKey::LeaderPortrait(Leader::Boudica))
        );
        assert!(
            loader
                .ingest_avatar_webp_bytes(AvatarFetchKey::Leader(Leader::Boudica), b"broken sheet")
                .is_err()
        );
    }

    #[test]
    fn avatar_slots_cover_builtin_and_geo_entity_portraits_without_dropping_them() {
        let mut loader = AssetLoader::new();
        let mut entity_avatars = sow_data::geo_entities::all()
            .filter_map(|entity| entity.avatar.clone())
            .collect::<HashSet<_>>();
        fn collect_avatars(value: &serde_json::Value, out: &mut HashSet<String>) {
            match value {
                serde_json::Value::Object(object) => {
                    for (key, value) in object {
                        if key == "avatar"
                            && let Some(slug) = value.as_str()
                        {
                            out.insert(slug.to_owned());
                        }
                        collect_avatars(value, out);
                    }
                }
                serde_json::Value::Array(array) => {
                    for value in array {
                        collect_avatars(value, out);
                    }
                }
                _ => {}
            }
        }
        let campaigns = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/campaign");
        for entry in std::fs::read_dir(campaigns).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                collect_avatars(
                    &serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap(),
                    &mut entity_avatars,
                );
            }
        }
        entity_avatars.remove("null");
        assert!(
            entity_avatars.len() + sow_render::text::AVATAR_CAMPAIGN_FIRST_SLOT
                <= sow_render::text::AVATAR_SLOT_COUNT
        );
        for avatar in &entity_avatars {
            loader.queue_campaign_avatar(avatar);
        }
        assert_eq!(loader.campaign_avatar_slots.len(), entity_avatars.len());
        assert!(
            loader
                .campaign_avatar_slots
                .values()
                .all(|slot| *slot < sow_render::text::AVATAR_SLOT_COUNT)
        );
        assert_eq!(
            loader
                .campaign_avatar_slots
                .values()
                .copied()
                .collect::<HashSet<_>>()
                .len(),
            entity_avatars.len()
        );
    }
}
