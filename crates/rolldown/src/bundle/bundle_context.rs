use std::sync::Arc;

use rolldown_common::SharedNormalizedBundlerOptions;
use rolldown_plugin::SharedPluginDriver;

/// Used to store context information during the bundling process.
#[derive(Clone)]
pub struct BundleContext {
  pub(crate) options: SharedNormalizedBundlerOptions,
  pub(crate) plugin_driver: SharedPluginDriver,
}

impl BundleContext {
  /// Get the bundler options used in this bundle context.
  pub fn options(&self) -> &SharedNormalizedBundlerOptions {
    &self.options
  }

  /// Get the watch files collected during this bundle context.
  pub fn watch_files(&self) -> &Arc<rolldown_utils::dashmap::FxDashSet<arcstr::ArcStr>> {
    &self.plugin_driver.watch_files
  }

  pub fn plugin_driver(&self) -> &SharedPluginDriver {
    &self.plugin_driver
  }
}
