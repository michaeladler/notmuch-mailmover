use core::str;
use std::path::PathBuf;

use clap::{crate_authors, Parser, ValueEnum};

use git_version::git_version;

pub const VERSION: &[u8] = remove_leading_v(
    git_version!(
        cargo_prefix = "",
        prefix = "",
        // Note that on the CLI, the v* needs to be in single quotes
        // When passed here though there seems to be some magic quoting that happens.
        args = ["--always", "--dirty=-dirty", "--match=v*", "--tags"]
    )
    .as_bytes(),
);

const fn remove_leading_v(bytes: &[u8]) -> &[u8] {
    if !bytes.is_empty() && bytes[0] == b'v' {
        konst::slice::slice_from(bytes, 1)
    } else {
        bytes
    }
}

#[derive(Parser)]
#[clap(
    name = "notmuch-mailmover",
    version = str::from_utf8( VERSION).unwrap(),
    author = crate_authors!(),
)]
pub struct Cli {
    /// Use the provided config file instead of the default
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Configure the log level
    #[clap(short,long, value_parser, ignore_case = true, value_enum, default_value_t = Default::default())]
    pub log_level: LogLevel,

    /// Enable dry-run mode, i.e. no files are being moved
    #[clap(short, long, action)]
    pub dry_run: bool,
}

#[derive(Clone, ValueEnum, Debug, Default)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self).to_ascii_lowercase())
    }
}
