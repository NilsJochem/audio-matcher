pub trait IteratorExt: Iterator + Sized {
    fn with_size(self, size: usize) -> ExactSizeWrapper<Self>;
}
impl<Iter: Iterator> IteratorExt for Iter {
    fn with_size(self, size: usize) -> ExactSizeWrapper<Self> {
        ExactSizeWrapper::new(self, size)
    }
}
pub trait FutIterExt: IntoIterator + Sized
where
    Self::Item: core::future::Future,
{
    fn join_all(self) -> futures::future::JoinAll<<Self as IntoIterator>::Item>;
}
impl<Iter: IntoIterator + Sized> FutIterExt for Iter
where
    Iter::Item: core::future::Future,
{
    fn join_all(self) -> futures::future::JoinAll<<Self as IntoIterator>::Item> {
        futures::future::join_all(self)
    }
}

pub trait CloneIteratorExt: Iterator + Sized {
    fn chunked(self, window_size: usize, hop_length: usize) -> ChunkedIterator<Self>;
    fn filter_surrounding<F>(self, predicate: F) -> SurroundingFilterIterator<Self, F>
    where
        F: FnMut(&Option<Self::Item>, &Self::Item, &Option<Self::Item>) -> bool;
    fn open_border_pairs(self) -> OpenBorderWindowIterator<Self>;
}
impl<Iter> CloneIteratorExt for Iter
where
    Iter: Iterator,
    Iter::Item: Clone,
{
    fn chunked(self, window_size: usize, hop_length: usize) -> ChunkedIterator<Self> {
        ChunkedIterator::new(self, window_size, hop_length)
    }
    fn filter_surrounding<F>(self, predicate: F) -> SurroundingFilterIterator<Self, F>
    where
        F: FnMut(&Option<Self::Item>, &Self::Item, &Option<Self::Item>) -> bool,
    {
        SurroundingFilterIterator::new(self, predicate)
    }
    fn open_border_pairs(self) -> OpenBorderWindowIterator<Self> {
        OpenBorderWindowIterator::new(self)
    }
}

pub struct ChunkedIterator<Iter: Iterator> {
    iter: Iter,
    window_size: usize,
    hop_length: usize,
    buffer: Vec<Iter::Item>,
}
impl<Iter> ChunkedIterator<Iter>
where
    Iter: Iterator,
    Iter::Item: Clone,
{
    fn new(iter: Iter, window_size: usize, hop_length: usize) -> Self {
        Self {
            iter,
            window_size,
            hop_length,
            buffer: Vec::with_capacity(hop_length),
        }
    }
}
impl<Iter> Iterator for ChunkedIterator<Iter>
where
    Iter: Iterator,
    Iter::Item: Clone,
{
    type Item = Vec<Iter::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.buffer.len() < self.window_size {
            match self.iter.next() {
                Some(e) => self.buffer.push(e),
                None => break,
            }
        }
        if self.buffer.is_empty() {
            return None;
        }
        let ret = self.buffer.clone();
        self.buffer.drain(..self.hop_length.min(self.buffer.len()));

        Some(ret)
    }
}
impl<Iter> ExactSizeIterator for ChunkedIterator<Iter>
where
    Iter: ExactSizeIterator,
    Iter::Item: Clone,
{
    fn len(&self) -> usize {
        (self.iter.len() as f64 / self.hop_length as f64).ceil() as usize
    }
}

pub struct SurroundingFilterIterator<
    Iter: Iterator,
    F: FnMut(&Option<Iter::Item>, &Iter::Item, &Option<Iter::Item>) -> bool,
> {
    iter: Iter,
    predicate: F,
    last: Option<Iter::Item>,
    element: Option<Iter::Item>,
    next: Option<Iter::Item>,
}
impl<Iter, F> SurroundingFilterIterator<Iter, F>
where
    Iter: Iterator,
    Iter::Item: Clone,
    F: FnMut(&Option<Iter::Item>, &Iter::Item, &Option<Iter::Item>) -> bool,
{
    fn new(mut iter: Iter, predicate: F) -> Self {
        Self {
            predicate,
            last: None,
            element: iter.next(),
            next: iter.next(),
            iter,
        }
    }
}
impl<Iter, F> Iterator for SurroundingFilterIterator<Iter, F>
where
    Iter: Iterator,
    Iter::Item: Clone,
    F: FnMut(&Option<Iter::Item>, &Iter::Item, &Option<Iter::Item>) -> bool,
{
    type Item = Iter::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let include = (self.predicate)(&self.last, self.element.as_ref()?, &self.next);
        let element = std::mem::replace(&mut self.next, self.iter.next()); // get next element
        self.last = std::mem::replace(&mut self.element, element);
        if include {
            Some(self.last.clone().unwrap()) // return clone of self.last==old.element
        } else {
            self.next() // skip this element
        }
    }
}

pub struct ExactSizeWrapper<Iter: Iterator> {
    iter: Iter,
    i: usize,
    size: usize,
}
impl<Iter: Iterator> ExactSizeWrapper<Iter> {
    const fn new(iter: Iter, size: usize) -> Self {
        Self { iter, i: 0, size }
    }
}
impl<Iter: Iterator> Iterator for ExactSizeWrapper<Iter> {
    type Item = Iter::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.iter.next();
        self.i += ret.is_some() as usize;
        ret
    }
}
impl<Iter: Iterator> ExactSizeIterator for ExactSizeWrapper<Iter> {
    fn len(&self) -> usize {
        self.size - self.i
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum State<T> {
    Start(T),
    Middle(T, T),
    End(T),
}
impl<T> State<T> {
    #[allow(clippy::missing_const_for_fn)]
    fn new(a: Option<T>, b: Option<T>) -> Option<Self> {
        match (a, b) {
            (None, None) => None,
            (None, Some(b)) => Some(Self::Start(b)),
            (Some(a), Some(b)) => Some(Self::Middle(a, b)),
            (Some(a), None) => Some(Self::End(a)),
        }
    }
}
pub struct OpenBorderWindowIterator<Iter: Iterator> {
    iter: Iter,
    next: Option<Iter::Item>,
}
impl<Iter> OpenBorderWindowIterator<Iter>
where
    Iter: Iterator,
    Iter::Item: Clone,
{
    const fn new(iter: Iter) -> Self {
        Self { iter, next: None }
    }
}
impl<Iter> Iterator for OpenBorderWindowIterator<Iter>
where
    Iter: Iterator,
    Iter::Item: Clone,
{
    type Item = State<Iter::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        let last = std::mem::replace(&mut self.next, self.iter.next());
        State::new(last, self.next.clone())
    }
}
impl<Iter> ExactSizeIterator for OpenBorderWindowIterator<Iter>
where
    Iter: ExactSizeIterator,
    Iter::Item: Clone,
{
    fn len(&self) -> usize {
        self.iter.len() + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;

    #[test]
    fn chunked_test() {
        let expected = vec![0..6, 4..10, 8..14, 12..15]
            .into_iter()
            .map(itertools::Itertools::collect_vec)
            .collect_vec();
        let is = (0..15).chunked(6, 4);
        assert_eq!(expected.len(), is.len());

        let is = is.collect_vec();
        assert!(&is.eq(&expected), "expected {expected:?} but was {is:?}");
    }

    #[test]
    fn surrounding_filter_test() {
        let is = (0..4)
            .filter_surrounding(|l, _e, a| {
                !(l.is_some_and(|it| it == 2) || a.is_some_and(|it| it == 2))
            })
            .collect_vec();
        let expected = vec![0, 2];
        assert!(&is.eq(&expected), "expected {expected:?} but got {is:?}");
    }
    #[test]
    fn open_border_iter() {
        let iter = [1, 2, 3].into_iter().open_border_pairs();
        assert_eq!(iter.len(), 4);
        assert!(iter.eq([
            State::Start(1),
            State::Middle(1, 2),
            State::Middle(2, 3),
            State::End(3)
        ]
        .into_iter()));
    }
}
