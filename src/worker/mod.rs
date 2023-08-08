use audacity::AudacityApi;
use log::{debug, trace};
use std::{path::PathBuf, time::Duration};
use thiserror::Error;
use tokio::task::JoinSet;

use crate::{
    archive::data::{build_timelabel_name, ChapterNumber},
    args::Inputs,
    extensions::vec::PushReturn,
    iter::CloneIteratorExt,
    worker::tagger::{
        Album, Artist, Disc, Genre, TaggedFile, Title, TotalDiscs, TotalTracks, Track,
    },
};

use self::args::Arguments;

pub mod args;
mod index;
pub mod tagger;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum Error {
    Index(#[from] index::Error),
    Move(#[from] MoveError),
    Launch(#[from] audacity::LaunchError),
    Audacity(#[from] audacity::Error),
    Tag(#[from] id3::Error),
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
        Self::new(args.timeout())
    }
    pub async fn get_api_handle(&mut self) -> Result<&mut AudacityApi, Error> {
        let option = &mut self.cache;
        Ok(match option {
            Some(x) => x,
            None => option.insert({
                audacity::AudacityApiGeneric::launch_audacity().await?;
                audacity::AudacityApiGeneric::new(self.timeout).await?
            }),
        })
    }
}

pub async fn run(args: &Arguments) -> Result<(), Error> {
    assert_eq!(
        args.audio_paths().len(),
        1,
        "currently only supporting 1 path at a time"
    );
    let mut audacity_api = LazyApi::from_args(args);

    if !args.skip_load() {
        prepare_project(audacity_api.get_api_handle().await?, args).await?;
    }

    let patterns;
    if args.skip_name() {
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
        ];
    } else {
        let audacity_api = audacity_api.get_api_handle().await?;
        let _ = args
            .always_answer()
            .input("press enter when you are ready to start renaming", None);
        let tags;
        (patterns, tags) = rename_labels(args, audacity_api).await?;
        adjust_labels(args, audacity_api).await?;

        let _ = args
            .always_answer()
            .input("press enter when you are ready to finish", None);
        if args.dry_run() {
            println!("asking to export audio and labels");
            for tag in tags {
                tag.drop_changes();
            }
        } else {
            audacity_api
                .write_assume_empty(audacity::command::ExportLabels)
                .await?;
            audacity_api
                .write_assume_empty(audacity::command::ExportMultiple)
                .await?;
            for mut tag in tags {
                tag.reload_empty()?;
                tag.save_changes(false)?;
            }
        }
    };

    move_results(patterns, args.tmp_path(), args).await?;
    Ok(())
}

async fn prepare_project(
    audacity: &mut AudacityApi,
    args: &Arguments,
) -> Result<(), audacity::Error> {
    trace!("opened audacity");
    if audacity.get_track_info().await?.is_empty() {
        trace!("no need to open new project");
    } else {
        audacity.write_assume_empty(audacity::command::New).await?;
        trace!("opened new project");
    }
    audacity
        .import_audio(&args.audio_paths().first().unwrap())
        .await?;
    trace!("loaded audio");
    audacity
        .write_assume_empty(audacity::command::ImportLabels)
        .await?;
    Ok(())
}

///expecting that number of parts divides the length of the input or default to 4
const EXPECTED_PARTS: [usize; 13] = [0, 1, 2, 3, 4, 3, 3, 4, 4, 3, 5, 4, 4];
async fn rename_labels(
    args: &Arguments,
    audacity: &mut AudacityApi,
) -> Result<(Vec<Pattern>, Vec<TaggedFile>), Error> {
    let labels = audacity.get_label_info().await?;
    assert!(labels.len() == 1, "expecting one label track");
    let labels = labels.into_values().next().unwrap();

    let mut patterns = Vec::new();
    let mut tags = Vec::new();
    let mut i = 0;
    let series = args
        .always_answer()
        .input("Welche Serie ist heute dran: ", None);

    let index = crate::worker::index::Index::get_index(args, &series).await?;
    let index_len = index.as_ref().map(index::Index::len);
    let mut expected_next_chapter_number: Option<ChapterNumber> = None;

    while i < labels.len() {
        let chapter_number = args
            .always_answer()
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

        let index_value = index.as_ref().map(|index| index.get(chapter_number));
        let artist = index_value.and_then(|it| it.1);
        let chapter_name =
            index_value.map_or_else(|| request_next_chapter_name(args), |(n, _)| n.to_owned());

        let number = read_number(
            args.always_answer(),
            "Wie viele Teile hat die n\u{e4}chste Folge: ",
            Some(EXPECTED_PARTS.get(labels.len()).map_or(4, |i| *i)),
        )
        .min(labels.len() - i);
        for j in 0..number {
            let name = build_timelabel_name(&series, &chapter_number, j + 1, &chapter_name);

            let mut path = args.tmp_path();
            path.push(format!("{name}.mp3"));
            let tag = tags.push_return(TaggedFile::new_empty(path));
            tag.set::<Title>(Some(&format!("{chapter_name} {}", j + 1)));
            tag.set::<Album>(Some(&series));
            tag.set::<Track>(Some((j + 1) as u32));
            tag.set::<TotalTracks>(Some(number as u32));
            tag.set::<Genre>(Some(args.genre()));
            tag.set::<Disc>(Some(chapter_number.nr() as u32));
            if let Some(l) = index_len {
                tag.set::<TotalDiscs>(Some(l as u32));
            }
            if let Some(artist) = artist {
                tag.set::<Artist>(Some(artist));
            }

            audacity
                .set_label(i + j, Some(name), None, None, Some(false))
                .await?;
        }
        i += number;
        patterns.push((series.clone(), chapter_number, chapter_name));
    }
    Ok((patterns, tags))
}

pub async fn adjust_labels(
    args: &Arguments,
    audacity: &mut AudacityApi,
) -> Result<(), audacity::Error> {
    let labels = audacity.get_label_info().await?; // get new labels

    for element in labels.values().flatten().open_border_pairs() {
        let (prev_end, next_start) = match element {
            crate::iter::State::Start(a) => (a.start, a.start + Duration::from_secs(10)),
            crate::iter::State::Middle(a, b) => (a.end, b.start),
            crate::iter::State::End(b) => (b.end, b.end + Duration::from_secs(10)),
        };
        audacity
            .select_time(
                Some(prev_end - Duration::from_secs(10)),
                Some(next_start + Duration::from_secs(10)),
                Some(audacity::RelativeTo::ProjectStart),
            )
            .await?;

        audacity
            .write_assume_empty(audacity::command::ZoomSel)
            .await?;
        let _ = args.always_answer().input(
            "Dr\u{fc}ck Enter, wenn du bereit f\u{fc}r den n\u{e4}chsten Schritt bist",
            None,
        );
    }
    audacity
        .write_assume_empty(audacity::command::ZoomNormal)
        .await?;
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
        dir.push("current");
        dir.push(&series);
        dir.push(format!("{nr} {chapter}"));

        let mut glob_path = tmp_path.clone();
        glob_path.push(format!("{series} {nr}.* {chapter}.mp3"));
        handles.spawn(move_result(
            dir,
            glob_path
                .to_str()
                .expect("glob_path contained non UTF-8 char")
                .to_owned(),
            args.dry_run(),
        ));
    }
    while let Some(result) = handles.join_next().await {
        result??;
    }
    Ok(())
}
async fn move_result(dir: PathBuf, glob_pattern: String, dry_run: bool) -> Result<(), MoveError> {
    if dry_run {
        println!("create directory {dir:?}");
        println!("moving {glob_pattern:?} to {dir:?}");
        return Ok(());
    }
    tokio::fs::create_dir_all(&dir).await?;
    trace!("create directory {dir:?}");

    let mut handles = JoinSet::new();
    for f in glob::glob(&glob_pattern)? {
        let f = f?;
        let mut target = dir.clone();
        target.push(f.file_name().unwrap());
        trace!("moving {f:?} to {target:?}");
        handles.spawn(async move {
            match tokio::fs::rename(&f, &target).await {
                Ok(()) => Ok(()),
                Err(_err) => {
                    debug!("couldn't just rename file, try to copy and remove old");
                    tokio::fs::copy(&f, &target).await?;
                    tokio::fs::remove_file(&f).await
                }
            }
        });
    }
    while let Some(result) = handles.join_next().await {
        result??;
    }
    Ok(())
}

fn request_next_chapter_name(args: &Arguments) -> String {
    args.always_answer()
        .input("Wie hei\u{df}t die n\u{e4}chste Folge: ", None)
}

fn read_number(input: Inputs, msg: &str, default: Option<usize>) -> usize {
    input
        .try_input(msg, default, |rin| rin.parse().ok())
        .expect("gib was vern\u{fc}nftiges ein")
}
