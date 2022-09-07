use std::fs::{self, File};
use std::{io::BufReader, path::PathBuf};

use anyhow::{anyhow, Result};
use directories::BaseDirs;
use log::debug;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub database_path: String,
    pub config_path: String,
    pub rename: bool,
    pub max_age_days: Option<u32>,
    pub rules: Vec<Rule>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_path: "~/mail".to_string(),
            config_path: "~/.notmuchrc".to_string(),
            rename: false,
            max_age_days: None,
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rule {
    pub folder: String,
    pub query: String,
}

pub fn load_config(fname: &Option<PathBuf>) -> Result<Config> {
    let bd = BaseDirs::new().unwrap();
    let basedir = bd.config_dir().join("notmuch-mailmover");
    let default_cfg_path = basedir.join("config.yaml");

    let fname: &PathBuf = match fname {
        Some(fname) => fname,
        None => {
            if !default_cfg_path.exists() {
                fs::create_dir_all(basedir)?;
                let f = File::create(&default_cfg_path)?;
                let default_cfg: Config = Default::default();
                serde_yaml::to_writer(f, &default_cfg)?;
            }
            &default_cfg_path
        }
    };

    debug!("loading config {:?}", fname);
    match File::open(&fname) {
        Ok(f) => {
            let reader = BufReader::new(f);
            let mut cfg: Config = serde_yaml::from_reader(reader)?;

            let db_path = shellexpand::full(&cfg.database_path)?;
            cfg.database_path = db_path.to_string();

            let cfg_path = shellexpand::full(&cfg.config_path)?;
            cfg.config_path = cfg_path.to_string();

            Ok(cfg)
        }
        Err(e) => Err(anyhow!("Failed to open {}: {}", fname.to_string_lossy(), e)),
    }
}
