use itertools::Itertools;
use pad::PadStr;
use std::io::{stdout, Write};

pub struct Arrow<'a, const N: usize> {
    pub arrow_prefix: &'a str,
    pub arrow_suffix: &'a str,
    pub arrow_chars: [char; N],
    pub arrow_tip: char,
}

impl Default for Arrow<'_, 1> {
    fn default() -> Self {
        Self {
            arrow_prefix: "[",
            arrow_suffix: "]",
            arrow_chars: ['='],
            arrow_tip: '>',
        }
    }
}
impl Default for Arrow<'_, 2> {
    fn default() -> Self {
        Self {
            arrow_chars: ['=', '-'],
            arrow_prefix: Arrow::<'_, 1>::default().arrow_prefix,
            arrow_suffix: Arrow::<'_, 1>::default().arrow_suffix,
            arrow_tip: Arrow::<'_, 1>::default().arrow_tip,
        }
    }
}

impl<const N: usize> Arrow<'_, N> {
    fn build(&self, fractions: [f64; N], bar_length: usize) -> String {
        let mut arrow = String::new();
        arrow.push_str(self.arrow_prefix);

        for i in 0..N {
            let fraction = fractions[i];
            let char = self.arrow_chars[i];
            arrow.push_str(&char.to_string().repeat(
                (fraction * bar_length as f64).round() as usize
                    - (arrow.len() - self.arrow_prefix.len()),
            ));
        }

        if bar_length - (arrow.len() - self.arrow_prefix.len()) > 0 {
            arrow.push(self.arrow_tip);
        }
        arrow.push_str(
            " ".repeat(bar_length - (arrow.len() - self.arrow_prefix.len()))
                .as_str(),
        );
        arrow.push_str(self.arrow_suffix);
        arrow
    }
}

pub struct ProgressBar<'a, const N: usize> {
    pub bar_length: usize,
    pub pre_msg: &'a str,
    pub arrow: Arrow<'a, N>,
}

impl Default for ProgressBar<'_, 1> {
    fn default() -> Self {
        Self {
            bar_length: 20,
            pre_msg: "Progress: ",
            arrow: Arrow::default(),
        }
    }
}
impl Default for ProgressBar<'_, 2> {
    fn default() -> Self {
        Self {
            bar_length: 20,
            pre_msg: "Progress: ",
            arrow: Arrow::default(),
        }
    }
}

impl<const N: usize> ProgressBar<'_, N> {
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
        let start = if current[N - 1] == 0 { "" } else { "\r" };
        let ending = if current[0] == total { "\n" } else { "" };

        crate::leveled_output::print(
            crate::leveled_output::OutputLevel::Info,
            &format!(
                "{start}{}{} {current_fmt}/{}{}{ending}",
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
