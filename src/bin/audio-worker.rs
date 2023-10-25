#[tokio::main]
async fn main() {
    let args = audio_matcher::worker::args::Arguments::parse();
    audio_matcher::worker::run(&args).await.unwrap_or_else(|e| {
        log::error!("Program error :'{e}'");
        std::process::exit(1);
    });
}
