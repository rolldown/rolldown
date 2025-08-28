use rolldown_common::HmrUpdate;
use std::sync::Arc;

pub type OnHmrUpdatesCallback = Arc<dyn Fn(Vec<HmrUpdate>) + Send + Sync>;

pub type SharedNormalizedDevOptions = Arc<NormalizedDevOptions>;

#[derive(Default)]
pub struct DevOptions {
  pub on_hmr_updates: Option<OnHmrUpdatesCallback>,
  /// If `true`, A rebuild will be always issued when a file changes.
  pub eager_rebuild: Option<bool>,
}

pub struct NormalizedDevOptions {
  pub on_hmr_updates: Option<OnHmrUpdatesCallback>,
  pub eager_rebuild: bool,
}

pub fn normalize_dev_options(options: DevOptions) -> NormalizedDevOptions {
  NormalizedDevOptions {
    on_hmr_updates: options.on_hmr_updates,
    eager_rebuild: options.eager_rebuild.unwrap_or_default(),
  }
}
