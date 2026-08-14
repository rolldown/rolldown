use crate::types::binding_sourcemap::{BindingJsonSourcemap, BindingSourcemap};

#[napi_derive::napi]
pub fn collapse_sourcemaps(
  sourcemap_chain: Vec<BindingSourcemap>,
) -> napi::Result<BindingJsonSourcemap> {
  let sourcemap_chain =
    sourcemap_chain.into_iter().map(TryInto::try_into).collect::<Result<Vec<_>, _>>()?;
  let collapsed =
    rolldown_sourcemap::collapse_sourcemaps(&sourcemap_chain.iter().collect::<Vec<_>>());
  Ok(collapsed.to_json().into())
}
