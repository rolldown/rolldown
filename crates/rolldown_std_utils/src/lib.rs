//! Only utils/extensions for the rust std library.

mod option_ext;
mod path_buf_ext;
mod path_ext;
mod pretty_type_name;

pub use crate::{
  option_ext::OptionExt,
  path_buf_ext::PathBufExt,
  path_ext::{PathExt, representative_file_name_for_preserve_modules},
  pretty_type_name::pretty_type_name,
};
