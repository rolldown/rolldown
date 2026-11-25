use napi_derive::napi;

/// One update for every client. The client walks its own graph from `changedIds` and pulls the
/// code it needs with `renderPatch`.
#[napi(discriminant = "type", object_from_js = false)]
#[derive(Debug)]
pub enum BindingHmrUpdate {
  Changed {
    changed_ids: Vec<String>,
    /// Every module stamped at `gen`. A client drops its factory of each one.
    stale_ids: Vec<String>,
    #[napi(js_name = "gen")]
    generation: u32,
  },
  FullReload {
    reason: Option<String>,
  },
  Noop,
}

impl From<rolldown_common::HmrUpdate> for BindingHmrUpdate {
  fn from(value: rolldown_common::HmrUpdate) -> Self {
    match value {
      rolldown_common::HmrUpdate::Changed { changed_ids, stale_ids, generation } => {
        Self::Changed { changed_ids, stale_ids, generation }
      }
      rolldown_common::HmrUpdate::FullReload { reason } => {
        Self::FullReload { reason: Some(reason) }
      }
      rolldown_common::HmrUpdate::Noop => Self::Noop,
    }
  }
}
