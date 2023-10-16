#![warn(
    clippy::nursery,
    clippy::pedantic,
    clippy::empty_structs_with_brackets,
    clippy::format_push_string,
    clippy::if_then_some_else_none,
    clippy::impl_trait_in_params,
    clippy::missing_assert_message,
    clippy::multiple_inherent_impl,
    clippy::non_ascii_literal,
    clippy::self_named_module_files,
    clippy::semicolon_inside_block,
    clippy::separated_literal_suffix,
    clippy::str_to_string,
    clippy::string_to_string,
    missing_docs
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::single_match_else,
    clippy::return_self_not_must_use,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate
)]
//! some common functionalitys

pub mod boo;
/// a collection for extionsion functions
pub mod extensions {
    ///extention functions for [`std::borrow::Cow`]
    pub mod cow;
    ///extention functions for [`std::time::Duration`]
    pub mod duration;
    /// extention function for Iterators
    pub mod iter;
    ///extention functions for [`Option`]
    pub mod option;
    ///extention functions for [`Vec`]
    pub mod vec;
}
pub mod io;
pub mod rc;

#[allow(missing_docs)]
pub mod str_convert {
    use itertools::Itertools;
    use std::borrow::Cow;
    use thiserror::Error;

    use crate::extensions::iter::CloneIteratorExt;

    #[derive(Debug, Error, PartialEq, Eq)]
    pub enum ParseError {
        #[error("mixed delimiter, found, {0:?}")]
        MixedDelimitor(Vec<char>),
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum WordCase {
        Lower,
        Upper,
        Capitalized,
        Mixed,
    }
    impl WordCase {
        fn is_case(self, data: &str) -> bool {
            match self {
                Self::Mixed => true,
                Self::Lower => data.chars().all(char::is_lowercase),
                Self::Upper => data.chars().all(char::is_uppercase),
                Self::Capitalized => {
                    data.is_empty()
                        || Self::Upper.is_case(&data[..1]) && Self::Lower.is_case(&data[1..])
                }
            }
        }
        #[momo::momo]
        #[allow(clippy::needless_lifetimes)]
        fn convert<'a>(self, data: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
            match self {
                Self::Mixed => data,
                Self::Lower => match data {
                    Cow::Borrowed(data) => Cow::Owned(data.to_lowercase()),
                    Cow::Owned(mut data) => {
                        data.make_ascii_lowercase();
                        Cow::Owned(data)
                    }
                },
                Self::Upper => match data {
                    Cow::Borrowed(data) => Cow::Owned(data.to_uppercase()),
                    Cow::Owned(mut data) => {
                        data.make_ascii_uppercase();
                        Cow::Owned(data)
                    }
                },
                Self::Capitalized => {
                    if data.is_empty() {
                        return data;
                    }
                    let mut data = data.into_owned();
                    data[..1].make_ascii_uppercase();
                    data[1..].make_ascii_lowercase();
                    Cow::Owned(data)
                }
            }
        }

        fn conver_if_needed<'a>(self, it: Cow<'a, str>, has_changed: &mut bool) -> Cow<'a, str> {
            if self.is_case(&it) {
                it
            } else {
                *has_changed = true;
                self.convert(it)
            }
        }
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Case {
        Camel,
        Other {
            case: WordCase,
            delimiter: Option<char>,
        },
    }
    impl Case {
        #[allow(non_upper_case_globals)]
        pub const Pascal: Self = Self::Other {
            case: WordCase::Capitalized,
            delimiter: None,
        };
        #[allow(non_upper_case_globals)]
        pub const Snake: Self = Self::Other {
            case: WordCase::Lower,
            delimiter: Some('_'),
        };
        #[allow(non_upper_case_globals)]
        pub const ScreamingSnake: Self = Self::Other {
            case: WordCase::Upper,
            delimiter: Some('_'),
        };
        #[allow(non_upper_case_globals)]
        pub const Kebab: Self = Self::Other {
            case: WordCase::Lower,
            delimiter: Some('-'),
        };
        #[allow(non_upper_case_globals)]
        pub const Upper: Self = Self::Other {
            case: WordCase::Upper,
            delimiter: Some(' '),
        };
        #[allow(non_upper_case_globals)]
        pub const Lower: Self = Self::Other {
            case: WordCase::Lower,
            delimiter: Some(' '),
        };

        fn split(self, data: &str) -> Vec<Cow<'_, str>> {
            #[allow(clippy::match_same_arms)] // TODO add validation
            match self {
                Self::Camel => Self::split_capitalized(data),
                Self::Other {
                    case: _,
                    delimiter: Some(delimiter),
                } => Self::split_delimiter(data, delimiter),
                Self::Other {
                    case: WordCase::Capitalized,
                    delimiter: None,
                } => Self::split_capitalized(data),
                Self::Other {
                    case: WordCase::Lower | WordCase::Upper | WordCase::Mixed,
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
                        crate::extensions::iter::State::Start((e, _)) => {
                            (e != 0).then(|| &data[..e])
                        }
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
                        .map(|(it, case)| case.conver_if_needed(it, &mut has_changed))
                        .collect_vec();
                    (has_changed, vec)
                }
                Self::Other { case, .. } => {
                    let mut has_changed = false;
                    let vec = data
                        .into_iter()
                        .map(|it| case.conver_if_needed(it, &mut has_changed))
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
        data: Vec<Cow<'a, str>>,
        case: Case,
    }

    impl<'a> CapitalizedString<'a> {
        pub fn new(data: &'a str, case: Case) -> Self {
            let split = case.split(data);
            // if split.iter().any(|it| !case.is_case(&it)) {
            //     todo!("started as wrong case")
            // }
            Self {
                original_data: Some(data),
                data: split,
                case,
            }
        }
        pub fn convert(data: &'a str, into_case: Case) -> Result<Self, ParseError> {
            let mut tmp = Self::try_from(data)?;
            tmp.change_case(into_case);
            Ok(tmp)
        }
        pub fn from_words<Iter>(data: Iter, case: Case) -> Self
        where
            Iter: IntoIterator,
            Iter::Item: Into<Cow<'a, str>>,
        {
            Self {
                original_data: None,
                data: data.into_iter().map(Iter::Item::into).collect_vec(),
                case,
            }
        }
        pub fn change_case(&mut self, case: Case) {
            if self.case == case {
                return;
            }
            let data = std::mem::take(&mut self.data);
            let (changed, data) = case.convert(data);
            if changed || (self.data.len() > 1 && self.case.delimiter() != case.delimiter()) {
                // remove if some data was changed, or a deliminator would change (there are at least to words and a differend deliminator)
                self.original_data = None;
            }
            self.data = data;
            self.case = case;
        }
    }
    impl<'a> From<&CapitalizedString<'a>> for Cow<'a, str> {
        fn from(value: &CapitalizedString<'a>) -> Self {
            value.original_data.map_or_else(
                || {
                    let delimiter = value.case.delimiter().map(String::from);
                    let sep = delimiter.as_deref().unwrap_or("");
                    Cow::Owned(value.data.iter().join(sep))
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
            let delimiter = {
                let candidates = value
                    .chars()
                    .filter(|char| DELIMITERS.contains(char))
                    .unique()
                    .collect_vec();
                match candidates.as_slice() {
                    [] => None,
                    [x] => Some(*x),
                    _ => return Err(ParseError::MixedDelimitor(candidates)),
                }
            };
            let case = match delimiter {
                Some(_) => WordCase::Mixed,
                None => {
                    let mut contains_lower = false;
                    let mut contains_upper = false;
                    for char in value.chars() {
                        contains_lower |= char.is_lowercase();
                        contains_upper |= char.is_uppercase();
                        if contains_lower && contains_upper {
                            break;
                        }
                    }
                    match (contains_lower, contains_upper) {
                        (false | true, false) => WordCase::Lower,
                        (false, true) => WordCase::Upper,
                        (true, true) => WordCase::Capitalized,
                    }
                }
            };
            Ok(CapitalizedString::new(
                value,
                Case::Other { case, delimiter },
            ))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn new_splits_correct() {
            __test_from_str(vec![""], Case::Lower);
            __test_from_str(vec!["test", "with", "spaces"], Case::Lower);
            __test_from_str(vec!["test", "with", "underscores"], Case::Lower);
            __test_from_str(
                vec!["testwithoutdelimitor"],
                Case::Other {
                    case: WordCase::Lower,
                    delimiter: None,
                },
            );
            __test_from_str(vec!["Test", "Without", "Delimitor"], Case::Pascal);
            __test_from_str(vec!["test", "Without", "Delimitor"], Case::Camel);
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
            let case = Case::Lower;

            assert_eq!(
                data,
                CapitalizedString::from_words(data.clone(), case).data,
                "failed with borrowed"
            );
            assert_eq!(
                data,
                CapitalizedString::from_words(data.iter().map(|it| it.to_owned()), case).data,
                "failed with owned"
            );
        }

        #[test]
        fn format_correctly() {
            __test_to_string("test with spaces", Case::Lower);
            __test_to_string("test_with_underscores", Case::Lower);
            __test_to_string(
                "testwithoutdelimitor",
                Case::Other {
                    case: WordCase::Lower,
                    delimiter: None,
                },
            );
            __test_to_string("TestWithoutDelimitor", Case::Pascal);
            __test_to_string("testWithoutDelimitor", Case::Camel);
        }

        #[test]
        fn convert() {
            let mut data = CapitalizedString::new("some data", Case::Lower);
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
            let orig = "datawithoutdelimitor";
            let mut data = CapitalizedString::new(orig, Case::Lower);
            data.change_case(Case::Kebab);
            assert_eq!(Some(orig), data.original_data);
        }

        #[test]
        fn detect() {
            let mut data = CapitalizedString::try_from("some data with spaces").unwrap();
            data.change_case(Case::Other {
                case: WordCase::Capitalized,
                delimiter: Some('-'),
            });
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

        fn __test_from_str(data: Vec<&str>, case: Case) {
            let binding = case.delimiter().map(String::from);

            assert_eq!(
                data,
                CapitalizedString::new(
                    data.iter().join(binding.as_deref().unwrap_or("")).as_str(),
                    case
                )
                .data,
                "failed with case {case:?}"
            );
        }
        fn __test_to_string(data: &str, case: Case) {
            assert_eq!(
                data,
                CapitalizedString::new(data, case).to_string(),
                "failed with case {case:?}",
            );
        }
    }
}
