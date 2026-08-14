use arcstr::ArcStr;

#[derive(Debug)]
pub struct HmrLazyChunkOutput {
  pub code: String,
  pub filename: String,
  /// The chunk's sourcemap, when `sourcemap` is `File` or `Hidden`. The consumer
  /// serves it under `sourcemap_filename`, which is what the chunk's
  /// `sourceMappingURL` refers to.
  pub sourcemap: Option<String>,
  pub sourcemap_filename: Option<String>,
  /// `(stable id, render-time stamp)` for every module this chunk carries — the
  /// pending-payload entry that the delivery-time ship-map write consumes when the
  /// serving middleware observes the response complete.
  pub carried: Vec<(ArcStr, u32)>,
}
