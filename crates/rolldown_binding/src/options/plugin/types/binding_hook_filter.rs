use super::binding_js_or_regex::BindingStringOrRegex;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Clone, Debug)]
pub struct BindingGeneralHookFilter {
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default, Clone)]
pub struct BindingTransformHookFilter {
  pub code: Option<BindingGeneralHookFilter>,
  pub module_type: Option<Vec<String>>,
  pub id: Option<BindingGeneralHookFilter>,
}
