use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use thiserror::Error;

mod field_kind {
    use std::time::Duration;

    pub enum Set<'a> {
        Title(Option<&'a str>),
        Artist(Option<&'a str>),
        Album(Option<&'a str>),
        Genre(Option<&'a str>),
        Year(Option<i32>),
        Track(Option<u32>),
        TotalTracks(Option<u32>),
        Disk(Option<u32>),
        TotalDisks(Option<u32>),
        Length(Option<Duration>),
    }

    pub enum Get {
        Title,
        Artist,
        Album,
        Genre,
        Year,
        Track,
        TotalTracks,
        Disk,
        TotalDisks,
        Length,
    }
}

pub trait Field {
    type Type<'a>: FieldValue<'a>
    where
        Self: 'a;
    const KIND: field_kind::Get;

    fn wrap_value(value: Option<Self::Type<'_>>) -> field_kind::Set<'_>;
}
macro_rules! field {
    ($name: ident, str) => {
        field!($name, &'a str);
    };
    ($name: ident, $ref:ty) => {
        pub struct $name;
        impl Field for $name {
            type Type<'a> = $ref where Self: 'a;
            const KIND: field_kind::Get = field_kind::Get::$name;

            fn wrap_value(value: Option<Self::Type<'_>>) -> field_kind::Set<'_> {
                field_kind::Set::$name(value)
            }
        }
    };
}

field!(Title, str);
field!(Artist, str);
field!(Album, str);
field!(Genre, str);

field!(Year, i32);

field!(Track, u32);
field!(TotalTracks, u32);
field!(Disk, u32);
field!(TotalDisks, u32);

field!(Length, Duration);

pub trait FieldValue<'a>: Sized {
    #[must_use]
    fn from_str(_value: &'a str) -> Option<Self> {
        None
    }
    #[must_use]
    fn from_duration(_value: Duration) -> Option<Self> {
        None
    }
    #[must_use]
    fn from_u32(_value: u32) -> Option<Self> {
        None
    }
    #[must_use]
    fn from_i32(_value: i32) -> Option<Self> {
        None
    }
}
impl<'a> FieldValue<'a> for &'a str {
    fn from_str(value: &'a str) -> Option<Self> {
        Some(value)
    }
}
impl FieldValue<'_> for Duration {
    fn from_duration(value: Duration) -> Option<Self> {
        Some(value)
    }
}
impl FieldValue<'_> for u32 {
    fn from_u32(value: u32) -> Option<Self> {
        Some(value)
    }
}
impl FieldValue<'_> for i32 {
    fn from_i32(value: i32) -> Option<Self> {
        Some(value)
    }
}

pub trait Tag {
    fn title(&self) -> Option<&str>;
    fn artist(&self) -> Option<&str>;
    fn album(&self) -> Option<&str>;
    fn genre(&self) -> Option<&str>;
    fn year(&self) -> Option<i32>;
    fn track(&self) -> Option<u32>;
    fn total_tracks(&self) -> Option<u32>;
    fn disk(&self) -> Option<u32>;
    fn total_disks(&self) -> Option<u32>;
    fn duration(&self) -> Option<Duration>;

    fn set(&mut self, value: field_kind::Set);

    fn write_to_path(&self, path: &Path) -> Result<(), Error>;

    fn read_from_path(path: impl AsRef<Path>) -> Result<Self, Error>
    where
        Self: Sized;
    fn new_empty() -> Self
    where
        Self: Sized;
}

mod mp3 {
    use super::{field_kind, Duration, Error, Path, Tag};

    impl Tag for id3::Tag {
        fn title(&self) -> Option<&str> {
            id3::TagLike::title(self)
        }
        fn artist(&self) -> Option<&str> {
            id3::TagLike::artist(self)
        }
        fn album(&self) -> Option<&str> {
            id3::TagLike::album(self)
        }
        fn genre(&self) -> Option<&str> {
            id3::TagLike::genre(self)
        }
        fn year(&self) -> Option<i32> {
            id3::TagLike::year(self)
        }
        fn track(&self) -> Option<u32> {
            id3::TagLike::track(self)
        }
        fn total_tracks(&self) -> Option<u32> {
            id3::TagLike::total_tracks(self)
        }
        fn disk(&self) -> Option<u32> {
            id3::TagLike::disc(self)
        }
        fn total_disks(&self) -> Option<u32> {
            id3::TagLike::total_discs(self)
        }
        fn duration(&self) -> Option<Duration> {
            id3::TagLike::duration(self).map(|it| Duration::from_secs(it as u64))
        }

        fn set(&mut self, value: field_kind::Set) {
            use field_kind::Set as Kind;
            use id3::TagLike;
            match value {
                Kind::Title(Some(value)) => self.set_title(value),
                Kind::Artist(Some(value)) => self.set_artist(value),
                Kind::Album(Some(value)) => self.set_album(value),
                Kind::Genre(Some(value)) => self.set_genre(value),
                Kind::Year(Some(value)) => self.set_year(value),
                Kind::Track(Some(value)) => self.set_track(value),
                Kind::TotalTracks(Some(value)) => self.set_total_tracks(value),
                Kind::Disk(Some(value)) => self.set_disc(value),
                Kind::TotalDisks(Some(value)) => self.set_total_discs(value),
                Kind::Length(Some(value)) => self.set_duration(value.as_secs() as u32),

                Kind::Title(None) => self.remove_title(),
                Kind::Artist(None) => self.remove_artist(),
                Kind::Album(None) => self.remove_album(),
                Kind::Genre(None) => self.remove_genre(),
                Kind::Year(None) => self.remove_year(),
                Kind::Track(None) => self.remove_track(),
                Kind::TotalTracks(None) => self.remove_total_tracks(),
                Kind::Disk(None) => self.remove_disc(),
                Kind::TotalDisks(None) => self.remove_total_discs(),
                Kind::Length(None) => self.remove_duration(),
            }
        }

        fn write_to_path(&self, path: &Path) -> Result<(), Error> {
            self.write_to_path(path, self.version()).map_err(map_err)
        }
        fn read_from_path(path: impl AsRef<Path>) -> Result<Self, Error>
        where
            Self: Sized,
        {
            Self::read_from_path(path).map_err(map_err)
        }
        fn new_empty() -> Self
        where
            Self: Sized,
        {
            Self::new()
        }
    }

    fn map_err(err: id3::Error) -> Error {
        match err.kind {
            id3::ErrorKind::NoTag => Error::NoTag,
            _ => Error::Other(Box::new(err)),
        }
    }
}

mod opus {
    use opus_tag::opus_tagger::{Comment, VorbisComment};

    use super::{field_kind, Duration, Error, Path, Tag};

    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub enum VorbisKeys {
        Title,
        Artist,
        Album,
        Genre,
        DiskNumber,
        TrackNumber,
        TotalDiskNumber,
        TotalTrackNumber,
        Year,
        Duration,
    }

    impl VorbisKeys {
        /// [spec](https://picard-docs.musicbrainz.org/downloads/MusicBrainz_Picard_Tag_Map.html)
        /// "author" for artits is used by audacity, when converting from mp3
        pub(crate) const fn get_keys(self) -> &'static [&'static str] {
            match self {
                Self::Title => &["TITLE"],
                Self::Artist => &["ARTIST", "AUTHOR"],
                Self::Album => &["ALBUM"],
                Self::Genre => &["GENRE"],
                Self::DiskNumber => &["DISKNUMBER"],
                Self::TrackNumber => &["TRACKNUMBER"],
                Self::Year => &["YEAR"],
                Self::TotalDiskNumber => &["TOTALDISCS", "DISCTOTAL"],
                Self::TotalTrackNumber => &["TOTALTRACKS", "TRACKTOTAL"],
                Self::Duration => &["DURATIONHINT", "DURATION"],
            }
        }

        pub(crate) fn get_first(self, tag: &VorbisComment) -> Option<&'_ str> {
            let comments = self
                .get_all(tag)
                .map(|Comment { key: _, value }| value.as_str())
                .collect::<Vec<_>>();
            if comments.len() >= 2 {
                log::warn!("more than one comment for {self:?} found: {comments:?}");
            }
            comments.first().copied()
        }
        pub(crate) fn get_first_map<'a, T>(
            self,
            tag: &'a VorbisComment,
            map: impl Fn(&'a str) -> Option<T>,
        ) -> Option<T> {
            let value = self.get_first(tag)?;
            let value = map(value);
            if value.is_some() {
                return value;
            }
            // TODO remove invalid key
            // self.remove();
            None
        }
        pub(crate) fn get_all(self, tag: &VorbisComment) -> impl Iterator<Item = &'_ Comment> {
            let keys = self.get_keys();
            keys.iter().flat_map(|key| tag.find_comments(key))
        }

        pub(crate) fn set_first(self, tag: &mut VorbisComment, value: &impl ToString) {
            let comments = self.get_all(tag).collect::<Vec<_>>();
            let keys = self.get_keys();
            match comments.as_slice() {
                [] => {}
                [_] => {
                    log::warn!("one comment for {self:?} found: {comments:?}, will overwrite");
                    for key in keys {
                        if tag.remove_first(key).is_some() {
                            break;
                        }
                    }
                }
                [..] => {
                    log::warn!(
                        "more than one comment for {self:?} found: {comments:?}, will append"
                    );
                    todo!("handle better")
                }
            }
            tag.add_comment((keys[0], value.to_string()));
        }

        pub(crate) fn remove_all(self, tag: &mut VorbisComment) {
            let keys = self.get_keys();
            for key in keys {
                tag.remove_all(key);
            }
        }
    }

    impl Tag for VorbisComment {
        fn title(&self) -> Option<&str> {
            VorbisKeys::Title.get_first(self)
        }

        fn artist(&self) -> Option<&str> {
            VorbisKeys::Artist.get_first(self)
        }

        fn album(&self) -> Option<&str> {
            VorbisKeys::Album.get_first(self)
        }

        fn genre(&self) -> Option<&str> {
            VorbisKeys::Genre.get_first(self)
        }

        fn year(&self) -> Option<i32> {
            VorbisKeys::Year.get_first_map(self, |it| it.parse().ok())
        }

        fn track(&self) -> Option<u32> {
            VorbisKeys::TrackNumber.get_first_map(self, |it| {
                it.split('/').next().and_then(|it| it.parse().ok())
            })
        }

        fn total_tracks(&self) -> Option<u32> {
            VorbisKeys::TotalTrackNumber
                .get_first_map(self, |it| it.parse().ok())
                .or_else(|| {
                    VorbisKeys::TrackNumber.get_first_map(self, |it| {
                        it.split('/').nth(1).and_then(|it| it.parse().ok())
                    })
                })
        }

        fn disk(&self) -> Option<u32> {
            VorbisKeys::DiskNumber.get_first_map(self, |it| it.parse().ok())
        }

        fn total_disks(&self) -> Option<u32> {
            VorbisKeys::TotalDiskNumber.get_first_map(self, |it| it.parse().ok())
        }

        fn duration(&self) -> Option<Duration> {
            VorbisKeys::Duration.get_first_map(self, |it| it.parse().ok().map(Duration::from_secs))
        }

        fn set(&mut self, value: field_kind::Set) {
            use field_kind::Set as Kind;
            use VorbisKeys as Key;
            match value {
                Kind::Title(Some(value)) => Key::Title.set_first(self, &value),
                Kind::Artist(Some(value)) => Key::Artist.set_first(self, &value),
                Kind::Album(Some(value)) => Key::Album.set_first(self, &value),
                Kind::Genre(Some(value)) => Key::Genre.set_first(self, &value),
                Kind::Year(Some(value)) => Key::Year.set_first(self, &value),
                Kind::Track(Some(value)) => Key::TrackNumber.set_first(self, &value),
                Kind::TotalTracks(Some(value)) => Key::TotalTrackNumber.set_first(self, &value),
                Kind::Disk(Some(value)) => Key::DiskNumber.set_first(self, &value),
                Kind::TotalDisks(Some(value)) => Key::TotalDiskNumber.set_first(self, &value),
                Kind::Length(Some(value)) => Key::Duration.set_first(self, &value.as_secs()),

                Kind::Title(None) => Key::Title.remove_all(self),
                Kind::Artist(None) => Key::Artist.remove_all(self),
                Kind::Album(None) => Key::Album.remove_all(self),
                Kind::Genre(None) => Key::Genre.remove_all(self),
                Kind::Year(None) => Key::Year.remove_all(self),
                Kind::Track(None) => Key::TrackNumber.remove_all(self),
                Kind::TotalTracks(None) => Key::TotalTrackNumber.remove_all(self),
                Kind::Disk(None) => Key::DiskNumber.remove_all(self),
                Kind::TotalDisks(None) => Key::TotalDiskNumber.remove_all(self),
                Kind::Length(None) => Key::Duration.remove_all(self),
            }
        }

        fn write_to_path(&self, path: &Path) -> Result<(), Error> {
            self.write_opus_file(path).map_err(map_err)
        }
        fn read_from_path(path: impl AsRef<Path>) -> Result<Self, Error>
        where
            Self: Sized,
        {
            opus_tag::opus_tagger::OpusMeta::read_from_file(path)
                .map(|it| it.tags)
                .map_err(map_err)
        }
        fn new_empty() -> Self
        where
            Self: Sized,
        {
            Self::empty("Lavf60.3.100") // vendor should be read from the file
        }
    }
    fn map_err(err: opus_tag::error::Error) -> Error {
        Error::Other(Box::new(err))
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("extention {0:?} not supportet")]
    UnSupported(Option<String>),
    #[error("file hat no Tag info")]
    NoTag,
    #[error(transparent)]
    Other(Box<dyn std::error::Error>),
}
impl From<Option<&str>> for Error {
    fn from(value: Option<&str>) -> Self {
        Self::UnSupported(value.map(ToOwned::to_owned))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Supportet {
    Mp3,
    Opus,
}
impl TryFrom<&Path> for Supportet {
    type Error = Error;
    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        match value.extension().and_then(std::ffi::OsStr::to_str) {
            Some("mp3") => Ok(Self::Mp3),
            Some("opus") => Ok(Self::Opus),
            x => Err(x.into()),
        }
    }
}
impl Supportet {
    fn new_empty(self) -> Box<dyn Tag + Send> {
        match self {
            Self::Mp3 => Box::new(id3::Tag::new_empty()),
            Self::Opus => Box::new(opus_tag::opus_tagger::VorbisComment::new_empty()),
        }
    }
    fn read_boxed(self, path: &Path) -> Result<Box<dyn Tag + Send>, Error> {
        Ok(match self {
            Self::Mp3 => Box::new(<id3::Tag as Tag>::read_from_path(path)?),
            Self::Opus => Box::new(opus_tag::opus_tagger::VorbisComment::read_from_path(path)?),
        })
    }
}

#[must_use]
pub struct TaggedFile {
    inner: Box<dyn Tag + Send>,
    path: PathBuf,
    was_changed: bool,
}
impl TaggedFile {
    fn inner_from_path(path: &Path, default_empty: bool) -> Result<Box<dyn Tag + Send>, Error> {
        let format: Supportet = path.try_into()?;
        let tag: Result<Box<dyn Tag + Send>, Error> = format.read_boxed(path);
        match tag {
            Ok(tag) => Ok(tag),
            Err(Error::NoTag) if default_empty => {
                log::debug!("file {path:?} didn't have Tags, using empty");
                Ok(format.new_empty())
            }
            Err(err) => Err(err),
        }
    }
    /// reads the tags from `path` or returns empty tag, when the file doesn't have tags
    pub fn from_path(path: PathBuf, default_empty: bool) -> Result<Self, Error> {
        Ok(Self {
            inner: Self::inner_from_path(&path, default_empty)?,
            path,
            was_changed: false,
        })
    }
    /// creates a new set of tags
    pub fn new_empty(path: PathBuf) -> Result<Self, Error> {
        Ok(Self {
            inner: Supportet::new_empty(path.as_path().try_into()?),
            path,
            was_changed: false,
        })
    }
    /// drops all changes and loads the current tags
    pub fn reload(&mut self, default_empty: bool) -> Result<(), Error> {
        self.was_changed = false;
        self.inner = Self::inner_from_path(&self.path, default_empty)?;
        Ok(())
    }
    /// rereads tags and fills all that are currently empty
    pub fn reload_empty(&mut self) -> Result<(), Error> {
        self.fill_all_from(&Self::from_path(self.path.clone(), true)?);
        Ok(())
    }

    #[must_use]
    /// a reference to the current path of this file
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
    /// changes the internal file path in case the file was moved externally
    pub fn file_moved(&mut self, new_path: PathBuf) {
        self.path = new_path;
    }
    /// saves changes to file if something changes or `force_save`
    /// this function should be used instead of the implicit save in Drop, to react to errors
    ///
    /// returns if changes where
    pub fn save_changes(&mut self, force_save: bool) -> Result<bool, Error> {
        if !(force_save || self.was_changed) {
            return Ok(false);
        }
        self.inner.write_to_path(&self.path)?;
        self.was_changed = false;
        Ok(true)
    }
    /// drops the reference without saving changes to a file
    pub fn drop_changes(mut self) {
        self.was_changed = false; // disable save after dropping and drop
    }

    #[must_use]
    /// reads the field `F` and returns the contained value if it exists
    pub fn get<F: Field>(&self) -> Option<F::Type<'_>> {
        use field_kind::Get as Kind;
        match F::KIND {
            Kind::Title => self
                .inner
                .title()
                .map(|it| F::Type::from_str(it).expect("Title from str failed")),
            Kind::Artist => self
                .inner
                .artist()
                .map(|it| F::Type::from_str(it).expect("Artist from str failed")),
            Kind::Album => self
                .inner
                .album()
                .map(|it| F::Type::from_str(it).expect("Album from str failed")),
            Kind::Genre => self
                .inner
                .genre()
                .map(|it| F::Type::from_str(it).expect("Genre from str failed")),
            Kind::Year => self
                .inner
                .year()
                .map(|it| F::Type::from_i32(it).expect("Year from i32 failed")),
            Kind::Track => self
                .inner
                .track()
                .map(|it| F::Type::from_u32(it).expect("Track from u32 failed")),
            Kind::TotalTracks => self
                .inner
                .total_tracks()
                .map(|it| F::Type::from_u32(it).expect("TotalTracks from u32 failed")),
            Kind::Disk => self
                .inner
                .disk()
                .map(|it| F::Type::from_u32(it).expect("Disk from u32 failed")),
            Kind::TotalDisks => self
                .inner
                .total_disks()
                .map(|it| F::Type::from_u32(it).expect("TotalDisks from u32 failed")),
            Kind::Length => self
                .inner
                .duration()
                .map(|it| F::Type::from_duration(it).expect("length from Duration failed")),
        }
    }
    /// upates the field `F` with `value` or removes it, if `value` is `None`
    pub fn set<'a, F: Field + 'a>(&'a mut self, value: impl Into<Option<F::Type<'a>>>)
    where
        F::Type<'a>: PartialEq,
    {
        let value = value.into();
        {
            let ptr = self as *mut Self;
            // SAFTY: the reborrow is only needed to inform the borrow checker, that after the if block no borrow remains
            if unsafe { &*ptr }.get::<F>() == value {
                return;
            }
        }

        self.inner.set(F::wrap_value(value));
        self.was_changed = true;
    }
    /// upates the field `F` with `value` if it is currently `None`
    pub fn fill_from<'a, F: Field + 'a>(&'a mut self, other: &'a Self)
    where
        F::Type<'a>: PartialEq,
    {
        if self.get::<F>().is_some() {
            return;
        }
        self.set::<F>(other.get::<F>());
    }
    /// fills all fields with the values of `other`
    pub fn fill_all_from(&mut self, other: &Self) {
        self.fill_from::<Title>(other);
        self.fill_from::<Artist>(other);
        self.fill_from::<Album>(other);
        self.fill_from::<Genre>(other);
        self.fill_from::<Year>(other);
        self.fill_from::<Track>(other);
        self.fill_from::<TotalTracks>(other);
        self.fill_from::<Disk>(other);
        self.fill_from::<TotalDisks>(other);
        self.fill_from::<Length>(other);
    }
}

impl Drop for TaggedFile {
    fn drop(&mut self) {
        match self.save_changes(false) {
            Err(err) => log::error!("failed to save {:?} with {err:?}", self.path),
            Ok(true) => log::trace!("saved id3 for {:?}", self.path),
            Ok(false) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::atomic::AtomicUsize, time::Duration};

    use super::*;
    static FILE_NR: AtomicUsize = AtomicUsize::new(0);
    struct TestFile(PathBuf); // a Wrapper, that creates a copy of a file and removes it, when dropped, to allow file write tests with easy setup
    impl TestFile {
        fn new<P: AsRef<std::path::Path>>(file: P) -> Self {
            let mut path = file.as_ref().to_path_buf();
            path.set_file_name(format!(
                "tmp_{}_{}",
                FILE_NR.fetch_add(1, std::sync::atomic::Ordering::Relaxed), // give each call a uniqe number to allow parallel tests
                path.file_name().unwrap().to_str().unwrap()
            ));
            std::fs::copy(file, &path).unwrap();
            Self(path)
        }
    }
    impl AsRef<std::path::Path> for TestFile {
        fn as_ref(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for TestFile {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).unwrap();
        }
    }

    mod mp3 {
        use super::*;
        const FILE: &str = "res/id3test.mp3";

        #[test]
        fn save_when_needed() {
            let file = TestFile::new(FILE);
            let mut tag = TaggedFile::from_path(file.0.clone(), false).unwrap();

            super::save_when_needed_helper(&mut tag);
        }

        #[test]
        fn read() {
            let tag = TaggedFile::from_path(PathBuf::from(FILE), false).unwrap();
            super::read(&tag);
        }
        #[test]
        fn new_empty_is_empty() {
            let tag = TaggedFile::new_empty(PathBuf::from("/nofile.mp3")).unwrap();

            super::new_empty_is_empty(&tag);
        }
        #[test]
        fn read_saved() {
            let file = TestFile::new(FILE);
            super::read_saved(&file);
        }
    }

    mod opus {
        use super::*;
        const FILE: &str = "res/tag_test.opus";

        #[test]
        fn save_when_needed() {
            let file = TestFile::new(FILE);
            let mut tag = TaggedFile::from_path(file.0.clone(), false).unwrap();

            super::save_when_needed_helper(&mut tag);
        }

        #[test]
        fn read() {
            let tag = TaggedFile::from_path(PathBuf::from(FILE), false).unwrap();
            super::read(&tag);
        }
        #[test]
        fn new_empty_is_empty() {
            let tag = TaggedFile::new_empty(PathBuf::from("/nofile.opus")).unwrap();

            super::new_empty_is_empty(&tag);
        }
        #[test]
        fn read_saved() {
            let file = TestFile::new(FILE);
            super::read_saved(&file);
        }
    }

    fn save_when_needed_helper(tag: &mut TaggedFile) {
        assert!(
            tag.save_changes(true).unwrap(),
            "force save without changes"
        );
        assert!(!tag.save_changes(false).unwrap(), "save without changes");
        tag.set::<Title>(Some("test 1"));
        assert!(tag.save_changes(false).unwrap(), "save with changes");
        tag.set::<Title>(Some("test 2"));
        assert!(tag.save_changes(true).unwrap(), "force save with changes");
        tag.set::<Title>(Some("test 2"));
        assert!(
            !tag.save_changes(false).unwrap(),
            "save without true changes"
        );
    }

    fn read(tag: &TaggedFile) {
        assert_eq!(Some("title"), tag.get::<Title>());
        assert_eq!(Some("artist"), tag.get::<Artist>());
        assert_eq!(Some("album"), tag.get::<Album>());
        assert_eq!(Some("genre"), tag.get::<Genre>());
        assert_eq!(Some(2023), tag.get::<Year>());
        assert_eq!(Some(5), tag.get::<Track>());
        assert_eq!(Some(7), tag.get::<TotalTracks>());
        assert_eq!(Some(2), tag.get::<Disk>());
        assert_eq!(Some(Duration::from_secs(7)), tag.get::<Length>());
    }

    fn new_empty_is_empty(tag: &TaggedFile) {
        assert_eq!(None, tag.get::<Title>());
        assert_eq!(None, tag.get::<Artist>());
        assert_eq!(None, tag.get::<Album>());
        assert_eq!(None, tag.get::<Genre>());
        assert_eq!(None, tag.get::<Year>());
        assert_eq!(None, tag.get::<Track>());
        assert_eq!(None, tag.get::<TotalTracks>());
        assert_eq!(None, tag.get::<Disk>());
        assert_eq!(None, tag.get::<TotalDisks>());
        assert_eq!(None, tag.get::<Length>());
    }

    fn read_saved(file: &TestFile) {
        let mut tag = TaggedFile::from_path(file.0.clone(), false).unwrap();
        let new_title = "example";

        assert_ne!(
            Some(new_title),
            tag.get::<Title>(),
            "title already {new_title:?}"
        );
        tag.set::<Title>(Some(new_title));
        tag.save_changes(false).unwrap();

        let tag = TaggedFile::from_path(file.0.clone(), false).unwrap();
        assert_eq!(
            Some(new_title),
            tag.get::<Title>(),
            "after load new title got reset"
        );
    }
}
