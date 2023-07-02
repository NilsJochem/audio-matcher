use itertools::Itertools;
use pad::PadStr;
use std::io::{stdout, Write};

#[derive(PartialEq, Eq, Clone)]
pub struct Arrow<const N: usize> {
    arrow_prefix: String,
    arrow_suffix: String,
    arrow_chars: [char; N],
    arrow_tip: char,
}

impl Default for Arrow<1> {
    fn default() -> Self {
        Self {
            arrow_prefix: "[".to_owned(),
            arrow_suffix: "]".to_owned(),
            arrow_chars: ['='],
            arrow_tip: '>',
        }
    }
}
impl Default for Arrow<2> {
    fn default() -> Self {
        Self {
            arrow_chars: ['=', '-'],
            arrow_prefix: Arrow::<1>::default().arrow_prefix,
            arrow_suffix: Arrow::<1>::default().arrow_suffix,
            arrow_tip: Arrow::<1>::default().arrow_tip,
        }
    }
}

impl<const N: usize> Arrow<N> {
    fn build(&self, fractions: [f64; N], bar_length: usize) -> String {
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
            last_fraction = fraction
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
}

pub struct Open;
pub struct Closed;

#[derive(PartialEq, Eq, Clone)]
pub struct ProgressBar<const N: usize, State = Closed> {
    pub bar_length: usize,
    pub pre_msg: String,
    pub arrow: Arrow<N>,
    pub _state: std::marker::PhantomData<State>,
}

impl Default for ProgressBar<1, Closed> {
    fn default() -> Self {
        Self {
            bar_length: 20,
            pre_msg: "Progress: ".to_owned(),
            arrow: Arrow::default(),
            _state: Default::default(),
        }
    }
}
impl Default for ProgressBar<2, Closed> {
    fn default() -> Self {
        Self {
            bar_length: 20,
            pre_msg: "Progress: ".to_owned(),
            arrow: Arrow::default(),
            _state: Default::default(),
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
            _state: std::marker::PhantomData::<Open>,
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
            _state: std::marker::PhantomData::<Closed>,
        }
    }
    pub fn print_bar(&self, mut current: [usize; N], total: usize, post_msg: &str) {
        current[N - 1] = current[N - 1].max(0);
        for i in N - 1..0 {
            current[i + 1] = current[i + 1].min(current[i])
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

    #[test]
    fn empty_arrow() {
        assert_eq!(
            Arrow::default().build([0.0], 10),
            String::from("[>         ]")
        )
    }
    #[test]
    fn short_arrow() {
        assert_eq!(
            Arrow::default().build([0.2], 10),
            String::from("[==>       ]")
        )
    }
    #[test]
    fn long_arrow() {
        assert_eq!(
            Arrow::default().build([0.9], 10),
            String::from("[=========>]")
        )
    }
    #[test]
    fn full_arrow() {
        assert_eq!(
            Arrow::default().build([1.0], 10),
            String::from("[==========]")
        )
    }

    #[test]
    fn double_arrow() {
        assert_eq!(
            Arrow::default().build([0.3, 0.5], 10),
            String::from("[===-->    ]")
        );
    }
}
