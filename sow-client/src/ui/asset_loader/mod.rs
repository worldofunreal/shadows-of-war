//! Shared radial/nameplate portrait loading and atlas slot management.
use sow_core::emote::AvatarExpression;
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
    LeaderExpression(Leader, AvatarExpression),
    Campaign { slug: String, slot: usize },
    CatalogPrefetch(String),
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
    leader_portraits: HashSet<Leader>,
    leader_expressions: HashSet<(Leader, AvatarExpression)>,
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
    avatar_prefetch_wants: HashMap<String, AvatarFetchKey>,
    avatar_catalog_prefetch_started: bool,
    tutorial_avatar_prefetch_started: bool,
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
            leader_portraits: HashSet::new(),
            leader_expressions: HashSet::new(),
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
            avatar_catalog_prefetch_started: false,
            tutorial_avatar_prefetch_started: false,
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
            AvatarFetchKey::Leader(leader) => format!("{}.webp", leader.art_slug()),
            AvatarFetchKey::LeaderExpression(leader, expression) => format!(
                "expressions/{}_{}.webp",
                leader.art_slug(),
                avatar_expression_name(*expression)
            ),
            AvatarFetchKey::Campaign { slug, .. } => format!("{slug}.webp"),
            AvatarFetchKey::CatalogPrefetch(path) => path.clone(),
        }
    }

    /// The radial and map both use the generated Calm portrait.
    pub fn leader_fetch_key(&self, leader: Leader) -> AvatarFetchKey {
        AvatarFetchKey::Leader(leader)
    }

    pub fn queue_leader_avatar(&mut self, leader: Leader, priority: bool) {
        self.queue_avatar_fetch(AvatarFetchKey::Fallback, true);
        self.queue_avatar_fetch(self.leader_fetch_key(leader), priority);
    }

    pub fn queue_leader_expressions(&mut self, leader: Leader, priority: bool) {
        self.queue_avatar_fetch(AvatarFetchKey::Fallback, true);
        self.queue_avatar_fetch(self.leader_fetch_key(leader), priority);
        for expression in [
            AvatarExpression::Happy,
            AvatarExpression::Fear,
            AvatarExpression::Angry,
        ] {
            self.queue_avatar_fetch(AvatarFetchKey::LeaderExpression(leader, expression), false);
        }
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
        self.avatar_prefetch_wants.retain(|_, key| match key {
            AvatarFetchKey::Campaign { slug, .. } => wanted.contains(slug),
            _ => true,
        });
        for slug in stale {
            let path = format!("{slug}.webp");
            if self.avatar_prefetch_catalog.contains(&path) {
                self.enqueue_avatar_prefetch(&path);
            }
        }
        for slug in slugs {
            self.queue_campaign_avatar(slug);
        }
    }

    pub fn start_avatar_catalog_prefetch(&mut self, catalog: &[String]) {
        if self.avatar_catalog_prefetch_started {
            return;
        }
        self.avatar_catalog_prefetch_started = true;
        for path in catalog {
            if !valid_avatar_asset_path(path) {
                log::warn!("Ignoring invalid avatar asset catalog entry: {path:?}");
                continue;
            }
            if self.avatar_prefetch_catalog.insert(path.clone()) {
                self.enqueue_avatar_prefetch(path);
            }
        }
    }

    pub fn avatar_catalog_prefetch_started(&self) -> bool {
        self.avatar_catalog_prefetch_started
    }

    pub fn start_tutorial_bot_avatar_prefetch(&mut self) {
        if self.tutorial_avatar_prefetch_started {
            return;
        }
        self.tutorial_avatar_prefetch_started = true;
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
                let path = format!("{slug}.webp");
                if self.avatar_prefetch_catalog.insert(path.clone()) {
                    self.enqueue_avatar_prefetch(&path);
                }
            }
        }
    }

    pub fn pump_avatar_prefetch_retries(&mut self) {
        let retry: Vec<_> = self.avatar_prefetch_retry.iter().cloned().collect();
        for path in retry {
            let key = AvatarFetchKey::CatalogPrefetch(path.clone());
            if self
                .avatar_retry_state
                .get(&key)
                .is_some_and(|state| state.permanent)
            {
                self.avatar_prefetch_retry.remove(&path);
            } else if self.retry_ready(&key) {
                self.avatar_prefetch_retry.remove(&path);
                self.enqueue_avatar_prefetch(&path);
            }
        }
    }

    fn avatar_demand_pending_or_in_flight(&self, path: &str) -> bool {
        self.avatars_fetch_pending
            .iter()
            .chain(self.avatars_in_flight.iter())
            .any(|key| {
                !matches!(key, AvatarFetchKey::CatalogPrefetch(_))
                    && Self::avatar_filename(key) == path
            })
    }

    fn enqueue_avatar_prefetch(&mut self, path: &str) {
        let key = AvatarFetchKey::CatalogPrefetch(path.to_owned());
        let campaign_slug = campaign_slug_from_asset_path(path);
        if self.avatar_prefetched.contains(path)
            || campaign_slug.is_some_and(|slug| self.campaign_avatars.contains(slug))
            || self.avatars_in_flight.contains(&key)
            || self.avatar_prefetch_queued.contains(path)
            || self.avatar_demand_pending_or_in_flight(path)
            || !self.retry_ready(&key)
        {
            return;
        }
        self.avatar_prefetch_queued.insert(path.to_owned());
        self.avatar_prefetch_pending.push_back(path.to_owned());
    }

    pub fn take_next_avatar_prefetch_pending(&mut self) -> Option<AvatarFetchKey> {
        if self.avatar_prefetch_in_flight() >= MAX_AVATAR_PREFETCHES_IN_FLIGHT {
            return None;
        }
        let pending = self.avatar_prefetch_pending.len();
        for _ in 0..pending {
            let path = self.avatar_prefetch_pending.pop_front()?;
            self.avatar_prefetch_queued.remove(&path);
            let key = AvatarFetchKey::CatalogPrefetch(path.clone());
            let campaign_slug = campaign_slug_from_asset_path(&path);
            if self.avatar_prefetched.contains(&path)
                || campaign_slug.is_some_and(|slug| self.campaign_avatars.contains(slug))
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
                    self.avatar_prefetch_queued.insert(path.clone());
                    self.avatar_prefetch_pending.push_back(path);
                }
                continue;
            }
            if self.avatar_demand_pending_or_in_flight(&path) {
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
            .filter(|key| matches!(key, AvatarFetchKey::CatalogPrefetch(_)))
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
            let asset_path = format!("{evicted}.webp");
            if self.avatar_prefetch_catalog.contains(&asset_path) {
                self.enqueue_avatar_prefetch(&asset_path);
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
            AvatarFetchKey::Leader(leader) => self.leader_portraits.contains(leader),
            AvatarFetchKey::LeaderExpression(leader, expression) => {
                self.leader_expressions.contains(&(*leader, *expression))
            }
            AvatarFetchKey::Campaign { slug, .. } => self.campaign_avatars.contains(slug),
            AvatarFetchKey::CatalogPrefetch(path) => {
                self.avatar_prefetched.contains(path)
                    || campaign_slug_from_asset_path(path)
                        .is_some_and(|slug| self.campaign_avatars.contains(slug))
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
        if !matches!(&key, AvatarFetchKey::CatalogPrefetch(_)) {
            let path = Self::avatar_filename(&key);
            let prefetch = AvatarFetchKey::CatalogPrefetch(path.clone());
            if self.avatars_in_flight.contains(&prefetch) {
                self.avatar_prefetch_wants.insert(path, key);
                return;
            }
            if self.avatar_prefetch_queued.remove(&path) {
                self.avatar_prefetch_pending
                    .retain(|queued| queued != &path);
            }
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
            || reason.contains("invalid avatar pixels");
        let attempts = self
            .avatar_retry_state
            .get(&key)
            .map(|retry| retry.attempts + 1)
            .unwrap_or(1);
        let permanent = permanent_error || attempts >= 3;
        let retry = Retry {
            attempts,
            next_retry_at: Instant::now()
                + Duration::from_millis(500 * (1 << attempts.saturating_sub(1).min(4))),
            permanent,
        };
        self.avatar_retry_state.insert(key.clone(), retry.clone());
        match &key {
            AvatarFetchKey::CatalogPrefetch(path) => {
                if let Some(wanted) = self.avatar_prefetch_wants.remove(path) {
                    self.queue_avatar_fetch(wanted, true);
                } else if !permanent {
                    self.avatar_prefetch_retry.insert(path.clone());
                }
            }
            AvatarFetchKey::Campaign { slug, .. } if !permanent => {
                let path = format!("{slug}.webp");
                if self.avatar_prefetch_catalog.contains(&path) {
                    self.avatar_retry_state
                        .insert(AvatarFetchKey::CatalogPrefetch(path.clone()), retry);
                    self.avatar_prefetch_retry.insert(path);
                }
            }
            _ => {}
        }
    }

    pub fn ingest_avatar_prefetch(&mut self, key: AvatarFetchKey) -> Result<(), String> {
        let AvatarFetchKey::CatalogPrefetch(path) = key else {
            return Err("avatar prefetch returned a demand key".into());
        };
        let prefetch_key = AvatarFetchKey::CatalogPrefetch(path.clone());
        self.avatars_in_flight.remove(&prefetch_key);
        self.avatar_retry_state.remove(&prefetch_key);
        self.avatar_prefetched.insert(path.clone());
        let Some(wanted) = self.avatar_prefetch_wants.remove(&path) else {
            return Ok(());
        };
        if let AvatarFetchKey::Campaign { slug, slot } = &wanted
            && (self.campaign_avatar_slots.get(slug) != Some(slot)
                || !self.drawn_avatars.contains(slug))
        {
            return Ok(());
        }
        // The pixels are extracted only for a visible atlas slot. This request reuses the same
        // decoded browser image that the catalog prefetch warmed.
        self.queue_avatar_fetch(wanted, true);
        Ok(())
    }

    pub fn ingest_avatar_pixels(
        &mut self,
        key: AvatarFetchKey,
        pixels: &[u8],
    ) -> Result<(), String> {
        self.avatars_in_flight.remove(&key);
        self.avatar_retry_state.remove(&key);
        if let AvatarFetchKey::Campaign { slug, slot } = &key
            && (self.campaign_avatar_slots.get(slug) != Some(slot)
                || !self.drawn_avatars.contains(slug))
        {
            // The shared browser loader keeps the decoded portrait available.
            self.avatar_prefetched.insert(format!("{slug}.webp"));
            return Ok(());
        }
        let expected_len = (sow_render::text::AVATAR_CELL as usize).pow(2) * 4;
        if pixels.len() != expected_len {
            return Err(format!(
                "invalid avatar pixels: got {}, expected {expected_len}",
                pixels.len()
            ));
        }
        let path = Self::avatar_filename(&key);
        self.store_avatar_cells(key.clone(), pixels)?;
        self.avatar_prefetched.insert(path);
        match key {
            AvatarFetchKey::Fallback => self.avatar_fallback_loaded = true,
            AvatarFetchKey::Leader(leader) => {
                self.leader_portraits.insert(leader);
            }
            AvatarFetchKey::LeaderExpression(leader, expression) => {
                self.leader_expressions.insert((leader, expression));
            }
            AvatarFetchKey::Campaign { slug, .. } => {
                self.campaign_avatars.insert(slug.clone());
                self.avatar_prefetched.insert(format!("{slug}.webp"));
            }
            AvatarFetchKey::CatalogPrefetch(_) => unreachable!("prefetch handled above"),
        }
        Ok(())
    }

    fn store_avatar_cells(&mut self, key: AvatarFetchKey, pixels: &[u8]) -> Result<(), String> {
        let slot = match key {
            AvatarFetchKey::Leader(leader) => {
                sow_render::text::leader_avatar_slot(leader, AvatarExpression::Calm)
            }
            AvatarFetchKey::LeaderExpression(leader, expression) => {
                sow_render::text::leader_avatar_slot(leader, expression)
            }
            AvatarFetchKey::Fallback => sow_render::text::AVATAR_FALLBACK_SLOT,
            AvatarFetchKey::Campaign { slot, .. } => slot,
            AvatarFetchKey::CatalogPrefetch(_) => {
                return Err("cache-only avatar prefetch cannot fill an atlas slot".into());
            }
        };
        self.push_gpu_cell(slot, pixels.to_vec());
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

pub fn avatar_expression_name(expression: AvatarExpression) -> &'static str {
    match expression {
        AvatarExpression::Happy => "happy",
        AvatarExpression::Calm => "calm",
        AvatarExpression::Fear => "fear",
        AvatarExpression::Angry => "angry",
    }
}

fn valid_avatar_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn valid_avatar_asset_path(value: &str) -> bool {
    value.ends_with(".webp")
        && !value.starts_with('/')
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn campaign_slug_from_asset_path(path: &str) -> Option<&str> {
    let slug = path.strip_suffix(".webp")?;
    (!slug.contains('/')).then_some(slug)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[test]
    fn radial_portrait_paths_fill_each_leader_expression_slot() {
        let size = (sow_render::text::AVATAR_CELL as usize).pow(2) * 4;
        for leader in Leader::all() {
            let mut loader = AssetLoader::new();
            for expression in AvatarExpression::ALL {
                let key = match expression {
                    AvatarExpression::Calm => AvatarFetchKey::Leader(leader),
                    other => AvatarFetchKey::LeaderExpression(leader, other),
                };
                let expected_path = match expression {
                    AvatarExpression::Calm => format!("{}.webp", leader.art_slug()),
                    other => format!(
                        "expressions/{}_{}.webp",
                        leader.art_slug(),
                        avatar_expression_name(other)
                    ),
                };
                assert_eq!(AssetLoader::avatar_filename(&key), expected_path);
                loader.store_avatar_cells(key, &vec![255; size]).unwrap();
            }
            let cells = loader
                .gpu_avatar_cells
                .iter()
                .filter(|entry| entry.0 != sow_render::text::AVATAR_GOLD_SLOT)
                .collect::<Vec<_>>();
            assert_eq!(cells.len(), 4, "{}", sow_data::commerce::leader_id(leader));
            for expression in AvatarExpression::ALL {
                let slot = sow_render::text::leader_avatar_slot(leader, expression);
                assert!(
                    cells
                        .iter()
                        .any(|(found, pixels)| { *found == slot && pixels.len() == size })
                );
            }
        }
    }

    #[test]
    fn calm_portrait_fills_only_the_calm_slot() {
        let leader = Leader::of("boudica");
        let mut loader = AssetLoader::new();
        let size = (sow_render::text::AVATAR_CELL as usize).pow(2) * 4;
        loader
            .store_avatar_cells(AvatarFetchKey::Leader(leader), &vec![12; size])
            .unwrap();
        let cells = loader
            .gpu_avatar_cells
            .iter()
            .filter(|entry| entry.0 != sow_render::text::AVATAR_GOLD_SLOT)
            .collect::<Vec<_>>();
        assert_eq!(cells.len(), 1);
        assert_eq!(
            cells[0].0,
            sow_render::text::leader_avatar_slot(leader, sow_core::emote::AvatarExpression::Calm)
        );
    }

    #[wasm_bindgen_test]
    fn catalog_prefetch_is_cache_only_and_limited_to_four_requests() {
        let mut loader = AssetLoader::new();
        let catalog = (0..5)
            .map(|index| format!("bot_{index}.webp"))
            .collect::<Vec<_>>();
        loader.start_avatar_catalog_prefetch(&catalog);
        loader.start_avatar_catalog_prefetch(&["duplicate.webp".into()]);
        assert_eq!(loader.avatar_prefetch_pending.len(), 5);

        let mut in_flight = Vec::new();
        for _ in 0..MAX_AVATAR_PREFETCHES_IN_FLIGHT {
            in_flight.push(loader.take_next_avatar_prefetch_pending().unwrap());
        }
        assert_eq!(loader.avatar_prefetch_in_flight(), 4);
        assert!(loader.take_next_avatar_prefetch_pending().is_none());

        let gpu_cells_before = loader.gpu_avatar_cells.len();
        loader.ingest_avatar_prefetch(in_flight.remove(0)).unwrap();
        assert_eq!(loader.gpu_avatar_cells.len(), gpu_cells_before);
        assert!(matches!(
            loader.take_next_avatar_prefetch_pending(),
            Some(AvatarFetchKey::CatalogPrefetch(_))
        ));
        assert_eq!(loader.avatar_prefetch_in_flight(), 4);
    }

    #[wasm_bindgen_test]
    fn visible_portrait_promotes_queued_catalog_asset_to_demand() {
        let mut loader = AssetLoader::new();
        loader.start_avatar_catalog_prefetch(&["stonea.webp".into()]);
        loader.request_drawn_portraits(&["stonea".into()]);

        assert!(matches!(
            loader.take_next_avatar_fetch_pending(&AvatarFetchKey::Fallback),
            Some(AvatarFetchKey::Campaign { slug, .. }) if slug == "stonea"
        ));
        assert!(loader.take_next_avatar_prefetch_pending().is_none());
    }

    #[wasm_bindgen_test]
    fn visible_portrait_reuses_prefetched_image_for_atlas_upload() {
        let mut loader = AssetLoader::new();
        loader.start_avatar_catalog_prefetch(&["stonea.webp".into()]);
        let prefetch = loader.take_next_avatar_prefetch_pending().unwrap();
        loader.request_drawn_portraits(&["stonea".into()]);

        loader.ingest_avatar_prefetch(prefetch).unwrap();
        assert!(!loader.campaign_avatars.contains("stonea"));
        assert_eq!(loader.avatar_prefetch_in_flight(), 0);

        let demand = loader
            .take_next_avatar_fetch_pending(&AvatarFetchKey::Fallback)
            .unwrap();
        assert!(matches!(demand, AvatarFetchKey::Campaign { .. }));
        loader
            .ingest_avatar_pixels(
                demand,
                &vec![12; (sow_render::text::AVATAR_CELL as usize).pow(2) * 4],
            )
            .unwrap();

        assert!(loader.campaign_avatars.contains("stonea"));
        assert_eq!(loader.avatar_prefetch_in_flight(), 0);
        assert!(loader.avatars_in_flight.is_empty());
    }

    #[wasm_bindgen_test]
    fn leader_nameplate_requests_radial_portraits_for_each_expression() {
        let leader = Leader::of("boudica");
        let mut loader = AssetLoader::new();
        loader.queue_leader_avatar(leader, true);
        assert!(
            loader
                .avatars_fetch_pending
                .contains(&AvatarFetchKey::Leader(leader))
        );
        loader.queue_leader_expressions(leader, true);
        for expression in [
            AvatarExpression::Happy,
            AvatarExpression::Fear,
            AvatarExpression::Angry,
        ] {
            assert!(
                loader
                    .avatars_fetch_pending
                    .contains(&AvatarFetchKey::LeaderExpression(leader, expression))
            );
        }
        assert_eq!(
            AssetLoader::avatar_filename(&AvatarFetchKey::Leader(leader)),
            "boudica.webp"
        );
    }

    #[test]
    fn hero_campaign_identity_uses_the_shared_calm_slot_and_loader() {
        let mut loader = AssetLoader::new();
        loader.queue_campaign_avatar("boudica");
        let leader = Leader::of("boudica");
        assert_eq!(
            loader.campaign_avatar_slot("boudica"),
            Some(sow_render::text::leader_avatar_slot(
                leader,
                AvatarExpression::Calm
            ))
        );
        assert!(
            loader
                .avatars_fetch_pending
                .contains(&AvatarFetchKey::Leader(leader))
        );
        let size = (sow_render::text::AVATAR_CELL as usize).pow(2) * 4;
        loader
            .ingest_avatar_pixels(AvatarFetchKey::Leader(leader), &vec![20; size])
            .unwrap();
        assert!(loader.leader_portraits.contains(&leader));
        assert_eq!(
            loader.campaign_avatar_slot("boudica"),
            Some(sow_render::text::leader_avatar_slot(
                leader,
                AvatarExpression::Calm
            ))
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
