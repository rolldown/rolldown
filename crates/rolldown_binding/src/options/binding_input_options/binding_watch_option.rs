use std::time::Duration;

use serde::Deserialize;

use crate::options::plugin::types::binding_js_or_regex::{
  bindingify_string_or_regex_array, BindingStringOrRegex,
};

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingWatchOption {
  pub skip_write: Option<bool>,
  pub notify: Option<BindingNotifyOption>,
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
}

impl TryFrom<BindingWatchOption> for rolldown_common::WatchOption {
  type Error = anyhow::Error;

  fn try_from(value: BindingWatchOption) -> Result<Self, Self::Error> {
    Ok(Self {
      skip_write: value.skip_write.unwrap_or_default(),
      notify: value.notify.map(Into::into),
      include: value.include.map(bindingify_string_or_regex_array).transpose()?,
      exclude: value.exclude.map(bindingify_string_or_regex_array).transpose()?,
    })
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingNotifyOption {
  pub poll_interval: Option<u32>,
  pub compare_contents: Option<bool>,
}

impl From<BindingNotifyOption> for rolldown_common::NotifyOption {
  #[allow(clippy::cast_lossless)]
  fn from(value: BindingNotifyOption) -> Self {
    Self {
      poll_interval: value.poll_interval.map(|m| Duration::from_millis(m as u64)),
      compare_contents: value.compare_contents.unwrap_or_default(),
    }
  }
}
