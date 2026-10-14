//! Errors of the storage layer.
//!
//! Every error names the file or folder it concerns, because on a network
//! share "something went wrong" is useless for finding the cause. The texts
//! produced by [`std::fmt::Display`] are English and meant for logs and
//! developers; user interfaces translate the error *kind* into a message of
//! their own (see the `i18n` module).

use std::fmt;
use std::io;
use std::path::PathBuf;

use super::folder::FolderNameError;

/// Everything that can go wrong in the storage layer.
#[derive(Debug)]
#[non_exhaustive]
pub enum StorageError {
    /// An operating system call on a file or folder failed (network share
    /// gone, no permission, disk full, ...).
    Io {
        /// What Postkeep was trying to do, e.g. "create temporary file".
        action: &'static str,
        /// The file or folder concerned.
        path: PathBuf,
        /// The error reported by the operating system.
        source: io::Error,
    },
    /// The folder is not a Postkeep archive (the identification file
    /// `postkeep-archive.txt` is missing).
    NotAnArchive {
        /// The folder that was expected to be an archive.
        path: PathBuf,
    },
    /// A new archive can only be created in an empty folder, but this one
    /// already contains files.
    NotEmpty {
        /// The folder that is not empty.
        path: PathBuf,
    },
    /// The archive was created by a newer version of Postkeep with a storage
    /// format this version does not know.
    UnsupportedFormat {
        /// The identification file of the archive.
        path: PathBuf,
        /// What the identification file contains.
        found: String,
    },
    /// After writing a mail, reading it back did not return exactly the same
    /// bytes. The mail was **not** stored; the caller must not treat it as
    /// saved (and must not delete it at the provider).
    ReadBackMismatch {
        /// The temporary file that was compared.
        path: PathBuf,
    },
    /// The target file already exists. Postkeep never overwrites a file on
    /// the share (safety rule 3), so the operation was not carried out.
    TargetExists {
        /// The file that already exists.
        path: PathBuf,
    },
    /// The mail to move does not exist (any more) in the source folder -
    /// usually because another computer has just moved or deleted it.
    MailNotFound {
        /// Where the mail was expected.
        path: PathBuf,
    },
    /// The name is not allowed for a folder created by the user.
    InvalidFolderName {
        /// The rejected name.
        name: String,
        /// Why it was rejected.
        reason: FolderNameError,
    },
    /// The stored computer id (see `MachineId`) is not in the expected form.
    /// It is never replaced silently, because the computer's journal and its
    /// files on the share are tied to it.
    InvalidMachineId {
        /// The file the id was read from.
        path: PathBuf,
        /// What the file contains.
        found: String,
    },
}

impl StorageError {
    /// Shorthand for building an [`StorageError::Io`] in `map_err` calls:
    /// `.map_err(StorageError::io("read file", &path))`.
    pub(crate) fn io(
        action: &'static str,
        path: impl Into<PathBuf>,
    ) -> impl FnOnce(io::Error) -> Self {
        let path = path.into();
        move |source| StorageError::Io {
            action,
            path,
            source,
        }
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Io {
                action,
                path,
                source,
            } => write!(f, "{action} failed for {}: {source}", path.display()),
            StorageError::NotAnArchive { path } => {
                write!(f, "{} is not a Postkeep archive", path.display())
            }
            StorageError::NotEmpty { path } => write!(
                f,
                "{} is not empty; a new archive needs an empty folder",
                path.display()
            ),
            StorageError::UnsupportedFormat { path, found } => write!(
                f,
                "{} has an unsupported archive format: {found:?}",
                path.display()
            ),
            StorageError::ReadBackMismatch { path } => write!(
                f,
                "reading back {} did not return the written content",
                path.display()
            ),
            StorageError::TargetExists { path } => {
                write!(
                    f,
                    "{} already exists and is never overwritten",
                    path.display()
                )
            }
            StorageError::MailNotFound { path } => write!(f, "{} does not exist", path.display()),
            StorageError::InvalidFolderName { name, reason } => {
                write!(f, "invalid folder name {name:?}: {reason}")
            }
            StorageError::InvalidMachineId { path, found } => write!(
                f,
                "{} does not contain a valid computer id: {found:?}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StorageError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
