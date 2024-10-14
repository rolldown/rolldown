use std::fmt::Debug;

use string_wizard::MagicString;

pub trait SourceMutation: Debug + Send + Sync {
  fn apply(&self, magic_string: &mut MagicString<'_>);
}

pub type BoxedSourceMutation = Box<dyn SourceMutation>;
