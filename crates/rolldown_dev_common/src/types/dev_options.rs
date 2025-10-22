use derive_more::Debug;
use rolldown_common::ClientHmrUpdate;
use rolldown_error::BuildResult;
use std::sync::Arc;

use super::bundle_output::BundleOutput;
use super::dev_watch_options::DevWatchOptions;
use super::rebuild_strategy::RebuildStrategy;

pub type OnHmrUpdatesCallback =
  Arc<dyn Fn(BuildResult<(Vec<ClientHmrUpdate>, Vec<String>)>) + Send + Sync>;
pub type OnOutputCallback = Arc<dyn Fn(BuildResult<BundleOutput>) + Send + Sync>;

pub type SharedNormalizedDevOptions = Arc<NormalizedDevOptions>;

#[derive(Debug, Default)]
pub struct DevOptions {
  #[debug(skip)]
  pub on_hmr_updates: Option<OnHmrUpdatesCallback>,
  #[debug(skip)]
  pub on_output: Option<OnOutputCallback>,
  pub rebuild_strategy: Option<RebuildStrategy>,
  pub watch: Option<DevWatchOptions>,
}

#[expect(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub struct NormalizedDevOptions {
  #[debug(skip)]
  pub on_hmr_updates: Option<OnHmrUpdatesCallback>,
  #[debug(skip)]
  pub on_output: Option<OnOutputCallback>,
  pub disable_watcher: bool,
  pub skip_write: bool,
  pub rebuild_strategy: RebuildStrategy,
  pub use_polling: bool,
  pub poll_interval: u64,
  pub use_debounce: bool,
  pub debounce_duration: u64,
  pub compare_contents_for_polling: bool,
  pub debounce_tick_rate: Option<u64>,
}

pub fn normalize_dev_options(options: DevOptions) -> NormalizedDevOptions {
  let watch_options = options.watch.unwrap_or_default();
  NormalizedDevOptions {
    on_hmr_updates: options.on_hmr_updates,
    on_output: options.on_output,
    disable_watcher: watch_options.disable_watcher.unwrap_or_default(),
    skip_write: watch_options.skip_write.unwrap_or_default(),
    rebuild_strategy: options.rebuild_strategy.unwrap_or_default(),
    use_polling: watch_options.use_polling.unwrap_or(false),
    poll_interval: watch_options.poll_interval.unwrap_or(100),
    use_debounce: watch_options.use_debounce.unwrap_or(true),
    debounce_duration: watch_options.debounce_duration.unwrap_or(10),
    compare_contents_for_polling: watch_options.compare_contents_for_polling.unwrap_or(false),
    debounce_tick_rate: watch_options.debounce_tick_rate,
  }
}
