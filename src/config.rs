use std::fs::{self, File};
use std::{io::BufReader, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use directories::BaseDirs;
use log::debug;
use mlua::{Lua, LuaSerdeExt};
use serde::{Deserialize, Serialize};

use nm_mailmover::engine::{self, MatchMode, Rule};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub maildir: String,
    /// if omitted, it will use the same as notmuch would, see notmuch-config(1)
    pub notmuch_config: Option<String>,
    pub rename: bool,
    pub max_age_days: Option<u32>,
    pub rules: Vec<Rule>,
    pub rule_match_mode: Option<MatchMode>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            maildir: "~/mail".to_string(),
            notmuch_config: None,
            rename: false,
            max_age_days: None,
            rules: Vec::new(),
            rule_match_mode: None,
        }
    }
}

impl engine::Config for Config {
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
    fn rule_match_mode(&self) -> Option<MatchMode> {
        self.rule_match_mode
    }
}

pub fn load_config(fname: &Option<PathBuf>) -> Result<Config> {
    let bd = BaseDirs::new().context("could not determine config directory")?;
    let basedir = bd.config_dir().join("notmuch-mailmover");
    let default_cfg_path = basedir.join("config.yaml");
    let default_lua_path = basedir.join("config.lua");

    let fname = match fname {
        Some(fname) => fname.clone(),
        None => match (default_cfg_path.exists(), default_lua_path.exists()) {
            (true, true) => {
                return Err(anyhow!(
                    "Both {} and {} exist, please remove one",
                    default_cfg_path.to_string_lossy(),
                    default_lua_path.to_string_lossy(),
                ));
            }
            (true, false) => default_cfg_path,
            (false, true) => default_lua_path,
            (false, false) => {
                fs::create_dir_all(&basedir)?;
                let f = File::create(&default_cfg_path)?;
                let default_cfg: Config = Default::default();
                yaml_serde::to_writer(f, &default_cfg)?;
                default_cfg_path
            }
        },
    };
    debug!("loading config {fname:?}");

    let mut cfg: Config = if fname.extension().is_some_and(|ext| ext == "lua") {
        let lua = Lua::new();
        let moduledir = fname
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(&basedir)
            .to_string_lossy();
        lua.load(format!(
            "package.path = package.path .. ';{moduledir}/?.lua;{moduledir}/?/init.lua;;'"
        ))
        .exec()?;
        let val = lua.load(fname.clone()).eval()?;
        lua.from_value(val)?
    } else {
        let f = File::open(&fname)?;
        let reader = BufReader::new(f);
        yaml_serde::from_reader(reader)?
    };

    let db_path = shellexpand::full(&cfg.maildir)?;
    cfg.maildir = db_path.to_string();

    if let Some(cfg_path) = cfg.notmuch_config {
        let path = shellexpand::full(&cfg_path)?;
        cfg.notmuch_config = Some(path.to_string());
    }

    Ok(cfg)
}
