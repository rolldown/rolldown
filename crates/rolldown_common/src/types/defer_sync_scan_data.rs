use super::side_effects::HookSideEffects;

#[derive(Debug)]
pub struct DeferSyncScanData {
  pub id: String,
  pub side_effects: Option<HookSideEffects>,
}
