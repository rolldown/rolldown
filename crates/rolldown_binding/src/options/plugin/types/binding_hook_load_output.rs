use rolldown::ModuleType;

use super::binding_hook_side_effects::BindingHookSideEffects;
use crate::types::binding_sourcemap::BindingSourcemap;

// This struct is used to both pass to JS and receive from JS:
// - Pass to JS: `From<HookLoadOutput>` impl in binding_callable_builtin_plugin.rs:218
// - Receive from JS: `TryFrom` impl (line 17) when JS plugins return load results
#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingHookLoadOutput {
  pub code: String,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<BindingHookSideEffects>,
  pub map: Option<BindingSourcemap>,
  pub module_type: Option<String>,
}

impl TryFrom<BindingHookLoadOutput> for rolldown_plugin::HookLoadOutput {
  type Error = anyhow::Error;

  fn try_from(value: BindingHookLoadOutput) -> Result<Self, Self::Error> {
    Ok(rolldown_plugin::HookLoadOutput {
      code: value.code.into(),
      map: value.map.map(TryInto::try_into).transpose()?,
      side_effects: value.module_side_effects.map(TryInto::try_into).transpose()?,
      module_type: value.module_type.map(|ty| ModuleType::from_str_with_fallback(ty.as_str())),
    })
  }
}
