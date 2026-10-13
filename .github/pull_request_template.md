## What does this change do?

<!-- Short description. Link the related issue, e.g. "Fixes #123". -->

## How was this tested?

<!-- Operating system(s). If the change touches the archive on the network
share: which device/server and SMB version, and whether two computers were
used at the same time. -->

## Checklist

- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass
- [ ] `cargo deny check` passes (needed only if dependencies changed)
- [ ] New or changed user-visible texts are in **all** translation files under
      `crates/postkeep-core/locales/` (if you don't speak German, put the English
      text into the German file as well and mention it here - a maintainer will
      translate it)
- [ ] `docs/` is updated if the design or behaviour changed, especially `docs/ARCHITECTURE.md`
- [ ] No new dependency - or the reason for it is explained above
- [ ] Tests and examples contain no personal data (real mails, addresses, passwords)
