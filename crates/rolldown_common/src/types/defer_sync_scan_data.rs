use super::side_effects::HookSideEffects;

#[derive(Debug)]
pub struct DeferSyncScanData {
  pub side_effects: Option<HookSideEffects>,
}
