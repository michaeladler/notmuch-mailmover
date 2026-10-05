use std::fs::{self, File};
use std::{
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use directories::BaseDirs;
use log::debug;
#[cfg(feature = "lua")]
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
    pub rule_match_mode: MatchMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            maildir: "~/mail".to_string(),
            notmuch_config: None,
            rename: false,
            max_age_days: None,
            rules: Vec::new(),
            rule_match_mode: MatchMode::Unique,
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
    fn rule_match_mode(&self) -> MatchMode {
        self.rule_match_mode
    }
}

/// Loads the config from `fname`, or from the default location. If no default
/// config exists yet, it is written there first.
pub fn load_or_create_config(fname: &Option<PathBuf>) -> Result<Config> {
    let bd = BaseDirs::new().context("could not determine config directory")?;
    let basedir = bd.config_dir().join("notmuch-mailmover");
    let default_cfg_path = basedir.join("config.yaml");

    let fname = match fname {
        Some(fname) => fname.clone(),
        #[cfg(feature = "lua")]
        None => {
            let lua_path = basedir.join("config.lua");
            match (default_cfg_path.exists(), lua_path.exists()) {
                (true, true) => {
                    return Err(anyhow!(
                        "Both {} and {} exist, please remove one",
                        default_cfg_path.to_string_lossy(),
                        lua_path.to_string_lossy(),
                    ));
                }
                (true, false) => default_cfg_path,
                (false, true) => lua_path,
                (false, false) => {
                    write_default_cfg(&default_cfg_path)?;
                    default_cfg_path
                }
            }
        }
        #[cfg(not(feature = "lua"))]
        None => {
            if !default_cfg_path.exists() {
                write_default_cfg(&default_cfg_path)?;
            }
            default_cfg_path
        }
    };
    debug!("loading config {fname:?}");

    let mut cfg: Config = if fname.extension().is_some_and(|ext| ext == "lua") {
        load_lua(&fname, &basedir)?
    } else {
        let f = File::open(&fname)?;
        let reader = BufReader::new(f);
        yaml_serde::from_reader(reader)?
    };

    let maildir = shellexpand::full(&cfg.maildir)?;
    cfg.maildir = maildir.to_string();

    if let Some(p) = &mut cfg.notmuch_config {
        *p = shellexpand::full(p)?.to_string();
    }

    Ok(cfg)
}

fn write_default_cfg(path: &Path) -> Result<()> {
    fs::create_dir_all(path.parent().expect("config path has no parent"))?;
    yaml_serde::to_writer(File::create(path)?, &Config::default())?;
    Ok(())
}

#[cfg(feature = "lua")]
fn load_lua(fname: &Path, basedir: &Path) -> Result<Config> {
    let lua = Lua::new();
    let moduledir = fname
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(basedir)
        .to_string_lossy();
    lua.load(format!(
        "package.path = package.path .. ';{moduledir}/?.lua;{moduledir}/?/init.lua;;'"
    ))
    .exec()?;
    let val = lua.load(fname).eval()?;
    Ok(lua.from_value(val)?)
}

#[cfg(not(feature = "lua"))]
fn load_lua(fname: &Path, _: &Path) -> Result<Config> {
    Err(anyhow!(
        "{} needs Lua support, rebuild with `--features lua`",
        fname.display()
    ))
}
