mod action;
mod config;
mod engine;
mod repo;

use std::{path::PathBuf, time::Instant};

use anyhow::Result;
use env_logger::Env;
use gumdrop::Options;
use log::debug;

#[derive(Debug, Options)]
struct CliArgs {
    // Boolean options are treated as flags, taking no additional values.
    // The optional `help` attribute is displayed in `usage` text.
    #[options(help = "print help message")]
    help: bool,

    #[options(help = "use the given config file", meta = "FILE")]
    config: Option<PathBuf>,

    #[options(help = "set log level", meta = "LEVEL")]
    log_level: Option<String>,

    #[options(no_short, help = "enable dry run")]
    dry_run: bool,
}

fn main() -> Result<()> {
    let opts = CliArgs::parse_args_default_or_exit();

    let env = match &opts.log_level {
        Some(log_level) => Env::default().default_filter_or(log_level),
        None => Env::default().default_filter_or("info"),
    };
    env_logger::try_init_from_env(env)?;

    debug!("{:?}", &opts);

    let cfg = config::load_config(&opts.config)?;
    debug!("loaded {:?}", cfg);
    let db = notmuch::Database::open_with_config(
        Some(&cfg.database_path),
        notmuch::DatabaseMode::ReadOnly,
        Some(&cfg.config_path),
        None,
    )?;

    let start = Instant::now();

    let actions = engine::apply_rules(&cfg, &db)?;
    action::apply_actions(&cfg, opts.dry_run, &actions)?;

    let duration = start.elapsed();
    debug!("execution took {:?}", duration);

    Ok(())
}
