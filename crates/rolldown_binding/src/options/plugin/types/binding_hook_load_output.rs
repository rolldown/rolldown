use rolldown::ModuleType;
use rolldown_plugin::HookLoadOutput;

use super::{
  binding_hook_side_effects::BindingHookSideEffects, binding_shared_string::BindingSharedString,
};
use crate::types::binding_sourcemap::BindingSourcemap;

// This struct is used to both pass to JS and receive from JS:
// - Pass to JS: `From<HookLoadOutput>` impl returned from builtin plugins
// - Receive from JS: `TryFrom` impl when JS plugins return load results
#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingHookLoadOutput {
  #[napi(ts_type = "string")]
  pub code: BindingSharedString,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<BindingHookSideEffects>,
  pub map: Option<BindingSourcemap>,
  pub module_type: Option<String>,
}

impl TryFrom<BindingHookLoadOutput> for HookLoadOutput {
  type Error = anyhow::Error;

  fn try_from(value: BindingHookLoadOutput) -> Result<Self, Self::Error> {
    Ok(Self {
      code: value.code.into(),
      map: value.map.map(TryInto::try_into).transpose()?,
      side_effects: value.module_side_effects.map(TryInto::try_into).transpose()?,
      module_type: value.module_type.map(|ty| ModuleType::from_str_with_fallback(ty.as_str())),
    })
  }
}

impl From<HookLoadOutput> for BindingHookLoadOutput {
  fn from(value: HookLoadOutput) -> Self {
    Self {
      code: value.code.into(),
      map: value.map.map(|map| map.to_json_string().into()),
      module_side_effects: value.side_effects.map(Into::into),
      module_type: value.module_type.map(|ty| ty.to_string()),
    }
  }
}
