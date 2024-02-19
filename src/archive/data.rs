#![allow(dead_code)]
use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::{OsStr, OsString},
    fmt::{Display, Write},
    num::ParseIntError,
    path::Path,
    str::FromStr,
    time::Duration,
};

use audacity::data::TimeLabel;
use chrono::NaiveDate;
use itertools::Itertools;
use lazy_static::lazy_static;
use log::{debug, warn};
use regex::Regex;
use thiserror::Error;

use crate::{
    matcher::{mp3_reader::SampleType, start_as_duration},
    worker::ChapterList,
};
use common::extensions::{iter::IteratorExt, vec::FindOrPush};

pub trait StrOrOsStr {
    type Owned: std::fmt::Write;
    fn new() -> Self::Owned;

    fn push(s: &mut Self::Owned, push: &Self);
    fn push_char(s: &mut Self::Owned, push: char);
}
impl StrOrOsStr for str {
    type Owned = String;
    fn new() -> Self::Owned {
        String::new()
    }

    fn push(s: &mut Self::Owned, push: &Self) {
        s.push_str(push);
    }

    fn push_char(s: &mut Self::Owned, push: char) {
        s.push(push);
    }
}
impl StrOrOsStr for OsStr {
    type Owned = OsString;
    fn new() -> Self::Owned {
        OsString::new()
    }

    fn push(s: &mut Self::Owned, push: &Self) {
        s.push(push);
    }

    fn push_char(s: &mut Self::Owned, push: char) {
        s.push(&push.to_string());
    }
}

#[must_use]
pub fn build_timelabel_name<S: StrOrOsStr + ?Sized, S1: AsRef<S>, S2: AsRef<S>>(
    series_name: impl Into<Option<S1>>,
    nr: &ChapterNumber,
    part: impl Into<Option<usize>>,
    chapter_name: impl Into<Option<S2>>,
) -> S::Owned {
    let mut name = S::new();

    if let Some(series_name) = series_name.into() {
        S::push(&mut name, series_name.as_ref());
        S::push_char(&mut name, ' ');
    }
    let _ = write!(name, "{nr}");
    if let Some(part) = part.into() {
        let _ = write!(name, ".{part}");
    }
    if let Some(chapter_name) = chapter_name.into() {
        S::push_char(&mut name, ' ');
        S::push(&mut name, chapter_name.as_ref());
    }
    name
}

pub fn timelabel_from_peaks<'a, Iter>(
    peaks: Iter,
    sr: u16,
    delay_start: Duration,
    name_pattern: &'a str,
) -> impl Iterator<Item = TimeLabel> + 'a
where
    Iter: Iterator<Item = &'a find_peaks::Peak<SampleType>> + 'a,
{
    peaks
        .map(move |p| start_as_duration(p, sr))
        .tuple_windows()
        .lzip(1..)
        .map(move |(i, (start, end))| {
            TimeLabel::new::<String>(
                start + delay_start,
                end,
                Some(name_pattern.replace('#', &i.to_string())),
            )
        })
}
#[derive(Debug, Clone)]
pub struct Archive {
    data: Vec<Series>,
}
impl Archive {
    /// will only log warnings, when errors from parsing occure
    pub fn read(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().join("**/*.txt");
        let pattern = path
            .to_str()
            .expect("currently only supporting UTF-8 filenames");
        let tmp = glob::glob(pattern)
            .expect("glob pattern failed")
            .filter_map(|entry| {
                let entry = entry.expect("couldn't read globbet file");
                match Source::from_path(&entry) {
                    Ok(source) => Some((source, TimeLabel::read(&entry).ok()?.into_iter())),
                    Err(kind) => {
                        warn!("failed to parse source {entry:?} from filename because {kind:?}");
                        None
                    }
                }
            });
        Self::from(tmp)
    }

    #[must_use]
    pub fn parse_line(line: &str) -> Option<(&str, ChapterNumber, Option<usize>, Option<&str>)> {
        const REG_SERIES: &str = "series";
        const REG_NUMBER: &str = "nr";
        const REG_CHAPTER: &str = "chapter";
        const REG_PART: &str = "part";
        lazy_static! {
            static ref RE: Regex = Regex::new(&format!("^(?P<{REG_SERIES}>.+?) (?P<{REG_NUMBER}>{})(?:\\.(?P<{REG_PART}>\\d+))?(?: (?P<{REG_CHAPTER}>.+))?$", ChapterNumber::REGEX_PATTERN)).unwrap();
        }
        let captures = RE.captures(line)?;

        let series = captures.name(REG_SERIES).unwrap().as_str();

        let ch_nr = captures.name(REG_NUMBER).unwrap().as_str();
        let ch_nr = ch_nr.parse::<ChapterNumber>().unwrap_or_else(|err| {
            panic!("failed to read ChapterNumber {ch_nr:?}, because {err:?}")
        });
        let part = captures
            .name(REG_PART)
            .and_then(|it| it.as_str().parse().ok());
        let chapter = captures.name(REG_CHAPTER).map(|it| it.as_str());

        Some((series, ch_nr, part, chapter))
    }

    fn from<InnerIter, Iter>(value: Iter) -> Self
    where
        Iter: Iterator<Item = (Source, InnerIter)>,
        InnerIter: Iterator<Item = TimeLabel>,
    {
        let mut archive = Self { data: Vec::new() };
        for (source, labels) in value {
            for label in labels {
                if label
                    .name()
                    .is_some_and(|name| name.strip_prefix('#').is_some())
                {
                    debug!("skipping {:?}", label.name());
                    continue;
                }
                let Some((series_name, ch_nr, _, chapter_name)) =
                    label.name().and_then(Self::parse_line)
                else {
                    warn!(
                        "name {:?} in {source} couldn't be parsed to Series",
                        label.name()
                    );
                    continue;
                };

                let series = archive.data.find_or_push_else(
                    || Series::new(series_name.to_owned()),
                    |it| it.name == series_name,
                );
                // TODO handle mixed modifiers in one source
                let chapter = series.chapters.find_or_push_else(
                    || Chapter::new(ch_nr, chapter_name.map(std::borrow::ToOwned::to_owned)),
                    |it| it.nr == ch_nr,
                );

                chapter
                    .parts
                    .entry(source.clone())
                    .and_modify(|part| *part += 1)
                    .or_insert(1);
            }
        }
        archive.data.sort_by(|a, b| Ord::cmp(&a.name, &b.name));
        archive.data.iter_mut().for_each(|s| s.chapters.sort());
        archive
    }

    #[must_use]
    pub const fn as_display<'a>(
        &'a self,
        indent: &'a str,
        print_index: bool,
        print_all: bool,
        print_missing: bool,
    ) -> ArchiveDisplay<'a> {
        ArchiveDisplay {
            archive: self,
            indent,
            print_index,
            print_all,
            print_missing,
        }
    }

    #[must_use]
    pub fn get_element(&self, identifier: &str, just_series: bool) -> Option<ArchiveSearchResult> {
        lazy_static! {
            static ref RE: Regex =
                Regex::new("(?P<series>\\d+)(?:\\.(?P<chapter>\\d+\\??))?").unwrap();
        }
        match RE.captures(identifier) {
            Some(capture) => {
                let series_nr = capture
                    .name("series")
                    .unwrap()
                    .as_str()
                    .parse::<usize>()
                    .unwrap();
                let chapter_nr = capture
                    .name("chapter")
                    .map(|s| s.as_str().parse::<usize>().unwrap());

                let found_s = &self.data[series_nr - 1];
                match chapter_nr {
                    Some(chapter_nr) if !just_series => {
                        let res = found_s
                            .chapters
                            .iter()
                            .find(|ch| ch.nr.nr == chapter_nr)
                            .map(ArchiveSearchResult::Chapter);
                        if res.is_none() {
                            debug!(
                                "couldn't find Chapter with nr {chapter_nr} in series {:?}",
                                found_s.name
                            );
                        }
                        res
                    }
                    _ => Some(ArchiveSearchResult::Series(found_s)),
                }
            }
            None => self
                .get_series_by_name(identifier)
                .map(ArchiveSearchResult::Series),
        }
    }

    #[must_use]
    pub fn get_series_by_name(&self, identifier: &str) -> Option<&Series> {
        self.data.iter().find(|x| x.name == identifier)
    }
}

impl ChapterList for Series {
    fn len(&self) -> usize {
        self.chapters.len()
    }

    fn get(&self, nr: ChapterNumber) -> Option<Cow<'_, str>> {
        self.chapters.iter().find(|c| c.nr == nr).map(get_name_cow)
    }

    fn chapter_iter(&self) -> Box<(dyn Iterator<Item = (ChapterNumber, Cow<'_, str>)> + '_)> {
        Box::new(self.chapters.iter().map(|c| (c.nr, get_name_cow(c))))
    }
}
fn get_name_cow(c: &Chapter) -> Cow<'_, str> {
    Cow::Borrowed(c.name.as_deref().unwrap_or("<unnamed>"))
}

pub struct ArchiveDisplay<'a> {
    archive: &'a Archive,
    indent: &'a str,
    print_index: bool,
    print_all: bool,
    print_missing: bool,
}
impl<'a> Display for ArchiveDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use itertools::Position as Pos;
        let pad_len = self
            .print_index
            .then(|| ((self.archive.data.len() + 1) as f64).log10().ceil() as usize); // +1 needed so the breakpoint is earlier. [1-10] -> 1 => [0-9] -> 1
        let pad = pad_len.map_or_else(String::new, |l| " ".repeat(l + 3));

        for (pos, (i, series)) in self.archive.data.iter().enumerate().with_position() {
            if let Some(pad_len) = pad_len {
                write!(f, "[{:0pad_len$}] ", i + 1)?;
            }
            write!(
                f,
                "{}",
                series.as_display(
                    &format!("{pad}{}", self.indent),
                    self.print_all,
                    self.print_missing
                )
            )?;
            if let Pos::First | Pos::Middle = pos {
                f.write_char('\n')?;
            }
        }
        Ok(())
    }
}
pub enum ArchiveSearchResult<'a> {
    Series(&'a Series),
    Chapter(&'a Chapter),
}

#[derive(Debug, Clone)]
pub struct Series {
    name: String,
    chapters: Vec<Chapter>,
}
impl Series {
    const fn new(name: String) -> Self {
        Self {
            name,
            chapters: Vec::new(),
        }
    }
    #[must_use]
    const fn as_display<'a>(
        &'a self,
        indent: &'a str,
        print_chapters: bool,
        print_missing: bool,
    ) -> SeriesDisplay<'a> {
        SeriesDisplay {
            series: self,
            indent,
            print_chapters,
            print_missing,
        }
    }
}
struct SeriesDisplay<'a> {
    series: &'a Series,
    indent: &'a str,
    print_chapters: bool,
    print_missing: bool,
}
impl<'a> Display for SeriesDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.series.name)?;
        if self.print_chapters && !self.series.is_empty() {
            assert!(
                IteratorExt::is_sorted(self.series.chapters.iter()),
                "assumes series.chapter to be sorted"
            );
            let max_chapter_nr = self.series.chapters.last().unwrap().nr.nr;
            let nr_len = ((max_chapter_nr + 1) as f64).log10().ceil() as usize; // +1 needed so the breakpoint is earlier. [1-10] -> 1 => [0-9] -> 1
            let mut contains_extra = false;

            for chapter in &self.series.chapters {
                contains_extra |= chapter.nr.is_maybe | chapter.nr.is_partial;
            }

            let mut chapters = self.series.chapters.iter().peekable();
            for i in 1.. {
                if chapters.peek().is_none() {
                    break;
                }
                let mut found_some = false;
                while let Some(chapter) = chapters.peek() {
                    if chapter.nr.nr != i {
                        break;
                    }
                    found_some = true;
                    write!(
                        f,
                        "\n{}{}",
                        self.indent,
                        chapter.as_display(Some((nr_len, false)), contains_extra)
                    )?;
                    chapters.next();
                }
                if !found_some && self.print_missing {
                    write!(
                        f,
                        "\n{}{}",
                        self.indent,
                        Chapter::new(i.into(), None)
                            .as_display(Some((nr_len, false)), contains_extra)
                    )?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Chapter {
    nr: ChapterNumber,
    name: Option<String>,
    parts: HashMap<Source, u8>, // source and number of parts in source
}

impl PartialEq for Chapter {
    fn eq(&self, other: &Self) -> bool {
        self.nr == other.nr && self.name == other.name
    }
}
impl Eq for Chapter {}
impl PartialOrd for Chapter {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Chapter {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.nr.cmp(&other.nr) {
            std::cmp::Ordering::Equal => self.name.cmp(&other.name),
            x => x,
        }
    }
}

impl Chapter {
    fn new(nr: ChapterNumber, name: Option<String>) -> Self {
        Self {
            nr,
            name,
            parts: HashMap::new(),
        }
    }
    #[must_use]
    const fn as_display(&self, r_just: Option<(usize, bool)>, l_just: bool) -> ChapterDisplay<'_> {
        ChapterDisplay {
            chapter: self,
            r_just,
            l_just,
        }
    }
}
struct ChapterDisplay<'a> {
    chapter: &'a Chapter,
    r_just: Option<(usize, bool)>,
    l_just: bool,
}
impl<'a> Display for ChapterDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} - ",
            self.chapter.nr.as_display(self.r_just, self.l_just)
        )?;
        if let Some(name) = &self.chapter.name {
            write!(f, "{name} ")?;
        }
        write!(f, "[{}]", &self.chapter.parts.keys().sorted().join(", "))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[must_use]
pub struct ChapterNumber {
    pub nr: usize,
    pub is_maybe: bool,
    pub is_partial: bool,
}
impl ChapterNumber {
    const REGEX_PATTERN: &'static str = "\\d+\\??\\-?";
    pub const fn new(nr: usize, is_maybe: bool) -> Self {
        Self {
            nr,
            is_maybe,
            is_partial: false,
        }
    }
    pub const fn next(mut self) -> Self {
        self.nr += 1;
        self
    }

    /// formats the [`ChapterNumber`] onto `s`.
    ///
    /// # Arguments
    /// `r_just`: the length of the padding and if it should use zeros od spaces
    ///
    /// `l_just`: if it should pad for an extra '?' at the end
    ///
    /// # Examples
    /// ```
    /// use audio_matcher::archive::data::ChapterNumber;
    ///
    /// let nr = ChapterNumber { nr: 3, is_maybe: true, is_partial: false };
    /// assert_eq!("3?", nr.as_display(None, false).to_string());
    /// assert_eq!("0003?", nr.as_display(Some((4, true)), false).to_string());
    ///
    /// let nr = ChapterNumber { nr: 3, is_maybe: false, is_partial: false };
    /// assert_eq!("  3  ", nr.as_display(Some((3, false)), true).to_string());
    /// assert_eq!("0003  ", nr.as_display(Some((4, true)), true).to_string());
    /// ```
    #[must_use]
    pub const fn as_display(
        &self,
        r_just: Option<(usize, bool)>,
        l_just: bool,
    ) -> ChapterNumberDisplay<'_> {
        ChapterNumberDisplay {
            number: self,
            r_just,
            l_just,
        }
    }
}
impl Display for ChapterNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_display(None, false))
    }
}
impl From<usize> for ChapterNumber {
    fn from(value: usize) -> Self {
        Self {
            nr: value,
            is_maybe: false,
            is_partial: false,
        }
    }
}
pub struct ChapterNumberDisplay<'a> {
    number: &'a ChapterNumber,
    r_just: Option<(usize, bool)>,
    l_just: bool,
}
impl<'a> Display for ChapterNumberDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.r_just {
            Some(r_just) => {
                if r_just.1 {
                    write!(f, "{:0width$}", self.number.nr, width = r_just.0)?;
                } else {
                    write!(f, "{:width$}", self.number.nr, width = r_just.0)?;
                }
            }
            None => write!(f, "{}", self.number.nr)?,
        }
        if self.number.is_maybe {
            f.write_char('?')?;
        } else if self.l_just {
            f.write_char(' ')?;
        }
        if self.number.is_partial {
            f.write_char('-')?;
        } else if self.l_just {
            f.write_char(' ')?;
        }
        Ok(())
    }
}
impl std::str::FromStr for ChapterNumber {
    type Err = ParseIntError;

    /// Extracts a Chapter Number from a str.
    ///
    /// # Examples
    /// ```
    /// use audio_matcher::archive::data::ChapterNumber;
    ///
    /// assert_eq!(Ok(ChapterNumber { nr: 3, is_maybe: true , is_partial: false}),    "3?".parse::<ChapterNumber>());
    /// assert_eq!(Ok(ChapterNumber { nr: 3, is_maybe: false, is_partial: false}),     "3".parse::<ChapterNumber>());
    /// assert_eq!(Ok(ChapterNumber { nr: 3, is_maybe: true , is_partial: false}),  "003?".parse::<ChapterNumber>());
    /// assert_eq!(Ok(ChapterNumber { nr: 3, is_maybe: false, is_partial: false}),   " 3 ".parse::<ChapterNumber>());
    /// assert_eq!(Ok(ChapterNumber { nr: 3, is_maybe: true , is_partial: true }), "003?-".parse::<ChapterNumber>());
    /// assert_eq!(Ok(ChapterNumber { nr: 3, is_maybe: false, is_partial: true }),  " 3- ".parse::<ChapterNumber>());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.trim();
        let strip_minus = value.strip_suffix('-');
        let value = strip_minus.unwrap_or(value);
        let strip_question = value.strip_suffix('?'); // TODO maybe allow flipped
        let value = strip_question.unwrap_or(value);
        Ok(Self {
            nr: value.parse::<usize>()?,
            is_maybe: strip_question.is_some(),
            is_partial: strip_minus.is_some(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, derive_more::Display)]
#[display(fmt = "{station} - {}", "date.format(Self::DISPLAY_DATE_FMT)")]
pub struct Source {
    station: String,
    date: NaiveDate,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceErrorKind {
    #[error("the path didn't reference a file")]
    NotAFile,
    #[error("the name didn't contain a '-'")]
    InvalidSeperator,
    #[error("the date couldn't be parsed")]
    InvalidDate,
}
impl Source {
    const FILE_DATE_FMT: &'static str = "%Y_%m_%d";
    const DISPLAY_DATE_FMT: &'static str = "%Y-%m-%d";
    pub fn from_path(value: impl AsRef<Path>) -> Result<Self, SourceErrorKind> {
        let path = value.as_ref().with_extension("");
        let file_name = path.file_name().ok_or(SourceErrorKind::NotAFile)?;
        file_name
            .to_str()
            .unwrap_or_else(|| panic!("{file_name:?} contained invalid unicode"))
            .parse()
    }
    #[must_use]
    pub fn to_file_name(&self) -> String {
        format!("{}-{}", self.station, self.date.format(Self::FILE_DATE_FMT))
    }
}
impl FromStr for Source {
    type Err = SourceErrorKind;

    /// parses a Source from a string in the form of {station}-{%Y_%m_%d}
    ///
    /// # Errors
    ///  - [`SourceErrorKind::InvalidSeperator`] when no '-' is found in `s`
    ///  - [`SourceErrorKind::InvalidDate`] when the Date can't be parsed
    ///
    /// # Examples
    /// ```
    /// # use audio_matcher::archive::data::Source;
    /// # use audio_matcher::archive::data::SourceErrorKind;
    /// #
    /// assert_eq!("abc - 2023-07-13", "abc-2023_07_13".parse::<Source>().unwrap().to_string(), "parse and unparse display");
    /// assert_eq!("abc-2023_07_13", "abc-2023_07_13".parse::<Source>().unwrap().to_file_name(), "parse and unparse filename");
    /// assert_eq!(Err(SourceErrorKind::InvalidSeperator), "2023_07_13".parse::<Source>(), "fail without station adn seperator");
    /// assert_eq!(Err(SourceErrorKind::InvalidDate), "abc-2023-07-13".parse::<Source>(), "fail with wrong date seperator");
    /// assert_eq!(Err(SourceErrorKind::InvalidDate), "abc-2023_07".parse::<Source>(), "fail with wrong date format");
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (station, date) = s
            .splitn(2, '-')
            .collect_tuple()
            .ok_or(Self::Err::InvalidSeperator)?;
        Ok(Self {
            station: station.to_owned(),
            date: NaiveDate::parse_from_str(date, Self::FILE_DATE_FMT)
                .map_err(|_| Self::Err::InvalidDate)?,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    mod parser {
        use super::*;
        #[test]
        fn full_match() {
            let cap = Archive::parse_line("Gruselkabinett 6.2 Das verfluchte Haus")
                .expect("failed to match");

            assert_eq!("Gruselkabinett", cap.0);
            assert_eq!(
                ChapterNumber {
                    nr: 6,
                    is_maybe: false,
                    is_partial: false
                },
                cap.1
            );
            assert_eq!(Some(2), cap.2);
            assert_eq!(Some("Das verfluchte Haus"), cap.3);
        }
        #[test]
        fn patial_match() {
            let cap = Archive::parse_line("Gruselkabinett 6").expect("failed to match");

            assert_eq!("Gruselkabinett", cap.0);
            assert_eq!(
                ChapterNumber {
                    nr: 6,
                    is_maybe: false,
                    is_partial: false
                },
                cap.1
            );
        }

        #[test]
        fn extra_number() {
            let cap = Archive::parse_line("Gruselkabinett 6 Multipart 1").expect("failed to match");

            assert_eq!("Gruselkabinett", cap.0);
            assert_eq!(
                ChapterNumber {
                    nr: 6,
                    is_maybe: false,
                    is_partial: false
                },
                cap.1
            );
            assert_eq!(None, cap.2);
            assert_eq!(Some("Multipart 1"), cap.3);
        }
    }

    mod series_tests {
        use super::*;

        #[test]
        fn format() {
            let mut ser = Series::new("gute show".to_owned());
            ser.chapters.push(Chapter::new(
                ChapterNumber {
                    nr: 5,
                    is_maybe: true,
                    is_partial: false,
                },
                Some("unbekannt".to_owned()),
            ));
            ser.chapters.push(Chapter::new(
                ChapterNumber {
                    nr: 6,
                    is_maybe: false,
                    is_partial: false,
                },
                Some("bekannt".to_owned()),
            ));
            assert_eq!(
                "gute show\n.5?  - unbekannt []\n.6   - bekannt []",
                ser.as_display(".", true, false).to_string()
            );
            assert_eq!(
                "gute show\n.1   - []\n.2   - []\n.3   - []\n.4   - []\n.5?  - unbekannt []\n.6   - bekannt []",
                ser.as_display(".", true, true).to_string()
            );
        }
    }

    mod chapter_tests {
        use super::*;

        #[test]
        fn format_with_parts() {
            let mut ch = Chapter::new(
                ChapterNumber {
                    nr: 15,
                    is_maybe: false,
                    is_partial: false,
                },
                None,
            );
            ch.parts.insert("station-2023_1_1".parse().unwrap(), 2);
            assert_eq!(
                "15 - [station - 2023-01-01]",
                ch.as_display(None, false).to_string()
            );
            ch.parts.insert("station-2023_1_2".parse().unwrap(), 2);

            assert_eq!(
                "15 - [station - 2023-01-01, station - 2023-01-02]",
                ch.as_display(None, false).to_string()
            );
        }

        #[test]
        fn format_with_name() {
            let ch = Chapter::new(
                ChapterNumber {
                    nr: 15,
                    is_maybe: false,
                    is_partial: false,
                },
                Some("chapter name".to_owned()),
            );
            assert_eq!(
                "15 - chapter name []",
                ch.as_display(None, false).to_string()
            );
        }
    }

    mod source_tests {
        // tests from the inside, more in doctest
        use super::*;

        #[test]
        fn parse_source() {
            assert_eq!(
                Ok(Source {
                    station: "89.0rtl".to_owned(),
                    date: NaiveDate::from_ymd_opt(2023, 6, 17).unwrap()
                }),
                Source::from_path("/89.0rtl-2023_06_17.mp3")
            );
            assert_eq!(
                Ok(Source {
                    station: "station".to_owned(),
                    date: NaiveDate::from_ymd_opt(2023, 6, 17).unwrap()
                }),
                "station-2023_06_17".parse()
            );
        }

        #[test]
        fn format() {
            assert_eq!(
                "89.0rtl - 2023-06-17",
                Source {
                    station: "89.0rtl".to_owned(),
                    date: NaiveDate::from_ymd_opt(2023, 6, 17).unwrap()
                }
                .to_string()
            );
        }
    }

    mod chapter_number_tests {
        use super::*;
        #[test]
        fn format_no_just() {
            let nr = ChapterNumber {
                nr: 3,
                is_maybe: false,
                is_partial: false,
            };
            assert_eq!("3", nr.as_display(None, false).to_string());

            let nr = ChapterNumber {
                nr: 30,
                is_maybe: true,
                is_partial: false,
            };
            assert_eq!("30?", nr.as_display(None, false).to_string());
        }
        #[test]
        fn format_0_r_just() {
            let nr = ChapterNumber {
                nr: 3,
                is_maybe: false,
                is_partial: false,
            };
            assert_eq!("0003", nr.as_display(Some((4, true)), false).to_string());

            let nr = ChapterNumber {
                nr: 30,
                is_maybe: true,
                is_partial: false,
            };
            assert_eq!("0030?", nr.as_display(Some((4, true)), false).to_string());
        }
        #[test]
        fn format_space_r_just() {
            let nr = ChapterNumber {
                nr: 3,
                is_maybe: false,
                is_partial: false,
            };
            assert_eq!("   3", nr.as_display(Some((4, false)), false).to_string());

            let nr = ChapterNumber {
                nr: 30,
                is_maybe: true,
                is_partial: false,
            };
            assert_eq!("  30?", nr.as_display(Some((4, false)), false).to_string());
        }
        #[test]
        fn format_l_just() {
            let nr = ChapterNumber {
                nr: 3,
                is_maybe: false,
                is_partial: false,
            };
            assert_eq!("3  ", nr.as_display(None, true).to_string());

            let nr = ChapterNumber {
                nr: 30,
                is_maybe: true,
                is_partial: false,
            };
            assert_eq!("30? ", nr.as_display(None, true).to_string());
        }
    }
}
