#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingOptimization {
  pub inline_const: Option<bool>,
  pub pife_for_module_wrappers: Option<bool>,
  pub const_bindings: Option<bool>,
  pub reserved_names_as_props: Option<bool>,
  pub symbols: Option<bool>,
}

impl From<BindingOptimization> for rolldown_common::OptimizationOption {
  fn from(value: BindingOptimization) -> Self {
    Self {
      inline_const: value.inline_const,
      pife_for_module_wrappers: value.pife_for_module_wrappers,
      const_bindings: value.const_bindings,
      reserved_names_as_props: value.reserved_names_as_props,
      symbols: value.symbols,
    }
  }
}
