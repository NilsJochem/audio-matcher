use std::fmt::Debug;

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
    #[allow(dead_code)]
    pub(crate) fn unicode_grayscale() -> Self {
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
