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
    clippy::string_to_string
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::single_match_else
)]

use std::time::Duration;

mod bar;

pub use bar::{Bar, Progress};
pub mod arrow {
    pub use crate::bar::arrow::{Arrow, Fancy, Simple, UnicodeBar};
}
pub mod callback {
    pub use crate::bar::{Callback, Mut, Once};
}
#[must_use]
pub fn terminal_width() -> Option<usize> {
    term_size::dimensions().map(|(w, _)| w)
}

#[inline]
pub(crate) const fn split_duration(duration: &Duration) -> (usize, usize, usize) {
    let elapsed = duration.as_secs() as usize;
    let seconds = elapsed % 60;
    let minutes = (elapsed / 60) % 60;
    let hours = elapsed / 3600;
    (hours, minutes, seconds)
}
