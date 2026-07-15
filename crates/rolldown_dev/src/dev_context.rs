use std::{future::Future, pin::Pin, sync::Arc};

use arcstr::ArcStr;
use futures::future::Shared;
use rolldown_common::HmrStampTable;
use rustc_hash::FxHashMap;
use tokio::sync::Mutex;

use crate::{
  NormalizedDevOptions, SharedClients, type_aliases::CoordinatorSender,
  types::pending_payload::PendingPayload,
};

pub type SharedDevContext = Arc<DevContext>;

pub type PinBoxSendStaticFuture<T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

// The future represents an ongoing `BundlingTask`
pub type BundlingFuture = Shared<PinBoxSendStaticFuture<()>>;

/// Keep at most this many rendered-but-undelivered payloads per client. A dropped
/// entry just degrades to the existing delivery-failure reload path: its modules
/// stay stale in the ship map, so a later push re-ships or full-reloads them.
const MAX_PENDING_PAYLOADS_PER_CLIENT: usize = 8;

pub struct DevContext {
  pub options: NormalizedDevOptions,
  pub coordinator_tx: CoordinatorSender,
  pub clients: SharedClients,
  /// Dev-engine-wide rebuild-stamp ship map of the versioned delivery protocol.
  pub stamp_table: Arc<Mutex<HmrStampTable>>,
  /// Rendered-but-not-yet-delivered payloads, keyed by output filename. The
  /// delivery notification consumes an entry when the serving middleware sees
  /// the response for that filename complete.
  pub pending_payloads: Arc<Mutex<FxHashMap<String, PendingPayload>>>,
  /// Boot-evaluated map of the latest written bundle output: module stable id →
  /// render stamp of the copy the entry chunk evaluates at top level (computed
  /// statically — see `Bundler::compute_top_level_evaluated_modules`). Swapped whole
  /// after every successful rebuild; `register_client` freezes the then-current
  /// `Arc` into the new session, since a hello can only come from the runtime
  /// inside a served entry chunk.
  pub top_level_evaluated: Mutex<Arc<FxHashMap<ArcStr, u32>>>,
}

impl DevContext {
  /// Record a rendered payload as pending so the delivery notification can
  /// max-merge its stamps into that client's `shipped[C]` once the serving
  /// middleware observes the response complete.
  ///
  /// Bounds per-client growth: past `MAX_PENDING_PAYLOADS_PER_CLIENT` entries
  /// the oldest ones are dropped — see the constant's doc for why that is safe.
  pub async fn insert_pending_payload(&self, filename: String, payload: PendingPayload) {
    let client_id = payload.client_id.clone();
    let mut pending_payloads = self.pending_payloads.lock().await;
    pending_payloads.insert(filename, payload);

    // Count first: the common case is far below the bound, so don't build the
    // eviction list (filename clones + id parses) until it is actually needed.
    let client_count =
      pending_payloads.values().filter(|payload| payload.client_id == client_id).count();
    if client_count > MAX_PENDING_PAYLOADS_PER_CLIENT {
      let mut client_entries = pending_payloads
        .iter()
        .filter(|(_, payload)| payload.client_id == client_id)
        .map(|(filename, _)| (patch_id_of(filename), filename.clone()))
        .collect::<Vec<_>>();
      client_entries.sort_unstable();
      for (_, filename) in &client_entries[..client_entries.len() - MAX_PENDING_PAYLOADS_PER_CLIENT]
      {
        pending_payloads.remove(filename);
      }
    }
  }
}

/// The numeric id embedded in a payload filename (`hmr_patch_{id}.js` /
/// `lazy_compile_{id}.js`). Both formats draw from the engine's single patch-id
/// counter, so the id orders pending entries by age across the two kinds.
fn patch_id_of(filename: &str) -> u32 {
  filename
    .rsplit('_')
    .next()
    .and_then(|rest| rest.strip_suffix(".js"))
    .and_then(|id| id.parse().ok())
    .unwrap_or(0)
}
