use arcstr::ArcStr;

#[derive(Debug)]
pub struct HmrLazyChunkOutput {
  pub code: String,
  pub filename: String,
  /// `(stable id, render-time stamp)` for every module this chunk carries — the
  /// pending-payload entry that the delivery-time ship-map write consumes when the
  /// serving middleware observes the response complete.
  pub carried: Vec<(ArcStr, u32)>,
}
