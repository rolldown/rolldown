use serde::Deserialize;

use crate::options::plugin::types::binding_js_or_regex::{
  bindingify_string_or_regex_array, BindingStringOrRegex,
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingWatchOption {
  pub skip_write: Option<bool>,
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
}

impl TryFrom<BindingWatchOption> for rolldown_common::WatchOption {
  type Error = anyhow::Error;

  fn try_from(value: BindingWatchOption) -> Result<Self, Self::Error> {
    Ok(Self {
      skip_write: value.skip_write.unwrap_or_default(),
      include: value.include.map(bindingify_string_or_regex_array).transpose()?,
      exclude: value.exclude.map(bindingify_string_or_regex_array).transpose()?,
    })
  }
}
