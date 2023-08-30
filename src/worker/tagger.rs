use id3::{Tag, TagLike};
use std::path::{Path, PathBuf};

macro_rules! inner_field {
    ($ret: ty, $get_fn: ident, $set_fn: ident, $remove_fn: ident) => {
        type Type<'a> = $ret where Self: 'a;
        fn get(tag: &id3::Tag) -> Option<Self::Type<'_>> {
            tag.$get_fn()
        }
        fn set(tag: &mut id3::Tag, value: Self::Type<'_>) -> bool {
            if tag.$get_fn().is_some_and(|it| it == value) {
                return false;
            }
            tag.$set_fn(value);
            true
        }
        fn remove(tag: &mut id3::Tag) -> bool {
            if tag.$get_fn().is_none() {
                return false;
            }
            tag.$remove_fn();
            true
        }
        fn fill(tag: &mut id3::Tag, value: Self::Type<'_>) -> bool {
            if tag.$get_fn().is_some() {
                return false;
            }
            tag.$set_fn(value);
            true
        }
    };
}
macro_rules! field {
    ($ret: ty, $name: ident, $get_fn: ident, $set_fn: ident, $remove_fn: ident) => {
        pub struct $name;
        impl Field for $name {
            inner_field!($ret, $get_fn, $set_fn, $remove_fn);
        }
    };
}
macro_rules! ref_field {
    ($ret: ty, $name: ident, $get_fn: ident, $set_fn: ident, $remove_fn: ident) => {
        pub struct $name;
        impl Field for $name {
            inner_field!(&'a $ret, $get_fn, $set_fn, $remove_fn);
        }
    };
}

macro_rules! s_field {
    ($name: ident, $get_fn: ident, $set_fn: ident, $remove_fn: ident) => {
        ref_field!(str, $name, $get_fn, $set_fn, $remove_fn);
    };
}
macro_rules! i_field {
    ($name: ident, $get_fn: ident, $set_fn: ident, $remove_fn: ident) => {
        field!(i32, $name, $get_fn, $set_fn, $remove_fn);
    };
}
macro_rules! u_field {
    ($name: ident, $get_fn: ident, $set_fn: ident, $remove_fn: ident) => {
        field!(u32, $name, $get_fn, $set_fn, $remove_fn);
    };
}

pub trait Field {
    type Type<'a>
    where
        Self: 'a;
    /// returns the current value
    fn get(tag: &id3::Tag) -> Option<Self::Type<'_>>;

    /// sets the value to `value` and returns, if something changed
    fn set(tag: &mut id3::Tag, value: Self::Type<'_>) -> bool;
    /// removes the value to `value` and returns, if something changed
    fn remove(tag: &mut id3::Tag) -> bool;
    /// sets the value to `value` if it is currently `None`
    fn fill(tag: &mut id3::Tag, value: Self::Type<'_>) -> bool;

    /// updates the value to `value` and returns, if something changed
    fn update(tag: &mut id3::Tag, value: Option<Self::Type<'_>>) -> bool {
        match value {
            Some(value) => Self::set(tag, value),
            None => Self::remove(tag),
        }
    }
}

pub trait MyTrait {
    /// returns the current value
    fn get<F: Field>(&self) -> Option<F::Type<'_>>;
    /// sets the value to `value` and returns, if something changed
    fn set<'b, 'a: 'b, F: Field>(&'a mut self, value: F::Type<'b>) -> bool
    where
        F::Type<'b>: PartialEq;
    /// removes the value to `value` and returns, if something changed
    fn remove<F: Field>(&mut self) -> bool;
    /// sets the value to `value` if it is currently `None`
    fn fill<F: Field>(&mut self, value: F::Type<'_>) -> bool;

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
impl MyTrait for id3::Tag {
    fn get<F: Field>(&self) -> Option<F::Type<'_>> {
        F::get(self)
    }

    fn set<'b, 'a: 'b, F: Field>(&'a mut self, value: F::Type<'b>) -> bool
    where
        F::Type<'b>: PartialEq,
    {
        {
            let ptr = self as *mut Self;
            // SAFTY: the reborrow is only needed to inform the borrow checker, that after the if block no borrow remains
            let self_reborrow = unsafe { &*ptr };
            if F::get(self_reborrow).is_some_and(|it| it == value) {
                return false;
            }
        }

        F::set(self, value);
        true
    }

    fn remove<F: Field>(&mut self) -> bool {
        F::remove(self)
    }

    fn fill<F: Field>(&mut self, value: F::Type<'_>) -> bool {
        F::fill(self, value)
    }
}

s_field!(Title, title, set_title, remove_title);
s_field!(Artist, artist, set_artist, remove_artist);
s_field!(Album, album, set_album, remove_album);
s_field!(Genre, genre, set_genre, remove_genre);

i_field!(Year, year, set_year, remove_year);

u_field!(Track, track, set_track, remove_track);
u_field!(
    TotalTracks,
    total_tracks,
    set_total_tracks,
    remove_total_tracks
);
u_field!(Disc, disc, set_disc, remove_disc);
u_field!(TotalDiscs, total_discs, set_total_discs, remove_total_discs);

pub struct Length;
impl Field for Length {
    type Type<'a> = std::time::Duration;
    fn get(tag: &id3::Tag) -> Option<Self::Type<'_>> {
        tag.duration().map(|it| Self::Type::from_secs(it as u64))
    }
    fn set(tag: &mut id3::Tag, value: Self::Type<'_>) -> bool {
        if tag
            .duration()
            .is_some_and(|it| it == value.as_secs() as u32)
        {
            return false;
        }
        tag.set_duration(value.as_secs() as u32);
        true
    }
    fn remove(tag: &mut id3::Tag) -> bool {
        if tag.duration().is_none() {
            return false;
        }
        tag.remove_duration();
        true
    }
    fn fill(tag: &mut id3::Tag, value: Self::Type<'_>) -> bool {
        if tag.duration().is_some() {
            return false;
        }
        tag.set_duration(value.as_secs() as u32);
        true
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
        F::get(&self.inner)
    }
    /// upates the field `F` with `value` or removes it, if `value` is `None`
    pub fn set<'a, F: Field + 'a>(&'a mut self, value: impl Into<Option<F::Type<'a>>>) {
        self.was_changed |= F::update(&mut self.inner, value.into());
    }
    /// upates the field `F` with `value` if it is currently `None`
    pub fn fill_from<'a, F: Field>(&'a mut self, other: &'a Self) {
        if let Some(v) = other.get::<F>() {
            F::fill(&mut self.inner, v);
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
        assert!(Title::set(&mut tags, "test"), "set when empty");
        assert!(!Title::set(&mut tags, "test"), "set with same");
        assert!(Title::remove(&mut tags), "remove with some");
        assert!(!Title::remove(&mut tags), "remove when empty");

        assert!(
            Title::update(&mut tags, Some("test")),
            "update set when empty"
        );
        assert!(
            !Title::update(&mut tags, Some("test")),
            "update set with same"
        );
        assert!(Title::update(&mut tags, None), "update remove with some");
        assert!(!Title::update(&mut tags, None), "update remove when empty");
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
