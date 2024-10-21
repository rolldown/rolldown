use std::time::Duration;

use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingWatchOption {
  pub skip_write: Option<bool>,
  pub notify: Option<BindingNotifyOption>,
}

impl From<BindingWatchOption> for rolldown_common::WatchOption {
  fn from(value: BindingWatchOption) -> Self {
    Self { skip_write: value.skip_write.unwrap_or_default(), notify: value.notify.map(Into::into) }
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
