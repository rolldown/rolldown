use super::{
  binding_filter_expression::BindingFilterToken, binding_js_or_regex::BindingStringOrRegex,
  binding_module_type::BindingModuleType,
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Clone)]
pub struct BindingGeneralHookFilter {
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  // one Array of BindingFilterToken to construct a FilterExpression
  // use Array of Array of BindingFilterToken construct multiple FilterExpression
  pub custom: Option<Vec<Vec<BindingFilterToken>>>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default, Debug)]
pub struct BindingTransformHookFilter {
  pub code: Option<BindingGeneralHookFilter>,
  #[napi(ts_type = "Array<string>")]
  pub module_type: Option<Vec<BindingModuleType>>,
  pub id: Option<BindingGeneralHookFilter>,
  pub custom: Option<Vec<Vec<BindingFilterToken>>>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default, Clone, Debug)]
pub struct BindingRenderChunkHookFilter {
  pub code: Option<BindingGeneralHookFilter>,
}
