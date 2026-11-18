//! Translations: the very JSON files of the React app (`src/renderer/src/i18n/locales/<lang>/<ns>.json`,
//! embedded at build time), with i18next's rules: `ns:key.path`, default namespace `common`,
//! fallback language `en`, `{{name}}` interpolation and `_one` / `_other` (`_zero`) plurals with
//! `count`. The choice is stored under `orphyn.language` (`system`, `fr`, `en`) like the React app.
//!
//! In components: `let t = use_i18n(); t.t("shell:nav.home")`, `t.tf("music:album.moreBy",
//! &[("artist", name)])`, `t.tc("music:album.footer", n, &[])`.
use std::collections::HashMap;
use std::sync::OnceLock;

use dioxus::prelude::*;
use serde_json::Value;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/locales.rs"));
}

pub const LANGUAGE_STORAGE_KEY: &str = "orphyn.language";
pub const FALLBACK: Lang = Lang::En;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }

    fn from_code(code: &str) -> Option<Lang> {
        match code {
            "fr" => Some(Lang::Fr),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    /// `Intl.PluralRules` category for `count` (French counts 0 and 1 as singular).
    fn plural(self, count: i64) -> &'static str {
        match (self, count) {
            (Lang::Fr, 0 | 1) | (Lang::En, 1) => "one",
            _ => "other",
        }
    }
}

/// `system`, `fr` or `en` (`LanguageChoice` of `features/settings/language.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanguageChoice {
    System,
    Fixed(Lang),
}

impl LanguageChoice {
    pub fn parse(v: Option<&str>) -> Self {
        match v.and_then(Lang::from_code) {
            Some(l) => LanguageChoice::Fixed(l),
            None => LanguageChoice::System,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            LanguageChoice::System => "system",
            LanguageChoice::Fixed(l) => l.code(),
        }
    }

    pub fn resolve(self) -> Lang {
        match self {
            LanguageChoice::Fixed(l) => l,
            LanguageChoice::System => system_language(),
        }
    }
}

/// The OS language: French when it starts with `fr`, English otherwise (like `systemLanguage`).
pub fn system_language() -> Lang {
    let locale = std::env::var("ORPHYN_LANG")
        .ok()
        .or_else(sys_locale::get_locale)
        .unwrap_or_default();
    if locale.to_lowercase().starts_with("fr") {
        Lang::Fr
    } else {
        Lang::En
    }
}

type Bundles = HashMap<Lang, HashMap<&'static str, Value>>;

fn bundles() -> &'static Bundles {
    static B: OnceLock<Bundles> = OnceLock::new();
    B.get_or_init(|| {
        let mut map = HashMap::new();
        for (lang, ns, text) in embedded::LOCALES {
            let Some(lang) = Lang::from_code(lang) else { continue };
            match serde_json::from_str(text) {
                Ok(v) => {
                    map.entry(lang).or_insert_with(HashMap::new).insert(*ns, v);
                }
                Err(e) => log::warn!("[i18n] {}/{ns}.json: {e}", lang.code()),
            }
        }
        map
    })
}

fn lookup(lang: Lang, ns: &str, path: &str) -> Option<&'static str> {
    let mut v: &'static Value = bundles().get(&lang)?.get(ns)?;
    for part in path.split('.') {
        v = v.get(part)?;
    }
    v.as_str()
}

fn split_key(key: &str) -> (&str, &str) {
    match key.split_once(':') {
        Some((ns, k)) => (ns, k),
        None => ("common", key),
    }
}

fn interpolate(template: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let name = after[..end].trim();
                match args.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => out.push_str(v),
                    None => out.push_str(&rest[start..start + 2 + end + 2]),
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Translates `key` in `lang` (falls back to English, then to the key itself).
pub fn translate(lang: Lang, key: &str, count: Option<i64>, args: &[(&str, String)]) -> String {
    let (ns, path) = split_key(key);
    let find = |l: Lang| -> Option<&'static str> {
        if let Some(n) = count {
            if n == 0 {
                if let Some(s) = lookup(l, ns, &format!("{path}_zero")) {
                    return Some(s);
                }
            }
            if let Some(s) = lookup(l, ns, &format!("{path}_{}", l.plural(n))) {
                return Some(s);
            }
        }
        lookup(l, ns, path)
    };
    let Some(template) = find(lang).or_else(|| find(FALLBACK)) else {
        return path.to_string();
    };
    match count {
        Some(n) if !args.iter().any(|(k, _)| *k == "count") => {
            let mut all = args.to_vec();
            all.push(("count", n.to_string()));
            interpolate(template, &all)
        }
        _ => interpolate(template, args),
    }
}

/// The UI language, provided at the root and read by every component through [`use_i18n`].
#[derive(Clone, Copy, PartialEq)]
pub struct I18n {
    pub lang: Signal<Lang>,
}

impl I18n {
    pub fn lang(&self) -> Lang {
        (self.lang)()
    }

    pub fn t(&self, key: &str) -> String {
        translate(self.lang(), key, None, &[])
    }

    pub fn tf(&self, key: &str, args: &[(&str, String)]) -> String {
        translate(self.lang(), key, None, args)
    }

    pub fn tc(&self, key: &str, count: i64, args: &[(&str, String)]) -> String {
        translate(self.lang(), key, Some(count), args)
    }
}

pub fn use_i18n() -> I18n {
    use_context::<I18n>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_react_locales() {
        assert_eq!(translate(Lang::En, "shell:nav.albums", None, &[]), "Albums");
        assert_eq!(translate(Lang::Fr, "shell:nav.settings", None, &[]), "Réglages");
        assert_eq!(translate(Lang::En, "app.name", None, &[]), "Orphyn");
    }

    #[test]
    fn interpolates_and_pluralises() {
        assert_eq!(
            translate(Lang::En, "music:album.footer", Some(1), &[]),
            "1 song"
        );
        assert_eq!(
            translate(Lang::En, "music:album.footer", Some(12), &[]),
            "12 songs"
        );
        assert_eq!(
            translate(Lang::En, "music:album.moreBy", None, &[("artist", "Muse".into())]),
            "More by Muse"
        );
    }

    #[test]
    fn missing_keys_fall_back() {
        assert_eq!(translate(Lang::Fr, "shell:does.not.exist", None, &[]), "does.not.exist");
        assert_eq!(interpolate("{{a}} and {{b}}", &[("a", "1".into())]), "1 and {{b}}");
    }
}
