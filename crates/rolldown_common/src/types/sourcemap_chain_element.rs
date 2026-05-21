use arcstr::ArcStr;
use rolldown_sourcemap::SourceMap;

use crate::PluginIdx;

#[derive(Debug, Clone)]
pub enum SourcemapChainElement {
  /// A sourcemap produced by a transform hook.
  Transform((PluginIdx, SourceMap)),
  /// A sourcemap produced by a load hook.
  Load(SourceMap),
  /// A transform hook returned changed code without a sourcemap (or with an
  /// explicit `map: null`). No sourcemap is stored — generating one as a hires
  /// identity map of the original code would be costly. During collapse this
  /// acts as an identity layer: `original_code` is the code the transform
  /// received as input, used to anchor the collapsed map's `sources` /
  /// `sourcesContent` and to bound it to the original line range.
  Identity { plugin_idx: PluginIdx, original_code: ArcStr },
}
