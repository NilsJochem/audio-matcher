use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::args::{parse_duration, ConfigArgs};
use clap::Parser;
use common::args::{debug::OutputLevel, input::Inputs};

#[derive(Debug, Parser, Clone)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Parameter {
    #[clap(value_name = "FILE", help = "path to audio file")]
    pub audio_paths: Vec<PathBuf>,
    #[clap(long, value_name = "FILE", help = "path to index file")]
    pub index_folder: Option<PathBuf>,

    #[clap(
        long,
        value_name = "DURATION",
        help = "timeout, can be just seconds, or somthing like 3h5m17s"
    )]
    #[arg(value_parser = parse_duration)]
    pub timeout: Option<Duration>,

    #[clap(
        long,
        default_value_t = Cow::Borrowed("mp3"),
        value_name = "FORMAT",
        help = "expected format of exported files"
    )]
    pub export_ext: Cow<'static, str>,

    #[clap(long, help = "skips loading of data, assumes project is set up")]
    pub skip_load: bool,
    #[clap(long, help = "skips naming and exporting of labels")]
    pub skip_name: bool,

    #[clap(long)]
    pub dry_run: bool,

    #[command(flatten)]
    pub config: ConfigArgs,
    #[command(flatten)]
    pub always_answer: Inputs,
    #[command(flatten)]
    pub output_level: OutputLevel,
}

const SUB_CONFIG: &str = "worker";
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub genre: String,
    pub index_folder: Option<PathBuf>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            genre: "H\u{f6}rbuch".to_owned(),
            index_folder: None,
        }
    }
}

pub struct Arguments {
    config: Config,
    parameter: Parameter,
}
impl Arguments {
    #[must_use]
    pub const fn from(config: Config, parameter: Parameter) -> Self {
        Self { config, parameter }
    }
    #[must_use]
    pub fn parse() -> Self {
        let param = Parameter::parse();
        param.output_level.init_logger();
        let mut config = param.config.load_config::<Config>(SUB_CONFIG);

        if config.index_folder.is_none()
            && param.index_folder.is_some()
            && param.always_answer.ask_consent(format!(
                "Willst du die Indexdatei {:?} in der config speichern?",
                param.index_folder.as_ref().unwrap()
            ))
        {
            config.index_folder = param.index_folder.clone();
            param.config.save_config(SUB_CONFIG, &config);
        }

        Self::from(config, param)
    }

    #[must_use]
    pub fn genre(&self) -> &str {
        &self.config.genre
    }
    #[must_use]
    pub fn index_folder(&self) -> Option<&Path> {
        self.parameter
            .index_folder
            .as_ref()
            .or(self.config.index_folder.as_ref())
            .map(std::path::PathBuf::as_path)
    }

    #[must_use]
    pub const fn audio_paths(&self) -> &Vec<PathBuf> {
        &self.parameter.audio_paths
    }
    #[must_use]
    pub const fn timeout(&self) -> Option<Duration> {
        self.parameter.timeout
    }
    #[must_use]
    pub const fn skip_load(&self) -> bool {
        self.parameter.skip_load
    }
    #[must_use]
    pub const fn skip_name(&self) -> bool {
        self.parameter.skip_name
    }
    #[must_use]
    pub const fn dry_run(&self) -> bool {
        self.parameter.dry_run
    }
    #[must_use]
    pub const fn always_answer(&self) -> Inputs {
        self.parameter.always_answer
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn tmp_path(&self) -> &Path {
        self.parameter
            .audio_paths
            .first() // TODO find common value
            .expect("no paths")
            .parent()
            .expect("path without parent")
    }
    #[allow(dead_code)]
    fn label_paths(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.parameter
            .audio_paths
            .iter()
            .map(|label_path| label_path.with_extension("txt"))
    }
    #[must_use]
    pub fn export_ext(&self) -> &str {
        self.parameter.export_ext.as_ref()
    }
}
