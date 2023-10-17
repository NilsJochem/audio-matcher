#![allow(missing_docs)]
use itertools::Itertools;
use std::{borrow::Cow, collections::HashSet};
use thiserror::Error;

use crate::extensions::iter::CloneIteratorExt;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("mixed delimiter, found, {0:?}")]
    MixedDelimiter(HashSet<char>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WordCase {
    Lower,
    Upper,
    Capitalized,
}
impl WordCase {
    fn word_in_case(self, word: &str) -> bool {
        match self {
            Self::Lower => word.chars().all(char::is_lowercase),
            Self::Upper => word.chars().all(char::is_uppercase),
            Self::Capitalized => {
                word.is_empty()
                    || Self::Upper.word_in_case(&word[..1]) && Self::Lower.word_in_case(&word[1..])
            }
        }
    }
    #[momo::momo]
    #[allow(clippy::needless_lifetimes)]
    fn convert<'a>(self, word: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
        if word.is_empty() {
            return word;
        }
        match self {
            Self::Lower => Cow::Owned(word.to_lowercase()),
            Self::Upper => Cow::Owned(word.to_uppercase()),
            Self::Capitalized => {
                let mut new_word = word[..1].to_uppercase();
                new_word.push_str(&word[1..].to_lowercase());
                Cow::Owned(new_word)
            }
        }
    }

    fn conver_if_needed<'a>(
        case: Option<Self>,
        word: Cow<'a, str>,
        has_changed: &mut bool,
    ) -> Cow<'a, str> {
        match case {
            Some(case) if !case.word_in_case(&word) => {
                *has_changed = true;
                case.convert(word)
            }
            None | Some(_) => word,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Case {
    Camel,
    Other {
        case: Option<WordCase>,
        delimiter: Option<char>,
    },
}
impl Case {
    #[allow(non_upper_case_globals)]
    pub const Pascal: Self = Self::Other {
        case: Some(WordCase::Capitalized),
        delimiter: None,
    };
    #[allow(non_upper_case_globals)]
    pub const Snake: Self = Self::Other {
        case: Some(WordCase::Lower),
        delimiter: Some('_'),
    };
    #[allow(non_upper_case_globals)]
    pub const ScreamingSnake: Self = Self::Other {
        case: Some(WordCase::Upper),
        delimiter: Some('_'),
    };
    #[allow(non_upper_case_globals)]
    pub const Kebab: Self = Self::Other {
        case: Some(WordCase::Lower),
        delimiter: Some('-'),
    };
    #[allow(non_upper_case_globals)]
    pub const Upper: Self = Self::Other {
        case: Some(WordCase::Upper),
        delimiter: Some(' '),
    };
    #[allow(non_upper_case_globals)]
    pub const Lower: Self = Self::Other {
        case: Some(WordCase::Lower),
        delimiter: Some(' '),
    };

    #[inline]
    pub fn new(case: WordCase, delimiter: impl Into<Option<char>>) -> Self {
        Self::Other {
            case: Some(case),
            delimiter: delimiter.into(),
        }
    }

    unsafe fn split(self, data: &str) -> Vec<Cow<'_, str>> {
        #[allow(clippy::match_same_arms)]
        match self {
            Self::Camel => Self::split_capitalized(data),
            Self::Other {
                case: _,
                delimiter: Some(delimiter),
            } => Self::split_delimiter(data, delimiter),
            Self::Other {
                case: Some(WordCase::Capitalized),
                delimiter: None,
            } => Self::split_capitalized(data),
            Self::Other {
                case: Some(WordCase::Lower | WordCase::Upper) | None,
                delimiter: None,
            } => Self::no_split(data),
        }
    }
    fn no_split(data: &str) -> Vec<Cow<'_, str>> {
        vec![Cow::Borrowed(data)]
    }
    fn split_delimiter(data: &str, delimiter: char) -> Vec<Cow<'_, str>> {
        data.split(delimiter).map(Cow::Borrowed).collect_vec()
    }
    fn split_capitalized(data: &str) -> Vec<Cow<'_, str>> {
        data.match_indices(char::is_uppercase)
            .open_border_pairs()
            .filter_map(|it| {
                match it {
                    crate::extensions::iter::State::Start((e, _)) => (e != 0).then(|| &data[..e]),
                    crate::extensions::iter::State::Middle((s, _), (e, _)) => Some(&data[s..e]),
                    crate::extensions::iter::State::End((s, _)) => Some(&data[s..]),
                }
                .map(Cow::Borrowed)
            })
            .collect::<Vec<_>>()
    }

    fn convert<'a>(
        self,
        data: impl IntoIterator<Item = Cow<'a, str>>,
    ) -> (bool, Vec<Cow<'a, str>>) {
        match self {
            Self::Camel => {
                let mut has_changed = false;
                let mut data = data.into_iter();
                let vec = data
                    .next()
                    .map(|it| (it, WordCase::Lower)) // first element is Lowercase
                    .into_iter()
                    .chain(data.map(|it| (it, WordCase::Capitalized))) // other are Capitalized
                    .map(|(it, case)| WordCase::conver_if_needed(Some(case), it, &mut has_changed))
                    .collect_vec();
                (has_changed, vec)
            }
            Self::Other { case, .. } => {
                let mut has_changed = false;
                let vec = data
                    .into_iter()
                    .map(|it| WordCase::conver_if_needed(case, it, &mut has_changed))
                    .collect_vec();
                (has_changed, vec)
            }
        }
    }
    const fn delimiter(self) -> Option<char> {
        match self {
            Self::Camel => None,
            Self::Other { delimiter, .. } => delimiter,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapitalizedString<'a> {
    original_data: Option<&'a str>,
    words: Vec<Cow<'a, str>>,
    case: Case,
}

impl<'a> CapitalizedString<'a> {
    pub fn new(data: &'a str, delimiter: impl Into<Option<char>>) -> Self {
        let case = match delimiter.into() {
            Some(delimiter) => Case::Other {
                case: None,
                delimiter: Some(delimiter),
            },
            None if data.is_empty() => Case::Lower,
            None => {
                let mut contains_lower = false;
                let mut contains_upper = false;
                let first = data.chars().next().unwrap();
                let is_first_lower = if first.is_lowercase() {
                    contains_lower = true;
                    Some(true)
                } else if first.is_uppercase() {
                    contains_upper = true;
                    Some(false)
                } else {
                    None
                };
                for char in data.chars() {
                    contains_lower |= char.is_lowercase();
                    contains_upper |= char.is_uppercase();
                    if contains_lower && contains_upper {
                        break; // nothing more can be gained by checking the rest
                    }
                }
                match (is_first_lower, contains_lower, contains_upper) {
                    (_, false | true, false) => Case::Lower,
                    (_, false, true) => Case::Upper,
                    (Some(false), true, true) => Case::Pascal,
                    (Some(true) | None, true, true) => Case::Camel,
                }
            }
        };
        let split = unsafe { case.split(data) };
        unsafe { Self::from_words_unchecked(data, split, case) }
    }
    pub fn from_words<Iter>(words: Iter, delimiter: impl Into<Option<char>>) -> Self
    where
        Iter: IntoIterator,
        Iter::Item: Into<Cow<'a, str>>,
    {
        unsafe {
            Self::from_words_unchecked(
                None,
                words,
                Case::Other {
                    case: None,
                    delimiter: delimiter.into(),
                },
            )
        }
    }
    unsafe fn from_words_unchecked<Iter>(
        original_data: impl Into<Option<&'a str>>,
        words: Iter,
        case: Case,
    ) -> Self
    where
        Iter: IntoIterator,
        Iter::Item: Into<Cow<'a, str>>,
    {
        let words = words.into_iter().map(Iter::Item::into).collect_vec();
        Self {
            original_data: original_data.into(),
            words,
            case,
        }
    }

    pub fn convert(data: &'a str, into_case: Case) -> Result<Self, ParseError> {
        Self::try_from(data).map(|it| it.into_case(into_case))
    }
    pub fn into_case(mut self, case: Case) -> Self {
        self.change_case(case);
        self
    }
    pub fn change_case(&mut self, case: Case) {
        if self.case == case {
            return;
        }
        let data = std::mem::take(&mut self.words);
        let (changed, data) = case.convert(data);
        if changed || (self.words.len() > 1 && self.case.delimiter() != case.delimiter()) {
            // remove if some data was changed, or a deliminator would change (there are at least two words and a differend deliminator)
            self.original_data = None;
        }
        self.words = data;
        self.case = case;
    }
}
impl<'a> From<&CapitalizedString<'a>> for Cow<'a, str> {
    fn from(value: &CapitalizedString<'a>) -> Self {
        value.original_data.map_or_else(
            || {
                let delimiter = value.case.delimiter().map(String::from);
                let sep = delimiter.as_deref().unwrap_or("");
                Cow::Owned(value.words.iter().join(sep))
            },
            Cow::Borrowed,
        )
    }
}
impl<'a> ToString for CapitalizedString<'a> {
    fn to_string(&self) -> String {
        Cow::from(self).into_owned()
    }
}
impl<'a> TryFrom<&'a str> for CapitalizedString<'a> {
    type Error = ParseError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        const DELIMITERS: [char; 3] = [' ', '-', '_'];
        let candidates = value
            .chars()
            .filter(|char| DELIMITERS.contains(char))
            .collect::<HashSet<_>>();
        let delimiter = match candidates.len() {
            0 => None,
            1 => Some(candidates.into_iter().exactly_one().unwrap()),
            _ => return Err(ParseError::MixedDelimiter(candidates)),
        };
        Ok(CapitalizedString::new(value, delimiter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_correctly() {
        fn __test_to_string(data: &str, words: Vec<&str>, case: Case) {
            let mut s = CapitalizedString::new(data, case.delimiter());
            assert_eq!(words, s.words, "failed to seperate words with {case:?}");
            s.change_case(case);
            assert!(
                s.words.iter().all(|it| matches!(it, Cow::Borrowed(_))),
                "failed to borrow for {case:?}"
            );
            assert_eq!(
                Some(data),
                s.original_data,
                "failed to save original_data for {case:?}"
            );
            assert_eq!(
                data,
                CapitalizedString::from_words(words, case.delimiter()).to_string(),
                "failed to join words with {case:?}"
            );
        }

        __test_to_string("", vec![""], Case::Lower);
        __test_to_string(
            "test with spaces",
            vec!["test", "with", "spaces"],
            Case::Lower,
        );
        __test_to_string(
            "test_with_underscores",
            vec!["test", "with", "underscores"],
            Case::Snake,
        );
        __test_to_string(
            "testwithoutdelimiter",
            vec!["testwithoutdelimiter"],
            Case::new(WordCase::Lower, None),
        );
        __test_to_string(
            "TestWithoutDelimiter",
            vec!["Test", "Without", "Delimiter"],
            Case::Pascal,
        );
        __test_to_string(
            "testWithoutDelimiter",
            vec!["test", "Without", "Delimiter"],
            Case::Camel,
        );
    }

    #[test]
    fn some_extra() {
        fn format(s: &str) -> String {
            CapitalizedString::convert(s, Case::Pascal)
                .unwrap()
                .to_string()
        }
        assert_eq!("Abc", format("abc"));
        assert_eq!("Abc", format("Abc"));
        assert_eq!("Abc", format("ABC"));
        assert_eq!("Abc", format("_aBc"));
        assert_eq!("AbCd", format("aB_CD"));
    }

    #[test]
    fn from_words() {
        let data = vec!["test", "with", "spaces"];

        assert_eq!(
            data,
            CapitalizedString::from_words(data.clone(), None).words,
            "failed with borrowed"
        );
        assert_eq!(
            data,
            CapitalizedString::from_words(data.iter().map(|it| it.to_owned()), None).words,
            "failed with owned"
        );
    }
    #[test]
    fn convert() {
        let mut data = CapitalizedString::new("some data", ' ');
        data.change_case(Case::Upper);
        assert_eq!("SOME DATA", data.to_string());
        data.change_case(Case::Snake);
        assert_eq!("some_data", data.to_string());
        data.change_case(Case::Pascal);
        assert_eq!("SomeData", data.to_string());
        data.change_case(Case::Kebab);
        assert_eq!("some-data", data.to_string());
        data.change_case(Case::Camel);
        assert_eq!("someData", data.to_string());
        data.change_case(Case::Lower);
        assert_eq!("some data", data.to_string());
    }

    #[test]
    fn convert_no_extra_allocation() {
        let orig = "datawithoutdelimiter";
        let mut data = CapitalizedString::new(orig, ' ');
        data.change_case(Case::Kebab);
        assert_eq!(Some(orig), data.original_data);
        data.change_case(Case::Lower);
        assert_eq!(Some(orig), data.original_data);
    }

    #[test]
    fn detect() {
        let mut data = CapitalizedString::try_from("some data with spaces").unwrap();
        data.change_case(Case::new(WordCase::Capitalized, Some('-')));
        assert_eq!("Some-Data-With-Spaces", data.to_string());
        let mut data = CapitalizedString::try_from("SomeDataWithoutSpaces").unwrap();
        data.change_case(Case::Kebab);
        assert_eq!("some-data-without-spaces", data.to_string());
    }

    #[test]
    fn detect_no_extra_allocation() {
        let orig = "SomeDataWithoutSpaces";
        let mut data = CapitalizedString::try_from(orig).unwrap();
        data.change_case(Case::Pascal);
        assert_eq!(Some(orig), data.original_data);
    }
}
