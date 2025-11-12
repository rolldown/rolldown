use rolldown_common::{ExportsKind, ImportRecordIdx, ModuleIdx, NamedImport, NormalModule, Specifier};

use crate::types::linking_metadata::LinkingMetadataVec;

/// Check if a specific import specifier needs the `__toESM` helper.
/// Only namespace imports (`import * as foo`) and default imports (`import foo`)
/// need the `__toESM` helper. Named imports (`import { foo }`) do not need it.
fn specifier_needs_interop(specifier: &Specifier) -> bool {
  matches!(specifier, Specifier::Star)
    || matches!(specifier, Specifier::Literal(name) if name.as_str() == "default")
}

/// Check if the named imports from an external module need the `__toESM` helper.
pub fn external_import_needs_interop(
  named_imports: &[(rolldown_common::ModuleIdx, NamedImport)],
) -> bool {
  named_imports.iter().any(|(_, import)| specifier_needs_interop(&import.imported))
}

/// Check if an import record from a module needs the `__toESM` helper.
/// Only namespace imports (`import * as foo`) and default imports (`import foo`)
/// need the `__toESM` helper. Named imports (`import { foo }`) do not need it.
pub fn import_record_needs_interop(module: &NormalModule, rec_id: ImportRecordIdx) -> bool {
  module
    .named_imports
    .values()
    .any(|import| import.record_id == rec_id && specifier_needs_interop(&import.imported))
}

/// Check if a require() call needs the `__toCommonJS` helper.
/// The helper can be skipped if the required module is an ESM module that explicitly
/// exports a 'module.exports' property, because `__toCommonJS` will just return that
/// property directly at runtime.
pub fn require_needs_to_commonjs(
  importee_idx: ModuleIdx,
  importee_exports_kind: ExportsKind,
  linking_infos: &LinkingMetadataVec,
) -> bool {
  // CommonJS modules don't need __toCommonJS
  if importee_exports_kind.is_commonjs() {
    return false;
  }

  // ESM modules need __toCommonJS unless they export 'module.exports'
  let importee_linking_info = &linking_infos[importee_idx];
  let has_module_exports_export =
    importee_linking_info.resolved_exports.contains_key("module.exports");

  !has_module_exports_export
}
