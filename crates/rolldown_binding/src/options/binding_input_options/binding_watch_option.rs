use crate::options::plugin::types::binding_js_or_regex::{
  bindingify_string_or_regex_array, BindingStringOrRegex,
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingWatchOption {
  pub skip_write: Option<bool>,
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
}

impl From<BindingWatchOption> for rolldown_common::WatchOption {
  fn from(value: BindingWatchOption) -> Self {
    Self {
      skip_write: value.skip_write.unwrap_or_default(),
      include: value.include.map(bindingify_string_or_regex_array),
      exclude: value.exclude.map(bindingify_string_or_regex_array),
    }
  }
}
