//! Mail folders: the standard folders with fixed names, and folders created
//! by the user.
//!
//! On the share every folder is a directory below `mail/`. The names of the
//! **standard folders** are fixed, lowercase English identifiers (`inbox`,
//! `trash`, ...). They are never translated on the share - only the user
//! interface shows a translated name - because computers with different
//! interface languages work on the same archive.
//!
//! Folders **created by the user** keep the name the user typed, as long as
//! it is a safe directory name on both Windows and Linux (see
//! [`Folder::own`]). Two rules keep them apart from Postkeep's own names:
//!
//! - the standard names are reserved regardless of upper/lower case, because
//!   SMB shares do not distinguish `Inbox` from `inbox`;
//! - names starting with `_` are reserved for internal directories such as
//!   `mail/_tmp/`.

use std::fmt;

use super::error::StorageError;

/// The standard folders. Their directory names are part of the storage format
/// and must never change once released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardFolder {
    /// Incoming mail (`inbox`).
    Inbox,
    /// Sent mail (`sent`). Used from stage 1 on.
    Sent,
    /// Drafts (`drafts`). Used from stage 1 on.
    Drafts,
    /// Deleted mail, kept for a configurable period (`trash`).
    Trash,
    /// Spam (`junk`). Used in a later stage.
    Junk,
    /// Archived mail (`archive`). Used in a later stage.
    Archive,
}

impl StandardFolder {
    /// All standard folders. A new archive gets a directory for each of them,
    /// including those not used yet, so that later versions find them.
    pub const ALL: [StandardFolder; 6] = [
        StandardFolder::Inbox,
        StandardFolder::Sent,
        StandardFolder::Drafts,
        StandardFolder::Trash,
        StandardFolder::Junk,
        StandardFolder::Archive,
    ];

    /// The directory name on the share. The names follow common IMAP and
    /// Thunderbird usage, which makes matching IMAP folders and imports
    /// straightforward later.
    pub fn dir_name(self) -> &'static str {
        match self {
            StandardFolder::Inbox => "inbox",
            StandardFolder::Sent => "sent",
            StandardFolder::Drafts => "drafts",
            StandardFolder::Trash => "trash",
            StandardFolder::Junk => "junk",
            StandardFolder::Archive => "archive",
        }
    }
}

/// A mail folder: a standard folder or one created by the user.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Folder {
    /// One of the standard folders.
    Standard(StandardFolder),
    /// A folder created by the user; the name has been checked by
    /// [`Folder::own`].
    Own(String),
}

impl Folder {
    /// Shorthand for the inbox.
    pub const INBOX: Folder = Folder::Standard(StandardFolder::Inbox);
    /// Shorthand for the trash.
    pub const TRASH: Folder = Folder::Standard(StandardFolder::Trash);

    /// A folder created by the user, after checking that `name` is usable as
    /// a directory name on Windows, Linux and SMB shares and does not collide
    /// with Postkeep's own names.
    ///
    /// Only single folders directly below `mail/` are supported so far
    /// (no subfolders).
    pub fn own(name: &str) -> Result<Folder, StorageError> {
        check_own_name(name)
            .map(|()| Folder::Own(name.to_owned()))
            .map_err(|reason| StorageError::InvalidFolderName {
                name: name.to_owned(),
                reason,
            })
    }

    /// The directory name below `mail/`.
    pub fn dir_name(&self) -> &str {
        match self {
            Folder::Standard(f) => f.dir_name(),
            Folder::Own(name) => name,
        }
    }
}

/// Why a name was rejected for a folder created by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FolderNameError {
    /// The name is empty (or only spaces).
    Empty,
    /// The name is longer than 100 characters.
    TooLong,
    /// The name is (apart from case) one of the standard folder names.
    Reserved,
    /// The name starts with `_`, which is reserved for internal directories.
    StartsWithUnderscore,
    /// The name contains a character Windows does not allow in file names
    /// (`< > : " / \ | ? *`), a control character, or is `.`/`..`.
    InvalidCharacter,
    /// The name ends with a dot or a space, which Windows silently removes -
    /// the folder would then have a different name on Windows than on Linux.
    TrailingDotOrSpace,
    /// The name is a device name reserved by Windows (`CON`, `NUL`, `COM1`,
    /// ...), which cannot be used as a directory name there.
    WindowsDeviceName,
}

impl fmt::Display for FolderNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FolderNameError::Empty => "the name is empty",
            FolderNameError::TooLong => "the name is longer than 100 characters",
            FolderNameError::Reserved => "the name is reserved for a standard folder",
            FolderNameError::StartsWithUnderscore => "names starting with _ are reserved",
            FolderNameError::InvalidCharacter => {
                "the name contains a character that is not allowed"
            }
            FolderNameError::TrailingDotOrSpace => "the name ends with a dot or a space",
            FolderNameError::WindowsDeviceName => "the name is reserved by Windows",
        })
    }
}

/// Maximum length of a user folder name in characters. Well below the
/// 255-byte limit of most file systems, leaving room for multi-byte UTF-8.
const OWN_NAME_MAX_CHARS: usize = 100;

fn check_own_name(name: &str) -> Result<(), FolderNameError> {
    if name.trim().is_empty() {
        return Err(FolderNameError::Empty);
    }
    if name.chars().count() > OWN_NAME_MAX_CHARS {
        return Err(FolderNameError::TooLong);
    }
    if StandardFolder::ALL
        .iter()
        .any(|f| f.dir_name().eq_ignore_ascii_case(name))
    {
        return Err(FolderNameError::Reserved);
    }
    if name.starts_with('_') {
        return Err(FolderNameError::StartsWithUnderscore);
    }
    if name == "."
        || name == ".."
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        })
    {
        return Err(FolderNameError::InvalidCharacter);
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(FolderNameError::TrailingDotOrSpace);
    }
    // Windows reserves these names, also with any extension ("NUL.txt").
    let base = name.split('.').next().unwrap_or(name).trim_end();
    let upper = base.to_ascii_uppercase();
    let is_device = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes()[3].is_ascii_digit()
            && upper.as_bytes()[3] != b'0');
    if is_device {
        return Err(FolderNameError::WindowsDeviceName);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reason(name: &str) -> FolderNameError {
        match Folder::own(name) {
            Err(StorageError::InvalidFolderName { reason, .. }) => reason,
            other => panic!("{name:?} should be rejected, got {other:?}"),
        }
    }

    #[test]
    fn standard_folder_names_are_fixed() {
        let names: Vec<_> = StandardFolder::ALL.iter().map(|f| f.dir_name()).collect();
        assert_eq!(
            names,
            ["inbox", "sent", "drafts", "trash", "junk", "archive"]
        );
    }

    #[test]
    fn ordinary_names_are_accepted() {
        for name in [
            "Rechnungen",
            "Steuer 2025",
            "Kunden - Müller & Söhne",
            "日本語",
            "a.b",
        ] {
            assert_eq!(Folder::own(name).unwrap().dir_name(), name);
        }
    }

    #[test]
    fn reserved_names_are_rejected_in_any_case() {
        assert_eq!(reason("inbox"), FolderNameError::Reserved);
        assert_eq!(reason("Inbox"), FolderNameError::Reserved);
        assert_eq!(reason("TRASH"), FolderNameError::Reserved);
    }

    #[test]
    fn problematic_names_are_rejected() {
        assert_eq!(reason(""), FolderNameError::Empty);
        assert_eq!(reason("   "), FolderNameError::Empty);
        assert_eq!(reason(&"x".repeat(101)), FolderNameError::TooLong);
        assert_eq!(reason("_tmp"), FolderNameError::StartsWithUnderscore);
        assert_eq!(reason("a/b"), FolderNameError::InvalidCharacter);
        assert_eq!(reason("a\\b"), FolderNameError::InvalidCharacter);
        assert_eq!(reason("what?"), FolderNameError::InvalidCharacter);
        assert_eq!(reason("tab\there"), FolderNameError::InvalidCharacter);
        assert_eq!(reason(".."), FolderNameError::InvalidCharacter);
        assert_eq!(reason("name."), FolderNameError::TrailingDotOrSpace);
        assert_eq!(reason("name "), FolderNameError::TrailingDotOrSpace);
        assert_eq!(reason("NUL"), FolderNameError::WindowsDeviceName);
        assert_eq!(reason("com1"), FolderNameError::WindowsDeviceName);
        assert_eq!(reason("Lpt9.old"), FolderNameError::WindowsDeviceName);
    }

    #[test]
    fn names_resembling_device_names_are_fine() {
        for name in ["CONTRACTS", "COM0", "COMX", "Console"] {
            assert!(Folder::own(name).is_ok(), "{name}");
        }
    }
}
