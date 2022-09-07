use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use uuid::Uuid;

use crate::config::Config;

pub fn apply_actions(cfg: &Config, dry_run: bool, actions: &HashMap<PathBuf, &str>) -> Result<()> {
    if actions.is_empty() {
        info!("nothing to do");
        return Ok(());
    }

    debug!("applying {} actions", actions.len());

    let mut counter: usize = 0;
    for (src_file, folder) in actions {
        let basename = src_file
            .file_name()
            .ok_or_else(|| anyhow!("Failed to get filename from {}", src_file.to_string_lossy()))?;

        let db_path = PathBuf::from(&cfg.database_path);
        let mut dest_file = db_path.join(folder).join("cur");
        if cfg.rename {
            dest_file.push(get_new_name(basename));
        } else {
            dest_file.push(basename);
        };

        if dry_run {
            info!(
                "would move {} to {}",
                src_file.to_string_lossy(),
                dest_file.to_string_lossy()
            );
        } else {
            info!(
                "moving {} to {}",
                src_file.to_string_lossy(),
                dest_file.to_string_lossy()
            );
            if src_file.exists() {
                fs::rename(src_file, dest_file)?;
                counter += 1;
            } else {
                warn!(
                    "{} has vanished. Try running 'notmuch new'",
                    src_file.to_string_lossy()
                );
            }
        }
    }
    debug!("moved {} files", counter);
    Ok(())
}

/// Construct a new filename, composed of a made-up ID and the flags part of the original filename.
fn get_new_name(basename: &OsStr) -> String {
    let mut result = Uuid::new_v4().to_string();
    let parts: Vec<&str> = basename.to_str().unwrap().split(':').collect();
    let n = parts.len();
    if n > 1 {
        let flags = parts[n - 1];
        write!(result, ":{}", flags).unwrap();
    }
    result
}

#[cfg(test)]
mod tests {
    use regex::Regex;

    use super::*;

    #[test]
    fn get_new_name_test() {
        let uuid_re =
            Regex::new(r"\b[0-9a-f]{8}\b-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-\b[0-9a-f]{12}\b")
                .unwrap();

        {
            let fname = get_new_name(OsStr::new(
                "1662362645_0.322365.foo,U=55582,FMD5=7e33429f656f1e6e9d79b29c3f82c57e:2,S",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(uuid_re.is_match(parts[0]));
            assert_eq!("2,S", parts[1]);
            assert_eq!(2, parts.len());
        }

        {
            let fname = get_new_name(OsStr::new(
                "1662103908_2.328294.foo,U=55119,FMD5=7e33429f656f1e6e9d79b29c3f82c57e:2,RS",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(uuid_re.is_match(parts[0]));
            assert_eq!("2,RS", parts[1]);
            assert_eq!(2, parts.len());
        }

        {
            let fname = get_new_name(OsStr::new(
                "1662103908_2.328294.foo,U=55119,FMD5=7e33429f656f1e6e9d79b29c3f82c57e:",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(uuid_re.is_match(parts[0]));
            assert_eq!("", parts[1]);
            assert_eq!(2, parts.len());
        }

        {
            let fname = get_new_name(OsStr::new(
                "1662103908_2.328294.foo,U=55119,FMD5=7e33429f656f1e6e9d79b29c3f82c57e",
            ));
            let parts: Vec<&str> = fname.split(':').collect();
            assert!(uuid_re.is_match(parts[0]));
            assert_eq!(1, parts.len());
        }
    }
}
