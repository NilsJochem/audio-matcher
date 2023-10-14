#![allow(missing_docs)]
use std::{borrow::Borrow, ops::Deref, rc::Rc, sync::Arc};

pub trait Generic<T: ?Sized>: Clone + Deref + Borrow<T> + AsRef<T> + Unpin {
    fn new(value: T) -> Self
    where
        T: Sized;
    fn into_inner(this: Self) -> Option<T>
    where
        T: Sized;
    fn unwrap_or_clone(this: Self) -> T
    where
        T: Clone;
    fn get_mut(this: &mut Self) -> Option<&mut T>;
}
impl<T: ?Sized> Generic<T> for Rc<T> {
    fn new(value: T) -> Self
    where
        T: Sized,
    {
        Self::new(value)
    }
    fn into_inner(this: Self) -> Option<T>
    where
        T: Sized,
    {
        Self::try_unwrap(this).ok()
    }
    fn unwrap_or_clone(this: Self) -> T
    where
        T: Clone,
    {
        Self::try_unwrap(this).unwrap_or_else(|rc| (*rc).clone())
    }
    fn get_mut(this: &mut Self) -> Option<&mut T> {
        Self::get_mut(this)
    }
}
impl<T: ?Sized> Generic<T> for Arc<T> {
    fn new(value: T) -> Self
    where
        T: Sized,
    {
        Self::new(value)
    }
    fn into_inner(this: Self) -> Option<T>
    where
        T: Sized,
    {
        Self::try_unwrap(this).ok()
    }
    fn unwrap_or_clone(this: Self) -> T
    where
        T: Clone,
    {
        Self::try_unwrap(this).unwrap_or_else(|arc| (*arc).clone())
    }
    fn get_mut(this: &mut Self) -> Option<&mut T> {
        Self::get_mut(this)
    }
}
