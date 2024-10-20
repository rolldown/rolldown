use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingWatchOption {
  pub skip_write: Option<bool>,
}

impl From<BindingWatchOption> for rolldown_common::WatchOption {
  fn from(value: BindingWatchOption) -> Self {
    Self { skip_write: value.skip_write.unwrap_or_default() }
  }
}
