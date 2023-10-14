use audacity::AudacityApi;
use common::extensions::{
    iter::{CloneIteratorExt, FutIterExt, State},
    vec::PushReturn,
};
use futures::TryFutureExt;
use itertools::{Itertools, Position};
use log::trace;
use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::OsString,
    fmt::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use thiserror::Error;
use toml::value::{Date, Datetime};

use crate::{
    archive::data::{build_timelabel_name, ChapterNumber},
    args::Inputs,
    worker::tagger::{Album, Artist, Genre, TaggedFile, Title, TotalTracks, Track, Year},
};

use self::{
    args::Arguments,
    index::{Index, MultiIndex},
};

pub mod args;
pub mod index;
pub mod tagger;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum Error {
    Index(#[from] index::Error),
    Move(#[from] MoveError),
    Launch(#[from] audacity::LaunchError),
    Audacity(#[from] audacity::Error),
    #[error("id3 Error {1} for {0:?}")]
    Tag(PathBuf, #[source] tagger::Error),
}

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
                audacity::AudacityApiGeneric::launch(None).await?;
                audacity::AudacityApiGeneric::new(self.timeout).await?
            }),
        })
    }
}

pub async fn run(args: &Arguments) -> Result<(), Error> {
    assert!(
        !args.skip_load() || args.audio_paths().len() == 1,
        "skipping only allowed with single audio"
    );
    let mut audacity_api = LazyApi::from_args(args);
    let mut m_index = match args.index_folder() {
        Some(path) => Some((MultiIndex::new(path.to_owned())).await),
        None => None,
    };

    for (pos, audio_path) in args.audio_paths().iter().with_position() {
        let label_path = audio_path.with_extension("txt");
        let audacity_api = audacity_api.get_api_handle().await?;

        if !args.skip_load() {
            prepare_project(audacity_api, audio_path, &label_path).await?;
        }
        audacity_api
            .zoom_to(audacity::Selection::All, audacity::Save::Discard)
            .await?;

        // start rename
        if !args.skip_name() {
            let _ = args
                .always_answer()
                .input("press enter when you are ready to start renaming", None);
            rename_labels(args, audacity_api, m_index.as_mut()).await?;
            adjust_labels(args, audacity_api).await?;

            audacity_api
                .export_all_labels_to(label_path, args.dry_run())
                .await?;
        }

        //start export
        let tags = merge_parts(
            args,
            audacity_api,
            m_index.as_mut().expect("need index"),
            audacity::TrackHint::LabelTrackNr(0),
        )
        .await?;
        let _ = args.always_answer().input(
            "remove all lables you don't want to remove and then press enter to start exporting",
            None,
        );
        audacity_api
            .write_assume_empty(audacity::command::ExportMultiple)
            .await?;

        let (mut tags, missing) = tags
            .into_iter()
            .partition::<Vec<_>, _>(|tag| tag.path().exists());

        missing.into_iter().for_each(TaggedFile::drop_changes);

        if tags.is_empty() {
            log::warn!("no files exported, skipping move");
        } else {
            for tag in &mut tags {
                tag.reload_empty()
                    .map_err(|err| Error::Tag(tag.path().into(), err))?;
                tag.save_changes(false)
                    .map_err(|err| Error::Tag(tag.path().into(), err))?;
            }
            move_results(
                tags.iter(),
                args.tmp_path(),
                args.index_folder().unwrap_or_else(|| args.tmp_path()),
                args,
            )
            .await?;
        }
        drop(tags);

        if !args.skip_load() {
            // clear audacity after each round, but exit in last round
            audacity_api
                .write_assume_empty(match pos {
                    Position::First | Position::Middle => audacity::command::Close,
                    Position::Last | Position::Only => audacity::command::Exit,
                })
                .await?;
        }
    }

    Ok(())
}

async fn prepare_project(
    audacity: &mut AudacityApi,
    audio_path: impl AsRef<Path> + Send,
    label_path: impl AsRef<Path> + Send + Sync,
) -> Result<(), audacity::Error> {
    trace!("opened audacity");
    if audacity.get_track_info().await?.is_empty() {
        trace!("no need to open new project");
    } else {
        audacity.write_assume_empty(audacity::command::New).await?;
        trace!("opened new project");
    }
    audacity.import_audio(audio_path).await?;
    trace!("loaded audio");
    audacity
        .import_labels_from(label_path, None::<&str>)
        .await?;
    Ok(())
}

#[derive(Debug)]
pub struct ChapterCompleter<'a> {
    index: &'a Index<'a>,
    filter: Box<dyn crate::args::autocompleter::StrFilter + Send + Sync>,
}
impl<'a> ChapterCompleter<'a> {
    pub fn new(
        index: &'a Index<'a>,
        filter: impl crate::args::autocompleter::StrFilter + Send + Sync + 'static,
    ) -> Self {
        Self::new_boxed(index, Box::new(filter))
    }

    #[must_use]
    pub fn new_boxed(
        index: &'a Index<'a>,
        filter: Box<dyn crate::args::autocompleter::StrFilter + Send + Sync>,
    ) -> Self {
        Self { index, filter }
    }
    #[must_use]
    pub const fn index(&self) -> &Index<'a> {
        self.index
    }
    fn filter(&self) -> &dyn crate::args::autocompleter::StrFilter {
        self.filter.as_ref()
    }
}

impl<'a> crate::args::autocompleter::MyAutocomplete for ChapterCompleter<'a> {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, inquire::CustomUserError> {
        Ok(match input.parse::<ChapterNumber>() {
            Ok(number) => {
                if number.is_maybe {
                    // number ends  with '?', so nothing more will come
                    self.index()
                        .try_get(number)
                        .map_or_else(Vec::new, |it| vec![(number, it)])
                } else {
                    // find all possible numbers starting with current input
                    (0..self.index().main_len())
                        .filter_map(|i| {
                            i.to_string().starts_with(&number.nr.to_string()).then(|| {
                                let number = ChapterNumber::new(i, false);
                                (number, self.index().get(number))
                            })
                        })
                        .collect_vec()
                }
            }
            Err(_) => self
                .index()
                .chapter_iter()
                .enumerate()
                .filter(|(_, option)| self.filter().filter(&option.title, input))
                .map(|(i, chapter)| (ChapterNumber::new(i + 1, false), chapter.clone()))
                .collect_vec(),
        }
        .into_iter()
        .map(|(i, chapter)| format!("{i} {}", chapter.title))
        .collect_vec())
    }

    fn get_completion(
        &mut self,
        _input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<inquire::autocompletion::Replacement, inquire::CustomUserError> {
        Ok(highlighted_suggestion)
    }
}

///expecting that number of parts divides the length of the input or default to 4
const EXPECTED_PARTS: [usize; 13] = [0, 1, 2, 3, 4, 3, 3, 4, 4, 3, 5, 4, 4];
async fn rename_labels(
    args: &Arguments,
    audacity: &mut AudacityApi,
    m_index: Option<&mut MultiIndex<'static>>,
) -> Result<(), Error> {
    let labels = audacity.get_label_info().await?;
    assert!(labels.len() == 1, "expecting one label track");

    let (series, index) = read_index_from_args(args, m_index).await?;
    let index = index.as_deref();
    let mut ac = index.as_ref().map(|index| {
        // TODO better filter
        ChapterCompleter::new(index, crate::args::autocompleter::StartsWithIgnoreCase {})
    });

    let labels = labels.into_values().next().unwrap();
    let mut expected_next_chapter_number: Option<ChapterNumber> = None;
    let mut i = 0;
    while i < labels.len() {
        const MSG: &str = "Welche Nummer hat die n\u{e4}chste Folge";
        let chapter_number = match ac.as_mut() {
            Some(index) => {
                let input = Inputs::input_with_suggestion(
                    format!("{MSG}: "),
                    expected_next_chapter_number
                        .map(|it| it.to_string())
                        .as_deref(),
                    index,
                );
                input
                    .split_once(' ')
                    .map_or(input.as_ref(), |it| it.0)
                    .parse::<ChapterNumber>()
                    .ok()
            }
            None => args.always_answer().try_input(
                &format!(
                    "{MSG}{}: ",
                    expected_next_chapter_number
                        .map_or_else(String::new, |next| format!(", erwarte {next}"))
                ),
                expected_next_chapter_number,
                |rin| rin.parse::<ChapterNumber>().ok(),
            ),
        }
        .expect("gib was vern\u{fc}nftiges ein");
        expected_next_chapter_number = Some(chapter_number.next());

        let chapter_name = index.map_or_else(
            || Cow::Owned(request_next_chapter_name(args)),
            |it| it.get(chapter_number).title,
        );

        let remaining = labels.len() - i;
        let expected_number = EXPECTED_PARTS
            .get(labels.len())
            .map_or(4, |i| *i)
            .min(remaining);
        let number = read_number(
            args.always_answer(),
            &format!("Wie viele Teile hat die n\u{e4}chste Folge, erwarte {expected_number}: "),
            Some(expected_number),
        )
        .min(remaining);
        for j in 0..number {
            let name = build_timelabel_name(
                series.as_str(),
                &chapter_number,
                j + 1,
                chapter_name.as_ref(),
            );
            let name = name.to_str().expect("only utf-8 support");
            audacity
                .set_label(i + j, Some(name), None, None, Some(false))
                .await?;
        }
        i += number;
    }
    Ok(())
}

pub async fn read_index_from_args<'a, 'b>(
    args: &Arguments,
    m_index: Option<&'b mut MultiIndex<'a>>,
) -> Result<(String, Option<common::boo::Boo<'b, Index<'a>>>), crate::worker::index::Error> {
    const MSG: &str = "Welche Serie ist heute dran: ";

    let series = m_index.as_ref().map_or_else(
        || args.always_answer().input(MSG, None),
        |m_index| {
            let known = m_index
                .get_possible()
                .into_iter()
                .map(|it| it.to_str().expect("only UTF-8").to_owned())
                .collect_vec();
            Inputs::input_with_suggestion(
                MSG,
                None,
                crate::args::autocompleter::VecCompleter::new(
                    known,
                    crate::args::autocompleter::StartsWithIgnoreCase {},
                ),
            )
        },
    );
    if let Some(series) = series.strip_prefix('#') {
        return Ok((series[1..].to_owned(), None));
    }
    let index = match m_index {
        Some(m_index) => {
            // SAFTY: path points to the path of m_index.
            // This is needed, because the mutable borrow of get_index makes it impossible to get a reference to Path, even if they will not interact.
            let path = unsafe { std::ptr::NonNull::from(m_index.path()).as_ref() };
            let map = m_index.get_index(OsString::from(series.as_str())).await;
            match map {
                Ok(x) => Some(common::boo::Boo::Borrowed(x)),
                Err(index::Error::SeriesNotFound) => {
                    todo!("couldn't find {series:?} in {:?} re-ask for series", path)
                }
                Err(index::Error::NoIndexFile) => todo!("ask for direct path"),
                Err(index::Error::NotSupportedFile(_)) => unreachable!(),
                Err(
                    index::Error::Parse(_, _) | index::Error::Serde(_) | index::Error::IO(_, _),
                ) => {
                    // SAFTY: we are in an error path of map, so map is always an error
                    return Err(unsafe { map.unwrap_err_unchecked() });
                }
            }
        }
        None => {
            let path = args
                .always_answer()
                .try_input(
                    "welche Index Datei m\u{f6}chtest du verwenden?: ",
                    Some(None),
                    |it| Some(Some(PathBuf::from(it))),
                )
                .unwrap_or_else(|| unreachable!());
            match path {
                Some(path) => crate::worker::index::Index::try_read_from_path(path)
                    .await
                    .map(common::boo::Boo::Owned)
                    .map(Some)
                    .or_else(|err| match err {
                        index::Error::SeriesNotFound => unreachable!(),
                        index::Error::NoIndexFile | index::Error::NotSupportedFile(_) => {
                            todo!("re-ask for path")
                        }
                        index::Error::Parse(_, _)
                        | index::Error::Serde(_)
                        | index::Error::IO(_, _) => Err(err),
                    })?,
                None => None,
            }
        }
    };
    Ok((series, index))
}

pub async fn adjust_labels(
    args: &Arguments,
    audacity: &mut AudacityApi,
) -> Result<(), audacity::Error> {
    let labels = audacity.get_label_info().await?; // get new labels

    for element in labels.values().flatten().open_border_pairs() {
        let (prev_end, next_start) = match element {
            State::Start(a) => (a.start, a.start + Duration::from_secs(10)),
            State::Middle(a, b) => (a.end, b.start),
            State::End(b) => (b.end, b.end + Duration::from_secs(10)),
        };
        audacity
            .zoom_to(
                audacity::Selection::Part {
                    start: prev_end - Duration::from_secs(10),
                    end: next_start + Duration::from_secs(10),
                    relative_to: audacity::RelativeTo::ProjectStart,
                },
                audacity::Save::Discard,
            )
            .await?;

        let _ = args.always_answer().input(
            "Dr\u{fc}ck Enter, wenn du bereit f\u{fc}r den n\u{e4}chsten Schritt bist",
            None,
        );
    }
    audacity
        .zoom_to(audacity::Selection::All, audacity::Save::Discard)
        .await
}

#[derive(Debug, Error)]
#[error("couldn't move file {file:?} to {dst:?}, with reason \"{source}\"")]
pub struct MoveError {
    file: PathBuf,
    dst: PathBuf,
    source: common::io::MoveError,
}
async fn move_results(
    patterns: impl Iterator<Item = &TaggedFile> + Send,
    from: impl AsRef<Path> + Send + Sync,
    to: impl AsRef<Path> + Send + Sync,
    args: &Arguments,
) -> Result<(), MoveError> {
    patterns
        .map(|tag| {
            let mut dst = to.as_ref().to_path_buf();
            let mut file = from.as_ref().to_path_buf();
            let name = build_timelabel_name::<&str, &str>(
                tag.get::<Album>(),
                &(tag.get::<Track>().unwrap() as usize).into(),
                None,
                tag.get::<Title>(),
            );
            if let Some(series) = tag.get::<Album>() {
                let (main, sub) = series
                    .split_once(MultiIndex::SUBSERIES_DELIMENITER)
                    .map_or_else(|| (series, None), |(main, sub)| (main, Some(sub)));
                dst.push(main);
                if let Some(sub) = sub {
                    dst.push(sub);
                }
            }
            file.push(name);
            file.set_extension(tag.ext());

            common::io::move_file(file, dst, args.dry_run())
                .map_err(move |(source, file, dst)| MoveError { file, dst, source })
        })
        .join_all()
        .await
        .into_iter()
        .collect::<Result<(), _>>()
}

async fn merge_parts<'a>(
    args: &Arguments,
    audacity: &mut audacity::AudacityApi,
    m_index: &mut MultiIndex<'a>,
    hint: audacity::TrackHint,
) -> Result<Vec<TaggedFile>, audacity::Error> {
    let label_track_nr = hint.get_label_track_nr(audacity).await?;
    let labels = audacity
        .get_label_info()
        .await?
        .remove(&label_track_nr)
        .unwrap_or_else(|| panic!("couldn't get track with number {label_track_nr}"));
    audacity.select_tracks(std::iter::once(1)).await?;
    audacity
        .write_assume_empty(audacity::command::RemoveTracks)
        .await?;
    let grouped_labels = labels.iter().into_group_map_by(|label| {
        let Some((series, nr,_, chapter)) = crate::archive::data::Archive::parse_line(label.name.as_ref().unwrap()) else {
            panic!("couldn't parse {:?}", label.name.as_ref().unwrap());
        };
        (series, nr, chapter)
    });
    let hint = audacity::TrackHint::TrackNr(audacity.add_label_track(Some("merged")).await?).into();
    for (group, labels) in grouped_labels.iter().filter(|(_, value)| value.len() > 1) {
        let mut name = format!("{} {}", group.0, group.1);
        if let Some(chapter) = group.2 {
            let _ = write!(name, " {chapter}");
        }
        let _ = audacity
            .add_label(
                audacity::data::TimeLabel::new(
                    labels.first().unwrap().start,
                    labels.last().unwrap().end,
                    Some(name),
                ),
                Some(hint),
            )
            .await?;
    }
    audacity
        .write_assume_empty(audacity::command::SelAllTracks)
        .await?;
    for (_group, labels) in grouped_labels
        .iter()
        .sorted_by(|(g_a, _), (g_b, _)| Ord::cmp(g_b, g_a))
    {
        for (b, a) in labels.iter().rev().tuple_windows() {
            audacity
                .select(audacity::Selection::Part {
                    start: a.end,
                    end: b.start,
                    relative_to: audacity::RelativeTo::ProjectStart,
                })
                .await?;

            audacity
                .write_assume_empty(audacity::command::Delete)
                .await?;
        }
    }
    let (keys, values) = grouped_labels.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();
    let offsets = keys
        .into_iter()
        .zip(calc_merged_offsets(values))
        .collect::<HashMap<_, _>>();
    let mut tags = Vec::new();
    for ((series, chapter_number, chapter_name), offsets) in offsets {
        let chapter_name = chapter_name.unwrap();
        let index = m_index.get_index(OsString::from(series)).await.unwrap();
        let entry = index.get(chapter_number);

        let mut path = args.tmp_path().to_path_buf();
        path.push(build_timelabel_name(
            series,
            &chapter_number,
            None,
            chapter_name,
        ));
        path.set_extension(args.export_ext());
        let tag = tags.push_return(TaggedFile::new_empty(path).unwrap());

        tag.set::<Title>(chapter_name);
        tag.set::<Album>(series.as_ref());
        tag.set::<Genre>(args.genre());
        tag.set::<Track>(chapter_number.nr as u32);
        tag.set::<TotalTracks>(index.main_len() as u32);
        if let Some(artist) = entry.artist {
            tag.set::<Artist>(artist.as_ref());
        }
        match entry.release {
            Some(
                index::DateOrYear::Year(year)
                | index::DateOrYear::Date(Datetime {
                    date: Some(Date { year, .. }),
                    ..
                }),
            ) => tag.set::<Year>(year as i32),
            Some(index::DateOrYear::Date(Datetime { date: None, .. })) => {
                log::warn!("release didn't have a date");
            }
            None => {}
        }
        if !offsets.is_empty() {
            // don't add only label at 0
            for (i, offset) in std::iter::once(Duration::ZERO)
                .chain(offsets.into_iter())
                .enumerate()
            {
                tag.set_chapter(i, offset, Some(&format!("Part {}", i + 1)));
            }
        }
    }

    Ok(tags)
}

fn calc_merged_offsets<'a, Iter>(grouped_labels: Iter) -> Vec<Vec<Duration>>
where
    Iter: IntoIterator,
    Iter::Item: IntoIterator<Item = &'a audacity::data::TimeLabel>,
{
    let mut deleted = Duration::ZERO;
    grouped_labels
        .into_iter()
        .map(|labels| {
            let mut iter = labels.into_iter().peekable();
            let first = iter.peek().expect("need at least one element");
            let point_zero = first.start - deleted;
            let mut last = first.start;
            let mut out = Vec::new();
            for (pos, label) in iter.with_position() {
                deleted += label.start - last;

                match pos {
                    Position::Last | Position::Only => {}
                    Position::First | Position::Middle => {
                        last = label.end;
                        out.push(label.end - point_zero - deleted);
                    }
                }
            }
            out
        })
        .collect_vec()
}

fn request_next_chapter_name(args: &Arguments) -> String {
    args.always_answer()
        .input("Wie hei\u{df}t die n\u{e4}chste Folge: ", None)
}

fn read_number(input: Inputs, msg: impl AsRef<str>, default: Option<usize>) -> usize {
    input
        .try_input(msg, default, |rin| rin.parse().ok())
        .expect("gib was vern\u{fc}nftiges ein")
}

#[cfg(test)]
mod tests {
    use audacity::data::TimeLabel;
    use common::extensions::duration::Ext;

    use super::*;

    #[test]
    fn calc_offsets() {
        let labels = [
            TimeLabel::new(
                Duration::from_h_m_s_m(0, 3, 25, 372),
                Duration::from_h_m_s_m(0, 24, 15, 860),
                None,
            ),
            TimeLabel::new(
                Duration::from_h_m_s_m(0, 24, 23, 90),
                Duration::from_h_m_s_m(0, 46, 37, 240),
                None,
            ),
            TimeLabel::new(
                Duration::from_h_m_s_m(0, 46, 43, 970),
                Duration::from_h_m_s_m(1, 6, 24, 170),
                None,
            ),
            TimeLabel::new(
                Duration::from_h_m_s_m(1, 6, 46, 170),
                Duration::from_h_m_s_m(1, 30, 32, 490),
                None,
            ),
            TimeLabel::new(
                Duration::from_h_m_s_m(1, 30, 39, 830),
                Duration::from_h_m_s_m(1, 55, 4, 930),
                None,
            ),
        ];
        let data = [
            vec![&labels[0], &labels[1], &labels[2]],
            vec![&labels[3], &labels[4]],
        ];
        assert_eq!(
            [
                vec![
                    Duration::from_h_m_s_m(0, 20, 50, 488),
                    Duration::from_h_m_s_m(0, 43, 4, 638)
                ],
                vec![Duration::from_h_m_s_m(0, 23, 46, 320)]
            ]
            .into_iter()
            .collect_vec(),
            calc_merged_offsets(data.into_iter())
        );
    }
}
