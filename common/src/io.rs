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

pub async fn move_file(
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
        match std::fs::metadata(&path) {
            Ok(_) => Err(IoError::new(
                ErrorKind::AlreadyExists,
                format!("there is already a file at {path:?}"),
            )),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }?;
        let _ = std::fs::File::create(&path)?;
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
