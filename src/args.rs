use std::{path::PathBuf, time::Duration};

use clap::Args;
use confy::ConfyError;

#[derive(Args, Debug, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct ConfigArgs {
    #[clap(long, short, value_name = "FILE", help = "use this config file")]
    pub config: Option<PathBuf>,
    #[clap(long, help = "writes path into config")]
    pub overwrite_config: bool,
}
impl ConfigArgs {
    #[must_use]
    pub fn load_config<C>(&self, sub_config: &str) -> C
    where
        C: serde::Serialize + serde::de::DeserializeOwned + Default,
    {
        self.try_load_config(sub_config).unwrap()
    }
    pub fn try_load_config<C>(&self, sub_config: &str) -> Result<C, ConfyError>
    where
        C: serde::Serialize + serde::de::DeserializeOwned + Default,
    {
        self.config.as_ref().map_or_else(
            || confy::load(crate::APP_NAME, Some(sub_config)),
            |config_path| confy::load_path(config_path),
        )
    }

    pub fn save_config<C>(&self, sub_config: &str, config: &C)
    where
        C: serde::Serialize + serde::de::DeserializeOwned + Default,
    {
        self.try_save_config(sub_config, config).unwrap();
    }
    pub fn try_save_config<C>(&self, sub_config: &str, config: &C) -> Result<(), ConfyError>
    where
        C: serde::Serialize + serde::de::DeserializeOwned + Default,
    {
        self.config.as_ref().map_or_else(
            || confy::store(crate::APP_NAME, Some(sub_config), config),
            |config_path| confy::store_path(config_path, config),
        )
    }
}

use regex::Regex;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
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
