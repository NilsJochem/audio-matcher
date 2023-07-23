pub trait IteratorExt: Iterator + Sized {
    fn filter_surrounding<F>(self, predicate: F) -> SurroundingFilterIterator<Self, F>
    where
        F: FnMut(&Option<Self::Item>, &Self::Item, &Option<Self::Item>) -> bool;

    fn chunked(self, window_size: usize, hop_length: usize) -> ChunkedIterator<Self>;
}

impl<Iter: Iterator> IteratorExt for Iter {
    fn filter_surrounding<F>(self, predicate: F) -> SurroundingFilterIterator<Self, F>
    where
        F: FnMut(&Option<Self::Item>, &Self::Item, &Option<Self::Item>) -> bool,
    {
        SurroundingFilterIterator::new(self, predicate)
    }

    fn chunked(self, window_size: usize, hop_length: usize) -> ChunkedIterator<Self> {
        ChunkedIterator::new(self, window_size, hop_length)
    }
}

pub struct ChunkedIterator<Iter: Iterator> {
    iter: Iter,
    window_size: usize,
    hop_length: usize,
    buffer: Vec<Iter::Item>,
}
impl<Iter: Iterator> ChunkedIterator<Iter> {
    fn new(iter: Iter, window_size: usize, hop_length: usize) -> Self {
        Self {
            iter,
            window_size,
            hop_length,
            buffer: Vec::with_capacity(hop_length),
        }
    }
}
impl<T: Clone, Iter: Iterator<Item = T>> Iterator for ChunkedIterator<Iter> {
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

impl<Iter: Iterator, F: FnMut(&Option<Iter::Item>, &Iter::Item, &Option<Iter::Item>) -> bool>
    SurroundingFilterIterator<Iter, F>
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

impl<
        T: Clone,
        Iter: Iterator<Item = T>,
        F: FnMut(&Option<Iter::Item>, &Iter::Item, &Option<Iter::Item>) -> bool,
    > Iterator for SurroundingFilterIterator<Iter, F>
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let include = (self.predicate)(&self.last, self.element.as_ref()?, &self.next);
        let mut tmp = self.iter.next(); // get next element
        std::mem::swap(&mut tmp, &mut self.next); // store next as self.next, hold old.next
        std::mem::swap(&mut tmp, &mut self.element); // store old.next as self.element, hold old.element
        self.last = tmp; // store old.element as self.last, discard self.last
        if include {
            Some(self.last.clone().unwrap()) // return clone of self.last==old.element
        } else {
            self.next() // skip this element
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;

    #[test]
    fn chunked_test() {
        let is = (0..15).chunked(6, 4).collect_vec();
        let expected = vec![0..6, 4..10, 8..14, 12..15]
            .into_iter()
            .map(itertools::Itertools::collect_vec)
            .collect_vec();
        assert!(&is.eq(&expected), "expected {expected:?} but was {is:?}");
    }

    #[test]
    fn surrounding_filter_test() {
        let is = (0..4)
            .into_iter()
            .filter_surrounding(|l, _e, a| {
                !(l.is_some_and(|it| it == 2) || a.is_some_and(|it| it == 2))
            })
            .collect_vec();
        let expected = vec![0, 2];
        assert!(&is.eq(&expected), "expected {expected:?} but got {is:?}")
    }
}
