use crate::{EcmaAssetMeta, css::css_asset_meta::CssAssetMeta};

#[derive(Debug)]

pub enum InstantiationKind {
  Ecma(Box<EcmaAssetMeta>),
  Css(Box<CssAssetMeta>),
  Sourcemap(Box<SourcemapAssetMeta>),
  // Using Variant `None` instead of `Option<AssetMeta>` to make it friendly to use pattern matching.
  None,
}

impl Default for InstantiationKind {
  fn default() -> Self {
    InstantiationKind::None
  }
}

impl From<EcmaAssetMeta> for InstantiationKind {
  fn from(rendered_chunk: EcmaAssetMeta) -> Self {
    InstantiationKind::Ecma(Box::new(rendered_chunk))
  }
}

impl From<CssAssetMeta> for InstantiationKind {
  fn from(rendered_chunk: CssAssetMeta) -> Self {
    InstantiationKind::Css(Box::new(rendered_chunk))
  }
}

#[derive(Debug)]
pub struct SourcemapAssetMeta {
  pub names: Vec<String>,
  pub original_file_names: Vec<String>,
}
