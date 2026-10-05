use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::hash_map::{DefaultHasher, Entry};
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};

use crate::repo::Repo;

/// Message files and the destination folder each should be moved to, sorted by
/// source path so a run is reproducible.
pub type Moves<'a> = Vec<(PathBuf, &'a str)>;

/// `path -> folder` per message, unsorted.
type Assigned<'a> = HashMap<PathBuf, &'a str>;

/// Adds `path -> folder` to `moves`, replacing an existing assignment, and
/// returns the folder `path` had before (if any).
fn assign<'a>(moves: &mut Assigned<'a>, path: PathBuf, folder: &'a str) -> Option<&'a str> {
    moves.insert(path, folder)
}

/// An empty result over a non-empty input usually means the prefix does not
/// match how notmuch spells the paths, e.g. maildir is a symlink notmuch
/// resolved. The rule would then silently no-op.
fn filter_by_prefix(messages: &[PathBuf], prefix: &Path) -> Vec<PathBuf> {
    let filtered: Vec<PathBuf> = messages
        .iter()
        .filter(|p| p.starts_with(prefix))
        .cloned()
        .collect();
    if !messages.is_empty() && filtered.is_empty() {
        warn!(
            "prefix {prefix:?} matched none of {} messages; \
             check for a symlinked maildir, notmuch reports resolved paths",
            messages.len()
        );
    }
    filtered
}

/// Assigns each matching message file to its destination folder.
///
/// The result pairs each message file (as returned by [`Repo::search_messages`],
/// i.e. usually absolute) with a destination folder relative to the maildir. No
/// file is touched; pass the result to [`crate::action::move_files`] to move
/// them.
///
/// Every query is wrapped in `NOT folder:"<folder>" AND (...)`, so messages
/// already sitting in their destination are not returned and stay put. When
/// [`Config::max_age_days`] is set, `date:"<days>_days"..` is appended, limiting
/// the search to recent messages.
///
/// [`Config::rule_match_mode`] selects between three strategies:
///
/// * [`MatchMode::Unique`] (the default): rules must be pairwise disjoint. All
///   pairs are probed for overlap first and an [`anyhow::Error`] is returned
///   before anything moves, if two queries can match the same message.
/// * [`MatchMode::First`]: the first matching rule wins. Later rules get
///   `AND NOT (<earlier queries>)` appended, so rule order decides.
/// * [`MatchMode::All`]: every matching rule applies, in order. A message
///   matched twice is only moved once, to the last matching folder, and a
///   warning is logged.
///
/// # Errors
///
/// Returns an error if a query fails, or (in [`MatchMode::Unique`]) if two rules
/// overlap or assign one message to two folders.
pub fn plan_moves<'a>(cfg: &'a impl Config, repo: &dyn Repo) -> Result<Moves<'a>> {
    debug!("planning moves");
    let assigned = match cfg.rule_match_mode() {
        MatchMode::Unique => plan_unique(cfg, repo),
        MatchMode::First => plan_first(cfg, repo),
        MatchMode::All => plan_all(cfg, repo),
    }?;
    let mut moves: Moves<'a> = assigned.into_iter().collect();
    moves.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(moves)
}

/// Searches the messages of `rule`, restricted to `rule.prefix`.
///
/// `guard` is ANDed in front of the rule's query, so callers own the `folder:`
/// exclusion and, in [`MatchMode::First`], the queries of earlier rules. The
/// prefix is applied as a post-filter, like in [`plan_unique`], because `path:`
/// is not reliable enough for that.
fn search_rule(
    cfg: &impl Config,
    repo: &dyn Repo,
    rule: &Rule,
    guard: &str,
) -> Result<Vec<PathBuf>> {
    let mut query_str = String::with_capacity(256);
    if !guard.is_empty() {
        write!(query_str, "{guard} AND ")?;
    }
    write!(query_str, "({})", rule.query)?;
    if let Some(days) = cfg.max_age_days() {
        write!(query_str, " AND date:\"{days}_days\"..")?;
    }
    debug!("using query: {query_str}");
    let messages = repo.search_messages(&query_str)?;
    Ok(match &rule.prefix {
        Some(prefix) => {
            let prefix = Path::new(cfg.maildir()).join(prefix);
            debug!("using prefix: {prefix:?}");
            filter_by_prefix(&messages, &prefix)
        }
        None => {
            debug!("no prefix");
            messages
        }
    })
}

fn plan_unique<'a>(cfg: &'a impl Config, repo: &dyn Repo) -> Result<Assigned<'a>> {
    let mut moves = Assigned::new();
    let rules = cfg.rules();
    if rules.len() > 1 {
        debug!("checking if any two rules overlap");
        // One unguarded search per rule, then intersect the results in memory. A combined
        // query per pair would ask the DB n(n-1)/2 times for the same answer.
        // The searches stay unguarded on purpose: a message already sitting in a
        // destination folder still overlaps and must be reported.
        // Prefix filtering already happened per rule, so disjoint prefixes (mailbox1
        // vs mailbox10) yield disjoint sets without a pairwise path comparison.
        // claimed: hash(path) -> index of the first rule that returned it. Hashing
        // keeps the map at ~16 bytes per entry instead of the whole path, at the
        // price of a 1-in-2^64 chance of an overlap that isn't one.
        let mut claimed: HashMap<u64, usize> = HashMap::new();
        // (earlier rule, later rule) -> paths both returned, logged once per pair.
        let mut overlaps: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        for (i, rule) in rules.iter().enumerate() {
            for path in search_rule(cfg, repo, rule, "")? {
                let mut hasher = DefaultHasher::new();
                path.hash(&mut hasher);
                match claimed.entry(hasher.finish()) {
                    // Same path twice in one result set is not a rule overlap.
                    Entry::Occupied(e) if *e.get() != i => {
                        *overlaps.entry((*e.get(), i)).or_default() += 1;
                    }
                    Entry::Occupied(_) => {}
                    Entry::Vacant(e) => {
                        e.insert(i);
                    }
                }
            }
        }

        for ((i, j), count) in &overlaps {
            error!(
                "Queries '{}' and '{}' overlap ({} messages)",
                rules[*i].query, rules[*j].query, count
            );
        }
        let overlap_count: usize = overlaps.values().sum();
        if overlap_count > 0 {
            return Err(anyhow!("Rules overlap ({} messages)", overlap_count));
        }
    }

    for rule in cfg.rules() {
        let guard = format!("NOT folder:\"{}\"", rule.folder);
        let messages = search_rule(cfg, repo, rule, &guard)?;
        for filename in messages {
            debug!("processing {:?}", filename.to_str());
            if let Some(old) = assign(&mut moves, filename, rule.folder.as_str())
                && old != rule.folder
            {
                let msg = format!(
                    "Ambiguous rule! Message already assigned to folder {old}, \
                     cannot assign to folder {}",
                    rule.folder
                );
                return Err(anyhow!(msg));
            }
        }
    }
    Ok(moves)
}

fn plan_first<'a>(cfg: &'a impl Config, repo: &dyn Repo) -> Result<Assigned<'a>> {
    let mut moves = Assigned::new();
    // exclude previous rules and folders
    let mut exclude = String::with_capacity(32768);
    for rule in cfg.rules() {
        let mut guard = format!("(NOT folder:\"{}\")", rule.folder);
        if !exclude.is_empty() {
            write!(guard, " AND ({exclude})")?;
        }
        let messages = search_rule(cfg, repo, rule, &guard)?;
        debug!(
            "query with guard '{}' returned {} messages",
            guard,
            messages.len()
        );
        for msg in messages {
            let fname = msg
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("unknown");
            debug!("assigning {} to {}", fname, rule.folder);
            assign(&mut moves, msg, rule.folder.as_str());
        }

        if !exclude.is_empty() {
            write!(exclude, " AND ")?;
        }
        write!(exclude, "NOT ({})", rule.query)?;
    }
    Ok(moves)
}

fn plan_all<'a>(cfg: &'a impl Config, repo: &dyn Repo) -> Result<Assigned<'a>> {
    let mut moves = Assigned::new();
    for rule in cfg.rules() {
        // guard against the destination folder too, else every run re-picks mail already
        // moved there and, with rename, churns its UUID
        let guard = format!("NOT folder:\"{}\"", rule.folder);
        let messages = search_rule(cfg, repo, rule, &guard)?;
        debug!("query for rule {rule} returned {} messages", messages.len());
        for msg in messages {
            let fname = msg
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("unknown")
                .to_string();
            if let Some(folder) = assign(&mut moves, msg, rule.folder.as_str()) {
                warn!("Ambiguous rule! Message {fname} was previously assigned to folder {folder}");
            }
        }
    }
    Ok(moves)
}

/// Input contract for the rule engine and the file mover.
///
/// Implement this over your own configuration type; the engine and the mover
/// read everything through it and never look at a config file themselves.
pub trait Config {
    /// Root of the maildir. Rule prefixes are relative to it, and destinations
    /// are resolved below it. May contain `~`, which the caller is expected to
    /// have expanded already.
    fn maildir(&self) -> &str;

    /// If set, only messages newer than this many days are considered. Appended
    /// to every query as `date:"<days>_days"..`.
    fn max_age_days(&self) -> Option<u32>;

    /// Whether moved files get a fresh unique name, keeping only the Maildir
    /// flags suffix. Required by mbsync; turn off to keep original filenames.
    fn rename(&self) -> bool;

    /// The rules to apply, in the order they are evaluated.
    fn rules(&self) -> &[Rule];

    /// Which overlap strategy to use. Defaults to [`MatchMode::Unique`].
    fn rule_match_mode(&self) -> MatchMode;
}

/// A single "move messages matching `query` into `folder`" rule.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Rule {
    /// Destination Maildir folder, relative to the maildir. May contain
    /// subfolders (`mailbox1/Trash`).
    pub folder: String,
    /// notmuch query selecting the messages, e.g. `tag:trash and not tag:sent`.
    pub query: String,
    /// Restricts the rule to messages whose path starts with
    /// `<maildir>/<prefix>`, in every [`MatchMode`]. Useful to disambiguate the
    /// same query per mailbox in [`MatchMode::Unique`], which bypasses the
    /// limits of `folder:` and `path:` matching. Compared per path component, so
    /// `mailbox1` does not match `mailbox10`.
    pub prefix: Option<String>,
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Rule {}{}: {}",
            self.folder,
            self.prefix
                .as_ref()
                .map(|m| format!(" with prefix '{m}'"))
                .unwrap_or_default(),
            self.query
        )
    }
}

/// How to resolve messages matched by more than one rule.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    /// Rules must be pairwise disjoint; ambiguous rules are an error.
    /// Explicit, but requires verbose queries.
    #[default]
    Unique,
    /// First matching rule wins; rule order is significant.
    First,
    /// Every matching rule applies in order; a message matched twice ends up in
    /// the folder of the last match. Concise, but easy to misconfigure.
    All,
}

#[cfg(test)]
mod tests {

    use std::cell::Cell;
    use std::collections::HashMap;
    use std::str::FromStr;

    #[derive(Debug, Default)]
    struct DummyRepo {
        tag2mail: HashMap<String, Vec<PathBuf>>,
        searches: Cell<usize>,
    }

    impl DummyRepo {
        pub fn add_mail(&mut self, tag: String, fname: String) {
            self.tag2mail
                .entry(tag)
                .or_default()
                .push(PathBuf::from_str(&fname).unwrap());
        }

        pub fn search_count(&self) -> usize {
            self.searches.get()
        }
    }

    impl Repo for DummyRepo {
        fn search_messages(&self, query: &str) -> Result<Vec<PathBuf>> {
            self.searches.set(self.searches.get() + 1);
            debug!("[DummyRepo] searching for: {query}");
            if let Some(fnames) = self.tag2mail.get(query) {
                debug!("[DummyRepo] returning: {fnames:?}");
                return Ok(fnames.to_vec());
            }
            Ok(Vec::new())
        }
    }

    use super::*;

    /// Destination folder planned for `path`, if any.
    fn folder_of<'a>(moves: &'a Moves<'a>, path: &Path) -> Option<&'a str> {
        moves.iter().find(|(p, _)| p == path).map(|(_, f)| *f)
    }

    #[derive(Clone)]
    struct TestConfig {
        maildir: String,
        rename: bool,
        max_age_days: Option<u32>,
        rules: Vec<Rule>,
        rule_match_mode: MatchMode,
    }

    impl Default for TestConfig {
        fn default() -> Self {
            Self {
                maildir: "~/mail".to_string(),
                rename: false,
                max_age_days: None,
                rules: Vec::new(),
                rule_match_mode: MatchMode::Unique,
            }
        }
    }

    impl Config for TestConfig {
        fn maildir(&self) -> &str {
            &self.maildir
        }
        fn max_age_days(&self) -> Option<u32> {
            self.max_age_days
        }
        fn rename(&self) -> bool {
            self.rename
        }
        fn rules(&self) -> &[Rule] {
            &self.rules
        }
        fn rule_match_mode(&self) -> MatchMode {
            self.rule_match_mode
        }
    }

    #[test]
    fn simple_test() {
        let mut cfg: TestConfig = Default::default();
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

        let moves = plan_moves(&cfg, &repo).unwrap();
        assert_eq!(
            1,
            moves.len(),
            "moves should have exactly one element but was: {moves:?}"
        );

        let pb = PathBuf::from_str("some.mail").unwrap();
        let folder = folder_of(&moves, &pb).unwrap();
        assert_eq!("Trash", folder);
    }

    /// Same plan every run, whatever notmuch returns in which order.
    #[test]
    fn plan_is_sorted_by_message_file() {
        let cfg = TestConfig {
            rules: vec![Rule {
                folder: "Trash".to_string(),
                query: "tag:trash".to_string(),
                prefix: None,
            }],
            ..Default::default()
        };
        let mut repo: DummyRepo = Default::default();
        for f in ["b.mail", "a.mail", "c.mail"] {
            repo.add_mail(
                "NOT folder:\"Trash\" AND (tag:trash)".to_string(),
                f.to_string(),
            );
        }

        let moves = plan_moves(&cfg, &repo).unwrap();
        assert_eq!(
            vec!["a.mail", "b.mail", "c.mail"],
            moves
                .iter()
                .map(|(p, _)| p.to_str().unwrap())
                .collect::<Vec<_>>()
        );
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

        let mut cfg1: TestConfig = Default::default();
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
        cfg2.rule_match_mode = MatchMode::Unique;

        for cfg in &[cfg1, cfg2] {
            let moves = plan_moves(cfg, &repo);
            assert!(moves.is_err());
            let err = moves.unwrap_err();
            assert_eq!(
                "Ambiguous rule! Message already assigned to folder Trash, cannot assign to folder Deleted",
                err.to_string()
            );
        }
    }

    #[test]
    fn rule_match_mode_first_test() {
        let cfg = TestConfig {
            rule_match_mode: MatchMode::First,
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
            "(NOT folder:\"Trash\") AND (tag:trash)".to_string(),
            "some.mail".to_string(),
        );
        let moves = plan_moves(&cfg, &repo).unwrap();
        assert_eq!(moves.len(), 1);
        let pb = PathBuf::from_str("some.mail").unwrap();
        let folder = folder_of(&moves, &pb).unwrap();
        assert_eq!("Trash", folder);
    }

    #[test]
    fn rule_match_mode_all() {
        let cfg = TestConfig {
            rule_match_mode: MatchMode::All,
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
            "NOT folder:\"Deleted\" AND (tag:trash)".to_string(),
            "some.mail".to_string(),
        );
        let moves = plan_moves(&cfg, &repo).unwrap();
        assert_eq!(moves.len(), 1);
        let pb = PathBuf::from_str("some.mail").unwrap();
        let folder = folder_of(&moves, &pb).unwrap();
        assert_eq!("Deleted", folder);
    }

    #[test]
    fn rules_with_prefixes() {
        let cfg = TestConfig {
            rule_match_mode: MatchMode::Unique,
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

        let moves = plan_moves(&cfg, &repo).unwrap();
        assert_eq!(moves.len(), 2);

        let pb1 = PathBuf::from_str("~/mail/mailbox1/some.mail").unwrap();
        let pb2 = PathBuf::from_str("~/mail/mailbox2/some.mail").unwrap();

        let folder1 = folder_of(&moves, &pb1).unwrap();
        let folder2 = folder_of(&moves, &pb2).unwrap();

        assert_eq!("mailbox1/Trash", folder1);
        assert_eq!("mailbox2/Trash", folder2);
    }

    /// A message already sitting in the destination must not be planned again, or
    /// every run hands it a fresh UUID and mbsync loses flag continuity.
    #[test]
    fn all_mode_skips_mail_already_in_destination() {
        let cfg = TestConfig {
            rule_match_mode: MatchMode::All,
            rules: vec![Rule {
                folder: "Trash".to_string(),
                query: "tag:trash".to_string(),
                prefix: None,
            }],
            ..Default::default()
        };

        let mut repo: DummyRepo = Default::default();
        // notmuch returns the mail only for the unguarded query, i.e. the mail is
        // already in Trash and the guard is what keeps it out of the plan
        repo.add_mail("(tag:trash)".to_string(), "~/mail/Trash/x.mail".to_string());
        repo.add_mail(
            "NOT folder:\"Trash\" AND (tag:trash)".to_string(),
            "~/mail/INBOX/y.mail".to_string(),
        );

        let moves = plan_moves(&cfg, &repo).unwrap();
        assert_eq!(
            vec![PathBuf::from("~/mail/INBOX/y.mail")],
            moves.into_iter().map(|(p, _)| p).collect::<Vec<_>>()
        );
    }

    #[test]
    fn same_folder_for_two_rules_is_not_ambiguous() {
        let mut cfg: TestConfig = Default::default();
        for (folder, query) in [("Trash", "tag:trash"), ("Trash", "tag:junk")] {
            cfg.rules.push(Rule {
                folder: folder.to_string(),
                query: query.to_string(),
                prefix: None,
            });
        }

        let mut repo: DummyRepo = Default::default();
        repo.add_mail(
            "NOT folder:\"Trash\" AND (tag:trash)".to_string(),
            "trash.mail".to_string(),
        );
        repo.add_mail(
            "NOT folder:\"Trash\" AND (tag:junk)".to_string(),
            "trash.mail".to_string(),
        );

        let moves = plan_moves(&cfg, &repo).unwrap();
        assert_eq!(1, moves.len());
        assert_eq!(
            Some("Trash"),
            folder_of(&moves, Path::new("trash.mail")),
            "same destination from both rules is not ambiguous"
        );
    }

    #[test]
    fn prefix_is_honored_in_first_and_all_mode() {
        for mode in [MatchMode::First, MatchMode::All] {
            let cfg = TestConfig {
                rule_match_mode: mode,
                rules: vec![Rule {
                    folder: "mailbox1/Trash".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: Some("mailbox1".to_string()),
                }],
                ..Default::default()
            };

            let mut repo: DummyRepo = Default::default();
            let query = match mode {
                MatchMode::First => "(NOT folder:\"mailbox1/Trash\") AND (tag:trash)",
                _ => "NOT folder:\"mailbox1/Trash\" AND (tag:trash)",
            };
            repo.add_mail(query.to_string(), "~/mail/mailbox1/some.mail".to_string());
            repo.add_mail(query.to_string(), "~/mail/mailbox2/some.mail".to_string());

            let moves = plan_moves(&cfg, &repo).unwrap();
            assert_eq!(
                1,
                moves.len(),
                "{mode:?} must return only the mail under the prefix: {moves:?}"
            );
            assert!(folder_of(&moves, Path::new("~/mail/mailbox1/some.mail")).is_some());
        }
    }

    #[test]
    fn prefixes_are_matched_per_path_component() {
        let mut repo: DummyRepo = Default::default();
        // One mail matching both queries, but only inside the mailbox10 subtree: not a real
        // overlap, because "mailbox10" is not nested in "mailbox1".
        repo.add_mail(
            "(tag:trash)".to_string(),
            "~/mail/mailbox10/some.mail".to_string(),
        );

        let sibling_prefixes = TestConfig {
            rule_match_mode: MatchMode::Unique,
            rules: vec![
                Rule {
                    folder: "mailbox1/Trash".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: Some("mailbox1".to_string()),
                },
                Rule {
                    folder: "mailbox10/Trash".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: Some("mailbox10".to_string()),
                },
            ],
            ..Default::default()
        };
        assert!(plan_moves(&sibling_prefixes, &repo).is_ok());

        // Same query pair, but now nested prefixes: only the shared subtree can overlap.
        let nested_prefixes = TestConfig {
            rules: vec![
                sibling_prefixes.rules[0].clone(),
                Rule {
                    folder: "mailbox1/sub/Trash".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: Some("mailbox1/sub".to_string()),
                },
            ],
            ..sibling_prefixes
        };
        assert!(
            plan_moves(&nested_prefixes, &repo).is_ok(),
            "mail outside the shared subtree must not count as overlap"
        );

        repo.add_mail(
            "(tag:trash)".to_string(),
            "~/mail/mailbox1/sub/some.mail".to_string(),
        );
        let err = plan_moves(&nested_prefixes, &repo).unwrap_err();
        assert_eq!("Rules overlap (1 messages)", err.to_string());
    }

    #[test]
    fn missing_prefix_counts_as_maildir_root() {
        let mut repo: DummyRepo = Default::default();
        repo.add_mail(
            "(tag:trash)".to_string(),
            "~/mail/mailbox1/some.mail".to_string(),
        );

        let cfg = TestConfig {
            rule_match_mode: MatchMode::Unique,
            rules: vec![
                Rule {
                    folder: "Trash".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: Some("mailbox1".to_string()),
                },
                Rule {
                    folder: "Other".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: None,
                },
            ],
            ..Default::default()
        };
        let err = plan_moves(&cfg, &repo).unwrap_err();
        assert_eq!("Rules overlap (1 messages)", err.to_string());
    }

    #[test]
    fn unique_mode_does_not_grow_queries_with_rule_pairs() {
        let n = 10;
        let cfg = TestConfig {
            rules: (0..n)
                .map(|i| Rule {
                    folder: format!("folder{i}"),
                    query: format!("tag:tag{i}"),
                    prefix: None,
                })
                .collect(),
            ..Default::default()
        };

        let mut repo: DummyRepo = Default::default();
        for i in 0..n {
            repo.add_mail(format!("(tag:tag{i})"), format!("~/mail/folder{i}/mail{i}"));
        }
        // n overlap probes + n plan queries, not n(n-1)/2 + n.
        plan_moves(&cfg, &repo).unwrap();
        assert_eq!(2 * n, repo.search_count());
    }

    /// A message with two files in the same folder is returned twice per query; that
    /// is not the rule overlapping itself.
    #[test]
    fn duplicate_paths_in_one_result_are_not_an_overlap() {
        let cfg = TestConfig {
            rules: vec![
                Rule {
                    folder: "Trash".to_string(),
                    query: "tag:trash".to_string(),
                    prefix: None,
                },
                Rule {
                    folder: "Deleted".to_string(),
                    query: "tag:junk".to_string(),
                    prefix: None,
                },
            ],
            ..Default::default()
        };

        let mut repo: DummyRepo = Default::default();
        repo.add_mail("(tag:trash)".to_string(), "~/mail/a.mail".to_string());
        repo.add_mail("(tag:trash)".to_string(), "~/mail/a.mail".to_string());

        assert!(plan_moves(&cfg, &repo).is_ok());
    }
}
