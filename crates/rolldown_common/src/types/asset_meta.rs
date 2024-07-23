use crate::EcmaAssetMeta;

pub enum AssetMeta {
  Ecma(Box<EcmaAssetMeta>),
  // Using Variant `None` instead of `Option<AssetMeta>` to make it friendly to use pattern matching.
  None,
}

impl From<EcmaAssetMeta> for AssetMeta {
  fn from(rendered_chunk: EcmaAssetMeta) -> Self {
    AssetMeta::Ecma(Box::new(rendered_chunk))
  }
}
