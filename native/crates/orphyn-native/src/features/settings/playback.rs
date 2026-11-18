//! Settings > General (language), Playback (`PlaybackPane.tsx`) and Music (`MusicPane.tsx`).
use dioxus::prelude::*;
use orphyn_player::music::devices;

use super::ui::{GrayButton, List, ListRow, Toggle, ValueMenu};
use crate::design::{Segment, SegmentedControl, Slider};
use crate::i18n::{use_i18n, Lang, LanguageChoice};
use crate::preferences::{
    AudioQuality, PlaybackPreferences, AUDIO_BITRATES, CROSSFADE_SECONDS_MAX,
    CROSSFADE_SECONDS_MIN, LANGUAGES, VIDEO_BITRATES,
};
use crate::state::{use_app, use_services, Services};

/// Crossfade and output device are fed to the engine by `PanelsHost`; the transcoding quality is
/// baked in the URL resolver, so a change rebuilds it.
pub fn apply_playback_prefs(
    svc: &Services,
    p: &PlaybackPreferences,
    session: Option<orphyn_jellyfin::endpoints::Session>,
) {
    if let Some(s) = session {
        let quality = p.audio_quality;
        svc.music.set_resolver(std::sync::Arc::new(move |t| {
            Some(match quality {
                AudioQuality::Original => s.audio_url(&t.id),
                AudioQuality::Bitrate(b) => s.audio_transcode_url(&t.id, b as u32),
            })
        }));
    }
}

type PrefEdit = Box<dyn FnOnce(&mut PlaybackPreferences)>;

/// A preference setter that persists, then applies to the engine.
fn updater(app: crate::state::AppState, svc: Services) -> impl Fn(PrefEdit) + Clone {
    move |f| {
        app.set_preferences(&svc, f);
        apply_playback_prefs(&svc, &app.preferences.peek(), app.session.peek().clone());
    }
}

#[component]
pub fn GeneralPane() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let lang = use_context::<crate::i18n::I18n>().lang;
    let language = (app.language)().as_str();
    rsx! {
        List { header: t.t("settings:general.language.header"),
            ListRow {
                title: rsx! { {t.t("settings:general.language.label")} },
                subtitle: t.t("settings:general.language.description"),
                trailing: rsx! {
                    SegmentedControl {
                        label: t.t("settings:general.language.label"),
                        size: "small",
                        value: language,
                        options: ["system", "fr", "en"]
                            .iter()
                            .map(|l| Segment { value: l, label: t.t(&format!("settings:general.language.{l}")) })
                            .collect::<Vec<_>>(),
                        on_change: move |v: &'static str| {
                            let choice = match v {
                                "fr" => LanguageChoice::Fixed(Lang::Fr),
                                "en" => LanguageChoice::Fixed(Lang::En),
                                _ => LanguageChoice::System,
                            };
                            app.set_language(&svc, choice, lang);
                        },
                    }
                },
            }
        }
    }
}

#[component]
pub fn PlaybackPane() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let update = updater(app, svc);
    let p = (app.preferences)();

    let languages = |none: String| -> Vec<(Option<String>, String)> {
        std::iter::once((None, none))
            .chain(LANGUAGES.iter().map(|l| {
                (
                    Some(l.to_string()),
                    t.t(&format!("settings:playback.languages.{l}")),
                )
            }))
            .collect()
    };
    let bitrates: Vec<(Option<u64>, String)> =
        std::iter::once((None, t.t("settings:playback.auto")))
            .chain(VIDEO_BITRATES.iter().map(|b| {
                (
                    Some(*b),
                    t.tf(
                        "settings:playback.mbps",
                        &[("value", (b / 1_000_000).to_string())],
                    ),
                )
            }))
            .collect();
    let devs = use_hook(devices::output_devices);
    let mut outputs: Vec<(Option<String>, String)> =
        vec![(None, t.t("settings:playback.outputDefault"))];
    outputs.extend(devs.iter().map(|d| (Some(d.id.clone()), d.label.clone())));
    if let Some(id) = &p.audio_output_id {
        if !outputs.iter().any(|(v, _)| v.as_ref() == Some(id)) {
            outputs.push((Some(id.clone()), t.t("settings:playback.outputMissing")));
        }
    }
    let (u1, u2, u3, u4, u5, u6) = (
        update.clone(),
        update.clone(),
        update.clone(),
        update.clone(),
        update.clone(),
        update,
    );
    let max_bitrate = p.max_video_bitrate.map(|b| b as u64);
    let (audio_lang, sub_lang) = (
        p.preferred_audio_language.clone(),
        p.preferred_subtitle_language.clone(),
    );

    rsx! {
        List { header: t.t("settings:playback.videoHeader"),
            ListRow {
                title: rsx! { {t.t("settings:playback.maxBitrate")} },
                subtitle: t.t("settings:playback.maxBitrateFooter"),
                trailing: rsx! {
                    ValueMenu::<Option<u64>> {
                        label: t.t("settings:playback.maxBitrate"),
                        value: max_bitrate,
                        options: bitrates,
                        on_change: move |v: Option<u64>| u1(Box::new(move |p| p.max_video_bitrate = v.map(|b| b as f64))),
                    }
                },
            }
            ListRow {
                title: rsx! { {t.t("settings:playback.audioLanguage")} },
                trailing: rsx! {
                    ValueMenu::<Option<String>> {
                        label: t.t("settings:playback.audioLanguage"),
                        value: audio_lang,
                        options: languages(t.t("settings:playback.defaultTrack")),
                        on_change: move |v: Option<String>| u2(Box::new(move |p| p.preferred_audio_language = v)),
                    }
                },
            }
            ListRow {
                title: rsx! { {t.t("settings:playback.subtitleLanguage")} },
                trailing: rsx! {
                    ValueMenu::<Option<String>> {
                        label: t.t("settings:playback.subtitleLanguage"),
                        value: sub_lang,
                        options: languages(t.t("settings:playback.subtitlesOff")),
                        on_change: move |v: Option<String>| u3(Box::new(move |p| p.preferred_subtitle_language = v)),
                    }
                },
            }
            ListRow {
                title: rsx! {
                    Toggle {
                        label: t.t("settings:playback.autoPlayNext"),
                        description: t.t("settings:playback.autoPlayNextDescription"),
                        checked: p.auto_play_next,
                        on_change: move |v| u4(Box::new(move |p| p.auto_play_next = v)),
                    }
                },
            }
        }
        if !devs.is_empty() {
            List { header: t.t("settings:playback.outputHeader"),
                ListRow {
                    title: rsx! { {t.t("settings:playback.output")} },
                    subtitle: t.t("settings:playback.outputFooter"),
                    trailing: rsx! {
                        ValueMenu::<Option<String>> {
                            label: t.t("settings:playback.output"),
                            value: p.audio_output_id.clone(),
                            options: outputs,
                            on_change: move |v: Option<String>| u5(Box::new(move |p| p.audio_output_id = v)),
                        }
                    },
                }
            }
        }
        div {
            GrayButton {
                onclick: move |_| u6(Box::new(|p| *p = PlaybackPreferences::default())),
                {t.t("settings:playback.reset")}
            }
        }
    }
}

#[component]
pub fn MusicPane() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let update = updater(app, svc);
    let p = (app.preferences)();
    let (u1, u2, u3, u4, u5, u6, u7) = (
        update.clone(),
        update.clone(),
        update.clone(),
        update.clone(),
        update.clone(),
        update.clone(),
        update,
    );
    let seconds = t.tf(
        "settings:music.seconds",
        &[("value", p.crossfade_seconds.to_string())],
    );
    let quality: Vec<(String, String)> =
        std::iter::once(("original".to_string(), t.t("settings:music.original")))
            .chain(AUDIO_BITRATES.iter().map(|b| {
                (
                    b.to_string(),
                    t.tf("settings:music.kbps", &[("value", (b / 1000).to_string())]),
                )
            }))
            .collect();
    let current_quality = match p.audio_quality {
        AudioQuality::Original => "original".to_string(),
        AudioQuality::Bitrate(b) => (b as u64).to_string(),
    };
    let toggle = |label: &str, desc: &str, checked: bool, on: Callback<bool>| {
        rsx! {
            ListRow {
                title: rsx! {
                    Toggle { label: t.t(label), description: t.t(desc), checked, on_change: move |v| on.call(v) }
                },
            }
        }
    };

    rsx! {
        List { header: t.t("settings:music.header"), footer: t.t("settings:music.qualityFooter"),
            {toggle("settings:music.crossfade", "settings:music.crossfadeDescription", p.crossfade,
                Callback::new({ let u = u1.clone(); move |v| u(Box::new(move |p| p.crossfade = v)) }))}
            if p.crossfade {
                ListRow {
                    title: rsx! { {t.t("settings:music.crossfadeDuration")} },
                    trailing: rsx! {
                        div { class: "jr-appearance__slider",
                            Slider {
                                variant: "regular",
                                label: t.t("settings:music.crossfadeDuration"),
                                min: CROSSFADE_SECONDS_MIN as f64,
                                max: CROSSFADE_SECONDS_MAX as f64,
                                value: p.crossfade_seconds as f64,
                                on_change: move |v: f64| u2(Box::new(move |p| p.crossfade_seconds = v.round() as i64)),
                            }
                            span { class: "jr-music__value jr-tabular", "aria-hidden": "true", "{seconds}" }
                        }
                    },
                }
                {toggle("settings:music.crossfadeSkipAlbum", "settings:music.crossfadeSkipAlbumDescription", p.crossfade_skip_album,
                    Callback::new({ let u = u3.clone(); move |v| u(Box::new(move |p| p.crossfade_skip_album = v)) }))}
            }
            {toggle("settings:music.groupArtists", "settings:music.groupArtistsDescription", p.group_artists,
                Callback::new({ let u = u4.clone(); move |v| u(Box::new(move |p| p.group_artists = v)) }))}
            {toggle("settings:music.artistImages", "settings:music.artistImagesDescription", p.artist_images,
                Callback::new({ let u = u5.clone(); move |v| u(Box::new(move |p| p.artist_images = v)) }))}
            {toggle("settings:music.onlineLyrics", "settings:music.onlineLyricsDescription", p.online_lyrics,
                Callback::new({ let u = u6.clone(); move |v| u(Box::new(move |p| p.online_lyrics = v)) }))}
            ListRow {
                title: rsx! { {t.t("settings:music.quality")} },
                trailing: rsx! {
                    ValueMenu::<String> {
                        label: t.t("settings:music.quality"),
                        value: current_quality,
                        options: quality,
                        on_change: move |v: String| u7(Box::new(move |p| {
                            p.audio_quality = if v == "original" {
                                AudioQuality::Original
                            } else {
                                AudioQuality::Bitrate(v.parse().unwrap_or(320_000.0))
                            }
                        })),
                    }
                },
            }
        }
    }
}
