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
    #[clap(value_name = "FILE", help = "path to audio file")]
    pub audio_paths: Vec<PathBuf>,
    #[clap(long, value_name = "FILE", help = "path to index file")]
    pub index_folder: Option<PathBuf>,

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
        let mut tmp_path = self.audio_paths.first().unwrap().clone(); // TODO find common value
        tmp_path.pop();
        tmp_path
    }
    #[allow(dead_code)]
    fn label_paths(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.audio_paths.iter().cloned().map(|mut label_path| {
            label_path.set_extension("txt");
            label_path
        })
    }
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

type Pattern = (String, ChapterNumber, String);

struct LazyApi {
    timeout: Option<Duration>,
    cache: Option<AudacityApi>,
}
impl LazyApi {
    const fn new(timeout: Option<Duration>) -> Self {
        Self {
            timeout,
            cache: None,
        }
    }
    pub const fn from_args(args: &Arguments) -> Self {
        Self::new(args.timeout)
    }
    pub async fn get_api_handle(&mut self) -> Result<&mut AudacityApi, Error> {
        let option = &mut self.cache;
        Ok(match option {
            Some(x) => x,
            None => option.insert({
                audacity::AudacityApi::launch_audacity().await?;
                audacity::AudacityApi::new(self.timeout).await?
            }),
        })
    }
}

pub struct Index {
    data: Vec<String>,
}
impl Index {
    async fn get_index(args: &Arguments, series: &str) -> Result<Option<Self>, Error> {
        Ok(match &args.index_folder {
            Some(folder) => Some(Self::read_index(folder.clone(), series).await?),
            None => {
                let path = args
                    .always_answer
                    .try_input(
                        "welche Index Datei m\u{f6}chtest du verwenden?: ",
                        Some(None),
                        |it| Some(Some(PathBuf::from(it))),
                    )
                    .unwrap_or_else(|| unreachable!());
                match path {
                    Some(path) => Some(Self::from_path(path).await?),
                    None => None,
                }
            }
        })
    }
    pub async fn from_path<P: AsRef<Path> + Send>(path: P) -> Result<Self, MoveError> {
        Ok(Self::from_slice_iter(
            tokio::fs::read_to_string(path).await?.lines(),
        ))
    }
    pub async fn read_index(mut base_folder: PathBuf, series: &str) -> Result<Self, MoveError> {
        base_folder.push(series);
        base_folder.push("index.txt");
        Self::from_path(base_folder).await
    }

    pub fn from_slice_iter<'a, Iter: Iterator<Item = &'a str>>(data: Iter) -> Self {
        Self {
            data: data
                .filter(|line| !line.starts_with('#'))
                .map_into()
                .collect_vec(),
        }
    }
    #[must_use]
    pub fn get(&self, chapter_number: ChapterNumber) -> &str {
        &self.data[chapter_number.nr() - 1]
    }
    #[must_use]
    pub fn try_get(&self, chapter_number: ChapterNumber) -> Option<&str> {
        self.data.get(chapter_number.nr() - 1).map(String::as_str)
    }
}

pub async fn run(args: &Arguments) -> Result<(), Error> {
    assert_eq!(
        args.audio_paths.len(),
        1,
        "currently only supporting 1 path at a time"
    );
    let mut audacity_api = LazyApi::from_args(args);

    if !args.skip_load {
        prepare_project(audacity_api.get_api_handle().await?, args).await?;
    }

    let patterns;
    if args.skip_name {
        assert!(
            log::max_level() >= log::Level::Debug,
            "skip-name only allowed, when log level is Debug or lower"
        );
        patterns = vec![
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
    } else {
        let audacity_api = audacity_api.get_api_handle().await?;
        let _ = args
            .always_answer
            .input("press enter when you are ready to start renaming", None);
        patterns = rename_labels(args, audacity_api).await?;

        let _ = args
            .always_answer
            .input("press enter when you are ready to finish", None);
        if args.dry_run {
            println!("asking to export audio and labels");
        } else {
            audacity_api.export_labels().await?;
            audacity_api.export_multiple().await?;
        }
    };

    move_results(patterns, args.tmp_path(), args).await?;
    Ok(())
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
    audacity
        .import_audio(&args.audio_paths.first().unwrap())
        .await?;
    trace!("loaded audio");
    audacity.import_labels().await?;
    Ok(())
}

///expecting that number of parts divides the length of the input or default to 4
const EXPECTED_PARTS: [usize; 13] = [0, 1, 2, 3, 4, 3, 3, 4, 4, 3, 5, 4, 4];
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

    let index = Index::get_index(args, &series).await?;
    let mut expected_next_chapter_number: Option<ChapterNumber> = None;

    while i < labels.len() {
        let chapter_number = args
            .always_answer
            .try_input(
                &format!(
                    "Welche Nummer hat die n\u{e4}chste Folge{}: ",
                    expected_next_chapter_number
                        .map_or_else(String::new, |next| format!(", erwarte {next}"))
                ),
                expected_next_chapter_number,
                |rin| rin.parse::<ChapterNumber>().ok(),
            )
            .expect("gib was vern\u{fc}nftiges ein");
        expected_next_chapter_number = Some(chapter_number.next());

        let chapter_name = index
            .as_ref()
            .map(|index| index.get(chapter_number))
            .map_or_else(
                || request_next_chapter_name(args),
                std::borrow::ToOwned::to_owned,
            );
        let number = read_number(
            args.always_answer,
            "Wie viele Teile hat die n\u{e4}chste Folge: ",
            Some(EXPECTED_PARTS.get(labels.len()).map_or(4, |i| *i)),
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

fn request_next_chapter_name(args: &Arguments) -> String {
    args.always_answer
        .input("Wie hei\u{df}t die n\u{e4}chste Folge: ", None)
}

fn read_number(input: Inputs, msg: &str, default: Option<usize>) -> usize {
    input
        .try_input(msg, default, |rin| rin.parse().ok())
        .expect("gib was vern\u{fc}nftiges ein")
}

#[cfg(test)]
mod tests {
    use super::*;

    mod index {
        use super::*;

        #[test]
        fn filter_comments() {
            let data = [
                "first element",
                "second element",
                "# some comment",
                "third element",
            ];
            let index = Index::from_slice_iter(data.into_iter());
            assert_eq!(index.get(ChapterNumber::new(1, false)), data[0]);
            assert_eq!(index.get(ChapterNumber::new(2, false)), data[1]);
            assert_eq!(index.get(ChapterNumber::new(3, false)), data[3]);
            assert_eq!(index.try_get(ChapterNumber::new(4, false)), None);
        }
    }
}
