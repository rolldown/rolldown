use crate::HookTransformOutputMap;

#[derive(Debug)]
pub struct HookRenderChunkOutput {
  pub code: String,
  /// The sourcemap for the rendered chunk. Reuses [`HookTransformOutputMap`] so
  /// `renderChunk` can distinguish `map: null` (opt-out) from an omitted `map`
  /// (possibly broken sourcemap), mirroring Rollup.
  pub map: HookTransformOutputMap,
}
