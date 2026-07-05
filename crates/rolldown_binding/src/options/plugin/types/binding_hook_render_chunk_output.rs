use napi::{Either, bindgen_prelude::Null};

use crate::types::binding_sourcemap::BindingSourcemap;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default, Debug)]
pub struct BindingHookRenderChunkOutput {
  pub code: String,
  /// A sourcemap, or `null` to explicitly signal "no sourcemap" (distinct from
  /// omitting the field, which mirrors Rollup's "possibly broken" semantics).
  pub map: Option<Either<BindingSourcemap, Null>>,
}

impl TryFrom<BindingHookRenderChunkOutput> for rolldown_plugin::HookRenderChunkOutput {
  type Error = anyhow::Error;

  fn try_from(value: BindingHookRenderChunkOutput) -> Result<Self, Self::Error> {
    let map = match value.map {
      Some(Either::A(map)) => rolldown_plugin::HookTransformOutputMap::Sourcemap(
        TryInto::<rolldown_sourcemap::SourceMap>::try_into(map)?.into(),
      ),
      Some(Either::B(_)) => rolldown_plugin::HookTransformOutputMap::Null,
      None => rolldown_plugin::HookTransformOutputMap::Omitted,
    };
    Ok(rolldown_plugin::HookRenderChunkOutput { code: value.code, map })
  }
}
