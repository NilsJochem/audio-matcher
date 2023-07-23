use audio_matcher::{archive::run, error};
use clap::Parser;

fn main() {
    // let args = Arguments::parse();
    run().unwrap_or_else(|e| {
        error!("Program error :'{e}'");
        std::process::exit(1);
    });
}
