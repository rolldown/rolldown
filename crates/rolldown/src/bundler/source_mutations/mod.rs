pub mod remove_node;
pub mod rename_symbol;
pub mod rewrite_default_export;

use string_wizard::MagicString;

#[derive(Debug)]
pub struct Context {}

pub trait SourceMutation {
  #[allow(unused_variables)]
  fn apply<'me>(&'me self, ctx: &Context, s: &mut MagicString<'me>) {}
}
