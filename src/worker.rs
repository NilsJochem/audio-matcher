use audacity::AudacityApi;
use clap::Parser;
use itertools::Itertools;
use log::trace;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use thiserror::Error;
use tokio::task::JoinSet;

use crate::{
    archive::data::ChapterNumber,
    args::{parse_duration, Inputs, OutputLevel},
};

#[derive(Debug, Parser, Clone)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Arguments {
    #[clap(long, value_name = "FILE", help = "path to audio file")]
    pub audio_path: PathBuf,
    #[clap(long, value_name = "FILE", help = "path to index file")]
    pub index_file: Option<PathBuf>,

    #[clap(
        long,
        value_name = "DURATION",
        help = "timeout, can be just seconds, or somthing like 3h5m17s"
    )]
    #[arg(value_parser = parse_duration)]
    pub timeout: Option<Duration>,

    #[clap(long, help = "skips loading of data, assumes project is set up")]
    pub skip_load: bool,
    #[clap(long, help = "skips naming and exporting of labels")]
    pub skip_name: bool,

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

async fn get_api_handle<'a>(
    cache: &'a mut Option<AudacityApi>,
    args: &Arguments,
) -> Result<&'a mut AudacityApi, Error> {
    Ok(match cache {
        None => {
            audacity::AudacityApi::launch_audacity().await?;
            let x = audacity::AudacityApi::new(args.timeout).await?;
            cache.insert(x)
        }
        Some(a) => a,
    })
}

pub async fn run(args: &Arguments) -> Result<(), Error> {
    let mut audacity_cache: Option<AudacityApi> = None; // only start Audacity when needed

    if !args.skip_load {
        prepare_project(get_api_handle(&mut audacity_cache, args).await?, args).await?;
    }

    let patterns = if args.skip_name {
        debug_name()
    } else {
        let audacity_api = get_api_handle(&mut audacity_cache, args).await?;
        let ret = rename_labels(args, audacity_api).await?;

        let _ = args
            .always_answer
            .input("press enter when you are ready to finish", None);
        if args.dry_run {
            println!("asking to export audio and labels");
        } else {
            audacity_api.export_labels().await?;
            audacity_api.export_multiple().await?;
        }
        ret
    };

    move_results(patterns, args.tmp_path(), args).await?;
    Ok(())
}

type Pattern = (String, ChapterNumber, String);
fn debug_name() -> Vec<Pattern> {
    vec![
        (
            "Gruselkabinett".to_owned(),
            ChapterNumber::new(142, false),
            "Das Zeichen der Bestie".to_owned(),
        ),
        (
            "Gruselkabinett".to_owned(),
            ChapterNumber::new(143, false),
            "Der Wolverden-Turm".to_owned(),
        ),
    ]
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum Error {
    Move(#[from] MoveError),
    Launch(#[from] audacity::LaunchError),
    Audacity(#[from] audacity::Error),
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum MoveError {
    IO(#[from] tokio::io::Error),
    JoinError(#[from] tokio::task::JoinError),
    GlobPattern(#[from] glob::PatternError),
    Glob(#[from] glob::GlobError),
}

async fn move_result(dir: PathBuf, glob_pattern: String, dry_run: bool) -> Result<(), MoveError> {
    if dry_run {
        println!("create directory '{}'", dir.display());
        println!("moving {glob_pattern:?} to '{}'", dir.display());
        return Ok(());
    }
    tokio::fs::create_dir_all(&dir).await?;
    trace!("create directory {}", dir.display());

    let mut handles = JoinSet::new();
    for f in glob::glob(&glob_pattern)? {
        let f = f?;
        let mut target = dir.clone();
        target.push(f.file_name().unwrap());
        trace!("moving {} to {}", f.display(), target.display());
        handles.spawn(async move { tokio::fs::rename(&f, &target).await });
    }
    while let Some(result) = handles.join_next().await {
        result??;
    }
    Ok(())
}

async fn move_results(
    patterns: Vec<Pattern>,
    tmp_path: PathBuf,
    args: &Arguments,
) -> Result<(), MoveError> {
    let mut handles = JoinSet::new();
    for (series, nr, chapter) in patterns {
        let mut dir = tmp_path.clone();
        dir.push(format!("{nr} {chapter}"));
        let glob_pattern = format!("{}/{series} {nr}.* {chapter}.mp3", tmp_path.display());
        handles.spawn(move_result(dir, glob_pattern, args.dry_run));
    }
    while let Some(result) = handles.join_next().await {
        result??;
    }
    Ok(())
}

async fn prepare_project(
    audacity: &mut audacity::AudacityApi,
    args: &Arguments,
) -> Result<(), audacity::Error> {
    trace!("opened audacity");
    if audacity.get_track_info().await?.is_empty() {
        trace!("no need to open new project");
    } else {
        audacity.new_project().await?;
        trace!("opened new project");
    }
    audacity.import_audio(&args.audio_path).await?;
    trace!("loaded audio");
    audacity.import_labels().await?;
    Ok(())
}

async fn rename_labels(
    args: &Arguments,
    audacity: &mut audacity::AudacityApi,
) -> Result<Vec<Pattern>, Error> {
    let labels = audacity.get_label_info().await?;
    assert!(labels.len() == 1, "expecting one label track");
    let labels = labels.into_values().next().unwrap();

    let mut patterns = Vec::new();
    let mut i = 0;
    let series = args
        .always_answer
        .input("Welche Serie ist heute dran: ", None);
    let index = args.index_file.clone().map_or_else(
        || unsafe {
            args.always_answer
                .try_input(
                    "m\u{f6}chtest du eine Index Datei verwenden",
                    Some(None),
                    |it| Some(Some(it.into())),
                )
                .unwrap_unchecked()
        },
        Some,
    );
    let index = match index {
        Some(path) => Some(read_index(path).await?),
        None => None,
    };
    let mut expected_next_chapter_number: Option<ChapterNumber> = None;

    while i < labels.len() {
        let chapter_number = read_chapter_number(
            args.always_answer,
            &format!(
                "Welche Nummer hat die n\u{e4}chste Folge{}: ",
                expected_next_chapter_number
                    .map_or_else(String::new, |next| format!(", erwarte {next}"))
            ),
            expected_next_chapter_number,
        );
        expected_next_chapter_number = Some(chapter_number.next());

        let chapter_name = get_chapter_name_from_index(args, chapter_number, index.as_ref())
            .unwrap_or_else(|| request_next_chapter_name(args));
        let number = read_number(
            args.always_answer,
            "Wie viele Teile hat die n\u{e4}chste Folge: ",
            Some(4),
        );
        for j in 0..number.min(labels.len() - i) {
            let name = format!("{series} {chapter_number}.{} {chapter_name}", j + 1);
            audacity.set_label(i + j, Some(name), None, None).await?;
        }
        i += number;
        patterns.push((series.clone(), chapter_number, chapter_name));
    }
    Ok(patterns)
}

#[must_use]
pub fn get_chapter_name_from_index(
    args: &Arguments,
    chapter_number: ChapterNumber,
    index: Option<&Vec<String>>,
) -> Option<String> {
    index
        .map(|chaptes| chaptes[chapter_number.nr() - 1].clone())
        .filter(|it| {
            args.always_answer
                .ask_consent(&format!("Ist der Name {it:?} richtig"))
        })
}

fn request_next_chapter_name(args: &Arguments) -> String {
    args.always_answer
        .input("Wie hei\u{df}t die n\u{e4}chste Folge: ", None)
}

pub async fn read_index<P: AsRef<Path> + Send>(path: P) -> Result<Vec<String>, MoveError> {
    Ok(tokio::fs::read_to_string(path)
        .await?
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map_into()
        .collect_vec())
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

fn read_chapter_number(input: Inputs, msg: &str, default: Option<ChapterNumber>) -> ChapterNumber {
    input
        .try_input(msg, default, |rin| rin.parse::<ChapterNumber>().ok())
        .expect("gib was vern\u{fc}nftiges ein")
}

fn read_number(input: Inputs, msg: &str, default: Option<usize>) -> usize {
    input
        .try_input(msg, default, |rin| rin.parse().ok())
        .expect("gib was vern\u{fc}nftiges ein")
}
