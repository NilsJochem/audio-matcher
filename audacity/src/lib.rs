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
        #[error("Failed with '{0}'")]
        AudacityErr(String), // TODO parse Error
        #[error("couldn't parse result '{0:?}' because {1}")]
        MalformedResult(String, serde_json::Error),
        #[error("Unkown path '{0}', {1}")]
        PathErr(PathBuf, std::io::Error), // TODO parse Error
    }

    #[derive(Debug)]
    #[must_use]
    pub struct AudacityApi {
        // to_name: PathBuf,
        // from_name: PathBuf,
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
        pub fn new(timer: Option<Duration>) -> Result<Self, Error> {
            let uid = unsafe { geteuid() };
            let base_path = "/tmp/audacity_script_pipe";
            let options = tokio::net::unix::pipe::OpenOptions::new();
            Ok(Self {
                write_pipe: BufWriter::new(
                    options
                        .open_sender(format!("{base_path}.to.{uid}"))
                        .map_err(|e| Error::PipeBroken(format!("open writer with {e:?}",)))?,
                ),
                read_pipe: BufReader::new(
                    options
                        .open_receiver(format!("{base_path}.from.{uid}"))
                        .map_err(|e| Error::PipeBroken(format!("open reader with {e:?}",)))?,
                ),
                timer,
            })
        }

        async fn write(&mut self, command: &str) -> Result<String, Error> {
            if let Some(_timer) = self.timer {
                todo!("add timeout");
            }
            self.write_pipe.write_all(command.as_bytes()).await.unwrap();
            self.write_pipe
                .write_all(LINE_ENDING.as_bytes())
                .await
                .unwrap();
            self.write_pipe.flush().await.unwrap();

            self.read().await
        }

        async fn read(&mut self) -> Result<String, Error> {
            let mut result = String::new();
            loop {
                let mut line = String::new();
                self.read_pipe.read_line(&mut line).await.unwrap();

                if line == LINE_ENDING && !result.is_empty() {
                    return Err(Error::MissingOK { partial: result });
                }
                if line.is_empty() {
                    eprintln!("current result: {result}");
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
                        "OK" => Ok(result), //fine
                        "Failed!" => Err(Error::AudacityErr(result)),
                        x => panic!("need error handling for {x}"),
                    };
                }
                result.push_str(&line);
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
            let _json = self.write(&commant).await?;
            Ok(())
        }

        pub async fn get_label_info(
            &mut self,
        ) -> Result<Vec<(usize, Vec<(f64, f64, String)>)>, Error> {
            let json = self.write("GetInfo: Type=Labels Format=JSON").await?;
            serde_json::from_str(&json).map_err(|e| Error::MalformedResult(json, e))
        }

        pub async fn import_audio<P: AsRef<Path> + Send>(&mut self, path: P) -> Result<(), Error> {
            let path = path
                .as_ref()
                .canonicalize()
                .map_err(|e| Error::PathErr(path.as_ref().to_path_buf(), e))?;

            let _json = self
                .write(&format!("Import2: Filename=\"{}\"", path.display()))
                .await?;

            Ok(())
        }

        pub async fn export_multiple(&mut self) -> Result<(), Error> {
            let _json = self.write("ExportMultiple:").await?;
            Ok(())
        }

        pub async fn export_labels(&mut self) -> Result<(), Error> {
            let _json = self.write("ExportLabels:").await?;
            Ok(())
        }
        pub async fn import_labels(&mut self) -> Result<(), Error> {
            let _json = self.write("ImportLabels:").await?;
            Ok(())
        }
        pub async fn open_new(&mut self) -> Result<(), Error> {
            let _json = self.write("Open:").await?;
            Ok(())
        }
    }
}
