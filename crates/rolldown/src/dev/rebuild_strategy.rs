use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RebuildStrategy {
  /// Incremental rebuild will always be issued after HMR.
  Always,
  /// Never issue rebuilds after HMR.
  #[default]
  Never,
}

impl RebuildStrategy {
  pub fn is_always(&self) -> bool {
    matches!(self, Self::Always)
  }
}

impl fmt::Display for RebuildStrategy {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Always => write!(f, "always"),
      Self::Never => write!(f, "never"),
    }
  }
}
