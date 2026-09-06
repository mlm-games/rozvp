//! Pilot i18n: embedded Fluent catalogs, one bundle per locale.
//! Same keys (`TRANSLATION_KEYS`) and same embedded `.ftl` sources as the
//! bevy build, so translators touch one set of files. External crates only
//! (`fluent-bundle` + `unic-langid`); no new house crate.

use std::collections::HashMap;
use std::str::FromStr;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
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
    "finished-level",
    "pick-seeds",
    "ok",
    "user-label",
    "adventure-sub",
    "minigames-sub",
    "puzzle-sub",
    "survival-sub",
    "tagline",
    "hud-sun",
    "hud-flags",
    "hud-progress",
    "shovel",
    "advice-night-intro",
    "advice-click-sun",
    "advice-plant-sunflower",
    "advice-zombies-coming",
    "advice-huge-wave",
    "credits-line-1",
    "credits-line-2",
    "credits-line-3",
    "credits-line-4",
    "seed-sunflower",
    "seed-peashooter",
    "seed-snow-pea",
    "seed-repeater",
    "seed-wall-nut",
    "seed-potato-mine",
    "seed-cherry-bomb",
    "seed-chomper",
    "seed-squash",
    "seed-threepeater",
    "seed-jalapeno",
    "seed-spikeweed",
    "seed-torchwood",
    "seed-tall-nut",
    "seed-garlic",
    "seed-hypno-shroom",
    "seed-ice-shroom",
    "seed-doom-shroom",
    // Parameterized messages (need `t_with_args`; plain `t()` renders the
    // arg name). Kept out of the Bevy `TRANSLATION_KEYS` on purpose: the
    // Bevy extractor formats with no args, which would store garbage.
    "hud-sun-count",
    "hud-flags-count",
    "hud-progress-pct",
    "award-seed-is",
    // Short packet labels. Translators only need these where truncating
    // the full name reads badly; missing ones fall back to truncation.
    "seed-sunflower-short",
    "seed-peashooter-short",
    "seed-snow-pea-short",
    "seed-repeater-short",
    "seed-wall-nut-short",
    "seed-potato-mine-short",
    "seed-cherry-bomb-short",
    "seed-chomper-short",
    "seed-squash-short",
    "seed-threepeater-short",
    "seed-jalapeno-short",
    "seed-spikeweed-short",
    "seed-torchwood-short",
    "seed-tall-nut-short",
    "seed-garlic-short",
    "seed-hypno-shroom-short",
    "seed-ice-shroom-short",
    "seed-doom-shroom-short",
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
            // No bidi isolates: interpolated counts/names must stay plain
            // text for canvas measurement (mirrors game-utils core).
            bundle.set_use_isolating(false);
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
        self.t_with_args(key, None)
    }

    /// Fluent-aware lookup with args (counts, names) and plural selects.
    /// Same fallback chain as `t`: current locale, then English, then key.
    /// Translators can reorder `{ $name }` placeables per locale; callers
    /// must pass args for every placeable the message declares.
    pub fn t_with_args(&self, key: &str, args: Option<&FluentArgs>) -> String {
        self.lookup(&self.current, key, args)
            .or_else(|| self.lookup("en", key, args))
            .unwrap_or_else(|| key.to_string())
    }

    /// Current-locale-only lookup, no fallback. For optional keys
    /// (`seed-*-short`) where falling back to English would show the wrong
    /// language instead of a truncated local name.
    pub fn t_local(&self, key: &str) -> Option<String> {
        self.lookup(&self.current, key, None)
    }

    fn lookup(&self, code: &str, key: &str, args: Option<&FluentArgs>) -> Option<String> {
        let bundle = self.bundles.get(code)?;
        let msg = bundle.get_message(key)?;
        let pattern = msg.value()?;
        let mut errors = Vec::new();
        let value = bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned();
        // A message that fails to format (missing arg, bad placeable) is
        // treated as missing: fall back to English, then the key. A
        // half-rendered `{$count}` must never reach the canvas.
        if errors.is_empty() {
            Some(value)
        } else {
            None
        }
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

    /// Args for parameterized messages. Plain `t()` on these would render
    /// the arg name, so the coverage loop below goes through `t_with_args`.
    fn args_for(key: &str) -> Option<FluentArgs> {
        let mut args = FluentArgs::new();
        match key {
            "hud-sun-count" => {
                args.set("count", 150);
                Some(args)
            }
            "hud-flags-count" => {
                args.set("done", 2);
                args.set("total", 4);
                Some(args)
            }
            "hud-progress-pct" => {
                args.set("pct", 50);
                Some(args)
            }
            "award-seed-is" => {
                args.set("title", "T");
                args.set("seed", "S");
                Some(args)
            }
            _ => None,
        }
    }

    #[test]
    fn all_keys_resolve_in_all_locales() {
        // Non-English catalogs may still miss keys while translators work;
        // the contract is per-key fallback to English, never a raw key.
        let mut loc = Localizer::new();
        assert_eq!(loc.available().len(), 7);
        for code in loc.available() {
            assert!(loc.set_language(&code));
            for key in TRANSLATION_KEYS {
                let got = match args_for(key) {
                    Some(args) => loc.t_with_args(key, Some(&args)),
                    None => loc.t(key),
                };
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
    fn args_interpolate_and_fall_back() {
        let loc = Localizer::new();
        let mut sun = FluentArgs::new();
        sun.set("count", 150);
        assert_eq!(
            loc.t_with_args("hud-sun-count", Some(&sun)),
            "Sun 150"
        );
        let mut flags = FluentArgs::new();
        flags.set("done", 2);
        flags.set("total", 4);
        assert_eq!(
            loc.t_with_args("hud-flags-count", Some(&flags)),
            "Flags 2/4"
        );
        let mut award = FluentArgs::new();
        award.set("title", "You got a new plant!");
        award.set("seed", "Wall-nut");
        assert_eq!(
            loc.t_with_args("award-seed-is", Some(&award)),
            "You got a new plant! Wall-nut"
        );
        // Other locales render their own word order through the same call.
        let mut fr = Localizer::new();
        assert!(fr.set_language("fr"));
        assert_eq!(
            fr.t_with_args("hud-sun-count", Some(&sun)),
            "Soleil 150"
        );
        // Unknown locale falls back to English, never the raw key.
        assert_eq!(loc.t_with_args("hud-sun-count", None), "hud-sun-count");
    }

    #[test]
    fn shorts_fall_back_without_leaking_english() {
        let loc = Localizer::new();
        // English ships translator shorts.
        assert_eq!(loc.t_local("seed-sunflower-short"), Some("Sunf".into()));
        // Other locales have none yet: `t_local` reports absence so views
        // truncate the translated full name instead of showing English.
        let mut ja = Localizer::new();
        assert!(ja.set_language("ja"));
        assert_eq!(ja.t_local("seed-sunflower-short"), None);
        assert_eq!(ja.t("seed-sunflower"), "ひまわり");
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
