#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingExperimentalOptions {
  pub strict_execution_order: Option<bool>,
  pub disable_live_bindings: Option<bool>,
  pub vite_mode: Option<bool>,
  pub resolve_new_url_to_asset: Option<bool>,
}
