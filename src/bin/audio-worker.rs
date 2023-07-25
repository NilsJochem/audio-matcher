use clap::Parser;
use log::trace;
use std::{error::Error, path::PathBuf, process::Command, time::Duration};

use audio_matcher::args::{parse_duration, Inputs, OutputLevel};

const LAUNCHER: &str = "gtk4-launch";
const AUDACITY_APP_NAME: &str = "audacity";

fn launch_audacity() -> Result<bool, impl Error> {
    Command::new(LAUNCHER)
        .arg(AUDACITY_APP_NAME)
        .output()
        .map(|it| it.status.code() == Some(0))
}

#[derive(Debug, Parser, Clone)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
struct Arguments {
    #[clap(long, value_name = "FILE", help = "path to audio file")]
    pub audio_path: PathBuf,

    #[clap(
        long,
        value_name = "DURATION",
        help = "timeout, can be just seconds, or somthing like 3h5m17s"
    )]
    #[arg(value_parser = parse_duration)]
    pub timeout: Option<Duration>,

    #[clap(long)]
    pub dry_run: bool,

    #[command(flatten)]
    pub always_answer: Inputs,
    #[command(flatten)]
    pub output_level: OutputLevel,
}

impl Arguments {
    #[allow(dead_code)]
    fn tmp_path(&self) -> PathBuf {
        let mut tmp_path = self.audio_path.clone();
        tmp_path.pop();
        tmp_path
    }
    #[allow(dead_code)]
    fn label_path(&self) -> PathBuf {
        let mut label_path = self.audio_path.clone();
        label_path.set_extension("txt");
        label_path
    }
}

#[tokio::main]
async fn main() {
    let args = Arguments::parse();
    args.output_level.init_logger();

    run(&args).await.unwrap_or_else(|e| {
        log::error!("Program error :'{e}'");
        std::process::exit(1);
    });
}

async fn run(args: &Arguments) -> Result<(), Box<dyn Error>> {
    // let mut label_path: PathBuf = audio_path.clone();
    // label_path.set_extension("txt");
    let tmp_path = args.tmp_path();
    assert!(launch_audacity()?, "couldn't launch audacity");
    let mut audacity = audacity::AudacityApi::new(args.timeout).await?;
    trace!("opened audacity");
    if !audacity.get_track_info().await?.is_empty() {
        audacity.open_new().await?;
        trace!("opened new project");
    } else {
        trace!("no need to open new project");
    }

    audacity.import_audio(&args.audio_path).await?;
    trace!("loaded audio");
    audacity.import_labels().await?;
    let labels = audacity.get_label_info().await?;
    assert!(labels.len() == 1);
    let labels = &labels[0].1;

    let mut patterns = Vec::new();

    let mut i = 0;
    let series = args
        .always_answer
        .input("Welche Serie ist heute dran: ", None);
    while i < labels.len() {
        let chapter_number: String = args
            .always_answer
            .input("Welche Nummer hat die nächste Folge: ", None);
        let chapter_name: String = args
            .always_answer
            .input("Wie heißt die nächste Folge: ", None);
        let number = read_number(
            &args.always_answer,
            "Wie viele Teile hat die nächste Folge: ",
            Some(4),
        );
        for j in 0..number.min(labels.len() - i) {
            let name = format!("{series} {chapter_number}.{j} {chapter_name}");
            audacity.set_label(i + j, Some(name), None, None).await?;
        }
        i += number;
        patterns.push(format!("{chapter_number}# {chapter_name}"));
    }
    let _ = args
        .always_answer
        .input("press enter when you are ready to finish", None);
    audacity.export_labels().await?;
    audacity.export_multiple().await?;

    for p in patterns {
        let mut dir = tmp_path.clone();
        dir.push(p.replace('#', ""));
        std::fs::create_dir_all(&dir)?;
        for f in glob::glob(&p.replace('#', ".*"))? {
            let f = f?;
            let mut target = dir.clone();
            target.push(f.file_name().unwrap());
            std::fs::rename(f, target)?;
        }
    }
    Ok(())
}

// fn read_pattern(input: &Inputs, i: usize) -> String {
//     input
//         .try_input(
//             &format!("input label pattern {}+ (# for changing number): ", i),
//             None,
//             |rin| rin.contains('#').then_some(rin),
//         )
//         .expect("need #")
// }

fn read_number(input: &Inputs, msg: &str, default: Option<usize>) -> usize {
    input
        .try_input(msg, default, |rin| rin.parse().ok())
        .expect("gib was vernünftiges ein")
}
