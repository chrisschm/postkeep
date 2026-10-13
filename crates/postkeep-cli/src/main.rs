//! Command-line program `postkeep`.
//!
//! In stage 0 (proof of concept) this program is deliberately small: it shows
//! an overview, help and version, and lets the user choose the language. It
//! grows with the core library, mainly as a tool to exercise and test the
//! core (storing mails, fetching, status journal) on real network shares.
//!
//! Command-line parsing is written by hand instead of using a library such as
//! `clap`: for a handful of options that keeps the dependency list short. If
//! the option set grows considerably, switching to a library is fine.
//!
//! All texts come from the translation files of the core library
//! (`postkeep_core::i18n`); only option names such as `--help`, which are the
//! same in every language, are written here directly.

use std::ffi::OsString;
use std::process::ExitCode;

use postkeep_core::i18n::{FluentArgs, Localizer, available_languages};

/// Name of the program as users type it.
const PROGRAM: &str = env!("CARGO_BIN_NAME");

/// Exit code for wrong usage (unknown option and the like). 2 is the common
/// convention for command-line programs; 0 means success, 1 other errors.
const EXIT_USAGE: u8 = 2;

/// What the user asked the program to do.
#[derive(Debug, PartialEq)]
enum Command {
    /// No option given: short description of the program.
    Overview,
    /// `--help`: list of options.
    Help,
    /// `--version`: version number.
    Version,
}

/// Result of reading the command line.
#[derive(Debug, PartialEq)]
struct Options {
    command: Command,
    /// Value of `--lang`, still unchecked. `None` means: system language.
    language: Option<String>,
}

/// Mistakes on the command line, reported to the user in their language.
#[derive(Debug, PartialEq)]
enum UsageError {
    /// An option the program does not know, as typed.
    UnknownOption(String),
    /// An option that needs a value was given without one.
    MissingValue(String),
}

/// Reads the command-line arguments (without the program name).
///
/// Accepted: `-h`/`--help`, `-V`/`--version`, `--lang <code>` and
/// `--lang=<code>`. If several commands are given, the last one wins.
fn parse_args<I>(args: I) -> Result<Options, UsageError>
where
    I: IntoIterator<Item = String>,
{
    let mut options = Options {
        command: Command::Overview,
        language: None,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => options.command = Command::Help,
            "-V" | "--version" => options.command = Command::Version,
            "--lang" => match args.next() {
                Some(value) => options.language = Some(value),
                None => return Err(UsageError::MissingValue(arg)),
            },
            _ => match arg.strip_prefix("--lang=") {
                Some(value) => options.language = Some(value.to_owned()),
                None => return Err(UsageError::UnknownOption(arg)),
            },
        }
    }
    Ok(options)
}

fn main() -> ExitCode {
    // `args_os` instead of `args`: `args` would crash on arguments that are
    // not valid Unicode (possible with odd file names). Such arguments are
    // converted with replacement characters instead; for option names that
    // is good enough.
    let args = std::env::args_os()
        .skip(1) // the first argument is the program's own path
        .map(|arg: OsString| arg.to_string_lossy().into_owned());

    let options = match parse_args(args) {
        Ok(options) => options,
        Err(error) => {
            // The user's --lang choice may not have been read yet, so errors
            // on the command line itself are shown in the system language.
            let l10n = Localizer::for_system();
            let (id, option) = match &error {
                UsageError::UnknownOption(option) => ("cli-unknown-option", option),
                UsageError::MissingValue(option) => ("cli-missing-value", option),
            };
            let mut args = FluentArgs::new();
            args.set("option", option.as_str());
            eprintln!("{}", l10n.text_with(id, &args));
            eprintln!("{}", help_hint(&l10n));
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let l10n = match &options.language {
        None => Localizer::for_system(),
        Some(code) => match code.parse() {
            Ok(language) => Localizer::new(&[language]),
            Err(_) => {
                let l10n = Localizer::for_system();
                let mut args = FluentArgs::new();
                args.set("language", code.as_str());
                eprintln!("{}", l10n.text_with("cli-invalid-language", &args));
                eprintln!("{}", help_hint(&l10n));
                return ExitCode::from(EXIT_USAGE);
            }
        },
    };

    match options.command {
        Command::Overview => {
            println!("{}", l10n.text("cli-about"));
            println!("{}", l10n.text("cli-stage0-note"));
            println!();
            println!("{}", help_hint(&l10n));
        }
        Command::Help => print_help(&l10n),
        // The version line contains no words, so it needs no translation.
        Command::Version => println!("{PROGRAM} {}", env!("CARGO_PKG_VERSION")),
    }
    ExitCode::SUCCESS
}

/// Prints the list of options.
fn print_help(l10n: &Localizer) {
    let languages = available_languages()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let mut lang_args = FluentArgs::new();
    lang_args.set("languages", languages);

    // Left column: option names (not translated); right column: description.
    let lang_option = format!("--lang <{}>", l10n.text("cli-lang-placeholder"));
    let rows = [
        ("-h, --help".to_owned(), l10n.text("cli-option-help")),
        ("-V, --version".to_owned(), l10n.text("cli-option-version")),
        (lang_option, l10n.text_with("cli-option-lang", &lang_args)),
    ];
    // The placeholder differs in length between languages, so the column
    // width is computed instead of fixed. `chars().count()` instead of
    // `len()`, because `len()` counts bytes and umlauts take two bytes.
    let width = rows
        .iter()
        .map(|(name, _)| name.chars().count())
        .max()
        .unwrap_or(0);

    let mut usage_args = FluentArgs::new();
    usage_args.set("program", PROGRAM);
    println!("{}", l10n.text("cli-about"));
    println!();
    println!("{}", l10n.text_with("cli-usage", &usage_args));
    println!();
    println!("{}", l10n.text("cli-options-heading"));
    for (name, description) in rows {
        println!("  {name:<width$}  {description}");
    }
}

/// The hint how to get help. Returned instead of printed, because it goes to
/// the normal output after the overview but to the error output after errors.
fn help_hint(l10n: &Localizer) -> String {
    let mut args = FluentArgs::new();
    args.set("program", PROGRAM);
    l10n.text_with("cli-help-hint", &args)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: parse a command line given as string slices.
    fn parse(args: &[&str]) -> Result<Options, UsageError> {
        parse_args(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn no_arguments_shows_overview() {
        assert_eq!(
            parse(&[]),
            Ok(Options {
                command: Command::Overview,
                language: None
            })
        );
    }

    #[test]
    fn short_and_long_options_are_equivalent() {
        assert_eq!(parse(&["-h"]), parse(&["--help"]));
        assert_eq!(parse(&["-V"]), parse(&["--version"]));
    }

    #[test]
    fn language_with_space_or_equals_sign() {
        let expected = Ok(Options {
            command: Command::Help,
            language: Some("de".to_owned()),
        });
        assert_eq!(parse(&["--lang", "de", "--help"]), expected);
        assert_eq!(parse(&["--help", "--lang=de"]), expected);
    }

    #[test]
    fn language_without_value_is_an_error() {
        assert_eq!(
            parse(&["--lang"]),
            Err(UsageError::MissingValue("--lang".to_owned()))
        );
    }

    #[test]
    fn unknown_option_is_an_error() {
        assert_eq!(
            parse(&["--frobnicate"]),
            Err(UsageError::UnknownOption("--frobnicate".to_owned()))
        );
    }
}
