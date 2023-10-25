use super::{stdout, Debug, Itertools, ProgressBarHolder, Write};

pub trait Bound: Sized + Debug {
    fn display<const N: usize>(&self, progress: &ProgressBarHolder<N, Self>, post_msg: &str);
    fn cleanup();
    fn is_in_bound(&self, n: usize) -> bool;
}

#[derive(Debug)]
pub struct Unbounded;
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

#[derive(Debug)]
pub struct Bounded {
    size: usize,
    post_msg_len: usize,
    pub(super) max_len: Option<usize>,
}
impl Bounded {
    pub(super) const fn new(size: usize, post_msg_len: usize, max_len: Option<usize>) -> Self {
        Self {
            size,
            post_msg_len,
            max_len,
        }
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

        let size_width = ((self.size + 1) as f32).log10().ceil() as usize;
        let current_fmt = progress
            .i
            .iter()
            .rev()
            .map(|f| format!("{f:0size_width$}"))
            .join("+");

        let bar_len = self
            .max_len
            .map_or(self.size + progress.bar.arrow.padding_needed(), |max| {
                max - (progress.bar.pre_msg.len()
                    + current_fmt.len()
                    + size_width * 2
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
        println!();
    }
}
