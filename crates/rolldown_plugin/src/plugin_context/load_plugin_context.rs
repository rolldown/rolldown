use std::{ops::Deref, sync::Arc};

use crate::PluginContext;
use rolldown_common::ModuleIdx;

#[derive(Debug)]
pub struct LoadPluginContext {
  pub inner: PluginContext,
  module_idx: ModuleIdx,
}

impl LoadPluginContext {
  pub fn new(inner: PluginContext, module_idx: ModuleIdx) -> Self {
    Self { inner, module_idx }
  }

  /// Add a file as a dependency.
  ///
  /// * file - The file to add as a watch dependency. This should be a normalized absolute path.
  pub fn add_watch_file(&self, file: &str) {
    // Call the parent method to add to global watch files
    self.inner.add_watch_file(file);

    // Also add to this module's transform dependencies for HMR invalidation
    if let crate::PluginContext::Native(ctx) = &self.inner {
      if let Some(plugin_driver) = ctx.plugin_driver.upgrade() {
        plugin_driver.add_transform_dependency(self.module_idx, file);
      }
    }
  }
}

impl Deref for LoadPluginContext {
  type Target = PluginContext;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

pub type SharedLoadPluginContext = Arc<LoadPluginContext>;
