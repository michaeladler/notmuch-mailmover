use anyhow::Result;
use clap::Parser;
use env_logger::Env;
use log::{debug, info};
use std::time::Instant;

use nm_mailmover::{action, cli, config, engine};

fn main() -> Result<()> {
    let opts = cli::Cli::parse();

    let env = Env::default().default_filter_or(opts.log_level.to_string());
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
    action::apply_actions(&cfg, opts.dry_run, &actions)?;

    let duration = start.elapsed();
    info!("execution took {:?}", duration);

    Ok(())
}
