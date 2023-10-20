use std::fmt::Write as _;
use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Result};
use log::debug;

use crate::config::Config;
use crate::repo::MailRepo;

pub fn apply_rules<'a>(cfg: &'a Config, repo: &dyn MailRepo) -> Result<HashMap<PathBuf, &'a str>> {
    debug!("applying rules");
    let mut actions = HashMap::new();

    let n = cfg.rules.len();
    if n > 0 {
        debug!("checking if rules overlap");
        let mut combined_query = String::with_capacity(1024);
        for rule in cfg.rules.iter().take(n - 1) {
            write!(combined_query, "({}) AND ", rule.query)?;
        }
        write!(combined_query, "({})", cfg.rules[n - 1].query)?;
        if let Some(days) = cfg.max_age_days {
            write!(combined_query, " AND date:\"{}_days\"..", days)?;
        }
        debug!("combined query: {}", combined_query);
        let messages = repo.search_message(&combined_query)?;
        let count = messages.len();
        if count > 0 {
            return Err(anyhow!("Rules overlap! overlap count: {}", count));
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
                return Err(anyhow!("Ambiguous result: {} and {}", old, rule.folder));
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
        let mut cfg: Config = Default::default();
        cfg.rules.push(Rule {
            folder: "Trash".to_string(),
            query: "tag:trash".to_string(),
        });
        cfg.rules.push(Rule {
            folder: "Deleted".to_string(),
            query: "tag:trash".to_string(),
        });

        let mut repo: DummyRepo = Default::default();
        repo.add_mail(
            "NOT folder:\"Trash\" AND (tag:trash)".to_string(),
            "some.mail".to_string(),
        );
        repo.add_mail(
            "NOT folder:\"Deleted\" AND (tag:trash)".to_string(),
            "some.mail".to_string(),
        );
        let actions = apply_rules(&cfg, &repo);
        assert!(actions.is_err());
        let err = actions.unwrap_err();
        assert_eq!("Ambiguous result: Trash and Deleted", err.to_string());
    }
}
