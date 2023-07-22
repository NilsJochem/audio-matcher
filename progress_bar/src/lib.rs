#![warn(clippy::nursery, clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate
)]

use std::time::Duration;

mod bar;

pub use bar::{Bar, Progress};
pub mod arrow {
    pub use crate::bar::arrow::{Arrow, FancyArrow, SimpleArrow, UnicodeBar};
}
pub mod callback {
    pub use crate::bar::{Callback, MutCallback, OnceCallback};
}

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
