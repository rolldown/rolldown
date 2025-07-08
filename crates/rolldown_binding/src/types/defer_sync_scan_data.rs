use rolldown_common::DeferSyncScanData;

use crate::options::plugin::types::binding_hook_side_effects::BindingHookSideEffects;

#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingDeferSyncScanData {
  /// ModuleId
  pub id: String,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub side_effects: Option<BindingHookSideEffects>,
}

impl TryFrom<BindingDeferSyncScanData> for DeferSyncScanData {
  type Error = napi::Error;

  fn try_from(data: BindingDeferSyncScanData) -> Result<Self, Self::Error> {
    Ok(Self { id: data.id, side_effects: data.side_effects.map(TryInto::try_into).transpose()? })
  }
}
