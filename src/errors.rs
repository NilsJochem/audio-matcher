use itertools::Itertools;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct SampleRateMismatch(pub Box<[u16]>);

impl std::error::Error for SampleRateMismatch {}
impl Display for SampleRateMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Files have the different samplerates ({}), and resampling isn't implementet jet",
            self.0.iter().join(", ")
        )
    }
}

struct PathWrap(Box<dyn AsRef<std::path::Path>>);

impl core::fmt::Debug for PathWrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", &self.0.as_ref().as_ref())
    }
}

#[derive(Debug)]
pub struct FileError<'a> {
    path: PathBuf,
    msg: &'a str,
}
impl<'a> FileError<'a> {
    pub fn new<A: AsRef<Path>>(path: A, msg: &'a str) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            msg,
        }
    }
}
impl Display for FileError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} '{}'", self.msg, self.path.display())
    }
}
impl std::error::Error for FileError<'_> {}

pub struct NoFile;
impl NoFile {
    pub fn new<A: AsRef<Path>>(path: A) -> FileError<'static> {
        FileError::new(path, "couldn't open file at path")
    }
}
pub struct CantCreateFile;
impl CantCreateFile {
    pub fn new<A: AsRef<Path>>(path: A) -> FileError<'static> {
        FileError::new(path, "couldn't create file at path")
    }
}

pub struct NoMp3;
impl NoMp3 {
    pub fn new<A: AsRef<Path>>(path: A) -> FileError<'static> {
        FileError::new(path, "no valid mp3 data in")
    }
}
