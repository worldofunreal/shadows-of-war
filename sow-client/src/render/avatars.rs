//! Nameplate portraits: which image each avatar atlas slot holds and how it gets there.
//! A portrait gets a slot when its player appears in the match and keeps it until the match ends,
//! so nothing a frame draws or skips can take it away. The browser loader the radial uses decodes
//! each file, and the atlas copies it straight to the GPU.
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

    /// The face a player shows first: Calm for a hero, the only face otherwise. The match
    /// loader waits for these; a hero's other three faces load after it.
    fn is_first_face(&self) -> bool {
        !matches!(self, Self::Hero(_, expression) if *expression != AvatarExpression::Calm)
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
                // A campaign bot can play as a hero and show all four faces. Hero faces are named by art slug.
                match sow_data::leaders::leader_from_art_slug(slug) {
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

    /// Whether every player's first face has loaded or failed once (a failure retries in the
    /// background and shows the fallback). The match loader releases only after this.
    pub(crate) fn first_faces_settled(&self) -> bool {
        self.in_match
            && self
                .slots
                .iter()
                .filter(|slot| slot.portrait.is_first_face())
                .all(|slot| slot.load == Load::Loaded || slot.failures > 0)
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
    /// Pending reserved portraits stay hidden; heroes use Calm when ready. Unallocated identities
    /// and explicit fallback identities use the shared fallback.
    fn slot_for(
        &self,
        identity: AvatarIdentityRef<'_>,
        expression: AvatarExpression,
        filled: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        let filled_slot = |slot: Option<&usize>| slot.copied().filter(|slot| filled(*slot));
        let fallback_slot = || filled(AVATAR_FALLBACK_SLOT).then_some(AVATAR_FALLBACK_SLOT);
        match self.owner(identity) {
            Some(Owner::Hero(leader)) => {
                let expression_slot = self.heroes.get(&(leader, expression));
                let calm_slot = self.heroes.get(&(leader, AvatarExpression::Calm));
                filled_slot(expression_slot).or_else(|| filled_slot(calm_slot))
                    .or_else(fallback_slot)
            }
            Some(Owner::Portrait(slug)) => match self.portraits.get(slug) {
                Some(slot) => filled(*slot).then_some(*slot).or_else(fallback_slot),
                None => fallback_slot(),
            },
            None => fallback_slot(),
        }
    }

    /// The UV rect to draw, or `None` while a reserved portrait is still loading.
    pub(crate) fn uv(
        &self,
        text: &TextRenderer,
        identity: AvatarIdentityRef<'_>,
        expression: AvatarExpression,
    ) -> Option<[f32; 4]> {
        self.slot_for(identity, expression, |slot| text.avatar_uv(slot).is_some())
            .and_then(|slot| text.avatar_uv(slot))
    }

    /// Whether a catalog or hero portrait has a loaded slot of its own, rather than the fallback.
    pub(crate) fn has_loaded_portrait(
        &self,
        text: &TextRenderer,
        identity: AvatarIdentityRef<'_>,
        expression: AvatarExpression,
    ) -> bool {
        self.slot_for(identity, expression, |slot| text.avatar_uv(slot).is_some())
            .is_some_and(|slot| slot != AVATAR_FALLBACK_SLOT)
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
        // First faces before the other hero faces, so the match loader can release sooner.
        let first = |index: &usize| self.slots[*index].portrait.is_first_face();
        let order: Vec<usize> = (0..self.slots.len())
            .filter(first)
            .chain((0..self.slots.len()).filter(|index| !first(index)))
            .collect();
        for index in order {
            if in_flight >= MAX_LOADS_IN_FLIGHT {
                break;
            }
            let slot = &mut self.slots[index];
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

