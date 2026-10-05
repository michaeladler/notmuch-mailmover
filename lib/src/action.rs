use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use log::{debug, info, trace, warn};

use crate::engine::Moves;
use crate::{Error, Result};

/// Moves the files collected by [`crate::engine::plan_moves`].
///
/// `moves` maps a message file to a destination folder relative to `maildir`.
/// Each file is moved to `<maildir>/<folder>/<mailbox>`, where `<mailbox>` is
/// the file's current parent directory name, so nested mailboxes keep their
/// subtree. With `rename` set, the file gets a new unique name that keeps the
/// Maildir flags suffix, which mbsync needs.
///
/// Messages already at their destination are skipped. With `dry_run`, only the
/// intended moves are logged and nothing is written. Missing parent
/// directories are created. A file that has disappeared (not yet indexed by
/// `notmuch new`) is logged as a warning and skipped, so one stale entry does
/// not abort the run.
///
/// # Errors
///
/// Returns an error if a path has no file or mailbox component, or if creating
/// the destination directory or renaming a file fails. Moves performed before
/// the failure are kept.
pub fn move_files(maildir: &str, rename: bool, dry_run: bool, moves: &Moves) -> Result<()> {
    if moves.is_empty() {
        info!("nothing to do");
        return Ok(());
    }

    debug!("applying {} moves", moves.len());

    let mut counter: usize = 0;
    for (src_file, folder) in moves {
        let basename = src_file
            .file_name()
            .ok_or_else(|| Error::NoFileName(src_file.to_string_lossy().into_owned()))?;

        // keep the original mailbox subtree, so nested folders move within it
        let mailbox = src_file
            .parent()
            .and_then(|p| p.file_name())
            .ok_or_else(|| Error::NoMailboxName(src_file.to_string_lossy().into_owned()))?;

        let mut dest_file = PathBuf::from(maildir).join(folder).join(mailbox);
        if rename {
            dest_file.push(new_name(basename));
        } else {
            dest_file.push(basename);
        };

        if *src_file == dest_file {
            trace!(
                "skipping {} as message is already in the correct destination",
                src_file.to_string_lossy()
            );
            continue;
        }

        if dry_run {
            info!(
                "would move {} to {}",
                src_file.to_string_lossy(),
                dest_file.to_string_lossy()
            );
            continue;
        }

        info!(
            "moving {} to {}",
            src_file.to_string_lossy(),
            dest_file.to_string_lossy()
        );
        if src_file.exists() {
            fs::create_dir_all(dest_file.parent().unwrap())?;
            fs::rename(src_file, dest_file)?;
            counter += 1;
        } else {
            warn!(
                "{} has vanished. Try running 'notmuch new'",
                src_file.to_string_lossy()
            );
        }
    }
    debug!("moved {counter} files");
    Ok(())
}

/// Construct a new filename, composed of a made-up ID and the flags part of the original filename.
fn new_name(basename: &OsStr) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut result = format!(
        "{nanos:x}.{}.{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let basename = basename.to_string_lossy();
    let parts: Vec<&str> = basename.split(':').collect();
    let n = parts.len();
    if n > 1 {
        let flags = parts[n - 1];
        write!(result, ":{flags}").unwrap();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The made-up ID part of a generated name must be `{nanos:x}.{pid}.{counter}`.
    fn is_id(s: &str) -> bool {
        let parts: Vec<&str> = s.split('.').collect();
        parts.len() == 3
            && !parts[0].is_empty()
            && parts[0].chars().all(|c| c.is_ascii_hexdigit())
            && parts[1].parse::<u32>().is_ok()
            && parts[2].parse::<u64>().is_ok()
    }

    #[test]
    fn bare_filename_is_an_error_not_a_panic() {
        let moves = Moves::from([(PathBuf::from("some.mail"), "Trash")]);
        let err = move_files("/tmp/mail", false, false, &moves).unwrap_err();
        assert_eq!("Failed to get mailbox name from some.mail", err.to_string());
    }

    /// A mail already in the target mailbox must still be renamed.
    #[test]
    fn rename_applies_to_mail_already_in_target_mailbox() {
        let maildir = std::env::temp_dir().join("nmm-rename-test");
        let _ = fs::remove_dir_all(&maildir);
        // nested mailbox: destination parent resolves to the very same directory
        let nested = maildir.join("INBOX").join("list");
        fs::create_dir_all(&nested).unwrap();
        let src = nested.join("1234.mail:2,S");
        fs::write(&src, "mail").unwrap();

        let moves = Moves::from([(src.clone(), "INBOX")]);
        move_files(&maildir.to_string_lossy(), true, false, &moves).unwrap();

        assert!(!src.exists(), "old name must be gone");
        let names: Vec<_> = fs::read_dir(&nested)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(1, names.len());
        assert!(names[0].ends_with(":2,S"), "flags kept: {}", names[0]);
        assert!(is_id(names[0].split(':').next().unwrap()));
    }

    #[test]
    fn move_creates_missing_destination_directory() {
        let maildir = std::env::temp_dir().join("nmm-mkdir-test");
        let _ = fs::remove_dir_all(&maildir);
        let src_dir = maildir.join("INBOX");
        fs::create_dir_all(&src_dir).unwrap();
        let src = src_dir.join("1234.mail:2,S");
        fs::write(&src, "mail").unwrap();

        let moves = Moves::from([(src, "Archive/2026")]);
        move_files(&maildir.to_string_lossy(), false, false, &moves).unwrap();

        assert!(maildir.join("Archive/2026/INBOX").is_dir());
    }

    #[test]
    fn new_name_handles_non_utf8_basename() {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let basename = OsStr::from_bytes(b"1234.mail\xff:2,S");
            let new_name = new_name(basename);
            let parts: Vec<&str> = new_name.split(':').collect();
            assert_eq!(2, parts.len());
            assert_eq!("2,S", parts[1]);
        }
    }

    /// Names must never collide, even within one run and one nanosecond.
    #[test]
    fn new_name_is_unique() {
        let names: std::collections::HashSet<String> = (0..10_000)
            .map(|_| new_name(OsStr::new("1234.mail:2,S")))
            .collect();
        assert_eq!(10_000, names.len());
    }

    #[test]
    fn new_name_test() {
        {
            let fname = new_name(OsStr::new(
                "1662362645_0.322365.foo,U=55582,FMD5=7e33429f656f1e6e9d79b29c3f82c57e:2,S",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(is_id(parts[0]));
            assert_eq!("2,S", parts[1]);
            assert_eq!(2, parts.len());
        }

        {
            let fname = new_name(OsStr::new(
                "1662103908_2.328294.foo,U=55119,FMD5=7e33429f656f1e6e9d79b29c3f82c57e:2,RS",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(is_id(parts[0]));
            assert_eq!("2,RS", parts[1]);
            assert_eq!(2, parts.len());
        }

        {
            let fname = new_name(OsStr::new(
                "1662103908_2.328294.foo,U=55119,FMD5=7e33429f656f1e6e9d79b29c3f82c57e:",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(is_id(parts[0]));
            assert_eq!("", parts[1]);
            assert_eq!(2, parts.len());
        }

        {
            let fname = new_name(OsStr::new(
                "1662103908_2.328294.foo,U=55119,FMD5=7e33429f656f1e6e9d79b29c3f82c57e",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(is_id(parts[0]));
            assert_eq!(1, parts.len());
        }
    }
}
