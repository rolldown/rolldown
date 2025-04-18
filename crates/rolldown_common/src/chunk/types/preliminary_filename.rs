use std::ops::Deref;

use arcstr::ArcStr;

#[derive(Debug, Clone)]
/// Represents a filename that might contains hash placeholder.
pub struct PreliminaryFilename {
  /// Might contains preliminary hash
  filename: ArcStr,
  /// Something like `!~{abcd}~`
  hash_placeholder: Option<Vec<String>>,
}

impl PreliminaryFilename {
  pub fn new(filename: ArcStr, hash_placeholder: Option<Vec<String>>) -> Self {
    Self { filename, hash_placeholder }
  }

  pub fn hash_placeholder(&self) -> Option<&[String]> {
    self.hash_placeholder.as_deref()
  }
}

impl Deref for PreliminaryFilename {
  type Target = ArcStr;

  fn deref(&self) -> &Self::Target {
    &self.filename
  }
}
