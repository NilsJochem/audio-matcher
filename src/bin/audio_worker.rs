use std::{path::PathBuf, process::Command};

use audio_matcher::args::Inputs;

const LAUNCHER: &str = "gtk4-launch";
const AUDACITY_APP_NAME: &str = "audacity";

fn launch_audacity() -> bool {
    let out = Command::new(LAUNCHER)
        .arg(AUDACITY_APP_NAME)
        .output()
        .unwrap();
    out.status.code() == Some(0)
}

#[tokio::main]
async fn main() {
    let audio_path: PathBuf = PathBuf::from("./res/local/small_test.mp3");
    // let mut label_path: PathBuf = audio_path.clone();
    // label_path.set_extension("txt");
    // let mut tmp_path = audio_path.clone();
    // tmp_path.pop();
    let tmp_path = PathBuf::from("~/Musik/newly ripped/Aufnahmen/");
    let input = Inputs::test();

    assert!(launch_audacity(), "couldn't launch audacity");
    let mut audacity = audacity::scripting_interface::AudacityApi::new(None).unwrap();
    audacity.open_new().await.unwrap();
    audacity.import_audio(audio_path).await.unwrap();
    audacity.import_labels().await.unwrap();
    let labels = audacity.get_label_info().await.unwrap();
    assert!(labels.len() == 1);
    let labels = &labels[0].1;

    let mut patterns = Vec::new();

    let mut i = 0;
    while i < labels.len() {
        let pattern: String = read_pattern(&input, i + 1);
        let number: usize = read_number(&input);
        for j in 0..number.min(labels.len() - i) {
            let name = pattern.replace('#', &format!(".{}", j + 1));
            audacity.set_label(i, Some(name), None, None).await.unwrap();
            i += 1;
        }
        patterns.push(pattern);
    }
    let _ = input.input("press enter when you are ready to finish", None);
    audacity.export_labels().await.unwrap();
    audacity.export_multiple().await.unwrap();

    for p in patterns {
        let mut dir = tmp_path.clone();
        dir.push(p.replace('#', ""));
        std::fs::create_dir_all(&dir).unwrap();
        for f in glob::glob(&p.replace('#', ".*")).unwrap() {
            let f = f.unwrap();
            let mut target = dir.clone();
            target.push(f.file_name().unwrap());
            std::fs::rename(f, target).unwrap();
        }
    }

    //TODO move files
}

fn read_pattern(input: &Inputs, i: usize) -> String {
    input
        .try_input(
            &format!("input label pattern {}+ (# for changing number): ", i),
            None,
            |rin| rin.contains('#').then_some(rin),
        )
        .expect("need #")
}

fn read_number(input: &Inputs) -> usize {
    input
        .try_input(
            "number of labels (default 4): ",
            Some(4),
            |rin| rin.parse().ok(),
        )
        .expect("gib was vernünftiges ein")
}
