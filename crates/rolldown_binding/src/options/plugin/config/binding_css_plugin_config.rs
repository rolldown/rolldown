use rolldown_plugin_css::{CssPlugin, CssPluginOptions};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingCssPluginConfig {
  pub code_split: Option<bool>,
  pub minify: Option<bool>,
  pub sourcemap: Option<bool>,
}

impl From<BindingCssPluginConfig> for CssPlugin {
  fn from(config: BindingCssPluginConfig) -> Self {
    CssPlugin::new(CssPluginOptions {
      code_split: config.code_split.unwrap_or_default(),
      minify: config.minify.unwrap_or_default(),
      sourcemap: config.sourcemap.unwrap_or_default(),
    })
  }
}
