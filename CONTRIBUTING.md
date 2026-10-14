# Contributing

Thanks for considering a contribution! For how Postkeep is built and why, start at
[`docs/README.md`](docs/README.md). This file covers the practical side: setting up a development
environment, the checks every change has to pass, and the project's conventions.
*(Eine deutsche Kurzfassung findest du am Ende dieser Datei.)*

Postkeep is at an early stage (proof of concept). For anything larger than a small fix, please
open an issue first so the approach can be discussed before you invest time.

## Development environment

Postkeep is written in [Rust](https://www.rust-lang.org/) and supports **Windows** and **Linux**.

1. Install Rust with [rustup](https://rustup.rs/). The file `rust-toolchain.toml` makes rustup
   use the right toolchain (current stable) with `rustfmt` and `clippy` automatically.
2. Platform requirements:
   - **Windows:** the Microsoft C++ build tools (MSVC), as described by the rustup installer.
   - **Linux:** a C compiler and `pkg-config`, e.g. on Debian/Ubuntu
     `sudo apt install build-essential pkg-config`. For tests against a network share also
     `cifs-utils`.
3. Install [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) for the dependency checks:
   `cargo install cargo-deny` (or `cargo binstall cargo-deny` for a prebuilt binary).
4. Build and run:

   ```
   cargo build
   cargo run -p postkeep-cli -- --help
   ```

**Tip for building the same checkout from Windows and WSL:** both would write into the same
`target/` folder and keep rebuilding each other's files. In WSL, use a separate build folder, e.g.
`export CARGO_TARGET_DIR=$HOME/target/postkeep`.

### Repository layout

| Path | Content |
|---|---|
| `crates/postkeep-core/` | Core library: everything the user interfaces build on |
| `crates/postkeep-core/locales/` | Translation files (one folder per language) |
| `crates/postkeep-cli/` | Command-line program `postkeep` |
| `docs/` | Developer documentation, starting at `docs/README.md` |
| `.github/` | Issue forms, pull request template and CI workflow (`workflows/`) |

## Checks before every pull request

All of these must pass - the pull request template contains the same list:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check        # needed when dependencies changed
```

Run `cargo fmt` to fix formatting instead of formatting by hand.

The same checks run automatically on GitHub (workflow `.github/workflows/ci.yml`) for every pull
request, on Linux and Windows. A pull request can only be merged when they pass. Running them
locally first saves you a round trip. CI uses `--locked`, so a pull request that changes
dependencies has to include the updated `Cargo.lock`.

Tests against real infrastructure (a network share, a mail server) are not part of CI; they are
run by hand, see [Tests](#tests).

## Tests

The archive Postkeep manages is the only copy of the user's mail (see
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), "Safety rules"). Tests are therefore not optional:

- **Every safety rule has unit tests, especially for error cases:** an interruption after every
  single step, a read-back that does not match, the network share disappearing, a lock that is
  taken or expired.
- **Integration tests run against a real SMB share**, from Windows and Linux **at the same time**
  - many problems only show up with two computers and a real network drive.
- **Never use real mailboxes in tests.** Use a local test server or a dedicated test account.
- **No personal data in tests, examples or issues:** no real mails, addresses, server names or
  passwords. Use placeholders such as `user@example.com` (the `example.com`/`.org`/`.net` domains
  are reserved for exactly this).

## Conventions

### Languages

- Code, comments, commit messages and developer documentation: **English**.
- Issues and pull requests: English or German.

### User-visible texts and translations

- **No user-visible text is written directly into the code.** Every text is a message in the
  translation files under `crates/postkeep-core/locales/` and is looked up at runtime. Log output
  for developers stays in English and is not translated.
- **Add every new message to all language files.** A unit test checks that all files contain the
  same messages. If you don't speak a language, copy the English text into that file and say so
  in your pull request - a maintainer will translate it.
- **Data on the network share is language-neutral:** folder names of standard folders, journal and
  configuration contain identifiers only, never translated text.
- Details, naming of messages and how to add a language: module documentation of
  `crates/postkeep-core/src/i18n.rs`.

### Code

- Comment generously. Many people reading this code are not Rust experts; explain *why* code does
  something, not only what. Every public item of the core library needs documentation (the
  compiler warns otherwise).
- `unsafe` code is forbidden in Postkeep's own crates.
- Text files use LF line endings and UTF-8 (enforced by `.gitattributes` and `.editorconfig`).

### Dependencies

- Prefer the standard library and small, focused crates; every new dependency needs a reason,
  noted as a comment next to it in the workspace `Cargo.toml` and in the pull request.
- Its license must be compatible with GPL-3.0-or-later; `cargo deny check` verifies this.

### Documentation

- Keep `docs/` up to date with every change, ideally in the same commit. `docs/ARCHITECTURE.md`
  is updated with every development step; mark what is implemented and what is planned.
- New documentation files are listed in `docs/README.md`.
- The user manual lives in the [wiki](https://github.com/chrisschm/postkeep/wiki).

### Issues

- New issues are opened through the forms (bug report, feature request).
- **Security vulnerabilities are never reported as public issues** - see
  [`SECURITY.md`](SECURITY.md).

## License

Postkeep is licensed under the [GNU General Public License v3.0 or later](LICENSE). By submitting a
contribution, you agree that it is licensed under the same terms.

---

## Auf Deutsch (Kurzfassung)

Danke fürs Mitmachen! Postkeep steht am Anfang (Machbarkeitsnachweis); bei größeren Vorhaben bitte
zuerst ein Issue eröffnen.

- **Umgebung:** Rust über rustup (Version und Komponenten legt `rust-toolchain.toml` fest); unter
  Windows die MSVC-Build-Tools, unter Linux `build-essential`, `pkg-config` (für Share-Tests
  `cifs-utils`); dazu `cargo-deny`. Beim Bauen desselben Ordners aus Windows und WSL in WSL einen
  eigenen `CARGO_TARGET_DIR` setzen.
- **Pflichtprüfungen vor jedem Pull Request:** `cargo fmt --check`, `cargo clippy --all-targets --
  -D warnings`, `cargo test`, bei geänderten Abhängigkeiten `cargo deny check`. Dieselben
  Prüfungen laufen für jeden Pull Request automatisch auf GitHub (Linux und Windows,
  `.github/workflows/ci.yml`); gemergt wird nur, wenn sie grün sind. Tests gegen echte
  Infrastruktur (Share, Mailserver) laufen nicht in der CI, sondern von Hand.
- **Tests:** Jede Sicherheitsregel mit Unit-Tests, besonders für Fehlerfälle; Integrationstests
  gegen einen echten SMB-Share von Windows und Linux gleichzeitig; nie echte Postfächer, nie
  persönliche Daten (Platzhalter wie `user@example.com`).
- **Sprachen:** Code, Kommentare, Commits und Entwickler-Doku auf Englisch; Issues und Pull
  Requests auf Englisch oder Deutsch.
- **Texte:** Kein sichtbarer Text fest im Code, alles über die Übersetzungsdateien; neue Texte in
  allen Sprachdateien anlegen (wer eine Sprache nicht spricht, übernimmt den englischen Text und
  sagt es im Pull Request). Daten auf dem Share sind sprachneutral.
- **Code:** großzügig kommentieren, kein `unsafe`, LF-Zeilenenden.
- **Abhängigkeiten:** sparsam und begründet; Lizenz muss zur GPL-3.0-or-later passen.
- **Doku:** `docs/` mit jeder Änderung nachziehen, `docs/ARCHITECTURE.md` bei jedem Schritt;
  Handbuch im Wiki.
- **Sicherheitslücken** nie als öffentliches Issue, siehe `SECURITY.md`.
- **Lizenz:** Beiträge stehen unter derselben Lizenz (GPL-3.0-or-later).
