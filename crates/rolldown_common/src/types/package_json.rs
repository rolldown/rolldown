use std::path::PathBuf;

use crate::side_effects::{glob_match_with_normalized_pattern, SideEffects};

#[derive(Debug, Clone)]
pub struct PackageJson {
  /// Path to `package.json`. Contains the `package.json` filename.
  pub path: PathBuf,
  pub r#type: Option<String>,
  pub side_effects: SideEffects,
}

impl PackageJson {
  pub fn new(path: PathBuf) -> Self {
    Self { path, r#type: None, side_effects: SideEffects::None }
  }

  #[must_use]
  pub fn with_type(mut self, value: Option<&serde_json::Value>) -> Self {
    self.r#type =
      value.and_then(|v| v.get("type").and_then(|v| v.as_str()).map(ToString::to_string));
    self
  }

  #[must_use]
  pub fn with_side_effects(mut self, value: Option<&serde_json::Value>) -> Self {
    self.side_effects = value.map(SideEffects::from).unwrap_or_default();
    self
  }

  pub fn r#type(&self) -> Option<&str> {
    self.r#type.as_deref()
  }

  pub fn check_side_effects_for(&self, module_path: &str) -> Option<bool> {
    // Is it necessary to convert module_path to relative path?
    match &self.side_effects {
      SideEffects::None => None,
      SideEffects::Bool(s) => Some(*s),
      SideEffects::String(p) => Some(glob_match_with_normalized_pattern(p.as_str(), module_path)),
      SideEffects::Array(pats) => {
        Some(pats.iter().any(|p| glob_match_with_normalized_pattern(p.as_str(), module_path)))
      }
    }
  }
}
