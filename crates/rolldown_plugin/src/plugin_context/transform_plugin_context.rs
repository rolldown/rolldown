use std::{ops::Deref, sync::Arc};

use crate::PluginContext;
use arcstr::ArcStr;
use rolldown_common::{ModuleIdx, PluginIdx, SourceMapGenMsg, SourcemapChainElement};
use rolldown_sourcemap::{SourceMap, anchor_sourcemap_to_source, collapse_sourcemaps};
use rolldown_utils::unique_arc::WeakRef;
use std::sync::mpsc;
use string_wizard::{MagicString, SourceMapOptions};

#[derive(Debug)]
pub struct TransformPluginContext {
  pub inner: PluginContext,
  sourcemap_chain: WeakRef<Vec<SourcemapChainElement>>,
  original_code: ArcStr,
  id: ArcStr,
  module_idx: ModuleIdx,
  plugin_idx: PluginIdx,
  magic_string_tx: Option<Arc<mpsc::Sender<SourceMapGenMsg>>>,
}

impl TransformPluginContext {
  pub fn new(
    inner: PluginContext,
    sourcemap_chain: WeakRef<Vec<SourcemapChainElement>>,
    original_code: ArcStr,
    id: ArcStr,
    module_idx: ModuleIdx,
    plugin_idx: PluginIdx,
    magic_string_tx: Option<Arc<mpsc::Sender<SourceMapGenMsg>>>,
  ) -> Self {
    Self { inner, sourcemap_chain, original_code, id, module_idx, plugin_idx, magic_string_tx }
  }

  pub fn get_combined_sourcemap(&self) -> SourceMap {
    self.sourcemap_chain.with_inner(|sourcemap_chain| {
      // `Identity` elements (transforms that changed code without a sourcemap)
      // carry no map; collapse only the real maps.
      let mut real_maps = sourcemap_chain
        .iter()
        .filter_map(|element| match element {
          SourcemapChainElement::Transform((_, sourcemap))
          | SourcemapChainElement::Load(sourcemap) => Some(sourcemap),
          SourcemapChainElement::Identity { .. } => None,
        })
        .collect::<Vec<_>>();

      // TODO Here could be cache result for pervious sourcemap_chain, only remapping new sourcemap chain
      let collapsed = match real_maps.len() {
        // No real maps yet — fall back to a fresh identity map of the current code.
        0 => return self.create_sourcemap(),
        1 => real_maps.remove(0).clone(),
        _ => collapse_sourcemaps(&real_maps),
      };

      // A leading `Identity` layer means the collapsed map lost the original
      // source; re-anchor it the same way the module render path does.
      match sourcemap_chain.first() {
        Some(SourcemapChainElement::Identity { original_code, .. }) => {
          anchor_sourcemap_to_source(&collapsed, self.id.as_str(), original_code)
        }
        _ => collapsed,
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

  /// Add a file as a dependency.
  ///
  /// * file - The file to add as a watch dependency. This should be a normalized absolute path.
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
        plugin_driver.add_transform_dependency(self.module_idx, file);
      }
    }
  }

  pub fn send_magic_string(
    &self,
    magic_string: MagicString<'static>,
  ) -> Result<Option<String>, mpsc::SendError<SourceMapGenMsg>> {
    if let Some(tx) = self.magic_string_tx.as_ref() {
      tx.send(SourceMapGenMsg::MagicString(Box::new((
        self.module_idx,
        self.plugin_idx,
        magic_string,
      ))))
      .map(|()| None)
    } else {
      Ok(Some(magic_string.source_map(string_wizard::SourceMapOptions::default()).to_json_string()))
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
