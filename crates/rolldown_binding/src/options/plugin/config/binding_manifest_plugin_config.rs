use rolldown_plugin_manifest::ManifestPluginConfig;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingManifestPluginConfig {
  pub root: String,
  pub out_path: String,
  // TODO: Link this with assets plugin
  // pub generated_assets: Option<Map<String,  GeneratedAssetMeta>>,
}

impl From<BindingManifestPluginConfig> for ManifestPluginConfig {
  fn from(value: BindingManifestPluginConfig) -> Self {
    Self { root: value.root, out_path: value.out_path }
  }
}
