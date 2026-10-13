# Security Policy

*(Eine deutsche Kurzfassung findest du am Ende dieser Datei.)*

## Reporting a vulnerability

**Please do not report security vulnerabilities through public issues, pull requests or
discussions.**

Use GitHub's private vulnerability reporting instead:

**[Report a vulnerability](https://github.com/chrisschm/postkeep/security/advisories/new)**
(repository tab "Security" → "Report a vulnerability")

Your report is visible only to you and the maintainers. Please include:

- what is affected (component, version or commit),
- how to reproduce it - steps or a proof of concept,
- the impact you expect (what an attacker could achieve).

Please do not include real personal data (mail contents, addresses, passwords) - use placeholders.

## What happens next

Postkeep is maintained by volunteers in their spare time, so please allow some time for a
response. The report is confirmed, assessed and - if it is a vulnerability - fixed in coordination
with you. Once a fix is available, a security advisory is published; on request you are credited
in it. Please keep the vulnerability confidential until then.

## Supported versions

Postkeep is in an early development stage (proof of concept) and has no release yet. Security
fixes are made on the `main` branch. Once releases exist, this section will state which versions
receive security fixes.

## Areas of particular interest

Because Postkeep's archive is the only copy of the user's mail and it processes mail from
strangers, these areas matter most:

- integrity of the archive on the network share (loss or corruption of mail),
- storage of account credentials,
- parsing of untrusted mail content (MIME, headers, attachments),
- display of HTML mail (scripts, external content, tracking).

---

## Auf Deutsch (Kurzfassung)

**Sicherheitslücken bitte nie als öffentliches Issue, Pull Request oder Diskussion melden**, sondern
vertraulich über GitHub: **[Sicherheitslücke melden](https://github.com/chrisschm/postkeep/security/advisories/new)**
(Reiter „Security“ → „Report a vulnerability“). Die Meldung sehen nur du und die Maintainer.

Bitte angeben: was betroffen ist, wie es sich nachvollziehen lässt, welche Auswirkung du erwartest -
ohne echte persönliche Daten. Postkeep wird ehrenamtlich gepflegt; Antworten können etwas dauern.
Die Behebung wird mit dir abgestimmt, danach erscheint ein Sicherheitshinweis (auf Wunsch mit
Nennung). Bis dahin bitte vertraulich behandeln.

Es gibt noch kein Release; Sicherheitskorrekturen erfolgen auf `main`. Besonders wichtig sind die
Unversehrtheit des Archivs, die Speicherung von Zugangsdaten, die Verarbeitung fremder Mails und
die Anzeige von HTML-Mails.
