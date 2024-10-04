//! Only utils/extensions for the rust std library.

mod option_ext;
mod pretty_type_name;

pub use crate::{option_ext::OptionExt, pretty_type_name::pretty_type_name};
