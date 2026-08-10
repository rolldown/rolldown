use std::path::{Path, PathBuf};

use arcstr::ArcStr;
use oxc_resolver::PackageType;

use crate::side_effects::{SideEffects, glob_match_with_normalized_pattern};

#[derive(Debug, Clone)]
pub struct PackageJson {
  name: Option<ArcStr>,
  version: Option<ArcStr>,
  pub r#type: Option<&'static str>,
  pub side_effects: Option<SideEffects>,
  realpath: PathBuf,
}

impl PackageJson {
  pub fn from_oxc_pkg_json(oxc_pkg_json: &oxc_resolver::PackageJson) -> Self {
    Self {
      name: oxc_pkg_json.name().map(ArcStr::from),
      version: oxc_pkg_json.version().map(ArcStr::from),
      r#type: oxc_pkg_json.r#type().map(|t| match t {
        PackageType::CommonJs => "commonjs",
        PackageType::Module => "module",
      }),
      side_effects: oxc_pkg_json.side_effects().as_ref().map(SideEffects::from_resolver),
      realpath: oxc_pkg_json.realpath.clone(),
    }
  }

  /// Realpath to `package.json`. Contains the `package.json` filename.
  pub fn realpath(&self) -> &Path {
    &self.realpath
  }

  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  pub fn version(&self) -> Option<&str> {
    self.version.as_deref()
  }

  pub fn r#type(&self) -> Option<&str> {
    self.r#type
  }

  /// * `module_path`: relative path to the module from `package.json` path
  pub fn check_side_effects_for(&self, module_path: &str) -> Option<bool> {
    let side_effects = self.side_effects.as_ref()?;
    // Is it necessary to convert module_path to relative path?
    // `sideEffects` is a positive allowlist, so a leading-`!` entry never marks a module as
    // side-effectful. Handing it to `fast_glob` would instead match nearly every path that
    // does not match the remainder of the pattern.
    match side_effects {
      SideEffects::Bool(s) => Some(*s),
      SideEffects::String(pattern) => Some(
        !pattern.starts_with('!')
          && glob_match_with_normalized_pattern(pattern.as_str(), module_path),
      ),
      SideEffects::Array(patterns) => Some(
        patterns
          .iter()
          .filter(|pattern| !pattern.starts_with('!'))
          .any(|pattern| glob_match_with_normalized_pattern(pattern, module_path)),
      ),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn package_with_side_effects(side_effects: SideEffects) -> PackageJson {
    PackageJson {
      name: None,
      version: None,
      r#type: None,
      side_effects: Some(side_effects),
      realpath: PathBuf::from("/package/package.json"),
    }
  }

  #[test]
  fn negative_side_effect_patterns_do_not_include_unrelated_modules() {
    let package = package_with_side_effects(SideEffects::Array(
      [
        "**/*.css",
        "**/tokens/*.{js,ts,ts.esnext}",
        "!**/@scope/excluded/**/tokens/*.{js,ts,ts.esnext}",
        "**/configure.{js,mjs}",
      ]
      .map(str::to_string)
      .to_vec(),
    ));

    assert_eq!(package.check_side_effects_for("src/index.mjs"), Some(false));
    assert_eq!(package.check_side_effects_for("src/configure.mjs"), Some(true));
    assert_eq!(package.check_side_effects_for("src/styles.css"), Some(true));
    assert_eq!(package.check_side_effects_for("src/tokens/colors.js"), Some(true));

    let only_negative =
      package_with_side_effects(SideEffects::Array(vec!["!**/excluded/**".to_string()]));
    assert_eq!(only_negative.check_side_effects_for("src/index.mjs"), Some(false));

    let negative_string =
      package_with_side_effects(SideEffects::String("!**/excluded/**".to_string()));
    assert_eq!(negative_string.check_side_effects_for("src/index.mjs"), Some(false));
  }
}
