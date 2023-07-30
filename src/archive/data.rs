#![allow(dead_code)]
use std::{
    collections::HashMap,
    error::Error,
    fmt::{Display, Write},
    num::ParseIntError,
    path::Path,
    str::FromStr,
    time::Duration,
};

use chrono::NaiveDate;
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use thiserror::Error;

use crate::matcher::{mp3_reader::SampleType, start_as_duration};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LableParseError {
    #[error("Missing elements in {0:?}")]
    MissingElement(String),
    #[error("Failed to parse {0} Duration in {1:?}")]
    DuratrionParseError(&'static str, String),
}
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display)]
#[display(fmt = "{}\t{}\t{}", "start.as_secs_f64()", "end.as_secs_f64()", name)]
pub struct TimeLabel {
    start: Duration,
    end: Duration,
    name: String,
}

impl TimeLabel {
    #[must_use]
    pub fn new_with_pattern(
        start: Duration,
        end: Duration,
        number: usize,
        name_pattern: &str,
    ) -> Self {
        // TODO allow escaping, document
        Self::new(start, end, Self::name_convert(name_pattern, number))
    }
    #[must_use]
    pub const fn new(start: Duration, end: Duration, name: String) -> Self {
        Self { start, end, name }
    }
    #[must_use]
    pub fn name_convert(pattern: &str, number: usize) -> String {
        pattern.replace('#', &number.to_string())
    }

    pub fn from_peaks<'a, Iter>(
        peaks: Iter,
        sr: u16,
        delay_start: Duration,
        name_pattern: &'a str,
    ) -> impl Iterator<Item = Self> + 'a
    where
        Iter: Iterator<Item = &'a find_peaks::Peak<SampleType>> + 'a,
    {
        peaks
            .map(move |p| start_as_duration(p, sr))
            .tuple_windows()
            .enumerate()
            .map(move |(i, (start, end))| {
                Self::new_with_pattern(start + delay_start, end, i + 1, name_pattern)
            })
    }
    pub fn write_text_marks<P, Iter>(
        lables: Iter,
        path: P,
        dry_run: bool,
    ) -> Result<(), crate::matcher::errors::CliError>
    where
        P: AsRef<std::path::Path>,
        Iter: Iterator<Item = Self>,
    {
        let out = lables.map(|it| it.to_string()).join("\n");

        if dry_run {
            println!(
                "writing: \"\"\"\n{out}\n\"\"\" > {}",
                path.as_ref().display()
            );
        } else {
            std::fs::write(&path, out)
                .map_err(|_| crate::matcher::errors::CliError::CantCreateFile(path.into()))?;
        }
        Ok(())
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
            name: name.to_owned(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Archive {
    data: Vec<Series>,
}
impl Archive {
    pub fn read<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        use std::fs;
        let mut tmp = Vec::new();
        for entry in glob::glob(&format!(
            "{}/*.txt",
            path.as_ref().to_str().expect("path contained wierd char")
        ))
        .expect("glob pattern failed")
        {
            let entry = entry.expect("couldn't read globbet file");
            let source = Source::from_path(&entry)
                .unwrap_or_else(|_| panic!("couldn't parse '{}'", entry.display()));

            tmp.push((
                source,
                fs::read_to_string(&entry)
                    .unwrap_or_else(|_| panic!("couldn't read '{}'", entry.display()))
                    .lines()
                    .map(|line| {
                        TimeLabel::try_from(line)
                            .unwrap_or_else(|_| panic!("couldn't parse lable {line}"))
                    })
                    .collect_vec()
                    .into_iter(),
            ));
        }

        Ok(Self::try_from(tmp.into_iter()).expect("msg"))
    }

    fn try_from<InnerIter: Iterator<Item = TimeLabel>>(
        value: impl Iterator<Item = (Source, InnerIter)>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut archive = Self { data: Vec::new() };
        lazy_static! {
            static ref RE: Regex = Regex::new("(?:(?P<series>.*) )(?:(?P<nr>[\\d]+)(?P<extra>\\??)(?:\\.[\\d?]+)+)(?: (?P<chapter>.*))?").unwrap();
        }
        for (source, labels) in value {
            for label in labels {
                let captures = RE
                    .captures(&label.name)
                    .unwrap_or_else(|| panic!("name of {label:?} couldn't be parsed to Series"));

                let ch_nr = ChapterNumber::new(
                    captures.name("nr").unwrap().as_str().parse::<usize>()?,
                    !captures.name("extra").unwrap().is_empty(),
                );
                let series_name = captures.name("series").map(|it| it.as_str()).unwrap();
                let chapter_name = captures.name("chapter").map(|it| it.as_str());

                let series = if let Some(it) = archive.get_mut_series_by_name(series_name) {
                    it
                } else {
                    archive.data.push(Series::new(series_name.to_owned()));
                    archive.data.last_mut().unwrap()
                };

                let chapter = if let Some(it) = series.chapters.iter_mut().find(|it| it.nr == ch_nr)
                {
                    it
                } else {
                    series.chapters.push(Chapter::new(
                        ch_nr,
                        chapter_name.map(std::borrow::ToOwned::to_owned),
                    ));
                    series.chapters.last_mut().unwrap()
                };

                if let Some(part) = chapter.parts.get_mut(&source) {
                    *part += 1;
                } else {
                    chapter.parts.insert(source.clone(), 1);
                }
            }
        }
        archive.data.sort_by(|a, b| Ord::cmp(&a.name, &b.name));
        for s in &mut archive.data {
            s.chapters.sort_by(|a, b| Ord::cmp(&a.nr.nr, &b.nr.nr));
        }
        Ok(archive)
    }

    pub fn format(
        &self,
        s: &mut impl Write,
        indent: &str,
        print_index: bool,
        print_all: bool,
    ) -> Result<(), std::fmt::Error> {
        let pad_len = print_index.then(|| (self.data.len() as f64).log10().ceil() as usize);
        let pad = pad_len.map_or_else(String::new, |l| " ".repeat(l + 3));

        for (i, series) in self.data.iter().enumerate() {
            if let Some(pad_len) = pad_len {
                write!(s, "[{:0pad_len$}] ", i + 1)?;
            }
            series.format(s, &format!("{pad}{indent}"), print_all)?;
        }
        Ok(())
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
                    .expect("series not found")
                    .as_str()
                    .parse::<usize>()
                    .unwrap();
                let chapter_nr = capture
                    .name("chapter")
                    .map(|s| s.as_str().parse::<usize>().unwrap());

                let found_s = &self.data[series_nr - 1];
                if !just_series {
                    if let Some(chapter_nr) = chapter_nr {
                        return Some(ArchiveSearchResult {
                            chapter: found_s
                                .chapters
                                .iter()
                                .find(|ch| ch.nr.nr == chapter_nr)
                                .unwrap_or_else(|| {
                                    panic!("no chapter with identifier '{identifier}'")
                                }),
                        });
                    }
                }
                Some(ArchiveSearchResult { series: found_s })
            }
            None => self
                .get_series_by_name(identifier)
                .map(|series| ArchiveSearchResult { series }),
        }
    }

    #[must_use]
    pub fn get_mut_series_by_name(&mut self, identifier: &str) -> Option<&mut Series> {
        self.data.iter_mut().find(|x| x.name == identifier)
    }
    #[must_use]
    pub fn get_series_by_name(&self, identifier: &str) -> Option<&Series> {
        self.data.iter().find(|x| x.name == identifier)
    }
}
pub union ArchiveSearchResult<'a> {
    series: &'a Series,
    chapter: &'a Chapter,
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
    pub fn format(
        &self,
        s: &mut impl Write,
        indent: &str,
        print_chapters: bool,
    ) -> Result<(), std::fmt::Error> {
        s.write_str(&self.name)?;
        s.write_char('\n')?;
        if print_chapters {
            let mut nr_len = 0;
            let mut contains_extra = false;
            for chapter in &self.chapters {
                nr_len = nr_len.max((chapter.nr.nr as f64).log10().ceil() as usize);
                contains_extra |= chapter.nr.is_maybe;
            }
            for chapter in &self.chapters {
                s.write_str(indent)?;
                chapter.format(s, Some((nr_len, false)), contains_extra)?;
                s.write_char('\n')?;
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

impl Chapter {
    fn new(nr: ChapterNumber, name: Option<String>) -> Self {
        Self {
            nr,
            name,
            parts: HashMap::new(),
        }
    }
    pub fn format(
        &self,
        s: &mut impl Write,
        r_just: Option<(usize, bool)>,
        l_just: bool,
    ) -> Result<(), std::fmt::Error> {
        self.nr.format(s, r_just, l_just)?;
        s.write_str(" - ")?;
        if let Some(name) = &self.name {
            write!(s, "{name} ")?;
        }
        s.write_char('[')?;
        s.write_str(&self.parts.keys().sorted().join(", "))?;
        s.write_char(']')?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[must_use]
pub struct ChapterNumber {
    nr: usize,
    is_maybe: bool,
}
impl ChapterNumber {
    pub const fn new(nr: usize, is_maybe: bool) -> Self {
        Self { nr, is_maybe }
    }
    #[must_use]
    pub const fn nr(&self) -> usize {
        self.nr
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
    /// let nr = ChapterNumber::new(3, true);
    /// let mut s1 = String::new();
    /// nr.format(&mut s1, None, false).unwrap();
    /// assert_eq!(s1, "3?");
    ///
    /// let mut s2 = String::new();
    /// nr.format(&mut s2, Some((4, true)), false).unwrap();
    /// assert_eq!(s2, "0003?");
    ///
    /// let nr = ChapterNumber::new(3, false);
    /// let mut s1 = String::new();
    /// nr.format(&mut s1, Some((3, false)), true).unwrap();
    /// assert_eq!(s1, "  3 ");
    ///
    /// let mut s2 = String::new();
    /// nr.format(&mut s2, Some((4, true)), true).unwrap();
    /// assert_eq!(s2, "0003 ");
    /// ```
    pub fn format(
        &self,
        s: &mut impl Write,
        r_just: Option<(usize, bool)>,
        l_just: bool,
    ) -> Result<(), std::fmt::Error> {
        match r_just {
            Some(r_just) => {
                if r_just.1 {
                    write!(s, "{:0width$}", self.nr, width = r_just.0)
                } else {
                    write!(s, "{:width$}", self.nr, width = r_just.0)
                }
            }
            None => write!(s, "{}", self.nr),
        }?;
        if self.is_maybe {
            s.write_char('?')?;
        } else if l_just {
            s.write_char(' ')?;
        }
        Ok(())
    }
}
impl Display for ChapterNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f, None, false)
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
    /// assert_eq!(Ok(ChapterNumber::new(3, true)), "3?".parse::<ChapterNumber>());
    /// assert_eq!(Ok(ChapterNumber::new(3, false)), "3".parse::<ChapterNumber>());
    /// assert_eq!(Ok(ChapterNumber::new(3, true)), "003?".parse::<ChapterNumber>());
    /// assert_eq!(Ok(ChapterNumber::new(3, false)), " 3 ".parse::<ChapterNumber>());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.trim();
        let strip = value.strip_suffix('?');
        Ok(Self {
            nr: strip.unwrap_or(value).parse::<usize>()?,
            is_maybe: strip.is_some(),
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
    const FILE_DATE_FMT: &str = "%Y_%m_%d";
    const DISPLAY_DATE_FMT: &str = "%Y-%m-%d";
    pub fn from_path<P: AsRef<Path>>(value: &P) -> Result<Self, SourceErrorKind> {
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
    /// use audio_matcher::archive::data::Source;
    /// use audio_matcher::archive::data::SourceErrorKind;
    ///
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

    mod series_tests {
        use super::*;

        #[test]
        fn format() {
            let mut ser = Series::new("gute show".to_owned());
            ser.chapters.push(Chapter::new(
                ChapterNumber::new(5, true),
                Some("unbekannt".to_owned()),
            ));
            ser.chapters.push(Chapter::new(
                ChapterNumber::new(6, false),
                Some("bekannt".to_owned()),
            ));
            let mut s = String::new();
            ser.format(&mut s, ".", true).unwrap();
            assert_eq!("gute show\n.5? - unbekannt []\n.6  - bekannt []\n", s);
        }
    }

    mod chapter_tests {
        use super::*;

        #[test]
        fn format_with_parts() {
            let mut ch = Chapter::new(ChapterNumber::new(15, false), None);
            ch.parts.insert("station-2023_1_1".parse().unwrap(), 2);
            let mut s = String::new();
            ch.format(&mut s, None, false).unwrap();
            assert_eq!("15 - [station - 2023-01-01]", s);
            ch.parts.insert("station-2023_1_2".parse().unwrap(), 2);

            s.clear();
            ch.format(&mut s, None, false).unwrap();
            assert_eq!("15 - [station - 2023-01-01, station - 2023-01-02]", s);
        }

        #[test]
        fn format_with_name() {
            let ch = Chapter::new(
                ChapterNumber::new(15, false),
                Some("chapter name".to_owned()),
            );
            let mut s = String::new();
            ch.format(&mut s, None, false).unwrap();
            assert_eq!("15 - chapter name []", s);
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
                Source::from_path(&"/89.0rtl-2023_06_17.mp3")
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
            let nr = ChapterNumber::new(3, false);
            let mut s1 = String::new();
            nr.format(&mut s1, None, false).unwrap();
            assert_eq!(s1, "3");

            let nr = ChapterNumber::new(30, true);
            let mut s1 = String::new();
            nr.format(&mut s1, None, false).unwrap();
            assert_eq!(s1, "30?");
        }
        #[test]
        fn format_0_r_just() {
            let nr = ChapterNumber::new(3, false);
            let mut s1 = String::new();
            nr.format(&mut s1, Some((4, true)), false).unwrap();
            assert_eq!(s1, "0003");

            let nr = ChapterNumber::new(30, true);
            let mut s1 = String::new();
            nr.format(&mut s1, Some((4, true)), false).unwrap();
            assert_eq!(s1, "0030?");
        }
        #[test]
        fn format_space_r_just() {
            let nr = ChapterNumber::new(3, false);
            let mut s1 = String::new();
            nr.format(&mut s1, Some((4, false)), false).unwrap();
            assert_eq!(s1, "   3");

            let nr = ChapterNumber::new(30, true);
            let mut s1 = String::new();
            nr.format(&mut s1, Some((4, false)), false).unwrap();
            assert_eq!(s1, "  30?");
        }
        #[test]
        fn format_l_just() {
            let nr = ChapterNumber::new(3, false);
            let mut s1 = String::new();
            nr.format(&mut s1, None, true).unwrap();
            assert_eq!(s1, "3 ");

            let nr = ChapterNumber::new(30, true);
            let mut s1 = String::new();
            nr.format(&mut s1, None, true).unwrap();
            assert_eq!(s1, "30?");
        }
    }
}
