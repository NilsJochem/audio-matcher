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
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]
pub mod scripting_interface {
    use log::{debug, error, trace};
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };
    use thiserror::Error;
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
        net::unix::pipe::{Receiver, Sender},
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
        #[error("Pipe Broken at {0}")]
        PipeBroken(String),
        #[error("Didn't finish with OK or Failed!, {partial:?}")]
        MissingOK { partial: String },
        #[error("Failed with {0:?}")]
        AudacityErr(String), // TODO parse Error
        #[error("couldn't parse result {0:?} because {1}")]
        MalformedResult(String, MalformedCause),
        #[error("Unkown path {0:?}, {1}")]
        PathErr(PathBuf, std::io::Error),
    }
    #[derive(Error, Debug)]
    pub enum MalformedCause {
        #[error("{0}")]
        JSON(serde_json::Error),
        #[error("{0}")]
        Own(result::Error),
        #[error("was empty")]
        Empty,
    }

    #[derive(Debug)]
    #[must_use]
    pub struct AudacityApi {
        write_pipe: BufWriter<Sender>,
        read_pipe: BufReader<Receiver>,
        timer: Option<Duration>,
    }

    impl AudacityApi {
        #[cfg(target_os = "windows")]
        pub const fn new(timer: Option<Duration>) -> Self {
            Self {
                to_name: PathBuf::from("\\\\.\\pipe\\ToSrvPipe"),
                from_name: PathBuf::from("\\\\.\\pipe\\FromSrvPipe"),
                timer,
            }
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        pub async fn new(timer: Option<Duration>) -> Result<Self, Error> {
            let uid = unsafe { geteuid() };
            let base_path = "/tmp/audacity_script_pipe";
            let options = tokio::net::unix::pipe::OpenOptions::new();
            let mut poll_rate = tokio::time::interval(Duration::from_millis(100));
            let writer = loop {
                poll_rate.tick().await;
                match options.open_sender(format!("{base_path}.to.{uid}")) {
                    Ok(writer) => break writer,
                    Err(_err) => {} // Error::PipeBroken(format!("open writer with {err:?}",))
                }
                trace!("waiting for audacity to start");
            };
            let reader = options
                .open_receiver(format!("{base_path}.from.{uid}"))
                .map_err(|e| Error::PipeBroken(format!("open reader with {e:?}")))?;

            Self::with_pipes(reader, writer, timer, poll_rate).await
        }

        async fn with_pipes(
            reader: Receiver,
            writer: Sender,
            timer: Option<Duration>,
            mut poll_rate: tokio::time::Interval,
        ) -> Result<Self, Error> {
            let mut audacity_api = Self {
                write_pipe: BufWriter::new(writer),
                read_pipe: BufReader::new(reader),
                timer,
            };
            poll_rate.reset();
            poll_rate.tick().await;
            // waiting for audacity to be ready
            while !audacity_api.ping().await? {
                poll_rate.tick().await;
            }
            Ok(audacity_api)
        }

        async fn write(&mut self, command: &str) -> Result<String, Error> {
            if let Some(_timer) = self.timer {
                todo!("add timeout");
            }
            self.just_write(command).await;
            self.read(false).await
        }

        async fn just_write(&mut self, command: &str) {
            debug!("writing '{command}' to audacity");
            self.write_pipe.write_all(command.as_bytes()).await.unwrap();
            self.write_pipe
                .write_all(LINE_ENDING.as_bytes())
                .await
                .unwrap();
            self.write_pipe.flush().await.unwrap();
        }

        async fn read(&mut self, allow_empty: bool) -> Result<String, Error> {
            let mut result = String::new();
            loop {
                let mut line = String::new();
                if !allow_empty {
                    trace!("reading next line from audacity");
                }
                self.read_pipe.read_line(&mut line).await.unwrap();
                if !allow_empty {
                    trace!("read line {line:?} from audacity");
                }

                if line == LINE_ENDING {
                    return if !result.is_empty() {
                        Err(Error::MissingOK { partial: result })
                    } else if allow_empty {
                        Ok(String::new())
                    } else {
                        trace!("recieved empty result");
                        continue;
                    };
                }
                if line.is_empty() {
                    error!("current result: {result:?}");
                    return Err(Error::PipeBroken("empty reader".to_owned()));
                }
                if line.starts_with("BatchCommand finished: ") {
                    let mut tmp = String::new();
                    self.read_pipe.read_line(&mut tmp).await.unwrap();
                    assert_eq!(
                        "\n", tmp,
                        "message didn't end after 'BatchCommand finished: {{}}'"
                    );
                    return match &line[23..(line.len() - 1)] {
                        "OK" => {
                            debug!("read '{result}' from audacity");
                            Ok(result) //fine
                        }
                        "Failed!" => Err(Error::AudacityErr(result)),
                        x => panic!("need error handling for {x}"),
                    };
                }
                result.push_str(&line);
            }
        }

        async fn send_msg(&mut self, msg: &str, allow_empty_result: bool) -> Result<String, Error> {
            self.just_write(&format!("Message: Text=\"{msg}\"")).await;
            let result = self.read(allow_empty_result).await?;
            Ok(result)
        }

        pub async fn ping(&mut self) -> Result<bool, Error> {
            let result = self.send_msg("ping", true).await?;

            if result.is_empty() {
                Ok(false)
            } else if result == "ping\n" {
                Ok(true)
            } else {
                Err(Error::MalformedResult(
                    format!("ping returned {result:?}"),
                    MalformedCause::Empty,
                ))
            }
        }

        pub async fn set_label(
            &mut self,
            i: usize,
            text: Option<String>,
            start: Option<String>,
            end: Option<String>,
        ) -> Result<(), Error> {
            let mut commant = format!("SetLabel: Label={i}");
            if let Some(text) = text {
                commant.push_str(" Text=\"");
                commant.push_str(&text);
                commant.push('"');
            }
            if let Some(start) = start {
                commant.push_str(" Start=");
                commant.push_str(&start);
            }
            if let Some(end) = end {
                commant.push_str(" End=");
                commant.push_str(&end);
            }
            let _result = self.write(&commant).await?;
            Ok(())
        }

        pub async fn get_label_info(
            &mut self,
        ) -> Result<Vec<(usize, Vec<(f64, f64, String)>)>, Error> {
            let json = self.write("GetInfo: Type=Labels Format=JSON").await?;
            serde_json::from_str(&json)
                .map_err(|e| Error::MalformedResult(json, MalformedCause::JSON(e)))
        }
        pub async fn get_track_info(&mut self) -> Result<Vec<result::TrackInfo>, Error> {
            let json = self.write("GetInfo: Type=Tracks Format=JSON").await?;
            serde_json::from_str::<Vec<result::RawTrackInfo>>(&json)
                .map_err(|e| Error::MalformedResult(json.clone(), MalformedCause::JSON(e)))?
                .into_iter()
                .map(|it| {
                    it.try_into()
                        .map_err(|e| Error::MalformedResult(json.clone(), MalformedCause::Own(e)))
                })
                .collect()
        }

        pub async fn import_audio<P: AsRef<Path> + Send>(&mut self, path: P) -> Result<(), Error> {
            let path = path
                .as_ref()
                .canonicalize()
                .map_err(|e| Error::PathErr(path.as_ref().to_path_buf(), e))?;

            let _result = self
                .write(&format!("Import2: Filename=\"{}\"", path.display()))
                .await?;

            Ok(())
        }

        pub async fn export_multiple(&mut self) -> Result<(), Error> {
            let _result = self.write("ExportMultiple:").await?;
            Ok(())
        }

        pub async fn export_labels(&mut self) -> Result<(), Error> {
            let _result = self.write("ExportLabels:").await?;
            Ok(())
        }
        pub async fn import_labels(&mut self) -> Result<(), Error> {
            let _result = self.write("ImportLabels:").await?;
            Ok(())
        }
        pub async fn open_new(&mut self) -> Result<(), Error> {
            let _result = self.write("New:").await?;
            Ok(())
        }
    }

    pub mod result {
        use serde::{Deserialize, Serialize};
        use thiserror::Error;

        #[derive(Debug, Error, Clone)]
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
}
