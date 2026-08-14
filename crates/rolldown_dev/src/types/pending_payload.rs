use arcstr::ArcStr;

/// A rendered-but-not-yet-delivered payload: filename → the modules and render-time
/// stamps it carries. Consumed by the delivery notification when the middleware sees
/// the response for `filename` complete.
pub struct PendingPayload {
  pub client_id: String,
  pub modules: Vec<(ArcStr, u32)>,
}
