use std::{borrow::Borrow, ops::Deref};

pub enum Boo<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}

impl<'a, T> Borrow<T> for Boo<'a, T> {
    fn borrow(&self) -> &T {
        match self {
            Boo::Borrowed(t) => t,
            Boo::Owned(t) => t,
        }
    }
}
impl<'a, T> Deref for Boo<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        match self {
            Boo::Borrowed(t) => t,
            Boo::Owned(t) => t,
        }
    }
}
