use std::{fmt, sync::Arc};

use string_wizard::MagicString;

/// `Debug` is implemented on the trait object below instead of being a supertrait so concrete
/// mutation formatters are not retained in release binaries solely through erased vtables.
pub trait SourceMutation: Send + Sync {
  fn apply(&self, magic_string: &mut MagicString<'_>);
}

impl fmt::Debug for dyn SourceMutation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("SourceMutation")
  }
}

pub type ArcSourceMutation = Arc<dyn SourceMutation>;
