use std::borrow::Cow;

pub trait Ext<'a> {
    /// returns a borrowed of the given Cow type
    fn reborrow(&'a self) -> Self;
}

impl<'a, 'c: 'a, B: ?Sized + 'a + ToOwned> Ext<'c> for Cow<'a, B> {
    #[inline]
    fn reborrow(&'c self) -> Self {
        Cow::Borrowed(self.as_ref())
    }
}
impl<'a, 'c: 'a, B: ?Sized + 'c + ToOwned> Ext<'c> for Option<Cow<'a, B>> {
    #[inline]
    fn reborrow(&'c self) -> Self {
        self.as_ref().map(Ext::reborrow)
    }
}
