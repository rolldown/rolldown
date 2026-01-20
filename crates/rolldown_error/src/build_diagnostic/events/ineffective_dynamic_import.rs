use arcstr::ArcStr;

use crate::{EventKind, build_diagnostic::events::BuildEvent};

#[derive(Debug)]
pub struct IneffectiveDynamicImport {
  pub module_id: ArcStr,
  pub dynamic_importers: Vec<ArcStr>,
  pub static_importers: Vec<ArcStr>,
}

impl BuildEvent for IneffectiveDynamicImport {
  fn kind(&self) -> crate::EventKind {
    EventKind::IneffectiveDynamicImport
  }

  fn message(&self, _opts: &crate::DiagnosticOptions) -> String {
    format!(
      "\n(!) {} is dynamically imported by {} but also statically imported by {}, dynamic import will not move module into another chunk.\n",
      self.module_id,
      join_with_limit(&self.dynamic_importers, ", ", 5),
      join_with_limit(&self.static_importers, ", ", 5),
    )
  }
}

fn join_with_limit<T: AsRef<str>>(items: &[T], sep: &str, limit: usize) -> String {
  debug_assert!(limit > 0, "limit must be greater than 0");
  if items.len() <= limit {
    items.iter().map(AsRef::as_ref).collect::<Vec<_>>().join(sep)
  } else {
    let mut result = items[..limit].iter().map(AsRef::as_ref).collect::<Vec<_>>().join(sep);
    result.push_str(sep);
    result.push_str("...");
    result
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_join_with_limit_less_than_limit() {
    let items = vec!["a", "b", "c"];
    assert_eq!(join_with_limit(&items, ", ", 5), "a, b, c");
  }

  #[test]
  fn test_join_with_limit_equal_to_limit() {
    let items = vec!["a", "b", "c", "d", "e"];
    assert_eq!(join_with_limit(&items, ", ", 5), "a, b, c, d, e");
  }

  #[test]
  fn test_join_with_limit_more_than_limit() {
    let items = vec!["a", "b", "c", "d", "e", "f", "g"];
    assert_eq!(join_with_limit(&items, ", ", 5), "a, b, c, d, e, ...");
  }

  #[test]
  fn test_join_with_limit_custom_separator() {
    let items = vec!["a", "b", "c", "d", "e", "f"];
    assert_eq!(join_with_limit(&items, " | ", 3), "a | b | c | ...");
  }

  #[test]
  fn test_join_with_limit_empty_vector() {
    let items: Vec<&str> = vec![];
    assert_eq!(join_with_limit(&items, ", ", 5), "");
  }
}
