use audacity::AudacityApi;
use common::{
    args::input::autocompleter,
    extensions::{
        iter::{FutIterExt, IteratorExt},
        option::Ext,
        vec::PushReturn,
    },
};
use futures::TryFutureExt;
use itertools::{Itertools, Position};
use log::trace;
use regex::Regex;
use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::{OsStr, OsString},
    fmt::{Debug, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use thiserror::Error;

use toml::value::{Date, Datetime};

use crate::{
    archive::data::{build_timelabel_name, ChapterNumber},
    worker::tagger::{Album, Artist, Genre, TaggedFile, Title, TotalTracks, Track, Year},
};
use common::args::input::Inputs;

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
    Audacity(Box<dyn std::error::Error>),
    #[error("id3 Error {1} for {0:?}")]
    Tag(PathBuf, #[source] tagger::Error),
}
impl From<audacity::ConnectionError> for Error {
    fn from(value: audacity::ConnectionError) -> Self {
        Self::Audacity(value.into())
    }
}
impl From<audacity::ExportLabelError> for Error {
    fn from(value: audacity::ExportLabelError) -> Self {
        Self::Audacity(value.into())
    }
}
impl From<audacity::ImportLabelError> for Error {
    fn from(value: audacity::ImportLabelError) -> Self {
        Self::Audacity(value.into())
    }
}
impl From<audacity::ImportAudioError> for Error {
    fn from(value: audacity::ImportAudioError) -> Self {
        Self::Audacity(value.into())
    }
}

#[allow(dead_code)]
enum LoopControlFlow<B, R, Res> {
    Continue,
    Break(B),
    Return(R),
    Result(Res),
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
mod progress {
    use common::extensions::iter::IteratorExt;
    use itertools::Itertools;
    use std::path::PathBuf;
    use tokio::{
        fs,
        io::{AsyncBufReadExt, AsyncSeekExt, AsyncWriteExt},
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub enum State {
        Loaded,
        Named,
        Done,
    }
    impl<'a> TryFrom<&'a str> for State {
        type Error = &'a str;

        fn try_from(value: &'a str) -> Result<Self, Self::Error> {
            match value.to_ascii_lowercase().as_str() {
                "loaded" => Ok(Self::Loaded),
                "named" => Ok(Self::Named),
                "done" => Ok(Self::Done),
                _ => Err(value),
            }
        }
    }
    impl From<State> for &'static str {
        fn from(value: State) -> Self {
            match value {
                State::Loaded => "loaded",
                State::Named => "named",
                State::Done => "done",
            }
        }
    }

    #[derive(Debug)]
    pub struct Progress {
        file: PathBuf,
        content: Vec<(String, State)>,
        need_save: bool,
    }
    #[allow(dead_code)]
    impl Progress {
        pub async fn read(path: impl Into<PathBuf> + Send) -> Result<Self, std::io::Error> {
            let mut content = Vec::new();
            let path = path.into();
            let mut lines = tokio::io::BufReader::new(
                fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&path)
                    .await?,
            )
            .lines();
            let mut line_index = 0usize;
            while let Some(line) = lines.next_line().await? {
                match line
                    .rsplit_once(' ')
                    .map(|(path, state)| (path, State::try_from(state)))
                {
                    None => log::warn!("can't parse {line_index}:{line:?}, will ignore"),
                    Some((path, Err(state))) => {
                        log::warn!("unkown state for {line_index}:{path} {state:?}, will ignore");
                    }
                    Some((path, Ok(state))) => {
                        if let Some((pos, (old_path, _))) =
                            content.iter().find_position(|&(it, _)| path == it)
                        {
                            log::warn!(
                                "duplicate at {pos}:{old_path:?} {line_index}:{path:?}, forgetting old one"
                            );
                            content.remove(pos);
                        }
                        content.push((path.to_owned(), state));
                    }
                }
                line_index += 1;
            }

            Ok(Self {
                file: path,
                content,
                need_save: false,
            })
        }

        pub async fn delete(self) -> std::io::Result<()> {
            if fs::try_exists(&self.file).await? {
                log::debug!("deleting progress file");
                fs::remove_file(self.file).await?;
            }
            Ok(())
        }
        #[allow(clippy::unused_async)]
        pub async fn save(&self) -> std::io::Result<()> {
            // assumes no external change to the file
            if !self.need_save {
                return Ok(());
            }
            let mut file = tokio::io::BufWriter::new(
                fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&self.file)
                    .await?,
            );
            for (name, state) in &self.content {
                file.write_all(build_line(name, *state).as_bytes()).await?;
            }
            file.flush().await
        }
        pub async fn append(
            &mut self,
            name: impl AsRef<str> + Send,
            state: State,
        ) -> std::io::Result<()> {
            // encode if saving is needed anyway, else prepare a file to try to append to
            let mut file = if self.need_save {
                None
            } else {
                Some(
                    fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&self.file)
                        .await?,
                )
            };
            let content_len_without_last = self.content.len().saturating_sub(1);
            match self
                .content
                .iter_mut()
                .lzip(
                    (0..content_len_without_last)
                        .map(Some)
                        .chain(std::iter::once(None)),
                )
                .filter(|(_, (last_name, _))| last_name.as_str() == name.as_ref())
                .last()
            {
                Some((None, (_, old_state))) => {
                    *old_state = state;
                    if let Some(file) = file.as_mut() {
                        common::io::truncate_const_last_lines::<1>(file).await?;
                    }
                }
                Some((Some(pos), _)) => {
                    file = None; // can't just append, so full save is needet
                    self.content.remove(pos);
                    self.content.push((name.as_ref().to_owned(), state));
                }
                None => self.content.push((name.as_ref().to_owned(), state)),
            }

            if let Some(mut file) = file {
                file.seek(std::io::SeekFrom::End(0)).await?;
                file.write_all(build_line(name, state).as_bytes()).await?;
                file.flush().await
            } else {
                Ok(())
            }
        }
        pub async fn truncate_fixed<const LINES: usize>(&mut self) -> std::io::Result<()> {
            self.content.truncate(LINES);
            if self.need_save {
                Ok(()) // if save is already needed, no need to modify the file
            } else {
                common::io::truncate_const_last_lines::<LINES>(
                    &mut fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&self.file)
                        .await?,
                )
                .await
            }
        }
        pub async fn truncate(&mut self, lines: usize) -> std::io::Result<()> {
            self.content.truncate(lines);
            if !self.need_save {
                // only save if not safe already needed
                let mut file = fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&self.file)
                    .await?;
                common::io::truncate_last_lines(&mut file, lines).await?;
            }
            Ok(())
        }

        pub fn set(&mut self, name: String, state: State) {
            // todo try append
            if let Some(last) = self
                .content
                .iter_mut()
                .find(|(last_name, _)| last_name.as_str() == name.as_str())
                .map(|(_, state)| state)
            {
                *last = state;
            } else {
                self.content.push((name, state));
            }
            self.need_save = true;
        }
        pub fn remove(&mut self, name: impl AsRef<str>) -> Option<(String, State)> {
            self.content
                .iter()
                .find_position(|(last_name, _)| last_name.as_str() == name.as_ref())
                .map(|(pos, _)| pos) // extra map to end borrow of self.content
                .map(|pos| {
                    self.need_save = true;
                    self.content.remove(pos)
                })
        }
        pub fn get(&self, name: impl AsRef<str>) -> Option<State> {
            self.content
                .iter()
                .find(|(last_name, _)| last_name.as_str() == name.as_ref())
                .map(|(_, state)| *state)
        }
    }
    fn build_line(name: impl AsRef<str>, state: State) -> String {
        format!("{} {state:?}\n", name.as_ref())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn read() {
            let data = Progress::read(PathBuf::from("./res/progress.txt"))
                .await
                .unwrap();

            assert_eq!(
                vec![
                    ("element 1".to_owned(), State::Done),
                    ("element 2".to_owned(), State::Loaded),
                    ("element 3".to_owned(), State::Done),
                    ("element 4".to_owned(), State::Named)
                ],
                data.content
            );
        }
        #[tokio::test]
        async fn get() {
            let data = Progress::read(PathBuf::from("./res/progress.txt"))
                .await
                .unwrap();

            assert_eq!(Some(State::Done), data.get("element 1"));
            assert_eq!(Some(State::Loaded), data.get("element 2"));
            assert_eq!(Some(State::Done), data.get("element 3"));
            assert_eq!(Some(State::Named), data.get("element 4"));
            assert_eq!(None, data.get("element 5"));
        }
        #[tokio::test]
        async fn append() {
            let file = common::io::TmpFile::new_copy(
                PathBuf::from("./res/progress_append.txt"),
                "./res/progress.txt",
            )
            .unwrap();
            let mut data = Progress::read(file.as_ref()).await.unwrap();
            data.append("element 4", State::Done).await.unwrap();

            assert_eq!(
                Some(State::Done),
                data.get("element 4"),
                "failed to update internal data"
            );

            let data = Progress::read(file.as_ref()).await.unwrap();
            assert_eq!(
                Some(State::Done),
                data.get("element 4"),
                "failed to update file"
            );
        }
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
    let mut already_done = progress::Progress::read(args.tmp_path().join(".done.txt"))
        .await
        .unwrap();

    let re = Regex::new(r"\((d+)\)(.[a-zA-Z0-9]+)?$").unwrap();

    for (pos, audio_path) in args.audio_paths().iter().with_position() {
        let name = audio_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        if re.is_match(&name) {
            log::info!("skipping sub file");
            // TODO maybe run main file
            continue;
        }

        let label_path = audio_path.with_extension("txt");

        let audacity_api = audacity_api.get_api_handle().await?;
        let state = already_done.get(&name);

        if !args.skip_load() && state.is_none_or(|state| state < progress::State::Loaded) {
            prepare_project(audacity_api, audio_path, &label_path).await?;
            already_done
                .append(&name, progress::State::Loaded)
                .await
                .unwrap();
        } else {
            log::debug!("skipping load");
        }

        // start rename
        if !args.skip_name() && state.is_none_or(|state| state < progress::State::Named) {
            audacity_api
                .zoom_to(
                    audacity::data::Selection::All,
                    audacity::data::Save::Discard,
                )
                .await?;
            let _ = Inputs::read("press enter when you are ready to start renaming", None);

            if let Some(m_index) = m_index.as_mut() {
                // explicit binder, so Future is Send
                let mut binder = rename_labels::FancyNamer::new(audacity_api, m_index).await?;
                binder.rename().await?;
            } else {
                // TODO clean else path
                rename_labels::old(args, audacity_api).await?;
                rename_labels::adjust_labels(audacity_api).await?;
            }

            audacity_api
                .zoom_to(
                    audacity::data::Selection::All,
                    audacity::data::Save::Discard,
                )
                .await?;
            audacity_api
                .export_all_labels_to(label_path, args.dry_run())
                .await?;

            already_done
                .append(&name, progress::State::Named)
                .await
                .unwrap();
        } else {
            log::debug!("skipping naming");
        }
        if state.is_none_or(|state| state < progress::State::Done) {
            //start export
            let tags = merge_parts(
                args,
                audacity_api,
                m_index.as_mut().expect("need index"),
                audacity::data::TrackHint::LabelTrackNr(0),
            )
            .await?;
            let _ = Inputs::read(
                // "remove all lables you don't want to export and then press enter to start exporting",
                "remove all lables you don't want to remove, then press Ctrl+Shift+E to export and then press enter to continue",
                None,
            );
            // TODO find out how to fix "Ihr Stapelverarbeitungs-Befehl ExportAudio wurde nicht erkannt."
            // audacity_api
            //     .write_assume_empty(audacity::command::ExportAudio)
            //     .await?;

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

            already_done
                .append(name, progress::State::Done)
                .await
                .unwrap();
        } else {
            log::debug!("skipping export");
        }

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
    // download of progress done in external script
    Ok(())
}

async fn prepare_project(
    audacity: &mut AudacityApi,
    audio_path: impl AsRef<Path> + Send,
    label_path: impl AsRef<Path> + Send + Sync,
) -> Result<(), Error> {
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
    index: Box<dyn ChapterList + 'a + Send + Sync>,
    metric: Box<dyn common::str::filter::StrMetric + Send + Sync>,
}
impl<'a> ChapterCompleter<'a> {
    pub fn new(
        index: impl ChapterList + 'a + Send + Sync,
        metric: impl common::str::filter::StrMetric + Send + Sync + 'static,
    ) -> Self {
        Self::new_boxed(Box::new(index), Box::new(metric))
    }

    #[must_use]
    pub fn new_boxed(
        index: Box<dyn ChapterList + 'a + Send + Sync>,
        metric: Box<dyn common::str::filter::StrMetric + Send + Sync>,
    ) -> Self {
        Self { index, metric }
    }
    #[must_use]
    pub fn index(&self) -> &dyn ChapterList {
        self.index.as_ref()
    }
    fn metric(&self) -> &dyn common::str::filter::StrMetric {
        self.metric.as_ref()
    }
}

pub trait ChapterList: Debug {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn get(&self, nr: ChapterNumber) -> Option<Cow<'_, str>>;
    fn chapter_iter(&self) -> Box<(dyn Iterator<Item = (ChapterNumber, Cow<'_, str>)> + '_)>;
}

impl<'a> ChapterList for &'a Index<'a> {
    fn len(&self) -> usize {
        Index::main_len(self)
    }

    fn get(&self, nr: ChapterNumber) -> Option<Cow<'_, str>> {
        Index::try_get(self, nr).map(|it| it.title)
    }

    fn chapter_iter(&self) -> Box<(dyn Iterator<Item = (ChapterNumber, Cow<'_, str>)> + '_)> {
        Box::new(
            Index::chapter_iter(self)
                .lzip(1..)
                .map(|(i, entry)| (ChapterNumber::from(i), entry.title)),
        )
    }
}

impl<'a> autocompleter::Autocomplete for ChapterCompleter<'a> {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, autocompleter::Error> {
        Ok(match input.parse::<ChapterNumber>() {
            Ok(number) => {
                if number.is_maybe || number.is_partial {
                    // number ends  with '?' or '-', so nothing more will come
                    self.index()
                        .get(number)
                        .map_or_else(Vec::new, |it| vec![(number, it)])
                } else {
                    // find all possible numbers starting with current input
                    (0..self.index().len())
                        .filter(|&i| i.to_string().starts_with(&number.nr.to_string()))
                        .map(|i| {
                            let number = ChapterNumber::from(i);
                            (number, self.index().get(number).unwrap())
                        })
                        .collect_vec()
                }
            }
            Err(_) => common::str::filter::sort_with(
                self.metric(),
                self.index().chapter_iter(),
                input,
                |(_, it)| it,
            )
            .collect_vec(),
        }
        .into_iter()
        .map(|(i, chapter)| format!("{i} {chapter}"))
        .collect_vec())
    }

    fn get_completion(
        &mut self,
        _input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<autocompleter::Replacement, autocompleter::Error> {
        Ok(highlighted_suggestion)
    }
}

mod rename_labels {
    use itertools::Itertools;
    use std::{borrow::Cow, path::PathBuf, str::FromStr, time::Duration};

    use audacity::{data::TimeLabel, AudacityApi};
    use common::{
        args::input::{autocompleter, Inputs},
        extensions::iter::{CloneIteratorExt, State},
    };

    use super::{args::Arguments, ChapterCompleter, Error};
    use crate::{
        archive::data::{build_timelabel_name, ChapterNumber},
        worker::index::{Error as IdxError, Index, MultiIndex},
    };

    #[derive(Debug)]
    enum CompleterState {
        Command,
        Series(String),
        None,
    }
    #[derive(Debug)]
    pub struct FullNameCompleter<'r, 'i, Metric> {
        state: CompleterState,
        m_index: &'r mut MultiIndex<'i>,
        metric: Metric,
        command_prefix: &'static str,
    }
    impl<'i, 'r, Metric: common::str::filter::StrMetric> FullNameCompleter<'r, 'i, Metric> {
        #[must_use]
        pub fn new(m_index: &'r mut MultiIndex<'i>, metric: Metric) -> Self {
            Self {
                state: CompleterState::None,
                m_index,
                metric,
                command_prefix: "> ",
            }
        }
    }

    impl<'r, 'i, Metric: common::str::filter::StrMetric + Clone + Send + Sync + 'static>
        autocompleter::Autocomplete for FullNameCompleter<'r, 'i, Metric>
    {
        fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, autocompleter::Error> {
            if let Some(command) = input.strip_prefix(self.command_prefix) {
                self.state = CompleterState::Command;
                return Ok(common::str::filter::sort_with(
                    &self.metric,
                    Command::iter().map(<&'static str as From<_>>::from),
                    command,
                    |it| it,
                )
                .map(|it| format!("{}{}", self.command_prefix, it))
                .collect_vec());
            }

            match &self.state {
                CompleterState::Series(series) => {
                    if let Some(chapter_start) = input
                        .strip_prefix(series)
                        .and_then(|it| it.strip_prefix(' '))
                    {
                        return if let Some(index) = self.m_index.get_known_index(&series.into()) {
                            ChapterCompleter::new(index, self.metric.clone())
                                .get_suggestions(chapter_start)
                                .map(|res| {
                                    res.into_iter()
                                        .map(|it| format!("{series} {it}"))
                                        .collect_vec()
                                })
                        } else {
                            println!("couldn't find series, just let the user write stuff");
                            Ok(Vec::new())
                        };
                    }
                    self.state = CompleterState::None;
                }
                CompleterState::Command => self.state = CompleterState::None, // inputs startig with command prefix where already filtered
                CompleterState::None => {}
            };

            // only state = None remaining, ask for series
            let known = self
                .m_index
                .get_possible()
                .into_iter()
                .map(|it| it.to_str().expect("only UTF-8"));
            Ok(
                common::str::filter::sort_with(&self.metric, known, input, |it| it)
                    .map(std::borrow::ToOwned::to_owned)
                    .collect_vec(),
            )
        }

        fn get_completion(
            &mut self,
            _input: &str,
            highlighted_suggestion: Option<String>,
        ) -> Result<autocompleter::Replacement, autocompleter::Error> {
            Ok(match &self.state {
                CompleterState::Command | CompleterState::Series(_) => highlighted_suggestion,
                CompleterState::None => {
                    if let Some(series) = highlighted_suggestion.clone() {
                        self.state = CompleterState::Series(series);
                    }

                    highlighted_suggestion.map(|it| format!("{it} "))
                }
            })
        }
    }

    #[tokio::test]
    #[ignore = "user input test"]
    async fn full_ac_test() {
        let mut m_index =
            MultiIndex::new("/home/nilsj/Musik/newly ripped/Aufnahmen/current".into()).await;
        let ac = FullNameCompleter::new(&mut m_index, common::str::filter::Levenshtein::new(true));
        let res =
            common::args::input::Inputs::read_with_suggestion("gib ein Kapitel an:", None, ac);
        println!("{res:?} wurde gelesen");
    }

    ///expecting that number of parts divides the length of the input or default to 4
    const EXPECTED_PARTS: [usize; 13] = [0, 1, 2, 3, 4, 3, 3, 4, 4, 3, 5, 4, 4];
    const ASK_ALL_MSG: &str = "Was ist die n\u{e4}chste Folge:";
    const ASK_PARTS_MSG: &str = "Wie viele Teile hat die n\u{e4}chste Folge";
    const ASK_NUMBER_MSG: &str = "Welche Nummer hat die n\u{e4}chste Folge";
    const MSG: &str = "Welche Serie ist heute dran:";

    async fn get_labels(api: &mut AudacityApi) -> Result<Vec<TimeLabel>, Error> {
        let labels = api.get_label_info().await?;
        Ok::<_, Error>(
            labels
                .into_values()
                .exactly_one()
                .unwrap_or_else(|err| panic!("expecting one label track, but got {}", err.len())),
        )
    }
    fn request_next_chapter_name() -> String {
        Inputs::read("Wie hei\u{df}t die n\u{e4}chste Folge: ", None)
    }
    fn read_number(input: Inputs, msg: impl AsRef<str>, default: Option<usize>) -> usize {
        input
            .try_read(msg, default, |rin| rin.parse().ok())
            .expect("gib was vern\u{fc}nftiges ein")
    }

    async fn read_index_from_args<'i>(
        args: &Arguments,
    ) -> Result<(String, Option<Index<'i>>), IdxError> {
        let series = Inputs::read(MSG, None);
        if let Some(value) = series
            .strip_prefix('#')
            .map(|series| series[1..].to_owned())
        {
            return Ok((value, None));
        }

        let index = loop {
            let path = args
                .always_answer()
                .try_read(
                    "welche Index Datei m\u{f6}chtest du verwenden?: ",
                    Some(None),
                    |it| Some(Some(PathBuf::from(it))),
                )
                .unwrap_or_else(|| unreachable!());
            match path {
                None => break None,
                Some(path) => {
                    let map = crate::worker::index::Index::try_read_from_path(path).await;
                    match map {
                        Ok(index) => break Some(index),
                        Err(IdxError::SeriesNotFound) => unreachable!(),
                        Err(IdxError::NoIndexFile | IdxError::NotSupportedFile(_)) => {
                            println!("couldn't find requested index, try again");
                            continue;
                        }
                        Err(IdxError::Parse(_, _) | IdxError::Serde(_) | IdxError::IO(_, _)) => {
                            // SAFETY: we are in the error case
                            Err(unsafe { map.unwrap_err_unchecked() })?;
                        }
                    }
                }
            };
        };
        Ok((series, index))
    }

    pub async fn old(args: &Arguments, api: &mut audacity::AudacityApi) -> Result<(), Error> {
        let labels = get_labels(api).await?;
        let (series, index) = read_index_from_args(args).await?;
        let index = index.as_ref();
        let mut ac = index.as_ref().map(|&index| {
            ChapterCompleter::new(index, common::str::filter::Levenshtein::new(true))
        });

        let mut expected_next_chapter_number: Option<ChapterNumber> = None;
        let mut i = 0;
        while i < labels.len() {
            let chapter_number = match ac.as_mut() {
                Some(index) => {
                    let input = Inputs::read_with_suggestion(
                        format!("{ASK_NUMBER_MSG}:"),
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
                None => args.always_answer().try_read(
                    &format!(
                        "{ASK_NUMBER_MSG}{}: ",
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
                || Cow::Owned(request_next_chapter_name()),
                |it| it.get(chapter_number).title,
            );

            let remaining = labels.len() - i;
            let expected_number = EXPECTED_PARTS
                .get(labels.len())
                .map_or(4, |i| *i)
                .min(remaining);
            let number = read_number(
                args.always_answer(),
                &format!("{ASK_PARTS_MSG}, erwarte {expected_number}: "),
                Some(expected_number),
            )
            .min(remaining);
            for j in 0..number {
                let name = build_timelabel_name::<str, _, _>(
                    &series,
                    &chapter_number,
                    j + 1,
                    chapter_name.as_ref(),
                );
                api.set_label(i + j, Some(name), None, None, Some(false))
                    .await?;
            }
            i += number;
        }
        Ok(())
    }

    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    enum Command {
        ReloadIndex,
        ReloadLabel,
        Restart,
        Join,
    }
    impl FromStr for Command {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "reload_label" => Ok(Self::ReloadLabel),
                "reload_index" => Ok(Self::ReloadIndex),
                "resize" => Ok(Self::Restart),
                "join" => Ok(Self::Join),
                _ => Err(s.to_owned()),
            }
        }
    }
    impl From<Command> for &'static str {
        fn from(value: Command) -> Self {
            match value {
                Command::ReloadLabel => "reload_label",
                Command::ReloadIndex => "reload_index",
                Command::Restart => "resize",
                Command::Join => "join",
            }
        }
    }
    impl Command {
        fn iter() -> std::array::IntoIter<Self, 4> {
            [
                Self::ReloadIndex,
                Self::ReloadLabel,
                Self::Restart,
                Self::Join,
            ]
            .into_iter()
        }
    }

    pub struct FancyNamer<'a, 'r, 'i> {
        api: &'a mut AudacityApi,
        m_index: &'r mut MultiIndex<'i>,
        labels: Vec<TimeLabel>,
        last_read: Option<(String, ChapterNumber, usize, String)>,
        i: usize,
    }
    impl<'a, 'r, 'i> FancyNamer<'a, 'r, 'i> {
        pub async fn new(
            api: &'a mut AudacityApi,
            m_index: &'r mut MultiIndex<'i>,
        ) -> Result<Self, Error> {
            let labels = get_labels(api).await?;
            Ok(Self {
                api,
                m_index,
                labels,
                last_read: None,
                i: 0,
            })
        }

        pub async fn rename(&mut self) -> Result<(), Error> {
            while self.i < self.labels.len() {
                zoom_to_label(
                    self.api,
                    self.labels.iter().open_border_pairs().nth(self.i).unwrap(),
                )
                .await?;
                let (series, chapter_number, chapter_name, part) = loop {
                    let initial = self.last_read.as_ref().map(|(series, nr, _, chapter)| {
                        if self.m_index.has_index(&series.into()) {
                            format!("{series} {nr}")
                        } else {
                            format!("{series} {nr} {chapter}") // keep chapter when no index is found
                        }
                    });
                    let mut ac = FullNameCompleter::new(
                        self.m_index,
                        common::str::filter::Levenshtein::new(true),
                    );
                    if let Some((series, _, _, _)) = self.last_read.as_ref() {
                        ac.state = CompleterState::Series(series.clone());
                    }
                    let command_prefix = ac.command_prefix;
                    let res = common::args::input::Inputs::read_with_suggestion(
                        ASK_ALL_MSG,
                        initial.as_deref(),
                        ac,
                    );

                    match res.strip_prefix(command_prefix).map(str::parse) {
                        Some(Ok(command)) => {
                            self.run_command(command).await?;
                            continue;
                        }
                        Some(Err(command)) => {
                            println!("unkown command {command:?}");
                            continue;
                        }
                        None => {}
                    }
                    if let Some((series, nr, _, chapter)) =
                        crate::archive::data::Archive::parse_line(&res)
                    {
                        let chapter = match chapter {
                            Some(chapter) => chapter.to_owned(),
                            None => match self.m_index.get_index(series.into()).await {
                                Ok(index) => index.get(nr).title.into_owned(),
                                Err(_) => request_next_chapter_name(),
                            },
                        };
                        let part = self
                            .last_read
                            .as_ref()
                            .filter(|(last_series, last_nr, _, _)| {
                                last_series == series && *last_nr == nr
                            })
                            .map_or(1, |(_, _, last_part, _)| last_part + 1);
                        self.last_read = Some((series.to_owned(), nr, part, chapter.clone()));
                        break (series.to_owned(), nr, chapter, part);
                    }
                    println!("konnte {res} nicht erkennen");
                };

                let name =
                    build_timelabel_name::<str, _, _>(series, &chapter_number, part, &chapter_name);
                self.api
                    .set_label(self.i, Some(name), None, None, Some(false))
                    .await?;
                self.i += 1;
            }
            zoom_to_label(
                self.api,
                self.labels.iter().open_border_pairs().last().unwrap(),
            )
            .await?;
            let _ = Inputs::read(
                "Dr\u{fc}ck Enter, wenn du bereit f\u{fc}r den n\u{e4}chsten Schritt bist",
                None,
            );
            Ok(())
        }

        async fn run_command(&mut self, command: Command) -> Result<(), Error> {
            match command {
                Command::ReloadIndex => {
                    self.m_index.reload().await;
                }
                Command::ReloadLabel => {
                    let old_i = self.labels.remove(self.i);
                    self.labels = get_labels(self.api).await?;

                    if self.labels.get(self.i).is_some_and(|label| *label != old_i) {
                        if let Some((i, _)) = self.labels.iter().find_position(|&it| *it == old_i) {
                            self.i = i;
                        }
                    }
                }
                Command::Restart => {
                    self.i = 0;
                    self.last_read = None;
                    self.labels = get_labels(self.api).await?;
                }
                Command::Join => {
                    if self.i == 0 {
                        log::warn!("can't join first");
                        return Ok(());
                    }
                    // assumes no overlapping labels in this track so no reload of labels is needed
                    let label_delete = self.labels.remove(self.i);
                    self.api
                        .select(audacity::data::Selection::Part {
                            start: *label_delete.start(),
                            end: *label_delete.end(),
                            relative_to: audacity::data::RelativeTo::ProjectStart,
                        })
                        .await?;
                    self.api.select_tracks(std::iter::once(1)).await?;
                    self.api
                        .write_assume_empty(audacity::command::SplitDelete)
                        .await?;
                    self.api
                        .set_label(
                            self.i - 1,
                            None::<&str>,
                            None,
                            Some(*label_delete.end()),
                            None,
                        )
                        .await?;
                }
            }
            Ok(())
        }
    }

    pub async fn adjust_labels(
        audacity: &mut AudacityApi,
    ) -> Result<(), audacity::ConnectionError> {
        let labels = audacity.get_label_info().await?; // get new labels

        for element in labels.values().flatten().open_border_pairs() {
            zoom_to_label(audacity, element).await?;

            let _ = Inputs::read(
                "Dr\u{fc}ck Enter, wenn du bereit f\u{fc}r den n\u{e4}chsten Schritt bist",
                None,
            );
        }
        Ok(())
    }
    async fn zoom_to_label(
        api: &mut audacity::AudacityApi,
        label: State<&TimeLabel>,
    ) -> Result<(), audacity::ConnectionError> {
        let (prev_end, next_start) = match label {
            State::Start(a) => (a.start(), *a.start() + Duration::from_secs(10)),
            State::Middle(a, b) => (a.end(), *b.start()),
            State::End(b) => (b.end(), *b.end() + Duration::from_secs(10)),
        };
        api.zoom_to(
            audacity::data::Selection::Part {
                start: *prev_end - Duration::from_secs(10),
                end: next_start + Duration::from_secs(10),
                relative_to: audacity::data::RelativeTo::ProjectStart,
            },
            audacity::data::Save::Discard,
        )
        .await
    }
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
            let name = build_timelabel_name::<OsStr, &str, &str>(
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
    hint: audacity::data::TrackHint,
) -> Result<Vec<TaggedFile>, audacity::ConnectionError> {
    let label_track_nr = hint
        .get_label_track_nr(audacity)
        .await?
        .expect("no track found");
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
        let Some((series, nr, _, chapter)) =
            crate::archive::data::Archive::parse_line(label.name().unwrap())
        else {
            panic!("couldn't parse {:?}", label.name().unwrap());
        };
        (series, nr, chapter)
    });
    let hint =
        audacity::data::TrackHint::TrackNr(audacity.add_label_track(Some("merged")).await?).into();
    for (group, labels) in grouped_labels.iter().filter(|(_, value)| value.len() > 1) {
        let mut name = format!("{} {}", group.0, group.1);
        if let Some(chapter) = group.2 {
            let _ = write!(name, " {chapter}");
        }
        let _ = audacity
            .add_label(
                audacity::data::TimeLabel::new::<String>(
                    *labels.first().unwrap().start(),
                    *labels.last().unwrap().end(),
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
                .select(audacity::data::Selection::Part {
                    start: *a.end(),
                    end: *b.start(),
                    relative_to: audacity::data::RelativeTo::ProjectStart,
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

        let mut path = args.tmp_path().to_path_buf();
        path.push(build_timelabel_name::<OsStr, _, _>(
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

        if let Ok(index) = m_index.get_index(OsString::from(series)).await {
            let entry = index.get(chapter_number);

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
        }
        if !offsets.is_empty() {
            // don't add only label at 0
            for (i, offset) in std::iter::once(Duration::ZERO)
                .chain(offsets.into_iter())
                .lzip(1..)
            {
                tag.set_chapter(i, offset, Some(&format!("Part {i}")));
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
            let point_zero = *first.start() - deleted;
            let mut last = first.start();
            let mut out = Vec::new();
            for (pos, label) in iter.with_position() {
                deleted += *label.start() - *last;

                match pos {
                    Position::Last | Position::Only => {}
                    Position::First | Position::Middle => {
                        last = label.end();
                        out.push(*label.end() - point_zero - deleted);
                    }
                }
            }
            out
        })
        .collect_vec()
}

#[cfg(test)]
mod tests {
    use audacity::data::TimeLabel;
    use common::extensions::duration::duration_from_h_m_s_m;

    use super::*;

    #[test]
    fn calc_offsets() {
        let labels = [
            TimeLabel::new::<&str>(
                duration_from_h_m_s_m(0, 3, 25, 372),
                duration_from_h_m_s_m(0, 24, 15, 860),
                None,
            ),
            TimeLabel::new::<&str>(
                duration_from_h_m_s_m(0, 24, 23, 90),
                duration_from_h_m_s_m(0, 46, 37, 240),
                None,
            ),
            TimeLabel::new::<&str>(
                duration_from_h_m_s_m(0, 46, 43, 970),
                duration_from_h_m_s_m(1, 6, 24, 170),
                None,
            ),
            TimeLabel::new::<&str>(
                duration_from_h_m_s_m(1, 6, 46, 170),
                duration_from_h_m_s_m(1, 30, 32, 490),
                None,
            ),
            TimeLabel::new::<&str>(
                duration_from_h_m_s_m(1, 30, 39, 830),
                duration_from_h_m_s_m(1, 55, 4, 930),
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
                    duration_from_h_m_s_m(0, 20, 50, 488),
                    duration_from_h_m_s_m(0, 43, 4, 638)
                ],
                vec![duration_from_h_m_s_m(0, 23, 46, 320)]
            ]
            .into_iter()
            .collect_vec(),
            calc_merged_offsets(data.into_iter())
        );
    }

    #[ignore = "needs user input"]
    #[tokio::test]
    async fn test_chapter_completer() {
        let metric = common::str::filter::Levenshtein::new(true);
        let binding = Index::try_read_from_path(
            "/home/nilsj/Musik/newly ripped/Aufnahmen/current/Gruselkabinett/index.toml",
        )
        .await
        .unwrap();

        println!(
            "read {:?}",
            Inputs::read_with_suggestion("$>", None, ChapterCompleter::new(&binding, metric))
        );
    }
}
