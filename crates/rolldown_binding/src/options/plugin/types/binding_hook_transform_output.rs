use rolldown::ModuleType;
use rolldown_plugin::HookTransformOutput;

use super::binding_hook_side_effects::BindingHookSideEffects;
use crate::types::binding_sourcemap::BindingSourcemap;

#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingHookTransformOutput {
  pub code: Option<String>,
  pub side_effects: Option<BindingHookSideEffects>,
  pub map: Option<BindingSourcemap>,
  pub module_type: Option<String>,
}

impl TryFrom<BindingHookTransformOutput> for HookTransformOutput {
  type Error = anyhow::Error;

  fn try_from(value: BindingHookTransformOutput) -> Result<Self, Self::Error> {
    Ok(Self {
      code: value.code,
      map: value.map.map(TryInto::try_into).transpose()?,
      side_effects: value.side_effects.map(TryInto::try_into).transpose()?,
      module_type: value.module_type.map(|ty| ModuleType::from_str_with_fallback(ty.as_str())),
    })
  }
}

impl From<HookTransformOutput> for BindingHookTransformOutput {
  fn from(value: HookTransformOutput) -> Self {
    Self {
      code: value.code,
      map: value.map.map(|v| v.to_json().into()),
      side_effects: value.side_effects.map(Into::into),
      module_type: value.module_type.map(|v| v.to_string()),
    }
  }
}
