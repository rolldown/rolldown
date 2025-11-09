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
    if let Some(last_bundle_context) = &self.last_bundle_context {
      &last_bundle_context.plugin_driver.watch_files
    } else {
      &EMPTY_SET
    }
  }
}
