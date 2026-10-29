use rolldown_plugin::HookResolveIdArgs;

use super::binding_import_attributes::{BindingImportAttributes, to_binding_import_attributes};

/// The extra arguments of `resolveDynamicImport`. Empty attributes and a raw id equal to the
/// importer are left out, and `bindingify-build-hooks.ts` fills them in.
#[napi_derive::napi(object, object_from_js = false)]
#[derive(Default, Debug)]
pub struct BindingHookResolveDynamicImportExtraArgs {
  /// The import attributes of the import being resolved. Omitted when empty.
  #[napi(ts_type = "Record<string, string>")]
  pub attributes: Option<BindingImportAttributes>,
  /// The importer without its import attributes. Omitted when equal to the importer.
  pub importer_raw_id: Option<String>,
  /// The import attributes of the importer itself. Omitted when empty.
  #[napi(ts_type = "Record<string, string>")]
  pub importer_attributes: Option<BindingImportAttributes>,
}

impl From<&HookResolveIdArgs<'_>> for BindingHookResolveDynamicImportExtraArgs {
  fn from(args: &HookResolveIdArgs<'_>) -> Self {
    Self {
      attributes: to_binding_import_attributes(args.attributes),
      importer_raw_id: args
        .importer_raw_id
        .filter(|raw_id| Some(*raw_id) != args.importer)
        .map(str::to_string),
      importer_attributes: to_binding_import_attributes(args.importer_attributes),
    }
  }
}
