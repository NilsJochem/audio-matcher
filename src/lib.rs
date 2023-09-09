#![warn(
    clippy::nursery,
    clippy::pedantic,
    clippy::empty_structs_with_brackets,
    clippy::format_push_string,
    clippy::if_then_some_else_none,
    // clippy::impl_trait_in_params,
    clippy::missing_assert_message,
    clippy::multiple_inherent_impl,
    clippy::non_ascii_literal,
    clippy::self_named_module_files,
    clippy::semicolon_inside_block,
    clippy::separated_literal_suffix,
    clippy::str_to_string,
    clippy::string_to_string
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::single_match_else
)]

pub mod archive;
pub mod args;
pub mod matcher;
pub mod worker;

mod extensions;
pub use extensions::{iter, option};

use std::{time::Duration, usize};

pub const APP_NAME: &str = "audio-matcher"; // on change remember to change value in audacity

const fn offset_range(range: &std::ops::Range<usize>, offset: usize) -> std::ops::Range<usize> {
    (range.start + offset)..(range.end + offset)
}

#[inline]
#[must_use]
pub const fn split_duration(duration: &Duration) -> (usize, usize, usize) {
    let elapsed = duration.as_secs() as usize;
    let seconds = elapsed % 60;
    let minutes = (elapsed / 60) % 60;
    let hours = elapsed / 3600;
    (hours, minutes, seconds)
}
// TODO exchange with:
// pub (const) fn split_duration(duration: &Duration) -> (u64, u64, u64) {
//    (duration.hours(), duration.minutes(), duration.seconds())
// }

pub mod io {
    use log::{debug, trace};
    use std::path::{Path, PathBuf};
    use thiserror::Error;

    use std::io::Error as IoError;
    use std::io::ErrorKind;

    #[derive(Debug, Error)]
    pub enum MoveError {
        #[error("file not found")]
        FileNotFound,
        #[error("target folder not found")]
        TargetNotFound,
        #[error(transparent)]
        OtherIO(IoError),
    }
    impl From<IoError> for MoveError {
        fn from(value: IoError) -> Self {
            match value.kind() {
                // some kinds are commented out because they are unstable
                ErrorKind::NotFound /*| ErrorKind::IsADirectory*/ => Self::FileNotFound,
                // ErrorKind::NotADirectory => Self::TargetNotFound,
                _ => Self::OtherIO(value),
            }
        }
    }

    pub(crate) async fn move_file(
        file: impl AsRef<Path> + Send + Sync,
        dst: impl AsRef<Path> + Send + Sync,
        dry_run: bool,
    ) -> Result<(), MoveError> {
        let dst = dst.as_ref();
        let file = file.as_ref();
        if !tokio::fs::try_exists(dst).await? && tokio::fs::metadata(dst).await?.is_dir() {
            return Err(MoveError::TargetNotFound);
        }
        if !tokio::fs::try_exists(file).await? && tokio::fs::metadata(dst).await?.is_file() {
            return Err(MoveError::FileNotFound);
        }
        if dry_run {
            println!("moving {file:?} to {dst:?}");
            return Ok(());
        }

        let mut dst = dst.to_path_buf();
        dst.push(file.file_name().unwrap());
        trace!("moving {file:?} to {dst:?}");
        match tokio::fs::rename(&file, &dst).await {
            Ok(()) => Ok(()),
            Err(_err) /*if err.kind() == IoErrorKind::CrossesDevices is unstable*/ => {
                debug!("couldn't just rename file, try to copy and remove old");
                tokio::fs::copy(&file, &dst).await?;
                tokio::fs::remove_file(&file).await?;
                Ok(())
            }
            // Err(err) => Err(err.into()),
        }
    }

    /// a Wrapper, that creates a copy of a file and removes it, when dropped
    pub struct TmpFile {
        path: PathBuf,
        is_removed: bool,
    }
    impl TmpFile {
        const fn new(path: PathBuf) -> Self {
            Self {
                path,
                is_removed: false,
            }
        }
        pub fn new_copy(path: PathBuf, orig: impl AsRef<Path>) -> Result<Self, IoError> {
            match std::fs::metadata(&path) {
                Ok(_) => Err(IoError::new(
                    ErrorKind::AlreadyExists,
                    format!("there is already a file at {path:?}"),
                )),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }?;
            std::fs::copy(orig, &path)?;
            Ok(Self::new(path))
        }
        pub fn new_empty(path: PathBuf) -> Result<Self, IoError> {
            let _ = std::fs::OpenOptions::new()
                .create_new(true)
                .read(false)
                .write(false)
                .open(&path)?;
            Ok(Self::new(path))
        }
        pub fn remove(&mut self) -> Result<(), IoError> {
            if !self.is_removed {
                std::fs::remove_file(&self.path)?;
                self.was_removed();
            }
            Ok(())
        }
        pub fn was_removed(&mut self) {
            self.is_removed = true;
        }
    }

    impl AsRef<std::path::Path> for TmpFile {
        fn as_ref(&self) -> &std::path::Path {
            &self.path
        }
    }
    impl Drop for TmpFile {
        fn drop(&mut self) {
            self.remove().unwrap();
        }
    }
}
