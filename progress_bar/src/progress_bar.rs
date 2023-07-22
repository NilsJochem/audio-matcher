use itertools::Itertools;
use std::fmt::Debug;
use std::time::Instant;
use std::{
    io::{stdout, Write},
    sync::{Arc, Mutex},
};

pub trait Arrow<const N: usize>: Debug {
    fn build(&self, fractions: [f64; N], bar_length: usize) -> String;
    fn padding_needed(&self) -> usize;
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SimpleArrow<const N: usize> {
    arrow_prefix: &'static str,
    arrow_suffix: &'static str,
    base_char: char,
    arrow_chars: [char; N],
    arrow_tip: &'static str,
}

pub struct UnicodeBar(char, char);
#[allow(non_snake_case, dead_code)]
impl UnicodeBar {
    pub fn Rising() -> Self {
        Self('█', '▁')
    }
    pub fn Box() -> Self {
        Self('■', '□')
    }
    pub fn Circle() -> Self {
        Self('⬤', '◯')
    }
    pub fn Parallelogramm() -> Self {
        Self('▰', '▱')
    }
}
impl From<UnicodeBar> for SimpleArrow<1> {
    fn from(value: UnicodeBar) -> Self {
        Self {
            arrow_prefix: "",
            arrow_suffix: "",
            base_char: value.1,
            arrow_chars: [value.0],
            arrow_tip: "",
        }
    }
}
impl SimpleArrow<2> {
    pub fn unicode_grayscale() -> Self {
        Self {
            arrow_prefix: "",
            arrow_suffix: "",
            base_char: '▒',
            arrow_chars: ['█', '▓'],
            arrow_tip: "",
        }
    }
}

impl Default for SimpleArrow<1> {
    fn default() -> Self {
        Self {
            arrow_prefix: "[",
            arrow_suffix: "]",
            base_char: ' ',
            arrow_chars: ['='],
            arrow_tip: ">",
        }
    }
}
impl Default for SimpleArrow<2> {
    fn default() -> Self {
        Self {
            arrow_chars: ['=', '-'],
            base_char: SimpleArrow::<1>::default().base_char,
            arrow_prefix: SimpleArrow::<1>::default().arrow_prefix,
            arrow_suffix: SimpleArrow::<1>::default().arrow_suffix,
            arrow_tip: SimpleArrow::<1>::default().arrow_tip,
        }
    }
}
impl<const N: usize> Arrow<N> for SimpleArrow<N> {
    fn build(&self, fractions: [f64; N], bar_length: usize) -> String {
        let mut arrow = String::with_capacity(bar_length);
        let bar_length = bar_length - self.padding_needed(); //remove surrounding

        arrow.push_str(self.arrow_prefix);

        let mut last_fraction = 0.0;
        for (fraction, char) in fractions.into_iter().zip(self.arrow_chars) {
            for _ in 0..((fraction - last_fraction) * bar_length as f64).floor() as usize {
                arrow.push(char);
            }
            last_fraction = fraction;
        }
        if bar_length - (arrow.len() - self.arrow_prefix.len()) >= self.arrow_tip.len() {
            arrow.push_str(self.arrow_tip);
        }

        for _ in 0..bar_length.saturating_sub(arrow.len() - self.arrow_prefix.len()) {
            arrow.push(self.base_char);
        }
        arrow.push_str(self.arrow_suffix);
        arrow
    }
    fn padding_needed(&self) -> usize {
        self.arrow_prefix.len() + self.arrow_suffix.len()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct FancyArrow {
    empty_bar: [char; 3],
    full_bar: [char; 3],
}
impl Default for FancyArrow {
    /// uses fira typeset to print connected progress bar
    fn default() -> Self {
        Self {
            empty_bar: ['', '', ''], // '\u{ee00}', '\u{ee01}', '\u{ee02}'
            full_bar: ['', '', ''],  // '\u{ee03}', '\u{ee04}', '\u{ee05}'
        }
    }
}
// just use the last bar
impl<const N: usize> Arrow<N> for FancyArrow {
    fn build(&self, fractions: [f64; N], bar_length: usize) -> String {
        let mut arrow = String::with_capacity(bar_length);

        let arrow_len = (bar_length as f64 * fractions[0]).round() as usize;
        let full_len = (arrow_len.saturating_sub(1)).min(bar_length - 2);
        let empty_len = bar_length - (full_len + 2);
        arrow.push(
            if arrow_len == 0 {
                self.empty_bar
            } else {
                self.full_bar
            }[0],
        );
        for _ in 0..full_len {
            arrow.push(self.full_bar[1]);
        }
        for _ in 0..empty_len {
            arrow.push(self.empty_bar[1]);
        }
        arrow.push(
            if arrow_len == bar_length {
                self.full_bar
            } else {
                self.empty_bar
            }[2],
        );
        arrow
    }
    fn padding_needed(&self) -> usize {
        0
    }
}
#[derive(Debug)]
pub struct Unbounded;
#[derive(Debug)]
pub struct Bounded {
    size: usize,
    post_msg_len: usize,
    max_len: Option<usize>,
}
impl Bounded {
    fn new(size: usize, post_msg_len: usize, max_len: Option<usize>) -> Self {
        Self {
            size,
            post_msg_len,
            max_len,
        }
    }
}
pub trait Bound: Sized + Debug {
    fn display<const N: usize>(&self, progress: &ProgressBarHolder<N, Self>, post_msg: &str);
    fn cleanup();
    fn is_in_bound(&self, n: usize) -> bool;
}
impl Bound for Unbounded {
    fn is_in_bound(&self, _n: usize) -> bool {
        true
    }
    fn display<const N: usize>(&self, _progress: &ProgressBarHolder<N, Self>, _post_msg: &str) {
        todo!()
    }
    fn cleanup() {
        todo!()
    }
}
impl Bound for Bounded {
    fn is_in_bound(&self, n: usize) -> bool {
        self.size > n
    }
    fn display<const N: usize>(&self, progress: &ProgressBarHolder<N, Self>, post_msg: &str) {
        assert!(
            post_msg.len() <= self.post_msg_len,
            "given post_msg '{post_msg}' is to long"
        );
        let mut fractions = progress.i.map(|c| c as f64 / self.size as f64);
        fractions.reverse();

        let width = ((self.size + 1) as f32).log10().ceil() as usize;
        let current_fmt = progress
            .i
            .iter()
            .rev()
            .map(|f| format!("{f:0width$}"))
            .join("+");

        let bar_len = self
            .max_len
            .map_or(self.size + progress.bar.arrow.padding_needed(), |max| {
                max - (progress.bar.pre_msg.len()
                    + current_fmt.len()
                    + width * 2
                    + self.post_msg_len)
            });

        print!(
            "\r{}{} {current_fmt}/{}{}",
            progress.bar.pre_msg,
            progress.bar.arrow.build(fractions, bar_len),
            self.size,
            post_msg,
        );
        stdout().flush().unwrap();
    }
    fn cleanup() {
        println!("");
    }
}

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

pub struct Progress<Iter, const N: usize, B: Bound> {
    iter: Iter,
    holder: ProgressBarHolder<N, B>,
}
pub struct ProgressBarHolder<const N: usize, B: Bound> {
    bar: Bar<N>,
    i: [usize; N],
    start: Option<Instant>,
    bound: B,
}

impl<const N: usize, B: Bound> ProgressBarHolder<N, B> {
    pub fn inc(&mut self, n: usize) {
        assert!(n < N, "can't increment at {n}, max layers {N}");
        assert!(
            self.bound.is_in_bound(self.i[n]),
            "exceeding bounds with {:?}, with bound {:?}",
            self.i[n],
            self.bound
        );
        Self::__inc(&mut self.i, n);
        let is_last = !self.bound.is_in_bound(self.i[N - 1]);

        let fmt_elapsed = self.start.map_or_else(
            || String::new(),
            |start| {
                let elapsed = Instant::now().duration_since(start);
                let (_, minutes, seconds) = crate::split_duration(&elapsed);
                format!(" {minutes:0>2}:{seconds:0>2}")
            },
        );

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
    pub fn new_unbound(iter: Iter, bar: Bar<N>) -> Self {
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
        Progress::new_external_bound(iter, bar, post_msg_len, size)
    }
}
impl<Iter: Iterator, const N: usize> Progress<Iter, N, Bounded> {
    pub fn new_external_bound(iter: Iter, bar: Bar<N>, post_msg_len: usize, size: usize) -> Self {
        // add 6 to post_len, when time is shown to display extra ' MM:SS'
        let post_msg_len = post_msg_len + (bar.is_timed as usize * 6);
        let start = bar.is_timed.then(|| Instant::now());
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
    pub fn fit_bound(mut self) -> Option<Self> {
        let terminal_width = term_size::dimensions().map(|(w, _)| w)?;
        self.holder.bound.max_len = Some(terminal_width);
        Some(self)
    }
}

impl<const N: usize, Iter: Iterator, B: Bound> Progress<Iter, N, B> {
    pub fn get_iter(self) -> (Iter, ProgressBarHolder<N, B>) {
        self.into()
    }
    pub fn get_arc_iter(self) -> (Iter, Arc<Mutex<ProgressBarHolder<N, B>>>) {
        self.into()
    }
}
impl<const N: usize, Iter, B: Bound> Into<(Iter, ProgressBarHolder<N, B>)>
    for Progress<Iter, N, B>
{
    fn into(self) -> (Iter, ProgressBarHolder<N, B>) {
        (self.iter, self.holder)
    }
}
impl<const N: usize, Iter, B: Bound> Into<(Iter, Arc<Mutex<ProgressBarHolder<N, B>>>)>
    for Progress<Iter, N, B>
{
    fn into(self) -> (Iter, Arc<Mutex<ProgressBarHolder<N, B>>>) {
        (self.iter, Arc::new(Mutex::new(self.holder)))
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

pub struct Callback<const N: usize, B: Bound> {
    progress: Arc<Mutex<ProgressBarHolder<N, B>>>,
}
impl<const N: usize, B: Bound> Callback<N, B> {
    pub fn new(holder: &Arc<Mutex<ProgressBarHolder<N, B>>>) -> Self {
        Self {
            progress: Arc::clone(holder),
        }
    }

    pub fn get_once_calls(self) -> [OnceCallback<N, B>; N] {
        let mut vec = Vec::with_capacity(N);
        for i in 0..N {
            vec.push({
                OnceCallback {
                    progress: Arc::clone(&self.progress),
                    i,
                }
            })
        }
        vec.try_into().map_err(|_| "const N doesn't hold").unwrap()
    }
    pub fn get_mut_call(self) -> MutCallback<N, B> {
        MutCallback {
            progress: self.progress,
            i: 0,
        }
    }
}

pub struct OnceCallback<const N: usize, B: Bound> {
    progress: Arc<Mutex<ProgressBarHolder<N, B>>>,
    i: usize,
}
impl<const N: usize, B: Bound> OnceCallback<N, B> {
    pub fn new(holder: &Arc<Mutex<ProgressBarHolder<N, B>>>) -> [Self; N] {
        Callback::new(holder).get_once_calls()
    }
    pub fn new_fn(holder: &Arc<Mutex<ProgressBarHolder<N, B>>>) -> [impl FnOnce(); N] {
        Self::new(holder).map(|it| it.as_fn())
    }

    pub fn call(self) {
        self.progress.lock().unwrap().inc(self.i);
    }
    pub fn as_fn(self) -> impl FnOnce() {
        || self.call()
    }
}

pub struct MutCallback<const N: usize, B: Bound> {
    progress: Arc<Mutex<ProgressBarHolder<N, B>>>,
    i: usize,
}
impl<const N: usize, B: Bound> MutCallback<N, B> {
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

#[cfg(test)]
mod tests {
    use super::*;

    mod simple_arrow {
        use super::*;

        #[test]
        fn empty_arrow() {
            assert_eq!(
                SimpleArrow::default().build([0.0], 12),
                String::from("[>         ]")
            )
        }
        #[test]
        fn short_arrow() {
            assert_eq!(
                SimpleArrow::default().build([0.2], 12),
                String::from("[==>       ]")
            )
        }
        #[test]
        fn long_arrow() {
            assert_eq!(
                SimpleArrow::default().build([0.9], 12),
                String::from("[=========>]")
            )
        }
        #[test]
        fn full_arrow() {
            assert_eq!(
                SimpleArrow::default().build([1.0], 12),
                String::from("[==========]")
            )
        }

        #[test]
        fn double_arrow() {
            assert_eq!(
                SimpleArrow::default().build([0.3, 0.5], 12),
                String::from("[===-->    ]")
            );
        }
    }
    mod fancy_arrow {
        use super::*;

        fn ascci_arrow() -> FancyArrow {
            FancyArrow {
                empty_bar: ['(', ' ', ')'],
                full_bar: ['{', '-', '}'],
            }
        }

        #[test]
        fn empty_arrow() {
            assert_eq!(ascci_arrow().build([0.0], 10), String::from("(        )"))
        }
        #[test]
        fn short_arrow() {
            assert_eq!(ascci_arrow().build([0.2], 10), String::from("{-       )"))
        }
        #[test]
        fn long_arrow() {
            assert_eq!(ascci_arrow().build([0.9], 10), String::from("{--------)"))
        }
        #[test]
        fn full_arrow() {
            assert_eq!(ascci_arrow().build([1.0], 10), String::from("{--------}"))
        }
    }
}
