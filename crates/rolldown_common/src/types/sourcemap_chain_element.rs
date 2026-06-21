use arcstr::ArcStr;
use rolldown_sourcemap::SourceMap;

use crate::PluginIdx;

#[derive(Debug, Clone)]
pub enum SourcemapChainElement {
  /// A string representing the URL of an external source map.
  ///
  /// `SourceMap` is ~200 bytes, so it is boxed to keep this enum small: a
  /// `Vec<SourcemapChainElement>` is stored per module and the `Omitted`/`Null`
  /// variants would otherwise pay for the largest variant on every element.
  Transform((PluginIdx, Box<SourceMap>)),
  /// An inline source map represented as a JSON string.
  Load(Box<SourceMap>),
  /// A transform hook returned changed code without a sourcemap.
  Omitted { plugin_idx: PluginIdx, plugin_name: ArcStr },
  /// A transform hook returned changed code together with an explicit
  /// `map: null`.
  Null { plugin_idx: PluginIdx, original_content: ArcStr },
}

// Boxing the `SourceMap`-carrying variants keeps this enum tiny (the unboxed
// form was 224 bytes, dominated by the inline `SourceMap`). Guard against a
// regression that re-inlines a large payload.
const _: () = assert!(
  std::mem::size_of::<SourcemapChainElement>() <= 24,
  "SourcemapChainElement must stay small; box large variants instead of inlining them"
);
