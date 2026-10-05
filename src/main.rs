use anyhow::{Context, Result};
use clap::Parser;
use env_logger::Env;
use log::{debug, info};
use std::time::Instant;

mod cli;
mod config;

use nm_mailmover::{action, engine};

fn main() -> Result<()> {
    let opts = cli::Cli::parse();

    let env = Env::default().default_filter_or(opts.log_level.to_string());
    env_logger::try_init_from_env(env)?;

    let cfg = config::load_config(&opts.config)?;
    debug!("successfully loaded {cfg:?}");

    debug!("opening notmuch db");
    let db = notmuch::Database::open_with_config(
        None::<&str>,
        notmuch::DatabaseMode::ReadOnly,
        cfg.notmuch_config.as_ref(),
        None,
    )
    .context("failed to open notmuch database")?;
    debug!("successfully opened notmuch db");

    let start = Instant::now();

    let actions = engine::apply_rules(&cfg, &db).context("failed to apply rules")?;
    action::apply_actions(&cfg, opts.dry_run, &actions).context("failed to apply actions")?;

    let duration = start.elapsed();
    info!("execution took {duration:?}");

    Ok(())
}
