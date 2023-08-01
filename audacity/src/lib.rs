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
    // clippy::missing_errors_doc,
    // clippy::missing_panics_doc
)]

use log::{debug, error, trace, warn};
use std::{
    collections::HashMap,
    marker::Send,
    path::{Path, PathBuf},
    time::Duration,
};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    time::{error::Elapsed, interval, timeout},
};

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

    /// writes `command` to audacity and waits for a result
    /// applys timeout if `self.timer` is Some
    /// this errors when either `self.write` or `self.read` errors, or the timeout occures
    async fn write(&mut self, command: &str) -> Result<String, Error> {
        let timer = self.timer;
        let future = async {
            self.just_write(command).await?;
            self.read(false).await
        };

        Self::maybe_timeout(timer, future).await?
    }
    async fn write_assume_empty(&mut self, command: &str) -> Result<(), Error> {
        let result = self.write(command).await?;
        assert_eq!(result, "", "expecting empty result");
        Ok(())
    }
    /// sends `msg` to audacity and waits for a result, which should always be Ok(msg).
    /// used for ping
    /// applys timeout if `self.timer` is Some
    ///
    /// # Errors
    /// this errors when either `self.write` or `self.read` errors, or the timeout occures
    async fn send_msg(&mut self, msg: &str, allow_empty_result: bool) -> Result<String, Error> {
        let timer = self.timer;
        let future = async {
            self.just_write(&format!("Message: Text=\"{msg}\"")).await?;
            self.read(allow_empty_result).await
        };

        Self::maybe_timeout(timer, future).await?
    }
    /// sends `command` to audacity, but doesn't wait for a result.
    ///
    /// # Errors
    /// this will Error with [`Error::PipeBroken`] when the `command` couldn't be written to audacity
    async fn just_write(&mut self, command: &str) -> Result<(), Error> {
        debug!("writing '{command}' to audacity");
        self.write_pipe
            .write_all(format!("{}{LINE_ENDING}", command.replace('\n', LINE_ENDING)).as_bytes())
            .await
            .map_err(|err| Error::PipeBroken(format!("failed to send {command:?}"), Some(err)))?;

        Ok(())
    }
    /// Reads the next answer from audacity.
    /// When not `allow_empty` reads lines until {[`Self::ACK_START`]}+\["OK"|"Failed!"\]+"\n\n" is reached and returns everything before.
    /// Else will also accept just "\n".
    ///
    /// # Errors
    ///  - [`Error::PipeBroken`] when the read pipe is closed or it reads ""
    ///  - [`Error::MissingOK`] or [`Error::MalformedResult`] when it didn't recieve OK\n
    ///  - [`Error::AudacityErr`] when it recieved an "Failed!", the error will contain the Error message
    ///
    /// # Panics
    /// This can panic, when after {[`Self::ACK_START`]} somthing unexpected appears
    async fn read(&mut self, allow_empty: bool) -> Result<String, Error> {
        let mut result = Vec::new();
        loop {
            let mut line = String::new();
            if !allow_empty {
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
            if !allow_empty {
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
                } else if allow_empty {
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
        let result = self.send_msg("ping", true).await?;

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
        let json = self.write("GetInfo: Type=Tracks Format=JSON").await?;
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
        let command = &format!("SelectTracks: Mode=Set Track={}", tracks.next().unwrap());
        let _result = self.write(command).await?;
        for track in tracks {
            let command = &format!("SelectTracks: Mode=Add Track={track}");
            self.write_assume_empty(command).await?;
        }
        Ok(())
    }
    /// Mutes the selected tracks.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn mute_selected_tracks(&mut self, mute: bool) -> Result<(), Error> {
        let command = if mute { "MuteTracks:" } else { "UnmuteTracks:" };
        self.write_assume_empty(command).await
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

        self.write_assume_empty(&format!("Import2: Filename=\"{}\"", path.display()))
            .await
    }
    /// Opens the dialoge for export multiple.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn export_multiple(&mut self) -> Result<(), Error> {
        self.write_assume_empty("ExportMultiple:").await
    }

    /// Gets Infos of the lables in the currently open Project.
    ///
    /// # Errors
    ///  - when write/send errors
    ///  - [`Error::MalformedResult`] when the result can't be parsed
    pub async fn get_label_info(
        &mut self,
    ) -> Result<HashMap<usize, Vec<(f64, f64, String)>>, Error> {
        let json = self.write("GetInfo: Type=Labels Format=JSON").await?;
        serde_json::from_str(&json)
            .map_err(|e| Error::MalformedResult(json, e.into()))
            .map(|list: Vec<(usize, Vec<_>)>| list.into_iter().collect())
    }
    /// Adds a new label track to the currently open Project.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn add_label_track(&mut self) -> Result<(), Error> {
        self.write_assume_empty("NewLabelTrack:").await
    }
    /// Opens the dialoge for import labels.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn import_labels(&mut self) -> Result<(), Error> {
        self.write_assume_empty("ImportLabels:").await
    }
    /// Opens the dialoge for export labels
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn export_labels(&mut self) -> Result<(), Error> {
        self.write_assume_empty("ExportLabels:").await
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
        text: Option<String>,
        start: Option<f64>,
        end: Option<f64>,
    ) -> Result<(), Error> {
        if text.is_none() && start.is_none() && end.is_none() {
            warn!("attempted to set_label with no values");
            return Ok(());
        }
        let mut command = format!("SetLabel: Label={i}");
        push_if_some(&mut command, "Text", text, true);
        push_if_some(&mut command, "Start", start, false);
        push_if_some(&mut command, "End", end, false);
        self.write_assume_empty(&command).await
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
        track_nr: usize,
        text: Option<String>,
        start: f64,
        end: f64,
    ) -> Result<usize, Error> {
        self.select_tracks(std::iter::once(track_nr)).await?;
        self.select_time(Some(start), Some(end), Some(RelativeTo::ProjectStart))
            .await?;
        self.write_assume_empty("AddLabel:").await?;
        let new_labels = self.get_label_info().await?.remove(&track_nr).unwrap();

        let mut new_candidates = new_labels.into_iter().enumerate().filter(|(_, (s, e, t))| {
            t.is_empty() && compare_times(*s, start) && compare_times(*e, end)
        });
        let new_id = new_candidates.next().expect("not enought labels").0;
        assert!(new_candidates.next().is_none(), "to many labels");
        //TODO check if new_id needs to be offset for non first tracks

        if let Some(text) = text.filter(|it| !it.is_empty()) {
            self.set_label(new_id, Some(text), None, None).await?;
        }
        Ok(new_id)
    }

    /// opens a new project.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn new_project(&mut self) -> Result<(), Error> {
        self.write_assume_empty("New:").await
    }
    /// closes the current project. May ask the User what to do with unsaved changes.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn close_projext(&mut self) -> Result<(), Error> {
        self.write_assume_empty("Close:").await
    }

    /// selects time from `start` to `end` in the selected track. If one is None keeps the current selection for it.
    ///
    /// Only logs a warning if all parameters are [`None`], buts returns [`Ok`]
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn select_time(
        &mut self,
        start: Option<f64>,
        end: Option<f64>,
        relative_to: Option<RelativeTo>,
    ) -> Result<(), Error> {
        if start.is_none() && end.is_none() {
            warn!("attempted to select_time with no values");
            return Ok(());
        }
        let mut command = "SelectTime:".to_owned();
        push_if_some(&mut command, "Start", start, false);
        push_if_some(&mut command, "End", end, false);
        push_if_some(&mut command, "RelativeTo", relative_to, false);
        self.write_assume_empty(&command).await
    }
    /// Removes the selected audio data and/or labels without copying these to the Audacity clipboard. Any audio or labels to right of the selection are shifted to the left.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn delete_selection(&mut self) -> Result<(), Error> {
        self.write_assume_empty("Delete:").await
    }
    /// Removes the selected audio data and/or labels without copying these to the Audacity clipboard. None of the audio data or labels to right of the selection are shifted.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn split_delete_selection(&mut self) -> Result<(), Error> {
        self.write_assume_empty("SplitDelete:").await
    }
    /// Creates a new track containing only the current selection as a new clip.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn duplicate_selection(&mut self) -> Result<(), Error> {
        self.write_assume_empty("Duplicate:").await
    }

    /// Does a Split Cut on the current selection in the current track, then creates a new track and pastes the selection into the new track.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn split_new(&mut self) -> Result<(), Error> {
        self.write_assume_empty("SplitNew:").await
    }
    // pub async fn edit_metadate(&mut self) -> Result<(), Error> {
    //     let _result = self.write("EditMetaData:").await?;
    //     Ok(())
    // }

    /// Zooms in or out so that the selected audio fills the width of the window.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn zoom_to_selection(&mut self) -> Result<(), Error> {
        self.write_assume_empty("ZoomSel:").await
    }
    /// Zooms to the default view which displays about one inch per second.
    ///
    /// # Errors
    ///  - when write/send errors
    pub async fn zoom_normal(&mut self) -> Result<(), Error> {
        self.write_assume_empty("ZoomNormal:").await
    }
}

fn push_if_some<T: ToString>(s: &mut String, cmd: &str, param: Option<T>, escape: bool) {
    if let Some(value) = param {
        let value = value.to_string();
        s.reserve(4 + cmd.len() + value.len());
        s.push(' ');
        s.push_str(cmd);
        s.push('=');
        if escape {
            s.push('"');
        }
        s.push_str(&value);
        if escape {
            s.push('"');
        }
    }
}

#[inline]
fn compare_times(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-5
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
        name: String,
        focused: bool,
        selected: bool,
        kind: Kind,
    }
    #[derive(Debug)]
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
            ExpectAction::Write("Message: Text=\"ping\"\n"), // ping with empty result
            ExpectAction::ReadEmpty,
            ExpectAction::Write("Message: Text=\"ping\"\n"), // until one ping succeeds
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
                ExpectAction::Write("Message: Text=\"ping\"\n"),
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
