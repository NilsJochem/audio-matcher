use std::path::PathBuf;

use clap::Parser;

use crate::args::{Inputs, OutputLevel};

#[derive(Debug, Parser, Clone)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Arguments {
    #[clap(value_name = "FILE", help = "path to folder of archive")]
    pub path: PathBuf,
    #[clap(long, short)]
    pub interactive: bool,

    #[clap(long)]
    pub dry_run: bool,

    #[command(flatten)]
    pub always_answer: Inputs,
    #[command(flatten)]
    pub output_level: OutputLevel,
}
