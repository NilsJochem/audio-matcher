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
pub mod str_convert;
