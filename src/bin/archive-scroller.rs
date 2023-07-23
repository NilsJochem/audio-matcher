use audio_matcher::{archive::run, error};
use clap::Parser;

fn main() {
    let args = audio_matcher::archive::args::Arguments::parse();
    run(&args).unwrap_or_else(|e| {
        error!("Program error :'{e}'");
        std::process::exit(1);
    });
}
