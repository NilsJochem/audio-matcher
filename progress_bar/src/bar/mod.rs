pub mod arrow;
pub mod bound;

use arrow::Arrow;
use bound::{Bound, Bounded, Unbounded};
use itertools::Itertools;
use std::{
    fmt::Debug,
    io::{stdout, Write},
    sync::{Arc, Mutex},
    time::Instant,
};

#[must_use]
pub struct Bar<const N: usize> {
    pre_msg: String,
    is_timed: bool,
    arrow: Box<dyn Arrow<N> + Send>,
}
impl<const N: usize> Bar<N> {
    pub fn new(pre_msg: String, is_timed: bool, arrow: Box<dyn Arrow<N> + Send>) -> Self {
        Self {
            pre_msg,
            is_timed,
            arrow,
        }
    }
}

pub struct Progress<Iter, const N: usize, B> {
    iter: Iter,
    holder: ProgressBarHolder<N, B>,
}
pub struct ProgressBarHolder<const N: usize, B> {
    bar: Bar<N>,
    i: [usize; N],
    start: Option<Instant>,
    bound: B,
}

impl<const N: usize, B: Bound> ProgressBarHolder<N, B> {
    pub(crate) fn inc(&mut self, n: usize) {
        assert!(n < N, "can't increment at {n}, max layers {N}");
        assert!(
            self.bound.is_in_bound(self.i[n]),
            "exceeding bounds with {:?}, with bound {:?}",
            self.i[n],
            self.bound
        );
        Self::__inc(&mut self.i, n);
        let is_last = !self.bound.is_in_bound(self.i[N - 1]);

        let fmt_elapsed = self.start.map_or_else(String::new, |start| {
            let elapsed = Instant::now().duration_since(start);
            let (_, minutes, seconds) = crate::split_duration(&elapsed);
            format!(" {minutes:0>2}:{seconds:0>2}")
        });

        self.bound.display(self, &fmt_elapsed); //update screen on every update
        if is_last {
            B::cleanup();
        }
    }

    fn __inc(i: &mut [usize; N], n: usize) {
        i[n] += 1;
        if n > 0 && i[n - 1] < i[n] {
            Self::__inc(i, n - 1);
        }
    }
}

impl<Iter: Iterator, const N: usize> Progress<Iter, N, Unbounded> {
    pub const fn new_unbound(iter: Iter, bar: Bar<N>) -> Self {
        Self {
            iter,
            holder: ProgressBarHolder {
                bar,
                i: [0; N],
                start: None,
                bound: Unbounded {},
            },
        }
    }
}
impl<Iter: ExactSizeIterator, const N: usize> Progress<Iter, N, Bounded> {
    pub fn new_bound(iter: Iter, bar: Bar<N>, post_msg_len: usize) -> Self {
        let size = iter.len();
        Self::new_external_bound(iter, bar, post_msg_len, size)
    }
}
impl<Iter: Iterator, const N: usize> Progress<Iter, N, Bounded> {
    pub fn new_external_bound(iter: Iter, bar: Bar<N>, post_msg_len: usize, size: usize) -> Self {
        // add 6 to post_len, when time is shown to display extra ' MM:SS'
        let post_msg_len = post_msg_len + (bar.is_timed as usize * 6);
        let start = bar.is_timed.then(Instant::now);
        Self {
            iter,
            holder: ProgressBarHolder {
                bar,
                i: [0; N],
                start,
                bound: Bounded::new(size, post_msg_len, None),
            },
        }
    }
    pub const fn max_len(&self) -> Option<usize> {
        self.holder.bound.max_len
    }
    pub fn mut_max_len(&mut self) -> &mut Option<usize> {
        &mut self.holder.bound.max_len
    }
    pub fn unset_max_len(&mut self) {
        *self.mut_max_len() = None;
    }
    pub fn set_max_len(&mut self, new_max_len: usize) {
        *self.mut_max_len() = Some(new_max_len);
    }
}

impl<const N: usize, Iter: Iterator, B> Progress<Iter, N, B> {
    pub fn get_iter(self) -> (Iter, ProgressBarHolder<N, B>) {
        self.into()
    }
    pub fn get_arc_iter(self) -> (Iter, Arc<Mutex<ProgressBarHolder<N, B>>>) {
        self.into()
    }
}
impl<const N: usize, Iter, B> From<Progress<Iter, N, B>> for (Iter, ProgressBarHolder<N, B>) {
    fn from(val: Progress<Iter, N, B>) -> Self {
        (val.iter, val.holder)
    }
}
impl<const N: usize, Iter, B> From<Progress<Iter, N, B>>
    for (Iter, Arc<Mutex<ProgressBarHolder<N, B>>>)
{
    fn from(val: Progress<Iter, N, B>) -> Self {
        (val.iter, Arc::new(Mutex::new(val.holder)))
    }
}

impl<Iter: Iterator, B: Bound> Iterator for Progress<Iter, 1, B> {
    type Item = Iter::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.iter.next();
        if res.is_some() {
            self.holder.inc(0);
        }
        res
    }
}

#[must_use]
pub struct Callback<const N: usize, B> {
    progress: Arc<Mutex<ProgressBarHolder<N, B>>>,
}
impl<const N: usize, B> Callback<N, B> {
    fn new(holder: &Arc<Mutex<ProgressBarHolder<N, B>>>) -> Self {
        Self {
            progress: Arc::clone(holder),
        }
    }

    pub fn get_once_calls(self) -> [Once<N, B>; N] {
        let mut vec = Vec::with_capacity(N);
        for i in 0..N {
            vec.push({
                Once {
                    progress: Arc::clone(&self.progress),
                    i,
                }
            });
        }
        vec.try_into().map_err(|_| "const N doesn't hold").unwrap()
    }
    #[allow(clippy::missing_const_for_fn)]
    pub fn get_mut_call(self) -> Mut<N, B> {
        Mut {
            progress: self.progress,
            i: 0,
        }
    }
}

#[must_use]
pub struct Once<const N: usize, B> {
    progress: Arc<Mutex<ProgressBarHolder<N, B>>>,
    i: usize,
}
impl<const N: usize, B: Bound> Once<N, B> {
    pub fn new(holder: &Arc<Mutex<ProgressBarHolder<N, B>>>) -> [Self; N] {
        Callback::new(holder).get_once_calls()
    }
    pub fn new_fn(holder: &Arc<Mutex<ProgressBarHolder<N, B>>>) -> [impl FnOnce(); N] {
        Self::new(holder).map(Self::as_fn)
    }

    pub fn call(self) {
        self.progress.lock().unwrap().inc(self.i);
    }
    pub fn as_fn(self) -> impl FnOnce() {
        || self.call()
    }
}

#[must_use]
pub struct Mut<const N: usize, B> {
    progress: Arc<Mutex<ProgressBarHolder<N, B>>>,
    i: usize,
}
impl<const N: usize, B: Bound> Mut<N, B> {
    pub fn new(holder: &Arc<Mutex<ProgressBarHolder<N, B>>>) -> Self {
        Callback::new(holder).get_mut_call()
    }
    pub fn new_fn(holder: &Arc<Mutex<ProgressBarHolder<N, B>>>) -> impl FnMut() {
        Self::new(holder).as_fn()
    }

    pub fn call(&mut self) {
        self.progress.lock().unwrap().inc(self.i);
        self.i += 1;
    }
    pub fn as_fn(mut self) -> impl FnMut() {
        move || self.call()
    }
}
