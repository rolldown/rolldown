#[napi_derive::napi(object)]
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingMinifyOptions {
  pub mangle: bool,
  pub compress: bool,
  pub remove_whitespace: bool,
}

impl From<BindingMinifyOptions> for rolldown_common::RawMinifyOptions {
  fn from(value: BindingMinifyOptions) -> Self {
    Self::Object(rolldown_common::MinifyOptionsObject {
      mangle: value.mangle,
      compress: value.compress,
      remove_whitespace: value.remove_whitespace,
    })
  }
}

impl From<&rolldown_common::MinifyOptionsObject> for BindingMinifyOptions {
  fn from(value: &rolldown_common::MinifyOptionsObject) -> Self {
    Self {
      mangle: value.mangle,
      compress: value.compress,
      remove_whitespace: value.remove_whitespace,
    }
  }
}
