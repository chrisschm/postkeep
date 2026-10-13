//! Translations of all user-visible texts ("internationalisation", i18n).
//!
//! # Rules for contributors
//!
//! - **No user-visible text is written directly into the code.** Every text
//!   shown to users (command-line output, later the desktop interface) is a
//!   *message* in the translation files and is fetched through a
//!   [`Localizer`]. Log output for developers stays in English and does not
//!   go through this module.
//! - **English is the base language.** It is always complete and is used as
//!   fallback whenever a text is missing in the user's language.
//! - **Every message exists in every bundled language.** A unit test
//!   (`all_languages_have_the_same_messages`) fails if a message is added to
//!   one file but not to the others.
//! - Translation files live in `crates/postkeep-core/locales/<language>/` and
//!   are compiled into the program, so no files need to be installed next to
//!   it. Only this core crate holds translations; user interfaces get their
//!   texts from here.
//!
//! # Format
//!
//! The files use [Project Fluent](https://projectfluent.org/) (`.ftl`), the
//! format Firefox and Thunderbird use. Unlike simple key/value files, Fluent
//! lets each language express its own grammar, for example plural forms
//! ("1 mail" / "2 mails") without the code having to know about them.
//!
//! # Adding a language
//!
//! 1. Copy `locales/en/postkeep.ftl` to `locales/<code>/postkeep.ftl`
//!    (e.g. `fr`) and translate the texts, keeping the message ids.
//! 2. Add the language to [`BUNDLED`] below.
//! 3. Run `cargo test` - the tests check that the file parses and contains
//!    exactly the same messages as the English one.

use fluent_bundle::FluentResource;
use fluent_bundle::concurrent::FluentBundle;
use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use unic_langid::LanguageIdentifier;

/// All languages shipped with Postkeep: language code and file content.
///
/// The files are embedded at compile time with `include_str!`. The first
/// entry must be the base language (English), see [`BASE_LANGUAGE`].
const BUNDLED: &[(&str, &str)] = &[
    ("en", include_str!("../locales/en/postkeep.ftl")),
    ("de", include_str!("../locales/de/postkeep.ftl")),
];

/// Code of the base language. It is complete by definition and the last
/// fallback for every lookup.
pub const BASE_LANGUAGE: &str = "en";

/// Looks up translated texts for one user, in the best matching language.
///
/// A `Localizer` holds the translations of the chosen language plus the
/// English base language as fallback. Create it once at program start and
/// pass it to wherever texts are needed.
///
/// The type is thread-safe (`Send + Sync`), so a later desktop interface can
/// share one instance between threads.
pub struct Localizer {
    /// Translations in lookup order: the chosen language first, English
    /// last. Never empty; English is always contained.
    bundles: Vec<FluentBundle<FluentResource>>,
}

impl Localizer {
    /// Creates a localizer for the given preferred languages, best first.
    ///
    /// The preferences are matched against the bundled languages; for
    /// example `de-AT` (German as used in Austria) selects `de`. Languages
    /// Postkeep does not ship are skipped. If nothing matches, English is
    /// used.
    pub fn new(preferred: &[LanguageIdentifier]) -> Self {
        let available = available_languages();
        let base: LanguageIdentifier = BASE_LANGUAGE
            .parse()
            .expect("BASE_LANGUAGE is a valid language code");

        // "Filtering" keeps every available language that matches one of the
        // preferences, in the order of preference, and appends the default
        // (English) at the end if it is not already contained. That gives us
        // exactly the fallback chain we want.
        let chosen = negotiate_languages(
            preferred,
            &available,
            Some(&base),
            NegotiationStrategy::Filtering,
        );

        let bundles = chosen.into_iter().map(build_bundle).collect();
        Localizer { bundles }
    }

    /// Creates a localizer that follows the operating system's language
    /// settings (Windows: display language, Linux: `LANG`/`LC_*` variables).
    pub fn for_system() -> Self {
        Self::new(&system_languages())
    }

    /// The language actually used for texts, e.g. `de`.
    pub fn language(&self) -> &LanguageIdentifier {
        // `bundles` is never empty and every bundle has exactly one locale,
        // see `new` and `build_bundle`.
        &self.bundles[0].locales[0]
    }

    /// Returns the text of a message without variables.
    ///
    /// If the message does not exist in any language, the message id itself
    /// is returned, so a missing text is visible instead of silently empty.
    pub fn text(&self, id: &str) -> String {
        self.format(id, None)
    }

    /// Returns the text of a message, filling in variables.
    ///
    /// Variables appear in the translation files as `{ $name }`:
    ///
    /// ```
    /// use postkeep_core::i18n::{FluentArgs, Localizer};
    ///
    /// let l10n = Localizer::new(&["en".parse().unwrap()]);
    /// let mut args = FluentArgs::new();
    /// args.set("option", "--frobnicate");
    /// assert_eq!(
    ///     l10n.text_with("cli-unknown-option", &args),
    ///     "Unknown option: --frobnicate"
    /// );
    /// ```
    pub fn text_with(&self, id: &str, args: &FluentArgs) -> String {
        self.format(id, Some(args))
    }

    /// Shared implementation of [`Self::text`] and [`Self::text_with`]:
    /// tries every language in fallback order.
    fn format(&self, id: &str, args: Option<&FluentArgs>) -> String {
        for bundle in &self.bundles {
            let Some(pattern) = bundle.get_message(id).and_then(|m| m.value()) else {
                continue; // not translated in this language - try the next one
            };
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, args, &mut errors);
            // Errors here are mistakes in a translation, typically a variable
            // the code did not supply. Fluent still produces usable text (it
            // shows the variable name), so users are not blocked. Debug builds
            // and tests stop here to make the mistake obvious.
            debug_assert!(errors.is_empty(), "message {id}: {errors:?}");
            return text.into_owned();
        }
        id.to_owned()
    }
}

/// Variables for [`Localizer::text_with`]. Re-exported so callers can pass
/// variables without depending on `fluent-bundle` themselves.
pub use fluent_bundle::FluentArgs;

/// All languages Postkeep ships with, base language first.
pub fn available_languages() -> Vec<LanguageIdentifier> {
    BUNDLED
        .iter()
        .map(|(code, _)| code.parse().expect("bundled language code is valid"))
        .collect()
}

/// The user's preferred languages according to the operating system, best
/// first. Entries that are not valid language codes (such as the `C` locale
/// on Linux) are skipped; the result may therefore be empty.
pub fn system_languages() -> Vec<LanguageIdentifier> {
    sys_locale::get_locales()
        .filter_map(|code| code.parse().ok())
        .collect()
}

/// Builds the translation bundle for one bundled language.
///
/// The translation files are part of the program, so errors in them are
/// programming errors, not runtime conditions: they panic here and are caught
/// by the unit tests long before a release.
fn build_bundle(language: &LanguageIdentifier) -> FluentBundle<FluentResource> {
    let source = BUNDLED
        .iter()
        .find(|(code, _)| code.parse::<LanguageIdentifier>().as_ref() == Ok(language))
        .map(|(_, source)| *source)
        .expect("negotiation only returns bundled languages");

    let resource = FluentResource::try_new(source.to_owned())
        .unwrap_or_else(|_| panic!("translation file for {language} has syntax errors"));

    let mut bundle = FluentBundle::new_concurrent(vec![language.clone()]);
    // By default Fluent wraps every inserted variable in invisible Unicode
    // direction marks (needed for mixing left-to-right and right-to-left
    // scripts in graphical interfaces). In a terminal these marks show up as
    // garbage or break copy & paste, so they are switched off. Revisit when
    // right-to-left languages are added.
    bundle.set_use_isolating(false);
    bundle
        .add_resource(resource)
        .unwrap_or_else(|_| panic!("translation file for {language} defines a message twice"));
    bundle
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn lang(code: &str) -> LanguageIdentifier {
        code.parse().unwrap()
    }

    /// Message ids defined in one translation file.
    fn message_ids(source: &str) -> BTreeSet<String> {
        let resource = fluent_syntax::parser::parse(source)
            .unwrap_or_else(|(_, errors)| panic!("syntax errors: {errors:?}"));
        resource
            .body
            .iter()
            .filter_map(|entry| match entry {
                fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn base_language_comes_first() {
        assert_eq!(BUNDLED[0].0, BASE_LANGUAGE);
    }

    #[test]
    fn every_bundled_file_loads() {
        for language in available_languages() {
            build_bundle(&language); // panics on syntax errors or duplicates
        }
    }

    #[test]
    fn all_languages_have_the_same_messages() {
        let base = message_ids(BUNDLED[0].1);
        assert!(!base.is_empty());
        for (code, source) in &BUNDLED[1..] {
            let ids = message_ids(source);
            let missing: Vec<_> = base.difference(&ids).collect();
            let extra: Vec<_> = ids.difference(&base).collect();
            assert!(
                missing.is_empty() && extra.is_empty(),
                "{code}: missing {missing:?}, not in English {extra:?}"
            );
        }
    }

    #[test]
    fn regional_variant_selects_language() {
        assert_eq!(Localizer::new(&[lang("de-AT")]).language(), &lang("de"));
    }

    #[test]
    fn unknown_or_no_language_falls_back_to_english() {
        assert_eq!(Localizer::new(&[lang("fr")]).language(), &lang("en"));
        assert_eq!(Localizer::new(&[]).language(), &lang("en"));
    }

    #[test]
    fn variables_are_inserted_without_direction_marks() {
        let mut args = FluentArgs::new();
        args.set("option", "-x");
        let text = Localizer::new(&[lang("de")]).text_with("cli-unknown-option", &args);
        assert_eq!(text, "Unbekannte Option: -x");
    }

    #[test]
    fn unknown_message_returns_its_id() {
        assert_eq!(
            Localizer::new(&[]).text("no-such-message"),
            "no-such-message"
        );
    }
}
