use std::{path::PathBuf, time::Duration};

use clap::Args;
use confy::ConfyError;
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
    pub fn new(bools: impl Into<Option<bool>>, trys: impl Into<Option<u8>>) -> Self {
        let bools: Option<_> = bools.into();
        Self {
            yes: bools.is_some(),
            no: bools.is_some_and(|it| !it),
            trys: trys.into().unwrap_or(3),
        }
    }

    #[inline]
    #[allow(clippy::needless_pass_by_value)]
    fn inner_read<T>(
        msg: impl AsRef<str>,
        default: impl Into<Option<T>>,
        retry_msg: Option<impl AsRef<str>>,
        mut map: impl FnMut(String) -> Option<T>,
        trys: impl IntoIterator<Item = u8>,
    ) -> Option<T> {
        let msg = msg.as_ref();
        let retry_msg = retry_msg.as_ref().map(std::convert::AsRef::as_ref);
        let default = default.into();

        print!("{msg}");
        for _ in trys {
            let rin: String = text_io::read!("{}\n");
            if default.is_some() && rin.is_empty() {
                return default;
            }
            match (map(rin), retry_msg) {
                (Some(t), _) => return Some(t),
                (None, Some(retry_msg)) => println!("{retry_msg}"),
                (None, None) => print!("{msg}"),
            }
        }
        None
    }

    const DEFAULT_RETRY_MSG: &str = "couldn't parse that, please try again: ";
    pub fn read(msg: impl AsRef<str>, default: Option<String>) -> String {
        Self::inner_read(
            msg,
            default,
            Some(Self::DEFAULT_RETRY_MSG),
            Some,
            std::iter::once(1),
        )
        .unwrap_or_else(|| unreachable!())
    }
    pub fn map_read<T>(
        msg: impl AsRef<str>,
        default: impl Into<Option<T>>,
        retry_msg: Option<impl AsRef<str>>,
        map: impl FnMut(String) -> Option<T>,
    ) -> T {
        Self::inner_read(msg, default, retry_msg, map, 1..).unwrap_or_else(|| unreachable!())
    }
    // TODO remove trys from Self
    pub fn try_read<T>(
        &self,
        msg: impl AsRef<str>,
        default: Option<T>,
        map: impl FnMut(String) -> Option<T>,
    ) -> Option<T> {
        Self::inner_read(
            msg,
            default,
            Some(Self::DEFAULT_RETRY_MSG),
            map,
            1..self.trys,
        )
    }

    #[must_use]
    #[momo::momo]
    pub fn ask_consent(self, msg: impl AsRef<str>) -> bool {
        if self.yes || self.no {
            return self.yes;
        }
        self.try_read(format!("{msg} [y/n]: "), None, |it| {
            if ["y", "yes", "j", "ja"].contains(&it.as_str()) {
                Some(true)
            } else if ["n", "no", "nein"].contains(&it.as_str()) {
                Some(false)
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            info!("probably not");
            false
        })
    }

    #[must_use]
    pub fn read_with_suggestion(
        msg: impl AsRef<str>,
        initial: Option<&str>,
        mut suggestor: impl autocompleter::MyAutocomplete,
    ) -> String {
        let mut text = inquire::Text::new(msg.as_ref());
        text.initial_value = initial;
        // SAFTY: the reference to suggestor must be kept alive until ac is dropped. black-box should do this.
        let ac = unsafe { autocompleter::BorrowCompleter::new(&mut suggestor) };
        let res = text.with_autocomplete(ac).prompt().unwrap();
        drop(std::hint::black_box(suggestor));
        res
    }
}

pub mod autocompleter {
    use std::fmt::Debug;

    use common::extensions::iter::IteratorExt;
    use inquire::{autocompletion::Replacement, Autocomplete, CustomUserError};
    use itertools::Itertools;

    pub trait MyAutocomplete: Debug {
        fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError>;
        fn get_completion(
            &mut self,
            input: &str,
            highlighted_suggestion: Option<String>,
        ) -> Result<Replacement, CustomUserError>;
    }
    impl<AC: MyAutocomplete> MyAutocomplete for &mut AC {
        #[inline]
        fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError> {
            (**self).get_suggestions(input)
        }

        #[inline]
        fn get_completion(
            &mut self,
            input: &str,
            highlighted_suggestion: Option<String>,
        ) -> Result<Replacement, CustomUserError> {
            (**self).get_completion(input, highlighted_suggestion)
        }
    }
    #[derive(Debug)]
    pub(super) struct BorrowCompleter {
        inner: &'static mut dyn MyAutocomplete,
    }
    impl BorrowCompleter {
        pub(super) unsafe fn new<'a>(other: &'a mut dyn MyAutocomplete) -> Self {
            // SAFTY: transmute to upgrade lifetime to static, so one can uphold Autocompletes Clone + 'static needs
            Self {
                inner: unsafe {
                    std::mem::transmute::<&'a mut dyn MyAutocomplete, &'static mut dyn MyAutocomplete>(
                        other,
                    )
                },
            }
        }
    }
    // fake being clone, it's (probably) only needed, when the holding inquire::Text ist cloned
    impl Clone for BorrowCompleter {
        fn clone(&self) -> Self {
            panic!("cloned Autocompleter {self:?}");
            // Self { inner: self.inner }
        }
    }
    impl Autocomplete for BorrowCompleter {
        fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError> {
            self.inner.get_suggestions(input)
        }

        fn get_completion(
            &mut self,
            input: &str,
            highlighted_suggestion: Option<String>,
        ) -> Result<Replacement, CustomUserError> {
            self.inner.get_completion(input, highlighted_suggestion)
        }
    }

    #[derive(Debug)]
    pub struct VecCompleter {
        data: Vec<String>,
        metric: Box<dyn StrMetric>,
    }
    impl VecCompleter {
        #[must_use]
        pub fn new(data: Vec<String>, metric: impl StrMetric + 'static) -> Self {
            Self {
                data,
                metric: Box::new(metric),
            }
        }
        #[allow(clippy::should_implement_trait)] // will prob change signature
        pub fn from_iter<Iter>(iter: Iter, metric: impl StrMetric + 'static) -> Self
        where
            Iter: IntoIterator,
            Iter::Item: ToString,
        {
            Self::new(
                iter.into_iter().map(|it| it.to_string()).collect_vec(),
                metric,
            )
        }
    }
    impl MyAutocomplete for VecCompleter {
        fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError> {
            Ok(
                sort_with(self.metric.as_ref(), self.data.iter(), input, |it| it)
                    .cloned()
                    .collect_vec(),
            )
        }

        fn get_completion(
            &mut self,
            _input: &str,
            highlighted_suggestion: Option<String>,
        ) -> Result<Replacement, CustomUserError> {
            Ok(highlighted_suggestion)
        }
    }

    pub trait StrFilter: Debug {
        /// returns true if `input` matches the `option`
        fn filter(&self, option: &str, input: &str) -> bool;
    }
    pub trait StrMetric: Debug {
        /// the relative distance between two words between 0 and 1.
        /// 0 => the words are the same
        /// 1 => maximum distance
        fn distance(&self, option: &str, input: &str) -> f64;
    }
    impl<F: StrFilter> StrMetric for F {
        fn distance(&self, option: &str, input: &str) -> f64 {
            // default to 0 if the filter matches and 1 if not
            !self.filter(option, input) as u8 as f64
        }
    }

    /// use `filter` to sort the elements of `iter` in regards to `input`
    pub fn sort_with<I, M, F>(
        filter: &M,
        iter: I,
        input: &str,
        mut get_str: F,
    ) -> impl Iterator<Item = I::Item>
    where
        I: IntoIterator,
        F: FnMut(&I::Item) -> &str,
        M: StrMetric + ?Sized,
    {
        iter.into_iter()
            .map(|it| {
                let distance = filter.distance(get_str(&it), input);
                (it, distance)
            })
            .sorted_by(|(_, d1), (_, d2)| {
                d1.partial_cmp(d2).unwrap_or_else(|| {
                    log::warn!("encountered uncomparable values {d1:?} and {d2:?}");
                    std::cmp::Ordering::Greater
                })
            }) // sort 0->1->NaN
            .map(|(it, _)| it)
    }
    pub const fn compare_char(a: char, b: char, ignore_case: bool) -> bool {
        (ignore_case && a.eq_ignore_ascii_case(&b)) || a == b
    }

    #[derive(Debug, Clone, Copy)]
    pub struct StartsWithIgnoreCase;
    impl StrFilter for StartsWithIgnoreCase {
        fn filter(&self, option: &str, input: &str) -> bool {
            option.to_lowercase().starts_with(&input.to_lowercase())
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Levenshtein {
        ignore_case: bool,
    }
    impl StrMetric for Levenshtein {
        fn distance(&self, option: &str, input: &str) -> f64 {
            let lev_distance = self.dynamic_distance(option.chars(), &input.chars().collect_vec());
            let max = option.len().max(input.len());
            lev_distance as f64 / max as f64
        }
    }
    impl Levenshtein {
        pub const fn new(ignore_case: bool) -> Self {
            Self { ignore_case }
        }
        #[allow(dead_code)]
        fn recursive_distance(self, a: &[char], b: &[char]) -> usize {
            if a.is_empty() {
                b.len()
            } else if b.is_empty() {
                a.len()
            } else if compare_char(a[0], b[0], self.ignore_case) {
                self.recursive_distance(&a[1..], &b[1..])
            } else {
                let s1 = self.recursive_distance(&a[1..], b);
                let s2 = self.recursive_distance(a, &b[1..]);
                let s3 = self.recursive_distance(&a[1..], &b[1..]);
                1 + s1.min(s2).min(s3)
            }
        }
        fn dynamic_distance(self, s: impl IntoIterator<Item = char>, t: &[char]) -> usize {
            let n = t.len();

            // initialize v0 (the previous row of distances)
            // this row is A[0][i]: edit distance from an empty s to t;
            // that distance is the number of characters to append to  s to make t.
            let mut v0 = (0..=n).collect_vec();
            // v1 may as well be uninit
            let mut v1 = vec![0; n + 1];

            for (i, s_char) in s.into_iter().lzip(1..) {
                // calculate v1 (current row distances) from the previous row v0

                // first element of v1 is A[i][0]
                // edit distance is delete (i) chars from s to match empty t
                v1[0] = i;

                // use formula to fill in the rest of the row
                for (j, &t_char) in t.iter().enumerate() {
                    // calculating costs for A[i][j + 1]
                    let (substitution_cost, overflowing) =
                        v0[j].overflowing_sub(
                            compare_char(s_char, t_char, self.ignore_case) as usize
                        );
                    v1[j + 1] = if overflowing {
                        0
                    } else {
                        let deletion_cost = v0[j + 1];
                        let insertion_cost = v1[j];
                        substitution_cost.min(insertion_cost).min(deletion_cost) + 1
                    };
                }
                // copy v1 (current row) to v0 (previous row) for next iteration
                // since data in v1 is always invalidated, a swap without copy could be more efficient
                std::mem::swap(&mut v0, &mut v1);
            }
            // after the last swap, the results of v1 are now in v0
            v0[n]
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct SameStartBoost<O> {
        pub ignore_case: bool,
        pub same_start_bonus: f64,
        pub other: O,
    }
    impl<O: StrMetric> StrMetric for SameStartBoost<O> {
        fn distance(&self, option: &str, input: &str) -> f64 {
            let distance = self.other.distance(option, input);
            let max = option.len().max(input.len());
            let prefix_len = option
                .chars()
                .zip(input.chars())
                .take_while(|(a, b)| compare_char(*a, *b, self.ignore_case))
                .count();
            let prefix_factor = prefix_len as f64 / max as f64;
            distance * (prefix_factor.mul_add(-self.same_start_bonus, 1.0))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn __test_levenshtein(a: &str, b: &str, dist: usize, algo: Levenshtein) {
            let a = a.chars().collect_vec();
            let b = b.chars().collect_vec();
            assert_eq!(dist, algo.recursive_distance(&a, &b), "failed recursive");
            assert_eq!(
                dist,
                algo.recursive_distance(&b, &a),
                "failed recursive reversed"
            );
            assert_eq!(
                dist,
                algo.dynamic_distance(a.clone(), &b),
                "failed iterative"
            );
            assert_eq!(
                dist,
                algo.dynamic_distance(b, &a),
                "failed iterative reversed"
            );
        }
        #[test]
        fn test_levenshtein_same() {
            __test_levenshtein("Levenshtein", "Levenshtein", 0, Levenshtein::new(false));
            __test_levenshtein("levENSHTein", "LEVENshtein", 0, Levenshtein::new(true));
        }
        #[test]
        fn test_levenshtein_differend() {
            __test_levenshtein("kitten", "sitting", 3, Levenshtein::new(false));
            __test_levenshtein("levENSHTein", "LEVENshtein", 6, Levenshtein::new(false));
        }
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
