use sow_core::player::Leader;
use std::collections::{HashMap, HashSet};
use web_time::{Duration, Instant};

pub const MAX_AVATAR_FETCHES_IN_FLIGHT: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AvatarFetchKey {
    Fallback,
    Leader(Leader),
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
    pub avatar_fallback: Option<Vec<u8>>,
    pub avatars_fetch_pending: Vec<AvatarFetchKey>,
    pub avatars_in_flight: HashSet<AvatarFetchKey>,
    avatars_fetch_all_queued: bool,
    avatar_retry_state: HashMap<AvatarFetchKey, Retry>,
    pub gpu_avatar_cells: Vec<(AvatarFetchKey, Vec<u8>)>,
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
            avatar_fallback: None,
            avatars_fetch_pending: Vec::new(),
            avatars_in_flight: HashSet::new(),
            avatars_fetch_all_queued: false,
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

    fn leader_slug(leader: Leader) -> String {
        leader.name().to_lowercase().replace(' ', "_")
    }

    pub fn avatar_filename(key: AvatarFetchKey) -> String {
        match key {
            AvatarFetchKey::Fallback => "null.webp".into(),
            AvatarFetchKey::Leader(leader) => format!("{}.webp", Self::leader_slug(leader)),
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

    pub fn request_avatars_fetch_all(&mut self) {
        if self.avatars_fetch_all_queued {
            return;
        }
        self.avatars_fetch_all_queued = true;
        self.queue_avatar_fetch(AvatarFetchKey::Fallback, true);
        for &leader in &Leader::ALL {
            self.queue_avatar_fetch(AvatarFetchKey::Leader(leader), false);
        }
    }

    pub fn ensure_avatars_loaded(&mut self) {
        self.request_avatars_fetch_all();
    }

    fn avatar_loaded(&self, key: AvatarFetchKey) -> bool {
        match key {
            AvatarFetchKey::Fallback => self.avatar_fallback.is_some(),
            AvatarFetchKey::Leader(leader) => self.avatars.contains_key(&leader),
        }
    }

    fn retry_ready(&self, key: AvatarFetchKey) -> bool {
        self.avatar_retry_state
            .get(&key)
            .map(|retry| !retry.permanent && Instant::now() >= retry.next_retry_at)
            .unwrap_or(true)
    }

    fn queue_avatar_fetch(&mut self, key: AvatarFetchKey, front: bool) {
        if self.avatar_loaded(key)
            || self.avatars_in_flight.contains(&key)
            || !self.retry_ready(key)
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
        priority: AvatarFetchKey,
    ) -> Option<AvatarFetchKey> {
        if self.avatars_in_flight.len() >= MAX_AVATAR_FETCHES_IN_FLIGHT {
            return None;
        }
        let index = self
            .avatars_fetch_pending
            .iter()
            .position(|key| *key == priority)
            .or((!self.avatars_fetch_pending.is_empty()).then_some(0))?;
        let key = self.avatars_fetch_pending.remove(index);
        if self.avatar_loaded(key) {
            return self.take_next_avatar_fetch_pending(priority);
        }
        self.avatars_in_flight.insert(key);
        Some(key)
    }

    pub fn note_avatar_fetch_failed(&mut self, key: AvatarFetchKey, reason: impl Into<String>) {
        self.avatars_in_flight.remove(&key);
        let reason = reason.into();
        let permanent = reason.contains("404");
        let attempts = self
            .avatar_retry_state
            .get(&key)
            .map(|retry| retry.attempts + 1)
            .unwrap_or(1);
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
        let cell = image::imageops::resize(
            &image,
            128,
            128,
            image::imageops::FilterType::Triangle,
        )
        .into_raw();
        self.gpu_avatar_cells.retain(|(loaded_key, _)| *loaded_key != key);
        self.gpu_avatar_cells.push((key, cell));
        match key {
            AvatarFetchKey::Fallback => self.avatar_fallback = Some(bytes.to_vec()),
            AvatarFetchKey::Leader(leader) => {
                self.avatars.insert(leader, bytes.to_vec());
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
