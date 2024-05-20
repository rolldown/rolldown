use std::path::PathBuf;
use std::sync::Arc;

use globset::GlobSet;

use crate::side_effects::SideEffects;

#[derive(Debug, Clone)]
pub struct PackageJson {
  pub raw: Arc<serde_json::Value>,
  /// Realpath to `package.json`. Contains the `package.json` filename.
  pub realpath: PathBuf,
  pub side_effects: Option<(SideEffects, GlobSet)>,
}

impl PackageJson {
  pub fn new(raw: impl Into<Arc<serde_json::Value>>, realpath: PathBuf) -> Self {
    let raw = raw.into();
    Self {
      side_effects: SideEffects::from_description(&raw).map(|item| {
        let matcher = item.global_matcher();
        (item, matcher)
      }),
      raw,
      realpath,
    }
  }

  pub fn r#type(&self) -> Option<&str> {
    self.raw.get("type").and_then(|v| v.as_str())
  }

  pub fn check_side_effects_for(&self, module_path: &str) -> Option<bool> {
    let (side_effects, matcher) = self.side_effects.as_ref()?;
    // Is it necessary to convert module_path to relative path?
    match side_effects {
      SideEffects::Bool(s) => Some(*s),
      _ => Some(matcher.is_match(module_path)),
    }
  }
}
