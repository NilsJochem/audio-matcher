//! A module for Boo (Borrow or Owned)
use std::{borrow::Borrow, ops::Deref};

/// A Borrow or Owned Smart pointer
/// The Boo type is a smaller Variant of Cow, which doesn't need special types for Borrowed and Owned Types
pub enum Boo<'a, T> {
    /// Borrowed data
    Borrowed(&'a T),
    /// Owned data
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
