use rolldown_plugin_vite_asset_import_meta_url::ViteAssetImportMetaUrlPlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingViteAssetImportMetaUrlPluginConfig {
  pub client_entry: String,
}

impl From<BindingViteAssetImportMetaUrlPluginConfig> for ViteAssetImportMetaUrlPlugin {
  fn from(value: BindingViteAssetImportMetaUrlPluginConfig) -> Self {
    Self { client_entry: value.client_entry }
  }
}
