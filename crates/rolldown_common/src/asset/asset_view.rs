use derive_more::Debug;

#[derive(Debug, Clone)]
pub struct AssetView {
  #[debug("Box<u8>")]
  pub source: Box<[u8]>,
}
