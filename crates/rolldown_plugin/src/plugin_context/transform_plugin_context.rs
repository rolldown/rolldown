use std::{
  ops::Deref,
  sync::{Arc, OnceLock},
};

use crate::PluginContext;
use arcstr::ArcStr;
use rolldown_common::{ModuleIdx, PendingSourcemap, SourcemapChainElement, WatchPath};
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps, empty_sourcemap};
use rolldown_utils::unique_arc::WeakRef;
use string_wizard::{MagicString, SourceMapOptions};

#[derive(Debug)]
pub struct TransformPluginContext {
  pub inner: PluginContext,
  sourcemap_chain: WeakRef<Vec<SourcemapChainElement>>,
  original_code: ArcStr,
  id: ArcStr,
  module_idx: ModuleIdx,
  /// Whether the sourcemap worker runs. If it does, `send_magic_string` leaves
  /// the map to it instead of generating the map on the JS thread.
  has_sourcemap_worker: bool,
  /// Set by `send_magic_string`. The plugin driver adds it to the sourcemap
  /// chain once the hook returns. Only the JS thread sets it and the driver only
  /// reads it, so neither ever waits for the other.
  pending_sourcemap: OnceLock<Arc<PendingSourcemap>>,
}

impl TransformPluginContext {
  pub fn new(
    inner: PluginContext,
    sourcemap_chain: WeakRef<Vec<SourcemapChainElement>>,
    original_code: ArcStr,
    id: ArcStr,
    module_idx: ModuleIdx,
    has_sourcemap_worker: bool,
  ) -> Self {
    Self {
      inner,
      sourcemap_chain,
      original_code,
      id,
      module_idx,
      has_sourcemap_worker,
      pending_sourcemap: OnceLock::new(),
    }
  }

  pub fn get_combined_sourcemap(&self) -> SourceMap {
    self.sourcemap_chain.with_inner(|sourcemap_chain| {
      let empty_map = empty_sourcemap();
      let chain: Vec<&SourceMap> = sourcemap_chain
        .iter()
        .filter_map(|element| match element {
          SourcemapChainElement::Transform((_, sourcemap))
          | SourcemapChainElement::Load(sourcemap) => Some(sourcemap),
          // Generates the map on this thread if no earlier call did. The
          // sourcemap worker only gets it after the module's last hook.
          SourcemapChainElement::MagicString((_, pending)) => Some(pending.get()),
          SourcemapChainElement::Omitted { .. } => Some(&empty_map),
          SourcemapChainElement::Null { .. } => None,
        })
        .collect();
      match chain.as_slice() {
        [] => self.create_sourcemap(),
        [single] => (*single).clone(),
        // TODO Here could be cache result for pervious sourcemap_chain, only remapping new sourcemap chain
        _ => collapse_sourcemaps(&chain),
      }
    })
  }

  fn create_sourcemap(&self) -> SourceMap {
    let magic_string = MagicString::new(self.original_code.as_str());
    magic_string.source_map(SourceMapOptions {
      hires: string_wizard::Hires::Boundary,
      include_content: true,
      source: self.id.as_str().into(),
    })
  }

  /// Add a file or a directory as a watch dependency.
  pub fn add_watch_file(&self, file: &str) {
    // Skip all operations for virtual modules (starting with \0)
    // Virtual modules can't be refetched from disk during HMR
    if self.id.starts_with('\0') {
      return;
    }

    // Add to global watch files
    self.inner.add_watch_file(file);

    // Add to this module's transform dependencies
    if let crate::PluginContext::Native(ctx) = &self.inner {
      if let Some(plugin_driver) = ctx.plugin_driver.upgrade() {
        plugin_driver
          .add_transform_dependency(self.module_idx, WatchPath::new(file, &ctx.options.cwd));
      }
    }
  }

  /// Returns the map as JSON if there is no sourcemap worker. Otherwise the map
  /// is generated later and `None` is returned.
  pub fn send_magic_string(
    &self,
    magic_string: MagicString<'static>,
  ) -> anyhow::Result<Option<String>> {
    if !self.has_sourcemap_worker {
      return Ok(Some(
        magic_string.source_map(string_wizard::SourceMapOptions::default()).to_json_string(),
      ));
    }
    let pending = Arc::new(PendingSourcemap::new(self.id.clone(), magic_string));
    if self.pending_sourcemap.set(pending).is_err() {
      anyhow::bail!(
        "TransformPluginContext: `sendMagicString` can only be called once per transform hook"
      );
    }
    Ok(None)
  }

  /// The map registered by `send_magic_string` during this hook, if any.
  pub fn pending_sourcemap(&self) -> Option<Arc<PendingSourcemap>> {
    self.pending_sourcemap.get().cloned()
  }
}

impl Deref for TransformPluginContext {
  type Target = PluginContext;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

pub type SharedTransformPluginContext = Arc<TransformPluginContext>;
