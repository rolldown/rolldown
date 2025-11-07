use super::Bundler;
use crate::SharedOptions;
use arcstr::ArcStr;
use rolldown_utils::dashmap::FxDashSet;
use std::sync::Arc;

impl Bundler {
  pub fn options(&self) -> &SharedOptions {
    &self.build_factory.options
  }

  pub fn closed(&self) -> bool {
    self.closed
  }

  pub fn watch_files(&self) -> &Arc<FxDashSet<ArcStr>> {
    &self.build_factory.plugin_driver.watch_files
  }
}
