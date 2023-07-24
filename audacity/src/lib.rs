pub mod scripting_interface {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };
    use thiserror::Error;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};

    #[cfg(windows)]
    const LINE_ENDING: &'static str = "\r\n";
    #[cfg(not(windows))]
    const LINE_ENDING: &'static str = "\n";

    #[link(name = "c")]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    extern "C" {
        fn geteuid() -> u32;
    }

    #[derive(Error, Debug)]
    pub enum Error {
        #[error("Pipe Broken")]
        PipeBroken,
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
    pub struct AudacityApi {
        to_name: PathBuf,
        from_name: PathBuf,
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
        pub fn new(timer: Option<Duration>) -> Self {
            let uid = unsafe { geteuid() };
            let base_path = "/tmp/audacity_script_pipe";
            Self {
                to_name: PathBuf::from(format!("{base_path}.to.{uid}")),
                from_name: PathBuf::from(format!("{base_path}.from.{uid}")),
                timer,
            }
        }

        async fn write(&mut self, command: &str) -> Result<String, Error> {
            let mut write_pipe = BufWriter::new(
                tokio::net::unix::pipe::OpenOptions::new()
                    .open_sender(&self.to_name)
                    .map_err(|_| Error::PipeBroken)?,
            );
            if let Some(_timer) = self.timer {
                todo!("add timeout");
            }
            write_pipe.write_all(command.as_bytes()).await.unwrap();
            write_pipe.write_all(LINE_ENDING.as_bytes()).await.unwrap();
            write_pipe.flush().await.unwrap();

            self.read().await
        }

        async fn read(&mut self) -> Result<String, Error> {
            let mut reader = BufReader::new(
                tokio::net::unix::pipe::OpenOptions::new()
                    .open_receiver(&self.from_name)
                    .map_err(|_| Error::PipeBroken)?,
            );

            let mut result = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();

                if line == LINE_ENDING && !result.is_empty() {
                    return Err(Error::MissingOK { partial: result });
                }
                if line.is_empty() {
                    return Err(Error::PipeBroken);
                }
                if line.starts_with("BatchCommand finished: ") {
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
                commant.push_str(" Text=");
                commant.push_str(&text);
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

        pub async fn import_audio<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
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
    }
}
