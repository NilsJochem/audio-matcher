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
impl Error {
    fn io_err(path: impl AsRef<Path>, err: &std::io::Error) -> Self {
        Self::IO(path.as_ref().to_path_buf(), err.kind())
    }
    fn parse_err(line: impl AsRef<str>, parser: Parser) -> Self {
        Self::ParseError(line.as_ref().to_owned(), parser)
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Parser {
    WithoutArtist,
    WithArtist,
    TryWithArtist,
}
impl Parser {
    fn parse_line(self, line: &str) -> Result<(String, Option<String>), Error> {
        match self {
            Self::WithoutArtist => Ok((line.to_owned(), None)),
            Self::WithArtist => line
                .rsplit_once(" - ")
                .map(|(name, author)| (name.to_owned(), Some(author.to_owned())))
                .ok_or_else(|| Error::parse_err(line, self)),
            Self::TryWithArtist => Self::WithArtist
                .parse_line(line)
                .or_else(|_| Self::WithoutArtist.parse_line(line)),
        }
    }
}

#[derive(Debug)]
pub struct Index {
    data: Vec<(String, Option<String>)>,
}
impl Index {
    pub async fn try_get_index<A>(args: &Arguments, series: A) -> Result<Option<Self>, Error>
    where
        A: AsRef<str> + Send,
    {
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
                    Some(path) => Self::try_from_path(&path, Parser::WithoutArtist).await?,
                    None => None,
                }
            }
        })
    }
    #[allow(clippy::doc_markdown)]
    /// returns None if neither "index.txt", nor "index_full.txt" exists in `base_folder`
    async fn try_read_index<A>(mut folder: PathBuf, series: A) -> Result<Option<Self>, Error>
    where
        A: AsRef<str> + Send,
    {
        folder.push(series.as_ref());
        folder.push("index_full.txt");
        match Self::try_from_path(&folder, Parser::WithArtist).await? {
            Some(index) => Ok(Some(index)),
            None => {
                folder.set_file_name("index.txt");
                Self::try_from_path(&folder, Parser::WithoutArtist).await
            }
        }
    }
    async fn try_from_path<P>(path: P, parser: Parser) -> Result<Option<Self>, Error>
    where
        P: AsRef<Path> + Send + Sync,
    {
        if Self::file_exists(&path).await? {
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => Self::from_slice_iter(content.lines(), parser).map(Some),
                Err(err) => Err(Error::io_err(path, &err)),
            }
        } else {
            log::trace!("couldn't find {:?}", path.as_ref().display());
            Ok(None)
        }
    }
    async fn file_exists(base_folder: impl AsRef<Path> + Send + Sync) -> Result<bool, Error> {
        tokio::fs::try_exists(&base_folder)
            .await
            .map_err(|err| Error::io_err(base_folder, &err))
    }

    pub fn from_slice_iter<Iter>(iter: Iter, parser: Parser) -> Result<Self, Error>
    where
        Iter: Iterator,
        Iter::Item: AsRef<str>,
    {
        iter.filter(|line| !line.as_ref().trim_start().starts_with('#'))
            .map(|line| parser.parse_line(line.as_ref()))
            .collect::<Result<_, Error>>()
            .map(|data| Self { data })
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
