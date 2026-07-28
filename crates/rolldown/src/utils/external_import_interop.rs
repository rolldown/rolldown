use rolldown_common::{
  ExternalInteropUse, ImportRecordIdx, NamedImport, NormalModule, Specifier, SymbolRef,
};

use crate::stages::link_stage::LinkStageOutput;

/// Check if a specific import specifier needs the `__toESM` helper.
/// Only namespace imports (`import * as foo`) and default imports (`import foo`)
/// need the `__toESM` helper. Named imports (`import { foo }`) do not need it.
pub fn specifier_needs_interop(specifier: &Specifier) -> bool {
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

/// Interop the inclusion pass recorded for an external module, keyed by its namespace ref.
///
/// See internal-docs/runtime-helpers/implementation.md.
///
/// [`external_import_needs_interop`] only sees the static imports written by modules that live in
/// the chunk being rendered. That misses every reference whose importing module was tree-shaken
/// away — a re-export shim (`import d from 'external'; export { d }`) is dropped as soon as the
/// external is side-effect free, yet the `<external_ns>.default` it produced still needs the
/// wrapper (issue #10069). Callers OR this into their own `named_imports`-derived answer.
///
/// Granularity is per-bundle, not per-chunk — inclusion runs before chunking. So one chunk reading
/// `ns.default` makes every chunk that emits this external wrap it, even a chunk that only reads a
/// named export (see the `named-user` chunk in the `reexport_default_import_of_external_multi_chunk`
/// fixture).
///
/// Reading a name through the wrapper still yields the same value — `__copyProps` installs getters
/// that forward to the original. The wrapper is *not* fully transparent though: `__toESM` returns a
/// fresh object (`__create(__getProtoOf(mod))`), and `__copyProps` eagerly walks
/// `__getOwnPropertyNames`/`__getOwnPropertyDescriptor`. A named-only chunk therefore observes a
/// different namespace identity, accessor-shaped descriptors instead of data properties, and — for
/// a CommonJS export implemented as a Proxy — extra `ownKeys`/`getOwnPropertyDescriptor` traps at
/// require time. Plain named access is unaffected; introspection and exotic exports are not.
///
/// Narrowing this to the chunk means recording *which* module observes the external and testing
/// chunk membership. Not done yet, because the observing module is frequently the eliminated shim
/// itself — it has no chunk to test against, so a naive membership test under-approximates, and
/// under-approximating here is what produced #10069, a silently wrong bundle. A correct narrowing
/// has to keep wrapping when the observer has no chunk, and only skip when every observer is live
/// and assigned elsewhere.
pub fn recorded_external_interop(
  link_output: &LinkStageOutput,
  external_namespace_ref: SymbolRef,
) -> Option<ExternalInteropUse> {
  let canonical_ref = link_output.symbol_db.canonical_ref_for(external_namespace_ref);
  link_output.used_external_symbols.interop_use(&canonical_ref)
}
