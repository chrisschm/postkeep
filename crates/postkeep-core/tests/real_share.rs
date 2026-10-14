//! Tests against a real network share (SMB).
//!
//! These tests are **not** run by `cargo test` or in CI, because they need a
//! share. Run them by hand, on Windows and on Linux:
//!
//! ```text
//! # Windows (PowerShell)
//! $env:POSTKEEP_TEST_SHARE = '\\nas\postkeep-test'
//! cargo test -p postkeep-core --test real_share -- --ignored
//!
//! # Linux (share mounted with cifs, e.g. at /mnt/postkeep-test)
//! POSTKEEP_TEST_SHARE=/mnt/postkeep-test cargo test -p postkeep-core --test real_share -- --ignored
//! ```
//!
//! Use a folder that is meant for tests only - never the real archive. Each
//! run works in a new subfolder and removes it at the end.
//!
//! The test first checks that the folder really is on a network share. On
//! Linux an unmounted mount point is just an empty local folder, and a test
//! running there would pass without ever touching SMB - so it refuses to run
//! instead.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use postkeep_core::storage::{Archive, Folder, MachineId, StorageError};

/// The test share from the environment, or `None` (test is then skipped with
/// a message, so a forgotten variable is visible).
fn test_share() -> Option<PathBuf> {
    match std::env::var_os("POSTKEEP_TEST_SHARE") {
        Some(path) => Some(PathBuf::from(path)),
        None => {
            eprintln!("POSTKEEP_TEST_SHARE is not set - skipping the real-share test");
            None
        }
    }
}

/// Panics unless `path` (a folder below `share`) is on a network share.
fn assert_on_network_share(share: &Path, path: &Path) {
    if cfg!(windows) {
        let text = path.to_string_lossy();
        assert!(
            text.starts_with(r"\\") || text.starts_with("//"),
            "{text} is not a UNC path (\\\\server\\share\\...). Use the UNC path, not a drive \
             letter, so the test cannot accidentally run on a local disk."
        );
    } else {
        // Find the mount that contains `path` (longest matching mount point)
        // and check its file system type.
        let canonical = fs::canonicalize(path).expect("test share path exists");
        let mounts = fs::read_to_string("/proc/self/mounts").expect("read /proc/self/mounts");
        let (mount_point, fs_type) = mounts
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let _source = fields.next()?;
                let mount_point = fields.next()?.replace("\\040", " ");
                let fs_type = fields.next()?.to_owned();
                Some((mount_point, fs_type))
            })
            .filter(|(mp, _)| canonical.starts_with(mp))
            .max_by_key(|(mp, _)| mp.len())
            .expect("every path is below some mount");
        assert!(
            fs_type == "cifs" || fs_type == "smb3",
            "{} is on a {fs_type} file system (mount point {mount_point}), not on an SMB share. \
             Is the share mounted at {}? (e.g. `sudo mount {}`)",
            canonical.display(),
            share.display(),
            share.display()
        );
    }
}

/// Removes the test folder at the end, also when the test fails.
struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "needs a real SMB share, see the module documentation"]
fn store_move_and_cleanup_on_a_real_share() {
    let Some(share) = test_share() else { return };
    let machine = MachineId::generate(&format!("test-{}", postkeep_core::storage::host_name()));
    let root = share.join(format!("real-share-test-{}", machine));
    // Windows: the path itself tells whether it is a share, so check before
    // anything is created.
    if cfg!(windows) {
        assert_on_network_share(&share, &share);
    }
    fs::create_dir_all(&root).expect("create test folder on the share");
    let _cleanup = Cleanup(root.clone());
    // Linux: check after creating the folder - an automount (if any) has then
    // been triggered, so /proc/self/mounts shows the share. A wrongly created
    // local folder is removed again by `_cleanup`.
    assert_on_network_share(&share, &root);

    let archive = Archive::create(&root, machine.clone()).expect("create archive on the share");

    // Mails of different sizes, including empty and several megabytes.
    let sizes = [0usize, 1, 100, 4096, 65_537, 1_000_000, 5_000_000];
    let mut stored = Vec::new();
    for (n, size) in sizes.iter().enumerate() {
        let content: Vec<u8> = (0..*size).map(|i| ((i + n) % 251) as u8).collect();
        let mail = archive
            .store_new(&Folder::INBOX, &content)
            .expect("store mail on the share");
        assert_eq!(fs::read(&mail.path).expect("read stored mail"), content);
        stored.push((mail, content));
    }
    assert_eq!(
        archive.list_folder(&Folder::INBOX).unwrap().len(),
        sizes.len()
    );

    // Move every second mail to the trash and check it arrived intact.
    for (mail, content) in stored.iter().step_by(2) {
        let moved = archive
            .move_mail(mail, &Folder::TRASH)
            .expect("move on the share");
        assert_eq!(fs::read(&moved.path).unwrap(), *content);
        let again = archive.move_mail(mail, &Folder::TRASH);
        assert!(
            matches!(again, Err(StorageError::MailNotFound { .. })),
            "second move must fail: {again:?}"
        );
    }
    assert_eq!(
        archive.list_folder(&Folder::TRASH).unwrap().len(),
        sizes.len().div_ceil(2)
    );

    // Nothing may be left in _tmp/, and cleanup must find nothing to do.
    assert_eq!(archive.cleanup_tmp(Duration::ZERO).unwrap(), 0);
}
