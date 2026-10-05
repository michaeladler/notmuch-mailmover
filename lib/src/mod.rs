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
//!     fn search_messages(&self, query: &str) -> anyhow::Result<Vec<std::path::PathBuf>> {
//!         Ok(Vec::new())
//!     }
//! }
//!
//! let moves = engine::plan_moves(&cfg, &MyRepo)?;
//! action::move_files(&cfg, false, &moves)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod action;
pub mod engine;
pub mod repo;
