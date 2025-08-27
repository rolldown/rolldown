use rolldown_common::HmrUpdate;
use std::sync::Arc;

pub type OnHmrUpdatesCallback = Arc<dyn Fn(Vec<HmrUpdate>) + Send + Sync>;

#[derive(Default)]
pub struct DevOptions {
  pub on_hmr_updates: Option<OnHmrUpdatesCallback>,
}

pub struct NormalizedDevOptions {
  pub on_hmr_updates: Option<OnHmrUpdatesCallback>,
}

pub fn normalize_dev_options(options: DevOptions) -> NormalizedDevOptions {
  NormalizedDevOptions { on_hmr_updates: options.on_hmr_updates }
}
