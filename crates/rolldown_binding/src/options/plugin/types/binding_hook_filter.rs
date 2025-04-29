use super::binding_filter_expression::BindingFilterToken;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Clone)]
pub struct BindingHookFilter {
  pub value: Option<Vec<Vec<BindingFilterToken>>>,
}
