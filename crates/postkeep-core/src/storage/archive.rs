//! The archive on the share: creating and opening it, storing mails as
//! immutable files, moving them between folders, and cleaning up after
//! crashes.
//!
//! # How a mail is stored
//!
//! Storing must never leave a half-written mail where other computers (or a
//! later step such as deleting the mail at the provider) could mistake it for
//! a complete one. Every mail therefore goes through these steps:
//!
//! 1. Create a new file in `mail/_tmp/` under the mail's final, unique name.
//!    "Create new" fails if the file exists - nothing is ever overwritten.
//! 2. Write the complete content and force it out to the share
//!    (`sync_all`, which on a network share sends an SMB flush).
//! 3. Close the file, open it again and read it back; the content must be
//!    byte-for-byte identical. Otherwise the temporary file is removed and the
//!    mail counts as **not stored**.
//! 4. Make sure the year folder exists (`mail/<folder>/<year>/`).
//! 5. Rename the file into the year folder. On an SMB share a rename is
//!    atomic: other computers see either no file or the complete file. This
//!    was measured on Windows and Linux clients.
//!
//! Only when all steps succeed is the mail stored. If anything fails or the
//! program crashes in between, the only leftover is a file in `mail/_tmp/`,
//! which [`Archive::cleanup_tmp`] removes later.
//!
//! **Known limit of step 3:** the operating system may answer the read-back
//! partly from its own cache instead of the share. The check reliably catches
//! errors while writing (short writes, broken connections), but cannot prove
//! that the bytes are on the NAS's disk. Tests against a real share (see
//! `CONTRIBUTING.md`) cover the rest.
//!
//! # Seeing changes of other computers
//!
//! Measurements showed that Windows may show a *new* file in another
//! computer's folder listing only after up to about 10 seconds, and even
//! report "file not found" for that long. Callers must therefore treat a
//! missing file announced by another computer as "not yet visible" and retry,
//! and must never treat a folder listing as complete or current. Listings are
//! for reconciliation only.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::error::StorageError;
use super::folder::{Folder, StandardFolder};
use super::names::{MachineId, MailId, Timestamp};

/// Name of the identification file in the archive's root folder. Its
/// presence marks a folder as a Postkeep archive.
pub const ARCHIVE_MARKER_FILE: &str = "postkeep-archive.txt";

/// Content of the identification file for the storage format this version
/// writes. A newer format gets a new number; this version then refuses to
/// work on the archive instead of damaging it.
const ARCHIVE_MARKER_CONTENT: &str = "postkeep-archive-format 1";

/// Directory for mails, below the archive root.
const MAIL_DIR: &str = "mail";
/// Staging directory for writes in progress, below `mail/`. Starts with `_`
/// so it can never collide with a folder created by the user.
const TMP_DIR: &str = "_tmp";
/// Directory for the per-computer journals (used from issue "journal" on).
const JOURNAL_DIR: &str = "journal";
/// Directory for short-lived locks (used by POP3 fetching).
const LOCKS_DIR: &str = "locks";

/// A mail stored in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMail {
    /// The mail's unique id (its file name without `.eml`).
    pub id: MailId,
    /// The folder the mail is in.
    pub folder: Folder,
    /// Full path of the mail file.
    pub path: PathBuf,
}

/// The steps of storing a mail, used by tests to simulate a crash after each
/// of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// The temporary file exists but is empty.
    TmpCreated,
    /// The content has been written (maybe still in buffers).
    Written,
    /// The content has been flushed to the share and the file closed.
    Synced,
    /// The read-back comparison succeeded.
    Verified,
    /// The year folder exists.
    TargetFolderReady,
    /// The file has been renamed into place - the mail is stored.
    Renamed,
}

/// An archive on the share, opened by one computer.
#[derive(Debug, Clone)]
pub struct Archive {
    root: PathBuf,
    machine: MachineId,
}

impl Archive {
    /// Creates a new archive in `root`, or opens it if it is already one.
    ///
    /// `root` must not exist yet or be empty. The identification file is
    /// written first, so an interrupted creation can simply be repeated;
    /// [`Archive::open`] then completes the directory structure.
    pub fn create(root: &Path, machine: MachineId) -> Result<Archive, StorageError> {
        let marker = root.join(ARCHIVE_MARKER_FILE);
        if marker.exists() {
            return Archive::open(root, machine);
        }
        fs::create_dir_all(root).map_err(StorageError::io("create archive folder", root))?;
        let mut entries =
            fs::read_dir(root).map_err(StorageError::io("list archive folder", root))?;
        if entries.next().is_some() {
            return Err(StorageError::NotEmpty {
                path: root.to_owned(),
            });
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)
            .map_err(StorageError::io(
                "create archive identification file",
                &marker,
            ))?;
        file.write_all(format!("{ARCHIVE_MARKER_CONTENT}\n").as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(StorageError::io(
                "write archive identification file",
                &marker,
            ))?;
        drop(file);
        Archive::open(root, machine)
    }

    /// Opens an existing archive. Missing standard directories are created,
    /// so archives made by older versions get folders added later.
    pub fn open(root: &Path, machine: MachineId) -> Result<Archive, StorageError> {
        let marker = root.join(ARCHIVE_MARKER_FILE);
        let content = match fs::read_to_string(&marker) {
            Ok(content) => content,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(StorageError::NotAnArchive {
                    path: root.to_owned(),
                });
            }
            Err(e) => {
                return Err(StorageError::io(
                    "read archive identification file",
                    &marker,
                )(e));
            }
        };
        if content.trim() != ARCHIVE_MARKER_CONTENT {
            return Err(StorageError::UnsupportedFormat {
                path: marker,
                found: content.trim().to_owned(),
            });
        }
        let archive = Archive {
            root: root.to_owned(),
            machine,
        };
        archive.ensure_layout()?;
        Ok(archive)
    }

    /// Creates every directory of the archive layout that does not exist yet.
    /// Safe to run on several computers at the same time.
    fn ensure_layout(&self) -> Result<(), StorageError> {
        let mut dirs = vec![
            self.root.join(JOURNAL_DIR),
            self.root.join(LOCKS_DIR),
            self.tmp_dir(),
        ];
        dirs.extend(
            StandardFolder::ALL
                .iter()
                .map(|f| self.folder_dir(&Folder::Standard(*f))),
        );
        for dir in dirs {
            fs::create_dir_all(&dir).map_err(StorageError::io("create folder", &dir))?;
        }
        Ok(())
    }

    /// The archive's root folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The id of the computer this archive was opened by.
    pub fn machine(&self) -> &MachineId {
        &self.machine
    }

    /// `mail/<folder>/`
    fn folder_dir(&self, folder: &Folder) -> PathBuf {
        self.root.join(MAIL_DIR).join(folder.dir_name())
    }

    /// `mail/_tmp/`
    fn tmp_dir(&self) -> PathBuf {
        self.root.join(MAIL_DIR).join(TMP_DIR)
    }

    /// Where a mail with this id lives in `folder`:
    /// `mail/<folder>/<year>/<id>.eml`.
    pub fn mail_path(&self, folder: &Folder, id: &MailId) -> PathBuf {
        self.folder_dir(folder)
            .join(id.year().to_string())
            .join(id.file_name())
    }

    /// Stores a new mail in `folder` and returns where it is.
    ///
    /// When this returns `Ok`, the mail is completely and verifiably on the
    /// share. On `Err` the caller must assume the mail is **not** stored. (In
    /// rare cases - a failure right after the final rename - it may be stored
    /// anyway; storing it again then creates a duplicate, which is harmless,
    /// whereas losing a mail is not.)
    pub fn store_new(&self, folder: &Folder, content: &[u8]) -> Result<StoredMail, StorageError> {
        self.store_new_with(folder, content, &mut |_, _| Ok(()))
    }

    /// [`Archive::store_new`] with a hook that is called after each [`Step`].
    /// Tests use it to simulate a crash (the hook returns an error) or a
    /// damaged write (the hook changes the temporary file).
    pub(crate) fn store_new_with(
        &self,
        folder: &Folder,
        content: &[u8],
        hook: &mut dyn FnMut(Step, &Path) -> io::Result<()>,
    ) -> Result<StoredMail, StorageError> {
        let id = MailId::new(Timestamp::now(), &self.machine);
        let tmp = self.tmp_dir().join(id.file_name());
        let target = self.mail_path(folder, &id);

        let result = self.write_verify_rename(content, &tmp, &target, hook);
        if result.is_err() {
            // Best effort: remove what is left in _tmp/. If this fails too
            // (e.g. the share is gone), cleanup_tmp removes it later.
            let _ = fs::remove_file(&tmp);
        }
        result.map(|()| StoredMail {
            id,
            folder: folder.clone(),
            path: target,
        })
    }

    /// Steps 1-5 of storing a mail, see the module documentation.
    fn write_verify_rename(
        &self,
        content: &[u8],
        tmp: &Path,
        target: &Path,
        hook: &mut dyn FnMut(Step, &Path) -> io::Result<()>,
    ) -> Result<(), StorageError> {
        let crash =
            |step: Step| StorageError::io("simulated failure", PathBuf::from(format!("{step:?}")));

        // 1. Create the temporary file; fails if it exists.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(tmp)
            .map_err(StorageError::io("create temporary file", tmp))?;
        hook(Step::TmpCreated, tmp).map_err(crash(Step::TmpCreated))?;

        // 2. Write everything and flush it to the share.
        file.write_all(content)
            .map_err(StorageError::io("write temporary file", tmp))?;
        hook(Step::Written, tmp).map_err(crash(Step::Written))?;
        file.sync_all()
            .map_err(StorageError::io("flush temporary file", tmp))?;
        drop(file); // close before reading back
        hook(Step::Synced, tmp).map_err(crash(Step::Synced))?;

        // 3. Read back through a new handle and compare.
        let mut read_back = Vec::with_capacity(content.len());
        fs::File::open(tmp)
            .and_then(|mut f| f.read_to_end(&mut read_back))
            .map_err(StorageError::io("read back temporary file", tmp))?;
        if read_back != content {
            return Err(StorageError::ReadBackMismatch {
                path: tmp.to_owned(),
            });
        }
        hook(Step::Verified, tmp).map_err(crash(Step::Verified))?;

        // 4. Year folder. Several computers may create it at the same time;
        //    create_dir_all treats "already exists" as success.
        let target_dir = target.parent().expect("mail path has a parent folder");
        fs::create_dir_all(target_dir)
            .map_err(StorageError::io("create year folder", target_dir))?;
        hook(Step::TargetFolderReady, tmp).map_err(crash(Step::TargetFolderReady))?;

        // 5. Move into place.
        rename_no_overwrite(tmp, target)?;
        hook(Step::Renamed, target).map_err(crash(Step::Renamed))?;
        Ok(())
    }

    /// Moves a mail into another folder by renaming it. The mail's id and
    /// year folder stay the same.
    ///
    /// Fails with [`StorageError::MailNotFound`] if the mail is not (or no
    /// longer) in `mail.folder` - typically because another computer moved it
    /// first. "The first rename wins" is the intended behaviour when two
    /// computers move the same mail at the same time.
    pub fn move_mail(&self, mail: &StoredMail, to: &Folder) -> Result<StoredMail, StorageError> {
        let from = self.mail_path(&mail.folder, &mail.id);
        let target = self.mail_path(to, &mail.id);
        if !from.exists() {
            return Err(StorageError::MailNotFound { path: from });
        }
        let target_dir = target.parent().expect("mail path has a parent folder");
        fs::create_dir_all(target_dir)
            .map_err(StorageError::io("create year folder", target_dir))?;
        rename_no_overwrite(&from, &target).map_err(|e| match e {
            // The source vanished between the check and the rename.
            StorageError::Io { source, .. } if source.kind() == io::ErrorKind::NotFound => {
                StorageError::MailNotFound { path: from.clone() }
            }
            other => other,
        })?;
        Ok(StoredMail {
            id: mail.id.clone(),
            folder: to.clone(),
            path: target,
        })
    }

    /// The ids of all mails in `folder`, sorted.
    ///
    /// **For reconciliation only:** on Windows a listing can lag up to about
    /// 10 seconds behind changes made by other computers, so it must never
    /// be treated as complete or current. Files that are not mails are
    /// ignored.
    pub fn list_folder(&self, folder: &Folder) -> Result<Vec<MailId>, StorageError> {
        let dir = self.folder_dir(folder);
        let mut ids = Vec::new();
        for year in fs::read_dir(&dir).map_err(StorageError::io("list folder", &dir))? {
            let year = year.map_err(StorageError::io("list folder", &dir))?;
            if !year.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let year_dir = year.path();
            for entry in
                fs::read_dir(&year_dir).map_err(StorageError::io("list folder", &year_dir))?
            {
                let entry = entry.map_err(StorageError::io("list folder", &year_dir))?;
                if let Some(id) = entry.file_name().to_str().and_then(MailId::from_file_name) {
                    ids.push(id);
                }
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// Removes leftovers of interrupted writes from `mail/_tmp/`: files that
    /// **this computer** created (recognised by the computer id in the name)
    /// and that are older than `older_than`. Returns how many were removed.
    ///
    /// Files of other computers are never touched - another computer may be
    /// writing them right now. The age limit protects writes of this computer
    /// that are still in progress (e.g. in another program instance).
    pub fn cleanup_tmp(&self, older_than: Duration) -> Result<usize, StorageError> {
        let dir = self.tmp_dir();
        let now = SystemTime::now();
        let mut removed = 0;
        for entry in fs::read_dir(&dir).map_err(StorageError::io("list folder", &dir))? {
            let entry = entry.map_err(StorageError::io("list folder", &dir))?;
            let Some(id) = entry.file_name().to_str().and_then(MailId::from_file_name) else {
                continue; // not a file written by Postkeep - leave it alone
            };
            if id.machine() != self.machine {
                continue;
            }
            let path = entry.path();
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .map_err(StorageError::io("read file time", &path))?;
            // A modification time in the future (clock differences between
            // computer and NAS) counts as "new" and is kept for now.
            let age = now.duration_since(modified).unwrap_or(Duration::ZERO);
            if age >= older_than {
                match fs::remove_file(&path) {
                    Ok(()) => removed += 1,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(e) => return Err(StorageError::io("remove temporary file", &path)(e)),
                }
            }
        }
        Ok(removed)
    }
}

/// Renames `from` to `to`, refusing to overwrite an existing `to`.
///
/// The standard library's rename overwrites existing files on Windows and
/// Linux. Postkeep never overwrites (safety rule 3), so this checks first.
/// Between check and rename another computer could in theory create `to`;
/// that cannot happen in practice because every file name contains the
/// computer id and a random part, and only the computer that created a mail
/// renames it into a new place under the same unique name.
fn rename_no_overwrite(from: &Path, to: &Path) -> Result<(), StorageError> {
    match fs::symlink_metadata(to) {
        Ok(_) => {
            return Err(StorageError::TargetExists {
                path: to.to_owned(),
            });
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(StorageError::io("check target file", to)(e)),
    }
    fs::rename(from, to).map_err(StorageError::io("rename file", from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::TempDir;

    const MAIL: &[u8] =
        b"From: alice@example.com\r\nTo: bob@example.com\r\nSubject: Test\r\n\r\nHello\r\n";

    fn machine(name: &str) -> MachineId {
        MachineId::parse(name).unwrap()
    }

    fn new_archive(label: &str) -> (TempDir, Archive) {
        let dir = TempDir::new(label);
        let archive = Archive::create(&dir.path().join("archive"), machine("pc-1a2b")).unwrap();
        (dir, archive)
    }

    fn files_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn create_builds_the_layout() {
        let (_dir, archive) = new_archive("create");
        let root = archive.root();
        assert_eq!(
            fs::read_to_string(root.join(ARCHIVE_MARKER_FILE))
                .unwrap()
                .trim(),
            ARCHIVE_MARKER_CONTENT
        );
        for sub in [
            "journal",
            "locks",
            "mail/_tmp",
            "mail/inbox",
            "mail/sent",
            "mail/drafts",
            "mail/trash",
            "mail/junk",
            "mail/archive",
        ] {
            assert!(root.join(sub).is_dir(), "{sub} missing");
        }
    }

    #[test]
    fn create_on_an_existing_archive_opens_it() {
        let (_dir, archive) = new_archive("create-twice");
        assert!(Archive::create(archive.root(), machine("pc-1a2b")).is_ok());
    }

    #[test]
    fn create_refuses_a_foreign_non_empty_folder() {
        let dir = TempDir::new("create-foreign");
        fs::write(dir.path().join("something.txt"), "x").unwrap();
        let err = Archive::create(dir.path(), machine("pc-1a2b")).unwrap_err();
        assert!(matches!(err, StorageError::NotEmpty { .. }), "{err}");
    }

    #[test]
    fn open_refuses_a_folder_without_identification_file() {
        let dir = TempDir::new("open-foreign");
        let err = Archive::open(dir.path(), machine("pc-1a2b")).unwrap_err();
        assert!(matches!(err, StorageError::NotAnArchive { .. }), "{err}");
    }

    #[test]
    fn open_refuses_an_unknown_format() {
        let (_dir, archive) = new_archive("open-format");
        fs::write(
            archive.root().join(ARCHIVE_MARKER_FILE),
            "postkeep-archive-format 99\n",
        )
        .unwrap();
        let err = Archive::open(archive.root(), machine("pc-1a2b")).unwrap_err();
        assert!(
            matches!(err, StorageError::UnsupportedFormat { .. }),
            "{err}"
        );
    }

    #[test]
    fn open_completes_an_interrupted_creation() {
        let dir = TempDir::new("open-complete");
        let root = dir.path().join("archive");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(ARCHIVE_MARKER_FILE),
            format!("{ARCHIVE_MARKER_CONTENT}\n"),
        )
        .unwrap();
        let archive = Archive::open(&root, machine("pc-1a2b")).unwrap();
        assert!(archive.root().join("mail/inbox").is_dir());
    }

    #[test]
    fn stored_mail_is_complete_in_its_year_folder() {
        let (_dir, archive) = new_archive("store");
        let mail = archive.store_new(&Folder::INBOX, MAIL).unwrap();
        assert_eq!(fs::read(&mail.path).unwrap(), MAIL);
        assert_eq!(
            mail.path.parent().unwrap(),
            archive
                .root()
                .join("mail/inbox")
                .join(mail.id.year().to_string())
        );
        assert!(mail.id.as_str().contains("-pc-1a2b-"));
        assert!(
            files_in(&archive.tmp_dir()).is_empty(),
            "_tmp/ must be empty"
        );
        assert_eq!(archive.list_folder(&Folder::INBOX).unwrap(), vec![mail.id]);
    }

    #[test]
    fn many_mails_get_distinct_files() {
        let (_dir, archive) = new_archive("store-many");
        for _ in 0..200 {
            archive.store_new(&Folder::INBOX, MAIL).unwrap();
        }
        assert_eq!(archive.list_folder(&Folder::INBOX).unwrap().len(), 200);
    }

    #[test]
    fn empty_and_large_mails_are_stored_exactly() {
        let (_dir, archive) = new_archive("store-sizes");
        let large: Vec<u8> = (0..5_000_000u32).map(|i| (i % 251) as u8).collect();
        for content in [&b""[..], &large[..]] {
            let mail = archive.store_new(&Folder::INBOX, content).unwrap();
            assert_eq!(fs::read(&mail.path).unwrap(), content);
        }
    }

    /// The central safety property: whatever step fails, the target folder
    /// never contains the mail unless storing succeeded, and nothing is
    /// left behind that cleanup cannot remove.
    #[test]
    fn a_failure_before_the_rename_never_leaves_a_mail_in_the_folder() {
        for fail_at in [
            Step::TmpCreated,
            Step::Written,
            Step::Synced,
            Step::Verified,
            Step::TargetFolderReady,
        ] {
            let (_dir, archive) = new_archive("crash");
            let result = archive.store_new_with(&Folder::INBOX, MAIL, &mut |step, _| {
                if step == fail_at {
                    Err(io::Error::other("simulated crash"))
                } else {
                    Ok(())
                }
            });
            assert!(result.is_err(), "{fail_at:?}: must report failure");
            assert!(
                archive.list_folder(&Folder::INBOX).unwrap().is_empty(),
                "{fail_at:?}: mail must not be in the folder"
            );
            archive.cleanup_tmp(Duration::ZERO).unwrap();
            assert!(
                files_in(&archive.tmp_dir()).is_empty(),
                "{fail_at:?}: _tmp/ must be clean"
            );
        }
    }

    #[test]
    fn a_failure_after_the_rename_leaves_a_complete_mail() {
        let (_dir, archive) = new_archive("crash-after");
        let result = archive.store_new_with(&Folder::INBOX, MAIL, &mut |step, _| {
            if step == Step::Renamed {
                Err(io::Error::other("simulated crash"))
            } else {
                Ok(())
            }
        });
        // Reported as failure (the caller must not rely on it) ...
        assert!(result.is_err());
        // ... but what is in the folder is complete - never a partial mail.
        let ids = archive.list_folder(&Folder::INBOX).unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(
            fs::read(archive.mail_path(&Folder::INBOX, &ids[0])).unwrap(),
            MAIL
        );
    }

    #[test]
    fn a_damaged_write_is_detected_by_reading_back() {
        let (_dir, archive) = new_archive("damaged");
        let result = archive.store_new_with(&Folder::INBOX, MAIL, &mut |step, path| {
            if step == Step::Synced {
                // Simulate a write that went wrong on the way to the share.
                fs::write(path, &MAIL[..MAIL.len() - 3])?;
            }
            Ok(())
        });
        let err = result.unwrap_err();
        assert!(
            matches!(err, StorageError::ReadBackMismatch { .. }),
            "{err}"
        );
        assert!(archive.list_folder(&Folder::INBOX).unwrap().is_empty());
        assert!(
            files_in(&archive.tmp_dir()).is_empty(),
            "damaged temporary file must be removed"
        );
    }

    #[test]
    fn moving_renames_the_mail_into_the_other_folder() {
        let (_dir, archive) = new_archive("move");
        let mail = archive.store_new(&Folder::INBOX, MAIL).unwrap();
        let moved = archive.move_mail(&mail, &Folder::TRASH).unwrap();
        assert_eq!(moved.id, mail.id);
        assert!(!mail.path.exists());
        assert_eq!(fs::read(&moved.path).unwrap(), MAIL);
        assert!(archive.list_folder(&Folder::INBOX).unwrap().is_empty());
        assert_eq!(
            archive.list_folder(&Folder::TRASH).unwrap(),
            vec![mail.id.clone()]
        );

        // A second move from the old place: the mail is gone ("first wins").
        let err = archive
            .move_mail(&mail, &Folder::own("Rechnungen").unwrap())
            .unwrap_err();
        assert!(matches!(err, StorageError::MailNotFound { .. }), "{err}");
    }

    #[test]
    fn moving_never_overwrites() {
        let (_dir, archive) = new_archive("move-exists");
        let mail = archive.store_new(&Folder::INBOX, MAIL).unwrap();
        let blocker = archive.mail_path(&Folder::TRASH, &mail.id);
        fs::create_dir_all(blocker.parent().unwrap()).unwrap();
        fs::write(&blocker, b"other").unwrap();
        let err = archive.move_mail(&mail, &Folder::TRASH).unwrap_err();
        assert!(matches!(err, StorageError::TargetExists { .. }), "{err}");
        assert_eq!(fs::read(&mail.path).unwrap(), MAIL, "source untouched");
        assert_eq!(fs::read(&blocker).unwrap(), b"other", "target untouched");
    }

    #[test]
    fn cleanup_only_removes_own_old_files() {
        let (_dir, archive) = new_archive("cleanup");
        let tmp = archive.tmp_dir();
        let own = MailId::new(Timestamp::now(), archive.machine());
        let other = MailId::new(Timestamp::now(), &machine("other-9f9f"));
        fs::write(tmp.join(own.file_name()), b"x").unwrap();
        fs::write(tmp.join(other.file_name()), b"x").unwrap();
        fs::write(tmp.join("unrelated.txt"), b"x").unwrap();

        // Too young: kept.
        assert_eq!(archive.cleanup_tmp(Duration::from_secs(3600)).unwrap(), 0);
        // Old enough: only our own file goes.
        assert_eq!(archive.cleanup_tmp(Duration::ZERO).unwrap(), 1);
        let mut expected = vec![other.file_name(), "unrelated.txt".to_owned()];
        expected.sort();
        assert_eq!(files_in(&tmp), expected);
    }

    #[test]
    fn listing_ignores_foreign_files() {
        let (_dir, archive) = new_archive("list-foreign");
        let mail = archive.store_new(&Folder::INBOX, MAIL).unwrap();
        let year_dir = mail.path.parent().unwrap();
        fs::write(year_dir.join("Thumbs.db"), b"x").unwrap();
        fs::write(archive.root().join("mail/inbox/desktop.ini"), b"x").unwrap();
        assert_eq!(archive.list_folder(&Folder::INBOX).unwrap(), vec![mail.id]);
    }
}
