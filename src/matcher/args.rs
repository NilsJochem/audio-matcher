use clap::{Args, Parser};
use std::{path::PathBuf, time::Duration};

use crate::args::parse_duration;
use common::args::{debug::OutputLevel, input::Inputs};

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
    #[clap(
        long,
        value_name = "SECONDS",
        help = "minimum distance between matches in seconds"
    )]
    #[arg(value_parser = parse_duration)]
    distance: Option<Duration>,
    #[clap(
        long,
        value_name = "SECONDS",
        help = "length in seconds of chunks to be processed"
    )]
    #[arg(value_parser = parse_duration)]
    chunk_size: Option<Duration>,
    #[clap(long, help = "use fancy bar, needs fira ttf to work")]
    pub fancy_bar: bool,
    // #[clap(long, help="use new implementation for fftcorrelate")]
    // pub new_correlate: bool,
    #[clap(long)]
    pub dry_run: bool,
    #[clap(long)]
    pub skip_existing: bool,

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
    pub fn chunk_size(&self) -> Duration {
        self.chunk_size.unwrap_or(Duration::from_secs(60))
    }
    #[must_use]
    pub fn distance(&self) -> Duration {
        self.distance.unwrap_or(Duration::from_secs(8 * 60))
    }
}
