use audio_matcher::{args::Arguments, run, error};
use clap::Parser;

fn main() {
    let args = Arguments::parse();
    run(&args).unwrap_or_else(|e| {
        error!("Program error :'{e}'");
        std::process::exit(1);
    });
}
