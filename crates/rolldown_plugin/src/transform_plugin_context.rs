use std::sync::Arc;

use crate::PluginContext;
use arcstr::ArcStr;
use rolldown_sourcemap::SourceMap;
use rolldown_utils::unique_arc::WeakRef;

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

  // pub fn get_combined_sourcemap(&self) -> SourceMap {
  //   if self.sourcemap_chain.is_empty() {
  //     self.create_sourcemap()
  //   } else if self.sourcemap_chain.len() == 1 {
  //     // TODO (fix): clone is not necessary
  //     self.sourcemap_chain.first().expect("should have one sourcemap").clone()
  //   } else {
  //     let sourcemap_chain = self.sourcemap_chain.iter().collect::<Vec<_>>();
  //     // TODO Here could be cache result for pervious sourcemap_chain, only remapping new sourcemap chain
  //     collapse_sourcemaps(sourcemap_chain).unwrap_or_else(|| self.create_sourcemap())
  //   }
  // }

  // fn create_sourcemap(&self) -> SourceMap {
  //   let magic_string = MagicString::new(self.original_code);
  //   magic_string.source_map(SourceMapOptions {
  //     hires: true,
  //     include_content: true,
  //     source: self.id.into(),
  //   })
  // }
}

pub type SharedTransformPluginContext = Arc<TransformPluginContext>;
