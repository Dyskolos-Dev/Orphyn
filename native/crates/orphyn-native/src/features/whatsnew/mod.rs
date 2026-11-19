//! "What's new": the release notes of `src/renderer/src/features/whatsnew/notes/<version>.json`
//! (bundled at build time), shown once after each update (`WhatsNewHost.tsx`, `WhatsNewSheet.tsx`).
use dioxus::prelude::*;
use serde::Deserialize;

use crate::design::{Button, Icon};
use crate::features::servers::Sheet;
use crate::features::system::app_version;
use crate::i18n::{use_i18n, Lang};
use crate::state::{use_app, use_services};

const LAST_SEEN_KEY: &str = "orphyn.lastSeenVersion";

macro_rules! notes {
    ($($v:literal),* $(,)?) => {
        &[$(($v, include_str!(concat!(
            "../../../../../../src/renderer/src/features/whatsnew/notes/", $v, ".json"
        )))),*]
    };
}

/// Every bundled note file, as `(version, json)`.
const FILES: &[(&str, &str)] = notes![
    "1.0.2", "1.0.3", "1.0.4", "1.0.5", "1.1.0", "1.2.0", "1.2.1", "1.3.0", "1.4.0", "1.4.1",
    "1.5.0", "2.0.0", "2.0.1"
];

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct NoteItem {
    pub icon: String,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct NoteText {
    pub title: String,
    pub items: Vec<NoteItem>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ReleaseNote {
    pub fr: NoteText,
    pub en: NoteText,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VersionNotes {
    pub version: String,
    pub note: ReleaseNote,
}

/// Parses `1.2.3` (a leading `v` and any `-pre` / `+build` suffix are ignored).
pub fn parse_version(v: &str) -> Option<[u64; 3]> {
    let core = v
        .trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()?
        .to_string();
    let mut it = core.split('.').map(|p| p.parse::<u64>());
    let out = [it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?];
    Some(out)
}

/// Negative when a < b, 0 when equal, positive when a > b. Unparsable versions sort first.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    match (parse_version(a), parse_version(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

/// Every bundled release note, newest first.
pub fn bundled_notes() -> Vec<VersionNotes> {
    let mut out: Vec<VersionNotes> = FILES
        .iter()
        .filter_map(|(version, json)| {
            serde_json::from_str::<ReleaseNote>(json)
                .ok()
                .map(|note| VersionNotes {
                    version: (*version).to_string(),
                    note,
                })
        })
        .collect();
    out.sort_by(|a, b| compare_versions(&b.version, &a.version));
    out
}

/// Notes to present at launch, newest first. A fresh install (nothing stored, no account) shows
/// nothing; an unstored version with an account is an upgrade whose last seen version is unknown
/// (the 1.x web storage is not migrated): only the running version's notes show; skipped
/// versions all show up. Versions newer than the running app are never shown.
pub fn notes_to_show(
    last_seen: Option<&str>,
    current: &str,
    has_account: bool,
    notes: &[VersionNotes],
) -> Vec<VersionNotes> {
    let from = match last_seen {
        Some(v) => v,
        None if has_account => {
            return notes
                .iter()
                .filter(|n| compare_versions(&n.version, current).is_eq())
                .cloned()
                .collect();
        }
        None => return Vec::new(),
    };
    let mut out: Vec<VersionNotes> = notes
        .iter()
        .filter(|n| {
            compare_versions(&n.version, from).is_gt()
                && compare_versions(&n.version, current).is_le()
        })
        .cloned()
        .collect();
    out.sort_by(|a, b| compare_versions(&b.version, &a.version));
    out
}

fn note_icon(name: &str) -> &'static str {
    match name {
        "layout" => "layout-panel-top",
        "music" => "music",
        "download" => "download",
        "film" => "film",
        "users" => "users",
        "cast" => "cast",
        "refresh" => "refresh-cw",
        _ => "sparkles",
    }
}

/// The sheet: logo, title, one icon row per item, Continue.
#[component]
pub fn WhatsNewSheet(notes: Vec<VersionNotes>, on_close: EventHandler<()>) -> Element {
    let t = use_i18n();
    let Some(newest) = notes.first() else {
        return rsx! {};
    };
    let fr = t.lang() == Lang::Fr;
    let title = t.tf(
        "common:whatsNew.title",
        &[("version", newest.version.clone())],
    );
    rsx! {
        Sheet {
            title,
            size: "medium",
            on_close,
            footer: rsx! {
                Button { onclick: move |_| on_close.call(()), {t.t("common:whatsNew.continue")} }
            },
            div { class: "jr-whatsnew__body",
                img { class: "jr-whatsnew__logo", src: crate::assets::logo_url(), width: "64", height: "64", alt: "" }
                for (i , n) in notes.iter().enumerate() {
                    {
                        let text = if fr { &n.note.fr } else { &n.note.en };
                        rsx! {
                            section { key: "{n.version}", class: "jr-whatsnew__version",
                                if i > 0 {
                                    div { class: "jr-whatsnew__heading",
                                        {t.tf("common:whatsNew.versionHeading", &[("version", n.version.clone()), ("title", text.title.clone())])}
                                    }
                                }
                                ul { class: "jr-whatsnew__items",
                                    for item in text.items.iter() {
                                        li { key: "{item.title}", class: "jr-whatsnew__item",
                                            span { class: "jr-whatsnew__icon", Icon { name: note_icon(&item.icon), size: 28 } }
                                            div {
                                                div { class: "jr-whatsnew__item-title", "{item.title}" }
                                                div { class: "jr-whatsnew__item-text", "{item.text}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Mounted once at the root: shows the notes of every version newer than the last one seen,
/// then stores the running version. A fresh install only stores it.
#[component]
pub fn WhatsNewHost() -> Element {
    let svc = use_services();
    let app = use_app();
    let current = app_version();
    let mut notes = use_signal(Vec::<VersionNotes>::new);
    use_hook({
        let (svc, current) = (svc.clone(), current.clone());
        move || {
            let last = svc.storage.get(LAST_SEEN_KEY);
            let has_account = !app.accounts.peek().is_empty();
            let toshow = notes_to_show(last.as_deref(), &current, has_account, &bundled_notes());
            if toshow.is_empty() {
                svc.storage.set(LAST_SEEN_KEY, &current);
            } else {
                notes.set(toshow);
            }
        }
    });
    let list = notes();
    if list.is_empty() {
        return rsx! {};
    }
    rsx! {
        WhatsNewSheet {
            notes: list,
            on_close: move |_| {
                svc.storage.set(LAST_SEEN_KEY, &current);
                notes.set(Vec::new());
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(v: &str) -> VersionNotes {
        let item = NoteItem {
            icon: "x".into(),
            title: "t".into(),
            text: "t".into(),
        };
        let text = NoteText {
            title: "t".into(),
            items: vec![item],
        };
        VersionNotes {
            version: v.into(),
            note: ReleaseNote {
                fr: text.clone(),
                en: text,
            },
        }
    }

    #[test]
    fn every_bundled_file_parses_with_both_languages() {
        assert_eq!(bundled_notes().len(), FILES.len());
        assert_eq!(bundled_notes()[0].version, "2.0.1");
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(compare_versions("1.10.0", "1.9.0").is_gt());
        assert!(compare_versions("v1.0.0-beta", "1.0.0").is_eq());
        assert!(compare_versions("nope", "1.0.0").is_lt());
    }

    #[test]
    fn upgrades_show_each_skipped_version_once_newest_first() {
        let all = vec![note("1.0.3"), note("1.1.0"), note("1.2.0"), note("2.0.0")];
        let shown = |last: Option<&str>, has_account| {
            notes_to_show(last, "1.2.0", has_account, &all)
                .iter()
                .map(|n| n.version.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(shown(Some("1.0.3"), true), ["1.2.0", "1.1.0"]);
        assert!(shown(Some("1.2.0"), true).is_empty());
        assert!(
            shown(None, false).is_empty(),
            "a fresh install shows nothing"
        );
        assert_eq!(shown(None, true), ["1.2.0"]);
    }

    #[test]
    fn the_running_version_has_bundled_notes() {
        let v = crate::features::system::app_version();
        assert!(
            FILES.iter().any(|(n, _)| *n == v),
            "add notes/{v}.json to FILES"
        );
    }
}
