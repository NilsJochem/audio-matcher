#![warn(
    clippy::nursery,
    clippy::pedantic,
    clippy::empty_structs_with_brackets,
    clippy::format_push_string,
    clippy::if_then_some_else_none,
    clippy::impl_trait_in_params,
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
    clippy::single_match_else
    // clippy::missing_errors_doc,
    // clippy::missing_panics_doc
)]

use data::TimeLabel;
use itertools::Itertools;
use log::{debug, error, trace, warn};
use std::{
    collections::HashMap,
    fmt::Debug,
    marker::Send,
    path::{Path, PathBuf},
    time::Duration,
};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    time::{error::Elapsed, interval, timeout},
};

pub mod command;

#[cfg(windows)]
const LINE_ENDING: &str = "\r\n";
#[cfg(not(windows))]
const LINE_ENDING: &str = "\n";

#[link(name = "c")]
#[cfg(any(target_os = "linux", target_os = "macos"))]
extern "C" {
    fn geteuid() -> u32;
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    PipeBroken(String, #[source] Option<tokio::io::Error>),
    #[error("Didn't finish with OK or Failed!, {0:?}")]
    MissingOK(String),
    #[error("Failed with {0:?}")]
    AudacityErr(String), // TODO parse Error
    #[error("couldn't parse result {0:?} because {1}")]
    MalformedResult(String, #[source] MalformedCause),
    #[error("Unkown path {0:?}, {1}")]
    PathErr(PathBuf, #[source] std::io::Error),
    #[error("timeout after {0:?}")]
    Timeout(Duration),
}
impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::MissingOK(l0), Self::MissingOK(r0))
            | (Self::AudacityErr(l0), Self::AudacityErr(r0)) => l0 == r0,
            (Self::Timeout(l0), Self::Timeout(r0)) => l0 == r0,
            (Self::MalformedResult(l0, l1), Self::MalformedResult(r0, r1)) => l0 == r0 && l1 == r1,
            _ => false,
        }
    }
}

#[derive(Error, Debug)]
pub enum MalformedCause {
    #[error(transparent)]
    JSON(#[from] serde_json::Error),
    #[error(transparent)]
    Own(#[from] result::Error),
    #[error("ping returned {0:?}")]
    BadPingResult(String),
    #[error("missing line break")]
    MissingLineBreak,
}
impl PartialEq for MalformedCause {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Own(l0), Self::Own(r0)) => l0 == r0,
            (Self::BadPingResult(l0), Self::BadPingResult(r0)) => l0 == r0,
            (Self::MissingLineBreak, Self::MissingLineBreak) => true,
            _ => false,
        }
    }
}

pub mod data;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativeTo {
    ProjectStart,
    Project,
    ProjectEnd,
    SelectionStart,
    Selection,
    SelectionEnd,
}
impl std::fmt::Display for RelativeTo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Error)]
pub enum LaunchError {
    #[error(transparent)]
    IO(#[from] tokio::io::Error),
    #[error("failed with status code {0}")]
    Failed(i32),
    #[error("process was terminated")]
    Terminated,
}

impl LaunchError {
    const fn from_status_code(value: Option<i32>) -> Result<(), Self> {
        match value {
            Some(0) => Ok(()),
            Some(code) => Err(Self::Failed(code)),
            None => Err(Self::Terminated),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Config {
    launcher: String,
    audacity_app_name: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            launcher: "gtk4-launch".to_owned(),
            audacity_app_name: "audacity".to_owned(),
        }
    }
}

#[derive(Debug)]
#[must_use]
pub struct AudacityApiGeneric<W: AsyncWrite, R: AsyncRead> {
    write_pipe: W,
    read_pipe: BufReader<R>,
    timer: Option<Duration>,
}

///exposes an os specific version
#[cfg(windows)]
pub type AudacityApi = AudacityApiGeneric<
    tokio::net::windows::named_pipe::NamedPipeClient,
    tokio::net::windows::named_pipe::NamedPipeClient,
>;
#[cfg(windows)]
impl AudacityApi {
    pub async fn launch_audacity() -> Result<(), LaunchError> {
        todo!("stub");
    }
    pub const fn new(timer: Option<Duration>) -> Self {
        todo!("stub");
        use tokio::net::windows::named_pipe::ClientOptions;
        let options = ClientOptions::new();
        let mut poll_rate = interval(Duration::from_millis(100));

        Self::with_pipes(
            options.open(r"\\.\pipe\ToSrvPipe"),
            options.open(r"\\.\pipe\FromSrvPipe"),
            timer,
            poll_rate,
        );
    }
}

///exposes an os specific version
#[cfg(unix)]
pub type AudacityApi =
    AudacityApiGeneric<tokio::net::unix::pipe::Sender, tokio::net::unix::pipe::Receiver>;
#[cfg(unix)]
impl AudacityApi {
    const BASE_PATH: &str = "/tmp/audacity_script_pipe";

    /// Launches Audacity.
    ///
    /// # Panics
    /// can panic, when loading of config fails.
    ///
    /// # Errors
    /// - [`LaunchError::IO`] when executing the commant failed
    /// - [`LaunchError::Failed`] when the launcher exited with an statuscode != 0
    /// - [`LaunchError::Terminated`] when the launcher was terminated by a signal
    pub async fn launch_audacity() -> Result<(), LaunchError> {
        let config = confy::load::<Config>("audio-matcher", "audacity").unwrap();
        LaunchError::from_status_code(
            tokio::process::Command::new(config.launcher)
                .arg(config.audacity_app_name)
                .output()
                .await?
                .status
                .code(),
        )
    }

    /// creates a new Instance of `AudacityApi` for linux.
    ///
    /// Will wait for `timer` until the pipe is ready, and saves the timer in `self`.
    /// Will also wait for ping to answer.
    ///
    /// # Errors
    /// - when a Timeout occures
    /// - when the other Pipe isn't ready after waiting for the first pipe
    /// - when Ping returns false
    pub async fn new(timer: Option<Duration>) -> Result<Self, Error> {
        use tokio::net::unix::pipe::OpenOptions;

        let uid = unsafe { geteuid() };
        let options = OpenOptions::new();
        let mut poll_rate = interval(Duration::from_millis(100));
        let writer_path = format!("{}.to.{uid}", Self::BASE_PATH);
        let future = async {
            loop {
                poll_rate.tick().await;
                match options.open_sender(&writer_path) {
                    Ok(writer) => break writer,
                    Err(err) => {
                        debug!("{}", Error::PipeBroken("open writer".to_owned(), Some(err)));
                    }
                }
                trace!("waiting for audacity to start");
            }
        };
        let writer = Self::maybe_timeout(timer, future).await?;
        let reader = options
            .open_receiver(format!("{}.from.{uid}", Self::BASE_PATH))
            .map_err(|err| Error::PipeBroken("open reader".to_owned(), Some(err)))?;
        debug!("pipes found");
        Self::with_pipes(reader, writer, timer, poll_rate).await
    }
}

impl<W: AsyncWrite + Send + Unpin, R: AsyncRead + Send + Unpin> AudacityApiGeneric<W, R> {
    const ACK_START: &str = "BatchCommand finished: ";
    pub(crate) async fn with_pipes(
        reader: R,
        writer: W,
        timer: Option<Duration>,
        mut poll_rate: tokio::time::Interval,
    ) -> Result<Self, Error> {
        let mut audacity_api = Self {
            write_pipe: writer,
            read_pipe: BufReader::new(reader),
            timer,
        };
        poll_rate.reset();
        // waiting for audacity to be ready
        while !audacity_api.ping().await? {
            poll_rate.tick().await;
        }
        Ok(audacity_api)
    }

    /// writes `command` directly to audacity, waits for a result but asserts it is empty.
    ///
    /// for commands with output use its dedicated Method. Also prefer a dedicated method it one is available
    ///
    /// # Errors
    /// when either `self.write` or `self.read` errors, or the timeout occures
    ///
    /// # Panics
    /// when a non empty result is recieved
    pub async fn write_assume_empty<'a>(
        &mut self,
        command: command::NoOut<'a>,
    ) -> Result<(), Error> {
        let result = self.write_any(command.clone(), false).await?;
        assert_eq!(result, "", "expecting empty result for {command:?}");
        Ok(())
    }
    async fn write_assume_result<'a>(
        &mut self,
        command: command::Out<'a>,
    ) -> Result<String, Error> {
        self.write_any(command, false).await
    }
    /// writes `command` to audacity and waits for a result.
    ///
    /// applys timeout if `self.timer` is Some.
    ///
    /// forwarts `allow_no_ok` to read. This is only intendet to ping until ready
    ///
    /// one should use `write_assume_empty` or `write_assume_result`
    /// this errors when either `self.write` or `self.read` errors, or the timeout occures
    async fn write_any<'a>(
        &mut self,
        command: impl command::Command + Debug + Send + Sync,
        allow_no_ok: bool,
    ) -> Result<String, Error> {
        let timer = self.timer;
        let future = async {
            let command_str = command.to_string().replace('\n', LINE_ENDING);
            debug!("writing {command_str:?} to audacity");
            self.write_pipe
                .write_all(format!("{command_str}{LINE_ENDING}").as_bytes())
                .await
                .map_err(|err| {
                    Error::PipeBroken(format!("failed to send {command:?}"), Some(err))
                })?;

            self.read(allow_no_ok).await
        };

        Self::maybe_timeout(timer, future).await?
    }

    /// Reads the next answer from audacity.
    /// When not `allow_no_ok` reads lines until {[`Self::ACK_START`]}+\["OK"|"Failed!"\]+"\n\n" is reached and returns everything before.
    /// Else will also accept just "\n".
    ///
    /// # Errors
    ///  - [`Error::PipeBroken`] when the read pipe is closed or it reads ""
    ///  - [`Error::MissingOK`] or [`Error::MalformedResult`] when it didn't recieve OK\n
    ///  - [`Error::AudacityErr`] when it recieved an "Failed!", the error will contain the Error message
    ///
    /// # Panics
    /// This can panic, when after {[`Self::ACK_START`]} somthing unexpected appears
    async fn read(&mut self, allow_no_ok: bool) -> Result<String, Error> {
        let mut result = Vec::new();
        loop {
            let mut line = String::new();
            if !allow_no_ok {
                trace!("reading next line from audacity");
            }
            self.read_pipe.read_line(&mut line).await.map_err(|err| {
                Error::PipeBroken(
                    format!(
                        "failed to read next line, current buffer: {:?}",
                        result.join("\n")
                    ),
                    Some(err),
                )
            })?;
            if !allow_no_ok {
                trace!("read line {line:?} from audacity");
            }

            if line.is_empty() {
                error!("current result: {result:?}");
                return Err(Error::PipeBroken("empty reader".to_owned(), None));
            }

            // remove line ending
            let line = &line[..(line.len() - 1)];
            let line = line.strip_suffix('\r').unwrap_or(line);

            if line.is_empty() {
                return if !result.is_empty() {
                    Err(Error::MissingOK(result.join("\n")))
                } else if allow_no_ok {
                    Ok(String::new())
                } else {
                    trace!("skipping empty line");
                    continue;
                };
            }
            // let line = &line[..(line.len() - LINE_ENDING.len())];

            if let Some(rest) = line.strip_prefix(Self::ACK_START) {
                let mut tmp = String::new();
                self.read_pipe.read_line(&mut tmp).await.map_err(|_err| {
                    Error::PipeBroken(
                        format!(
                            "failed to read newline after ok. Current buffer: {:?}",
                            result.join("\n")
                        ),
                        None,
                    )
                })?;
                let result = result.join("\n");
                if tmp != "\n" && tmp != "\r\n" {
                    return Err(Error::MalformedResult(
                        result,
                        MalformedCause::MissingLineBreak,
                    ));
                }
                return match rest {
                    "OK" => {
                        debug!("read '{result}' from audacity");
                        Ok(result)
                    }
                    "Failed!" => Err(Error::AudacityErr(result)),
                    x => panic!("need error handling for {x}"),
                };
            }
            result.push(line.to_owned());
        }
    }

    /// formats the error of [`maybe_timeout`] to [`Error::Timeout`]
    async fn maybe_timeout<T, F: std::future::Future<Output = T> + Send>(
        timer: Option<Duration>,
        future: F,
    ) -> Result<T, Error> {
        maybe_timeout(timer, future)
            .await
            .map_err(|_err| Error::Timeout(timer.unwrap()))
    }
    /// Pings Audacity and returns if the result is correct.
    ///
    /// # Errors
    ///  - when write/send errors
    ///  - [`Error::MalformedResult`] when something other then ping is answered
    pub async fn ping(&mut self) -> Result<bool, Error> {
        let result = self
            .write_any(command::Message { text: "ping" }, true)
            .await?;

        if result.is_empty() {
            Ok(false)
        } else if result == "ping" {
            Ok(true)
        } else {
            Err(Error::MalformedResult(
                result.clone(),
                MalformedCause::BadPingResult(result),
            ))
        }
    }

    /// Gets Infos of the Tracks in the currently open Project.
    ///
    /// # Errors
    ///  - when write/send errors
    ///  - [`Error::MalformedResult`] when the result can't be parsed
    pub async fn get_track_info(&mut self) -> Result<Vec<result::TrackInfo>, Error> {
        let json = self
            .write_assume_result(command::GetInfo {
                type_info: command::InfoType::Tracks,
                format: command::OutputFormat::Json,
            })
            .await?;
        serde_json::from_str::<Vec<result::RawTrackInfo>>(&json)
            .map_err(|e| Error::MalformedResult(json.clone(), e.into()))?
            .into_iter()
            .map(|it| {
                it.try_into()
                    .map_err(|e: result::Error| Error::MalformedResult(json.clone(), e.into()))
            })
            .collect()
    }
    /// Selects the tracks with position `tracks`.
    ///
    /// # Errors
    ///  - when write/send errors
    ///  - [`Error::AudacityErr`] when any of `tracks` is invalid
    ///
    /// # Panics
    ///  - when `tracks` is empty
    pub async fn select_tracks(
        &mut self,
        mut tracks: impl Iterator<Item = usize> + Send,
    ) -> Result<(), Error> {
        self.write_assume_empty(command::SelectTracks {
            mode: command::SelectMode::Set,
            track: tracks.next().unwrap(),
            track_count: Some(1),
        })
        .await?;
        for track in tracks {
            self.write_assume_empty(command::SelectTracks {
                mode: command::SelectMode::Add,
                track,
                track_count: Some(1),
            })
            .await?;
        }
        Ok(())
    }
    //TODO align tracks

    /// imports the audio file at `path` into a new track.
    ///
    /// # Errors
    ///  - when write/send errors
    ///  - [`Error::AudacityErr`] when path is not a valid audio file (probably)
    pub async fn import_audio<P: AsRef<Path> + Send>(&mut self, path: P) -> Result<(), Error> {
        let path = path
            .as_ref()
            .canonicalize()
            .map_err(|e| Error::PathErr(path.as_ref().to_path_buf(), e))?;

        self.write_assume_empty(command::Import2 { filename: &path })
            .await
    }

    /// Gets Infos of the lables in the currently open Project.
    ///
    /// # Errors
    ///  - when write/send errors
    ///  - [`Error::MalformedResult`] when the result can't be parsed
    pub async fn get_label_info(&mut self) -> Result<HashMap<usize, Vec<TimeLabel>>, Error> {
        type RawTimeLabel = (f64, f64, String);
        let json = self
            .write_assume_result(command::GetInfo {
                type_info: command::InfoType::Labels,
                format: command::OutputFormat::Json,
            })
            .await?;
        serde_json::from_str(&json)
            .map_err(|e| Error::MalformedResult(json, e.into()))
            .map(|list: Vec<(usize, Vec<RawTimeLabel>)>| {
                list.into_iter()
                    .map(|(nr, labels)| (nr, labels.into_iter().map_into().collect_vec()))
                    .collect()
            })
    }
    /// Adds a new label track to the currently open Project.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn add_label_track(
        &mut self,
        name: Option<impl AsRef<str> + Send>,
    ) -> Result<usize, Error> {
        self.write_assume_empty(command::NewLabelTrack).await?;
        if let Some(name) = name {
            let name = Some(name.as_ref());
            self.write_assume_empty(command::SetTrackStatus {
                name,
                selected: None,
                focused: None,
            })
            .await?;
        }

        Ok(self.get_track_info().await?.len() - 1)
    }

    /// imports labels from the file at `path`
    ///
    /// # Errors
    ///  - when write/send errors
    ///  - [`Error::PathErr`] when the file at `path` can't be read
    pub async fn import_labels_from(
        &mut self,
        path: impl AsRef<Path> + Send + Sync,
        track_name: Option<impl AsRef<str> + Send>,
    ) -> Result<(), Error> {
        let nr = self.add_label_track(track_name).await?;
        for label in TimeLabel::read(&path)
            .map_err(|err| Error::PathErr(path.as_ref().to_path_buf(), err))?
        {
            let _ = self.add_label(label, Some(nr)).await?;
        }
        Ok(())
    }

    /// Export all labels to the file at `path`.
    ///
    /// Uses the format of audacitys marks file, with all tracks concatinated,
    ///
    /// # Errors
    ///  - when write/send errors
    ///  - [`Error::PathErr`] when the file at `path` can't be written to
    pub async fn export_all_labels_to(
        &mut self,
        path: impl AsRef<Path> + Send,
        dry_run: bool,
    ) -> Result<(), Error> {
        TimeLabel::write(
            self.get_label_info().await?.into_values().flatten(),
            &path,
            dry_run,
        )
        .map_err(|err| Error::PathErr(path.as_ref().to_path_buf(), err))?;
        Ok(())
    }
    /// Sets the `text`, `start`, `end` of the label at position `i`.
    ///
    /// When the project has multiple label tracks the position seems to be offset by all labels in tracks before.
    ///
    /// Only logs a warning if all parameters are [`None`], buts returns [`Ok`]
    ///
    /// # Errors
    ///  - when write/send errors
    ///  - [`Error::AudacityErr`] when `i` is not a valid track position
    pub async fn set_label(
        &mut self,
        i: usize,
        text: Option<impl AsRef<str> + Send>,
        start: Option<Duration>,
        end: Option<Duration>,
        selected: Option<bool>,
    ) -> Result<(), Error> {
        if text.is_none() && start.is_none() && end.is_none() && selected.is_none() {
            warn!("attempted to set_label with no values");
            return Ok(());
        }

        let text = text.as_ref().map(std::convert::AsRef::as_ref);
        self.write_assume_empty(command::SetLabel {
            label: i,
            text,
            start: start.map(|it| it.as_secs_f64()),
            end: end.map(|it| it.as_secs_f64()),
            selected,
        })
        .await
    }

    #[allow(
        unreachable_code,
        unused_variables,
        clippy::missing_errors_doc,
        clippy::missing_panics_doc
    )]
    pub async fn add_label_to(
        &mut self,
        track_nr: usize,
        label: TimeLabel,
    ) -> Result<usize, Error> {
        unimplemented!("fix select track");
        self.select_tracks(std::iter::once(track_nr)).await?;
        self.write_assume_empty(command::SetTrackStatus {
            name: None,
            selected: None,
            focused: Some(true),
        })
        .await?;
        self.add_label(label, Some(track_nr)).await
    }
    /// Creates a new label on track `track_nr` from `start` to `end` with Some(text).
    ///
    /// Sets the current selection to the given values and then adds a new blank Label. If text is not empty updates the label to `text`
    /// returns the postition of the label in this track
    ///
    /// # Panics
    /// - when the new label can't be located after creation
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn add_label(
        &mut self,
        label: TimeLabel,
        track_hint: Option<usize>,
    ) -> Result<usize, Error> {
        self.select_time(
            Some(label.start),
            Some(label.end),
            Some(RelativeTo::ProjectStart),
        )
        .await?;
        self.write_assume_empty(command::AddLabel).await?;

        let track_hint = match track_hint {
            Some(v) => v,
            None => self.get_focused_track().await?,
        };

        let labels = self.get_label_info().await?;
        let id_offset: usize = labels
            .iter()
            .filter(|(t_nr, _)| t_nr < &&track_hint)
            .map(|(_, l)| l.len())
            .sum();

        let new_labels = labels.get(&track_hint).unwrap();
        let new_id = id_offset
            + new_labels
                .iter()
                .enumerate()
                .find(|(_, candidate)| {
                    candidate.name.is_none()
                        && is_near_to(candidate.start, label.start, Duration::from_millis(10))
                        && is_near_to(candidate.end, label.end, Duration::from_millis(10))
                })
                .unwrap_or_else(|| panic!("not enought labels in track {track_hint}"))
                .0;

        self.set_label(
            new_id,
            label.name,
            None,
            None,
            Some(false), // always drop seelected state
        )
        .await?;

        Ok(new_id)
    }

    async fn get_focused_track(&mut self) -> Result<usize, Error> {
        Ok(self
            .get_track_info()
            .await?
            .into_iter()
            .enumerate()
            .filter(|(_, t)| t.focused)
            .exactly_one()
            .expect("no track focused")
            .0)
    }

    /// selects time from `start` to `end` in the selected track. If one is None keeps the current selection for it.
    ///
    /// Only logs a warning if all parameters are [`None`], buts returns [`Ok`]
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn select_time(
        &mut self,
        start: Option<Duration>,
        end: Option<Duration>,
        relative_to: Option<RelativeTo>,
    ) -> Result<(), Error> {
        if start.is_none() && end.is_none() {
            warn!("attempted to select_time with no values");
            return Ok(());
        }
        self.write_assume_empty(command::SelectTime {
            start: start.map(|it| it.as_secs_f64()),
            end: end.map(|it| it.as_secs_f64()),
            reative_to: relative_to,
        })
        .await
    }
}

#[inline]
fn is_near_to(a: Duration, b: Duration, delta: Duration) -> bool {
    (if a >= b { a - b } else { b - a }) < delta
}

async fn maybe_timeout<T, F: std::future::Future<Output = T> + Send>(
    timer: Option<Duration>,
    future: F,
) -> Result<T, Elapsed> {
    match timer {
        Some(timer) => timeout(timer, future).await,
        None => Ok(future.await),
    }
}

pub mod result {
    use serde::{Deserialize, Serialize};
    use thiserror::Error;

    #[derive(Debug, Error, PartialEq, Eq)]
    pub enum Error {
        #[error("Missing field {0}")]
        MissingField(&'static str),
        #[error("Unkown Kind at {0}")]
        UnkownKind(String),
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub(super) struct RawTrackInfo {
        name: String,
        focused: u8,
        selected: u8,
        kind: String,
        start: Option<f64>,
        end: Option<f64>,
        pan: Option<usize>,
        gain: Option<f64>,
        channels: Option<usize>,
        solo: Option<u8>,
        mute: Option<u8>,
    }
    impl TryFrom<RawTrackInfo> for TrackInfo {
        type Error = Error;

        fn try_from(value: RawTrackInfo) -> Result<Self, Self::Error> {
            Ok(Self {
                name: value.name,
                focused: value.focused == 1,
                selected: value.selected == 1,
                kind: match value.kind.as_str() {
                    "wave" => Kind::Wave {
                        start: value.start.ok_or(Error::MissingField("wave.start"))?,
                        end: value.end.ok_or(Error::MissingField("wave.end"))?,
                        pan: value.pan.ok_or(Error::MissingField("wave.pan"))?,
                        gain: value.gain.ok_or(Error::MissingField("wave.gain"))?,
                        channels: value.channels.ok_or(Error::MissingField("wave.channels"))?,
                        solo: value.solo.ok_or(Error::MissingField("wave.solo"))? == 1,
                        mute: value.mute.ok_or(Error::MissingField("wave.mute"))? == 1,
                    },
                    "label" => Kind::Label,
                    "time" => Kind::Time,
                    _ => return Err(Error::UnkownKind(value.kind)),
                },
            })
        }
    }
    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct TrackInfo {
        pub name: String,
        pub focused: bool,
        pub selected: bool,
        pub kind: Kind,
    }
    impl PartialEq for TrackInfo {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name && self.kind == other.kind
        }
    }

    #[derive(Debug, PartialEq)]
    #[allow(dead_code)]
    pub enum Kind {
        Wave {
            start: f64,
            end: f64,
            pan: usize,
            gain: f64,
            channels: usize,
            solo: bool,
            mute: bool,
        },
        Label,
        Time,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tokio::io::{sink, ReadHalf, Sink, WriteHalf};
    use tokio_test::io::{Builder, Mock};

    #[allow(dead_code)]
    enum ReadMsg<'a> {
        Ok(&'a str),
        Fail(&'a str),
        Empty,
    }
    impl<'a> ReadMsg<'a> {
        fn to_string(&self, line_ending: &str) -> String {
            match self {
                ReadMsg::Empty => line_ending.to_owned(),
                ReadMsg::Fail(msg) => format!(
                    "{msg}\n{}Failed!\n\n",
                    AudacityApiGeneric::<Mock, Mock>::ACK_START
                )
                .replace("\n", line_ending),
                ReadMsg::Ok(msg) => format!(
                    "{msg}\n{}OK\n\n",
                    AudacityApiGeneric::<Mock, Mock>::ACK_START
                )
                .replace("\n", line_ending),
            }
        }
    }
    enum ExpectAction<'a> {
        Read(ReadMsg<'a>),
        Write(&'a str),
    }
    impl<'a> ExpectAction<'a> {
        #[allow(non_upper_case_globals)]
        const ReadEmpty: Self = Self::Read(ReadMsg::Empty);
        #[allow(non_snake_case)]
        fn ReadOk(msg: &'a str) -> Self {
            Self::Read(ReadMsg::Ok(msg))
        }
        #[allow(non_snake_case)]
        fn ReadFail(msg: &'a str) -> Self {
            Self::Read(ReadMsg::Fail(msg))
        }
    }

    async fn new_mocked_api(
        actions: impl Iterator<Item = ExpectAction<'_>>,
        windows_line_ending: bool,
    ) -> AudacityApiGeneric<WriteHalf<Mock>, ReadHalf<Mock>> {
        let line_ending = if windows_line_ending { "\r\n" } else { "\n" };
        let mut builder = Builder::new();
        let iter = [
            ExpectAction::Write("Message: Text=ping\n"), // ping with empty result
            ExpectAction::ReadEmpty,
            ExpectAction::Write("Message: Text=ping\n"), // until one ping succeeds
            ExpectAction::ReadOk("ping"),
        ]
        .into_iter()
        .chain(actions);
        for action in iter {
            match action {
                ExpectAction::Read(msg) => builder.read(msg.to_string(line_ending).as_bytes()),
                ExpectAction::Write(msg) => {
                    builder.write(msg.replace("\n", LINE_ENDING).as_bytes())
                }
            };
        }
        let (read_mock, write_mock) = tokio::io::split(builder.build());

        timeout(
            Duration::from_secs(1),
            AudacityApiGeneric::with_pipes(
                read_mock,
                write_mock,
                None,
                interval(Duration::from_millis(100)),
            ),
        )
        .await
        .expect("timed out")
        .expect("failed to setup")
    }

    struct ReadHandle {
        handle: tokio_test::io::Handle,
    }
    #[allow(dead_code)]
    impl ReadHandle {
        fn expect(&mut self, msg: ReadMsg) {
            self.handle.read(msg.to_string("\n").as_bytes());
        }
        fn expect_ok(&mut self, msg: &str) {
            self.expect(ReadMsg::Ok(msg));
        }
        fn expect_fail(&mut self, msg: &str) {
            self.expect(ReadMsg::Fail(msg));
        }
    }

    async fn ignore_write_api() -> (AudacityApiGeneric<Sink, Mock>, ReadHandle) {
        let (mock, handle) = Builder::new().build_with_handle();
        let mut handle = ReadHandle { handle };
        handle.expect_ok("ping");
        (
            timeout(
                Duration::from_secs(1),
                AudacityApiGeneric::with_pipes(
                    mock,
                    sink(),
                    None,
                    interval(Duration::from_millis(100)),
                ),
            )
            .await
            .expect("timed out")
            .expect("failed to setup"),
            handle,
        )
    }

    #[tokio::test]
    async fn extra_ping() {
        let mut api = new_mocked_api(
            [
                ExpectAction::Write("Message: Text=ping\n"),
                ExpectAction::ReadOk("ping"),
            ]
            .into_iter(),
            false,
        )
        .await;

        api.ping().await.unwrap();
    }
    #[tokio::test]
    async fn ping_ignore_write() {
        let (mut api, mut handle) = ignore_write_api().await;
        handle.expect_ok("ping");
        api.ping().await.unwrap();
    }

    #[tokio::test]
    async fn read_mulitline_ok() {
        let msg = "some multiline\n Message".to_owned();
        let mut api = new_mocked_api([ExpectAction::ReadOk(&msg)].into_iter(), false).await;
        assert_eq!(Ok(msg), api.read(false).await);
    }
    #[tokio::test]
    async fn read_mulitline_failed() {
        let msg = "some multiline\n Message".to_owned();
        let mut api = new_mocked_api([ExpectAction::ReadFail(&msg)].into_iter(), false).await;

        assert_eq!(Err(Error::AudacityErr(msg)), api.read(false).await);
    }
    #[tokio::test]
    async fn read_mulitline_ok_windows_line_ending() {
        let msg = "some multiline\n Message".to_owned();
        let mut api = new_mocked_api([ExpectAction::ReadOk(&msg)].into_iter(), true).await;
        assert_eq!(Ok(msg), api.read(false).await);
    }
    #[tokio::test]
    async fn read_mulitline_failed_windows_line_ending() {
        let msg = "some multiline\n Message".to_owned();
        let mut api = new_mocked_api([ExpectAction::ReadFail(&msg)].into_iter(), true).await;

        assert_eq!(Err(Error::AudacityErr(msg)), api.read(false).await);
    }
}
