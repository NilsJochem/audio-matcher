use clap::{Args, Parser};
use std::path::PathBuf;

use crate::args::{Inputs, OutputLevel};

#[derive(Debug, Parser, Clone)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Arguments {
    #[clap(value_name = "FILE", help = "file in which samples are searched")]
    pub within: Vec<PathBuf>,

    #[clap(long, value_name = "FILE", help = "snippet to be found in file")]
    pub snippet: PathBuf,

    #[clap(
        short,
        long,
        default_value_t = 13.0 as crate::matcher::mp3_reader::SampleType,
        help = "minimum prominence of the peaks"
    )]
    pub prominence: crate::matcher::mp3_reader::SampleType,
    #[clap(long, default_value_t = 8*60, value_name = "SECONDS", help="minimum distance between matches in seconds")]
    pub distance: usize,
    #[clap(
        long,
        default_value_t = 60,
        value_name = "SECONDS",
        help = "length in seconds of chunks to be processed"
    )]
    pub chunk_size: usize,
    #[clap(long, help = "use fancy bar, needs fira ttf to work")]
    pub fancy_bar: bool,
    // #[clap(long, help="use new implementation for fftcorrelate")]
    // pub new_correlate: bool,
    #[clap(long)]
    pub dry_run: bool,

    #[command(flatten)]
    pub always_answer: Inputs,
    #[command(flatten)]
    pub out_file: OutFile,
    #[command(flatten)]
    pub output_level: OutputLevel,
}

#[derive(Args, Debug, Clone, Default)]
#[group(required = false, multiple = false)]
pub struct OutFile {
    #[clap(long, help = "generates no file with times")]
    pub no_out: bool,
    #[clap(
        short,
        long = "out",
        value_name = "FILE",
        help = "file to save a text track"
    )]
    pub out_file: Option<PathBuf>,
}

impl Arguments {
    #[must_use]
    pub fn should_overwrite_if_exists(&self, path: &std::path::PathBuf) -> bool {
        let out = !std::path::Path::new(path).exists() || {
            self.always_answer.ask_consent(&format!(
                "file '{}' already exists, overwrite",
                path.display()
            ))
        };
        if !out {
            crate::error!("won't overwrite '{}'", path.display());
        }
        out
    }
}
