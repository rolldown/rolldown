use napi::{Either, bindgen_prelude::Null};
use rolldown::ModuleType;
use rolldown_plugin::{HookTransformOutput, HookTransformOutputMap};

use super::binding_hook_side_effects::BindingHookSideEffects;
use crate::types::binding_sourcemap::BindingSourcemap;

// This struct is used to both pass to JS and receive from JS:
// - Pass to JS: `From<HookTransformOutput>` impl returned from builtin plugins
// - Receive from JS: `TryFrom` impl when JS plugins return transform results
#[napi_derive::napi(object)]
#[derive(Default, derive_more::Debug)]
pub struct BindingHookTransformOutput {
  pub code: Option<String>,
  pub module_side_effects: Option<BindingHookSideEffects>,
  /// A sourcemap, or `null` to explicitly signal "no sourcemap" (distinct from
  /// omitting the field, which mirrors Rollup's "possibly broken" semantics).
  pub map: Option<Either<BindingSourcemap, Null>>,
  pub module_type: Option<String>,
}

impl TryFrom<BindingHookTransformOutput> for HookTransformOutput {
  type Error = anyhow::Error;

  fn try_from(value: BindingHookTransformOutput) -> Result<Self, Self::Error> {
    let map = match value.map {
      Some(Either::A(map)) => HookTransformOutputMap::Sourcemap(
        TryInto::<rolldown_sourcemap::SourceMap>::try_into(map)?.into(),
      ),
      Some(Either::B(_)) => HookTransformOutputMap::Null,
      None => HookTransformOutputMap::Omitted,
    };
    Ok(Self {
      code: value.code,
      map,
      side_effects: value.module_side_effects.map(TryInto::try_into).transpose()?,
      module_type: value.module_type.map(|ty| ModuleType::from_str_with_fallback(ty.as_str())),
    })
  }
}

impl From<HookTransformOutput> for BindingHookTransformOutput {
  fn from(value: HookTransformOutput) -> Self {
    let map = match value.map {
      HookTransformOutputMap::Sourcemap(map) => Some(Either::A(map.to_json().into())),
      HookTransformOutputMap::Null => Some(Either::B(Null)),
      HookTransformOutputMap::Omitted => None,
    };
    Self {
      code: value.code,
      map,
      module_side_effects: value.side_effects.map(Into::into),
      module_type: value.module_type.map(|v| v.to_string()),
    }
  }
}
