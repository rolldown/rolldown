use arcstr::ArcStr;
use rustc_hash::FxHashMap;

/// Per-client input for selecting the factories an HMR push ships. The server never
/// sees execution state — the selection reads only `shipped[C]`, the record of the
/// server's own deliveries (module stable id → rebuild stamp of the copy this client
/// holds).
#[derive(Debug)]
pub struct ClientHmrInput<'a> {
  pub client_id: &'a str,
  /// The ship map `shipped[C]`: module stable id → rebuild stamp.
  pub shipped: &'a FxHashMap<ArcStr, u32>,
}
