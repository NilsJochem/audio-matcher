use audio_matcher::{args::Arguments, leveled_output::error, run};
use clap::Parser;

fn main() {
    let args = Arguments::parse();
    run(args).unwrap_or_else(|e| {
        error(&format!("Program error :'{e}'"));
        std::process::exit(1);
    });
}
