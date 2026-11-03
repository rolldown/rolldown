use arcstr::ArcStr;

/// A rendered-but-not-yet-delivered payload: filename → the modules and render-time
/// stamps it carries. Consumed by the delivery notification for `filename`
/// (`DevEngine::notify_payload_delivered`).
pub struct PendingPayload {
  pub client_id: String,
  pub modules: Vec<(ArcStr, u32)>,
}
