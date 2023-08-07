use std::{path::Path, str::FromStr, time::Duration};

use itertools::Itertools;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LableParseError {
    #[error("Missing elements in {0:?}")]
    MissingElement(String),
    #[error("Failed to parse {0} Duration in {1:?}")]
    DuratrionParseError(&'static str, String),
}
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display)]
#[display(fmt = "{}\t{}\t{:?}", "start.as_secs_f64()", "end.as_secs_f64()", name)]
pub struct TimeLabel {
    pub start: Duration,
    pub end: Duration,
    pub name: Option<String>,
}

impl TimeLabel {
    /// creates a new [`Timelabel`] with the given values
    #[must_use]
    pub fn new(start: Duration, end: Duration, name: Option<String>) -> Self {
        Self {
            start,
            end,
            name: name.filter(|it| !it.is_empty()),
        }
    }
    /// creates a new [`Timelabel`] with a name build from pattern
    /// // TODO doc how pattern works
    #[must_use]
    pub fn new_with_pattern(
        start: Duration,
        end: Duration,
        number: usize,
        name_pattern: &str,
    ) -> Self {
        Self::new(start, end, Some(Self::name_convert(name_pattern, number)))
    }
    #[must_use]
    fn name_convert(pattern: &str, number: usize) -> String {
        // TODO allow escaping, document
        pattern.replace('#', &number.to_string())
    }

    /// writes the labels of `labels` into `path` in a format of audacitys text mark file
    /// use `dry_run` to simulate the operation
    ///
    /// # Errors
    /// forwards the [`std::io::Error`] of writing `path`
    pub fn write<P, Iter>(lables: Iter, path: P, dry_run: bool) -> Result<(), std::io::Error>
    where
        P: AsRef<Path>,
        Iter: Iterator<Item = Self>,
    {
        let out = lables.map(|it| it.to_string()).join("\n");

        if dry_run {
            println!(
                "writing: \"\"\"\n{out}\n\"\"\" > {}",
                path.as_ref().display()
            );
        } else {
            std::fs::write(&path, out)?;
        }
        Ok(())
    }

    /// reads the labels of `path` in a format of audacitys text mark file
    ///
    /// will just log a warning if a label couldn't be parsed
    ///
    /// # Errors
    /// forwards the [`std::io::Error`] of reading `path`
    pub fn read<P: AsRef<Path>>(path: P) -> Result<Vec<Self>, std::io::Error> {
        Ok(std::fs::read_to_string(&path)?
            .lines()
            .filter(|it| !it.trim_start().starts_with('#'))
            .filter_map(|line| match line.parse() {
                Ok(label) => Some(label),
                Err(err) => {
                    log::warn!("couldn't parse lable {line:?} because {err:?}");
                    None
                }
            })
            .collect_vec())
    }

    fn parse_duration(
        part: &str,
        name: &'static str,
        value: &str,
    ) -> Result<Duration, <Self as FromStr>::Err> {
        part.parse::<f64>()
            .map(Duration::from_secs_f64)
            .map_err(|_| LableParseError::DuratrionParseError(name, value.to_owned()))
    }
}
impl FromStr for TimeLabel {
    type Err = LableParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (start, end, name) = value
            .splitn(3, '\t')
            .collect_tuple::<(_, _, _)>()
            .ok_or_else(|| LableParseError::MissingElement(value.to_owned()))?;
        Ok(Self {
            start: Self::parse_duration(start, "start", value)?,
            end: Self::parse_duration(end, "end", value)?,
            name: Some(name.to_owned()),
        })
    }
}
impl From<(f64, f64, String)> for TimeLabel {
    fn from(value: (f64, f64, String)) -> Self {
        Self::new(
            Duration::from_secs_f64(value.0),
            Duration::from_secs_f64(value.1),
            Some(value.2),
        )
    }
}
