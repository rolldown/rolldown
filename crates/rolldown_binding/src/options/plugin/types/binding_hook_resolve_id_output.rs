use super::{
  binding_hook_side_effects::BindingHookSideEffects,
  binding_resolved_external::BindingResolvedExternal,
};

#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingHookResolveIdOutput {
  pub id: String,
  pub external: Option<BindingResolvedExternal>,
  pub normalize_external_id: Option<bool>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<BindingHookSideEffects>,
}

impl TryFrom<BindingHookResolveIdOutput> for rolldown_plugin::HookResolveIdOutput {
  type Error = napi::Error;

  fn try_from(value: BindingHookResolveIdOutput) -> Result<Self, Self::Error> {
    Ok(Self {
      id: value.id.into(),
      external: value.external.map(TryInto::try_into).transpose()?,
      normalize_external_id: value.normalize_external_id,
      side_effects: value.module_side_effects.map(TryInto::try_into).transpose()?,
    })
  }
}
