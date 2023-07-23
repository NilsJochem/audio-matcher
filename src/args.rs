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
        default_value_t = 13.0 as crate::mp3_reader::SampleType,
        help = "minimum prominence of the peaks"
    )]
    pub prominence: crate::mp3_reader::SampleType,
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

#[derive(Args, Debug, Clone, Copy)]
#[group(required = false, multiple = false)]
pub struct Inputs {
    #[clap(short, help = "always answer yes")]
    pub yes: bool,
    #[clap(short, help = "always answer no")]
    pub no: bool,
    #[clap(long, default_value_t = 3, help = "number of retrys")]
    pub trys: u8,
}
impl Inputs {
    pub fn ask_consent(&self, msg: &str) -> bool {
        if self.yes || self.no {
            return self.yes;
        }
        print!("{msg} [y/n]: ");
        for _ in 0..self.trys {
            let rin: String = text_io::read!("{}\n");
            if ["y", "yes", "j", "ja"].contains(&rin.as_str()) {
                return true;
            } else if ["n", "no", "nein"].contains(&rin.as_str()) {
                return false;
            }
            print!("couldn't parse that, please try again [y/n]: ");
        }
        println!("probably not");
        false
    }
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

impl From<OutputLevel> for super::leveled_output::OutputLevel {
    fn from(val: OutputLevel) -> Self {
        if val.silent {
            Self::Error
        } else if val.verbose {
            Self::Verbose
        } else if val.debug {
            Self::Debug
        } else {
            Self::Info
        }
    }
}

impl Arguments {
    pub fn should_overwrite_if_exists(&self, path: &std::path::PathBuf) -> bool {
        let out = !std::path::Path::new(path).exists() || {
            self.always_answer.ask_consent(&format!(
                "file '{}' already exists, overwrite",
                path.display()
            ))
        };
        if !out {
            crate::leveled_output::error(&format!("won't overwrite '{}'", path.display()));
        }
        out
    }
}
