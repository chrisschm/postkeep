//! Names of files on the share: timestamps, the computer id and mail ids.
//!
//! Every new file gets a globally unique name made of
//!
//! ```text
//! <UTC timestamp>-<computer id>-<random part>.eml
//! 20261014T221530123Z-office-7f3a-9c41e2d0.eml
//! ```
//!
//! - the **timestamp** (UTC, millisecond precision) sorts files by the time
//!   they were stored and also decides the year folder;
//! - the **computer id** makes names from different computers distinct even
//!   if they are created in the same millisecond, and shows which computer
//!   created a file;
//! - the **random part** separates files created by the same computer in the
//!   same millisecond.
//!
//! No two computers can therefore ever create the same file name, which is
//! what allows them to write to the share at the same time without locking
//! (safety rule 3 in `docs/ARCHITECTURE.md`).
//!
//! This module deliberately uses only the standard library: date
//! calculations are a few lines of well-known arithmetic, and the random part
//! only has to be unpredictable enough to avoid collisions, not secure.

use std::fs;
use std::hash::{BuildHasher, Hasher, RandomState};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::error::StorageError;

/// File extension of a stored mail (RFC 5322 message, "eml" format).
pub const MAIL_EXTENSION: &str = "eml";

/// Length of a formatted [`Timestamp`], e.g. `20261014T221530123Z`.
const TIMESTAMP_LEN: usize = 19;

/// Number of hex digits of the random part of a file name.
const RANDOM_HEX_DIGITS: usize = 8;

/// Number of hex digits appended to the host name in a computer id.
const MACHINE_SUFFIX_HEX_DIGITS: usize = 4;

/// Maximum length of the host-name part of a computer id.
const MACHINE_NAME_MAX: usize = 20;

// ---------------------------------------------------------------------------
// Timestamps
// ---------------------------------------------------------------------------

/// A point in time in UTC with millisecond precision, as used in file names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    millisecond: u32,
}

impl Timestamp {
    /// The current time.
    pub fn now() -> Self {
        Self::from_system_time(SystemTime::now())
    }

    /// Converts a system time. Times before 1970 (a badly set clock) are
    /// treated as 1970-01-01 so that file names stay well-formed.
    pub fn from_system_time(time: SystemTime) -> Self {
        let since_epoch = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
        let total_seconds = since_epoch.as_secs();
        let days = (total_seconds / 86_400) as i64;
        let seconds_of_day = (total_seconds % 86_400) as u32;
        let (year, month, day) = civil_from_days(days);
        Timestamp {
            year,
            month,
            day,
            hour: seconds_of_day / 3600,
            minute: seconds_of_day % 3600 / 60,
            second: seconds_of_day % 60,
            millisecond: since_epoch.subsec_millis(),
        }
    }

    /// The calendar year (UTC). Decides the year folder of a mail.
    pub fn year(&self) -> i64 {
        self.year
    }

    /// The compact form used in file names: `YYYYMMDDTHHMMSSmmmZ`.
    pub fn compact(&self) -> String {
        format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}{:03}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second, self.millisecond
        )
    }
}

/// Converts a day count since 1970-01-01 into (year, month, day) of the
/// proleptic Gregorian calendar.
///
/// This is the well-known algorithm by Howard Hinnant
/// (<https://howardhinnant.github.io/date_algorithms.html#civil_from_days>).
/// It works in 400-year "eras" because the Gregorian calendar repeats exactly
/// every 400 years, and it counts years from March so that the leap day is
/// the last day of the (shifted) year.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468; // shift the epoch from 1970-01-01 to 0000-03-01
    let era = z.div_euclid(146_097); // 146 097 days per 400-year era
    let day_of_era = z.rem_euclid(146_097); // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365; // [0, 399]
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // [0, 365]
    let month_index = (5 * day_of_year + 2) / 153; // [0, 11], 0 = March
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32; // [1, 31]
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    } as u32; // [1, 12]
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

// ---------------------------------------------------------------------------
// Random numbers
// ---------------------------------------------------------------------------

/// A random 64-bit number, good enough to keep file names apart.
///
/// The standard library has no random number generator, but every
/// [`RandomState`] is seeded from the operating system's random source (once
/// per thread, then varied for each new instance). Hashing a counter, the
/// current time and the process id with it gives numbers that differ between
/// computers, processes and calls. This is **not** suitable for anything
/// security-related (keys, passwords) - it is only used for unique names.
pub(crate) fn random_u64() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    hasher.write_u128(nanos);
    hasher.write_u32(std::process::id());
    hasher.finish()
}

/// `digits` lowercase hex digits of a fresh random number.
fn random_hex(digits: usize) -> String {
    let hex = format!("{:016x}", random_u64());
    hex[..digits].to_owned()
}

fn is_lower_hex(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

// ---------------------------------------------------------------------------
// Computer id
// ---------------------------------------------------------------------------

/// Identifies one computer working on the archive, e.g. `office-7f3a`.
///
/// It consists of the computer's host name (lowercase, only `a-z`, `0-9`
/// and `-`, at most 20 characters) and four random hex digits. The random
/// part keeps ids unique even if two computers have the same name; storing
/// the id locally keeps it stable when the computer is renamed (decision in
/// the project documentation: the id is created once and then reused).
///
/// The id appears in file names on the share and names the computer's
/// journal, so it must never change once files have been written with it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MachineId(String);

impl MachineId {
    /// Creates a new id for a computer with the given host name.
    pub fn generate(host_name: &str) -> Self {
        MachineId(format!(
            "{}-{}",
            sanitize_host_name(host_name),
            random_hex(MACHINE_SUFFIX_HEX_DIGITS)
        ))
    }

    /// Checks and wraps an existing id. Returns `None` if `value` is not in
    /// the form produced by [`MachineId::generate`].
    pub fn parse(value: &str) -> Option<Self> {
        let (name, suffix) = value.rsplit_once('-')?;
        let name_ok = !name.is_empty()
            && name.len() <= MACHINE_NAME_MAX
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !name.starts_with('-')
            && !name.ends_with('-')
            && !name.contains("--");
        let suffix_ok = suffix.len() == MACHINE_SUFFIX_HEX_DIGITS && is_lower_hex(suffix);
        (name_ok && suffix_ok).then(|| MachineId(value.to_owned()))
    }

    /// Reads the id stored in `file`, or creates a new one and stores it
    /// there if the file does not exist yet.
    ///
    /// `file` must be on the computer's **local** disk (per-computer
    /// settings), never on the share. An existing but malformed file is an
    /// error and is not overwritten.
    pub fn load_or_create(file: &Path, host_name: &str) -> Result<Self, StorageError> {
        match fs::read_to_string(file) {
            Ok(content) => {
                let found = content.trim();
                MachineId::parse(found).ok_or_else(|| StorageError::InvalidMachineId {
                    path: file.to_owned(),
                    found: found.to_owned(),
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let id = MachineId::generate(host_name);
                write_new_small_file(file, &format!("{}\n", id.0))?;
                Ok(id)
            }
            Err(e) => Err(StorageError::io("read computer id", file)(e)),
        }
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MachineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Turns a host name into the name part of a computer id: lowercase ASCII
/// letters and digits, everything else becomes `-`, repeated dashes are
/// merged, at most 20 characters. An unusable name becomes `computer`.
fn sanitize_host_name(host_name: &str) -> String {
    let mut out = String::new();
    for c in host_name.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut out: String = out
        .trim_matches('-')
        .chars()
        .take(MACHINE_NAME_MAX)
        .collect();
    while out.ends_with('-') {
        out.pop(); // truncation may have cut right after a dash
    }
    if out.is_empty() {
        "computer".to_owned()
    } else {
        out
    }
}

/// The name of this computer as far as it can be found out without extra
/// libraries: `COMPUTERNAME` (Windows), `HOSTNAME`, `/etc/hostname` (Linux).
/// Only used to make computer ids readable, so a wrong or missing name is
/// harmless.
pub fn host_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .or_else(|| fs::read_to_string("/etc/hostname").ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "computer".to_owned())
}

/// Writes a small file that must not exist yet: first to `<file>.tmp`, then
/// renamed, so a crash never leaves a half-written file under the real name.
fn write_new_small_file(file: &Path, content: &str) -> Result<(), StorageError> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(StorageError::io("create folder", parent))?;
    }
    let mut tmp_name = file.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    let mut f = fs::File::create(&tmp).map_err(StorageError::io("create file", &tmp))?;
    f.write_all(content.as_bytes())
        .and_then(|()| f.sync_all())
        .map_err(StorageError::io("write file", &tmp))?;
    drop(f);
    fs::rename(&tmp, file).map_err(StorageError::io("rename file", file))
}

// ---------------------------------------------------------------------------
// Mail ids
// ---------------------------------------------------------------------------

/// The unique id of a stored mail: its file name without the `.eml`
/// extension, e.g. `20261014T221530123Z-office-7f3a-9c41e2d0`.
///
/// The id never changes - moving a mail to another folder keeps it, only the
/// folder changes. Journals refer to mails by this id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MailId(String);

impl MailId {
    /// A new, unique id for a mail stored now by this computer.
    pub fn new(time: Timestamp, machine: &MachineId) -> Self {
        MailId(format!(
            "{}-{}-{}",
            time.compact(),
            machine.as_str(),
            random_hex(RANDOM_HEX_DIGITS)
        ))
    }

    /// Recognises a mail file name (`<id>.eml`). Returns `None` for any other
    /// file, which callers then leave alone.
    pub fn from_file_name(file_name: &str) -> Option<Self> {
        let stem = file_name.strip_suffix(&format!(".{MAIL_EXTENSION}"))?;
        let id = MailId(stem.to_owned());
        let valid = id.parts().is_some();
        valid.then_some(id)
    }

    /// Splits the id into timestamp text, computer id and random part, after
    /// checking each of them.
    fn parts(&self) -> Option<(&str, MachineId, &str)> {
        let s = self.0.as_str();
        if !s.is_char_boundary(TIMESTAMP_LEN) {
            return None;
        }
        let (time, rest) = s.split_at(TIMESTAMP_LEN);
        let rest = rest.strip_prefix('-')?;
        let (machine, random) = rest.rsplit_once('-')?;
        let time_ok = time.len() == TIMESTAMP_LEN
            && time.as_bytes()[8] == b'T'
            && time.ends_with('Z')
            && time[..8].bytes().all(|b| b.is_ascii_digit())
            && time[9..18].bytes().all(|b| b.is_ascii_digit());
        let random_ok = random.len() == RANDOM_HEX_DIGITS && is_lower_hex(random);
        let machine = MachineId::parse(machine)?;
        (time_ok && random_ok).then_some((time, machine, random))
    }

    /// The computer that stored the mail.
    pub fn machine(&self) -> MachineId {
        self.parts().expect("MailId is validated on creation").1
    }

    /// The year (UTC) the mail was stored in; decides its year folder.
    pub fn year(&self) -> i64 {
        self.0[..4]
            .parse()
            .expect("MailId is validated on creation")
    }

    /// The file name of the mail: `<id>.eml`.
    pub fn file_name(&self) -> String {
        format!("{}.{MAIL_EXTENSION}", self.0)
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MailId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn at(seconds: u64, millis: u64) -> Timestamp {
        Timestamp::from_system_time(UNIX_EPOCH + Duration::from_millis(seconds * 1000 + millis))
    }

    #[test]
    fn timestamp_formats_known_points_in_time() {
        assert_eq!(at(0, 0).compact(), "19700101T000000000Z");
        // 2026-10-14 22:15:30.123 UTC
        assert_eq!(at(1_792_016_130, 123).compact(), "20261014T221530123Z");
        assert_eq!(at(1_792_016_130, 123).year(), 2026);
    }

    #[test]
    fn timestamp_handles_leap_years_and_year_ends() {
        // 2000-02-29 (leap year divisible by 400)
        assert_eq!(at(951_782_400, 0).compact(), "20000229T000000000Z");
        // 2024-12-31 23:59:59.999 and the next millisecond
        assert_eq!(at(1_735_689_599, 999).compact(), "20241231T235959999Z");
        assert_eq!(at(1_735_689_600, 0).compact(), "20250101T000000000Z");
        // 2100-03-01 (2100 is not a leap year, so no 29 February)
        assert_eq!(at(4_107_542_400, 0).compact(), "21000301T000000000Z");
    }

    #[test]
    fn times_before_1970_are_clamped() {
        let before = UNIX_EPOCH - Duration::from_secs(10);
        assert_eq!(
            Timestamp::from_system_time(before).compact(),
            "19700101T000000000Z"
        );
    }

    #[test]
    fn host_names_are_sanitized() {
        assert_eq!(sanitize_host_name("OFFICE"), "office");
        assert_eq!(sanitize_host_name("my_PC.example.lan"), "my-pc-example-lan");
        assert_eq!(sanitize_host_name("--Büro  PC--"), "b-ro-pc");
        assert_eq!(sanitize_host_name("äöü"), "computer");
        assert_eq!(
            sanitize_host_name("a-very-long-host-name-indeed"),
            "a-very-long-host-nam"
        );
        assert_eq!(
            sanitize_host_name("abcdefghijklmnopqrs-tuv"),
            "abcdefghijklmnopqrs"
        );
    }

    #[test]
    fn generated_machine_ids_parse() {
        for name in ["OFFICE", "my_PC.example.lan", "", "---"] {
            let id = MachineId::generate(name);
            assert_eq!(MachineId::parse(id.as_str()), Some(id.clone()), "{id}");
        }
    }

    #[test]
    fn malformed_machine_ids_are_rejected() {
        for bad in [
            "office",
            "office-7f3",
            "office-7F3A",
            "-7f3a",
            "off--ice-7f3a",
            "OFFICE-7f3a",
            "off ice-7f3a",
        ] {
            assert_eq!(MachineId::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn mail_ids_round_trip_through_file_names() {
        let machine = MachineId::parse("office-7f3a").unwrap();
        let id = MailId::new(at(1_792_016_130, 123), &machine);
        let name = id.file_name();
        assert!(
            name.starts_with("20261014T221530123Z-office-7f3a-"),
            "{name}"
        );
        assert!(name.ends_with(".eml"));
        let parsed = MailId::from_file_name(&name).unwrap();
        assert_eq!(parsed, id);
        assert_eq!(parsed.machine(), machine);
        assert_eq!(parsed.year(), 2026);
    }

    #[test]
    fn foreign_file_names_are_not_mail_ids() {
        for name in [
            "notes.txt",
            "20261014T221530123Z-office-7f3a-9c41e2d0.txt",
            "20261014T221530123Z-office-7f3a-9c41e2d.eml",
            "20261014X221530123Z-office-7f3a-9c41e2d0.eml",
            "20261014T221530123Z-office-9c41e2d0.eml",
            "ä0261014T221530123Z-office-7f3a-9c41e2d0.eml",
            ".eml",
        ] {
            assert_eq!(MailId::from_file_name(name), None, "{name}");
        }
    }

    #[test]
    fn mail_ids_are_unique() {
        let machine = MachineId::parse("office-7f3a").unwrap();
        let time = Timestamp::now();
        let ids: HashSet<_> = (0..10_000).map(|_| MailId::new(time, &machine)).collect();
        assert_eq!(ids.len(), 10_000);
    }

    #[test]
    fn machine_id_is_created_once_and_then_reused() {
        let dir = crate::storage::test_support::TempDir::new("machine-id");
        let file = dir.path().join("local").join("machine-id");
        let first = MachineId::load_or_create(&file, "OFFICE").unwrap();
        let second = MachineId::load_or_create(&file, "OTHER").unwrap();
        assert_eq!(first, second);
        assert!(first.as_str().starts_with("office-"));
    }

    #[test]
    fn malformed_machine_id_file_is_an_error_and_kept() {
        let dir = crate::storage::test_support::TempDir::new("machine-id-bad");
        let file = dir.path().join("machine-id");
        fs::write(&file, "not an id").unwrap();
        let err = MachineId::load_or_create(&file, "OFFICE").unwrap_err();
        assert!(
            matches!(err, StorageError::InvalidMachineId { .. }),
            "{err}"
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), "not an id");
    }
}
