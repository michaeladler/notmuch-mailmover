use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use log::{debug, info, trace, warn};
use uuid::Uuid;

use crate::engine::{Config, Moves};

/// Moves the files collected by [`crate::engine::plan_moves`].
///
/// `moves` maps a message file to a destination folder relative to
/// [`Config::maildir`]. Each file is moved to `<maildir>/<folder>/<mailbox>`,
/// where `<mailbox>` is the file's current parent directory name, so nested
/// mailboxes keep their subtree. With [`Config::rename`] set, the file gets a
/// new unique name that keeps the Maildir flags suffix, which mbsync needs.
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
pub fn move_files(cfg: &impl Config, dry_run: bool, moves: &Moves) -> Result<()> {
    if moves.is_empty() {
        info!("nothing to do");
        return Ok(());
    }

    debug!("applying {} moves", moves.len());

    let mut counter: usize = 0;
    for (src_file, folder) in moves {
        let basename = src_file
            .file_name()
            .ok_or_else(|| anyhow!("Failed to get filename from {}", src_file.to_string_lossy()))?;

        // keep the original mailbox subtree, so nested folders move within it
        let mailbox = src_file
            .parent()
            .and_then(|p| p.file_name())
            .ok_or_else(|| {
                anyhow!(
                    "Failed to get mailbox name from {}",
                    src_file.to_string_lossy()
                )
            })?;

        let maildir = PathBuf::from(cfg.maildir());
        let mut dest_file = maildir.join(folder).join(mailbox);
        if cfg.rename() {
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
    let mut result = Uuid::new_v4().to_string();
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
    use std::collections::HashMap;

    use super::*;
    use crate::engine::{MatchMode, Rule};

    struct TestCfg {
        maildir: String,
        rename: bool,
    }

    impl Config for TestCfg {
        fn maildir(&self) -> &str {
            &self.maildir
        }
        fn max_age_days(&self) -> Option<u32> {
            None
        }
        fn rename(&self) -> bool {
            self.rename
        }
        fn rules(&self) -> &[Rule] {
            &[]
        }
        fn rule_match_mode(&self) -> MatchMode {
            MatchMode::Unique
        }
    }

    #[test]
    fn bare_filename_is_an_error_not_a_panic() {
        let cfg = TestCfg {
            maildir: "/tmp/mail".to_string(),
            rename: false,
        };
        let mut moves = HashMap::new();
        moves.insert(PathBuf::from("some.mail"), "Trash");
        let err = move_files(&cfg, false, &moves).unwrap_err();
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

        let cfg = TestCfg {
            maildir: maildir.to_string_lossy().into_owned(),
            rename: true,
        };
        let mut moves = HashMap::new();
        moves.insert(src.clone(), "INBOX");
        move_files(&cfg, false, &moves).unwrap();

        assert!(!src.exists(), "old name must be gone");
        let names: Vec<_> = fs::read_dir(&nested)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(1, names.len());
        assert!(names[0].ends_with(":2,S"), "flags kept: {}", names[0]);
        assert!(Uuid::parse_str(names[0].split(':').next().unwrap()).is_ok());
    }

    #[test]
    fn move_creates_missing_destination_directory() {
        let maildir = std::env::temp_dir().join("nmm-mkdir-test");
        let _ = fs::remove_dir_all(&maildir);
        let src_dir = maildir.join("INBOX");
        fs::create_dir_all(&src_dir).unwrap();
        let src = src_dir.join("1234.mail:2,S");
        fs::write(&src, "mail").unwrap();

        let cfg = TestCfg {
            maildir: maildir.to_string_lossy().into_owned(),
            rename: false,
        };
        let mut moves = HashMap::new();
        moves.insert(src, "Archive/2026");
        move_files(&cfg, false, &moves).unwrap();

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

    #[test]
    fn new_name_test() {
        let is_uuid = |s: &str| Uuid::parse_str(s).is_ok();

        {
            let fname = new_name(OsStr::new(
                "1662362645_0.322365.foo,U=55582,FMD5=7e33429f656f1e6e9d79b29c3f82c57e:2,S",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(is_uuid(parts[0]));
            assert_eq!("2,S", parts[1]);
            assert_eq!(2, parts.len());
        }

        {
            let fname = new_name(OsStr::new(
                "1662103908_2.328294.foo,U=55119,FMD5=7e33429f656f1e6e9d79b29c3f82c57e:2,RS",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(is_uuid(parts[0]));
            assert_eq!("2,RS", parts[1]);
            assert_eq!(2, parts.len());
        }

        {
            let fname = new_name(OsStr::new(
                "1662103908_2.328294.foo,U=55119,FMD5=7e33429f656f1e6e9d79b29c3f82c57e:",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(is_uuid(parts[0]));
            assert_eq!("", parts[1]);
            assert_eq!(2, parts.len());
        }

        {
            let fname = new_name(OsStr::new(
                "1662103908_2.328294.foo,U=55119,FMD5=7e33429f656f1e6e9d79b29c3f82c57e",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(is_uuid(parts[0]));
            assert_eq!(1, parts.len());
        }
    }
}
