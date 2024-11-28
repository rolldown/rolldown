use std::ops::Deref;

use rustc_hash::FxBuildHasher;
use typedmap::TypedDashMap;

#[derive(Debug)]
pub struct CustomField(
  TypedDashMap<(), typedmap::SyncAnyBounds, typedmap::SyncAnyBounds, FxBuildHasher>,
);

impl CustomField {
  pub fn new() -> Self {
    CustomField::default()
  }
}

impl Default for CustomField {
  fn default() -> Self {
    Self(TypedDashMap::with_hasher(FxBuildHasher))
  }
}

impl Deref for CustomField {
  type Target = TypedDashMap<(), typedmap::SyncAnyBounds, typedmap::SyncAnyBounds, FxBuildHasher>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
