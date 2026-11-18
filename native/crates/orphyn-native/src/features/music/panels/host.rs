//! Feeds the playback preferences to the music engine: the crossfade settings and the audio
//! output device. Rendered once, by the Now Playing layer (always mounted).
use dioxus::prelude::*;
use orphyn_player::music::crossfade::CrossfadeSettings;
use orphyn_player::music::devices::{output_devices, resolve_device};

use crate::state::{use_app, use_services};

#[component]
pub fn PanelsHost() -> Element {
    let app = use_app();
    let player = use_services().music;
    use_effect(move || {
        let p = (app.preferences)();
        player.set_crossfade(CrossfadeSettings {
            enabled: p.crossfade,
            seconds: p.crossfade_seconds as f64,
            skip_same_album: p.crossfade_skip_album,
        });
        // A saved device that is unplugged falls back to the system default.
        let device = p.audio_output_id.as_deref().and_then(|saved| {
            let devices = output_devices();
            resolve_device(Some(saved), Some(&devices))
        });
        player.set_output_device(device);
    });
    rsx! {}
}
