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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Serializes the tests that mutate the environment, so they cannot observe
    /// each other's `XDG_CONFIG_HOME`.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Throwaway directory, removed on drop. The name is unique per test and
    /// process, so a crashed test only leaves debris for its own next run.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("nmm-{}-{tag}-{n}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// `$HOME`, or `None` where the environment has none, in which case
    /// `shellexpand` leaves a leading `~` alone.
    fn home() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }

    /// What `shellexpand` makes of a path starting with `~`.
    fn expanded(name: &str) -> String {
        home()
            .map(|h| h.join(name).to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("~/{name}"))
    }

    fn yaml(body: &str) -> String {
        format!("maildir: ~/mail\n{body}")
    }

    /// [`Config::default`] as it comes back from
    /// [`load_or_create_config`], i.e. with `maildir` expanded.
    fn loaded_default() -> Config {
        Config {
            maildir: expanded("mail"),
            ..Config::default()
        }
    }

    #[test]
    fn default_config_is_the_documented_one() {
        let cfg = Config::default();
        assert_eq!(cfg.maildir, "~/mail");
        assert_eq!(cfg.notmuch_config, None);
        assert!(!cfg.rename);
        assert_eq!(cfg.max_age_days, None);
        assert!(cfg.rules.is_empty());
        assert_eq!(cfg.rule_match_mode, MatchMode::Unique);
    }

    #[test]
    fn engine_config_reads_the_fields() {
        let cfg = Config {
            maildir: "/var/mail".to_string(),
            notmuch_config: Some("/etc/notmuchrc".to_string()),
            rename: true,
            max_age_days: Some(7),
            rules: vec![Rule {
                folder: "Trash".to_string(),
                query: "tag:trash".to_string(),
                prefix: None,
            }],
            rule_match_mode: MatchMode::All,
        };
        assert_eq!(engine::Config::maildir(&cfg), "/var/mail");
        assert_eq!(engine::Config::max_age_days(&cfg), Some(7));
        assert_eq!(engine::Config::rules(&cfg), &cfg.rules);
        assert_eq!(engine::Config::rule_match_mode(&cfg), MatchMode::All);

        let defaults = Config::default();
        assert_eq!(engine::Config::max_age_days(&defaults), None);
        assert!(engine::Config::rules(&defaults).is_empty());
    }

    #[test]
    fn missing_fields_fall_back_to_the_defaults() {
        let cfg = load_or_create_config(&Some(CfgFile::new("empty", "").path().into())).unwrap();
        assert_eq!(cfg, loaded_default());
    }

    #[test]
    fn reads_all_fields() {
        let dir = TempDir::new("all-fields");
        let path = dir.join("config.yaml");
        fs::write(
            &path,
            yaml(
                "notmuch_config: ~/.notmuchrc
rename: true
max_age_days: 30
rule_match_mode: all
rules:
  - folder: Trash
    query: tag:trash
    prefix: INBOX
",
            ),
        )
        .unwrap();

        let cfg = load_or_create_config(&Some(path)).unwrap();
        assert_eq!(cfg.maildir, expanded("mail"));
        assert_eq!(cfg.notmuch_config, Some(expanded(".notmuchrc")));
        assert!(cfg.rename);
        assert_eq!(cfg.max_age_days, Some(30));
        assert_eq!(cfg.rule_match_mode, MatchMode::All);
        assert_eq!(cfg.rules.len(), 1);
        assert_eq!(cfg.rules[0].folder, "Trash");
        assert_eq!(cfg.rules[0].prefix.as_deref(), Some("INBOX"));
    }

    #[test]
    fn paths_are_shell_expanded() {
        let dir = TempDir::new("expand");
        let path = dir.join("config.yaml");
        fs::write(&path, yaml("notmuch_config: /etc/notmuchrc\n")).unwrap();

        let cfg = load_or_create_config(&Some(path)).unwrap();
        assert_eq!(cfg.maildir, expanded("mail"));
        assert_eq!(
            cfg.notmuch_config,
            Some("/etc/notmuchrc".to_string()),
            "a path without ~ must stay as is"
        );
    }

    #[test]
    fn missing_file_is_an_error() {
        let dir = TempDir::new("missing");
        let err = load_or_create_config(&Some(dir.join("nope.yaml"))).unwrap_err();
        assert!(
            format!("{err:#}").contains("No such file"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn broken_yaml_is_an_error() {
        let dir = TempDir::new("broken");
        let path = dir.join("config.yaml");
        fs::write(&path, "maildir: [\n").unwrap();
        assert!(load_or_create_config(&Some(path)).is_err());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let dir = TempDir::new("unknown");
        let path = dir.join("config.yaml");
        fs::write(&path, yaml("no_such_field: 42\n")).unwrap();
        assert!(load_or_create_config(&Some(path)).is_ok());
    }

    /// A config file in a temp dir. The dir lives as long as the value.
    struct CfgFile {
        _dir: TempDir,
        path: PathBuf,
    }

    impl CfgFile {
        /// `body` is appended to a `maildir` line, so an empty `body` yields an
        /// all-defaults config.
        fn new(tag: &str, body: &str) -> Self {
            let dir = TempDir::new(tag);
            let path = dir.join("config.yaml");
            fs::write(&path, yaml(body)).unwrap();
            Self { _dir: dir, path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    /// Points `XDG_CONFIG_HOME` at an empty directory for the duration of `f`.
    fn with_config_home<T>(f: impl FnOnce(&Path) -> T) -> T {
        // Restores `XDG_CONFIG_HOME`, so the other tests keep seeing the real one.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new("config-home");
        let old = std::env::var_os("XDG_CONFIG_HOME");
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir.0) };
        let out = f(&dir.0);
        unsafe {
            match old {
                Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }
        out
    }

    #[test]
    fn default_config_is_written_when_missing() {
        with_config_home(|home| {
            let cfg = load_or_create_config(&None).unwrap();
            assert_eq!(cfg, loaded_default());
            let written = home.join("notmuch-mailmover").join("config.yaml");
            assert!(written.exists(), "the default config must be written");

            // and the second call reads it back
            assert_eq!(load_or_create_config(&None).unwrap(), cfg);
        });
    }

    #[test]
    fn existing_default_yaml_is_read() {
        with_config_home(|home| {
            let basedir = home.join("notmuch-mailmover");
            fs::create_dir_all(&basedir).unwrap();
            fs::write(
                basedir.join("config.yaml"),
                yaml("rename: true\nnotmuch_config: ~/.notmuchrc\n"),
            )
            .unwrap();
            #[cfg(feature = "lua")]
            assert!(!basedir.join("config.lua").exists());

            let cfg = load_or_create_config(&None).unwrap();
            assert!(cfg.rename);
            assert_eq!(cfg.notmuch_config, Some(expanded(".notmuchrc")));
        });
    }

    #[cfg(feature = "lua")]
    #[test]
    fn two_default_configs_are_an_error() {
        with_config_home(|home| {
            let basedir = home.join("notmuch-mailmover");
            fs::create_dir_all(&basedir).unwrap();
            fs::write(basedir.join("config.yaml"), yaml("")).unwrap();
            fs::write(
                basedir.join("config.lua"),
                "return { maildir = '/var/mail' }",
            )
            .unwrap();

            let err = load_or_create_config(&None).unwrap_err();
            let msg = format!("{err:#}");
            assert!(msg.contains("please remove one"), "unexpected error: {msg}");
        });
    }

    #[cfg(feature = "lua")]
    #[test]
    fn reads_lua_config() {
        let dir = TempDir::new("lua");
        let path = dir.join("config.lua");
        fs::write(
            &path,
            r#"
            return {
              maildir = "~/mail",
              notmuch_config = "~/.notmuchrc",
              rename = true,
              max_age_days = 12,
              rule_match_mode = "first",
              rules = { { folder = "Trash", query = "tag:trash" } },
            }
            "#,
        )
        .unwrap();

        let cfg = load_or_create_config(&Some(path.clone())).unwrap();
        assert_eq!(cfg.maildir, expanded("mail"));
        assert_eq!(cfg.notmuch_config, Some(expanded(".notmuchrc")));
        assert!(cfg.rename);
        assert_eq!(cfg.max_age_days, Some(12));
        assert_eq!(cfg.rule_match_mode, MatchMode::First);
        assert_eq!(cfg.rules.len(), 1);
        assert_eq!(cfg.rules[0].query, "tag:trash");

        // a bare name resolves against the cwd, not `basedir`
        assert!(load_lua(Path::new("config.lua"), &dir.0).is_err());
    }

    #[cfg(feature = "lua")]
    #[test]
    fn lua_modules_are_found_next_to_the_config() {
        let dir = TempDir::new("lua-module");
        fs::write(dir.join("helper.lua"), "return { extra = 1 }").unwrap();
        let path = dir.join("config.lua");
        fs::write(
            &path,
            "local helper = require('helper')\n\
             return { maildir = '~/mail', rename = helper.extra == 1 }",
        )
        .unwrap();

        assert!(load_or_create_config(&Some(path)).unwrap().rename);
    }

    #[cfg(feature = "lua")]
    #[test]
    fn broken_lua_config_is_an_error() {
        let dir = TempDir::new("lua-broken");
        let path = dir.join("config.lua");
        fs::write(&path, "return {").unwrap();
        assert!(load_or_create_config(&Some(path)).is_err());
    }

    #[cfg(feature = "lua")]
    #[test]
    fn default_lua_config_is_used_when_it_is_the_only_one() {
        with_config_home(|home| {
            let basedir = home.join("notmuch-mailmover");
            fs::create_dir_all(&basedir).unwrap();
            fs::write(
                basedir.join("config.lua"),
                "return { maildir = '/var/mail' }",
            )
            .unwrap();

            let cfg = load_or_create_config(&None).unwrap();
            assert_eq!(cfg.maildir, "/var/mail");
            assert!(!basedir.join("config.yaml").exists());
        });
    }

    #[cfg(not(feature = "lua"))]
    #[test]
    fn lua_config_needs_the_feature() {
        let dir = TempDir::new("no-lua");
        let path = dir.join("config.lua");
        fs::write(&path, "return {}").unwrap();
        let err = load_or_create_config(&Some(path.clone())).unwrap_err();
        assert!(
            format!("{err:#}").contains("needs Lua support"),
            "unexpected error: {err:#}"
        );
        assert!(load_lua(&path, dir.0.as_path()).is_err());
    }
}
