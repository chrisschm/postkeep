//! Core library of Postkeep.
//!
//! Postkeep is a mail client whose archive lives on a shared network drive
//! (SMB share) as plain, individual files - one `.eml` file per mail - so that
//! several computers can work on the same archive at the same time without
//! corrupting it.
//!
//! This crate contains everything the user interfaces (command line now,
//! desktop later) build on. It knows nothing about windows, buttons or
//! terminals; it only deals with files, network protocols and data.
//!
//! # Modules
//!
//! - [`i18n`] - translations of all user-visible texts.
//!
//! Planned for the proof of concept (stage 0), in this order:
//!
//! - storage layer: writing mails as immutable files onto the share
//!   (write to a temporary folder, read back and compare, then rename),
//! - POP3 fetching with a safety chain: a mail is deleted at the provider
//!   only after it has verifiably been stored on the share,
//! - journal: per-computer, append-only log of status changes (read, flagged,
//!   answered) that all computers merge into the same view.
//!
//! See `docs/ARCHITECTURE.md` in the repository for the overall design.

// Every public item must be documented: the documentation is generated from
// these comments and is the main entry point for new contributors.
#![warn(missing_docs)]

pub mod i18n;
