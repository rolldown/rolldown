use arcstr::ArcStr;

use crate::options::plugin::types::binding_asset_source::BindingAssetSource;

#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingPreRenderedAsset {
  pub names: Vec<String>,
  pub original_file_names: Vec<String>,
  pub source: BindingAssetSource,
}

impl From<rolldown_common::RollupPreRenderedAsset> for BindingPreRenderedAsset {
  fn from(value: rolldown_common::RollupPreRenderedAsset) -> Self {
    Self {
      names: value.names.iter().map(ArcStr::to_string).collect(),
      original_file_names: value.original_file_names.iter().map(ArcStr::to_string).collect(),
      source: value.source.into(),
    }
  }
}
