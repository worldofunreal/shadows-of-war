//! Nameplate portraits: which image each avatar atlas slot holds and how it gets there.
//! A portrait gets a slot when its player appears in the match and keeps it until the match ends,
//! so nothing a frame draws or skips can take it away. The browser loader the radial uses decodes
//! each file, and the atlas copies it straight to the GPU. See docs/nameplates-and-emotes.md.
use sow_core::emote::AvatarExpression;
use sow_core::player::{AvatarIdentityRef, Leader};
use sow_render::text::{AVATAR_FALLBACK_SLOT, AVATAR_MAX_SLOTS, TextRenderer};
use std::collections::HashMap;
use web_time::{Duration, Instant};

/// Browser image loads at once, so a big roster loads in the background without a decode spike.
const MAX_LOADS_IN_FLIGHT: usize = 6;
const FIRST_RETRY_DELAY: Duration = Duration::from_secs(2);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

/// The image an atlas slot holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AvatarPortrait {
    Fallback,
    Gold,
    Hero(Leader, AvatarExpression),
    /// A bot or campaign portrait, by catalog avatar id.
    Portrait(String),
}

impl AvatarPortrait {
    /// The published file, the one the radial shows.
    pub(crate) fn asset_path(&self) -> String {
        match self {
            Self::Fallback => "gameplay/avatars/null.webp".into(),
            Self::Gold => "gameplay/currency/gold.webp".into(),
            Self::Hero(leader, expression) => {
                sow_core::emote::hero_portrait_path(leader.art_slug(), *expression)
            }
            Self::Portrait(slug) => format!("gameplay/avatars/{slug}.webp"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Load {
    Queued,
    InFlight,
    Loaded,
    /// The browser could not load the file; try again after a delay.
    RetryAt(Instant),
}

struct Slot {
    portrait: AvatarPortrait,
    load: Load,
    failures: u32,
}

/// The portrait each avatar atlas slot holds for the current match. Slot `i` is `slots[i]`;
/// slots are only ever added until `end_match`.
pub struct AvatarSlots {
    slots: Vec<Slot>,
    heroes: HashMap<(Leader, AvatarExpression), usize>,
    portraits: HashMap<String, usize>,
    in_match: bool,
    roster_tick: Option<u64>,
    /// Bumped by `end_match`: images that arrive for an older roster are dropped.
    generation: u32,
    atlas_reset: bool,
    #[cfg(target_arch = "wasm32")]
    arrived: std::rc::Rc<std::cell::RefCell<Vec<Arrival>>>,
}

#[cfg(target_arch = "wasm32")]
struct Arrival {
    generation: u32,
    slot: usize,
    image: Result<web_sys::ImageBitmap, String>,
}

enum Owner<'a> {
    Hero(Leader),
    Portrait(&'a str),
}

impl Default for AvatarSlots {
    fn default() -> Self {
        Self::new()
    }
}

impl AvatarSlots {
    pub(crate) fn new() -> Self {
        let mut avatars = Self {
            slots: Vec::new(),
            heroes: HashMap::new(),
            portraits: HashMap::new(),
            in_match: false,
            roster_tick: None,
            generation: 0,
            atlas_reset: false,
            #[cfg(target_arch = "wasm32")]
            arrived: Default::default(),
        };
        avatars.add_fixed();
        avatars
    }

    fn add_fixed(&mut self) {
        // Pushed in slot order: `AVATAR_FALLBACK_SLOT`, then `AVATAR_GOLD_SLOT`.
        self.push(AvatarPortrait::Fallback);
        self.push(AvatarPortrait::Gold);
    }

    fn push(&mut self, portrait: AvatarPortrait) {
        let slot = self.slots.len();
        match &portrait {
            AvatarPortrait::Hero(leader, expression) => {
                self.heroes.insert((*leader, *expression), slot);
            }
            AvatarPortrait::Portrait(slug) => {
                self.portraits.insert(slug.clone(), slot);
            }
            AvatarPortrait::Fallback | AvatarPortrait::Gold => {}
        }
        self.slots.push(Slot {
            portrait,
            load: Load::Queued,
            failures: 0,
        });
    }

    fn has_room(&self) -> bool {
        let room = self.slots.len() < AVATAR_MAX_SLOTS;
        if !room {
            log::error!("Avatar atlas is full at {AVATAR_MAX_SLOTS} portraits; new ones show the fallback");
        }
        room
    }

    fn owner<'a>(&self, identity: AvatarIdentityRef<'a>) -> Option<Owner<'a>> {
        match identity {
            AvatarIdentityRef::Portrait {
                leader: Some(leader),
                ..
            } => Some(Owner::Hero(leader)),
            AvatarIdentityRef::Portrait { slug, leader: None } => {
                if self.portraits.contains_key(slug) {
                    return Some(Owner::Portrait(slug));
                }
                // A campaign bot can play as a hero and show all four faces.
                match Leader::all().find(|leader| leader.id() == slug) {
                    Some(leader) => Some(Owner::Hero(leader)),
                    None => valid_avatar_id(slug).then_some(Owner::Portrait(slug)),
                }
            }
            AvatarIdentityRef::Fallback => None,
        }
    }

    /// Give every face `identity` can show a slot. Repeat calls are free.
    pub(crate) fn add_identity(&mut self, identity: AvatarIdentityRef<'_>) {
        self.in_match = true;
        match self.owner(identity) {
            Some(Owner::Hero(leader)) => {
                // Calm first: it is the face a hero shows most and the one the others fall back to.
                for expression in [
                    AvatarExpression::Calm,
                    AvatarExpression::Happy,
                    AvatarExpression::Fear,
                    AvatarExpression::Angry,
                ] {
                    if !self.heroes.contains_key(&(leader, expression)) && self.has_room() {
                        self.push(AvatarPortrait::Hero(leader, expression));
                    }
                }
            }
            Some(Owner::Portrait(slug)) => {
                if !self.portraits.contains_key(slug) && self.has_room() {
                    self.push(AvatarPortrait::Portrait(slug.to_owned()));
                }
            }
            None => {}
        }
    }

    /// Give every player in the snapshot their slots, mine first. Players who leave keep theirs,
    /// so death and attack portraits still find them.
    pub(crate) fn add_players(
        &mut self,
        snapshot: &sow_core::protocol::SimSnapshot,
        me: Option<sow_core::player::PlayerId>,
    ) {
        if self.roster_tick == Some(snapshot.tick) {
            return;
        }
        self.roster_tick = Some(snapshot.tick);
        if let Some(mine) = me.and_then(|id| snapshot.players.iter().find(|player| player.id == id))
        {
            self.add_identity(sow_core::player::avatar_identity_ref(mine));
        }
        for player in &snapshot.players {
            self.add_identity(sow_core::player::avatar_identity_ref(player));
        }
    }

    /// Release every slot when the match ends; the atlas empties on the next frame.
    pub(crate) fn end_match(&mut self) {
        if !self.in_match {
            return;
        }
        self.slots.clear();
        self.heroes.clear();
        self.portraits.clear();
        self.in_match = false;
        self.roster_tick = None;
        self.generation = self.generation.wrapping_add(1);
        self.atlas_reset = true;
        self.add_fixed();
    }

    /// The slot to draw for `identity` showing `expression`, given which slots hold their image.
    /// A hero falls back to their Calm face, and every portrait to the shared fallback.
    fn slot_for(
        &self,
        identity: AvatarIdentityRef<'_>,
        expression: AvatarExpression,
        filled: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        let filled_slot = |slot: Option<&usize>| slot.copied().filter(|slot| filled(*slot));
        match self.owner(identity) {
            Some(Owner::Hero(leader)) => filled_slot(self.heroes.get(&(leader, expression)))
                .or_else(|| filled_slot(self.heroes.get(&(leader, AvatarExpression::Calm)))),
            Some(Owner::Portrait(slug)) => filled_slot(self.portraits.get(slug)),
            None => None,
        }
        .or_else(|| filled(AVATAR_FALLBACK_SLOT).then_some(AVATAR_FALLBACK_SLOT))
    }

    /// The UV rect to draw for `identity`, or `None` while not even the fallback has arrived.
    pub(crate) fn uv(
        &self,
        text: &TextRenderer,
        identity: AvatarIdentityRef<'_>,
        expression: AvatarExpression,
    ) -> Option<[f32; 4]> {
        self.slot_for(identity, expression, |slot| text.avatar_uv(slot).is_some())
            .and_then(|slot| text.avatar_uv(slot))
    }

    /// Once per frame, before anything reads `avatar_uv`: size or empty the atlas, copy in the
    /// images that arrived, and start the next loads.
    pub(crate) fn sync_atlas(
        &mut self,
        text: &mut TextRenderer,
        context: &blade_graphics::Context,
    ) {
        if std::mem::take(&mut self.atlas_reset) {
            text.reset_avatars(context);
        }
        text.reserve_avatars(context, self.slots.len());
        if self.slots[AVATAR_FALLBACK_SLOT].load == Load::Loaded
            && text.avatar_uv(AVATAR_FALLBACK_SLOT).is_none()
        {
            // A new text renderer starts with an empty atlas: load every portrait again.
            for slot in &mut self.slots {
                slot.load = Load::Queued;
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let now = Instant::now();
            let arrived = std::mem::take(&mut *self.arrived.borrow_mut());
            for Arrival {
                generation,
                slot,
                image,
            } in arrived
            {
                let current = generation == self.generation;
                let uploaded = image.and_then(|image| {
                    let uploaded = if current {
                        text.upload_avatar(context, slot, &image)
                    } else {
                        Ok(())
                    };
                    image.close();
                    uploaded
                });
                if current {
                    self.finish_load(slot, uploaded, now);
                }
            }
            self.start_loads(now);
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn finish_load(&mut self, slot: usize, result: Result<(), String>, now: Instant) {
        let Some(entry) = self.slots.get_mut(slot) else {
            return;
        };
        match result {
            Ok(()) => {
                entry.load = Load::Loaded;
                entry.failures = 0;
            }
            Err(error) => {
                entry.failures += 1;
                let delay = FIRST_RETRY_DELAY
                    .saturating_mul(1 << (entry.failures - 1).min(5))
                    .min(MAX_RETRY_DELAY);
                entry.load = Load::RetryAt(now + delay);
                log::warn!(
                    "Avatar {} did not load: {error}; retrying in {}s",
                    entry.portrait.asset_path(),
                    delay.as_secs()
                );
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn start_loads(&mut self, now: Instant) {
        let mut in_flight = self
            .slots
            .iter()
            .filter(|slot| slot.load == Load::InFlight)
            .count();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if in_flight >= MAX_LOADS_IN_FLIGHT {
                break;
            }
            let due = match slot.load {
                Load::Queued => true,
                Load::RetryAt(at) => now >= at,
                Load::InFlight | Load::Loaded => false,
            };
            if due {
                slot.load = Load::InFlight;
                in_flight += 1;
                load_image(
                    slot.portrait.asset_path(),
                    index,
                    self.generation,
                    self.arrived.clone(),
                );
            }
        }
    }
}

/// Ask the shell's shared loader (`SOW_loadAvatarBitmap`) for one atlas cell of `path`.
#[cfg(target_arch = "wasm32")]
fn load_image(
    path: String,
    slot: usize,
    generation: u32,
    arrived: std::rc::Rc<std::cell::RefCell<Vec<Arrival>>>,
) {
    use wasm_bindgen::JsCast;

    fn js_error(error: wasm_bindgen::JsValue) -> String {
        error.as_string().unwrap_or_else(|| format!("{error:?}"))
    }

    let request = || -> Result<js_sys::Promise, String> {
        let window = web_sys::window().ok_or("window unavailable")?;
        let loader = js_sys::Reflect::get(&window, &"SOW_loadAvatarBitmap".into())
            .ok()
            .and_then(|value| value.dyn_into::<js_sys::Function>().ok())
            .ok_or("the shell's avatar loader is missing")?;
        let pending = loader
            .call2(
                &window,
                &path.as_str().into(),
                &sow_render::text::AVATAR_CELL.into(),
            )
            .map_err(js_error)?;
        Ok(js_sys::Promise::resolve(&pending))
    };
    let promise = match request() {
        Ok(promise) => promise,
        Err(error) => {
            arrived.borrow_mut().push(Arrival {
                generation,
                slot,
                image: Err(error),
            });
            return;
        }
    };
    wasm_bindgen_futures::spawn_local(async move {
        let image = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(js_error)
            .and_then(|value| {
                value
                    .dyn_into::<web_sys::ImageBitmap>()
                    .map_err(|_| "the loader returned no image".to_owned())
            });
        arrived.borrow_mut().push(Arrival {
            generation,
            slot,
            image,
        });
    });
}

fn valid_avatar_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use sow_core::geo_entities::{Role, bots};
    use sow_render::text::AVATAR_GOLD_SLOT;
    use std::collections::{HashMap, HashSet};

    fn bot(slug: &str) -> AvatarIdentityRef<'_> {
        AvatarIdentityRef::Portrait { slug, leader: None }
    }

    fn human(leader: Leader) -> AvatarIdentityRef<'static> {
        AvatarIdentityRef::Portrait {
            slug: leader.id(),
            leader: Some(leader),
        }
    }

    /// Every distinct bot portrait in the catalog; a bot that plays a hero is counted as the hero.
    fn catalog_bots() -> Vec<String> {
        let mut slugs: Vec<String> = [Role::Tribe, Role::Nation]
            .into_iter()
            .flat_map(bots)
            .filter_map(|entity| entity.avatar.clone())
            .filter(|slug| valid_avatar_id(slug) && !Leader::all().any(|leader| leader.id() == slug))
            .collect();
        slugs.sort();
        slugs.dedup();
        slugs
    }

    /// The failure agents kept reintroducing: a portrait losing its slot, or showing another
    /// player's, partway through a match. Here players arrive late, leave, come back and switch
    /// faces over many updates; every portrait must keep one slot of its own the whole match.
    #[test]
    fn every_match_portrait_keeps_its_own_slot_for_the_whole_match() {
        let mut avatars = AvatarSlots::new();
        let heroes: Vec<Leader> = Leader::all().step_by(9).collect();
        let bots = catalog_bots();
        let mut first_seen: HashMap<AvatarPortrait, usize> = HashMap::new();

        for update in 0..60usize {
            // A different, growing part of the roster shows up on each update.
            for (index, slug) in bots.iter().enumerate() {
                if index % 60 <= update && (index + update) % 3 != 0 {
                    avatars.add_identity(bot(slug));
                }
            }
            for (index, leader) in heroes.iter().enumerate() {
                if index <= update {
                    avatars.add_identity(human(*leader));
                }
            }
            for (slot, entry) in avatars.slots.iter().enumerate() {
                let kept = *first_seen.entry(entry.portrait.clone()).or_insert(slot);
                assert_eq!(kept, slot, "{:?} moved to another slot", entry.portrait);
            }
        }

        assert_eq!(avatars.slots[AVATAR_FALLBACK_SLOT].portrait, AvatarPortrait::Fallback);
        assert_eq!(avatars.slots[AVATAR_GOLD_SLOT].portrait, AvatarPortrait::Gold);
        assert_eq!(avatars.slots.len(), 2 + heroes.len() * 4 + bots.len());
        let distinct: HashSet<_> = avatars.slots.iter().map(|slot| &slot.portrait).collect();
        assert_eq!(distinct.len(), avatars.slots.len(), "two slots hold the same portrait");

        // Once loaded, each player draws their own slot, whatever face they show.
        let filled = |_| true;
        for slug in &bots {
            let slot = avatars.slot_for(bot(slug), AvatarExpression::Calm, filled);
            assert_eq!(slot, Some(avatars.portraits[slug]));
        }
        for leader in &heroes {
            for expression in AvatarExpression::ALL {
                let slot = avatars.slot_for(human(*leader), expression, filled);
                assert_eq!(slot, Some(avatars.heroes[&(*leader, expression)]));
            }
        }
    }

    #[test]
    fn unloaded_faces_fall_back_to_calm_then_to_the_shared_portrait() {
        let mut avatars = AvatarSlots::new();
        let leader = Leader::of("boudica");
        avatars.add_identity(human(leader));
        avatars.add_identity(bot("some_portrait"));
        let calm = avatars.heroes[&(leader, AvatarExpression::Calm)];
        let only = |filled: &'static [usize]| move |slot| filled.contains(&slot);

        assert_eq!(avatars.slot_for(human(leader), AvatarExpression::Fear, only(&[])), None);
        assert_eq!(
            avatars.slot_for(human(leader), AvatarExpression::Fear, only(&[0])),
            Some(AVATAR_FALLBACK_SLOT)
        );
        let filled = [AVATAR_FALLBACK_SLOT, calm];
        assert_eq!(
            avatars.slot_for(human(leader), AvatarExpression::Fear, |slot| filled.contains(&slot)),
            Some(calm)
        );
        assert_eq!(
            avatars.slot_for(bot("some_portrait"), AvatarExpression::Calm, |slot| filled
                .contains(&slot)),
            Some(AVATAR_FALLBACK_SLOT)
        );
        assert_eq!(
            avatars.slot_for(AvatarIdentityRef::Fallback, AvatarExpression::Calm, |slot| {
                filled.contains(&slot)
            }),
            Some(AVATAR_FALLBACK_SLOT)
        );
    }

    #[test]
    fn a_campaign_bot_playing_a_hero_gets_the_hero_faces() {
        let mut avatars = AvatarSlots::new();
        avatars.add_identity(bot("boudica"));
        let leader = Leader::of("boudica");
        for expression in AvatarExpression::ALL {
            assert!(avatars.heroes.contains_key(&(leader, expression)));
        }
        assert!(avatars.portraits.is_empty());
        assert_eq!(
            avatars.slots[avatars.heroes[&(leader, AvatarExpression::Fear)]]
                .portrait
                .asset_path(),
            "gameplay/avatars/expressions/boudica_fear.webp"
        );
    }

    #[test]
    fn the_whole_catalog_fits_in_one_match() {
        let mut avatars = AvatarSlots::new();
        for leader in Leader::all() {
            avatars.add_identity(human(leader));
        }
        let bots = catalog_bots();
        for slug in &bots {
            avatars.add_identity(bot(slug));
        }
        assert_eq!(avatars.slots.len(), 2 + Leader::count() * 4 + bots.len());
        assert!(avatars.slots.len() <= AVATAR_MAX_SLOTS);
    }

    #[test]
    fn ending_the_match_releases_every_slot_and_drops_late_images() {
        let mut avatars = AvatarSlots::new();
        avatars.end_match();
        assert!(!avatars.atlas_reset, "no match yet, nothing to empty");

        avatars.add_identity(bot("some_portrait"));
        let generation = avatars.generation;
        avatars.end_match();
        assert!(avatars.atlas_reset);
        assert_ne!(avatars.generation, generation);
        assert!(avatars.portraits.is_empty() && avatars.heroes.is_empty());
        assert_eq!(avatars.slots.len(), 2);
        assert!(avatars.slots.iter().all(|slot| slot.load == Load::Queued));
    }

    #[test]
    fn failed_loads_retry_with_growing_delays() {
        let mut avatars = AvatarSlots::new();
        let now = Instant::now();
        avatars.finish_load(AVATAR_FALLBACK_SLOT, Err("offline".into()), now);
        assert_eq!(
            avatars.slots[AVATAR_FALLBACK_SLOT].load,
            Load::RetryAt(now + FIRST_RETRY_DELAY)
        );
        for _ in 0..10 {
            avatars.finish_load(AVATAR_FALLBACK_SLOT, Err("offline".into()), now);
        }
        assert_eq!(
            avatars.slots[AVATAR_FALLBACK_SLOT].load,
            Load::RetryAt(now + MAX_RETRY_DELAY)
        );
        avatars.finish_load(AVATAR_FALLBACK_SLOT, Ok(()), now);
        assert_eq!(avatars.slots[AVATAR_FALLBACK_SLOT].load, Load::Loaded);
    }
}
