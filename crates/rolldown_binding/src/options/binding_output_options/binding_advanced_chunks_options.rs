use crate::types::binding_string_or_regex::BindingStringOrRegex;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingAdvancedChunksOptions {
  pub min_size: Option<f64>,
  pub min_share_count: Option<u32>,
  pub groups: Option<Vec<BindingMatchGroup>>,
  pub max_size: Option<f64>,
  pub min_module_size: Option<f64>,
  pub max_module_size: Option<f64>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingMatchGroup {
  pub name: String,
  pub test: Option<BindingStringOrRegex>,
  // pub share_count: Option<u32>,
  pub priority: Option<u32>,
  pub min_size: Option<f64>,
  pub min_share_count: Option<u32>,
  pub min_module_size: Option<f64>,
  pub max_module_size: Option<f64>,
  pub max_size: Option<f64>,
}
