use clap::Parser;

fn main() {
    let args = audio_matcher::matcher::args::Arguments::parse();
    args.output_level.init_logger();
    audio_matcher::matcher::run(&args).unwrap_or_else(|e| {
        log::error!("Program error :'{e}'");
        std::process::exit(1);
    });
}
