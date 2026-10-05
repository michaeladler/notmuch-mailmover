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

#[derive(Clone, Debug, Default)]
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
