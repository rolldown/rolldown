use std::{ops::Deref, sync::Arc};

use crate::PluginContext;
use arcstr::ArcStr;
use rolldown_common::SourcemapHires;
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps};
use rolldown_utils::unique_arc::WeakRef;
use string_wizard::{MagicString, SourceMapOptions};

#[allow(unused)]
#[derive(Debug)]
pub struct TransformPluginContext {
  pub inner: PluginContext,
  sourcemap_chain: WeakRef<Vec<SourceMap>>,
  original_code: ArcStr,
  id: ArcStr,
}

impl TransformPluginContext {
  pub fn new(
    inner: PluginContext,
    sourcemap_chain: WeakRef<Vec<SourceMap>>,
    original_code: ArcStr,
    id: ArcStr,
  ) -> Self {
    Self { inner, sourcemap_chain, original_code, id }
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
        collapse_sourcemaps(sourcemap_chain)
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
}

impl Deref for TransformPluginContext {
  type Target = PluginContext;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

pub type SharedTransformPluginContext = Arc<TransformPluginContext>;
