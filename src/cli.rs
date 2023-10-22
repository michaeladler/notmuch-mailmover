use std::path::PathBuf;

use clap::{crate_authors, crate_version, Parser, ValueEnum};

#[derive(Parser)]
#[clap(
    name = "notmuch-mailmover",
    version = crate_version!(),
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
    #[default]
    Info,
    Debug,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self).to_ascii_lowercase())
    }
}
