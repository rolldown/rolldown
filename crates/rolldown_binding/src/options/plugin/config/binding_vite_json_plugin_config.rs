use napi_derive::napi;
use rolldown_plugin_vite_json::{ViteJsonPlugin, ViteJsonPluginStringify};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingViteJsonPluginConfig {
  pub minify: Option<bool>,
  pub named_exports: Option<bool>,
  pub stringify: Option<BindingViteJsonPluginStringify>,
}

impl TryFrom<BindingViteJsonPluginConfig> for ViteJsonPlugin {
  type Error = anyhow::Error;

  fn try_from(config: BindingViteJsonPluginConfig) -> Result<Self, Self::Error> {
    Ok(Self {
      minify: config.minify.unwrap_or_default(),
      named_exports: config.named_exports.unwrap_or_default(),
      stringify: config.stringify.map(TryInto::try_into).transpose()?.unwrap_or_default(),
    })
  }
}

#[derive(Debug)]
#[napi(transparent)]
pub struct BindingViteJsonPluginStringify(napi::Either<bool, String>);

impl TryFrom<BindingViteJsonPluginStringify> for ViteJsonPluginStringify {
  type Error = napi::Error;

  fn try_from(value: BindingViteJsonPluginStringify) -> Result<Self, Self::Error> {
    Ok(match value {
      BindingViteJsonPluginStringify(napi::Either::A(true)) => ViteJsonPluginStringify::True,
      BindingViteJsonPluginStringify(napi::Either::A(false)) => ViteJsonPluginStringify::False,
      BindingViteJsonPluginStringify(napi::Either::B(s)) if s == "auto" => {
        ViteJsonPluginStringify::Auto
      }
      BindingViteJsonPluginStringify(napi::Either::B(s)) => {
        return Err(napi::Error::new(
          napi::Status::InvalidArg,
          format!("Invalid stringify option: {s}"),
        ));
      }
    })
  }
}
