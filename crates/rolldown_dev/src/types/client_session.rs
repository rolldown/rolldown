use arcstr::ArcStr;
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct ClientSession {
  /// `shipped[C]`: module stable id → rebuild stamp of the copy this client holds.
  /// Written ONLY when the serving middleware observes a payload response complete
  /// (the delivery notification), never at render or push. `ArcStr` keys share the
  /// id strings with the stamp table and every other client's ship map.
  pub shipped: FxHashMap<ArcStr, u32>,
  /// Per-client envelope sequence counter.
  pub next_seq: u32,
}
