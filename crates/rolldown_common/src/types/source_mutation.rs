use std::{fmt::Debug, sync::Arc};

use string_wizard::MagicString;

pub trait SourceMutation: Debug + Send + Sync {
  fn apply(&self, magic_string: &mut MagicString<'_>);
}

pub type ArcSourceMutation = Arc<dyn SourceMutation>;
