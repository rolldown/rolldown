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
}
