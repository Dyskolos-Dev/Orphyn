//! Settings > Appearance (`AppearancePane.tsx`): style, light / dark, accent, materials, layout.
use dioxus::prelude::*;
use serde_json::Value;

use super::style_gallery::{PreviewLayout, StyleGallery, StylePreview};
use super::ui::{List, ListRow, Toggle};
use crate::design::{Button, Segment, SegmentedControl, Slider};
use crate::i18n::use_i18n;
use crate::state::{use_app, use_services};
use crate::theme::{accent_tokens, ThemeMode, ACCENT_PRESETS};

const CORNERS: [&str; 3] = ["sharp", "standard", "round"];
const DENSITY: [&str; 3] = ["comfortable", "standard", "compact"];
const TEXT_SIZE: [&str; 4] = ["s", "m", "l", "xl"];
const SIDEBAR: [&str; 3] = ["expanded", "icons", "auto-hide"];
const BACKDROP: [&str; 3] = ["off", "subtle", "immersive"];
const MOTION: [&str; 2] = ["full", "reduced"];
const VIDEO_CARDS: [&str; 2] = ["poster", "landscape"];
const SOLID_STYLES: [&str; 1] = ["minimal"];

fn seg(t: &crate::i18n::I18n, prefix: &str, values: &[&'static str]) -> Vec<Segment> {
    values
        .iter()
        .map(|v| Segment {
            value: v,
            label: t.t(&format!("{prefix}.{v}")),
        })
        .collect()
}

#[component]
pub fn AppearancePane() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let style = app.interface.read().style;
    let a = app.interface.read().appearance();
    let has_overrides = !app.interface.read().overrides.is_empty();
    let solid = SOLID_STYLES.contains(&style);
    let style_name = t.t(&format!("settings:appearance.styles.{style}.name"));
    let mode = app.theme.read().mode.as_str();
    let (accent, follow) = {
        let th = app.theme.read();
        (th.accent.clone(), th.follow_artwork)
    };
    let resolved = app.theme.read().resolve((app.os_dark)());

    let set = {
        let svc = svc.clone();
        move |key: &'static str, value: Value| {
            let mut s = app.interface.peek().clone();
            s.overrides.insert(key.into(), value);
            s.save(&svc.storage);
            let mut sig = app.interface;
            sig.set(s);
        }
    };
    let set_str = {
        let set = set.clone();
        move |key: &'static str, v: &'static str| set(key, Value::from(v))
    };
    let (s_mode, s_accent, s_follow) =
        (svc.clone(), svc.clone(), svc.clone());
    let s_reset = svc.clone();
    let transparency = if solid { 0 } else { a.transparency };
    let (set_t, set_c, set_d, set_x, set_sb, set_bd, set_m, set_v) = (
        set.clone(),
        set_str.clone(),
        set_str.clone(),
        set_str.clone(),
        set_str.clone(),
        set_str.clone(),
        set_str.clone(),
        set_str.clone(),
    );

    rsx! {
        div { class: "jr-appearance",
            figure { class: "jr-appearance__preview",
                StylePreview { style, layout: PreviewLayout { sidebar: a.sidebar, backdrop: a.backdrop, video_cards: a.video_cards }, large: true }
                figcaption { class: "jr-appearance__caption",
                    span { class: "jr-appearance__caption-title", {style_name.clone()} }
                    span { {t.t("settings:appearance.previewCaption")} }
                }
            }
            List { header: t.t("settings:appearance.style.header"),
                li { class: "jr-list__item",
                    StyleGallery {}
                }
            }
            List { header: t.t("settings:appearance.mode.header"),
                ListRow {
                    title: rsx! { {t.t("settings:appearance.mode.label")} },
                    trailing: rsx! {
                        SegmentedControl {
                            label: t.t("settings:appearance.mode.label"),
                            size: "small",
                            value: mode,
                            options: seg(&t, "settings:appearance.mode", &["system", "light", "dark"]),
                            on_change: move |v: &'static str| app.set_theme(&s_mode, |th| th.mode = ThemeMode::parse(Some(v))),
                        }
                    },
                }
            }
            List { header: t.t("settings:appearance.accent.header"),
                li { class: "jr-list__item",
                    div { class: "jr-swatches", role: "radiogroup", "aria-label": t.t("settings:appearance.accent.label"),
                        for (name , _l , _d) in ACCENT_PRESETS.iter() {
                            {
                                let name: &'static str = name;
                                let fill = accent_tokens(&crate::theme::accent_hex(name, resolved), resolved).fill;
                                let svc = s_accent.clone();
                                let title = t.t(&format!("settings:appearance.accent.colors.{name}"));
                                rsx! {
                                    button {
                                        key: "{name}",
                                        r#type: "button",
                                        role: "radio",
                                        "aria-checked": (accent == name).to_string(),
                                        "aria-label": "{title}",
                                        title: "{title}",
                                        class: "jr-swatch",
                                        style: "background: {fill};",
                                        onmousedown: move |e| e.stop_propagation(),
                                        onclick: move |_| app.set_theme(&svc, |th| th.accent = name.to_string()),
                                    }
                                }
                            }
                        }
                    }
                }
                ListRow {
                    title: rsx! { {t.t("settings:appearance.accent.followArtwork.label")} },
                    subtitle: t.t("settings:appearance.accent.followArtwork.description"),
                    trailing: rsx! {
                        Toggle { checked: follow, label: String::new(), on_change: move |v| app.set_theme(&s_follow, |th| th.follow_artwork = v) }
                    },
                }
            }
            List { header: t.t("settings:appearance.materials.header"),
                ListRow {
                    title: rsx! { {t.t("settings:appearance.materials.transparency.label")} },
                    subtitle: if solid { t.t("settings:appearance.materials.transparency.solidStyle") } else { t.t("settings:appearance.materials.transparency.description") },
                    trailing: rsx! {
                        div { class: "jr-appearance__slider",
                            Slider {
                                variant: "regular",
                                label: t.t("settings:appearance.materials.transparency.label"),
                                min: 0.0,
                                max: 100.0,
                                value: transparency as f64,
                                disabled: solid,
                                on_change: move |v: f64| set_t("transparency", Value::from(((v / 5.0).round() * 5.0) as i64)),
                            }
                            span { class: "jr-appearance__value jr-tabular", "aria-hidden": "true", "{transparency}" }
                        }
                    },
                }
                ListRow {
                    title: rsx! { {t.t("settings:appearance.materials.corners.label")} },
                    trailing: rsx! {
                        SegmentedControl { label: t.t("settings:appearance.materials.corners.label"), size: "small", value: a.corners,
                            options: seg(&t, "settings:appearance.materials.corners", &CORNERS), on_change: move |v| set_c("corners", v) }
                    },
                }
                ListRow {
                    title: rsx! { {t.t("settings:appearance.materials.density.label")} },
                    subtitle: t.t("settings:appearance.materials.density.description"),
                    trailing: rsx! {
                        SegmentedControl { label: t.t("settings:appearance.materials.density.label"), size: "small", value: a.density,
                            options: seg(&t, "settings:appearance.materials.density", &DENSITY), on_change: move |v| set_d("density", v) }
                    },
                }
                ListRow {
                    title: rsx! { {t.t("settings:appearance.materials.textSize.label")} },
                    trailing: rsx! {
                        SegmentedControl { label: t.t("settings:appearance.materials.textSize.label"), size: "small", value: a.text_size,
                            options: seg(&t, "settings:appearance.materials.textSize", &TEXT_SIZE), on_change: move |v| set_x("textSize", v) }
                    },
                }
            }
            List { header: t.t("settings:appearance.layout.header"),
                ListRow {
                    title: rsx! { {t.t("settings:appearance.layout.sidebar.label")} },
                    subtitle: (a.sidebar == "auto-hide").then(|| t.t("settings:appearance.layout.sidebar.autoHideHint")),
                    trailing: rsx! {
                        SegmentedControl { label: t.t("settings:appearance.layout.sidebar.label"), size: "small", value: a.sidebar,
                            options: seg(&t, "settings:appearance.layout.sidebar", &SIDEBAR), on_change: move |v| set_sb("sidebar", v) }
                    },
                }
                ListRow {
                    title: rsx! { {t.t("settings:appearance.layout.backdrop.label")} },
                    subtitle: t.t("settings:appearance.layout.backdrop.description"),
                    trailing: rsx! {
                        SegmentedControl { label: t.t("settings:appearance.layout.backdrop.label"), size: "small", value: a.backdrop,
                            options: seg(&t, "settings:appearance.layout.backdrop", &BACKDROP), on_change: move |v| set_bd("backdrop", v) }
                    },
                }
                ListRow {
                    title: rsx! { {t.t("settings:appearance.layout.videoCards.label")} },
                    trailing: rsx! {
                        SegmentedControl { label: t.t("settings:appearance.layout.videoCards.label"), size: "small", value: a.video_cards,
                            options: seg(&t, "settings:appearance.layout.videoCards", &VIDEO_CARDS), on_change: move |v| set_v("videoCards", v) }
                    },
                }
                ListRow {
                    title: rsx! { {t.t("settings:appearance.layout.motion.label")} },
                    subtitle: t.t("settings:appearance.layout.motion.description"),
                    trailing: rsx! {
                        SegmentedControl { label: t.t("settings:appearance.layout.motion.label"), size: "small", value: a.motion,
                            options: seg(&t, "settings:appearance.layout.motion", &MOTION), on_change: move |v| set_m("motion", v) }
                    },
                }
            }
            div { class: "jr-appearance__reset",
                p { class: "jr-appearance__reset-text",
                    {t.tf("settings:appearance.reset.description", &[("style", style_name.clone())])}
                }
                Button {
                    variant: "gray",
                    icon: "rotate-ccw",
                    disabled: !has_overrides,
                    onclick: move |_| {
                        let mut s = app.interface.peek().clone();
                        s.overrides.clear();
                        s.save(&s_reset.storage);
                        let mut sig = app.interface;
                        sig.set(s);
                    },
                    {t.t("settings:appearance.reset.label")}
                }
            }
        }
    }
}
