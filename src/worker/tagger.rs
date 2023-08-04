use id3::{Tag, TagLike};
use std::path::PathBuf;

macro_rules! field {
    ($ret: ty, $name: ident, $get_fn: ident, $set_fn: ident, $remove_fn: ident) => {
        pub struct $name;
        impl<'a> Field<'a> for $name {
            type Type = $ret;
            fn get(tag: &'a id3::Tag) -> Option<Self::Type> {
                tag.$get_fn()
            }
            fn set(tag: &'a mut id3::Tag, value: Self::Type) -> bool {
                if tag.$get_fn().is_some_and(|it| it == value) {
                    return false;
                }
                tag.$set_fn(value);
                true
            }
            fn remove(tag: &'a mut id3::Tag) -> bool {
                if tag.$get_fn().is_none() {
                    return false;
                }
                tag.$remove_fn();
                true
            }
            fn fill(tag: &'a mut id3::Tag, value: Self::Type) -> bool {
                if tag.$get_fn().is_some() {
                    return false;
                }
                tag.$set_fn(value);
                true
            }
        }
    };
}
macro_rules! s_field {
    ($name: ident, $get_fn: ident, $set_fn: ident, $remove_fn: ident) => {
        field!(&'a str, $name, $get_fn, $set_fn, $remove_fn);
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

pub trait Field<'a> {
    type Type;
    /// returns the current value
    fn get(tag: &'a id3::Tag) -> Option<Self::Type>;
    /// sets the value to `value` and returns, if something changed
    fn set(tag: &'a mut id3::Tag, value: Self::Type) -> bool;
    /// removes the value to `value` and returns, if something changed
    fn remove(tag: &'a mut id3::Tag) -> bool;
    /// sets the value to `value` if it is currently `None`
    fn fill(tag: &'a mut id3::Tag, value: Self::Type) -> bool;

    /// updates the value to `value` and returns, if something changed
    fn update(tag: &'a mut id3::Tag, value: Option<Self::Type>) -> bool {
        match value {
            Some(value) => Self::set(tag, value),
            None => Self::remove(tag),
        }
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
impl<'a> Field<'a> for Length {
    type Type = std::time::Duration;
    fn get(tag: &id3::Tag) -> Option<Self::Type> {
        tag.duration().map(|it| Self::Type::from_secs(it as u64))
    }
    fn set(tag: &mut id3::Tag, value: Self::Type) -> bool {
        if tag
            .duration()
            .is_some_and(|it| it == value.as_secs() as u32)
        {
            return false;
        }
        tag.set_duration(value.as_secs() as u32);
        true
    }
    fn remove(tag: &'a mut id3::Tag) -> bool {
        if tag.duration().is_none() {
            return false;
        }
        tag.remove_duration();
        true
    }
    fn fill(tag: &mut id3::Tag, value: Self::Type) -> bool {
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
    /// reads the tags from `path`
    pub fn from_path(path: PathBuf) -> Result<Self, id3::Error> {
        Ok(Self {
            inner: Tag::read_from_path(&path)?,
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
    pub fn reload(&mut self) -> Result<(), id3::Error> {
        self.was_changed = false;
        self.inner = Tag::read_from_path(&self.path)?;
        Ok(())
    }
    /// rereads tags and fills all that are currently empty
    pub fn reload_empty(&mut self) -> Result<(), id3::Error> {
        self.fill_all_from(&Self::from_path(self.path.clone())?);
        Ok(())
    }

    #[must_use]
    /// a reference to the current path of this file
    pub const fn path(&self) -> &PathBuf {
        &self.path
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
    pub fn get<'a, F: Field<'a>>(&'a self) -> Option<F::Type> {
        F::get(&self.inner)
    }
    /// upates the field `F` with `value` or removes it, if `value` is `None`
    pub fn set<'a, F: Field<'a>>(&'a mut self, value: Option<F::Type>) {
        self.was_changed |= F::update(&mut self.inner, value);
    }
    /// upates the field `F` with `value` if it is currently `None`
    pub fn fill_from<'a, F: Field<'a>>(&'a mut self, other: &'a Self) {
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
    use std::time::Duration;

    use super::*;
    struct TestFile(PathBuf);
    impl TestFile {
        fn new<P: AsRef<std::path::Path>>(file: P) -> Self {
            let mut path = file.as_ref().to_path_buf();
            path.set_file_name(format!(
                "tmp_{}",
                path.file_name().unwrap().to_str().unwrap()
            ));
            std::fs::copy(file, &path).unwrap();
            Self(path)
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
    fn save_correctly() {
        let file = TestFile::new("res/id3test.mp3");
        let mut tag = TaggedFile::from_path(file.0.clone()).unwrap();

        assert_eq!(
            Some(true),
            tag.save_changes(true).ok(),
            "force save without changes"
        );
        assert_eq!(
            Some(false),
            tag.save_changes(false).ok(),
            "save without changes"
        );
        tag.set::<Title>(Some("test 1"));
        assert_eq!(
            Some(true),
            tag.save_changes(false).ok(),
            "save with changes"
        );
        tag.set::<Title>(Some("test 2"));
        assert_eq!(
            Some(true),
            tag.save_changes(true).ok(),
            "force save with changes"
        );
        tag.set::<Title>(Some("test 2"));
        assert_eq!(
            Some(false),
            tag.save_changes(false).ok(),
            "save without true changes"
        );
    }

    #[test]
    fn read() {
        let tag = TaggedFile::from_path(PathBuf::from("res/id3test.mp3")).unwrap();

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
    fn read_empty() {
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
}
