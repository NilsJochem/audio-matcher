use std::time::Duration;

use clap::Args;
use log::info;

#[derive(Args, Debug, Clone, Copy)]
#[group(required = false, multiple = false)]
pub struct Inputs {
    #[clap(short, help = "always answer yes")]
    pub yes: bool,
    #[clap(short, help = "always answer no")]
    pub no: bool,
    #[clap(long, default_value_t = 3, help = "number of retrys")]
    pub trys: u8,
}
impl Inputs {
    #[must_use]
    pub fn ask_consent(&self, msg: &str) -> bool {
        if self.yes || self.no {
            return self.yes;
        }
        self.try_input(&format!("{msg} [y/n]: "), None, |rin| {
            if ["y", "yes", "j", "ja"].contains(&rin.as_str()) {
                return Some(true);
            } else if ["n", "no", "nein"].contains(&rin.as_str()) {
                return Some(false);
            }
            None
        })
        .unwrap_or_else(|| {
            info!("probably not");
            false
        })
    }

    pub fn try_input<T>(
        &self,
        msg: &str,
        default: Option<T>,
        mut map: impl FnMut(String) -> Option<T>,
    ) -> Option<T> {
        print!("{msg}");
        for _ in 0..self.trys {
            let rin: String = text_io::read!("{}\n");
            if default.is_some() && rin.is_empty() {
                return default;
            }
            match map(rin) {
                Some(t) => return Some(t),
                None => print!("couldn't parse that, please try again: "),
            }
        }
        None
    }
    #[must_use]
    pub fn input(&self, msg: &str, default: Option<String>) -> String {
        self.try_input(msg, default, Some)
            .unwrap_or_else(|| unreachable!())
    }
}

#[derive(Args, Debug, Clone, Copy)]
#[group(required = false, multiple = false)]
#[allow(clippy::struct_excessive_bools)]
pub struct OutputLevel {
    #[clap(short, long, help = "print maximum info")]
    debug: bool,
    #[clap(short, long, help = "print more info")]
    verbose: bool,
    #[clap(short, long, help = "print sligtly more info")]
    warn: bool,
    #[clap(short, long, help = "print almost no info")]
    silent: bool,
}

impl OutputLevel {
    pub fn init_logger(&self) {
        let level = log::Level::from(*self);
        Self::init_logger_with(level);
    }
    pub fn init_logger_with(level: log::Level) {
        let env = env_logger::Env::default();
        let env = env.default_filter_or(level.as_str());

        let mut builder = env_logger::Builder::from_env(env);

        builder.format_timestamp(None);
        builder.format_target(false);
        builder.format_level(level < log::Level::Info);

        builder.init();
    }
}

impl From<OutputLevel> for log::Level {
    fn from(val: OutputLevel) -> Self {
        if val.silent {
            Self::Error
        } else if val.verbose {
            Self::Trace
        } else if val.debug {
            Self::Debug
        } else if val.warn {
            Self::Warn
        } else {
            Self::Info
        }
    }
}

use regex::Regex;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("couldn't find duration in {0:?}")]
pub struct NoMatch(String);
// TODO activate when Issue #67295 is finished
// #[cfg(doctest)]
impl NoMatch {
    /// only used for doctest
    #[must_use]
    pub fn new(s: &str) -> Self {
        Self(s.to_owned())
    }
}
/// parses a duration from `arg`, which can be just seconds, or somthing like `"3h5m17s"` or `"3hours6min1sec"`
/// # Example
/// ```
/// use std::time::Duration;
/// use audio_matcher::args::{NoMatch, parse_duration};
///
/// assert_eq!(Ok(Duration::from_secs(17)), parse_duration("17"), "blank seconds");
/// assert_eq!(Ok(Duration::from_secs(58)), parse_duration("58sec"), "seconds with identifier");
/// assert_eq!(Ok(Duration::from_secs(60)), parse_duration("1m"), "minutes without seconds");
/// assert_eq!(Ok(Duration::from_millis(100)), parse_duration("100ms"), "milliseconds");
/// assert_eq!(Ok(Duration::from_secs(3661)), parse_duration("1hour1m1s"), "hours, minutes and seconds");
///
/// assert_eq!(Err(NoMatch::new("")), parse_duration(""), "fail the empty string");
/// assert_eq!(Err(NoMatch::new("3abc")), parse_duration("3abc"), "fail random letters");
/// assert_eq!(Err(NoMatch::new("3s5m")), parse_duration("3s5m"), "fail wrong order");
/// ```
pub fn parse_duration(arg: &str) -> Result<std::time::Duration, NoMatch> {
    lazy_static::lazy_static! {
        static ref RE: Regex = Regex::new("^(?:(?:(?P<hour>\\d+)h(?:ours?)?)?(?:(?P<min>\\d+)m(?:in)?)?(?:(?P<sec>\\d+)s(?:ec)?)?)(?:(?P<msec>\\d+)ms(?:ec)?)?$").unwrap();
    }
    if arg.is_empty() {
        // special case, so one seconds capture group is enough
        return Err(NoMatch(arg.to_owned()));
    }
    if let Ok(seconds) = arg.parse::<u64>() {
        return Ok(Duration::from_secs(seconds));
    }
    let capures = RE.captures(arg).ok_or_else(|| NoMatch(arg.to_owned()))?;
    let mut milliseconds = 0;
    if let Some(hours) = capures.name("hour") {
        milliseconds += hours
            .as_str()
            .parse::<u64>()
            .unwrap_or_else(|_| unreachable!());
    }
    milliseconds *= 60;
    if let Some(min) = capures.name("min") {
        milliseconds += min
            .as_str()
            .parse::<u64>()
            .unwrap_or_else(|_| unreachable!());
    }
    milliseconds *= 60;
    if let Some(sec) = capures.name("sec") {
        milliseconds += sec
            .as_str()
            .parse::<u64>()
            .unwrap_or_else(|_| unreachable!());
    }
    milliseconds *= 1000;
    if let Some(msec) = capures.name("msec") {
        milliseconds += msec
            .as_str()
            .parse::<u64>()
            .unwrap_or_else(|_| unreachable!());
    }
    Ok(std::time::Duration::from_millis(milliseconds))
}
