//! Storage layer: the archive on the network share.
//!
//! The archive is a folder on an SMB share that several computers use at the
//! same time. Every mail is one immutable `.eml` file; folders are
//! directories. The layout:
//!
//! ```text
//! <archive>/
//!   postkeep-archive.txt   identifies the folder as a Postkeep archive
//!   mail/
//!     inbox/2026/<id>.eml   one file per mail, grouped by the year it was stored
//!     trash/, sent/, ...    other standard folders (fixed names, see Folder)
//!     <own folder>/2026/    folders created by the user
//!     _tmp/                 staging area for writes in progress
//!   journal/                status change logs, one per computer (later)
//!   locks/                  short-lived locks, e.g. for POP3 fetching (later)
//! ```
//!
//! Main types:
//!
//! - [`Archive`] - create/open an archive, store, move and list mails,
//!   clean up after crashes;
//! - [`Folder`] and [`StandardFolder`] - mail folders and their names;
//! - [`MachineId`] and [`MailId`] - the ids that make every file name unique.
//!
//! Why the archive works the way it does (immutable files, write to `_tmp/`
//! then rename, no database on the share) is explained in
//! `docs/ARCHITECTURE.md`. The rules that matter when changing this code are
//! repeated in the documentation of [`archive`].

pub mod archive;
pub mod error;
pub mod folder;
pub mod names;

pub use archive::{ARCHIVE_MARKER_FILE, Archive, StoredMail};
pub use error::StorageError;
pub use folder::{Folder, FolderNameError, StandardFolder};
pub use names::{MachineId, MailId, Timestamp, host_name};

/// Helpers shared by the unit tests of the storage modules.
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};

    /// A fresh, empty folder in the system's temporary directory that is
    /// removed again when the test ends (also when it fails).
    pub(crate) struct TempDir(PathBuf);

    impl TempDir {
        pub(crate) fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "postkeep-test-{label}-{}-{:016x}",
                std::process::id(),
                super::names::random_u64()
            ));
            std::fs::create_dir_all(&path).expect("create temporary test folder");
            TempDir(path)
        }

        pub(crate) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
