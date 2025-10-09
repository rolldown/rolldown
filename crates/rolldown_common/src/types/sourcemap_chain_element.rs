use rolldown_sourcemap::SourceMap;

use crate::PluginIdx;

#[derive(Debug, Clone)]
pub enum SourcemapChainElement {
  /// A string representing the URL of an external source map.
  Transform((PluginIdx, SourceMap)),
  /// An inline source map represented as a JSON string.
  Load(SourceMap),
}
