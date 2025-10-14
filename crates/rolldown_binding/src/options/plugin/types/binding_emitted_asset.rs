use super::binding_asset_source::BindingAssetSource;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default, Debug)]
pub struct BindingEmittedAsset {
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub original_file_name: Option<String>,
  pub source: BindingAssetSource,
}

impl From<BindingEmittedAsset> for rolldown_common::EmittedAsset {
  fn from(value: BindingEmittedAsset) -> Self {
    Self {
      name: value.name,
      file_name: value.file_name.map(Into::into),
      source: value.source.into(),
      original_file_name: value.original_file_name,
    }
  }
}
