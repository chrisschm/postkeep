# Postkeep - German texts (Deutsch)
#
# Translation of locales/en/postkeep.ftl, which is the reference: this file
# must contain exactly the same message ids (checked by the unit tests).
# Translator notes are taken over from the English file in English.

## Command-line program (postkeep-cli)

# One-line description, shown at the top of the overview and the help.
cli-about = Postkeep - Mailprogramm mit gemeinsamem Archiv auf einem Netzlaufwerk.
# Shown when the program is started without options. Stage 0 is the proof of
# concept, which does not handle mails yet.
cli-stage0-note = Machbarkeitsnachweis: noch keine Mail-Funktionen.
# First line of the help. $program is the name of the program, "postkeep".
cli-usage = Aufruf: { $program } [Optionen]
cli-options-heading = Optionen:
cli-option-help = Diese Hilfe anzeigen und beenden.
cli-option-version = Die Versionsnummer anzeigen und beenden.
# Placeholder for the value of the --lang option, shown as "--lang <code>".
cli-lang-placeholder = Sprachcode
# $languages is a comma-separated list of language codes, e.g. "en, de".
cli-option-lang = Diese Sprache statt der Systemsprache verwenden. Verfügbar: { $languages }
# Error messages. $option is the option as typed by the user, e.g. "--foo".
cli-unknown-option = Unbekannte Option: { $option }
cli-missing-value = Die Option { $option } braucht einen Wert.
# $language is the value given to --lang.
cli-invalid-language = „{ $language }“ ist kein gültiger Sprachcode.
# Shown below error messages. $program is the name of the program.
cli-help-hint = „{ $program } --help“ zeigt alle Optionen.
