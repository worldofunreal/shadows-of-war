use sow_core::player::Leader;
use std::collections::{HashMap, HashSet};
use web_time::{Duration, Instant};

pub const MAX_LEADER_FETCHES_IN_FLIGHT: usize = 1;
pub const MAX_BOOT_UI_FETCHES_IN_FLIGHT: usize = 4;
pub const MAX_AVATAR_FETCHES_IN_FLIGHT: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AvatarFetchKey { Fallback, Leader(Leader) }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LeaderPortraitKey { pub leader: Leader, pub mobile: bool }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiSplashTexture { LoaderEmpty, LoaderFull, SplashDesktop, SplashMobile }
impl UiSplashTexture {
    pub const ALL: [Self; 4] = [Self::LoaderEmpty, Self::LoaderFull, Self::SplashDesktop, Self::SplashMobile];
    pub fn filename(self) -> &'static str { match self { Self::LoaderEmpty => "loader_empty.webp", Self::LoaderFull => "loader_full.webp", Self::SplashDesktop => "sow-splash-desktop.webp", Self::SplashMobile => "sow-splash-mobile.webp" } }
}
#[derive(Clone, Debug)] struct Retry { attempts: u32, next_retry_at: Instant, permanent: bool }

pub struct AssetLoader {
    pub map_catalog: Option<Vec<sow_core::maps::MapCatalogEntry>>, pub catalog_in_flight: bool,
    pub maps: HashMap<String, Vec<u8>>, pub maps_in_flight: HashSet<String>,
    pub thumbnails: HashMap<String, Vec<u8>>, pub thumbnails_in_flight: HashSet<String>,
    pub thumbnail_errors: HashMap<String, String>, pub thumbnails_fetch_pending: Vec<String>,
    pub avatars: HashMap<Leader, Vec<u8>>, pub avatar_fallback: Option<Vec<u8>>,
    pub ui_loader_empty: Option<Vec<u8>>, pub ui_loader_full: Option<Vec<u8>>,
    pub leader_desktop_images: HashMap<Leader, Vec<u8>>, pub leader_mobile_images: HashMap<Leader, Vec<u8>>,
    pub splash_desktop: Option<Vec<u8>>, pub splash_mobile: Option<Vec<u8>>,
    pub leaders_fetch_pending: Vec<LeaderPortraitKey>, pub leaders_in_flight: HashSet<LeaderPortraitKey>,
    pub boot_ui_fetch_pending: Vec<UiSplashTexture>, pub boot_ui_in_flight: HashSet<UiSplashTexture>,
    pub avatars_fetch_pending: Vec<AvatarFetchKey>, pub avatars_in_flight: HashSet<AvatarFetchKey>,
    avatars_fetch_all_queued: bool, avatar_retry_state: HashMap<AvatarFetchKey, Retry>,
    pub gpu_avatar_cells: Vec<(AvatarFetchKey, Vec<u8>)>, pub portal_avatar: Option<Vec<u8>>,
    pub portal_avatar_request: Option<String>, pub portal_avatar_in_flight: bool,
}
impl Default for AssetLoader { fn default() -> Self { Self::new() } }
impl AssetLoader {
    pub fn new() -> Self { Self { map_catalog: None, catalog_in_flight: false, maps: HashMap::new(), maps_in_flight: HashSet::new(), thumbnails: HashMap::new(), thumbnails_in_flight: HashSet::new(), thumbnail_errors: HashMap::new(), thumbnails_fetch_pending: Vec::new(), avatars: HashMap::new(), avatar_fallback: None, ui_loader_empty: None, ui_loader_full: None, leader_desktop_images: HashMap::new(), leader_mobile_images: HashMap::new(), splash_desktop: None, splash_mobile: None, leaders_fetch_pending: Vec::new(), leaders_in_flight: HashSet::new(), boot_ui_fetch_pending: Vec::new(), boot_ui_in_flight: HashSet::new(), avatars_fetch_pending: Vec::new(), avatars_in_flight: HashSet::new(), avatars_fetch_all_queued: false, avatar_retry_state: HashMap::new(), gpu_avatar_cells: Vec::new(), portal_avatar: None, portal_avatar_request: None, portal_avatar_in_flight: false } }
    pub fn map_key(name: &str) -> String { sow_core::maps::map_key(name) }
    pub fn leader_slug(leader: Leader) -> String { leader.name().to_lowercase().replace(' ', "_") }
    pub fn avatar_filename(key: AvatarFetchKey) -> String { match key { AvatarFetchKey::Fallback => "null.webp".into(), AvatarFetchKey::Leader(l) => format!("{}.webp", Self::leader_slug(l)) } }
    pub fn leader_portrait_filename(key: LeaderPortraitKey) -> String { format!("{}{}.webp", Self::leader_slug(key.leader), if key.mobile { "_mobile" } else { "" }) }
    pub fn has_map(&self, name: &str) -> bool { let key = Self::map_key(name); self.maps.contains_key(&key) || sow_core::maps::map_payload_available(&key) }
    pub fn take_map(&mut self, name: &str) -> Option<Vec<u8>> { let key = Self::map_key(name); sow_core::maps::load_map_br_payload(&key, self.maps.remove(&key)) }
    pub fn thumbnail(&self, name: &str) -> Option<&Vec<u8>> { self.thumbnails.get(&Self::map_key(name)) }
    pub fn ingest_thumbnail(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> { image::load_from_memory(bytes).map_err(|e| format!("decode thumbnail: {e}"))?; let key = Self::map_key(name); self.thumbnails.insert(key.clone(), bytes.to_vec()); self.thumbnails_in_flight.remove(&key); self.thumbnail_errors.remove(&key); Ok(()) }
    pub fn note_thumbnail_failure(&mut self, name: &str, reason: impl Into<String>) { let key = Self::map_key(name); self.thumbnails_in_flight.remove(&key); self.thumbnail_errors.insert(key, reason.into()); }
    pub fn request_thumbnail(&mut self, name: &str) { let key = Self::map_key(name); if self.thumbnails.contains_key(&key) || self.thumbnails_in_flight.contains(&key) || self.thumbnails_fetch_pending.contains(&key) { return; } self.thumbnails_in_flight.insert(key.clone()); self.thumbnails_fetch_pending.push(key); }
    pub fn drain_thumbnail_fetch_pending(&mut self) -> Vec<String> { std::mem::take(&mut self.thumbnails_fetch_pending) }
    pub fn prefetch_matchmaking_thumbnails(&mut self) {}
    pub fn request_boot_ui_fetch(&mut self, kind: UiSplashTexture) { if !self.boot_ui_in_flight.contains(&kind) && !self.boot_ui_fetch_pending.contains(&kind) { self.boot_ui_fetch_pending.push(kind); } }
    pub fn take_next_boot_ui_fetch_pending(&mut self) -> Option<UiSplashTexture> { if self.boot_ui_in_flight.len() >= MAX_BOOT_UI_FETCHES_IN_FLIGHT { return None; } let kind = self.boot_ui_fetch_pending.pop()?; self.boot_ui_in_flight.insert(kind); Some(kind) }
    pub fn note_boot_ui_fetch_failed(&mut self, kind: UiSplashTexture) { self.boot_ui_in_flight.remove(&kind); }
    pub fn ingest_boot_ui_webp_bytes(&mut self, kind: UiSplashTexture, bytes: &[u8]) -> Result<(), String> { image::load_from_memory(bytes).map_err(|e| format!("decode boot asset: {e}"))?; self.boot_ui_in_flight.remove(&kind); match kind { UiSplashTexture::LoaderEmpty => self.ui_loader_empty = Some(bytes.to_vec()), UiSplashTexture::LoaderFull => self.ui_loader_full = Some(bytes.to_vec()), UiSplashTexture::SplashDesktop => self.splash_desktop = Some(bytes.to_vec()), UiSplashTexture::SplashMobile => self.splash_mobile = Some(bytes.to_vec()) }; Ok(()) }
    pub fn ui_splash_ready(&self) -> bool { self.ui_loader_empty.is_some() && self.ui_loader_full.is_some() && self.splash_desktop.is_some() && self.splash_mobile.is_some() }
    pub fn ensure_ui_assets_loaded(&mut self) { for kind in UiSplashTexture::ALL { self.request_boot_ui_fetch(kind); } }
    pub fn ensure_boot_leader_loaded(&mut self, _leader: Leader) {}
    pub fn boot_leader_ready(&self, _leader: Leader, _mobile: bool) -> bool { true }
    pub fn request_avatars_fetch_all(&mut self) { if self.avatars_fetch_all_queued { return; } self.avatars_fetch_all_queued = true; self.queue_avatar_fetch(AvatarFetchKey::Fallback, true); for &leader in &Leader::ALL { self.queue_avatar_fetch(AvatarFetchKey::Leader(leader), false); } }
    pub fn ensure_avatars_loaded(&mut self) { self.request_avatars_fetch_all(); }
    pub fn request_avatar_priority(&mut self, leader: Leader) { self.queue_avatar_fetch(AvatarFetchKey::Leader(leader), true); }
    fn avatar_loaded(&self, key: AvatarFetchKey) -> bool { match key { AvatarFetchKey::Fallback => self.avatar_fallback.is_some(), AvatarFetchKey::Leader(l) => self.avatars.contains_key(&l) } }
    fn retry_ready(&self, key: AvatarFetchKey) -> bool { self.avatar_retry_state.get(&key).map(|r| !r.permanent && Instant::now() >= r.next_retry_at).unwrap_or(true) }
    fn queue_avatar_fetch(&mut self, key: AvatarFetchKey, front: bool) { if self.avatar_loaded(key) || self.avatars_in_flight.contains(&key) || !self.retry_ready(key) || self.avatars_fetch_pending.contains(&key) { return; } if front { self.avatars_fetch_pending.insert(0, key); } else { self.avatars_fetch_pending.push(key); } }
    pub fn take_next_avatar_fetch_pending(&mut self, priority: AvatarFetchKey) -> Option<AvatarFetchKey> { if self.avatars_in_flight.len() >= MAX_AVATAR_FETCHES_IN_FLIGHT { return None; } let idx = self.avatars_fetch_pending.iter().position(|k| *k == priority).or((!self.avatars_fetch_pending.is_empty()).then_some(0))?; let key = self.avatars_fetch_pending.remove(idx); if self.avatar_loaded(key) { return self.take_next_avatar_fetch_pending(priority); } self.avatars_in_flight.insert(key); Some(key) }
    pub fn note_avatar_fetch_failed(&mut self, key: AvatarFetchKey, reason: impl Into<String>) { self.avatars_in_flight.remove(&key); let reason = reason.into(); let permanent = reason.contains("404"); let attempts = self.avatar_retry_state.get(&key).map(|r| r.attempts + 1).unwrap_or(1); self.avatar_retry_state.insert(key, Retry { attempts, next_retry_at: Instant::now() + Duration::from_millis(500 * (1 << attempts.saturating_sub(1).min(4))), permanent }); }
    pub fn ingest_avatar_webp_bytes(&mut self, key: AvatarFetchKey, bytes: &[u8]) -> Result<(), String> { self.avatars_in_flight.remove(&key); self.avatar_retry_state.remove(&key); let image = image::load_from_memory(bytes).map_err(|e| format!("decode avatar: {e}"))?.to_rgba8(); let cell = image::imageops::resize(&image, 128, 128, image::imageops::FilterType::Triangle).into_raw(); self.gpu_avatar_cells.retain(|(k, _)| *k != key); self.gpu_avatar_cells.push((key, cell)); match key { AvatarFetchKey::Fallback => self.avatar_fallback = Some(bytes.to_vec()), AvatarFetchKey::Leader(l) => { self.avatars.insert(l, bytes.to_vec()); } } Ok(()) }
    pub fn queue_portal_avatar(&mut self, url: String) { if self.portal_avatar.is_none() && !self.portal_avatar_in_flight { self.portal_avatar_request = Some(url); } }
    pub fn note_portal_avatar_failed(&mut self, _reason: impl Into<String>) { self.portal_avatar_in_flight = false; self.portal_avatar_request = None; }
    pub fn ingest_portal_avatar_bytes(&mut self, bytes: &[u8]) -> Result<(), String> { image::load_from_memory(bytes).map_err(|e| format!("decode avatar: {e}"))?; self.portal_avatar = Some(bytes.to_vec()); self.portal_avatar_in_flight = false; self.portal_avatar_request = None; Ok(()) }
    pub fn note_leader_portrait_fetch_failed(&mut self, key: LeaderPortraitKey, _reason: impl Into<String>) { self.leaders_in_flight.remove(&key); }
    pub fn enqueue_leader_portrait_bytes(&mut self, leader: Leader, mobile: bool, bytes: Vec<u8>) { self.leaders_in_flight.remove(&LeaderPortraitKey { leader, mobile }); if mobile { self.leader_mobile_images.insert(leader, bytes); } else { self.leader_desktop_images.insert(leader, bytes); } }
    pub fn process_leader_decode_budget(&mut self, _limit: usize, _focus: LeaderPortraitKey) {}
    pub fn get_assets_to_fetch(&mut self, lobbies: &[sow_core::protocol::LobbyInfo]) -> Vec<String> { lobbies.iter().filter_map(|l| { let key = Self::map_key(&l.map_name); (!self.has_map(&key) && self.maps_in_flight.insert(key.clone())).then_some(key) }).collect() }
    pub fn flush_except(&mut self, keep: &HashSet<String>) { self.maps.retain(|k, _| keep.contains(k)); }
}
