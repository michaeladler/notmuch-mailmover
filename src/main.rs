mod action;
mod cli;
mod config;
mod engine;
mod repo;

use clap::Parser;
use std::time::Instant;

use anyhow::Result;
use env_logger::Env;
use log::{debug, info};

fn main() -> Result<()> {
    let opts = cli::Cli::parse();

    let env = match &opts.log_level {
        Some(log_level) => Env::default().default_filter_or(log_level),
        None => Env::default().default_filter_or("info"),
    };
    env_logger::try_init_from_env(env)?;

    let cfg = config::load_config(&opts.config)?;
    let db_path: Option<String> = None;
    debug!("loaded {:?}", cfg);
    let db = notmuch::Database::open_with_config(
        db_path,
        notmuch::DatabaseMode::ReadOnly,
        cfg.notmuch_config.as_ref(),
        None,
    )?;

    let start = Instant::now();

    let actions = engine::apply_rules(&cfg, &db)?;
    let dry_run = opts.dry_run.unwrap_or_default();
    action::apply_actions(&cfg, dry_run, &actions)?;

    let duration = start.elapsed();
    info!("execution took {:?}", duration);

    Ok(())
}
