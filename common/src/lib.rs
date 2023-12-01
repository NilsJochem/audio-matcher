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
    missing_docs,
    unsafe_op_in_unsafe_fn
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
/// common string utils
pub mod str {
    /// A module for converting the case of strings
    pub mod convert;
    /// A module for searching strings
    pub mod filter;

    #[allow(missing_docs)]
    pub const fn compare_char(a: char, b: char, ignore_case: bool) -> bool {
        (ignore_case && a.eq_ignore_ascii_case(&b)) || a == b
    }
}

/// common utilitys for argparsing
pub mod args {
    /// common utilitys for input managing
    pub mod input {
        use clap::Args;

        #[derive(Args, Debug, Clone, Copy)]
        #[group(required = false, multiple = false)]
        /// clap struct for input config
        pub struct Inputs {
            /// always answer yes
            #[clap(short)]
            pub yes: bool,
            /// always answer no
            #[clap(short)]
            pub no: bool,
            /// number of retrys
            #[clap(long, default_value_t = 3)]
            pub trys: u8,
        }
        impl Inputs {
            /// creates a new Inputs struct
            ///
            /// when bools is None, no default answer is set
            pub fn new(bools: impl Into<Option<bool>>, trys: impl Into<Option<u8>>) -> Self {
                let bools: Option<_> = bools.into();
                Self {
                    yes: bools.is_some(),
                    no: bools.is_some_and(|it| !it),
                    trys: trys.into().unwrap_or(3),
                }
            }

            #[inline]
            #[allow(clippy::needless_pass_by_value)]
            fn inner_read<T>(
                msg: impl AsRef<str>,
                default: impl Into<Option<T>>,
                retry_msg: Option<impl AsRef<str>>,
                mut map: impl FnMut(String) -> Option<T>,
                trys: impl IntoIterator<Item = u8>,
            ) -> Option<T> {
                let msg = msg.as_ref();
                let retry_msg = retry_msg.as_ref().map(std::convert::AsRef::as_ref);
                let default = default.into();

                print!("{msg}");
                for _ in trys {
                    let rin: String = text_io::read!("{}\n");
                    if default.is_some() && rin.is_empty() {
                        return default;
                    }
                    match (map(rin), retry_msg) {
                        (Some(t), _) => return Some(t),
                        (None, Some(retry_msg)) => println!("{retry_msg}"),
                        (None, None) => print!("{msg}"),
                    }
                }
                None
            }

            const DEFAULT_RETRY_MSG: &str = "couldn't parse that, please try again: ";
            /// read userinput as a String
            pub fn read(msg: impl AsRef<str>, default: Option<String>) -> String {
                Self::inner_read(
                    msg,
                    default,
                    Some(Self::DEFAULT_RETRY_MSG),
                    Some,
                    std::iter::once(1),
                )
                .unwrap_or_else(|| unreachable!())
            }
            /// read userinput and map it. Retrys to read until `map` returns `Some`
            pub fn map_read<T>(
                msg: impl AsRef<str>,
                default: impl Into<Option<T>>,
                retry_msg: Option<impl AsRef<str>>,
                map: impl FnMut(String) -> Option<T>,
            ) -> T {
                Self::inner_read(msg, default, retry_msg, map, 1..)
                    .unwrap_or_else(|| unreachable!())
            }
            // TODO remove trys from Self
            /// read userinput and map it. Retrys to read until `map` returns `Some` or until self.trys
            pub fn try_read<T>(
                &self,
                msg: impl AsRef<str>,
                default: Option<T>,
                map: impl FnMut(String) -> Option<T>,
            ) -> Option<T> {
                Self::inner_read(
                    msg,
                    default,
                    Some(Self::DEFAULT_RETRY_MSG),
                    map,
                    1..self.trys,
                )
            }

            #[must_use]
            #[momo::momo]
            /// asks user for consent if no default is set
            pub fn ask_consent(self, msg: impl AsRef<str>) -> bool {
                if self.yes || self.no {
                    return self.yes;
                }
                self.try_read(format!("{msg} [y/n]: "), None, |it| {
                    if ["y", "yes", "j", "ja"].contains(&it.as_str()) {
                        Some(true)
                    } else if ["n", "no", "nein"].contains(&it.as_str()) {
                        Some(false)
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    log::info!("probably not");
                    false
                })
            }

            #[must_use]
            /// read userinput as a String.
            /// Starts with `initial`
            /// Uses `suggestor` for suggestions
            pub fn read_with_suggestion(
                msg: impl AsRef<str>,
                initial: Option<&str>,
                mut suggestor: impl autocompleter::Autocomplete,
            ) -> String {
                let mut text = inquire::Text::new(msg.as_ref());
                text.initial_value = initial;
                // SAFTY: the reference to suggestor must be kept alive until ac is dropped. black-box should do this.
                let ac = unsafe { autocompleter::BorrowCompleter::new(&mut suggestor) };
                let res = text.with_autocomplete(ac).prompt().unwrap();
                drop(std::hint::black_box(suggestor));
                res
            }
        }

        #[allow(missing_docs)]
        pub mod autocompleter {
            use std::fmt::Debug;

            use itertools::Itertools;

            use crate::str::filter::StrMetric;

            pub type Error = inquire::CustomUserError;
            pub type Replacement = inquire::autocompletion::Replacement;
            /// a wrapper around inquires Autocomplete, that doesn't have the Clone + 'static requirement
            pub trait Autocomplete: Debug {
                /// relays to `inquire::Autocompleter`
                fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, Error>;
                /// relays to `inquire::Autocompleter`
                fn get_completion(
                    &mut self,
                    input: &str,
                    highlighted_suggestion: Option<String>,
                ) -> Result<Replacement, Error>;
            }
            impl<AC: Autocomplete> Autocomplete for &mut AC {
                #[inline]
                fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, Error> {
                    (**self).get_suggestions(input)
                }

                #[inline]
                fn get_completion(
                    &mut self,
                    input: &str,
                    highlighted_suggestion: Option<String>,
                ) -> Result<Replacement, Error> {
                    (**self).get_completion(input, highlighted_suggestion)
                }
            }
            #[derive(Debug)]
            /// adapter to match `MyAutocompleter` to `inqure::Autocomplete`
            pub(super) struct BorrowCompleter {
                inner: &'static mut dyn Autocomplete,
            }
            impl BorrowCompleter {
                pub(super) unsafe fn new<'a>(other: &'a mut dyn Autocomplete) -> Self {
                    // SAFTY: transmute to upgrade lifetime to static, so one can uphold Autocompletes Clone + 'static needs
                    Self {
                        inner: unsafe {
                            std::mem::transmute::<
                                &'a mut dyn Autocomplete,
                                &'static mut dyn Autocomplete,
                            >(other)
                        },
                    }
                }
            }
            // fake being clone, it's (probably) only needed, when the holding inquire::Text ist cloned
            impl Clone for BorrowCompleter {
                fn clone(&self) -> Self {
                    panic!("cloned Autocompleter {self:?}");
                    // Self { inner: self.inner }
                }
            }
            impl inquire::Autocomplete for BorrowCompleter {
                fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, Error> {
                    self.inner.get_suggestions(input)
                }

                fn get_completion(
                    &mut self,
                    input: &str,
                    highlighted_suggestion: Option<String>,
                ) -> Result<Replacement, Error> {
                    self.inner.get_completion(input, highlighted_suggestion)
                }
            }

            #[derive(Debug)]
            /// takes a list of Strings for suggestions
            pub struct VecCompleter {
                data: Vec<String>,
                metric: Box<dyn StrMetric>,
            }
            impl VecCompleter {
                #[must_use]
                #[allow(missing_docs)]
                pub fn new(data: Vec<String>, metric: impl StrMetric + 'static) -> Self {
                    Self {
                        data,
                        metric: Box::new(metric),
                    }
                }
                #[allow(missing_docs)]
                pub fn from_iter<Iter>(iter: Iter, metric: impl StrMetric + 'static) -> Self
                where
                    Iter: IntoIterator,
                    Iter::Item: ToString,
                {
                    Self::new(
                        iter.into_iter().map(|it| it.to_string()).collect_vec(),
                        metric,
                    )
                }
            }
            impl Autocomplete for VecCompleter {
                fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, Error> {
                    Ok(crate::str::filter::sort_with(
                        self.metric.as_ref(),
                        self.data.iter(),
                        input,
                        |it| it,
                    )
                    .cloned()
                    .collect_vec())
                }

                fn get_completion(
                    &mut self,
                    _input: &str,
                    highlighted_suggestion: Option<String>,
                ) -> Result<Replacement, Error> {
                    Ok(highlighted_suggestion)
                }
            }
        }
    }

    /// common debug utils
    pub mod debug {
        use clap::Args;

        #[derive(Args, Debug, Clone, Copy)]
        #[group(required = false, multiple = false)]
        #[allow(clippy::struct_excessive_bools)]
        #[allow(missing_docs)]
        pub struct OutputLevel {
            #[clap(short, long, help = "print maximum info")]
            pub(crate) debug: bool,
            #[clap(short, long, help = "print more info")]
            pub(crate) verbose: bool,
            #[clap(short, long, help = "print sligtly more info")]
            pub(crate) warn: bool,
            #[clap(short, long, help = "print almost no info")]
            pub(crate) silent: bool,
        }

        impl OutputLevel {
            #[allow(missing_docs)]
            pub fn init_logger(&self) {
                let level = log::Level::from(*self);
                Self::init_logger_with(level);
            }
            #[allow(missing_docs)]
            pub fn init_logger_with(level: log::Level) {
                let env = env_logger::Env::default();
                let env = env.default_filter_or(level.as_str());

                let mut builder = env_logger::Builder::from_env(env);

                builder.format_timestamp(None);
                builder.format_target(false);
                builder.format_level(level < log::Level::Info);

                builder.init();
            }
        }

        impl From<OutputLevel> for log::Level {
            fn from(val: OutputLevel) -> Self {
                if val.silent {
                    Self::Error
                } else if val.verbose {
                    Self::Trace
                } else if val.debug {
                    Self::Debug
                } else if val.warn {
                    Self::Warn
                } else {
                    Self::Info
                }
            }
        }
    }
}
