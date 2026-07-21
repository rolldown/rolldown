use napi_derive::napi;

#[napi(discriminant = "type", object_from_js = false)]
#[derive(Debug)]
pub enum BindingHmrUpdate {
  Patch {
    code: String,
    filename: String,
    sourcemap: Option<String>,
    sourcemap_filename: Option<String>,
    /// Stable ids of the changed modules — the `changedIds` of the push envelope.
    /// The client walks from these on its own graph.
    changed_ids: Vec<String>,
    /// Per-client envelope sequence number.
    seq: u32,
  },
  FullReload {
    reason: Option<String>,
  },
  Noop,
}

impl From<rolldown_common::HmrUpdate> for BindingHmrUpdate {
  fn from(value: rolldown_common::HmrUpdate) -> Self {
    match value {
      // `carried` stays server-side: it feeds the engine's pending-payload
      // bookkeeping and is never exposed to JS.
      rolldown_common::HmrUpdate::Patch(patch) => Self::Patch {
        code: patch.code,
        filename: patch.filename,
        sourcemap: patch.sourcemap,
        sourcemap_filename: patch.sourcemap_filename,
        changed_ids: patch.changed_ids,
        seq: patch.seq,
      },
      rolldown_common::HmrUpdate::FullReload { reason } => {
        Self::FullReload { reason: Some(reason) }
      }
      rolldown_common::HmrUpdate::Noop => Self::Noop,
    }
  }
}
