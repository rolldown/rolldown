mod definitions;

pub use definitions::{
  Action, build_end::BuildEnd, build_start::BuildStart, hook_load_call_end::HookLoadCallEnd,
  hook_load_call_start::HookLoadCallStart, hook_resolve_id_call_end::HookResolveIdCallEnd,
  hook_resolve_id_call_start::HookResolveIdCallStart,
  hook_transform_call_end::HookTransformCallEnd, hook_transform_call_start::HookTransformCallStart,
};

use rustc_hash::FxHashMap;

scoped_tls::scoped_thread_local!(pub static PROVIDED_DATA: FxHashMap<String, String>);

pub(crate) fn serialize_with_provided_data<S>(
  filed_value: &&'static str,
  serializer: S,
) -> Result<S::Ok, S::Error>
where
  S: serde::Serializer,
{
  println!("serialize_with_provided_data: {}", filed_value);
  if filed_value.starts_with("${") && filed_value.ends_with("}") {
    let key = &filed_value[2..filed_value.len() - 1];
    PROVIDED_DATA.with(|data| {
      let Some(provided) = data.get(key) else {
        return Err(serde::ser::Error::custom(format!("Key {} not found in provided data", key)));
      };
      serializer.serialize_str(provided)
    })
  } else {
    Err(serde::ser::Error::custom(format!(
      "Invalid format for provided data: {}. It should looks like `${{...}}`",
      filed_value
    )))
  }
}
