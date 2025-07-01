use std::fmt::Write as _;
use std::{collections::HashMap, path::Path, path::PathBuf};

use anyhow::{anyhow, Result};
use log::{debug, error, warn};

use crate::config::{Config, MatchMode};
use crate::repo::MailRepo;

fn filter_messages_with_prefix(messages: &[PathBuf], prefix: &PathBuf) -> Vec<PathBuf> {
    messages
        .iter()
        .filter_map(|p| {
            if p.starts_with(prefix) {
                Some(p.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Apply the given rules to the mails in the repository.
/// The result is a HashMap which is assigns messages (files) to their new destination folders.
/// Note that no messages are actually moved at this stage.
pub fn apply_rules<'a>(cfg: &'a Config, repo: &dyn MailRepo) -> Result<HashMap<PathBuf, &'a str>> {
    debug!("applying rules");
    match cfg.rule_match_mode.unwrap_or(MatchMode::Unique) {
        MatchMode::Unique => apply_unique(cfg, repo),
        MatchMode::First => apply_first(cfg, repo),
        MatchMode::All => apply_all(cfg, repo),
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

                let prefix = match (lhs.prefix.clone(), rhs.prefix.clone()) {
                    (None, None) => None,
                    (Some(lp), Some(rp)) => Some(if lp.starts_with(&rp) {
                        lp
                    } else if rp.starts_with(&lp) {
                        rp
                    } else {
                        // If the two prefixes are not subwords of one another, then the queries
                        // must match different mails so the following is unnecessary.
                        continue;
                    }),
                    (l, r) => l.or(r),
                }
                .map(|s| Path::new(&cfg.maildir).join(s));
                debug!("prefix: {prefix:?}");

                combined_query.clear();
                write!(combined_query, "({}) AND ({})", lhs.query, rhs.query)?;
                if let Some(days) = cfg.max_age_days {
                    write!(combined_query, " AND date:\"{days}_days\"..")?;
                }
                debug!("combined query: {combined_query}");
                let all_messages = repo.search_message(&combined_query)?;
                let messages = prefix
                    .map(|p| filter_messages_with_prefix(&all_messages, &p))
                    .unwrap_or(all_messages);
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
            write!(query_str, " AND date:\"{days}_days\"..")?;
        }
        debug!("using query: {query_str}");
        let all_messages = repo.search_message(&query_str)?;
        let messages = if let Some(pre) = &rule.prefix {
            let prefix = Path::new(&cfg.maildir).join(pre);
            debug!("using prefix: {prefix:?}");
            filter_messages_with_prefix(&all_messages, &prefix)
        } else {
            debug!("No prefix");
            all_messages
        };
        for filename in messages {
            debug!("processing {:?}", filename.to_str());
            if let Some(old) = actions.insert(filename, rule.folder.as_str()) {
                return Err(anyhow!("Ambiguous rule! Message already assigned to folder {}, cannot assign to folder {}", old, rule.folder));
            }
        }
    }
    Ok(actions)
}

fn apply_first<'a>(cfg: &'a Config, repo: &dyn MailRepo) -> Result<HashMap<PathBuf, &'a str>> {
    let mut actions = HashMap::new();
    // exclude previous rules and folders
    let mut exclude = String::with_capacity(32768);
    for rule in &cfg.rules {
        let mut query_str = format!("(NOT folder:{}) AND ({})", &rule.folder, &rule.query);
        if !exclude.is_empty() {
            write!(query_str, " AND ({exclude})")?;
        }
        if let Some(days) = cfg.max_age_days {
            write!(query_str, " AND date:\"{days}_days\"..")?;
        }

        let messages = repo.search_message(&query_str)?;
        debug!("query '{}' returned {} messages", query_str, messages.len());
        for msg in messages {
            let fname = msg
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("unknown");
            debug!("assigning {} to {}", fname, rule.folder);
            actions.insert(msg, rule.folder.as_str());
        }

        if !exclude.is_empty() {
            write!(exclude, " AND ")?;
        }
        write!(exclude, "NOT ({})", rule.query)?;
    }
    Ok(actions)
}

fn apply_all<'a>(cfg: &'a Config, repo: &dyn MailRepo) -> Result<HashMap<PathBuf, &'a str>> {
    let mut actions = HashMap::new();
    for rule in &cfg.rules {
        let mut query_str = format!("({})", &rule.query);
        if let Some(days) = cfg.max_age_days {
            write!(query_str, " AND date:\"{days}_days\"..")?;
        }
        let messages = repo.search_message(&query_str)?;
        debug!("query '{}' returned {} messages", query_str, messages.len());
        for msg in messages {
            let fname = msg
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("unknown");
            // check if message was matched previously
            if let Some(folder) = actions.get(&msg) {
                warn!("Ambiguous rule! Message {fname} was previously assigned to folder {folder}");
            }
            debug!("Assigning {:?} to {}", msg, rule.folder);
            actions.insert(msg, rule.folder.as_str());
        }
    }
    Ok(actions)
}

#[cfg(test)]
mod tests {

    use std::{str::FromStr, vec};

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
            debug!("[DummyRepo] searching for: {query}");
            if let Some(fnames) = self.tag2mail.get(query) {
                debug!("[DummyRepo] returning: {fnames:?}");
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
            prefix: None,
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
            "actions should have exactly one element but was: {actions:?}"
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
            prefix: None,
        });
        cfg1.rules.push(Rule {
            folder: "Deleted".to_string(),
            query: "tag:trash".to_string(),
            prefix: None,
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
        let cfg = Config {
            rule_match_mode: Some(MatchMode::First),
            rules: vec![
                Rule {
                    folder: "Trash".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: None,
                },
                Rule {
                    folder: "Deleted".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: None,
                },
            ],
            ..Default::default()
        };

        let mut repo: DummyRepo = Default::default();
        repo.add_mail(
            "(NOT folder:Trash) AND (tag:trash)".to_string(),
            "some.mail".to_string(),
        );
        let actions = apply_rules(&cfg, &repo).unwrap();
        assert_eq!(actions.len(), 1);
        let pb = PathBuf::from_str("some.mail").unwrap();
        let folder = *actions.get(&pb).unwrap();
        assert_eq!("Trash", folder);
    }

    #[test]
    fn rule_match_mode_all() {
        let cfg = Config {
            rule_match_mode: Some(MatchMode::All),
            rules: vec![
                Rule {
                    folder: "Trash".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: None,
                },
                Rule {
                    folder: "Deleted".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: None,
                },
            ],
            ..Default::default()
        };

        let mut repo: DummyRepo = Default::default();
        repo.add_mail("(tag:trash)".to_string(), "some.mail".to_string());
        let actions = apply_rules(&cfg, &repo).unwrap();
        assert_eq!(actions.len(), 1);
        let pb = PathBuf::from_str("some.mail").unwrap();
        let folder = *actions.get(&pb).unwrap();
        assert_eq!("Deleted", folder);
    }

    #[test]
    fn rules_with_prefixes() {
        let cfg = Config {
            rule_match_mode: Some(MatchMode::Unique),
            rules: vec![
                Rule {
                    folder: "mailbox1/Trash".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: Some("mailbox1".to_string()),
                },
                Rule {
                    folder: "mailbox2/Trash".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: Some("mailbox2".to_string()),
                },
            ],
            ..Default::default()
        };

        let mut repo: DummyRepo = Default::default();
        repo.add_mail(
            "NOT folder:\"mailbox1/Trash\" AND (tag:trash)".to_string(),
            format!("{}/mailbox1/some.mail", cfg.maildir),
        );
        repo.add_mail(
            "NOT folder:\"mailbox2/Trash\" AND (tag:trash)".to_string(),
            format!("{}/mailbox2/some.mail", cfg.maildir),
        );

        let actions = apply_rules(&cfg, &repo).unwrap();
        assert_eq!(actions.len(), 2);

        dbg!(&actions);

        let pb1 = PathBuf::from_str("~/mail/mailbox1/some.mail").unwrap();
        let pb2 = PathBuf::from_str("~/mail/mailbox2/some.mail").unwrap();

        let folder1 = *actions.get(&pb1).unwrap();
        let folder2 = *actions.get(&pb2).unwrap();

        assert_eq!("mailbox1/Trash", folder1);
        assert_eq!("mailbox2/Trash", folder2);
    }
}
