use std::sync::Arc;

use rolldown_common::SharedNormalizedBundlerOptions;
use rolldown_plugin::SharedPluginDriver;

/// Used to store context information during the build process.
#[derive(Clone)]
pub struct BuildContext {
  pub(crate) options: SharedNormalizedBundlerOptions,
  pub(crate) plugin_driver: SharedPluginDriver,
}

impl BuildContext {
  /// Get the bundler options used in this build context.
  pub fn options(&self) -> &SharedNormalizedBundlerOptions {
    &self.options
  }

  /// Get the watch files collected during this build context.
  pub fn watch_files(&self) -> &Arc<rolldown_utils::dashmap::FxDashSet<arcstr::ArcStr>> {
    &self.plugin_driver.watch_files
  }

  pub fn plugin_driver(&self) -> &SharedPluginDriver {
    &self.plugin_driver
  }
}
