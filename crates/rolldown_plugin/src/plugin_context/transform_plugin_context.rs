use std::{ops::Deref, sync::Arc};

use crate::PluginContext;
use arcstr::ArcStr;
use rolldown_common::{ModuleIdx, SourcemapHires};
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps};
use rolldown_utils::unique_arc::WeakRef;
use string_wizard::{MagicString, SourceMapOptions};

#[derive(Debug)]
pub struct TransformPluginContext {
  pub inner: PluginContext,
  sourcemap_chain: WeakRef<Vec<SourceMap>>,
  original_code: ArcStr,
  id: ArcStr,
  module_idx: ModuleIdx,
}

impl TransformPluginContext {
  pub fn new(
    inner: PluginContext,
    sourcemap_chain: WeakRef<Vec<SourceMap>>,
    original_code: ArcStr,
    id: ArcStr,
    module_idx: ModuleIdx,
  ) -> Self {
    Self { inner, sourcemap_chain, original_code, id, module_idx }
  }

  pub fn get_combined_sourcemap(&self) -> SourceMap {
    self.sourcemap_chain.with_inner(|sourcemap_chain| {
      if sourcemap_chain.is_empty() {
        self.create_sourcemap()
      } else if sourcemap_chain.len() == 1 {
        sourcemap_chain.first().expect("should have one sourcemap").clone()
      } else {
        let sourcemap_chain = sourcemap_chain.iter().collect::<Vec<_>>();
        // TODO Here could be cache result for pervious sourcemap_chain, only remapping new sourcemap chain
        collapse_sourcemaps(&sourcemap_chain)
      }
    })
  }

  fn create_sourcemap(&self) -> SourceMap {
    let magic_string = MagicString::new(self.original_code.as_str());
    let hires = self
      .inner
      .options()
      .experimental
      .transform_hires_sourcemap
      .unwrap_or(SourcemapHires::Boolean(true));
    magic_string.source_map(SourceMapOptions {
      hires: hires.into(),
      include_content: true,
      source: self.id.as_str().into(),
    })
  }

  /// Add a file as a dependency.
  ///
  /// * file - The file to add as a watch dependency. This should be a normalized absolute path.
  pub fn add_watch_file(&self, file: &str) {
    // Call the parent method to add to global watch files
    self.inner.add_watch_file(file);

    // Also add to this module's transform dependencies
    if let crate::PluginContext::Native(ctx) = &self.inner {
      if let Some(plugin_driver) = ctx.plugin_driver.upgrade() {
        plugin_driver.add_transform_dependency(self.module_idx, file);
      }
    }
  }
}

impl Deref for TransformPluginContext {
  type Target = PluginContext;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

pub type SharedTransformPluginContext = Arc<TransformPluginContext>;
