use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error(
        "Files have the different samplerates ({0}, {1}), and resampling isn't implementet jet"
    )]
    SampleRateMismatch(u16, u16),

    #[error("couldn't open file at path {0}")]
    NoFile(PathWrap),

    #[error("couldn't create file at path {0}")]
    CantCreateFile(PathWrap),

    #[error("no valid mp3 data in {0}")]
    NoMp3(PathWrap),
    // #[error("data store disconnected")]
    // Disconnect(#[from] io::Error),
    // #[error("invalid header (expected {expected:?}, found {found:?})")]
    // InvalidHeader {
    //     expected: String,
    //     found: String,
    // },
    #[error(transparent)]
    ID3(#[from] id3::Error),
}

// a wrapper for paths, that has display
pub struct PathWrap(Box<dyn AsRef<std::path::Path>>);

impl<A: AsRef<Path>> From<A> for PathWrap {
    fn from(value: A) -> Self {
        Self(Box::new(value.as_ref().to_path_buf()))
    }
}

impl core::fmt::Debug for PathWrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", &self.0.as_ref().as_ref())
    }
}
impl core::fmt::Display for PathWrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0.as_ref().as_ref().display())
    }
}
