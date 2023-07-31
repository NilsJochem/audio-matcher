use std::path::PathBuf;

use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::args::{ConfigArgs, Inputs, OutputLevel};

#[derive(Debug, Parser, Clone)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Arguments {
    #[clap(value_name = "FILE", help = "path to folder of archive")]
    pub archive: Option<PathBuf>,
    #[clap(long, short)]
    pub interactive: bool,

    #[clap(long)]
    pub dry_run: bool,

    #[command(flatten)]
    pub config: ConfigArgs,
    #[command(flatten)]
    pub always_answer: Inputs,
    #[command(flatten)]
    pub output_level: OutputLevel,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub version: u8,
    pub path: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            path: None,
        }
    }
}
