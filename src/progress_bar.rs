use itertools::Itertools;
use pad::PadStr;
use std::convert::TryInto;
use std::time::Instant;
use std::{
    io::{stdout, Write},
    sync::{Arc, Mutex},
};

pub trait Arrow<const N: usize> {
    fn build(&self, fractions: [f64; N], bar_length: usize) -> String;
    fn padding_needed(&self) -> usize;
}
// // workaround to clone a Box<dyn Arrow>
// trait ArrowClone<const N: usize> {
//     fn clone_box(&self) -> Box<dyn Arrow<N>>;
// }
// impl <T, const N: usize> ArrowClone<N> for T where T: 'static + Arrow<N> + Clone {
//     fn clone_box(&self) -> Box<dyn Arrow<N>> {
//         Box::new(self.clone())
//     }
// }

#[derive(PartialEq, Eq, Clone)]
pub struct SimpleArrow<const N: usize> {
    arrow_prefix: String,
    arrow_suffix: String,
    arrow_chars: [char; N],
    arrow_tip: char,
}

impl Default for SimpleArrow<1> {
    fn default() -> Self {
        Self {
            arrow_prefix: "[".to_owned(),
            arrow_suffix: "]".to_owned(),
            arrow_chars: ['='],
            arrow_tip: '>',
        }
    }
}
impl Default for SimpleArrow<2> {
    fn default() -> Self {
        Self {
            arrow_chars: ['=', '-'],
            arrow_prefix: SimpleArrow::<1>::default().arrow_prefix,
            arrow_suffix: SimpleArrow::<1>::default().arrow_suffix,
            arrow_tip: SimpleArrow::<1>::default().arrow_tip,
        }
    }
}
impl<const N: usize> Arrow<N> for SimpleArrow<N> {
    fn build(&self, fractions: [f64; N], bar_length: usize) -> String {
        let bar_length = bar_length - (self.arrow_prefix.len() + self.arrow_suffix.len()); //remove surrounding

        let mut arrow =
            String::with_capacity(bar_length + self.arrow_prefix.len() + self.arrow_suffix.len());
        arrow.push_str(&self.arrow_prefix);

        let mut last_fraction = 0.0;
        for (i, fraction) in fractions.into_iter().enumerate() {
            let char = self.arrow_chars[i];
            arrow.push_str(
                &char
                    .to_string()
                    .repeat(((fraction - last_fraction) * bar_length as f64).round() as usize),
            );
            last_fraction = fraction;
        }

        if bar_length - (arrow.len() - self.arrow_prefix.len()) > 0 {
            arrow.push(self.arrow_tip);
        }
        arrow.push_str(
            " ".repeat(bar_length - (arrow.len() - self.arrow_prefix.len()))
                .as_str(),
        );
        arrow.push_str(&self.arrow_suffix);
        arrow
    }
    fn padding_needed(&self) -> usize {
        self.arrow_prefix.len() + self.arrow_suffix.len()
    }
}

pub struct FancyArrow {
    empty_bar: [char; 3],
    full_bar: [char; 3],
}
impl Default for FancyArrow {
    fn default() -> Self {
        // unicode progress bars
        Self {
            empty_bar: ['\u{ee00}', '\u{ee01}', '\u{ee02}'],
            full_bar: ['\u{ee03}', '\u{ee04}', '\u{ee05}'],
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
pub struct Unbounded;
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
pub trait Bound: Sized {
    fn display<Iter, const N: usize>(&self, progress: &Progress<Iter, N, Self>, post_msg: &str);
    fn cleanup();
    fn is_in_bound(&self, n: usize) -> bool;
}
impl Bound for Unbounded {
    fn is_in_bound(&self, _n: usize) -> bool {
        true
    }
    fn display<Iter, const N: usize>(&self, _progress: &Progress<Iter, N, Self>, _post_msg: &str) {
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
    fn display<Iter, const N: usize>(&self, progress: &Progress<Iter, N, Self>, post_msg: &str) {
        assert!(
            post_msg.len() <= self.post_msg_len,
            "given post_msg '{post_msg}' is to long"
        );
        let i = progress.i.lock().expect("progress poisened");
        let mut fractions = i.map(|c| c as f64 / self.size as f64);
        fractions.reverse();

        let width = ((self.size + 1) as f32).log10().ceil() as usize;
        let current_fmt = i.iter().rev().map(|f| format!("{f:0width$}")).join("+");

        let bar_len = self
            .max_len
            .map_or(self.size + progress.bar.arrow.padding_needed(), |max| {
                max + 1
                    - (progress.bar.pre_msg.len()
                        + current_fmt.len()
                        + width * 2
                        + self.post_msg_len)
            });

        crate::leveled_output::print(
            &crate::leveled_output::OutputLevel::Info,
            &format!(
                "\r{}{} {current_fmt}/{}{}",
                progress.bar.pre_msg,
                progress.bar.arrow.build(fractions, bar_len),
                self.size,
                post_msg,
            ),
        );
        stdout().flush().unwrap();
    }
    fn cleanup() {
        crate::leveled_output::info(&"");
    }
}

pub struct Bar<const N: usize> {
    pre_msg: String,
    is_timed: bool,
    arrow: Box<dyn Arrow<N> + Send + Sync>,
}
impl<const N: usize> Bar<N> {
    pub fn new(pre_msg: String, is_timed: bool, arrow: Box<dyn Arrow<N> + Send + Sync>) -> Self {
        Self {
            pre_msg,
            is_timed,
            arrow,
        }
    }
}

pub struct Progress<Iter, const N: usize, B: Bound> {
    bar: Bar<N>,
    iter: Iter,
    i: Mutex<[usize; N]>,
    start: Option<Instant>,
    bound: B,
}

impl<Iter, const N: usize, B: Bound> Progress<Iter, N, B> {
    pub fn inc(&self, n: usize) {
        assert!(n < N, "can't increment at {n}, max layers {N}");
        let mut i = self.i.lock().expect("mutex of progress poisend");
        assert!(self.bound.is_in_bound(i[n]), "exceeding bounds");
        Self::__inc(&mut i, n);
        let is_last = !self.bound.is_in_bound(i[N - 1]);
        drop(i);

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

impl<Iter, const N: usize> Progress<Iter, N, Bounded>
where
    Iter: ExactSizeIterator,
{
    pub fn new_bound(bar: Bar<N>, iter: Iter, post_msg_len: usize) -> Self {
        let size = iter.len();
        let post_msg_len = post_msg_len + (bar.is_timed as usize * 6); // add 6 to post_len, when time is shown
        let start = bar.is_timed.then(|| Instant::now());
        Self {
            bar,
            iter,
            i: Mutex::new([0; N]),
            start,
            bound: Bounded::new(size, post_msg_len, None),
        }
    }
    pub fn fit_bound(mut self) -> Option<Self> {
        let terminal_width = term_size::dimensions().map(|(w, _)| w)?;
        // assert!(terminal_width.is_some(), "couldn't get terminal width");
        self.bound.max_len = Some(terminal_width);
        Some(self)
    }
}

impl<Iter, const N: usize> Progress<Iter, N, Unbounded>
where
    Iter: Iterator,
{
    pub fn new_unbound(bar: Bar<N>, iter: Iter) -> Self {
        Self {
            bar,
            iter,
            i: Mutex::new([0; N]),
            start: None,
            bound: Unbounded {},
        }
    }
}

impl<Iter, const N: usize, B> Progress<Iter, N, B>
where
    Iter: Iterator,
    B: Bound,
{
    pub fn next_steps(
        &mut self,
    ) -> Option<([Box<dyn FnOnce(&Self) + Send + Sync>; N], Iter::Item)> {
        let res = self.iter.next();

        res.map(|it| {
            let mut funcs: Vec<Box<dyn FnOnce(&Self) + Send + Sync>> = Vec::with_capacity(N);
            for i in 0..N {
                funcs.push(Box::new(move |s| s.inc(i)));
            }
            (convert_to_array(funcs), it)
        })
    }
}
impl<Iter, B> Iterator for Progress<Iter, 1, B>
where
    Iter: Iterator,
    B: Bound,
{
    type Item = Iter::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.iter.next();
        if res.is_none() {
            B::cleanup();
        } else {
            self.inc(0);
        }

        res
    }
}

impl<'a, Iter: Iterator + Send + Sync + 'a, B: Bound + Send + Sync + 'a> Progress<Iter, 2, B> {
    pub fn iter_with_finish(self, mut work: impl FnMut(Box<dyn FnOnce() + Send + Sync + 'a>, Iter::Item)) {
        let arc = Arc::new(Mutex::new(self));
        let mut next = arc.lock().unwrap();
        while let Some(([f1, f2], i)) = next.next_steps() {
            f1(&next); // use and drop
            drop(next);

            let inner_arc = Arc::clone(&arc);
            work(Box::new(move || f2(&(inner_arc.lock().unwrap()))), i);

            next = arc.lock().unwrap(); // reclaim for next loop
        }
    }
}

fn convert_to_array<T, const N: usize>(v: Vec<T>) -> [T; N] {
    v.try_into()
        .unwrap_or_else(|v: Vec<T>| panic!("Expected a Vec of length {N} but it was {}", v.len()))
}

pub struct Open;
pub struct Closed;

pub struct ProgressBar<const N: usize, State = Closed> {
    pub bar_length: usize,
    pub pre_msg: String,
    pub arrow: Arc<dyn Arrow<N> + Send + Sync>,
    pub state: std::marker::PhantomData<State>,
}

impl<const N: usize, State> Clone for ProgressBar<N, State> {
    fn clone(&self) -> Self {
        Self {
            bar_length: self.bar_length,
            pre_msg: self.pre_msg.clone(),
            arrow: self.arrow.clone(),
            state: self.state,
        }
    }
}

impl Default for ProgressBar<1, Closed> {
    fn default() -> Self {
        Self {
            bar_length: 20,
            pre_msg: "Progress: ".to_owned(),
            arrow: Arc::new(SimpleArrow::default()),
            state: std::marker::PhantomData::default(),
        }
    }
}
impl Default for ProgressBar<2, Closed> {
    fn default() -> Self {
        Self {
            bar_length: 20,
            pre_msg: "Progress: ".to_owned(),
            arrow: Arc::new(SimpleArrow::default()),
            state: std::marker::PhantomData::default(),
        }
    }
}
#[must_use = "need to finalize Progressbar"]
trait Critical {}
impl<const N: usize> Critical for ProgressBar<N, Open> {}

impl<const N: usize> ProgressBar<N, Closed> {
    pub fn prepare_output(self) -> ProgressBar<N, Open> {
        println!();
        ProgressBar {
            bar_length: self.bar_length,
            pre_msg: self.pre_msg,
            arrow: self.arrow,
            state: std::marker::PhantomData::<Open>,
        }
    }
}

impl<const N: usize> ProgressBar<N, Open> {
    pub fn finish_output(self) -> ProgressBar<N, Closed> {
        println!();
        ProgressBar {
            bar_length: self.bar_length,
            pre_msg: self.pre_msg,
            arrow: self.arrow,
            state: std::marker::PhantomData::<Closed>,
        }
    }
    pub fn print_bar(&self, mut current: [usize; N], total: usize, post_msg: &str) {
        current[N - 1] = current[N - 1].max(0);
        for i in N - 1..0 {
            current[i + 1] = current[i + 1].min(current[i]);
        }
        let total = total.max(current[0]);
        let fractions = current.map(|c| c as f64 / total as f64);

        let current_fmt = current
            .iter()
            .map(|f| {
                f.to_string().pad(
                    (total as f32).log10().ceil() as usize,
                    '0',
                    pad::Alignment::Right,
                    false,
                )
            })
            .join("+");

        crate::leveled_output::print(
            &crate::leveled_output::OutputLevel::Info,
            &format!(
                "\r{}{} {current_fmt}/{}{}",
                self.pre_msg,
                self.arrow.build(fractions, self.bar_length),
                total,
                post_msg,
            ),
        );

        stdout().flush().unwrap();
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
