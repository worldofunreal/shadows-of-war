use super::clip::{SfxId, pick_clip};
use super::engine::play_ui;

pub fn play_victory_sound() {
    if let Some(clip) = pick_clip(SfxId::Victory, 0.50) {
        play_ui(clip);
    }
}

pub fn play_defeat_sound() {
    if let Some(clip) = pick_clip(SfxId::Defeat, 0.50) {
        play_ui(clip);
    }
}
