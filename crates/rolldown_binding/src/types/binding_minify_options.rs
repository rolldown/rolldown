#[napi_derive::napi(object)]
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingMinifyOptions {
  pub mangle: Option<bool>,
  pub compress: Option<bool>,
  pub remove_whitespace: Option<bool>,
}

impl From<BindingMinifyOptions> for rolldown_common::RawMinifyOptions {
  fn from(value: BindingMinifyOptions) -> Self {
    Self::Object(rolldown_common::MinifyOptionsObject {
      mangle: value.mangle.unwrap_or_default(),
      compress: value.compress.unwrap_or_default(),
      remove_whitespace: value.remove_whitespace.unwrap_or_default(),
    })
  }
}

impl From<&rolldown_common::MinifyOptionsObject> for BindingMinifyOptions {
  fn from(value: &rolldown_common::MinifyOptionsObject) -> Self {
    Self {
      mangle: Some(value.mangle),
      compress: Some(value.compress),
      remove_whitespace: Some(value.remove_whitespace),
    }
  }
}
