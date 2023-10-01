use audacity::AudacityApi;
use common::extensions::{
    iter::{CloneIteratorExt, FutIterExt, State},
    vec::PushReturn,
};
use futures::TryFutureExt;
use itertools::Itertools;
use log::trace;
use std::{
    borrow::Cow,
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

use self::{args::Arguments, index::Index};

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

    for audio_path in args.audio_paths() {
        let label_path = audio_path.with_extension("txt");
        let audacity_api = audacity_api.get_api_handle().await?;

        if !args.skip_load() {
            prepare_project(audacity_api, audio_path, &label_path).await?;
        }
        audacity_api
            .zoom_to(audacity::Selection::All, audacity::Save::Discard)
            .await?;
        let _ = args
            .always_answer()
            .input("press enter when you are ready to start renaming", None);

        let (patterns, mut tags, _) = rename_labels(args, audacity_api).await?;
        adjust_labels(args, audacity_api).await?;

        audacity_api
            .export_all_labels_to(label_path, args.dry_run())
            .await?;
        let offsets = merge_parts(audacity_api, audacity::TrackHint::LabelTrackNr(0)).await?;
        for (tag, offset) in tags.iter_mut().zip_eq(offsets) {
            if offset.is_empty() {
                continue; // don't add only label at 0
            }
            for (i, offset) in std::iter::once(Duration::ZERO)
                .chain(offset.into_iter())
                .enumerate()
            {
                tag.set_chapter(i, offset, Some(&format!("Part {}", i + 1)));
            }
        }
        let was_exported = if args.dry_run() {
            println!("asking to export audio and labels");
            for tag in tags {
                tag.drop_changes();
            }
            true
        } else {
            audacity_api
                .write_assume_empty(audacity::command::ExportMultiple)
                .await?;
            let all_exported = tags.iter().all(|tag| tag.path().exists());
            if all_exported {
                for mut tag in tags {
                    tag.reload_empty()
                        .map_err(|err| Error::Tag(tag.path().into(), err))?;
                    tag.save_changes(false)
                        .map_err(|err| Error::Tag(tag.path().into(), err))?;
                }
            } else {
                for tag in tags {
                    tag.drop_changes();
                }
            }
            all_exported
        };
        if was_exported {
            move_results(
                patterns,
                args.tmp_path(),
                args.index_folder().unwrap_or_else(|| args.tmp_path()),
                args,
            )
            .await?;
        } else {
            log::warn!("not all files exported, skipping move");
        }

        if !args.skip_load() {
            audacity_api
                .write_assume_empty(audacity::command::Close)
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

#[derive(Debug, Clone)]
pub struct ChapterCompleter<'a> {
    inner: std::sync::Arc<InnerCompleter<'a>>,
}
#[derive(Debug)]
struct InnerCompleter<'a> {
    index: Index<'a>,
    filter: Box<dyn crate::args::autocompleter::StrFilter + Send + Sync>,
}
impl<'a> ChapterCompleter<'a> {
    pub fn new(
        index: Index<'a>,
        filter: impl crate::args::autocompleter::StrFilter + Send + Sync + 'static,
    ) -> Self {
        Self::new_boxed(index, Box::new(filter))
    }
    #[must_use]
    pub fn new_boxed(
        index: Index<'a>,
        filter: Box<dyn crate::args::autocompleter::StrFilter + Send + Sync>,
    ) -> Self {
        Self {
            inner: std::sync::Arc::new(InnerCompleter { index, filter }),
        }
    }
    #[must_use]
    pub fn index(&self) -> &Index<'a> {
        &self.inner.index
    }
    fn filter(&self) -> &dyn crate::args::autocompleter::StrFilter {
        self.inner.filter.as_ref()
    }
}

impl<'a> inquire::Autocomplete for ChapterCompleter<'a> {
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
) -> Result<(Vec<Pattern>, Vec<TaggedFile>, Option<usize>), Error> {
    let labels = audacity.get_label_info().await?;
    assert!(labels.len() == 1, "expecting one label track");
    let labels = labels.into_values().next().unwrap();

    let (series, index) = read_index_from_args(args).await?;
    let index_rc = index.map(|index| {
        // TODO better filter
        ChapterCompleter::new(index, crate::args::autocompleter::StartsWithIgnoreCase {})
    });
    let index = index_rc.as_ref().map(ChapterCompleter::index);

    let index_len = index.map(index::Index::main_len);
    let nr_pad = index_len.map(|it| (it as f32).log10().ceil() as usize);

    let mut patterns = Vec::new();
    let mut tags = Vec::new();

    let mut expected_next_chapter_number: Option<ChapterNumber> = None;
    let mut i = 0;
    while i < labels.len() {
        const MSG: &str = "Welche Nummer hat die n\u{e4}chste Folge";
        let chapter_number = match index_rc.as_ref() {
            Some(index) => {
                let input = Inputs::input_with_suggestion(
                    format!("{MSG}: "),
                    expected_next_chapter_number
                        .map(|it| it.to_string())
                        .as_deref(),
                    index.clone(),
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

        let (chapter_name, artist, release) =
            if let Some(value) = index.as_ref().map(|it| it.get(chapter_number)) {
                (value.title, value.artist, value.release)
            } else {
                (Cow::Owned(request_next_chapter_name(args)), None, None)
            };

        let expected_number = Some(EXPECTED_PARTS.get(labels.len()).map_or(4, |i| *i));
        let number = read_number(
            args.always_answer(),
            &format!(
                "Wie viele Teile hat die n\u{e4}chste Folge{}: ",
                expected_number.map_or_else(String::new, |next| format!(", erwarte {next}"))
            ),
            expected_number,
        )
        .min(labels.len() - i);
        for j in 0..number {
            let name = build_timelabel_name(&series, &chapter_number, j + 1, &chapter_name);
            audacity
                .set_label(i + j, Some(name), None, None, Some(false))
                .await?;
        }
        i += number;

        let mut path = args.tmp_path().to_path_buf();
        path.push(format!(
            "{}.{}",
            build_timelabel_name(&series, &chapter_number, None, &chapter_name),
            args.export_ext()
        ));
        let tag = tags.push_return(TaggedFile::new_empty(path).unwrap());

        tag.set::<Title>(format!("{chapter_name}").as_ref());
        tag.set::<Album>(series.as_ref());
        tag.set::<Genre>(args.genre());
        tag.set::<Track>(chapter_number.nr as u32);
        if let Some(l) = index_len {
            tag.set::<TotalTracks>(l as u32);
        }
        if let Some(artist) = artist.as_deref() {
            tag.set::<Artist>(artist);
        }
        match release {
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
        patterns.push((series.clone(), chapter_number, chapter_name.into_owned()));
    }
    Ok((patterns, tags, nr_pad))
}

pub async fn read_index_from_args(
    args: &Arguments,
) -> Result<(String, Option<crate::worker::index::Index<'static>>), crate::worker::index::Error> {
    const MSG: &str = "Welche Serie ist heute dran: ";

    let series = args.index_folder().map_or_else(
        || args.always_answer().input(MSG, None),
        |path| {
            let known = index::Index::possible(path)
                .into_iter()
                .map(|it| it.to_str().expect("only UTF-8").to_owned())
                .collect_vec();
            Inputs::input_with_suggestion(
                MSG,
                None,
                crate::args::autocompleter::VecCompleter::new(
                    known,
                    std::rc::Rc::new(crate::args::autocompleter::StartsWithIgnoreCase {}),
                ),
            )
        },
    );
    if let Some(series) = series.strip_prefix('#') {
        return Ok((series[1..].to_owned(), None));
    }
    let index = match args.index_folder() {
        Some(folder) => crate::worker::index::Index::try_read_index(folder.to_owned(), &series)
            .await
            .map(Some)
            .or_else(|err| match err {
                index::Error::SeriesNotFound => {
                    todo!("couldn't find {series:?} in {folder:?} re-ask for series")
                }
                index::Error::NoIndexFile => todo!("ask for direct path"),
                index::Error::NotSupportedFile(_) => unreachable!(),
                index::Error::Parse(_, _) | index::Error::Serde(_) | index::Error::IO(_, _) => {
                    Err(err)
                }
            })?,
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
    patterns: Vec<Pattern>,
    from: impl AsRef<Path> + Send + Sync,
    to: impl AsRef<Path> + Send + Sync,
    args: &Arguments,
) -> Result<(), MoveError> {
    patterns
        .into_iter()
        .map(|(series, nr, chapter)| {
            let mut dst = to.as_ref().to_path_buf();
            dst.push(&series);

            let mut file = from.as_ref().to_path_buf();
            file.push(format!("{series} {nr} {chapter}.{}", args.export_ext()));
            common::io::move_file(file.clone(), dst.clone(), args.dry_run())
                .map_err(move |source| MoveError { file, dst, source })
        })
        .join_all()
        .await
        .into_iter()
        .collect::<Result<(), _>>()
}

async fn merge_parts(
    audacity: &mut audacity::AudacityApi,
    hint: audacity::TrackHint,
) -> Result<Vec<Vec<Duration>>, audacity::Error> {
    let label_track_nr = hint.get_label_track_nr(audacity).await?;
    let labels = audacity
        .get_label_info()
        .await?
        .remove(&label_track_nr)
        .unwrap();
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
    Ok(calc_merged_offsets(grouped_labels.values()))
}

fn calc_merged_offsets<'a>(
    grouped_labels: impl IntoIterator<Item = &'a Vec<&'a audacity::data::TimeLabel>>,
) -> Vec<Vec<Duration>> {
    let mut deleted = Duration::ZERO;
    grouped_labels
        .into_iter()
        .map(|labels| {
            let point_zero = labels[0].start - deleted;
            let mut last = labels[0].start;
            labels
                .iter()
                .map(|label| {
                    deleted += label.start - last;
                    last = label.end;
                    label.end - point_zero - deleted
                })
                .collect_vec()
                .into_iter()
                .dropping_back(1) // extra iter, so that last one will be calculated (to update deleted)
                .collect_vec()
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
            vec![
                vec![
                    Duration::from_h_m_s_m(0, 20, 50, 488),
                    Duration::from_h_m_s_m(0, 43, 4, 638)
                ],
                vec![Duration::from_h_m_s_m(0, 23, 46, 320)]
            ],
            calc_merged_offsets(data.iter())
        );
    }
}
