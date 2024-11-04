mod chunk;
mod joiner;
mod magic_string;
#[cfg(feature = "sourcemap")]
mod source_map;
mod span;
mod type_aliases;

type CowStr<'a> = Cow<'a, str>;

use std::borrow::Cow;

pub use crate::{
  joiner::{Joiner, JoinerOptions},
  magic_string::{indent::IndentOptions, update::UpdateOptions, MagicString, MagicStringOptions},
};

#[cfg(feature = "sourcemap")]
pub use crate::magic_string::source_map::SourceMapOptions;
