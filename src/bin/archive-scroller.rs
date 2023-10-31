use audio_matcher::archive::args::{Arguments, Config};
use clap::Parser;

const CONFIG_NAME: &str = "archive";
fn main() {
    let mut args = Arguments::parse();
    args.output_level.init_logger();
    let mut config: Config = args.config.load_config(CONFIG_NAME);
    let mut changed = false;
    config.path.get_or_insert_with(|| {
        changed = true;
        args.archive
            .as_ref()
            .filter(|path| {
                args.always_answer
                    .ask_consent(format!("should the path {path:?} be saved to the config"))
            })
            .cloned()
            .unwrap_or_else(|| {
                audio_matcher::args::Inputs::input("please input the path to the archive: ", None)
                    .into()
            })
    });
    if changed {
        args.config.save_config(CONFIG_NAME, &config);
    }
    args.archive.get_or_insert_with(|| {
        config
            .path
            .clone()
            .expect("need at least one path, either in path or in config")
    });
    audio_matcher::archive::run(&args).unwrap_or_else(|e| {
        log::error!("Program error :'{e}'");
        std::process::exit(1);
    });
}
