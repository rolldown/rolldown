use arcstr::ArcStr;
use rustc_hash::FxHashMap;

/// Per-client input for selecting the factories an HMR push ships. The server never
/// sees execution state; it reads only its own records.
#[derive(Debug)]
pub struct ClientHmrInput<'a> {
  pub client_id: &'a str,
  /// The ship map `shipped[C]`: module stable id → rebuild stamp.
  pub shipped: &'a FxHashMap<ArcStr, u32>,
  pub top_level_evaluated: &'a FxHashMap<ArcStr, u32>,
}
