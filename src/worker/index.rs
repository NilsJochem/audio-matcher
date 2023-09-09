use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
    ffi::OsStr,
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
    #[error("only supporting .toml and .txt")]
    NonSupportedFile,
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
#[derive(Debug, Deserialize)]
pub struct Index<'a> {
    url: Option<Cow<'a, str>>,
    artist: Option<Cow<'a, str>>,
    release: Option<DateOrYear>,
    chapters: Chapters<'a>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
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
            RawChapterEntry::JustTitel(title) => Self {
                title,
                artist: None,
                release: None,
            },
            RawChapterEntry::WithArtist((title, artist)) => Self {
                title,
                artist: Some(artist),
                release: None,
            },
            RawChapterEntry::WithDate((title, date)) => Self {
                title,
                artist: None,
                release: Some(date),
            },
            RawChapterEntry::WithDateAndArtist((title, artist, date)) => Self {
                title,
                artist: Some(artist),
                release: Some(date),
            },
        }
    }
}

impl<'a> Index<'a> {
    pub async fn try_read_from_path(
        path: impl AsRef<Path> + Send + Sync,
    ) -> Result<Index<'a>, Error> {
        match path.as_ref().extension().and_then(OsStr::to_str) {
            Some("toml") => Self::try_from_path(path, parser::Toml).await,
            Some("txt") => Self::try_from_path(path, parser::Txt::TryWithArtist).await,
            Some(_) | None => Err(Error::NonSupportedFile),
        }
        .and_then(|it| it.ok_or(Error::NoIndexFile))
    }

    pub async fn try_read_index(
        mut folder: PathBuf,
        series: impl AsRef<str> + Send,
    ) -> Result<Index<'a>, Error> {
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
    ) -> Result<Option<Index<'a>>, Error> {
        if Self::file_exists(&path).await? {
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|err| Error::io_err(path, &err))?;
            match parser.into() {
                parser::Parser::Toml => Self::from_toml_str(content),
                parser::Parser::Txt(parser) => Self::from_slice_iter(content.lines(), parser),
            }
            .map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn from_toml_str(content: impl AsRef<str>) -> Result<Self, Error> {
        Ok(toml::from_str(content.as_ref())?)
    }
    pub fn from_slice_iter<Iter>(iter: Iter, parser: parser::Txt) -> Result<Self, Error>
    where
        Iter: Iterator,
        Iter::Item: AsRef<str>,
    {
        iter.filter(|line| !line.as_ref().trim_start().starts_with('#'))
            .map(|line| parser.parse_line_owned(line))
            .collect::<Result<_, Error>>()
            .map(|data| Self {
                artist: None,
                release: None,
                url: None,
                chapters: Chapters {
                    main: data,
                    extra: Vec::new(),
                },
            })
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
        self.chapters.main.len()
    }
    #[allow(dead_code)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chapters.main.is_empty() && self.chapters.extra.is_empty()
    }

    #[must_use]
    pub fn get(&self, chapter_number: ChapterNumber) -> ChapterEntry {
        self.fill(&self.chapters.main[chapter_number.nr() - 1])
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn try_get(&self, chapter_number: ChapterNumber) -> Option<ChapterEntry> {
        self.chapters
            .main
            .get(chapter_number.nr() - 1)
            .map(|it| self.fill(it))
    }

    fn fill(&'a self, it: &'a ChapterEntry<'a>) -> ChapterEntry<'a> {
        it.fill(|| self.artist.reborrow(), || self.release)
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct MultiIndex<'a> {
    folder: PathBuf,
    data: HashMap<String, Index<'a>>,
}

impl<'a> MultiIndex<'a> {
    #[must_use]
    pub fn new(folder: PathBuf) -> Self {
        Self {
            folder,
            data: HashMap::new(),
        }
    }

    pub async fn get_index(&mut self, series: String) -> Result<&Index<'a>, Error> {
        if let Entry::Vacant(entry) = self.data.entry(series.clone()) {
            entry.insert(Index::try_read_index(self.folder.clone(), series.clone()).await?);
        }
        Ok(self.data.get(&series).unwrap())
    }
}

// doesn't work, because get returns a copy
// impl<'a> std::ops::Index<ChapterNumber> for Index<'a> {
//     type Output = ChapterEntry<'a>;

//     fn index(&self, index: ChapterNumber) -> &Self::Output {
//         self.get(index)
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_comments() {
        let data = [
            "first element",
            "second element",
            "# some comment",
            "third element",
        ];
        let index = Index::from_slice_iter(data.into_iter(), parser::Txt::WithoutArtist).unwrap();
        assert_eq!(
            index.get(ChapterNumber::new(1, false)),
            ChapterEntry {
                title: Cow::Borrowed(data[0]),
                artist: None,
                release: None
            }
        );
        assert_eq!(
            index.get(ChapterNumber::new(2, false)),
            ChapterEntry {
                title: Cow::Borrowed(data[1]),
                artist: None,
                release: None
            }
        );
        assert_eq!(
            index.get(ChapterNumber::new(3, false)),
            ChapterEntry {
                title: Cow::Borrowed(data[3]),
                artist: None,
                release: None
            }
        );
        assert_eq!(index.try_get(ChapterNumber::new(4, false)), None);
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
            parser::Txt::WithArtist,
        )
        .unwrap();
        assert_eq!(index.get(ChapterNumber::new(1, false)), data[0]);
        assert_eq!(index.get(ChapterNumber::new(2, false)), data[1]);
        assert_eq!(index.get(ChapterNumber::new(3, false)), data[3]);
        assert_eq!(index.try_get(ChapterNumber::new(4, false)), None);
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
            Index::from_slice_iter(data.into_iter(), parser::Txt::WithArtist).unwrap_err()
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
            Index::from_slice_iter(data.into_iter(), parser::Txt::TryWithArtist)
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
        )
        .unwrap();
        assert_eq!(
            ChapterEntry {
                title: Cow::Borrowed("chapter 1"),
                artist: Some(Cow::Borrowed("artist")),
                release: None
            },
            index.get(ChapterNumber::new(1, false))
        );
        assert_eq!(
            ChapterEntry {
                title: Cow::Borrowed("chapter 2"),
                artist: Some(Cow::Borrowed("artist")),
                release: None
            },
            index.get(ChapterNumber::new(2, false))
        );
        assert_eq!(
            ChapterEntry {
                title: Cow::Borrowed("chapter 3"),
                artist: Some(Cow::Borrowed("other artist")),
                release: None
            },
            index.get(ChapterNumber::new(3, false))
        );
        assert_eq!(None, index.try_get(ChapterNumber::new(4, false)));
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
        )
        .unwrap();
        assert_eq!(
            Some(DateOrYear::Year(2000)),
            index.get(ChapterNumber::new(1, false)).release
        );
        assert_eq!(
            Some(DateOrYear::Year(2001)),
            index.get(ChapterNumber::new(2, false)).release
        );
        assert!(matches!(
            index.get(ChapterNumber::new(3, false)).release.as_ref().unwrap(),
            DateOrYear::Date(date) if date.date.unwrap().year == 2002
        ));
        assert!(matches!(
            index.get(ChapterNumber::new(4, false)).release.as_ref().unwrap(),
            DateOrYear::Date(date) if date.date.unwrap().year == 2003
        ));
    }
}
