// This's a temporary abstraction to represent `normalized` resolved id, but if we call it `NormalizedResolvedId`, it may confuse with `ResolvedId`.
// We might be able to replace this with `ModuleId` in the future. However, to push things forward, we create this new type for now.

use arcstr::ArcStr;
use sugar_path::SugarPath;

/// Normalized module identifier that handles Windows path normalization.
/// - If the id is an absolute path on Windows, backslashes are converted to forward slashes
///   (e.g., `C:\path\to\file` → `C:/path/to/file`)
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct NormalizedId(ArcStr);

impl NormalizedId {
  pub fn new(value: impl Into<ArcStr>) -> Self {
    let value = value.into();
    if cfg!(windows) && value.as_path().is_absolute() {
      Self(value.as_path().to_slash_lossy().into())
    } else {
      Self(value)
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  pub fn as_arc_str(&self) -> &ArcStr {
    &self.0
  }
}

impl std::ops::Deref for NormalizedId {
  type Target = ArcStr;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl AsRef<str> for NormalizedId {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl std::fmt::Display for NormalizedId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    std::fmt::Display::fmt(&self.0, f)
  }
}
