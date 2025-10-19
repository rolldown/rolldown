use std::path::{Path, PathBuf};

use oxc_resolver::PackageType;

use crate::side_effects::{SideEffects, glob_match_with_normalized_pattern};

#[derive(Debug, Clone)]
pub struct PackageJson {
  pub r#type: Option<&'static str>,
  pub side_effects: Option<SideEffects>,
  realpath: PathBuf,
}

impl PackageJson {
  pub fn from_oxc_pkg_json(oxc_pkg_json: &oxc_resolver::PackageJson) -> Self {
    Self {
      r#type: oxc_pkg_json.r#type().map(|t| match t {
        PackageType::CommonJs => "commonjs",
        PackageType::Module => "module",
      }),
      side_effects: oxc_pkg_json.side_effects().as_ref().and_then(SideEffects::from_resolver),
      realpath: oxc_pkg_json.realpath.clone(),
    }
  }

  /// Realpath to `package.json`. Contains the `package.json` filename.
  pub fn realpath(&self) -> &Path {
    &self.realpath
  }

  pub fn r#type(&self) -> Option<&str> {
    self.r#type
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
