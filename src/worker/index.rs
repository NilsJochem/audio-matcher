use std::path::{Path, PathBuf};

use super::args::Arguments;
use crate::archive::data::ChapterNumber;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("failed to parse {0:?} with {1:?}")]
    ParseError(String, Parser),
    #[error("cant read {0:?} because {1:?}")]
    IO(PathBuf, std::io::ErrorKind),
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Parser {
    WithoutArtist,
    WithArtist,
    TryWithArtist,
}
impl Parser {
    fn parse_line(self, line: &str) -> Option<(String, Option<String>)> {
        match self {
            Self::WithoutArtist => Some((line.to_owned(), None)),
            Self::WithArtist => line
                .rsplit_once(" - ")
                .map(|(name, author)| (name.to_owned(), Some(author.to_owned()))),
            Self::TryWithArtist => Self::WithArtist
                .parse_line(line)
                .or_else(|| Self::WithoutArtist.parse_line(line)),
        }
    }
}

#[derive(Debug)]
pub struct Index {
    data: Vec<(String, Option<String>)>,
}
impl Index {
    pub async fn try_get_index(args: &Arguments, series: &str) -> Result<Option<Self>, Error> {
        Ok(match args.index_folder() {
            Some(folder) => Self::try_read_index(folder.clone(), series).await?,
            None => {
                let path = args
                    .always_answer()
                    .try_input(
                        "welche Index Datei m\u{f6}chtest du verwenden?: ",
                        Some(None),
                        |it| Some(Some(PathBuf::from(it))),
                    )
                    .unwrap_or_else(|| unreachable!());
                match path {
                    Some(path) => Some(Self::from_path(path, Parser::WithoutArtist).await?),
                    None => None,
                }
            }
        })
    }
    #[allow(clippy::doc_markdown)]
    /// returns None if neither "index.txt", nor "index_full.txt" exists in `base_folder`
    async fn try_read_index(mut base_folder: PathBuf, series: &str) -> Result<Option<Self>, Error> {
        base_folder.push(series);
        base_folder.push("index_full.txt");
        if file_exists(&base_folder).await? {
            Self::from_path(base_folder, Parser::WithArtist)
                .await
                .map(Some)
        } else {
            base_folder.set_file_name("index.txt");
            if file_exists(&base_folder).await? {
                Self::from_path(base_folder, Parser::WithoutArtist)
                    .await
                    .map(Some)
            } else {
                Ok(None)
            }
        }
    }
    async fn from_path<P>(path: P, parser: Parser) -> Result<Self, Error>
    where
        P: AsRef<Path> + Send + Clone,
    {
        let path_copy_for_error = path.as_ref().to_path_buf();
        Self::from_slice_iter(
            tokio::fs::read_to_string(path)
                .await
                .map_err(|err| Error::IO(path_copy_for_error, err.kind()))?
                .lines(),
            parser,
        )
    }
    pub fn from_slice_iter<Iter>(data: Iter, parser: Parser) -> Result<Self, Error>
    where
        Iter: Iterator,
        Iter::Item: AsRef<str>,
    {
        Ok(Self {
            data: data
                .filter(|line| !line.as_ref().trim_start().starts_with('#'))
                .map(|line| {
                    parser
                        .parse_line(line.as_ref())
                        .ok_or_else(|| Error::ParseError(line.as_ref().to_owned(), parser))
                })
                .collect::<Result<_, Error>>()?,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }
    #[allow(dead_code)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[must_use]
    pub fn get(&self, chapter_number: ChapterNumber) -> (&str, Option<&str>) {
        let (n, a) = &self.data[chapter_number.nr() - 1];
        (n, a.as_ref().map(std::string::String::as_str))
    }
    #[allow(dead_code)]
    #[must_use]
    pub fn try_get(&self, chapter_number: ChapterNumber) -> Option<(&str, Option<&str>)> {
        self.data
            .get(chapter_number.nr() - 1)
            .map(|(n, a)| (n.as_str(), a.as_ref().map(std::string::String::as_str)))
    }
}

async fn file_exists(base_folder: &PathBuf) -> Result<bool, Error> {
    tokio::fs::try_exists(base_folder)
        .await
        .map_err(|err| Error::IO(base_folder.clone(), err.kind()))
}

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
        let index = Index::from_slice_iter(data.into_iter(), Parser::WithoutArtist).unwrap();
        assert_eq!(index.get(ChapterNumber::new(1, false)), (data[0], None));
        assert_eq!(index.get(ChapterNumber::new(2, false)), (data[1], None));
        assert_eq!(index.get(ChapterNumber::new(3, false)), (data[3], None));
        assert_eq!(index.try_get(ChapterNumber::new(4, false)), None);
    }

    #[test]
    fn read_with_artist() {
        let data = [
            ("first element", Some("author 1")),
            ("second element", Some("author 2")),
            ("# some comment", None),
            ("third element - some extra", Some("author 1")),
        ];
        let index = Index::from_slice_iter(
            data.into_iter().map(|(n, a)| {
                let mut s = n.to_owned();
                if let Some(a) = a {
                    s.push_str(" - ");
                    s.push_str(a);
                }
                s
            }),
            Parser::WithArtist,
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
            Error::ParseError(data[1].to_owned(), Parser::WithArtist),
            Index::from_slice_iter(data.into_iter(), Parser::WithArtist).unwrap_err()
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
            Index::from_slice_iter(data.into_iter(), Parser::TryWithArtist)
                .unwrap()
                .len()
        );
    }
}
