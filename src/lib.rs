#![warn(
    clippy::nursery,
    clippy::pedantic,
    clippy::empty_structs_with_brackets,
    clippy::format_push_string,
    clippy::if_then_some_else_none,
    // clippy::impl_trait_in_params,
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
    clippy::single_match_else,
    clippy::option_if_let_else,
    clippy::must_use_candidate,
    clippy::too_many_lines
)]

pub mod archive;
pub mod args;
pub mod matcher;
pub mod worker;

pub const APP_NAME: &str = "audio-matcher"; // on change remember to change value in audacity

const fn offset_range(range: &std::ops::Range<usize>, offset: usize) -> std::ops::Range<usize> {
    (range.start + offset)..(range.end + offset)
}
// TODO exchange with:
// pub (const) fn split_duration(duration: &Duration) -> (u64, u64, u64) {
//    (duration.hours(), duration.minutes(), duration.seconds())
// }
