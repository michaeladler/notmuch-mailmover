//! Rule engine and file mover behind the `notmuch-mailmover` binary.
//!
//! The crate is split into a pure planning phase and a side-effecting phase:
//!
//! 1. [`engine::plan_moves`] evaluates the rules against a mail repository and
//!    returns the intended move for every matched message file. It touches no files.
//! 2. [`action::move_files`] performs those moves on disk, or only reports them
//!    when `dry_run` is set.
//!
//! Both phases are driven by a caller-supplied [`engine::Config`], so this crate
//! holds no configuration file of its own; the binary owns YAML/Lua loading.
//! Queries run through the [`repo::Repo`] trait, implemented for
//! `notmuch::Database` but replaceable, e.g. by tests.
//!
//! Note that applying rules is not idempotent: queries built from `folder:` or
//! `path:` can select a different set of messages on the next run, because the
//! first run changed the folders.
//!
//! # Example
//!
//! ```no_run
//! use nm_mailmover::{
//!     action,
//!     engine::{self, Config, MatchMode, Rule},
//!     repo::Repo,
//! };
//!
//! struct MyConfig {
//!     maildir: String,
//!     rules: Vec<Rule>,
//! }
//!
//! impl Config for MyConfig {
//!     fn maildir(&self) -> &str {
//!         &self.maildir
//!     }
//!     fn max_age_days(&self) -> Option<u32> {
//!         None
//!     }
//!     fn rename(&self) -> bool {
//!         false
//!     }
//!     fn rules(&self) -> &[Rule] {
//!         &self.rules
//!     }
//!     fn rule_match_mode(&self) -> MatchMode {
//!         MatchMode::Unique
//!     }
//! }
//!
//! let cfg = MyConfig {
//!     maildir: "/home/me/mail".to_string(),
//!     rules: vec![Rule {
//!         folder: "Trash".to_string(),
//!         query: "tag:trash".to_string(),
//!         prefix: None,
//!     }],
//! };
//!
//! // Any `Repo` works; `notmuch::Database` implements it with the `notmuch`
//! // feature.
//! struct MyRepo;
//!
//! impl Repo for MyRepo {
//!     fn search_messages(&self, query: &str) -> nm_mailmover::Result<Vec<std::path::PathBuf>> {
//!         Ok(Vec::new())
//!     }
//! }
//!
//! let moves = engine::plan_moves(&cfg, &MyRepo)?;
//! action::move_files(cfg.maildir(), cfg.rename(), false, &moves)?;
//! # Ok::<(), nm_mailmover::Error>(())
//! ```

pub mod action;
pub mod engine;
pub mod repo;

/// Result type used throughout this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong while planning or applying moves.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Two rules in `MatchMode::Unique` select the same message.
    #[error("Rules overlap ({0} messages)")]
    RuleOverlap(usize),

    /// One message ended up assigned to two different folders.
    #[error(
        "Ambiguous rule! Message already assigned to folder {old}, cannot assign to folder {new}"
    )]
    AmbiguousFolder { old: String, new: String },

    /// Message path has no file name component.
    #[error("Failed to get filename from {0}")]
    NoFileName(String),

    /// Message path has no mailbox (parent directory) component.
    #[error("Failed to get mailbox name from {0}")]
    NoMailboxName(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Only possible when writing into a `String`, i.e. never in practice.
    #[error(transparent)]
    Fmt(#[from] std::fmt::Error),

    #[cfg(feature = "notmuch")]
    #[error(transparent)]
    Notmuch(#[from] notmuch::Error),
}
