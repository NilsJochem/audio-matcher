use clap::{Args, Parser};
use std::path::PathBuf;

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
        default_value_t = 250.0,
        help = "minimum prominence of the peaks"
    )]
    pub prominence: f64,
    #[clap(long, default_value_t = 5*60, value_name = "SECONDS", help="minimum distance between matches in seconds")]
    pub distance: usize,
    #[clap(long, default_value_t = 2*60, value_name = "SECONDS", help="length in seconds of chunks to be processed")]
    pub chunk_size: usize,

    #[command(flatten)]
    pub always_answer: Inputs,
    #[command(flatten)]
    pub out_file: OutFile,
    #[command(flatten)]
    pub output_level: OutputLevel,
}

#[derive(Args, Debug, Clone)]
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

#[derive(Args, Debug, Clone, Copy)]
#[group(required = false, multiple = false)]
pub struct Inputs {
    #[clap(short, help = "always answer yes")]
    pub yes: bool,
    #[clap(short, help = "always answer no")]
    pub no: bool,
}

#[derive(Args, Debug, Clone, Copy)]
#[group(required = false, multiple = false)]
pub struct OutputLevel {
    #[clap(short, long, help = "print maximum info")]
    debug: bool,
    #[clap(short, long, help = "print more info")]
    verbose: bool,
    #[clap(short, long, help = "print less info")]
    silent: bool,
}

impl Into<super::leveled_output::OutputLevel> for OutputLevel {
    fn into(self) -> super::leveled_output::OutputLevel {
        return if self.silent {
            super::leveled_output::OutputLevel::Error
        } else if self.verbose {
            super::leveled_output::OutputLevel::Verbose
        } else if self.debug {
            super::leveled_output::OutputLevel::Debug
        } else {
            super::leveled_output::OutputLevel::Info
        };
    }
}
