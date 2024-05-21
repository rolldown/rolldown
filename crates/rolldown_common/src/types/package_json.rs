use std::path::PathBuf;
use std::sync::Arc;

use crate::side_effects::{glob_match_with_normalized_pattern, SideEffects};

#[derive(Debug, Clone)]
pub struct PackageJson {
  pub raw: Arc<serde_json::Value>,
  /// Path to `package.json`. Contains the `package.json` filename.
  pub path: PathBuf,
  pub side_effects: Option<SideEffects>,
}

impl PackageJson {
  pub fn new(raw: impl Into<Arc<serde_json::Value>>, path: PathBuf) -> Self {
    let raw = raw.into();
    Self { side_effects: SideEffects::from_description(&raw), raw, path }
  }

  pub fn r#type(&self) -> Option<&str> {
    self.raw.get("type").and_then(|v| v.as_str())
  }

  pub fn check_side_effects_for(&self, module_path: &str) -> Option<bool> {
    let side_effects = self.side_effects.as_ref()?;
    // Is it necessary to convert module_path to relative path?
    match side_effects {
      SideEffects::Bool(s) => Some(*s),
      SideEffects::String(p) => Some(glob_match_with_normalized_pattern(p.as_str(), module_path)),
      SideEffects::Array(pats) => {
        Some(pats.iter().any(|p| glob_match_with_normalized_pattern(p.as_str(), module_path)))
      }
    }
  }
}
