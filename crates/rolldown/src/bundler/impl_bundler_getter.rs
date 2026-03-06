use super::Bundler;
use crate::SharedOptions;
use arcstr::ArcStr;
use rolldown_utils::dashmap::FxDashSet;
use std::sync::{Arc, LazyLock};

impl Bundler {
  pub fn options(&self) -> &SharedOptions {
    &self.bundle_factory.options
  }

  pub fn closed(&self) -> bool {
    self.closed
  }

  pub fn watch_files(&self) -> &Arc<FxDashSet<ArcStr>> {
    static EMPTY_SET: LazyLock<Arc<FxDashSet<ArcStr>>> =
      LazyLock::new(|| Arc::new(FxDashSet::default()));
    if let Some(last_bundle_handle) = &self.last_bundle_handle {
      &last_bundle_handle.plugin_driver.watch_files
    } else {
      &EMPTY_SET
    }
  }
}
