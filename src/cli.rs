use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Use the provided config file instead of the default
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Configure the log level
    pub log_level: Option<String>,

    /// Enable dry-run mode, i.e. no files are being moved
    pub dry_run: Option<bool>,
}
