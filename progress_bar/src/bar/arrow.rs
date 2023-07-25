use std::fmt::Debug;

pub trait Arrow<const N: usize>: Debug {
    fn build(&self, fractions: [f64; N], bar_length: usize) -> String;
    fn padding_needed(&self) -> usize;
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Simple<const N: usize> {
    arrow_prefix: &'static str,
    arrow_suffix: &'static str,
    base_char: char,
    arrow_chars: [char; N],
    arrow_tip: &'static str,
}
#[must_use]
pub struct UnicodeBar(char, char);
#[allow(non_snake_case, dead_code)]
impl UnicodeBar {
    pub const fn Rising() -> Self {
        Self('\u{2588}', '\u{2581}')
    }
    pub const fn Box() -> Self {
        Self('\u{25a0}', '\u{25a1}')
    }
    pub const fn Circle() -> Self {
        Self('\u{2b24}', '\u{25ef}')
    }
    pub const fn Parallelogramm() -> Self {
        Self('\u{25b0}', '\u{25b1}')
    }
}
impl From<UnicodeBar> for Simple<1> {
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
impl Simple<2> {
    #[allow(dead_code)]
    pub(crate) const fn unicode_grayscale() -> Self {
        Self {
            arrow_prefix: "",
            arrow_suffix: "",
            base_char: '\u{2592}',
            arrow_chars: ['\u{2588}', '\u{2593}'],
            arrow_tip: "",
        }
    }
}

impl Default for Simple<1> {
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
impl Default for Simple<2> {
    fn default() -> Self {
        Self {
            arrow_chars: ['=', '-'],
            base_char: Simple::<1>::default().base_char,
            arrow_prefix: Simple::<1>::default().arrow_prefix,
            arrow_suffix: Simple::<1>::default().arrow_suffix,
            arrow_tip: Simple::<1>::default().arrow_tip,
        }
    }
}
impl<const N: usize> Arrow<N> for Simple<N> {
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
pub struct Fancy {
    empty_bar: [char; 3],
    full_bar: [char; 3],
}
impl Default for Fancy {
    /// uses fira typeset to print connected progress bar
    fn default() -> Self {
        Self {
            empty_bar: ['\u{ee00}', '\u{ee01}', '\u{ee02}'],
            full_bar: ['\u{ee03}', '\u{ee04}', '\u{ee05}'],
        }
    }
}
// just use the last bar
impl<const N: usize> Arrow<N> for Fancy {
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
                Simple::default().build([0.0], 12),
                String::from("[>         ]")
            )
        }
        #[test]
        fn short_arrow() {
            assert_eq!(
                Simple::default().build([0.2], 12),
                String::from("[==>       ]")
            )
        }
        #[test]
        fn long_arrow() {
            assert_eq!(
                Simple::default().build([0.9], 12),
                String::from("[=========>]")
            )
        }
        #[test]
        fn full_arrow() {
            assert_eq!(
                Simple::default().build([1.0], 12),
                String::from("[==========]")
            )
        }

        #[test]
        fn double_arrow() {
            assert_eq!(
                Simple::default().build([0.3, 0.5], 12),
                String::from("[===-->    ]")
            );
        }
    }
    mod fancy_arrow {
        use super::*;

        fn ascci_arrow() -> Fancy {
            Fancy {
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
