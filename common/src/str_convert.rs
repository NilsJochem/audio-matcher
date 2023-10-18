//! A module for converting the case of strings
use itertools::Itertools;
use std::{borrow::Cow, collections::HashSet};
use thiserror::Error;

use crate::extensions::iter::CloneIteratorExt;

/// Different Cases a Word can be in
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WordCase {
    /// All characters are lowercase, or don't have a case.
    /// Lower case is definded by [`char::is_lowercase`]
    Lower,
    /// All characters are uppercase, or don't have a case.
    /// Upper case is definded by [`char::is_uppercase`]
    Upper,
    /// The first character is uppercase or doesn't have a case, the rest are lowercase or don't have a case.
    /// Lower/Upper case is defined by [`WordCase::Lower`] / [`WordCase::Upper`]
    Capitalized,
}
impl WordCase {
    #[inline]
    fn word_not_in_case(self, word: &str) -> bool {
        match self {
            Self::Lower => word.chars().any(char::is_uppercase),
            Self::Upper => word.chars().any(char::is_lowercase),
            Self::Capitalized => {
                !word.is_empty()
                    && (Self::Upper.word_not_in_case(&word[..1])
                        || Self::Lower.word_not_in_case(&word[1..]))
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
            Some(case) if case.word_not_in_case(&word) => {
                *has_changed = true;
                case.convert(word)
            }
            None | Some(_) => word,
        }
    }
}

/// Different Cases a sequence of words can be in
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Case {
    /// The fist word is Lowercase and the rest are Capitalized. There is no seperator.
    Camel,
    /// Each Word is in `case` and seperated by `delimitor` if it is Some.
    Other {
        /// The [`WordCase`] for each word. May be None to indicate mixed case
        case: Option<WordCase>,
        /// The word seperator if existing
        seperator: Option<char>,
    },
}
impl Case {
    /// All Words are capitalized with no seperator
    #[allow(non_upper_case_globals)]
    pub const Pascal: Self = Self::Other {
        case: Some(WordCase::Capitalized),
        seperator: None,
    };
    /// All Words are lowercase and seperatet by '_'
    #[allow(non_upper_case_globals)]
    pub const Snake: Self = Self::Other {
        case: Some(WordCase::Lower),
        seperator: Some('_'),
    };
    /// All Words are uppercase and seperatet by '_'
    #[allow(non_upper_case_globals)]
    pub const ScreamingSnake: Self = Self::Other {
        case: Some(WordCase::Upper),
        seperator: Some('_'),
    };
    /// All Words are lower case and seperatet by '-'
    #[allow(non_upper_case_globals)]
    pub const Kebab: Self = Self::Other {
        case: Some(WordCase::Lower),
        seperator: Some('-'),
    };
    /// All Words are lower case and seperatet by ' '
    #[allow(non_upper_case_globals)]
    pub const Upper: Self = Self::Other {
        case: Some(WordCase::Upper),
        seperator: Some(' '),
    };
    /// All Words are lower case and seperatet by ' '
    #[allow(non_upper_case_globals)]
    pub const Lower: Self = Self::Other {
        case: Some(WordCase::Lower),
        seperator: Some(' '),
    };

    /// creates a new [`WordCase`].
    /// creation with [`WordCase::case`] = `None` is not intendet
    #[inline]
    pub fn new(case: WordCase, seperator: impl Into<Option<char>>) -> Self {
        Self::Other {
            case: Some(case),
            seperator: seperator.into(),
        }
    }

    fn split(seperator: impl Into<Option<char>>, data: &str) -> Vec<Cow<'_, str>> {
        seperator.into().map_or_else(
            || Self::split_capitalized(data),
            |seperator| Self::split_seperator(data, seperator),
        )
    }
    fn split_seperator(data: &str, seperator: char) -> Vec<Cow<'_, str>> {
        data.split(seperator).map(Cow::Borrowed).collect_vec()
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
    const fn seperator(self) -> Option<char> {
        match self {
            Self::Camel => None,
            Self::Other { seperator, .. } => seperator,
        }
    }
}

/// A String representation to easily change the case
/// holds a reference to the original data so that no new string needs to be created, when nothing changed
#[derive(Debug, Clone)]
pub struct CapitalizedString<'a> {
    original_data: Option<&'a str>,
    words: Vec<Cow<'a, str>>,
    case: Case,
}

impl<'a> CapitalizedString<'a> {
    /// splits `data` at `seperator` if `Some` or at capitalized letters if `None`
    pub fn new(data: &'a str, seperator: impl Into<Option<char>>) -> Self {
        let case = match seperator.into() {
            Some(seperator) => Case::Other {
                case: None,
                seperator: Some(seperator),
            },
            None if data.is_empty() => Case::Lower,
            None => {
                let mut contains_lower = false;
                let mut contains_upper = false;

                let first = data.chars().next().unwrap();
                let is_first_upper = first.is_uppercase();
                if is_first_upper {
                    contains_upper = true;
                } else if first.is_lowercase() {
                    contains_lower = true;
                };

                for char in data.chars() {
                    contains_lower |= char.is_lowercase();
                    contains_upper |= char.is_uppercase();
                    if contains_lower && contains_upper {
                        break; // nothing more can be gained by checking the rest
                    }
                }
                match (is_first_upper, contains_lower, contains_upper) {
                    (_, false | true, false) => Case::Lower,
                    (_, false, true) => Case::Upper,
                    (true, true, true) => Case::Pascal,
                    (false, true, true) => Case::Camel,
                }
            }
        };
        let split = Case::split(case.seperator(), data);
        Self::from_words_unchecked(data, split, case)
    }
    /// Creates a new `CapitaliedString` from `words` and `seperator`
    pub fn from_words<Iter>(words: Iter, seperator: impl Into<Option<char>>) -> Self
    where
        Iter: IntoIterator,
        Iter::Item: Into<Cow<'a, str>>,
    {
        Self::from_words_unchecked(
            None,
            words,
            Case::Other {
                case: None,
                seperator: seperator.into(),
            },
        )
    }
    fn from_words_unchecked<Iter>(
        original_data: impl Into<Option<&'a str>>,
        words: Iter,
        case: Case,
    ) -> Self
    where
        Iter: IntoIterator,
        Iter::Item: Into<Cow<'a, str>>,
    {
        Self {
            original_data: original_data.into(),
            words: words.into_iter().map(Iter::Item::into).collect_vec(),
            case,
        }
    }

    /// Parses `data` and changes its case to `into_case`
    ///
    /// # Errors
    /// relays [`MixedSeperators`] from [`Self::Try_from::<&str>`]
    #[inline]
    pub fn new_into(data: &'a str, into_case: Case) -> Result<Self, MixedSeperators> {
        Self::try_from(data).map(|it| it.into_case(into_case))
    }
    /// A chainable variant of [`CapatizedString::change_case`]
    #[inline]
    pub fn into_case(mut self, case: Case) -> Self {
        self.change_case(case);
        self
    }
    /// Changes the case of `self` to `case`
    /// keeps old references if nothing needs to be changed
    pub fn change_case(&mut self, case: Case) {
        if self.case == case {
            return;
        }
        let data = std::mem::take(&mut self.words);
        let (changed, data) = case.convert(data);
        if changed || (self.words.len() > 1 && self.case.seperator() != case.seperator()) {
            // remove if some data was changed, or a deliminator would change (there are at least two words and a differend deliminator)
            self.original_data = None;
        }
        self.words = data;
        self.case = case;
    }

    /// Copys all borrowed data to become an owned type
    /// sadly can't be expressed by [`alloc::borrow::ToOwned`]
    pub fn into_owned(self) -> CapitalizedString<'static> {
        CapitalizedString::from_words_unchecked(
            None,
            self.words
                .into_iter()
                .map(|it| Cow::Owned(it.into_owned()))
                .collect_vec(),
            self.case,
        )
    }
}
impl<'a> From<&CapitalizedString<'a>> for Cow<'a, str> {
    fn from(value: &CapitalizedString<'a>) -> Self {
        value.original_data.map_or_else(
            || {
                let seperator = value.case.seperator().map(String::from);
                let sep = seperator.as_deref().unwrap_or("");
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

/// an error denoting that different Seperators where found. Expected delemiters are ' ', '-' and '_'
#[derive(Debug, Error, PartialEq, Eq)]
#[error("mixed seperator, found, {0:?}")]
pub struct MixedSeperators(HashSet<char>);
impl<'a> TryFrom<&'a str> for CapitalizedString<'a> {
    type Error = MixedSeperators;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        const DELIMITERS: [char; 3] = [' ', '-', '_'];
        let candidates = value
            .chars()
            .filter(|char| DELIMITERS.contains(char))
            .collect::<HashSet<_>>();
        let seperator = match candidates.len() {
            0 => None,
            1 => Some(candidates.into_iter().next().unwrap()),
            _ => return Err(MixedSeperators(candidates)),
        };
        Ok(CapitalizedString::new(value, seperator))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_correctly() {
        fn __test_to_string(data: &str, words: Vec<&str>, case: Case) {
            let mut s = CapitalizedString::new(data, case.seperator());
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
                CapitalizedString::from_words(words, case.seperator()).to_string(),
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
            "testwithoutseperator",
            vec!["testwithoutseperator"],
            Case::new(WordCase::Lower, None),
        );
        __test_to_string(
            "TestWithoutSeperator",
            vec!["Test", "Without", "Seperator"],
            Case::Pascal,
        );
        __test_to_string(
            "testWithoutSeperator",
            vec!["test", "Without", "Seperator"],
            Case::Camel,
        );
    }

    #[test]
    fn some_extra() {
        fn format(s: &str, case: Case) -> String {
            CapitalizedString::new_into(s, case).unwrap().to_string()
        }
        assert_eq!("Abc", format("abc", Case::Pascal));
        assert_eq!("Abc", format("Abc", Case::Pascal));
        assert_eq!("Abc", format("ABC", Case::Pascal));
        assert_eq!("Abc", format("_aBc", Case::Pascal));
        assert_eq!("AbCd", format("aB_CD", Case::Pascal));
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
        let orig = "datawithoutseperator!";
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
