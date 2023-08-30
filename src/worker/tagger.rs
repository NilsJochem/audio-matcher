use id3::{Tag, TagLike};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

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
        pub struct $name;
        impl Field for $name {
            type Type<'a> = &'a str where Self: 'a;
            const KIND: FieldKind = FieldKind::$name;
        }
    };
    ($name: ident, u32) => {
        pub struct $name;
        impl Field for $name {
            type Type<'a> = u32 where Self: 'a;
            const KIND: FieldKind = FieldKind::$name;
        }
    };
    ($name: ident, i32) => {
        pub struct $name;
        impl Field for $name {
            type Type<'a> = i32 where Self: 'a;
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

field!(Title, str);
field!(Artist, str);
field!(Album, str);
field!(Genre, str);

field!(Year, i32);

field!(Track, u32);
field!(TotalTracks, u32);
field!(Disc, u32);
field!(TotalDiscs, u32);

pub struct Length;
impl Field for Length {
    type Type<'a> = std::time::Duration;
    const KIND: FieldKind = FieldKind::Length;
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

pub trait Field {
    type Type<'a>: FieldValue<'a>
    where
        Self: 'a;
    const KIND: FieldKind;
}

pub trait MyTag {
    /// returns the current value
    fn get<F: Field>(&self) -> Option<F::Type<'_>>;
    /// sets the value to `value`
    fn set_unchecked<'b, 'a: 'b, F: Field>(&'a mut self, value: F::Type<'b>)
    where
        F::Type<'b>: PartialEq;

    /// sets the value to `value` and returns true if something changed
    fn set<'b, 'a: 'b, F: Field>(&'a mut self, value: F::Type<'b>) -> bool
    where
        F::Type<'b>: PartialEq,
    {
        {
            let ptr = self as *mut Self;
            // SAFTY: the reborrow is only needed to inform the borrow checker, that after the if block no borrow remains
            let self_reborrow = unsafe { &*ptr };
            if MyTag::get::<F>(self_reborrow).is_some_and(|it| it == value) {
                return false;
            }
        }
        self.set_unchecked::<F>(value);
        true
    }
    /// removes the value to `value`
    fn remove_unchecked<F: Field>(&mut self);
    /// removes the value to `value` and returns true, if something changed
    fn remove<F: Field>(&mut self) -> bool {
        if MyTag::get::<F>(self).is_none() {
            return false;
        }
        self.remove_unchecked::<F>();
        true
    }

    /// sets the value to `value` if it is currently `None`
    fn fill<'b, 'a: 'b, F: Field>(&'a mut self, value: F::Type<'b>) -> bool
    where
        F::Type<'b>: PartialEq,
    {
        if self.get::<F>().is_some() {
            return false;
        }
        self.set::<F>(value);
        true
    }

    /// updates the value to `value` and returns, if something changed
    fn update<'a, F: Field>(&'a mut self, value: Option<F::Type<'a>>) -> bool
    where
        F::Type<'a>: PartialEq,
    {
        match value {
            Some(value) => self.set::<F>(value),
            None => self.remove::<F>(),
        }
    }
}

impl MyTag for id3::Tag {
    fn get<F: Field>(&self) -> Option<F::Type<'_>> {
        match F::KIND {
            FieldKind::Title => self
                .title()
                .map(|it| F::Type::from_str(it).expect("Title from str failed")),
            FieldKind::Artist => self
                .artist()
                .map(|it| F::Type::from_str(it).expect("Artist from str failed")),
            FieldKind::Album => self
                .album()
                .map(|it| F::Type::from_str(it).expect("Album from str failed")),
            FieldKind::Genre => self
                .genre()
                .map(|it| F::Type::from_str(it).expect("Genre from str failed")),
            FieldKind::Year => self
                .year()
                .map(|it| F::Type::from_i32(it).expect("Year from i32 failed")),
            FieldKind::Track => self
                .track()
                .map(|it| F::Type::from_u32(it).expect("Track from u32 failed")),
            FieldKind::TotalTracks => self
                .total_tracks()
                .map(|it| F::Type::from_u32(it).expect("TotalTracks from u32 failed")),
            FieldKind::Disc => self
                .disc()
                .map(|it| F::Type::from_u32(it).expect("Disc from u32 failed")),
            FieldKind::TotalDiscs => self
                .total_discs()
                .map(|it| F::Type::from_u32(it).expect("TotalDiscs from u32 failed")),
            FieldKind::Length => self.duration().map(|it| {
                F::Type::from_duration(Duration::from_secs(it as u64))
                    .expect("length from Duration failed")
            }),
        }
    }

    fn set_unchecked<'b, 'a: 'b, F: Field>(&'a mut self, value: F::Type<'b>)
    where
        F::Type<'b>: PartialEq,
    {
        match F::KIND {
            FieldKind::Title => self.set_title(value.into_str().expect("Title into str failed")),
            FieldKind::Artist => self.set_artist(value.into_str().expect("Artist into str failed")),
            FieldKind::Album => self.set_album(value.into_str().expect("Album into str failed")),
            FieldKind::Genre => self.set_genre(value.into_str().expect("Genre into str failed")),
            FieldKind::Year => self.set_year(value.into_i32().expect("Year into i32 failed")),
            FieldKind::Track => self.set_track(value.into_u32().expect("Track into u32 failed")),
            FieldKind::TotalTracks => {
                self.set_total_tracks(value.into_u32().expect("TotalTracks into u32 failed"));
            }
            FieldKind::Disc => self.set_disc(value.into_u32().expect("Discs into u32 failed")),
            FieldKind::TotalDiscs => {
                self.set_total_discs(value.into_u32().expect("TotalDiscs into u32 failed"));
            }
            FieldKind::Length => self.set_duration(
                value
                    .into_duration()
                    .expect("Length into Duration failed")
                    .as_secs() as u32,
            ),
        }
    }

    fn remove_unchecked<F: Field>(&mut self) {
        match F::KIND {
            FieldKind::Title => self.remove_title(),
            FieldKind::Artist => self.remove_artist(),
            FieldKind::Album => self.remove_album(),
            FieldKind::Genre => self.remove_genre(),
            FieldKind::Year => self.remove_year(),
            FieldKind::Track => self.remove_track(),
            FieldKind::TotalTracks => self.remove_total_tracks(),
            FieldKind::Disc => self.remove_disc(),
            FieldKind::TotalDiscs => self.remove_total_discs(),
            FieldKind::Length => self.remove_duration(),
        }
    }
}

#[must_use]
pub struct TaggedFile {
    inner: id3::Tag,
    path: PathBuf,
    was_changed: bool,
}
impl TaggedFile {
    fn inner_from_path(path: &Path, default_empty: bool) -> Result<Tag, id3::Error> {
        match Tag::read_from_path(path) {
            Ok(tag) => Ok(tag),
            Err(id3::Error {
                kind: id3::ErrorKind::NoTag,
                ..
            }) if default_empty => {
                log::debug!("file {path:?} didn't have Tags, using empty");
                Ok(Tag::new())
            }
            Err(err) => Err(err),
        }
    }
    /// reads the tags from `path` or returns empty tag, when the file doesn't have tags
    pub fn from_path(path: PathBuf, default_empty: bool) -> Result<Self, id3::Error> {
        Ok(Self {
            inner: Self::inner_from_path(&path, default_empty)?,
            path,
            was_changed: false,
        })
    }
    /// creates a new set of tags
    pub fn new_empty(path: PathBuf) -> Self {
        Self {
            inner: Tag::new(),
            path,
            was_changed: false,
        }
    }
    /// drops all changes and loads the current tags
    pub fn reload(&mut self, default_empty: bool) -> Result<(), id3::Error> {
        self.was_changed = false;
        self.inner = Self::inner_from_path(&self.path, default_empty)?;
        Ok(())
    }
    /// rereads tags and fills all that are currently empty
    pub fn reload_empty(&mut self) -> Result<(), id3::Error> {
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
    pub fn save_changes(&mut self, force_save: bool) -> Result<bool, id3::Error> {
        if !(force_save || self.was_changed) {
            return Ok(false);
        }
        self.inner.write_to_path(&self.path, self.inner.version())?;
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
        MyTag::get::<F>(&self.inner)
    }
    /// upates the field `F` with `value` or removes it, if `value` is `None`
    pub fn set<'a, F: Field + 'a>(&'a mut self, value: impl Into<Option<F::Type<'a>>>)
    where
        F::Type<'a>: PartialEq,
    {
        self.was_changed |= MyTag::update::<F>(&mut self.inner, value.into());
    }
    /// upates the field `F` with `value` if it is currently `None`
    pub fn fill_from<'a, F: Field + 'a>(&'a mut self, other: &'a Self)
    where
        F::Type<'a>: PartialEq,
    {
        if let Some(v) = other.get::<F>() {
            MyTag::fill::<F>(&mut self.inner, v);
        }
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

    #[test]
    fn update_field_return() {
        let mut tags = Tag::new();
        assert!(MyTag::set::<Title>(&mut tags, "test"), "set when empty");
        assert!(!MyTag::set::<Title>(&mut tags, "test"), "set with same");
        assert!(MyTag::remove::<Title>(&mut tags), "remove with some");
        assert!(!MyTag::remove::<Title>(&mut tags), "remove when empty");

        assert!(
            MyTag::update::<Title>(&mut tags, Some("test")),
            "update set when empty"
        );
        assert!(
            !MyTag::update::<Title>(&mut tags, Some("test")),
            "update set with same"
        );
        assert!(
            MyTag::update::<Title>(&mut tags, None),
            "update remove with some"
        );
        assert!(
            !MyTag::update::<Title>(&mut tags, None),
            "update remove when empty"
        );
    }

    #[test]
    fn save_when_needed() {
        let file = TestFile::new("res/id3test.mp3");
        let mut tag = TaggedFile::from_path(file.0.clone(), false).unwrap();

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

    #[test]
    fn read() {
        let tag = TaggedFile::from_path(PathBuf::from("res/id3test.mp3"), false).unwrap();

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
    #[test]
    fn new_empty_is_empty() {
        let tag = TaggedFile::new_empty(PathBuf::from("/nofile"));

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

    #[test]
    fn read_saved() {
        let file = TestFile::new("res/id3test.mp3");
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
