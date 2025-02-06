use crate::options::plugin::types::binding_hook_side_effects::BindingHookSideEffects;

#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingDeferSyncScanData {
  pub side_effects: Option<BindingHookSideEffects>,
}
