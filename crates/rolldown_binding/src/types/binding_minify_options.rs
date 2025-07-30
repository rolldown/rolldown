#[napi_derive::napi(object)]
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingMinifyOptions {
  pub mangle: Option<bool>,
  pub compress: Option<bool>,
  pub drop_console: Option<bool>,
  pub drop_debugger: Option<bool>,
  pub join_vars: Option<bool>,
  pub sequences: Option<bool>,
  pub remove_whitespace: Option<bool>,
}

impl From<BindingMinifyOptions> for rolldown_common::RawMinifyOptions {
  fn from(value: BindingMinifyOptions) -> Self {
    Self::Object(rolldown_common::MinifyOptionsObject {
      mangle: value.mangle.unwrap_or_default(),
      compress: value.compress.unwrap_or_default(),
      drop_console: value.drop_console.unwrap_or_default(),
      drop_debugger: value.drop_debugger.unwrap_or_default(),
      join_vars: value.join_vars.unwrap_or_default(),
      sequences: value.sequences.unwrap_or_default(),
      remove_whitespace: value.remove_whitespace.unwrap_or_default(),
    })
  }
}

impl From<&rolldown_common::MinifyOptionsObject> for BindingMinifyOptions {
  fn from(value: &rolldown_common::MinifyOptionsObject) -> Self {
    Self {
      mangle: Some(value.mangle),
      compress: Some(value.compress),
      drop_console: Some(value.drop_console),
      drop_debugger: Some(value.drop_debugger),
      join_vars: Some(value.join_vars),
      sequences: Some(value.sequences),
      remove_whitespace: Some(value.remove_whitespace),
    }
  }
}
