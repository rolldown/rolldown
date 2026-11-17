use crate::EcmaAssetMeta;

#[derive(Debug)]

pub enum InstantiationKind {
  Ecma(Box<EcmaAssetMeta>),
  Sourcemap(Box<SourcemapAssetMeta>),
}

#[derive(Debug)]
pub struct SourcemapAssetMeta {
  pub names: Vec<String>,
  pub original_file_names: Vec<String>,
}
