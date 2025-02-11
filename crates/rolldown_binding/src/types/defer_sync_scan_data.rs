use rolldown_common::DeferSyncScanData;

use crate::options::plugin::types::binding_hook_side_effects::BindingHookSideEffects;

#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingDeferSyncScanData {
  /// ModuleId
  pub id: String,
  pub side_effects: Option<BindingHookSideEffects>,
}

impl From<BindingDeferSyncScanData> for DeferSyncScanData {
  fn from(data: BindingDeferSyncScanData) -> Self {
    DeferSyncScanData { id: data.id, side_effects: data.side_effects.map(Into::into) }
  }
}
