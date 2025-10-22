use napi_derive::napi;
use rolldown_error::BuildDiagnostic;
use rolldown_plugin_json::{JsonPlugin, JsonPluginStringify};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingJsonPluginConfig {
  pub minify: Option<bool>,
  pub named_exports: Option<bool>,
  pub stringify: Option<BindingJsonPluginStringify>,
}

impl TryFrom<BindingJsonPluginConfig> for JsonPlugin {
  type Error = BuildDiagnostic;

  fn try_from(config: BindingJsonPluginConfig) -> Result<Self, Self::Error> {
    Ok(Self {
      minify: config.minify.unwrap_or_default(),
      named_exports: config.named_exports.unwrap_or_default(),
      stringify: config.stringify.map(TryInto::try_into).transpose()?.unwrap_or_default(),
    })
  }
}

#[derive(Debug)]
#[napi(transparent)]
pub struct BindingJsonPluginStringify(napi::Either<bool, String>);

impl TryFrom<BindingJsonPluginStringify> for JsonPluginStringify {
  type Error = BuildDiagnostic;

  fn try_from(value: BindingJsonPluginStringify) -> Result<Self, Self::Error> {
    Ok(match value {
      BindingJsonPluginStringify(napi::Either::A(true)) => JsonPluginStringify::True,
      BindingJsonPluginStringify(napi::Either::A(false)) => JsonPluginStringify::False,
      BindingJsonPluginStringify(napi::Either::B(s)) if s == "auto" => JsonPluginStringify::Auto,
      BindingJsonPluginStringify(napi::Either::B(s)) => {
        return Err(BuildDiagnostic::napi_error(napi::Error::new(
          napi::Status::InvalidArg,
          format!("Invalid stringify option: {s}"),
        )));
      }
    })
  }
}
