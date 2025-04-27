use super::{binding_js_or_regex::BindingStringOrRegex, binding_module_type::BindingModuleType};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Clone, Debug)]
pub struct BindingGeneralHookFilter {
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default, Clone, Debug)]
pub struct BindingTransformHookFilter {
  pub code: Option<BindingGeneralHookFilter>,
  #[napi(ts_type = "Array<string>")]
  pub module_type: Option<Vec<BindingModuleType>>,
  pub id: Option<BindingGeneralHookFilter>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default, Clone, Debug)]
pub struct BindingRenderChunkHookFilter {
  pub code: Option<BindingGeneralHookFilter>,
}
