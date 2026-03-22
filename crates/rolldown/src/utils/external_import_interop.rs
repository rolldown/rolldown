use rolldown_common::{
  ImportRecordIdx, IndexModules, ModuleIdx, NamedImport, NormalModule, Specifier,
};

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
pub fn import_record_needs_interop(module: &NormalModule, rec_idx: ImportRecordIdx) -> bool {
  module
    .named_imports
    .values()
    .any(|import| import.record_idx == rec_idx && specifier_needs_interop(&import.imported))
}

/// Check if any of the importer modules that performs a default/namespace import
/// from an external module is in "node mode" (i.e., the importer is `.mjs`/`.mts`
/// or the closest `package.json` has `"type": "module"`).
///
/// When this returns `true`, `__toESM` should be called with the second argument
/// set to `1` (node mode), so that `default` is always bound to the full CJS
/// `module.exports` object rather than relying on the `__esModule` heuristic.
pub fn external_import_is_in_node_mode(
  named_imports: &[(ModuleIdx, NamedImport)],
  module_table: &IndexModules,
) -> bool {
  named_imports.iter().any(|(importer_idx, import)| {
    if !specifier_needs_interop(&import.imported) {
      return false;
    }
    module_table[*importer_idx]
      .as_normal()
      .is_some_and(|m| m.should_consider_node_esm_spec_for_static_import())
  })
}
