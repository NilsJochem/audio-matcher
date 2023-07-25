use audio_matcher::matcher::{args::Arguments, run};
use clap::Parser;

fn main() {
    let args = Arguments::parse();
    args.output_level.init_logger();
    run(&args).unwrap_or_else(|e| {
        log::error!("Program error :'{e}'");
        std::process::exit(1);
    });
}
