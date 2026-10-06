use std::path::PathBuf;

use argh::FromArgs;

use git_version::git_version;

pub const VERSION: &str = remove_leading_v(git_version!(
    cargo_prefix = "",
    prefix = "",
    // Note that on the CLI, the v* needs to be in single quotes
    // When passed here though there seems to be some magic quoting that happens.
    args = ["--always", "--dirty=-dirty", "--match=v*", "--tags"]
));

const fn remove_leading_v(version: &'static str) -> &'static str {
    if !version.is_empty() && version.as_bytes()[0] == b'v' {
        konst::string::str_from(version, 1)
    } else {
        version
    }
}

/// move notmuch tagged mails into Maildir folders
#[derive(FromArgs)]
pub struct Cli {
    /// use the provided config file instead of the default
    #[argh(option, short = 'c')]
    pub config: Option<PathBuf>,

    /// configure the log level
    #[argh(option, short = 'l', default = "LogLevel::default()")]
    pub log_level: LogLevel,

    /// enable dry-run mode, i.e. no files are being moved
    #[argh(switch, short = 'd')]
    pub dry_run: bool,

    /// print version information
    #[argh(switch, short = 'V')]
    pub version: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(format!(
                "invalid log level {s:?}, expected one of trace, debug, info, warn, error"
            )),
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{self:?}").to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use argh::FromArgs;

    fn parse(args: &[&str]) -> Result<Cli, String> {
        Cli::from_args(&["notmuch-mailmover"], args).map_err(|e| e.output)
    }

    #[test]
    fn defaults_without_arguments() {
        let cli = parse(&[]).unwrap();
        assert_eq!(cli.config, None);
        assert_eq!(cli.log_level, LogLevel::Info);
        assert!(!cli.dry_run);
        assert!(!cli.version);
    }

    #[test]
    fn parses_every_option() {
        let cli = parse(&[
            "--config",
            "/tmp/cfg.yaml",
            "--log-level",
            "debug",
            "--dry-run",
            "--version",
        ])
        .unwrap();
        assert_eq!(cli.config, Some(PathBuf::from("/tmp/cfg.yaml")));
        assert_eq!(cli.log_level, LogLevel::Debug);
        assert!(cli.dry_run);
        assert!(cli.version);

        let short = parse(&["-c", "cfg.yaml", "-l", "trace", "-d", "-V"]).unwrap();
        assert_eq!(short.config, Some(PathBuf::from("cfg.yaml")));
        assert_eq!(short.log_level, LogLevel::Trace);
        assert!(short.dry_run);
        assert!(short.version);
    }

    #[test]
    fn rejects_unknown_arguments() {
        assert!(parse(&["--nope"]).is_err());
        assert!(parse(&["--log-level"]).is_err(), "missing value");
    }

    #[test]
    fn version_has_no_leading_v() {
        // With no matching tag in reach, `git describe --always` yields a short
        // hash, so only the missing "v" is guaranteed here.
        assert!(!VERSION.is_empty(), "empty version");
        assert!(!VERSION.starts_with('v'), "unexpected version {VERSION:?}");
    }

    #[test]
    fn strips_only_the_leading_v() {
        assert_eq!(remove_leading_v("v1.2.3"), "1.2.3");
        assert_eq!(remove_leading_v("1.2.3-dirty"), "1.2.3-dirty");
        assert_eq!(remove_leading_v(""), "");
        assert_eq!(remove_leading_v("v"), "");
        assert_eq!(remove_leading_v("version"), "ersion");
    }

    #[test]
    fn log_level_parses_case_insensitively() {
        for (input, expected) in [
            ("trace", LogLevel::Trace),
            ("DEBUG", LogLevel::Debug),
            ("Info", LogLevel::Info),
            ("warn", LogLevel::Warn),
            ("error", LogLevel::Error),
        ] {
            assert_eq!(input.parse::<LogLevel>().unwrap(), expected, "{input}");
        }
    }

    #[test]
    fn log_level_rejects_unknown_names() {
        let err = "verbose".parse::<LogLevel>().unwrap_err();
        assert_eq!(
            err,
            "invalid log level \"verbose\", expected one of trace, debug, info, warn, error"
        );
    }

    #[test]
    fn log_level_round_trips_through_display() {
        for level in [
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ] {
            assert_eq!(level.to_string().parse::<LogLevel>().unwrap(), level);
        }
        assert_eq!(LogLevel::default().to_string(), "info");
    }
}
