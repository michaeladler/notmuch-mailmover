use std::fmt::Write as _;
use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Result};
use log::{debug, error, trace, warn};

use crate::config::{Config, MatchMode};
use crate::repo::MailRepo;

/// Apply the given rules to the mails in the repository.
/// The result is a HashMap which is assigns messages (files) to their new destination folders.
/// Note that no messages are actually moved at this stage.
pub fn apply_rules<'a>(cfg: &'a Config, repo: &dyn MailRepo) -> Result<HashMap<PathBuf, &'a str>> {
    debug!("applying rules");
    let match_mode = cfg.rule_match_mode.unwrap_or(MatchMode::Unique);
    match match_mode {
        MatchMode::Unique => apply_unique(cfg, repo),
        MatchMode::First | MatchMode::All => {
            let mut actions = HashMap::new();
            for rule in &cfg.rules {
                let mut query_str = format!("({})", &rule.query);
                if let Some(days) = cfg.max_age_days {
                    write!(query_str, " AND date:\"{}_days\"..", days)?;
                }
                let messages = repo.search_message(&query_str)?;
                debug!("query '{}' returned {} messages", query_str, messages.len());
                for msg in messages {
                    let fname = msg
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("unknown");
                    trace!("processing {}", fname);
                    // check if message was matched previously
                    if let Some(folder) = actions.get(&msg) {
                        match match_mode {
                            MatchMode::First => {
                                debug!("Message {fname} was already assigned to folder {folder}, not moving into {}", rule.folder);
                                continue;
                            }
                            MatchMode::All => {
                                warn!("Ambiguous rule! Message {fname} was previously assigned to folder {folder}");
                            }
                            _ => {}
                        }
                    }
                    debug!("Assigning {:?} to {}", msg, rule.folder);
                    actions.insert(msg, rule.folder.as_str());
                }
            }
            Ok(actions)
        }
    }
}

fn apply_unique<'a>(cfg: &'a Config, repo: &dyn MailRepo) -> Result<HashMap<PathBuf, &'a str>> {
    let mut actions = HashMap::new();
    let n = cfg.rules.len();
    if n > 0 {
        let mut overlap_count: usize = 0;

        debug!("checking if any two rules overlap");
        let mut combined_query = String::with_capacity(2048);
        for i in 0..n - 1 {
            for j in i + 1..n {
                let lhs = cfg.rules.get(i).unwrap();
                let rhs = cfg.rules.get(j).unwrap();

                combined_query.clear();
                write!(combined_query, "({}) AND ({})", lhs.query, rhs.query)?;
                if let Some(days) = cfg.max_age_days {
                    write!(combined_query, " AND date:\"{}_days\"..", days)?;
                }
                debug!("combined query: {}", combined_query);
                let messages = repo.search_message(&combined_query)?;
                if !messages.is_empty() {
                    let count = messages.len();
                    overlap_count += count;
                    error!(
                        "Queries '{}' and '{}' overlap ({} messages)",
                        lhs.query, rhs.query, count
                    );
                }
            }
        }

        if overlap_count > 0 {
            return Err(anyhow!("Rules overlap ({} messages)", overlap_count));
        }
    }

    for rule in &cfg.rules {
        let mut query_str = format!("NOT folder:\"{}\" AND ({})", rule.folder, &rule.query);
        if let Some(days) = cfg.max_age_days {
            write!(query_str, " AND date:\"{}_days\"..", days)?;
        }
        debug!("using query: {}", query_str);
        let messages = repo.search_message(&query_str)?;
        for filename in messages {
            debug!("processing {:?}", filename.to_str());
            if let Some(old) = actions.insert(filename, rule.folder.as_str()) {
                return Err(anyhow!("Ambiguous rule! Message already assigned to folder {}, cannot assign to folder {}", old, rule.folder));
            }
        }
    }
    Ok(actions)
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    #[derive(Debug, Default)]
    struct DummyRepo {
        tag2mail: HashMap<String, Vec<PathBuf>>,
    }

    impl DummyRepo {
        pub fn add_mail(&mut self, tag: String, fname: String) {
            self.tag2mail
                .entry(tag)
                .or_default()
                .push(PathBuf::from_str(&fname).unwrap());
        }
    }

    impl MailRepo for DummyRepo {
        fn search_message(&self, query: &str) -> Result<Vec<PathBuf>> {
            debug!("[DummyRepo] searching for: {}", query);
            if let Some(fnames) = self.tag2mail.get(query) {
                debug!("[DummyRepo] returning: {:?}", fnames);
                return Ok(fnames.to_vec());
            }
            Ok(Vec::new())
        }
    }

    use super::*;
    use crate::config::Rule;

    #[test]
    fn simple_test() {
        let mut cfg: Config = Default::default();
        cfg.rules.push(Rule {
            folder: "Trash".to_string(),
            query: "tag:trash".to_string(),
        });

        let mut repo: DummyRepo = Default::default();
        repo.add_mail(
            "NOT folder:\"Trash\" AND (tag:trash)".to_string(),
            "some.mail".to_string(),
        );

        let actions = apply_rules(&cfg, &repo).unwrap();
        assert_eq!(
            1,
            actions.len(),
            "actions should have exactly one element but was: {:?}",
            actions
        );

        let pb = PathBuf::from_str("some.mail").unwrap();
        let folder = *actions.get(&pb).unwrap();
        assert_eq!("Trash", folder);
    }

    #[test]
    fn ambiguous_rule_test() {
        let mut repo: DummyRepo = Default::default();
        repo.add_mail(
            "NOT folder:\"Trash\" AND (tag:trash)".to_string(),
            "some.mail".to_string(),
        );
        repo.add_mail(
            "NOT folder:\"Deleted\" AND (tag:trash)".to_string(),
            "some.mail".to_string(),
        );

        let mut cfg1: Config = Default::default();
        cfg1.rules.push(Rule {
            folder: "Trash".to_string(),
            query: "tag:trash".to_string(),
        });
        cfg1.rules.push(Rule {
            folder: "Deleted".to_string(),
            query: "tag:trash".to_string(),
        });

        let mut cfg2 = cfg1.clone();
        cfg2.rule_match_mode = Some(MatchMode::Unique);

        for cfg in &[cfg1, cfg2] {
            let actions = apply_rules(cfg, &repo);
            assert!(actions.is_err());
            let err = actions.unwrap_err();
            assert_eq!("Ambiguous rule! Message already assigned to folder Trash, cannot assign to folder Deleted", err.to_string());
        }
    }

    #[test]
    fn rule_match_mode_first_test() {
        let mut cfg: Config = Default::default();
        cfg.rule_match_mode = Some(MatchMode::First);
        cfg.rules.push(Rule {
            folder: "Trash".to_string(),
            query: "tag:trash".to_string(),
        });
        cfg.rules.push(Rule {
            folder: "Deleted".to_string(),
            query: "tag:trash".to_string(),
        });

        let mut repo: DummyRepo = Default::default();
        repo.add_mail("(tag:trash)".to_string(), "some.mail".to_string());
        let actions = apply_rules(&cfg, &repo).unwrap();
        assert_eq!(actions.len(), 1);
        let pb = PathBuf::from_str("some.mail").unwrap();
        let folder = *actions.get(&pb).unwrap();
        assert_eq!("Trash", folder);
    }

    #[test]
    fn rule_match_mode_all() {
        let mut cfg: Config = Default::default();
        cfg.rule_match_mode = Some(MatchMode::All);
        cfg.rules.push(Rule {
            folder: "Trash".to_string(),
            query: "tag:trash".to_string(),
        });
        cfg.rules.push(Rule {
            folder: "Deleted".to_string(),
            query: "tag:trash".to_string(),
        });

        let mut repo: DummyRepo = Default::default();
        repo.add_mail("(tag:trash)".to_string(), "some.mail".to_string());
        let actions = apply_rules(&cfg, &repo).unwrap();
        assert_eq!(actions.len(), 1);
        let pb = PathBuf::from_str("some.mail").unwrap();
        let folder = *actions.get(&pb).unwrap();
        assert_eq!("Deleted", folder);
    }
}
