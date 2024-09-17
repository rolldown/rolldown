use derivative::Derivative;
use serde::Deserialize;

// In rollup, except the `presets` option, options in `generatedCode` is all bool.
#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Deserialize, Derivative)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingGeneratedCodeOptions {
  pub symbols: Option<bool>,
  pub const_bindings: Option<bool>,
}
