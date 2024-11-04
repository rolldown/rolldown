use std::fmt::Debug;

pub struct AssetView {
  pub source: Box<[u8]>,
}

impl Debug for AssetView {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("AssetView").field("source", &"Box<[u8]>").finish()
  }
}
