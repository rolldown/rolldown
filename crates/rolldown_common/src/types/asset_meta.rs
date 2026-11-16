use crate::EcmaAssetMeta;

#[derive(Debug)]

pub enum InstantiationKind {
  Ecma(Box<EcmaAssetMeta>),
  Sourcemap(Box<SourcemapAssetMeta>),
}

impl From<EcmaAssetMeta> for InstantiationKind {
  fn from(rendered_chunk: EcmaAssetMeta) -> Self {
    InstantiationKind::Ecma(Box::new(rendered_chunk))
  }
}

#[derive(Debug)]
pub struct SourcemapAssetMeta {
  pub names: Vec<String>,
  pub original_file_names: Vec<String>,
}
