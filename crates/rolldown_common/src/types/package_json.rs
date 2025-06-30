use std::path::PathBuf;

use arcstr::ArcStr;

use crate::side_effects::{SideEffects, glob_match_with_normalized_pattern};

#[derive(Debug, Clone)]
pub struct PackageJson {
  /// Path to `package.json`. Contains the `package.json` filename.
  pub path: PathBuf,
  pub r#type: Option<String>,
  pub side_effects: Option<SideEffects>,
  pub version: Option<ArcStr>,
}

impl PackageJson {
  pub fn new(path: PathBuf) -> Self {
    Self { path, r#type: None, side_effects: None, version: None }
  }

  #[must_use]
  pub fn with_version(mut self, value: Option<&str>) -> Self {
    self.version = value.map(Into::into);
    self
  }

  #[must_use]
  pub fn with_type(mut self, value: Option<&str>) -> Self {
    self.r#type = value.map(ToString::to_string);
    self
  }

  #[must_use]
  pub fn with_side_effects(mut self, value: Option<&serde_json::Value>) -> Self {
    self.side_effects = value.and_then(SideEffects::from_json_value);
    self
  }

  pub fn r#type(&self) -> Option<&str> {
    self.r#type.as_deref()
  }

  /// * `module_path`: relative path to the module from `package.json` path
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
