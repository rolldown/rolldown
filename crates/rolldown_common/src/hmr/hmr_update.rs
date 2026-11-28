/// One update for every client. The server sends ids only: each client walks its own graph and
/// pulls the code it needs (`DevEngine::render_patch`). `FullReload` is for changes only the
/// server can see, e.g. a tsconfig change re-transforms every governed module, which no patch
/// can represent.
#[derive(Debug, Clone)]
pub enum HmrUpdate {
  Changed {
    /// The modules whose own output changed. The client walks from these.
    changed_ids: Vec<String>,
    /// Every module stamped at `generation`: the changed modules, the importers re-rendered
    /// because an importee's interop changed, and the newly added modules. A client drops its
    /// factory of each one.
    stale_ids: Vec<String>,
    generation: u32,
  },
  FullReload {
    reason: String,
  },
  /// For the hmr request, there're no actual actions that need to be done.
  Noop,
}

impl HmrUpdate {
  pub fn is_full_reload(&self) -> bool {
    matches!(self, Self::FullReload { .. })
  }
}
