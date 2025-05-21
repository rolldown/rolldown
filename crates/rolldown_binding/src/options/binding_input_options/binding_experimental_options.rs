#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingExperimentalOptions {
  pub strict_execution_order: Option<bool>,
  pub disable_live_bindings: Option<bool>,
  pub vite_mode: Option<bool>,
  pub resolve_new_url_to_asset: Option<bool>,
  pub hmr: Option<BindingExperimentalHmrOptions>,
  pub attach_debug_info: Option<bool>,
}

impl From<BindingExperimentalOptions> for rolldown_common::ExperimentalOptions {
  fn from(value: BindingExperimentalOptions) -> Self {
    Self {
      strict_execution_order: value.strict_execution_order,
      disable_live_bindings: value.disable_live_bindings,
      vite_mode: value.vite_mode,
      resolve_new_url_to_asset: value.resolve_new_url_to_asset,
      // TODO: binding
      incremental_build: None,
      hmr: value.hmr.map(Into::into),
      attach_debug_info: value.attach_debug_info,
    }
  }
}

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingExperimentalHmrOptions {
  pub host: Option<String>,
  pub port: Option<u16>,
  pub implement: Option<String>,
}

impl From<BindingExperimentalHmrOptions> for rolldown_common::HmrOptions {
  fn from(value: BindingExperimentalHmrOptions) -> Self {
    Self { host: value.host, port: value.port, implement: value.implement }
  }
}
