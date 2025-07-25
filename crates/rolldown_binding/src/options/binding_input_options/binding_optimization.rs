#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingOptimization {
  pub inline_const: Option<bool>,
}

impl From<BindingOptimization> for rolldown_common::OptimizationOption {
  fn from(value: BindingOptimization) -> Self {
    Self { inline_const: value.inline_const, pife_for_module_wrappers: None /* TODO */ }
  }
}
