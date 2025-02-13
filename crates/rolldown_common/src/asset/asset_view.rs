use derive_more::Debug;

#[derive(Debug)]
pub struct AssetView {
  #[debug("Box<u8>")]
  pub source: Box<[u8]>,
}
