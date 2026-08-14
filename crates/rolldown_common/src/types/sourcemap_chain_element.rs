use arcstr::ArcStr;
use rolldown_sourcemap::SourceMap;

use crate::PluginIdx;

#[derive(Debug, Clone)]
pub enum SourcemapChainElement {
  /// A string representing the URL of an external source map.
  Transform((PluginIdx, SourceMap)),
  /// An inline source map represented as a JSON string.
  Load(SourceMap),
  /// A transform hook returned changed code without a sourcemap.
  Omitted { plugin_idx: PluginIdx, plugin_name: ArcStr },
  /// A transform hook returned changed code together with an explicit
  /// `map: null`.
  Null { plugin_idx: PluginIdx, original_content: ArcStr },
}
