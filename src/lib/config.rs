use std::fs::{self, File};
use std::{io::BufReader, path::PathBuf};

use anyhow::{anyhow, Result};
use directories::BaseDirs;
use log::debug;
use mlua::{Lua, LuaSerdeExt};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub maildir: String,
    /// if omitted, it will use the same as notmuch would, see notmuch-config(1)
    pub notmuch_config: Option<String>,
    pub rename: bool,
    pub max_age_days: Option<u32>,
    pub rules: Vec<Rule>,
    pub rule_match_mode: Option<MatchMode>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MatchMode {
    Unique,
    First,
    All,
}

impl Serialize for MatchMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        use MatchMode::*;
        match self {
            Unique => serializer.serialize_str("unique"),
            First => serializer.serialize_str("first"),
            All => serializer.serialize_str("all"),
        }
    }
}

impl<'de> Deserialize<'de> for MatchMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MatchModeVisitor;

        impl serde::de::Visitor<'_> for MatchModeVisitor {
            type Value = MatchMode;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string representing a match mode")
            }

            fn visit_str<E>(self, value: &str) -> Result<MatchMode, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "unique" => Ok(MatchMode::Unique),
                    "first" => Ok(MatchMode::First),
                    "all" => Ok(MatchMode::All),
                    _ => Err(E::custom(format!("unknown match mode: {}", value))),
                }
            }
        }

        deserializer.deserialize_str(MatchModeVisitor)
    }
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Rule {
    pub folder: String,
    pub query: String,
    pub prefix: Option<String>,
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Rule {}{}: {}",
            self.folder,
            self.prefix
                .as_ref()
                .map(|m| format!(" with prefix '{}'", m))
                .unwrap_or_default(),
            self.query
        )
    }
}

pub fn load_config(fname: &Option<PathBuf>) -> Result<Config> {
    let bd = BaseDirs::new().unwrap();
    let basedir = bd.config_dir().join("notmuch-mailmover");
    let default_cfg_path = basedir.join("config.yaml");
    let default_lua_path = basedir.join("config.lua");

    let fname: &PathBuf = match fname {
        Some(fname) => fname,
        None => match (default_cfg_path.exists(), default_lua_path.exists()) {
            (true, true) => {
                return Err(anyhow!(
                    "Both {} and {} exist, please remove one",
                    default_cfg_path.to_string_lossy(),
                    default_lua_path.to_string_lossy(),
                ));
            }
            (true, false) => &default_cfg_path,
            (false, true) => &default_lua_path,
            (false, false) => {
                fs::create_dir_all(&basedir)?;
                let f = File::create(&default_cfg_path)?;
                let default_cfg: Config = Default::default();
                serde_yml::to_writer(f, &default_cfg)?;
                &default_cfg_path
            }
        },
    };
    debug!("loading config {:?}", fname);

    let mut cfg = if fname.extension().is_some_and(|ext| ext == "lua") {
        let lua = Lua::new();
        let basedir = basedir.to_string_lossy();
        lua.load(format!(
            "package.path = package.path .. ';{}/?.lua;{}/?/init.lua;;'",
            basedir, basedir
        ))
        .exec()?;
        let val = lua.load(fname.clone()).eval()?;
        let cfg: Config = lua.from_value(val)?;
        cfg
    } else {
        let f = File::open(fname)?;
        let reader = BufReader::new(f);
        let cfg: Config = serde_yml::from_reader(reader)?;
        cfg
    };

    let db_path = shellexpand::full(&cfg.maildir)?;
    cfg.maildir = db_path.to_string();

    if let Some(cfg_path) = cfg.notmuch_config {
        let path = shellexpand::full(&cfg_path)?;
        cfg.notmuch_config = Some(path.to_string());
    }

    Ok(cfg)
}
