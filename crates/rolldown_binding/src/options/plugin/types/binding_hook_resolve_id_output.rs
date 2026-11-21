use std::sync::Arc;

use rolldown_common::JsModuleMeta;

use super::{
  binding_hook_side_effects::BindingHookSideEffects,
  binding_import_attributes::{BindingImportAttributes, from_binding_import_attributes},
  binding_module_meta_entry::BindingModuleMetaEntry,
  binding_resolved_external::BindingResolvedExternal,
};

// This struct is used to both pass to JS and receive from JS:
// - Pass to JS: `From<HookResolveIdOutput>` impl in binding_callable_builtin_plugin.rs:200
// - Receive from JS: `TryFrom` impl (line 21) used when JS plugins return resolve results
#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingHookResolveIdOutput {
  pub id: String,
  pub external: Option<BindingResolvedExternal>,
  pub normalize_external_id: Option<bool>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<BindingHookSideEffects>,
  /// @internal Used to store package json path resolved by oxc resolver,
  /// we could get the related package json object via the path string.
  #[napi(ts_type = "string | null")]
  pub package_json_path: Option<String>,
  /// Replaces the import attributes of the import. `{}` removes them. Ignored without
  /// `experimental.importAttributes`.
  #[napi(ts_type = "Record<string, string>")]
  pub attributes: Option<BindingImportAttributes>,
  /// @internal See `ResolvedId::module_options_receipt`.
  pub module_options_receipt: Option<u32>,
  /// @internal The `meta` of this result, when the persistent cache records the hook call.
  pub meta_entries: Option<Vec<BindingModuleMetaEntry>>,
}

impl TryFrom<BindingHookResolveIdOutput> for rolldown_plugin::HookResolveIdOutput {
  type Error = napi::Error;

  fn try_from(value: BindingHookResolveIdOutput) -> Result<Self, Self::Error> {
    Ok(Self {
      id: value.id.into(),
      external: value.external.map(TryInto::try_into).transpose()?,
      normalize_external_id: value.normalize_external_id,
      side_effects: value.module_side_effects.map(TryInto::try_into).transpose()?,
      package_json_path: value.package_json_path,
      attributes: value.attributes.map(from_binding_import_attributes),
      module_options_receipt: value.module_options_receipt,
      js_meta: value
        .meta_entries
        .map(|entries| Arc::new(JsModuleMeta::new(entries.into_iter().map(Into::into).collect()))),
    })
  }
}
