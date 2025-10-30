use rolldown_common::{NamedImport, Specifier};

/// Check if the named imports from an external module need the `__toESM` helper.
/// Only namespace imports (`import * as foo`) and default imports (`import foo`)
/// need the `__toESM` helper. Named imports (`import { foo }`) do not need it.
pub fn external_import_needs_interop(
  named_imports: &[(rolldown_common::ModuleIdx, NamedImport)],
) -> bool {
  named_imports.iter().any(|(_, import)| {
    matches!(import.imported, Specifier::Star)
      || matches!(import.imported, Specifier::Literal(ref name) if name.as_str() == "default")
  })
}
