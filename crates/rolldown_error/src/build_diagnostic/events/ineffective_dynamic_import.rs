use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};
use arcstr::ArcStr;

#[derive(Debug)]
pub struct IneffectiveDynamicImport {
  pub module_id: ArcStr,
  pub dynamic_importers: Vec<ArcStr>,
  pub static_importers: Vec<ArcStr>,
}

impl BuildEvent for IneffectiveDynamicImport {
  fn kind(&self) -> EventKind {
    EventKind::IneffectiveDynamicImport
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.to_string())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "\n(!) {} is dynamically imported by {} but also statically imported by {}, dynamic import will not move module into another chunk.\n",
      self.module_id,
      join_with_limit(&self.dynamic_importers, ", ", 5),
      join_with_limit(&self.static_importers, ", ", 5),
    )
  }
}

/// Joins a vector of items with a separator, showing only the first `limit` items
/// and adding "..." if there are more.
fn join_with_limit<T: AsRef<str>>(items: &[T], separator: &str, limit: usize) -> String {
  debug_assert!(limit > 0, "limit must be greater than 0");
  if items.len() <= limit {
    items.iter().map(AsRef::as_ref).collect::<Vec<_>>().join(separator)
  } else {
    let mut result = items[..limit].iter().map(AsRef::as_ref).collect::<Vec<_>>().join(separator);
    result.push_str(separator);
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
