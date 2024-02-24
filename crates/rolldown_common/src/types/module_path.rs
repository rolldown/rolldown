use std::fmt::Debug;

use crate::FilePath;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceId(FilePath);

impl ResourceId {
  pub fn new(path: FilePath) -> Self {
    Self(path)
  }

  // We may change `ResourceId` to enum in the future, so we have this method to make it easier to change.
  pub fn expect_file(&self) -> &FilePath {
    &self.0
  }
}
