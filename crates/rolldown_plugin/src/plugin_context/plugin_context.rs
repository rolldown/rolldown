use std::{
  ops::Deref,
  sync::{Arc, Weak},
};

use derive_more::Debug;
use rolldown_common::NormalizedBundlerOptions;
use rolldown_resolver::Resolver;

use crate::types::hook_resolve_id_skipped::HookResolveIdSkipped;

use super::PluginContextImpl;

#[derive(Debug, Clone)]
pub struct PluginContext(std::sync::Arc<PluginContextImpl>);

impl PluginContext {
  #[must_use]
  pub fn new_shared_with_skipped_resolve_calls(
    &self,
    skipped_resolve_calls: Vec<Arc<HookResolveIdSkipped>>,
  ) -> Self {
    Self(Arc::new(PluginContextImpl {
      skipped_resolve_calls,
      plugin_idx: self.plugin_idx,
      plugin_driver: Weak::clone(&self.plugin_driver),
      resolver: Arc::clone(&self.resolver),
      file_emitter: Arc::clone(&self.file_emitter),
      options: Arc::clone(&self.options),
      watch_files: Arc::clone(&self.watch_files),
      modules: Arc::clone(&self.modules),
      tx: Arc::clone(&self.tx),
    }))
  }

  pub fn options(&self) -> &NormalizedBundlerOptions {
    &self.options
  }

  pub fn resolver(&self) -> &Resolver {
    &self.resolver
  }
}

impl Deref for PluginContext {
  type Target = PluginContextImpl;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<PluginContextImpl> for PluginContext {
  fn from(ctx: PluginContextImpl) -> Self {
    Self(Arc::new(ctx))
  }
}
