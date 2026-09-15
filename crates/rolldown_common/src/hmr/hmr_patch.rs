use arcstr::ArcStr;

#[derive(Debug, Clone)]
pub struct HmrPatch {
  pub code: String,
  pub filename: String,
  pub sourcemap: Option<String>,
  pub sourcemap_filename: Option<String>,
  /// Stable ids of the changed modules — the `changedIds` of the push envelope.
  /// The client walks from these on its own graph; the server decides nothing.
  pub changed_ids: Vec<String>,
  /// Per-client envelope sequence number.
  pub seq: u32,
  /// `(stable id, render-time stamp)` for every module this patch carries — the
  /// pending-payload entry that the delivery-time ship-map write consumes when the
  /// serving middleware observes the response complete.
  pub carried: Vec<(ArcStr, u32)>,
}
