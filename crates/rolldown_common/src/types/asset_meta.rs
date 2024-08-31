use crate::EcmaAssetMeta;

#[derive(Debug)]

pub enum InstantiationKind {
  Ecma(Box<EcmaAssetMeta>),
  // Using Variant `None` instead of `Option<AssetMeta>` to make it friendly to use pattern matching.
  None,
}

impl From<EcmaAssetMeta> for InstantiationKind {
  fn from(rendered_chunk: EcmaAssetMeta) -> Self {
    InstantiationKind::Ecma(Box::new(rendered_chunk))
  }
}
