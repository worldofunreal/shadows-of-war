use super::engine::{ArpeggioSource, SAMPLE_RATE, play_ui};

fn result_source(
    note_freqs: [f32; 4],
    note_duration_secs: f32,
    decay: f32,
    amplitude: f32,
) -> ArpeggioSource {
    let note_duration = (SAMPLE_RATE as f32 * note_duration_secs) as u32;
    ArpeggioSource::new(
        note_freqs,
        [note_duration, note_duration, note_duration, 0],
        decay,
        amplitude,
    )
}

pub fn play_victory_sound() {
    play_ui(result_source([392.0, 523.25, 659.25, 0.0], 0.13, 6.5, 0.11));
}

pub fn play_defeat_sound() {
    play_ui(result_source([330.0, 262.0, 196.0, 0.0], 0.18, 5.5, 0.09));
}
