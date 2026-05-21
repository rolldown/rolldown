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
  ///
  /// Mirrors Rollup's `{ missing: true, plugin }` chain entry: when this
  /// element is collapsed during chunk generation, it breaks the chain
  /// (behaves like an empty sourcemap) and emits a `SOURCEMAP_BROKEN`
  /// warning — but only if the owning module's code actually contributes
  /// to a rendered chunk and sourcemap output is enabled. Deferring the
  /// warning to collapse time matches Rollup, which skips the warning for
  /// modules whose transformed code never reaches a chunk (e.g. an HTML
  /// entry rewritten to side-effect-only imports).
  Omitted { plugin_idx: PluginIdx, plugin_name: ArcStr },
}
