use derive_more::Debug;

use crate::StrOrBytes;

#[derive(Debug, Clone)]
pub struct AssetView {
  #[debug("Box<u8>")]
  pub source: StrOrBytes,
}
