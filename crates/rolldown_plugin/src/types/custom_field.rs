use std::fmt;
use std::ops::{Deref, DerefMut};

use rustc_hash::FxBuildHasher;
use typedmap::TypedMap;

type Inner = TypedMap<(), typedmap::SyncAnyBounds, typedmap::SyncAnyBounds, FxBuildHasher>;

pub struct CustomField(Inner);

impl fmt::Debug for CustomField {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CustomField").finish_non_exhaustive()
  }
}

impl CustomField {
  pub fn new() -> Self {
    CustomField::default()
  }
}

impl Default for CustomField {
  fn default() -> Self {
    Self(TypedMap::with_hasher(FxBuildHasher))
  }
}

impl Deref for CustomField {
  type Target = Inner;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for CustomField {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}
