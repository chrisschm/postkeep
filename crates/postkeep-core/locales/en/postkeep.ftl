# Postkeep - English texts (base language)
#
# This file is the reference for all other languages: every message defined
# here must exist in every other locales/<language>/postkeep.ftl as well
# (checked by the unit tests). Format: Project Fluent, see
# https://projectfluent.org/fluent/guide/
#
# Message ids start with the area they belong to (e.g. "cli-" for the
# command-line program). Comments directly above a message are notes for
# translators; please keep them in English in every language file.

## Command-line program (postkeep-cli)

# One-line description, shown at the top of the overview and the help.
cli-about = Postkeep - mail client with a shared archive on a network drive.
# Shown when the program is started without options. Stage 0 is the proof of
# concept, which does not handle mails yet.
cli-stage0-note = Proof of concept: no mail functions yet.
# First line of the help. $program is the name of the program, "postkeep".
cli-usage = Usage: { $program } [options]
cli-options-heading = Options:
cli-option-help = Show this help and exit.
cli-option-version = Show the version number and exit.
# Placeholder for the value of the --lang option, shown as "--lang <code>".
cli-lang-placeholder = code
# $languages is a comma-separated list of language codes, e.g. "en, de".
cli-option-lang = Use this language instead of the system language. Available: { $languages }
# Error messages. $option is the option as typed by the user, e.g. "--foo".
cli-unknown-option = Unknown option: { $option }
cli-missing-value = The option { $option } needs a value.
# $language is the value given to --lang.
cli-invalid-language = "{ $language }" is not a valid language code.
# Shown below error messages. $program is the name of the program.
cli-help-hint = Run "{ $program } --help" for a list of options.
