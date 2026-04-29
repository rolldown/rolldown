use std::{
  borrow::Borrow,
  path::{Path, PathBuf},
};

use arcstr::ArcStr;
use sugar_path::SugarPath;

use super::stable_module_id::StableModuleId;

const EMPTY_MODULE_PREFIX: &str = "\0rolldown/empty.js?";

/// `ModuleId` is the unique string identifier for each module.
/// - It will be used to identify the module in the whole bundle.
/// - Users could stored the `ModuleId` to track the module in different stages/hooks.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Default)]
pub struct ModuleId {
  inner: ArcStr,
}

impl ModuleId {
  #[inline]
  pub fn new(value: impl Into<ArcStr>) -> Self {
    let value = value.into();
    Self { inner: value }
  }

  #[inline]
  pub const fn new_arc_str(inner: ArcStr) -> Self {
    Self { inner }
  }

  /// Construct the sentinel id used for `browser: false` ignored modules,
  /// concatenated with the original resolved path so each ignored module
  /// stays distinguishable while sharing the empty-module load behavior.
  pub fn new_empty(original: &str) -> Self {
    Self::new(format!("{EMPTY_MODULE_PREFIX}{original}"))
  }

  pub fn is_empty_module(&self) -> bool {
    self.inner.starts_with(EMPTY_MODULE_PREFIX)
  }

  /// For an id created via `new_empty`, returns the original id portion.
  pub fn strip_empty_prefix(&self) -> Option<&str> {
    self.inner.strip_prefix(EMPTY_MODULE_PREFIX)
  }

  pub fn as_str(&self) -> &str {
    &self.inner
  }

  pub fn as_arc_str(&self) -> &ArcStr {
    &self.inner
  }

  pub fn stabilize(&self, cwd: &Path) -> StableModuleId {
    StableModuleId::new(self, cwd)
  }

  pub fn into_inner(self) -> ArcStr {
    self.inner
  }
}

impl AsRef<str> for ModuleId {
  fn as_ref(&self) -> &str {
    &self.inner
  }
}

impl std::ops::Deref for ModuleId {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl From<&str> for ModuleId {
  fn from(value: &str) -> Self {
    Self::new(value)
  }
}

impl From<String> for ModuleId {
  fn from(value: String) -> Self {
    Self::new(value)
  }
}

impl From<ArcStr> for ModuleId {
  fn from(value: ArcStr) -> Self {
    Self::new(value)
  }
}

impl std::fmt::Display for ModuleId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    std::fmt::Display::fmt(&self.inner, f)
  }
}

impl ModuleId {
  pub fn relative_path(&self, root: impl AsRef<Path>) -> PathBuf {
    let path = self.inner.as_path();
    path.relative(root)
  }
}

// This allows to use `&str` to lookup `HashMap<ModuleId, V>`. For `&String`, since it could coerce to `&str`, it also works.
impl Borrow<str> for ModuleId {
  fn borrow(&self) -> &str {
    &self.inner
  }
}
