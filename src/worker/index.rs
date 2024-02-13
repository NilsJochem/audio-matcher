use itertools::Itertools;
use log::warn;
use regex::Regex;
use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
    ffi::{OsStr, OsString},
    fmt::Debug,
    path::{Path, PathBuf},
};
use toml::value::Datetime;

use crate::archive::data::ChapterNumber;
use common::extensions::cow::Ext;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("failed to parse {0:?} with {1:?}")]
    Parse(String, parser::Txt),
    #[error(transparent)]
    Serde(#[from] toml::de::Error),
    #[error("cant read {0:?} because {1:?}")]
    IO(PathBuf, std::io::ErrorKind),
    #[error("couldn't find the given series")]
    SeriesNotFound,
    #[error("couldn't an index file")]
    NoIndexFile,
    #[error("only supporting .toml and .txt, but got {}", .0.as_deref().map(|it| format!(".{it}")).as_deref().unwrap_or("None"))]
    NotSupportedFile(Option<String>),
}
impl Error {
    fn io_err(path: impl AsRef<Path>, err: &std::io::Error) -> Self {
        Self::IO(path.as_ref().to_path_buf(), err.kind())
    }
    fn parse_err(line: impl AsRef<str>, parser: parser::Txt) -> Self {
        Self::Parse(line.as_ref().to_owned(), parser)
    }
}
pub mod parser {
    use std::borrow::Cow;

    use super::{ChapterEntry, Error};

    pub(super) use Parser::Toml; // exposing Toml directly to be used like Txt::<variant>

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub(super) enum Parser {
        Toml,
        Txt(Txt),
    }
    #[allow(clippy::enum_variant_names)]
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub enum Txt {
        WithoutArtist,
        WithArtist,
        TryWithArtist,
    }
    impl From<Txt> for Parser {
        fn from(value: Txt) -> Self {
            Self::Txt(value)
        }
    }
    impl Txt {
        /// parses `line` with `self` and takes ownership of the values
        pub(super) fn parse_line_owned<'b>(
            self,
            line: impl AsRef<str>,
        ) -> Result<ChapterEntry<'b>, Error> {
            self.parse_line(line.as_ref(), |it| Cow::Owned(it.to_owned()))
        }
        #[allow(dead_code)]
        /// parses `line` with `self` and references the orignal data
        pub(super) fn parse_line_borrowed(self, line: &str) -> Result<ChapterEntry, Error> {
            self.parse_line(line, Cow::Borrowed)
        }
        fn parse_line<'a, 'b>(
            self,
            line: &'a str,
            map_to_cow: impl Fn(&'a str) -> Cow<'b, str> + Clone,
        ) -> Result<ChapterEntry<'b>, Error> {
            match self {
                Self::WithoutArtist => Ok(ChapterEntry {
                    title: map_to_cow(line),
                    artist: None,
                    release: None,
                }),
                Self::WithArtist => line
                    .rsplit_once(" - ")
                    .map(|(name, author)| ChapterEntry {
                        title: map_to_cow(name),
                        artist: Some(map_to_cow(author)),
                        release: None,
                    })
                    .ok_or_else(|| Error::parse_err(line, self)),
                Self::TryWithArtist => Self::WithArtist
                    .parse_line(line, map_to_cow.clone())
                    .or_else(|_| Self::WithoutArtist.parse_line(line, map_to_cow)),
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct Index<'a> {
    url: Option<Cow<'a, str>>,
    artist: Option<Cow<'a, str>>,
    release: Option<DateOrYear>,
    #[serde(flatten)]
    part: IndexPart<'a>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum IndexPart<'a> {
    SubSeries { subseries: Vec<SubSeriesHolder<'a>> },
    Direct { chapters: Chapters<'a> },
}
#[derive(Debug, Deserialize, Clone)]
struct SubSeriesHolder<'a> {
    #[allow(dead_code)]
    name: Cow<'a, str>,
    chapters: Vec<ChapterEntry<'a>>,
}
#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
struct Chapters<'a> {
    #[serde(default)]
    main: Vec<ChapterEntry<'a>>,
    #[serde(default)]
    extra: Vec<ChapterEntry<'a>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize)]
#[serde(untagged)]
pub enum DateOrYear {
    Date(Datetime),
    Year(u16),
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone, Deserialize)]
#[serde(from = "RawChapterEntry")]
pub struct ChapterEntry<'a> {
    pub title: Cow<'a, str>,
    pub artist: Option<Cow<'a, str>>,
    pub release: Option<DateOrYear>,
}
impl<'a> ChapterEntry<'a> {
    fn new(
        title: Cow<'a, str>,
        artist: impl Into<Option<Cow<'a, str>>>,
        release: impl Into<Option<DateOrYear>>,
    ) -> ChapterEntry<'a> {
        Self {
            title,
            artist: artist.into(),
            release: release.into(),
        }
    }

    fn rename_empty_chapters(chapters: &mut [Self], series: impl AsRef<str>) {
        chapters
            .iter_mut()
            .zip(1..)
            .filter(|(chapter, _)| chapter.title == "")
            .for_each(|(chapter, i)| {
                chapter.title = Cow::Owned(format!("{} {i}", series.as_ref()));
            });
    }

    /// trys to fill None values
    fn fill(
        &'a self,
        artist: impl FnOnce() -> Option<Cow<'a, str>>,
        release: impl FnOnce() -> Option<DateOrYear>,
    ) -> Self {
        Self {
            title: self.title.reborrow(),
            artist: self.artist.reborrow().or_else(artist),
            release: self.release.or_else(release),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawChapterEntry<'a> {
    JustTitel(Cow<'a, str>),
    WithArtist((Cow<'a, str>, Cow<'a, str>)),
    WithDate((Cow<'a, str>, DateOrYear)),
    WithDateAndArtist((Cow<'a, str>, Cow<'a, str>, DateOrYear)),
}
impl<'a> From<RawChapterEntry<'a>> for ChapterEntry<'a> {
    fn from(value: RawChapterEntry<'a>) -> Self {
        match value {
            RawChapterEntry::JustTitel(title) => Self::new(title, None, None),
            RawChapterEntry::WithArtist((title, artist)) => Self::new(title, artist, None),
            RawChapterEntry::WithDate((title, date)) => Self::new(title, None, date),
            RawChapterEntry::WithDateAndArtist((title, artist, date)) => {
                Self::new(title, artist, date)
            }
        }
    }
}

impl Index<'static> {
    pub async fn try_read_from_path(path: impl AsRef<Path> + Send + Sync) -> Result<Self, Error> {
        match path.as_ref().extension().and_then(OsStr::to_str) {
            Some("toml") => Self::try_from_path(path, parser::Toml).await,
            Some("txt") => Self::try_from_path(path, parser::Txt::TryWithArtist).await,
            Some(ext) => Err(Error::NotSupportedFile(Some(ext.to_owned()))),
            None => Err(Error::NotSupportedFile(None)),
        }
        .and_then(|it| it.ok_or(Error::NoIndexFile))
    }

    pub async fn try_read_index(
        mut folder: PathBuf,
        series: impl AsRef<OsStr> + Send,
    ) -> Result<Self, Error> {
        folder.push(series.as_ref());
        Self::file_exists(&folder)
            .await
            .and_then(|exists| exists.then_some(()).ok_or(Error::SeriesNotFound))?;

        folder.push("index.toml");
        if let Some(index) = Self::try_from_path(&folder, parser::Toml).await? {
            return Ok(index);
        }
        folder.set_file_name("index_full.txt");
        if let Some(index) = Self::try_from_path(&folder, parser::Txt::WithArtist).await? {
            return Ok(index);
        }
        folder.set_file_name("index.txt");
        if let Some(index) = Self::try_from_path(&folder, parser::Txt::WithoutArtist).await? {
            return Ok(index);
        }
        Err(Error::NoIndexFile)
    }

    async fn try_from_path(
        path: impl AsRef<Path> + Send + Sync,
        parser: impl Into<parser::Parser> + Send,
    ) -> Result<Option<Self>, Error> {
        if Self::file_exists(&path).await? {
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|err| Error::io_err(&path, &err))?;
            let name = path.as_ref().with_extension("");
            let name = name.file_name().unwrap().to_string_lossy();
            match parser.into() {
                parser::Parser::Toml => Self::from_toml_str(content, name),
                parser::Parser::Txt(parser) => Self::from_slice_iter(content.lines(), name, parser),
            }
            .map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn from_toml_str(content: impl AsRef<str>, name: impl AsRef<str>) -> Result<Self, Error> {
        let mut index: Self = toml::from_str(content.as_ref())?;
        index.rename_empty_chapters(name);
        Ok(index)
    }
    pub fn from_slice_iter<Iter>(
        iter: Iter,
        name: impl AsRef<str>,
        parser: parser::Txt,
    ) -> Result<Self, Error>
    where
        Iter: Iterator,
        Iter::Item: AsRef<str>,
    {
        iter.filter(|line| !line.as_ref().trim_start().starts_with('#'))
            .map(|line| parser.parse_line_owned(line))
            .collect::<Result<_, Error>>()
            .map(|data| {
                let mut index = Self {
                    artist: None,
                    release: None,
                    url: None,
                    part: IndexPart::Direct {
                        chapters: Chapters {
                            main: data,
                            extra: Vec::new(),
                        },
                    },
                };
                index.rename_empty_chapters(name);
                index
            })
    }
}

impl<'a> Index<'a> {
    fn rename_empty_chapters(&mut self, name: impl AsRef<str>) {
        match &mut self.part {
            IndexPart::SubSeries { subseries } => {
                for sub in subseries {
                    ChapterEntry::rename_empty_chapters(&mut sub.chapters, &sub.name);
                }
            }
            IndexPart::Direct { chapters } => {
                ChapterEntry::rename_empty_chapters(&mut chapters.main, name);
            }
        };
    }

    async fn file_exists(base_folder: impl AsRef<Path> + Send + Sync) -> Result<bool, Error> {
        let exists = tokio::fs::try_exists(&base_folder)
            .await
            .map_err(|err| Error::io_err(&base_folder, &err))?;
        if !exists {
            log::trace!("couldn't find {:?}", base_folder.as_ref().display());
        }
        Ok(exists)
    }

    #[must_use]
    pub fn main_len(&self) -> usize {
        match &self.part {
            IndexPart::Direct { chapters } => chapters.main.len(),
            IndexPart::SubSeries { subseries } => {
                subseries.iter().map(|it| it.chapters.len()).sum()
            }
        }
    }
    #[must_use]
    pub fn chapter_iter(&'a self) -> Box<dyn Iterator<Item = ChapterEntry> + 'a> {
        let iter: Box<dyn Iterator<Item = _>> = match &self.part {
            IndexPart::Direct { chapters } => Box::new(chapters.main.iter()),
            IndexPart::SubSeries { subseries } => {
                Box::new(subseries.iter().flat_map(|it| it.chapters.iter()))
            }
        };
        Box::new(iter.map(|entry| self.fill(entry)))
    }
    #[allow(dead_code)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match &self.part {
            IndexPart::Direct { chapters } => chapters.main.is_empty() && chapters.extra.is_empty(),
            IndexPart::SubSeries { subseries } => subseries.iter().all(|it| it.chapters.is_empty()),
        }
    }

    #[must_use]
    pub fn get(&self, chapter_number: ChapterNumber) -> ChapterEntry {
        self.try_get(chapter_number).expect("can't find chapter")
    }

    #[must_use]
    pub fn try_get(&self, chapter_number: ChapterNumber) -> Option<ChapterEntry> {
        match &self.part {
            IndexPart::Direct { chapters } => chapters
                .main
                .get(chapter_number.nr - 1)
                .map(|it| self.fill(it)),
            IndexPart::SubSeries { subseries: _ } => todo!(),
        }
    }

    fn fill(&'a self, it: &'a ChapterEntry<'a>) -> ChapterEntry<'a> {
        it.fill(|| self.artist.reborrow(), || self.release)
    }
}
// doesn't work, because get returns a copy
// impl<'a> std::ops::Index<ChapterNumber> for Index<'a> {
//     type Output = ChapterEntry<'a>;

//     fn index(&self, index: ChapterNumber) -> &Self::Output {
//         self.get(index)
//     }
// }

#[allow(clippy::module_name_repetitions)]
pub struct MultiIndex<'a> {
    folder: PathBuf,
    data: HashMap<OsString, Index<'a>>,
}
impl<'i> Debug for MultiIndex<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiIndex")
            .field("folder", &self.folder)
            .field("data", &self.data.keys())
            .finish()
    }
}
impl MultiIndex<'static> {
    #[must_use]
    pub async fn new(folder: PathBuf) -> Self {
        let data = Self::possible(&folder).await;
        Self { folder, data }
    }
}

impl<'a> MultiIndex<'a> {
    pub const SUBSERIES_DELIMENITER: &'static str = ": ";
    async fn possible(path: impl AsRef<Path> + Send + Sync) -> HashMap<OsString, Index<'a>> {
        let path = path.as_ref();
        let mut known = HashMap::new();

        let paths = glob_expanded(path.join("**/*.{toml, txt}"))
            .unwrap()
            .collect::<Vec<_>>();

        for path in paths {
            let path = path.unwrap();
            let with_extension = path.with_extension("");
            let name = with_extension
                .file_name()
                .filter(|&it| {
                    let it = it.to_string_lossy();
                    it != "index" && it != "index_full"
                })
                .or_else(|| path.parent().unwrap().file_name())
                .expect("need filename")
                .to_owned();
            match Index::try_read_from_path(&path).await {
                Ok(index) => match index.part {
                    IndexPart::SubSeries { subseries } => {
                        for sub in subseries {
                            let mut name = name.clone();
                            name.push(Self::SUBSERIES_DELIMENITER);
                            name.push(sub.name.as_ref());

                            known.insert(
                                name,
                                Index {
                                    url: index.url.clone(),
                                    artist: index.artist.clone(),
                                    release: index.release,
                                    part: IndexPart::Direct {
                                        chapters: Chapters {
                                            main: sub.chapters,
                                            extra: Vec::new(),
                                        },
                                    },
                                },
                            );
                        }
                    }
                    IndexPart::Direct { chapters: _ } => {
                        known.insert(name, index);
                    }
                },
                Err(err) => warn!("failed to open index at {} because {err}", path.display()),
            }
        }

        known
    }
    pub async fn reload(&mut self) {
        self.data = Self::possible(&self.folder).await;
    }
    pub fn get_possible(&self) -> impl IntoIterator<Item = &OsStr> {
        self.data.keys().map(OsString::as_ref).sorted()
    }
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.folder
    }

    pub fn has_index(&self, series: &OsString) -> bool {
        self.data.contains_key(series)
    }
    pub fn get_known_index(&mut self, series: &OsString) -> Option<&Index<'a>> {
        self.data.get(series)
    }

    pub async fn get_index(&mut self, series: OsString) -> Result<&Index<'a>, Error> {
        if let Entry::Vacant(entry) = self.data.entry(series.clone()) {
            entry.insert(Index::try_read_index(self.folder.clone(), series.clone()).await?);
        }
        Ok(self.data.get(&series).unwrap())
    }
}

/// expands first "a{b1, b2, ...}c" into \["ab1c", "ab2c", ...\]
fn split_pattern(pattern: &str) -> Vec<Cow<'_, str>> {
    const REG_PRE: &str = "pre";
    const REG_OPTIONS: &str = "opt";
    const REG_POST: &str = "post";
    lazy_static::lazy_static! {
        static ref RE: Regex = Regex::new(&format!("^(?P<{REG_PRE}>.*?)(?:\\{{(?P<{REG_OPTIONS}>.+?)\\}}(?P<{REG_POST}>.*)$)?$")).unwrap();
    }
    let binding = RE.captures(pattern).unwrap();
    let pre = binding
        .name(REG_PRE)
        .expect("expecting at least pre to match")
        .as_str();
    let options = binding.name(REG_OPTIONS).map(|options| {
        (
            options.as_str().split(", "),
            binding.name(REG_POST).expect("need post match").as_str(),
        )
    });
    if let Some((options, post)) = options {
        options
            .map(|option| Cow::Owned(format!("{pre}{option}{post}")))
            .collect_vec()
    } else {
        vec![Cow::Borrowed(pre)]
    }
}
fn glob_expanded(
    pattern: impl AsRef<OsStr>,
) -> Result<impl Iterator<Item = Result<PathBuf, glob::GlobError>>, glob::PatternError> {
    Ok(split_pattern(
        pattern
            .as_ref()
            .to_str()
            .expect("currently only supporting UTF-8"),
    )
    .into_iter()
    .map(|it| glob::glob(it.as_ref()))
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .flatten())
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;

    #[test]
    fn multipattern() {
        assert_eq!(
            vec!["path/*.toml", "path/*.txt"],
            split_pattern("path/*.{toml, txt}")
        );
    }
    #[tokio::test]
    async fn list_possibilitys() {
        let m_index =
            MultiIndex::new("/home/nilsj/Musik/newly ripped/Aufnahmen/current".into()).await;
        assert_eq!(
            vec![
                "Gruselkabinett",
                "Kassandras Kinder",
                "Sherlock Holmes",
                "Terra Mortis",
                "test"
            ],
            m_index.get_possible().into_iter().collect_vec()
        );
    }

    #[test]
    fn filter_comments() {
        let data = [
            "first element",
            "second element",
            "# some comment",
            "third element",
        ];
        let index =
            Index::from_slice_iter(data.into_iter(), "not used", parser::Txt::WithoutArtist)
                .unwrap();
        assert_eq!(
            index.get(ChapterNumber {
                nr: 1,
                is_maybe: false,
                is_partial: false
            }),
            ChapterEntry {
                title: Cow::Borrowed(data[0]),
                artist: None,
                release: None
            }
        );
        assert_eq!(
            index.get(ChapterNumber {
                nr: 2,
                is_maybe: false,
                is_partial: false
            }),
            ChapterEntry {
                title: Cow::Borrowed(data[1]),
                artist: None,
                release: None
            }
        );
        assert_eq!(
            index.get(ChapterNumber {
                nr: 3,
                is_maybe: false,
                is_partial: false
            }),
            ChapterEntry {
                title: Cow::Borrowed(data[3]),
                artist: None,
                release: None
            }
        );
        assert_eq!(
            index.try_get(ChapterNumber {
                nr: 4,
                is_maybe: false,
                is_partial: false
            }),
            None
        );
    }
    #[test]
    fn rename_empty() {
        let data = ["", "first element", "", "# some comment", ""];
        let index =
            Index::from_slice_iter(data.into_iter(), "series", parser::Txt::WithoutArtist).unwrap();
        assert_eq!("series 1", index.get(ChapterNumber::from(1)).title);
        assert_eq!(data[1], index.get(ChapterNumber::from(2)).title);
        assert_eq!("series 3", index.get(ChapterNumber::from(3)).title);
        assert_eq!("series 4", index.get(ChapterNumber::from(4)).title);
        assert_eq!(None, index.try_get(ChapterNumber::from(5)));
    }

    #[test]
    fn read_with_artist() {
        let data = [
            ChapterEntry {
                title: Cow::Borrowed("first element"),
                artist: Some(Cow::Borrowed("author 1")),
                release: None,
            },
            ChapterEntry {
                title: Cow::Borrowed("second element"),
                artist: Some(Cow::Borrowed("author 2")),
                release: None,
            },
            ChapterEntry {
                title: Cow::Borrowed("# some comment"),
                artist: None,
                release: None,
            },
            ChapterEntry {
                title: Cow::Borrowed("third element - some extra"),
                artist: Some(Cow::Borrowed("author 1")),
                release: None,
            },
        ];
        let index = Index::from_slice_iter(
            data.iter().cloned().map(|it| {
                let mut s = it.title.as_ref().to_owned();
                if let Some(a) = it.artist {
                    s.push_str(" - ");
                    s.push_str(&a);
                }
                s
            }),
            "not used",
            parser::Txt::WithArtist,
        )
        .unwrap();
        assert_eq!(
            index.get(ChapterNumber {
                nr: 1,
                is_maybe: false,
                is_partial: false
            }),
            data[0]
        );
        assert_eq!(
            index.get(ChapterNumber {
                nr: 2,
                is_maybe: false,
                is_partial: false
            }),
            data[1]
        );
        assert_eq!(
            index.get(ChapterNumber {
                nr: 3,
                is_maybe: false,
                is_partial: false
            }),
            data[3]
        );
        assert_eq!(
            index.try_get(ChapterNumber {
                nr: 4,
                is_maybe: false,
                is_partial: false
            }),
            None
        );
    }

    #[test]
    fn fail_to_read() {
        let data = [
            "# some comment",
            "first element",
            "second element - with author",
        ];
        assert_eq!(
            Error::Parse(data[1].to_owned(), parser::Txt::WithArtist),
            Index::from_slice_iter(data.into_iter(), "not used", parser::Txt::WithArtist)
                .unwrap_err()
        );
    }
    #[test]
    fn detect_comments() {
        let data = [
            "# some comment",
            "first element",
            "     # comment with some spaces",
            "\t# comment with tabs",
            "   \t  \t # comment with spaces and tabs",
            "second element - with author",
        ];
        assert_eq!(
            2,
            Index::from_slice_iter(data.into_iter(), "not used", parser::Txt::TryWithArtist)
                .unwrap()
                .main_len()
        );
    }

    #[test]
    fn read_toml_with_one_artist() {
        let index = Index::from_toml_str(
            r#"
            artist = "artist"
            chapters.main = [
                "chapter 1", "chapter 2", ["chapter 3", "other artist"]
            ]
        "#,
            "not used",
        )
        .unwrap();
        assert_eq!(
            ChapterEntry {
                title: Cow::Borrowed("chapter 1"),
                artist: Some(Cow::Borrowed("artist")),
                release: None
            },
            index.get(ChapterNumber {
                nr: 1,
                is_maybe: false,
                is_partial: false
            })
        );
        assert_eq!(
            ChapterEntry {
                title: Cow::Borrowed("chapter 2"),
                artist: Some(Cow::Borrowed("artist")),
                release: None
            },
            index.get(ChapterNumber {
                nr: 2,
                is_maybe: false,
                is_partial: false
            })
        );
        assert_eq!(
            ChapterEntry {
                title: Cow::Borrowed("chapter 3"),
                artist: Some(Cow::Borrowed("other artist")),
                release: None
            },
            index.get(ChapterNumber {
                nr: 3,
                is_maybe: false,
                is_partial: false
            })
        );
        assert_eq!(
            None,
            index.try_get(ChapterNumber {
                nr: 4,
                is_maybe: false,
                is_partial: false
            })
        );
    }

    #[test]
    fn read_toml_dates() {
        let index = Index::from_toml_str(
            r#"
            artist = "artist"
            release = 2000
            chapters.main = [
                "chapter 1",
                ["chapter 2", 2001],
                ["chapter 3", 2002-02-02],
                ["chapter 4", "other artist", 2003-03-03]
            ]
            "#,
            "not used",
        )
        .unwrap();
        assert_eq!(
            Some(DateOrYear::Year(2000)),
            index
                .get(ChapterNumber {
                    nr: 1,
                    is_maybe: false,
                    is_partial: false
                })
                .release
        );
        assert_eq!(
            Some(DateOrYear::Year(2001)),
            index
                .get(ChapterNumber {
                    nr: 2,
                    is_maybe: false,
                    is_partial: false
                })
                .release
        );
        assert!(matches!(
            index.get(ChapterNumber { nr: 3, is_maybe: false, is_partial: false }).release.as_ref().unwrap(),
            DateOrYear::Date(date) if date.date.unwrap().year == 2002
        ));
        assert!(matches!(
            index.get(ChapterNumber { nr: 4, is_maybe: false, is_partial: false }).release.as_ref().unwrap(),
            DateOrYear::Date(date) if date.date.unwrap().year == 2003
        ));
    }
}
