use rolldown_plugin_fake_js::FakeJsOptions;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingFakeJsPluginConfig {
  pub sourcemap: Option<bool>,
  pub cjs_default: Option<bool>,
  pub side_effects: Option<bool>,
}

impl From<BindingFakeJsPluginConfig> for FakeJsOptions {
  fn from(config: BindingFakeJsPluginConfig) -> Self {
    FakeJsOptions {
      sourcemap: config.sourcemap.unwrap_or(false),
      cjs_default: config.cjs_default.unwrap_or(false),
      side_effects: config.side_effects.unwrap_or(false),
    }
  }
}
