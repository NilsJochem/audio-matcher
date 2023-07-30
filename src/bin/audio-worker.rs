use clap::Parser;

#[tokio::main]
async fn main() {
    let args = audio_matcher::worker::Arguments::parse();
    args.output_level.init_logger();
    audio_matcher::worker::run(&args).await.unwrap_or_else(|e| {
        log::error!("Program error :'{e}'");
        std::process::exit(1);
    });
}
