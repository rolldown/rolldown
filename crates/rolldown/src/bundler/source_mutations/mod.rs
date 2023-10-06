pub mod append;
pub mod overwrite;
pub mod remove_range;
pub mod rename_symbol;

use std::fmt::Debug;

use string_wizard::MagicString;

#[derive(Debug)]
pub struct Context {}

pub trait SourceMutation: Debug + Sync + Send {
  #[allow(unused_variables)]
  fn apply<'me>(&'me self, ctx: &Context, s: &mut MagicString<'me>) {}
}

pub type BoxedSourceMutation = Box<dyn SourceMutation>;
