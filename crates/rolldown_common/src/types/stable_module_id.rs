use std::{borrow::Borrow, path::Path};

use arcstr::ArcStr;
use rolldown_std_utils::PathExt as _;
use sugar_path::SugarPath as _;

use crate::{ModuleId, ModuleIdKind};

/// `StableModuleId` is the stabilized version of `ModuleId`.
/// - It is calculated based on `ModuleId` to be stable across machines and operating systems.
/// - Absolute paths are converted to relative paths from the cwd.
/// - Virtual module prefixes (`\0`) are escaped to `\\0`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct StableModuleId {
  inner: ArcStr,
}

impl StableModuleId {
  /// Creates a new `StableModuleId` by stabilizing the given module ID.
  ///
  /// The stabilization process:
  /// - Converts absolute paths to relative paths from the cwd
  /// - Converted paths use forward slashes (`/`) as separators even on Windows
  /// - Escapes virtual module prefixes (`\0` → `\\0`)
  /// - Returns non-path specifiers as-is
  pub fn new(id: &ModuleId, cwd: &Path) -> Self {
    // Classification already happened once when the `ModuleId` was built; branch on it
    // instead of re-detecting absolute/virtual here.
    let inner: ArcStr = match id.kind() {
      // Absolute path → relative to cwd, slashed (stable across machines/OSes).
      ModuleIdKind::Path => id.as_str().relative(cwd).as_path().expect_to_slash().into(),
      // Virtual module → escape the `\0` prefix.
      ModuleIdKind::Virtual => id.as_str().replace('\0', "\\0").into(),
      // Bare specifier / URL / … → as-is (cheap `Arc` clone).
      ModuleIdKind::Bare => id.as_arc_str().clone(),
    };
    Self { inner }
  }

  /// Creates a new `StableModuleId` from an `ArcStr` without stabilization.
  pub fn from_module_id(module_id: ModuleId) -> Self {
    Self { inner: module_id.into_inner() }
  }

  #[cfg(test)]
  fn with_str(id: &str, cwd: &Path) -> Self {
    Self::new(&ModuleId::new(id), cwd)
  }

  pub fn as_str(&self) -> &str {
    &self.inner
  }

  pub fn as_arc_str(&self) -> &ArcStr {
    &self.inner
  }
}

impl AsRef<str> for StableModuleId {
  fn as_ref(&self) -> &str {
    &self.inner
  }
}

impl std::ops::Deref for StableModuleId {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl std::fmt::Display for StableModuleId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    std::fmt::Display::fmt(&self.inner, f)
  }
}

// This allows to use `&str` to lookup `HashMap<StableModuleId, V>`. For `&String`, since it could coerce to `&str`, it also works.
impl Borrow<str> for StableModuleId {
  fn borrow(&self) -> &str {
    &self.inner
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_stabilize_id() {
    let cwd = std::env::current_dir().unwrap();
    // absolute path
    assert_eq!(
      StableModuleId::with_str(cwd.join("src").join("main.js").expect_to_str(), &cwd).as_str(),
      "src/main.js"
    );
    assert_eq!(
      StableModuleId::with_str(cwd.join("..").join("src").join("main.js").expect_to_str(), &cwd)
        .as_str(),
      "../src/main.js"
    );

    // non-path specifier
    assert_eq!(StableModuleId::with_str("fs", &cwd).as_str(), "fs");
    assert_eq!(
      StableModuleId::with_str("https://deno.land/x/oak/mod.ts", &cwd).as_str(),
      "https://deno.land/x/oak/mod.ts"
    );

    // virtual module
    assert_eq!(StableModuleId::with_str("\0foo", &cwd).as_str(), "\\0foo");
  }
}
