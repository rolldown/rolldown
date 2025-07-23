use rolldown_plugin_manifest::ManifestPlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingManifestPluginConfig {
  pub root: String,
  pub out_path: String,
}

impl From<BindingManifestPluginConfig> for ManifestPlugin {
  fn from(value: BindingManifestPluginConfig) -> Self {
    Self { root: value.root, out_path: value.out_path }
  }
}
