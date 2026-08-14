use std::sync::Arc;

use arcstr::ArcStr;
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct ClientSession {
  /// `shipped[C]`: module stable id → rebuild stamp of the copy this client holds.
  /// Written ONLY when the serving middleware observes a payload response complete
  /// (the delivery notification), never at render or push. `ArcStr` keys share the
  /// id strings with the stamp table and every other client's ship map.
  pub shipped: FxHashMap<ArcStr, u32>,
  /// Boot-evaluated map: module stable id → rebuild stamp of the copy the entry
  /// chunk evaluated at top level. Frozen at hello from the engine-wide snapshot
  /// (`DevContext::top_level_evaluated`) and never written again — a module that
  /// evaluates later got its factory through a payload, so `shipped` covers it
  /// with a fresher stamp. Shared, not owned: every client of one build points
  /// at the same map.
  pub top_level_evaluated: Arc<FxHashMap<ArcStr, u32>>,
  /// Per-client envelope sequence counter.
  pub next_seq: u32,
}
