//! Pilot i18n: embedded Fluent catalogs, one bundle per locale.
//! Same keys (`TRANSLATION_KEYS`) and same embedded `.ftl` sources as the
//! bevy build, so translators touch one set of files. External crates only
//! (`fluent-bundle` + `unic-langid`); no new house crate.

use std::collections::HashMap;
use std::str::FromStr;

use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

/// Keys the UI may look up. Mirrors `app::TRANSLATION_KEYS`.
pub const TRANSLATION_KEYS: &[&str] = &[
    "app-title",
    "adventure",
    "mini-games",
    "puzzle",
    "survival",
    "zen-garden",
    "almanac",
    "store",
    "settings",
    "help",
    "quit",
    "paused",
    "resume",
    "restart",
    "main-menu",
    "quit-to-title",
    "save",
    "back",
    "master-volume",
    "sfx-volume",
    "music-volume",
    "language",
    "choose-your-seeds",
    "your-bank",
    "available-packets",
    "lets-rock",
    "level-complete",
    "game-over",
    "zombies-ate-your-brains",
    "not-enough-sun",
    "try-again",
    "continue",
    "loading",
    "award-title",
    "awesome",
];

const LOCALES: &[(&str, &str)] = &[
    ("en", include_str!("../../assets/locales/en/main.ftl")),
    ("es", include_str!("../../assets/locales/es/main.ftl")),
    ("fr", include_str!("../../assets/locales/fr/main.ftl")),
    ("de", include_str!("../../assets/locales/de/main.ftl")),
    ("ja", include_str!("../../assets/locales/ja/main.ftl")),
    ("zh", include_str!("../../assets/locales/zh/main.ftl")),
    ("pt", include_str!("../../assets/locales/pt/main.ftl")),
];

pub struct Localizer {
    bundles: HashMap<String, FluentBundle<FluentResource>>,
    current: String,
}

impl Localizer {
    pub fn new() -> Self {
        let mut bundles = HashMap::new();
        for (code, ftl) in LOCALES {
            let lang = LanguageIdentifier::from_str(code).expect("locale code parses");
            let mut bundle = FluentBundle::new(vec![lang]);
            // Catalogs ship with the repo; a broken one is a build bug.
            // Skip it instead of panicking so one locale can't brick boot.
            if let Ok(res) = FluentResource::try_new(ftl.to_string()) {
                let _ = bundle.add_resource(res);
                bundles.insert(code.to_string(), bundle);
            }
        }
        Self {
            bundles,
            current: "en".to_string(),
        }
    }

    pub fn available(&self) -> Vec<String> {
        let mut codes: Vec<String> = self.bundles.keys().cloned().collect();
        codes.sort();
        codes
    }

    pub fn language(&self) -> &str {
        &self.current
    }

    /// Switch language. Returns false (keeping current) for unknown codes.
    pub fn set_language(&mut self, code: &str) -> bool {
        if self.bundles.contains_key(code) {
            self.current = code.to_string();
            true
        } else {
            false
        }
    }

    /// Look up a key. Falls back to English, then to the key itself
    /// (matches bevy `I18nPlugin` missing-key behavior of showing the key).
    pub fn t(&self, key: &str) -> String {
        self.lookup(&self.current, key)
            .or_else(|| self.lookup("en", key))
            .unwrap_or_else(|| key.to_string())
    }

    fn lookup(&self, code: &str, key: &str) -> Option<String> {
        let bundle = self.bundles.get(code)?;
        let msg = bundle.get_message(key)?;
        let pattern = msg.value()?;
        let mut errors = Vec::new();
        Some(
            bundle
                .format_pattern(pattern, None, &mut errors)
                .into_owned(),
        )
    }
}

impl Default for Localizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_keys_resolve_in_all_locales() {
        // Non-English catalogs are partial (translators in progress);
        // the contract is per-key fallback to English, never a raw key.
        let mut loc = Localizer::new();
        assert_eq!(loc.available().len(), 7);
        for code in loc.available() {
            assert!(loc.set_language(&code));
            for key in TRANSLATION_KEYS {
                let got = loc.t(key);
                assert!(!got.is_empty(), "{code} empty value for {key}");
                assert_ne!(got, *key, "{code} leaked raw key {key}");
            }
        }
    }

    #[test]
    fn english_spot_checks() {
        let loc = Localizer::new();
        assert_eq!(loc.t("app-title"), "RoZVP");
        assert_eq!(loc.t("lets-rock"), "Let's Rock!");
    }

    #[test]
    fn unknown_key_returns_key() {
        let loc = Localizer::new();
        assert_eq!(loc.t("no-such-key"), "no-such-key");
    }

    #[test]
    fn switch_rejects_unknown() {
        let mut loc = Localizer::new();
        assert!(loc.set_language("fr"));
        assert_eq!(loc.language(), "fr");
        assert!(!loc.set_language("xx"));
        assert_eq!(loc.language(), "fr");
    }
}
