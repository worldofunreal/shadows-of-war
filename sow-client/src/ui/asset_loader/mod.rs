//! Shared portrait and hero-expression loading; see docs/nameplates-and-emotes.md.
use sow_core::player::Leader;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::OnceLock;
use web_time::{Duration, Instant};

pub const MAX_AVATAR_FETCHES_IN_FLIGHT: usize = 6;
pub const MAX_AVATAR_PREFETCHES_IN_FLIGHT: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AvatarFetchKey {
    Fallback,
    Leader(Leader),
    LeaderPortrait(Leader),
    Campaign { slug: String, slot: usize },
    CampaignPrefetch(String),
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
    leader_sheets: HashSet<Leader>,
    leader_portraits: HashSet<Leader>,
    pub campaign_avatar_slots: HashMap<String, usize>,
    pub campaign_avatars: HashSet<String>,
    avatar_fallback_loaded: bool,
    pub avatars_fetch_pending: Vec<AvatarFetchKey>,
    pub avatars_in_flight: HashSet<AvatarFetchKey>,
    avatar_retry_state: HashMap<AvatarFetchKey, Retry>,
    avatar_prefetch_pending: VecDeque<String>,
    avatar_prefetch_queued: HashSet<String>,
    avatar_prefetch_retry: HashSet<String>,
    avatar_prefetch_catalog: HashSet<String>,
    avatar_prefetched: HashSet<String>,
    avatar_prefetch_wants: HashMap<String, usize>,
    avatar_prefetch_started: bool,
    drawn_avatars: HashSet<String>,
    pub gpu_avatar_cells: Vec<(usize, Vec<u8>)>,
    /// Slots whose cell changed since the renderer last uploaded them.
    pub gpu_avatar_dirty: Vec<usize>,
    /// Advances once per frame in `request_drawn_portraits`.
    drawn_frame: u64,
    /// Frame each portrait slot was last drawn in; the least recently drawn one is evicted first.
    portrait_last_drawn: Vec<u64>,
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
            leader_sheets: HashSet::new(),
            leader_portraits: HashSet::new(),
            campaign_avatar_slots: HashMap::new(),
            campaign_avatars: HashSet::new(),
            avatar_fallback_loaded: false,
            avatars_fetch_pending: Vec::new(),
            avatars_in_flight: HashSet::new(),
            avatar_retry_state: HashMap::new(),
            avatar_prefetch_pending: VecDeque::new(),
            avatar_prefetch_queued: HashSet::new(),
            avatar_prefetch_retry: HashSet::new(),
            avatar_prefetch_catalog: HashSet::new(),
            avatar_prefetched: HashSet::new(),
            avatar_prefetch_wants: HashMap::new(),
            avatar_prefetch_started: false,
            drawn_avatars: HashSet::new(),
            gpu_avatar_cells: vec![(
                sow_render::text::AVATAR_GOLD_SLOT,
                gold_icon_cell().to_vec(),
            )],
            gpu_avatar_dirty: Vec::new(),
            drawn_frame: 0,
            portrait_last_drawn: vec![0; sow_render::text::AVATAR_SLOT_COUNT],
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
            AvatarFetchKey::Leader(leader) => format!("spritesheets/{}.webp", leader.art_slug()),
            AvatarFetchKey::LeaderPortrait(leader) => format!("{}.webp", leader.art_slug()),
            AvatarFetchKey::Campaign { slug, .. } => format!("{slug}.webp"),
            AvatarFetchKey::CampaignPrefetch(slug) => format!("{slug}.webp"),
        }
    }

    /// Fetch the hero's expression sheet, falling back to its generated Calm portrait after a
    /// permanent sheet failure.
    pub fn leader_fetch_key(&self, leader: Leader) -> AvatarFetchKey {
        let sheet = AvatarFetchKey::Leader(leader);
        if self
            .avatar_retry_state
            .get(&sheet)
            .is_some_and(|retry| retry.permanent)
        {
            AvatarFetchKey::LeaderPortrait(leader)
        } else {
            sheet
        }
    }

    pub fn queue_leader_avatar(&mut self, leader: Leader, priority: bool) {
        self.queue_avatar_fetch(AvatarFetchKey::Fallback, true);
        let sheet = AvatarFetchKey::Leader(leader);
        if !self.leader_sheets.contains(&leader)
            && !self.avatars_in_flight.contains(&sheet)
            && !self.avatars_fetch_pending.contains(&sheet)
        {
            self.queue_avatar_fetch(AvatarFetchKey::LeaderPortrait(leader), priority);
        }
    }

    pub fn queue_leader_expressions(&mut self, leader: Leader, priority: bool) {
        self.queue_avatar_fetch(AvatarFetchKey::Fallback, true);
        let key = self.leader_fetch_key(leader);
        if matches!(&key, AvatarFetchKey::Leader(_)) {
            self.avatars_fetch_pending
                .retain(|pending| *pending != AvatarFetchKey::LeaderPortrait(leader));
        }
        self.queue_avatar_fetch(key, priority);
    }

    pub fn queue_campaign_avatar(&mut self, slug: &str) {
        if slug == "null" || !valid_avatar_id(slug) {
            return;
        }
        if let Some(leader) = Leader::all().find(|leader| leader.id() == slug) {
            if let Some(slot) = self.campaign_avatar_slots.get(slug).copied() {
                self.portrait_last_drawn[slot] = self.drawn_frame;
            } else {
                self.campaign_avatar_slots.insert(
                    slug.to_owned(),
                    sow_render::text::leader_avatar_slot(
                        leader,
                        sow_core::emote::AvatarExpression::Calm,
                    ),
                );
            }
            self.queue_leader_expressions(leader, true);
            return;
        }
        let Some(slot) = self.portrait_slot(slug) else {
            return;
        };
        if self.campaign_avatars.contains(slug) {
            return;
        }
        let prefetch = AvatarFetchKey::CampaignPrefetch(slug.to_string());
        if self.avatars_in_flight.contains(&prefetch) {
            self.avatar_prefetch_wants.insert(slug.to_owned(), slot);
            return;
        }
        if self.avatar_prefetch_queued.remove(slug) {
            self.avatar_prefetch_pending.retain(|queued| queued != slug);
        }
        self.queue_avatar_fetch(
            AvatarFetchKey::Campaign {
                slug: slug.to_owned(),
                slot,
            },
            false,
        );
    }

    /// Requests the portraits drawn this frame. Portraits not in this list may be evicted.
    pub fn request_drawn_portraits(&mut self, slugs: &[String]) {
        self.drawn_frame += 1;
        let wanted: HashSet<_> = slugs.iter().cloned().collect();
        self.drawn_avatars.clone_from(&wanted);
        let mut stale = Vec::new();
        self.avatars_fetch_pending.retain(|key| match key {
            AvatarFetchKey::Campaign { slug, .. } if !wanted.contains(slug) => {
                stale.push(slug.clone());
                false
            }
            _ => true,
        });
        self.avatar_prefetch_wants
            .retain(|slug, _| wanted.contains(slug));
        for slug in stale {
            if self.avatar_prefetch_catalog.contains(&slug) {
                self.enqueue_avatar_prefetch(&slug);
            }
        }
        for slug in slugs {
            self.queue_campaign_avatar(slug);
        }
    }

    pub fn start_tutorial_bot_avatar_prefetch(&mut self) {
        if self.avatar_prefetch_started {
            return;
        }
        self.avatar_prefetch_started = true;
        for role in [
            sow_core::geo_entities::Role::Tribe,
            sow_core::geo_entities::Role::Nation,
        ] {
            for entity in sow_core::geo_entities::bots(role) {
                let Some(slug) = entity
                    .avatar
                    .as_deref()
                    .filter(|slug| valid_avatar_id(slug))
                else {
                    continue;
                };
                if self.avatar_prefetch_catalog.insert(slug.to_owned()) {
                    self.enqueue_avatar_prefetch(slug);
                }
            }
        }
    }

    pub fn pump_tutorial_avatar_prefetch_retries(&mut self) {
        let retry: Vec<_> = self.avatar_prefetch_retry.iter().cloned().collect();
        for slug in retry {
            let key = AvatarFetchKey::CampaignPrefetch(slug.clone());
            if self
                .avatar_retry_state
                .get(&key)
                .is_some_and(|state| state.permanent)
            {
                self.avatar_prefetch_retry.remove(&slug);
            } else if self.retry_ready(&key) {
                self.avatar_prefetch_retry.remove(&slug);
                self.enqueue_avatar_prefetch(&slug);
            }
        }
    }

    fn enqueue_avatar_prefetch(&mut self, slug: &str) {
        let key = AvatarFetchKey::CampaignPrefetch(slug.to_owned());
        if self.avatar_prefetched.contains(slug)
            || self.campaign_avatars.contains(slug)
            || self.avatars_in_flight.contains(&key)
            || self.avatar_prefetch_queued.contains(slug)
            || self.avatars_fetch_pending.iter().any(
                |pending| matches!(pending, AvatarFetchKey::Campaign { slug: queued, .. } if queued == slug),
            )
            || self.avatars_in_flight.iter().any(
                |pending| matches!(pending, AvatarFetchKey::Campaign { slug: queued, .. } if queued == slug),
            )
            || !self.retry_ready(&key)
        {
            return;
        }
        self.avatar_prefetch_queued.insert(slug.to_owned());
        self.avatar_prefetch_pending.push_back(slug.to_owned());
    }

    pub fn take_next_avatar_prefetch_pending(&mut self) -> Option<AvatarFetchKey> {
        let pending = self.avatar_prefetch_pending.len();
        for _ in 0..pending {
            let slug = self.avatar_prefetch_pending.pop_front()?;
            self.avatar_prefetch_queued.remove(&slug);
            let key = AvatarFetchKey::CampaignPrefetch(slug.clone());
            if self.avatar_prefetched.contains(&slug)
                || self.campaign_avatars.contains(&slug)
                || self.avatars_in_flight.contains(&key)
            {
                continue;
            }
            if !self.retry_ready(&key) {
                if self
                    .avatar_retry_state
                    .get(&key)
                    .is_some_and(|state| !state.permanent)
                {
                    self.avatar_prefetch_queued.insert(slug.clone());
                    self.avatar_prefetch_pending.push_back(slug);
                }
                continue;
            }
            if self.avatars_fetch_pending.iter().any(
                |pending| matches!(pending, AvatarFetchKey::Campaign { slug: queued, .. } if *queued == slug),
            ) || self.avatars_in_flight.iter().any(
                |pending| matches!(pending, AvatarFetchKey::Campaign { slug: queued, .. } if *queued == slug),
            ) {
                continue;
            }
            self.avatars_in_flight.insert(key.clone());
            return Some(key);
        }
        None
    }

    pub fn avatar_prefetch_in_flight(&self) -> usize {
        self.avatars_in_flight
            .iter()
            .filter(|key| matches!(key, AvatarFetchKey::CampaignPrefetch(_)))
            .count()
    }

    /// The slot for a portrait drawn this frame. A full pool evicts the least recently drawn
    /// portrait; heroes and their slots are never evicted.
    // ponytail: fixed-capacity LRU after permanent hero blocks; further catalog growth lowers
    // AVATAR_CELL while keeping both atlas sides within the 4096 px GPU limit.
    fn portrait_slot(&mut self, slug: &str) -> Option<usize> {
        if let Some(slot) = self.campaign_avatar_slots.get(slug).copied() {
            self.portrait_last_drawn[slot] = self.drawn_frame;
            return Some(slot);
        }
        let first = sow_render::text::avatar_campaign_first_slot();
        let Some(slot) = (first..sow_render::text::AVATAR_SLOT_COUNT)
            .filter(|slot| self.portrait_last_drawn[*slot] < self.drawn_frame)
            .min_by_key(|slot| self.portrait_last_drawn[*slot])
        else {
            log::warn!("Avatar atlas full of portraits drawn this frame; {slug} stays on fallback");
            return None;
        };
        if let Some(evicted) = self
            .campaign_avatar_slots
            .iter()
            .find(|(_, owner)| **owner == slot)
            .map(|(evicted, _)| evicted.clone())
        {
            self.campaign_avatar_slots.remove(&evicted);
            self.campaign_avatars.remove(&evicted);
            self.avatars_fetch_pending.retain(
                |key| !matches!(key, AvatarFetchKey::Campaign { slug, .. } if *slug == evicted),
            );
            if self.avatar_prefetch_catalog.contains(&evicted) {
                self.enqueue_avatar_prefetch(&evicted);
            }
            // Blank until the new portrait arrives, so the evicted face never shows under the new owner.
            let blank = vec![0; (sow_render::text::AVATAR_CELL as usize).pow(2) * 4];
            self.push_gpu_cell(slot, blank);
        }
        self.campaign_avatar_slots.insert(slug.to_owned(), slot);
        self.portrait_last_drawn[slot] = self.drawn_frame;
        Some(slot)
    }

    fn push_gpu_cell(&mut self, slot: usize, cell: Vec<u8>) {
        self.gpu_avatar_cells
            .retain(|(loaded_slot, _)| *loaded_slot != slot);
        self.gpu_avatar_cells.push((slot, cell));
        if !self.gpu_avatar_dirty.contains(&slot) {
            self.gpu_avatar_dirty.push(slot);
        }
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

    pub fn restore_cached_map(&mut self, name: &str) -> bool {
        let key = Self::map_key(name);
        if self.has_map(&key) {
            return true;
        }
        if let Some(bytes) = crate::map_cache::load(&key) {
            self.maps.insert(key, bytes);
            return true;
        }
        false
    }

    pub fn take_map(&mut self, name: &str) -> Option<Vec<u8>> {
        let key = Self::map_key(name);
        sow_core::maps::load_map_br_payload(&key, self.maps.remove(&key))
    }

    fn avatar_loaded(&self, key: &AvatarFetchKey) -> bool {
        match key {
            AvatarFetchKey::Fallback => self.avatar_fallback_loaded,
            AvatarFetchKey::Leader(leader) => self.leader_sheets.contains(leader),
            AvatarFetchKey::LeaderPortrait(leader) => {
                self.leader_sheets.contains(leader) || self.leader_portraits.contains(leader)
            }
            AvatarFetchKey::Campaign { slug, .. } => self.campaign_avatars.contains(slug),
            AvatarFetchKey::CampaignPrefetch(slug) => {
                self.avatar_prefetched.contains(slug) || self.campaign_avatars.contains(slug)
            }
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
        if permanent && let AvatarFetchKey::Leader(leader) = &key {
            log::warn!(
                "Hero sheet unavailable for {}: {reason}; using static portrait",
                sow_data::commerce::leader_id(*leader)
            );
            self.queue_avatar_fetch(AvatarFetchKey::LeaderPortrait(*leader), true);
        }
        let retry = Retry {
            attempts,
            next_retry_at: Instant::now()
                + Duration::from_millis(500 * (1 << attempts.saturating_sub(1).min(4))),
            permanent,
        };
        self.avatar_retry_state.insert(key.clone(), retry.clone());
        match &key {
            AvatarFetchKey::CampaignPrefetch(slug) => {
                if let Some(slot) = self.avatar_prefetch_wants.remove(slug) {
                    self.queue_avatar_fetch(
                        AvatarFetchKey::Campaign {
                            slug: slug.clone(),
                            slot,
                        },
                        true,
                    );
                }
                if !permanent {
                    self.avatar_prefetch_retry.insert(slug.clone());
                }
            }
            AvatarFetchKey::Campaign { slug, .. }
                if self.avatar_prefetch_catalog.contains(slug) && !permanent =>
            {
                self.avatar_retry_state
                    .insert(AvatarFetchKey::CampaignPrefetch(slug.clone()), retry);
                self.avatar_prefetch_retry.insert(slug.clone());
            }
            _ => {}
        }
    }

    pub fn ingest_avatar_webp_bytes(
        &mut self,
        key: AvatarFetchKey,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.avatars_in_flight.remove(&key);
        self.avatar_retry_state.remove(&key);
        if let AvatarFetchKey::CampaignPrefetch(slug) = &key {
            self.avatar_prefetched.insert(slug.clone());
            let Some(slot) = self.avatar_prefetch_wants.remove(slug) else {
                return Ok(());
            };
            if self.campaign_avatar_slots.get(slug) != Some(&slot) {
                return Ok(());
            }
            let image = image::load_from_memory(bytes)
                .map_err(|error| format!("decode avatar: {error}"))?
                .to_rgba8();
            let campaign_key = AvatarFetchKey::Campaign {
                slug: slug.clone(),
                slot,
            };
            self.store_avatar_cells(campaign_key, image)?;
            self.campaign_avatars.insert(slug.clone());
            return Ok(());
        }
        if let AvatarFetchKey::Campaign { slug, slot } = &key
            && (self.campaign_avatar_slots.get(slug) != Some(slot)
                || !self.drawn_avatars.contains(slug))
        {
            // The response also warmed the browser's HTTP cache.
            self.avatar_prefetched.insert(slug.clone());
            return Ok(());
        }
        let image = image::load_from_memory(bytes)
            .map_err(|error| format!("decode avatar: {error}"))?
            .to_rgba8();
        self.store_avatar_cells(key.clone(), image)?;
        match key {
            AvatarFetchKey::Fallback => self.avatar_fallback_loaded = true,
            AvatarFetchKey::Leader(leader) => {
                self.leader_sheets.insert(leader);
            }
            AvatarFetchKey::LeaderPortrait(leader) => {
                self.leader_portraits.insert(leader);
            }
            AvatarFetchKey::Campaign { slug, .. } => {
                self.campaign_avatars.insert(slug.clone());
                self.avatar_prefetched.insert(slug);
            }
            AvatarFetchKey::CampaignPrefetch(_) => unreachable!("prefetch handled above"),
        }
        Ok(())
    }

    /// Decoded sheets fill all expressions; a Calm portrait fills one hero slot.
    fn store_avatar_cells(
        &mut self,
        key: AvatarFetchKey,
        image: image::RgbaImage,
    ) -> Result<(), String> {
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
                vec![(
                    sow_render::text::leader_avatar_slot(
                        leader,
                        sow_core::emote::AvatarExpression::Calm,
                    ),
                    square_avatar_cell(&image),
                )]
            }
            AvatarFetchKey::Fallback => vec![(
                sow_render::text::AVATAR_FALLBACK_SLOT,
                square_avatar_cell(&image),
            )],
            AvatarFetchKey::Campaign { slot, .. } => vec![(slot, square_avatar_cell(&image))],
            AvatarFetchKey::CampaignPrefetch(_) => {
                return Err("cache-only avatar prefetch cannot fill an atlas slot".into());
            }
        };
        for (slot, cell) in cells {
            self.push_gpu_cell(slot, cell);
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
                (!self.restore_cached_map(&key) && self.maps_in_flight.insert(key.clone()))
                    .then_some(key)
            })
            .collect()
    }
}

fn gold_icon_cell() -> &'static [u8] {
    static CELL: OnceLock<Vec<u8>> = OnceLock::new();
    CELL.get_or_init(|| {
        let image =
            image::load_from_memory(include_bytes!("../../../../assets/art/icons/gold.webp"))
                .expect("embedded gold icon must be a valid image");
        square_avatar_cell(&image.to_rgba8())
    })
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
    use wasm_bindgen_test::wasm_bindgen_test;

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
            assert_eq!(
                cell.len(),
                (sow_render::text::AVATAR_CELL as usize).pow(2) * 4
            );
            assert!(cell.chunks_exact(4).all(|pixel| pixel == colors[index]));
        }
    }

    #[test]
    fn all_playable_heroes_load_four_frames_from_one_sheet() {
        let avatars =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/art/avatars/heroes");
        for leader in Leader::all() {
            let key = AvatarFetchKey::Leader(leader);
            let id = sow_data::commerce::leader_id(leader);
            let path = avatars.join(format!("{}.webp", leader.art_slug()));
            let sheet = image::open(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
                .to_rgba8();
            let mut loader = AssetLoader::new();
            loader.store_avatar_cells(key, sheet).unwrap();
            assert!(loader.gpu_avatar_cells.iter().any(|(slot, cell)| {
                *slot == sow_render::text::AVATAR_GOLD_SLOT
                    && cell.len() == (sow_render::text::AVATAR_CELL as usize).pow(2) * 4
            }));
            let hero_cells = loader
                .gpu_avatar_cells
                .iter()
                .filter(|entry| entry.0 != sow_render::text::AVATAR_GOLD_SLOT)
                .collect::<Vec<_>>();
            assert_eq!(hero_cells.len(), 4, "{id}");
            for (index, (slot, cell)) in hero_cells.into_iter().enumerate() {
                assert_eq!(
                    *slot,
                    sow_render::text::leader_avatar_slot(
                        leader,
                        sow_core::emote::AvatarExpression::ALL[index]
                    )
                );
                assert_eq!(
                    cell.len(),
                    (sow_render::text::AVATAR_CELL as usize).pow(2) * 4
                );
            }
        }
    }

    #[test]
    fn calm_portrait_fallback_fills_only_the_calm_slot() {
        let leader = Leader::of("boudica");
        let mut loader = AssetLoader::new();
        let portrait = image::RgbaImage::from_pixel(256, 256, image::Rgba([12, 34, 56, 255]));
        loader
            .store_avatar_cells(AvatarFetchKey::LeaderPortrait(leader), portrait)
            .unwrap();
        let cells = loader
            .gpu_avatar_cells
            .iter()
            .filter(|entry| entry.0 != sow_render::text::AVATAR_GOLD_SLOT)
            .collect::<Vec<_>>();
        assert_eq!(cells.len(), 1);
        assert_eq!(
            cells[0].0,
            sow_render::text::leader_avatar_slot(
                leader,
                sow_core::emote::AvatarExpression::Calm
            )
        );
    }

    #[test]
    fn hero_campaign_identity_shares_slots_and_failed_sheet_queues_static_fallback() {
        let mut loader = AssetLoader::new();
        loader.queue_campaign_avatar("boudica");
        assert_eq!(
            loader.campaign_avatar_slot("boudica"),
            Some(sow_render::text::leader_avatar_slot(
                Leader::of("boudica"),
                sow_core::emote::AvatarExpression::Calm
            ))
        );
        loader.note_avatar_fetch_failed(AvatarFetchKey::Leader(Leader::of("boudica")), "HTTP 404");
        assert!(
            loader
                .avatars_fetch_pending
                .contains(&AvatarFetchKey::LeaderPortrait(Leader::of("boudica")))
        );
        assert!(
            loader
                .ingest_avatar_webp_bytes(
                    AvatarFetchKey::Leader(Leader::of("boudica")),
                    b"broken sheet"
                )
                .is_err()
        );
    }

    #[test]
    fn drawn_portraits_evict_the_least_recently_drawn_slot() {
        let mut loader = AssetLoader::new();
        let first = sow_render::text::avatar_campaign_first_slot();
        let pool = sow_render::text::AVATAR_SLOT_COUNT - first;
        for n in 0..pool {
            loader.request_drawn_portraits(&[format!("bot_{n}")]);
        }
        assert_eq!(loader.campaign_avatar_slot("bot_0"), Some(first));
        // bot_0 is drawn again, so bot_1 is the least recently drawn and gives up its slot.
        loader.request_drawn_portraits(&["bot_0".into(), "bot_extra".into()]);
        assert_eq!(loader.campaign_avatar_slot("bot_0"), Some(first));
        assert_eq!(loader.campaign_avatar_slot("bot_1"), None);
        assert_eq!(loader.campaign_avatar_slot("bot_extra"), Some(first + 1));
        assert!(loader.gpu_avatar_dirty.contains(&(first + 1)));
    }

    #[wasm_bindgen_test]
    fn death_avatar_requests_cold_portraits_and_releases_the_slot_next_frame() {
        let mut loader = AssetLoader::new();
        loader.request_drawn_portraits(&["stonea".into()]);

        let slot = loader.campaign_avatar_slot("stonea").unwrap();
        assert!(!loader.campaign_avatars.contains("stonea"));
        assert!(
            loader
                .avatars_fetch_pending
                .contains(&AvatarFetchKey::Campaign {
                    slug: "stonea".into(),
                    slot,
                })
        );
        assert_eq!(loader.portrait_last_drawn[slot], loader.drawn_frame);

        loader.request_drawn_portraits(&[]);
        assert!(loader.portrait_last_drawn[slot] < loader.drawn_frame);

        loader.request_drawn_portraits(&["boudica".into()]);
        assert!(
            loader
                .avatars_fetch_pending
                .contains(&AvatarFetchKey::Leader(Leader::of("boudica")))
        );
    }
}
