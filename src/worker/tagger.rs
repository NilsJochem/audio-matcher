use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use opus_tag::opus_tagger::{Comment, OpusMeta, VorbisComment};
use thiserror::Error;

macro_rules! field_none_method {
    (str) => {
        fn from_str(_: &str) -> Option<Self> {
            None
        }
        fn into_str(self) -> Option<&'a str> {
            None
        }
    };
    (Duration) => {
        fn from_duration(_: Duration) -> Option<Self> {
            None
        }
        fn into_duration(self) -> Option<Duration> {
            None
        }
    };
    (u32) => {
        fn from_u32(_: u32) -> Option<Self> {
            None
        }
        fn into_u32(self) -> Option<u32> {
            None
        }
    };
    (i32) => {
        fn from_i32(_: i32) -> Option<Self> {
            None
        }
        fn into_i32(self) -> Option<i32> {
            None
        }
    };
}

macro_rules! field {
    ($name: ident, str) => {
        field!($name, &'a str);
    };
    ($name: ident, $ref:ty) => {
        pub struct $name;
        impl Field for $name {
            type Type<'a> = $ref where Self: 'a;
            const KIND: FieldKind = FieldKind::$name;
        }
    };
}

pub enum FieldKind {
    Title,
    Artist,
    Album,
    Genre,
    Year,
    Track,
    TotalTracks,
    Disc,
    TotalDiscs,
    Length,
}
pub trait FieldValue<'a>: Sized {
    fn from_str(value: &'a str) -> Option<Self>;
    fn from_duration(value: Duration) -> Option<Self>;
    fn from_u32(value: u32) -> Option<Self>;
    fn from_i32(value: i32) -> Option<Self>;

    fn into_str(self) -> Option<&'a str>;
    fn into_duration(self) -> Option<Duration>;
    fn into_u32(self) -> Option<u32>;
    fn into_i32(self) -> Option<i32>;
}
pub trait Field {
    type Type<'a>: FieldValue<'a>
    where
        Self: 'a;
    const KIND: FieldKind;
}

field!(Title, str);
field!(Artist, str);
field!(Album, str);
field!(Genre, str);

field!(Year, i32);

field!(Track, u32);
field!(TotalTracks, u32);
field!(Disc, u32);
field!(TotalDiscs, u32);

field!(Length, Duration);

impl<'a> FieldValue<'a> for &'a str {
    fn from_str(value: &'a str) -> Option<Self> {
        Some(value)
    }
    fn into_str(self) -> Option<&'a str> {
        Some(self)
    }
    field_none_method!(Duration);
    field_none_method!(u32);
    field_none_method!(i32);
}
impl<'a> FieldValue<'a> for Duration {
    fn from_duration(value: Duration) -> Option<Self> {
        Some(value)
    }
    fn into_duration(self) -> Option<Duration> {
        Some(self)
    }
    field_none_method!(str);
    field_none_method!(u32);
    field_none_method!(i32);
}
impl<'a> FieldValue<'a> for u32 {
    fn from_u32(value: u32) -> Option<Self> {
        Some(value)
    }
    fn into_u32(self) -> Option<u32> {
        Some(self)
    }
    field_none_method!(str);
    field_none_method!(Duration);
    field_none_method!(i32);
}
impl<'a> FieldValue<'a> for i32 {
    fn from_i32(value: i32) -> Option<Self> {
        Some(value)
    }
    fn into_i32(self) -> Option<i32> {
        Some(self)
    }
    field_none_method!(str);
    field_none_method!(u32);
    field_none_method!(Duration);
}

pub trait Tag {
    fn title(&self) -> Option<&str>;
    fn artist(&self) -> Option<&str>;
    fn album(&self) -> Option<&str>;
    fn genre(&self) -> Option<&str>;
    fn year(&self) -> Option<i32>;
    fn track(&self) -> Option<u32>;
    fn total_tracks(&self) -> Option<u32>;
    fn disc(&self) -> Option<u32>;
    fn total_discs(&self) -> Option<u32>;
    fn duration(&self) -> Option<Duration>;

    fn set_title(&mut self, value: &str);
    fn set_artist(&mut self, value: &str);
    fn set_album(&mut self, value: &str);
    fn set_genre(&mut self, value: &str);
    fn set_year(&mut self, value: i32);
    fn set_track(&mut self, value: u32);
    fn set_total_tracks(&mut self, value: u32);
    fn set_disc(&mut self, value: u32);
    fn set_total_discs(&mut self, value: u32);
    fn set_duration(&mut self, value: Duration);

    fn remove_title(&mut self);
    fn remove_artist(&mut self);
    fn remove_album(&mut self);
    fn remove_genre(&mut self);
    fn remove_year(&mut self);
    fn remove_track(&mut self);
    fn remove_total_tracks(&mut self);
    fn remove_disc(&mut self);
    fn remove_total_discs(&mut self);
    fn remove_duration(&mut self);

    fn write_to_path(&self, path: &Path) -> Result<(), Error>;
}

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
    fn disc(&self) -> Option<u32> {
        id3::TagLike::disc(self)
    }
    fn total_discs(&self) -> Option<u32> {
        id3::TagLike::total_discs(self)
    }
    fn duration(&self) -> Option<Duration> {
        id3::TagLike::duration(self).map(|it| Duration::from_secs(it as u64))
    }

    fn set_title(&mut self, value: &str) {
        id3::TagLike::set_title(self, value);
    }
    fn set_artist(&mut self, value: &str) {
        id3::TagLike::set_artist(self, value);
    }
    fn set_album(&mut self, value: &str) {
        id3::TagLike::set_album(self, value);
    }
    fn set_genre(&mut self, value: &str) {
        id3::TagLike::set_genre(self, value);
    }
    fn set_year(&mut self, value: i32) {
        id3::TagLike::set_year(self, value);
    }
    fn set_track(&mut self, value: u32) {
        id3::TagLike::set_track(self, value);
    }
    fn set_total_tracks(&mut self, value: u32) {
        id3::TagLike::set_total_tracks(self, value);
    }
    fn set_disc(&mut self, value: u32) {
        id3::TagLike::set_disc(self, value);
    }
    fn set_total_discs(&mut self, value: u32) {
        id3::TagLike::set_total_discs(self, value);
    }
    fn set_duration(&mut self, value: Duration) {
        id3::TagLike::set_duration(self, value.as_secs() as u32);
    }

    fn remove_title(&mut self) {
        id3::TagLike::remove_title(self);
    }
    fn remove_artist(&mut self) {
        id3::TagLike::remove_artist(self);
    }
    fn remove_album(&mut self) {
        id3::TagLike::remove_album(self);
    }
    fn remove_genre(&mut self) {
        id3::TagLike::remove_genre(self);
    }
    fn remove_year(&mut self) {
        id3::TagLike::remove_year(self);
    }
    fn remove_track(&mut self) {
        id3::TagLike::remove_track(self);
    }
    fn remove_total_tracks(&mut self) {
        id3::TagLike::remove_total_tracks(self);
    }
    fn remove_disc(&mut self) {
        id3::TagLike::remove_disc(self);
    }
    fn remove_total_discs(&mut self) {
        id3::TagLike::remove_total_discs(self);
    }
    fn remove_duration(&mut self) {
        id3::TagLike::remove_duration(self);
    }

    fn write_to_path(&self, path: &Path) -> Result<(), Error> {
        Ok(self.write_to_path(path, self.version())?)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum VorbisKeys {
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
    const fn get_keys(self) -> &'static [&'static str] {
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

    fn get_first(self, tag: &VorbisComment) -> Option<&'_ str> {
        let comments = self
            .get_all(tag)
            .map(|Comment { key: _, value }| value.as_str())
            .collect::<Vec<_>>();
        if comments.len() >= 2 {
            log::warn!("more than one comment for {self:?} found: {comments:?}");
        }
        comments.first().copied()
    }
    fn get_first_map<'a, T>(
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
    fn get_all(self, tag: &VorbisComment) -> impl Iterator<Item = &'_ Comment> {
        let keys = self.get_keys();
        keys.iter().flat_map(|key| tag.find_comments(key))
    }

    fn set_first(self, tag: &mut VorbisComment, value: &impl ToString) {
        let comments = self.get_all(tag).collect::<Vec<_>>();
        let keys = self.get_keys();
        match comments.as_slice() {
            [] => {
                log::warn!("more than one comment for {self:?} found: {comments:?}");
            }
            [_] => {
                log::warn!("one comment for {self:?} found: {comments:?}, will overwrite");
                for key in keys {
                    if tag.remove_first(key).is_some() {
                        break;
                    }
                }
            }
            [..] => {
                log::warn!("more than one comment for {self:?} found: {comments:?}, will append");
                todo!("handle better")
            }
        }
        tag.add_comment((keys[0], value.to_string()));
    }

    fn remove_all(self, tag: &mut VorbisComment) {
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

    fn disc(&self) -> Option<u32> {
        VorbisKeys::DiskNumber.get_first_map(self, |it| it.parse().ok())
    }

    fn total_discs(&self) -> Option<u32> {
        VorbisKeys::TotalDiskNumber.get_first_map(self, |it| it.parse().ok())
    }

    fn duration(&self) -> Option<Duration> {
        VorbisKeys::Duration.get_first_map(self, |it| it.parse().ok().map(Duration::from_secs))
    }

    fn set_title(&mut self, value: &str) {
        VorbisKeys::Title.set_first(self, &value);
    }

    fn set_artist(&mut self, value: &str) {
        VorbisKeys::Artist.set_first(self, &value);
    }

    fn set_album(&mut self, value: &str) {
        VorbisKeys::Album.set_first(self, &value);
    }

    fn set_genre(&mut self, value: &str) {
        VorbisKeys::Genre.set_first(self, &value);
    }

    fn set_year(&mut self, value: i32) {
        VorbisKeys::Year.set_first(self, &value);
    }

    fn set_track(&mut self, value: u32) {
        VorbisKeys::TrackNumber.set_first(self, &value);
    }

    fn set_total_tracks(&mut self, value: u32) {
        VorbisKeys::TotalTrackNumber.set_first(self, &value);
    }

    fn set_disc(&mut self, value: u32) {
        VorbisKeys::DiskNumber.set_first(self, &value);
    }

    fn set_total_discs(&mut self, value: u32) {
        VorbisKeys::TotalDiskNumber.set_first(self, &value);
    }

    fn set_duration(&mut self, value: Duration) {
        VorbisKeys::Duration.set_first(self, &value.as_secs());
    }

    fn remove_title(&mut self) {
        VorbisKeys::Title.remove_all(self);
    }

    fn remove_artist(&mut self) {
        VorbisKeys::Artist.remove_all(self);
    }

    fn remove_album(&mut self) {
        VorbisKeys::Album.remove_all(self);
    }

    fn remove_genre(&mut self) {
        VorbisKeys::Genre.remove_all(self);
    }

    fn remove_year(&mut self) {
        VorbisKeys::Year.remove_all(self);
    }

    fn remove_track(&mut self) {
        VorbisKeys::TrackNumber.remove_all(self);
    }

    fn remove_total_tracks(&mut self) {
        VorbisKeys::TotalTrackNumber.remove_all(self);
    }

    fn remove_disc(&mut self) {
        VorbisKeys::DiskNumber.remove_all(self);
    }

    fn remove_total_discs(&mut self) {
        VorbisKeys::TotalDiskNumber.remove_all(self);
    }

    fn remove_duration(&mut self) {
        VorbisKeys::Duration.remove_all(self);
    }

    fn write_to_path(&self, path: &Path) -> Result<(), Error> {
        self.write_opus_file(path)
            .map_err(|err| Error::Other(Box::new(err)))
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
impl From<id3::Error> for Error {
    fn from(value: id3::Error) -> Self {
        match value.kind {
            id3::ErrorKind::NoTag => Self::NoTag,
            _ => Self::Other(Box::new(value)),
        }
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

#[must_use]
pub struct TaggedFile {
    inner: Box<dyn Tag + Send>,
    path: PathBuf,
    was_changed: bool,
}
impl TaggedFile {
    fn inner_from_path(path: &Path, default_empty: bool) -> Result<Box<dyn Tag + Send>, Error> {
        match path.try_into()? {
            Supportet::Mp3 => {
                match id3::Tag::read_from_path(path).map_err(std::convert::Into::into) {
                    Ok(tag) => Ok(Box::new(tag)),
                    Err(Error::NoTag) if default_empty => {
                        log::debug!("file {path:?} didn't have Tags, using empty");
                        Ok(Self::inner_empty(Supportet::Mp3))
                    }
                    Err(err) => Err(err),
                }
            }
            Supportet::Opus => match OpusMeta::read_from_file(path) {
                Ok(meta) => Ok(Box::new(meta.tags)),
                Err(err) => Err(Error::Other(Box::new(err))),
            },
        }
    }
    fn inner_empty(format: Supportet) -> Box<dyn Tag + Send> {
        match format {
            Supportet::Mp3 => Box::new(id3::Tag::new()),
            Supportet::Opus => Box::new(VorbisComment::empty("Lavf60.3.100")), // better vendor
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
            inner: Self::inner_empty(path.as_path().try_into()?),
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
        match F::KIND {
            FieldKind::Title => self
                .inner
                .title()
                .map(|it| F::Type::from_str(it).expect("Title from str failed")),
            FieldKind::Artist => self
                .inner
                .artist()
                .map(|it| F::Type::from_str(it).expect("Artist from str failed")),
            FieldKind::Album => self
                .inner
                .album()
                .map(|it| F::Type::from_str(it).expect("Album from str failed")),
            FieldKind::Genre => self
                .inner
                .genre()
                .map(|it| F::Type::from_str(it).expect("Genre from str failed")),
            FieldKind::Year => self
                .inner
                .year()
                .map(|it| F::Type::from_i32(it).expect("Year from i32 failed")),
            FieldKind::Track => self
                .inner
                .track()
                .map(|it| F::Type::from_u32(it).expect("Track from u32 failed")),
            FieldKind::TotalTracks => self
                .inner
                .total_tracks()
                .map(|it| F::Type::from_u32(it).expect("TotalTracks from u32 failed")),
            FieldKind::Disc => self
                .inner
                .disc()
                .map(|it| F::Type::from_u32(it).expect("Disc from u32 failed")),
            FieldKind::TotalDiscs => self
                .inner
                .total_discs()
                .map(|it| F::Type::from_u32(it).expect("TotalDiscs from u32 failed")),
            FieldKind::Length => self
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

        match value {
            Some(value) => match F::KIND {
                FieldKind::Title => self
                    .inner
                    .set_title(value.into_str().expect("Title into str failed")),
                FieldKind::Artist => self
                    .inner
                    .set_artist(value.into_str().expect("Artist into str failed")),
                FieldKind::Album => self
                    .inner
                    .set_album(value.into_str().expect("Album into str failed")),
                FieldKind::Genre => self
                    .inner
                    .set_genre(value.into_str().expect("Genre into str failed")),
                FieldKind::Year => self
                    .inner
                    .set_year(value.into_i32().expect("Year into i32 failed")),
                FieldKind::Track => self
                    .inner
                    .set_track(value.into_u32().expect("Track into u32 failed")),
                FieldKind::TotalTracks => self
                    .inner
                    .set_total_tracks(value.into_u32().expect("TotalTracks into u32 failed")),
                FieldKind::Disc => self
                    .inner
                    .set_disc(value.into_u32().expect("Discs into u32 failed")),
                FieldKind::TotalDiscs => self
                    .inner
                    .set_total_discs(value.into_u32().expect("TotalDiscs into u32 failed")),
                FieldKind::Length => self
                    .inner
                    .set_duration(value.into_duration().expect("Length into Duration failed")),
            },
            None => match F::KIND {
                FieldKind::Title => self.inner.remove_title(),
                FieldKind::Artist => self.inner.remove_artist(),
                FieldKind::Album => self.inner.remove_album(),
                FieldKind::Genre => self.inner.remove_genre(),
                FieldKind::Year => self.inner.remove_year(),
                FieldKind::Track => self.inner.remove_track(),
                FieldKind::TotalTracks => self.inner.remove_total_tracks(),
                FieldKind::Disc => self.inner.remove_disc(),
                FieldKind::TotalDiscs => self.inner.remove_total_discs(),
                FieldKind::Length => self.inner.remove_duration(),
            },
        }
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
        self.fill_from::<Disc>(other);
        self.fill_from::<TotalDiscs>(other);
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
        assert_eq!(Some(2), tag.get::<Disc>());
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
        assert_eq!(None, tag.get::<Disc>());
        assert_eq!(None, tag.get::<TotalDiscs>());
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
