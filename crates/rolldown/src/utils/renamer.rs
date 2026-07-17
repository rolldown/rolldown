use std::collections::hash_map::Entry;

use rustc_hash::{FxHashMap, FxHashSet};

use oxc::semantic::Scoping;
use oxc::syntax::keyword::{GLOBAL_OBJECTS, RESERVED_KEYWORDS};
use oxc_str::CompactStr;

use rolldown_common::{
  ExportsKind, ImportRecordMeta, ModuleIdx, NormalModule, OutputFormat, SymbolRef, SymbolRefDb,
  SymbolRefDbForModule, WrapKind,
};
use rolldown_utils::concat_string;

use crate::stages::link_stage::LinkStageOutput;
use crate::utils::chunk::conflict_resolver::ConflictResolver;

#[derive(Debug)]
pub struct Renamer<'name> {
  /// Shared conflict-suffix engine; owns the set of taken top-level names.
  resolver: ConflictResolver,
  /// Final symbol → name mappings.
  canonical_names: FxHashMap<SymbolRef, CompactStr>,
  symbol_db: &'name SymbolRefDb,
  /// Entry module index for this chunk, if any.
  entry_module_idx: Option<ModuleIdx>,
}

impl<'name> Renamer<'name> {
  pub fn new(
    base_module_index: Option<ModuleIdx>,
    symbol_db: &'name SymbolRefDb,
    format: OutputFormat,
  ) -> Self {
    // Port from https://github.com/rollup/rollup/blob/master/src/Chunk.ts#L1377-L1394.
    let mut manual_reserved = match format {
      OutputFormat::Esm => vec![],
      OutputFormat::Cjs => vec!["module", "require", "__filename", "__dirname", "exports"],
      OutputFormat::Iife | OutputFormat::Umd => vec!["exports"], // Also for AMD, but we don't support it yet.
    };
    // https://github.com/rollup/rollup/blob/bfbea66569491f5466fbba99de2ba6a0225f851b/src/Chunk.ts#L1359
    manual_reserved.extend(["Object", "Promise"]);

    Self {
      canonical_names: FxHashMap::default(),
      symbol_db,
      resolver: ConflictResolver::new(
        manual_reserved
          .iter()
          .chain(RESERVED_KEYWORDS.iter())
          .chain(GLOBAL_OBJECTS.iter())
          .map(|s| CompactStr::new(s)),
      ),
      entry_module_idx: base_module_index,
    }
  }

  /// Returns the canonical name for a symbol if it has an explicit entry in this renamer.
  ///
  /// Returns `None` when no explicit canonical name has been recorded for the symbol in
  /// this renamer, i.e. the symbol has not yet been processed by the renaming pass.
  /// Once a symbol is processed, it always has an explicit entry here, even if its
  /// canonical name is identical to its original name. Callers must treat all `None`
  /// cases identically and fall back to `symbol_db` to determine the effective name
  /// during code generation.
  pub fn get_canonical_name(&self, symbol_ref: SymbolRef) -> Option<&CompactStr> {
    let canonical_ref = self.symbol_db.canonical_ref_for(symbol_ref);
    self.canonical_names.get(&canonical_ref)
  }

  pub fn reserve(&mut self, name: CompactStr) {
    self.resolver.reserve(name);
  }

  /// Force a symbol to a specific name and reserve that name so no other symbol
  /// in this chunk can take it. Used by `experimental.minChunkSize` to give a
  /// duplicated leaf symbol the same globally-pinned name in every chunk it is
  /// copied into. Must be called before the normal deconfliction pass so the
  /// reservation wins over author symbols (including entry-module symbols, which
  /// otherwise bypass `accept` — `ConflictResolver::resolve` still checks the
  /// reserved `used` set first).
  pub fn pin_name(&mut self, symbol_ref: SymbolRef, name: CompactStr) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);
    self.resolver.reserve(name.clone());
    self.canonical_names.insert(canonical_ref, name);
  }

  /// Check if a candidate name is available for a top-level symbol without causing
  /// unintended variable capture in nested scopes.
  ///
  /// This function prevents a top-level symbol from being renamed to a name that
  /// already exists in a nested scope, which would cause the nested binding to
  /// "capture" references meant for the top-level symbol.
  ///
  /// # Rules
  ///
  /// 1. **Entry module symbols**: Always available. Shadowing conflicts are resolved
  ///    later by `NestedScopeRenamer` which renames the nested bindings instead.
  ///
  /// 2. **Facade symbols** (e.g., external module namespaces): Must not conflict with
  ///    entry module's nested scopes, since facade symbols can't be traced via references.
  ///
  /// 3. **Renamed candidates**: Must not conflict with the symbol's own module's nested
  ///    bindings. Original names are allowed to shadow (that's intentional by the author).
  ///
  /// # Example: Why renamed candidates must avoid nested bindings
  ///
  /// ```js
  /// // entry.js
  /// import { foo } from './dep.js';  // Suppose `foo` conflicts, try renaming to `foo$1`
  /// function bar(foo$1) {            // Nested binding `foo$1` exists!
  ///   console.log(foo$1);            // Would capture the wrong value
  /// }
  /// console.log(foo);                // Should reference the import
  /// ```
  ///
  /// If we renamed the import to `foo$1`, the nested parameter would capture it.
  /// So `is_name_available("foo$1", ...)` returns `false`, and we try `foo$2` instead.
  ///
  /// # Example: Why original names are allowed to shadow
  ///
  /// ```js
  /// // entry.js
  /// import { value } from './dep.js';  // Original name is `value`
  /// function helper(value) {           // Nested `value` intentionally shadows
  ///   return value * 2;                // Author intended to use parameter
  /// }
  /// console.log(value);                // Uses the import
  /// ```
  ///
  /// Here the author intentionally wrote a parameter named `value` that shadows the import.
  /// We allow this (`is_original_name = true`), so the import keeps its name `value`.
  #[inline]
  fn is_name_available_with(
    symbol_db: &SymbolRefDb,
    entry_module_idx: Option<ModuleIdx>,
    candidate_name: &str,
    symbol_ref: SymbolRef,
    is_original_name: bool,
  ) -> bool {
    if let Some(entry_idx) = entry_module_idx {
      if symbol_ref.owner == entry_idx {
        // Entry module symbols can use their original names freely - shadowing is
        // handled by reference-based renaming of nested bindings later
        return true;
      }
    }

    // Renamed candidates must not conflict with own module's nested bindings
    // (original names are allowed to shadow - that's intentional)
    if !is_original_name && has_nested_scope_binding(symbol_db, symbol_ref.owner, candidate_name) {
      return false;
    }

    true
  }

  /// Assign a canonical name to a top-level symbol, avoiding conflicts with
  /// other top-level names and nested scope names that could cause capture.
  pub fn add_symbol_in_root_scope(&mut self, symbol_ref: SymbolRef, needs_deconflict: bool) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);

    // The `!needs_deconflict` path always stores the bare original name and never
    // dedups, so build and insert directly.
    if !needs_deconflict {
      self.canonical_names.insert(canonical_ref, self.symbol_db.original_name(canonical_ref));
      return;
    }

    // Deconflict path: fuse the dedup check and the final insert into a single
    // `canonical_names` probe via the entry API. An Occupied slot means this
    // canonical_ref was already assigned, so re-adding is a no-op — and we still
    // skip building the owned name on that path (dedup-before-alloc). The previous
    // `contains_key(&canonical_ref)` + `insert(canonical_ref, _)` pair hashed and
    // walked the table twice for the same key.
    let Entry::Vacant(slot) = self.canonical_names.entry(canonical_ref) else {
      return;
    };

    let original_name = self.symbol_db.original_name(canonical_ref);

    // Bind the fields the `accept` closure reads as locals so the borrow of
    // `self.resolver` (mutable, in `resolve`) does not overlap a borrow of `self`.
    let symbol_db = self.symbol_db;
    let entry_module_idx = self.entry_module_idx;
    let resolved = self.resolver.resolve(original_name, |candidate, is_original| {
      Self::is_name_available_with(
        symbol_db,
        entry_module_idx,
        candidate,
        canonical_ref,
        is_original,
      )
    });
    slot.insert(resolved);
  }

  pub fn create_conflictless_name(&mut self, hint: &str) -> CompactStr {
    self.resolver.resolve(CompactStr::new(hint), |_, _| true)
  }

  pub fn register_nested_scope_symbols(&mut self, symbol_ref: SymbolRef, original_name: &str) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);
    if self.canonical_names.contains_key(&canonical_ref) {
      return;
    }

    // Find unique name: skip candidates that conflict with top-level symbols
    // or with existing bindings in nested scopes of the same module.
    for count in 1u32.. {
      let name: CompactStr =
        concat_string!(original_name, "$", itoa::Buffer::new().format(count)).into();

      if self.resolver.contains(&name) {
        continue;
      }

      // Also skip if the candidate name conflicts with an existing binding in
      // a nested scope of the same module. Without this check, renaming `child`
      // to `child$1` could collide with an existing `child$1` binding in the
      // same scope (e.g. from Gleam's variable shadowing convention).
      if has_nested_scope_binding(self.symbol_db, symbol_ref.owner, &name) {
        self.resolver.reserve(name);
        continue;
      }

      self.resolver.reserve(name.clone());
      self.canonical_names.insert(symbol_ref, name);
      return;
    }
  }

  /// Override the chunk-root name of a CJS closure-internal binding that shadows a chunk-root
  /// binding the closure references. The main deconfliction loop already gave this binding its
  /// original name (CJS root-scope symbols are exempt there, so `register_nested_scope_symbols`
  /// would skip it as already-named) — hence the override. The replacement skips names reserved at
  /// chunk scope and names used by any binding in the same module (root or nested); the latter
  /// stops it from landing on a sibling closure-local (the #9882 second-order case).
  pub fn override_root_scope_binding(
    &mut self,
    symbol_ref: SymbolRef,
    original_name: &str,
    scoping: &Scoping,
  ) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);
    for count in 1u32.. {
      let candidate: CompactStr =
        concat_string!(original_name, "$", itoa::Buffer::new().format(count)).into();
      if self.resolver.contains(&candidate) {
        continue;
      }
      if scoping.iter_bindings().any(|(_, bindings)| bindings.contains_key(candidate.as_str())) {
        // Reserve so a later-deconflicted chunk-root symbol can't be renamed onto this
        // module-binding name and then be shadowed by it.
        self.resolver.reserve(candidate);
        continue;
      }
      self.resolver.reserve(candidate.clone());
      self.canonical_names.insert(canonical_ref, candidate);
      return;
    }
  }

  #[inline]
  pub fn into_canonical_names(self) -> FxHashMap<SymbolRef, CompactStr> {
    self.canonical_names
  }
}

/// Returns true if `name` exists in any nested (non-root) scope of the module.
/// Returns false for modules without AST (external modules).
fn has_nested_scope_binding(symbol_db: &SymbolRefDb, module_idx: ModuleIdx, name: &str) -> bool {
  let Some(db) = &symbol_db[module_idx] else {
    return false;
  };
  // Skip root scope (index 0), check nested scopes only
  db.ast_scopes.scoping().iter_bindings().skip(1).any(|(_, bindings)| bindings.contains_key(name))
}

/// Context for renaming nested scope symbols that would shadow top-level symbols.
pub struct NestedScopeRenamer<'a, 'r> {
  pub module_idx: ModuleIdx,
  pub module: &'a NormalModule,
  pub db: &'a SymbolRefDbForModule,
  pub scoping: &'a Scoping,
  pub link_output: &'a LinkStageOutput,
  pub renamer: &'r mut Renamer<'a>,
}

impl NestedScopeRenamer<'_, '_> {
  /// Rename nested bindings that would capture star import member references.
  ///
  /// When a star import member (like `ns.foo`) is referenced inside a function,
  /// and a nested binding would capture that reference, the nested binding must be renamed.
  ///
  /// # Example (`argument-treeshaking-parameter-conflict`)
  ///
  /// ```js
  /// // dep.js
  /// export const mutate = () => value++;
  ///
  /// // main.js
  /// import * as dep from './dep';
  /// function test(mutate) {    // Parameter 'mutate' would capture dep.mutate
  ///   dep.mutate('hello');     // After bundling becomes: mutate("hello")
  /// }
  /// ```
  ///
  /// Output:
  /// ```js
  /// const mutate = () => value++;
  /// function test(mutate$1) {  // Parameter renamed to avoid capturing
  ///   mutate("hello");         // Correctly calls top-level mutate
  /// }
  /// ```
  pub fn rename_bindings_shadowing_star_imports(&mut self) {
    for member_expr_ref in
      self.link_output.metas[self.module_idx].resolved_member_expr_refs.values()
    {
      let Some(reference_id) = member_expr_ref.reference_id else {
        continue;
      };
      let current_reference = self.scoping.get_reference(reference_id);
      let Some(symbol) = current_reference.symbol_id() else {
        continue;
      };
      let Some(resolved_symbol) = member_expr_ref.resolved else {
        continue;
      };

      // Only check for shadowing if the symbol was processed by the renamer
      // (i.e. it has a canonical name entry and is rendered at the chunk's root scope).
      let Some(canonical_name) = self.renamer.get_canonical_name(resolved_symbol).cloned() else {
        continue;
      };

      for scope_id in self.scoping.scope_ancestors(current_reference.scope_id()) {
        if let Some(binding) = self.scoping.get_binding(scope_id, canonical_name.as_str().into())
          && binding != symbol
        {
          let symbol_ref = (self.module_idx, binding).into();
          self.renamer.register_nested_scope_symbols(symbol_ref, self.scoping.symbol_name(binding));
        }
      }
    }
  }

  /// Rename nested bindings that would capture renamed named imports.
  ///
  /// When a named import is renamed due to a top-level conflict, and a nested binding
  /// has the same name as the renamed import, that nested binding must be renamed
  /// to avoid capturing references.
  ///
  /// # Example (`basic_scoped`)
  ///
  /// ```js
  /// // a.js
  /// export const a = 'a.js';
  ///
  /// // main.js
  /// import { a as aJs } from './a';
  /// const a = 'main.js';       // Takes priority, so import renamed to a$1
  /// function foo(a$1) {        // Parameter would capture reference to aJs
  ///   return [a$1, a, aJs];
  /// }
  /// ```
  ///
  /// Output:
  /// ```js
  /// const a$1 = "a.js";        // Import renamed due to conflict
  /// const a = "main.js";
  /// function foo(a$1$1) {      // Parameter renamed to avoid capturing
  ///   return [a$1$1, a, a$1];  // aJs correctly resolves to `a$1`
  /// }
  /// ```
  pub fn rename_bindings_shadowing_named_imports(&mut self) {
    for (symbol_ref, _named_import) in &self.module.named_imports {
      if self.db.is_facade_symbol(symbol_ref.symbol) {
        continue;
      }

      // Only check for shadowing if the symbol was processed by the renamer
      // (i.e. it has a canonical name entry and is rendered at the chunk's root scope).
      let Some(canonical_name) = self.renamer.get_canonical_name(*symbol_ref).cloned() else {
        continue;
      };

      for reference in self.scoping.get_resolved_references(symbol_ref.symbol) {
        for scope_id in self.scoping.scope_ancestors(reference.scope_id()) {
          if let Some(binding) = self.scoping.get_binding(scope_id, canonical_name.as_str().into())
            && binding != symbol_ref.symbol
          {
            let nested_symbol_ref = (self.module_idx, binding).into();
            self
              .renamer
              .register_nested_scope_symbols(nested_symbol_ref, self.scoping.symbol_name(binding));
          }
        }
      }
    }
  }

  /// Rename nested bindings that would shadow CJS wrapper parameters.
  ///
  /// For CommonJS wrapped modules, nested scopes must avoid shadowing the synthetic
  /// `exports` and `module` parameters injected by the CJS wrapper.
  ///
  /// # Example
  ///
  /// ```js
  /// // cjs-module.js (detected as CommonJS)
  /// function helper() {
  ///   const exports = {};  // Would shadow CJS wrapper's exports parameter
  ///   return exports;
  /// }
  /// module.exports = helper;
  /// ```
  ///
  /// Output:
  /// ```js
  /// var require_cjs = __commonJS((exports, module) => {
  ///   function helper() {
  ///     const exports$1 = {};  // Renamed to avoid shadowing
  ///     return exports$1;
  ///   }
  ///   module.exports = helper;
  /// });
  /// ```
  /// Rename nested bindings that would shadow wrapper/factory parameters.
  ///
  /// This handles two cases:
  /// 1. CJS wrapper params ("exports", "module") for CJS-wrapped modules
  /// 2. External module factory params for IIFE/UMD/CJS formats
  ///
  /// # Example (external module)
  ///
  /// ```js
  /// // entry.js
  /// import Quill from 'quill';
  /// export class Editor {
  ///   constructor(quill) {     // Would shadow factory param 'quill'
  ///     console.log(Quill);    // After bundling: quill.default (shadowed!)
  ///   }
  /// }
  /// ```
  ///
  /// Output (fixed):
  /// ```js
  /// (function(exports, quill) {
  ///   class Editor {
  ///     constructor(quill$1) {   // Renamed to avoid shadowing
  ///       console.log(quill.default);  // Correctly references factory param
  ///     }
  ///   }
  /// })
  /// ```
  pub fn rename_bindings_shadowing_wrapper_params(&mut self, has_factory_params: bool) {
    /// CJS wrapper parameter names that nested scopes should avoid shadowing.
    const CJS_WRAPPER_NAMES: [&str; 2] = ["exports", "module"];

    let is_cjs_wrapped =
      matches!(self.link_output.metas[self.module_idx].wrap_kind(), WrapKind::Cjs);

    // Collect all wrapper/factory param names to check against
    let mut wrapper_param_names: FxHashSet<CompactStr> = FxHashSet::default();

    // Add CJS wrapper names if module is CJS wrapped
    if is_cjs_wrapped {
      wrapper_param_names.extend(CJS_WRAPPER_NAMES.iter().map(|s| CompactStr::new(s)));
    }

    // Add external module factory param names
    if has_factory_params {
      wrapper_param_names.extend(self.module.import_records.iter().filter_map(|rec| {
        let resolved_module = rec.resolved_module?;
        let external_module = self.link_output.module_table[resolved_module].as_external()?;
        self.renamer.get_canonical_name(external_module.namespace_ref).cloned()
      }));
    }

    if wrapper_param_names.is_empty() {
      return;
    }

    // Skip root scope (index 0), check nested scopes only
    for (_, bindings) in self.scoping.iter_bindings().skip(1) {
      for (&name, symbol_id) in bindings {
        if wrapper_param_names.contains(name.into()) {
          let symbol_ref = (self.module_idx, *symbol_id).into();
          self.renamer.register_nested_scope_symbols(symbol_ref, name.as_str());
        }
      }
    }
  }

  /// Rename a CJS-wrapped module's *root-scope* locals that shadow a chunk-root binding the closure
  /// actually references.
  ///
  /// A CJS module's real top-level statements render *inside* its `__commonJS((exports, module) =>
  /// { ... })` closure, and that closure captures every name at chunk-root scope. The main
  /// deconfliction loop leaves these root-scope locals with their original name (CJS root-scope
  /// symbols are exempt there), so a same-named binding rendered at chunk root ends up shadowed
  /// inside the closure — issue #9882. Unlike the other passes, the shadowing binding lives at
  /// module-root scope and is already named, so we *override* it via `override_root_scope_binding`
  /// (which also steers clear of sibling locals — the #9882 second-order case).
  ///
  /// This is reference-precise: a root-scope local is renamed only when the module genuinely
  /// references a chunk-root binding of the same final name, leaving unrelated same-named locals
  /// untouched.
  pub fn rename_cjs_locals_shadowing_referenced_chunk_bindings(&mut self) {
    if !matches!(self.link_output.metas[self.module_idx].wrap_kind(), WrapKind::Cjs) {
      return;
    }
    let root_scope_id = self.scoping.root_scope_id();

    // Resolve against the immutable views first, then override (a `&mut self` step). A *root-scope*
    // local shadows a referenced chunk-root binding when it shares that binding's final name. The
    // `binding != reference` guard is essential: the referenced binding is itself a root-scope
    // binding (e.g. `import * as m` accessed as `m.default`), so without it we'd "rename" the
    // reference onto itself (issue #7444). Looking only at the root scope keeps us off bindings the
    // other (nested-scope) passes already handle.
    let mut shadowing_locals: FxHashSet<SymbolRef> = FxHashSet::default();

    // `import { x } from ...` — a root-scope local sharing the import's final name shadows it.
    for (import_ref, _) in &self.module.named_imports {
      if self.db.is_facade_symbol(import_ref.symbol) {
        continue;
      }
      // Unused imports aren't referenced, so nothing shadows them.
      if self.scoping.get_resolved_references(import_ref.symbol).next().is_none() {
        continue;
      }
      let Some(canonical_name) = self.renamer.get_canonical_name(*import_ref).cloned() else {
        continue;
      };
      if let Some(binding) = self.scoping.get_binding(root_scope_id, canonical_name.as_str().into())
        && binding != import_ref.symbol
      {
        let local_ref: SymbolRef = (self.module_idx, binding).into();
        if self.link_output.symbol_db.canonical_ref_for(local_ref).owner == self.module_idx {
          shadowing_locals.insert(local_ref);
        }
      }
    }

    // `ns.foo` star-import member accesses — a root-scope local sharing the resolved export's final
    // name shadows the namespace reference.
    for member_expr_ref in
      self.link_output.metas[self.module_idx].resolved_member_expr_refs.values()
    {
      let Some(reference_id) = member_expr_ref.reference_id else {
        continue;
      };
      let Some(reference_symbol) = self.scoping.get_reference(reference_id).symbol_id() else {
        continue;
      };
      let Some(resolved_symbol) = member_expr_ref.resolved else {
        continue;
      };
      let Some(canonical_name) = self.renamer.get_canonical_name(resolved_symbol).cloned() else {
        continue;
      };
      if let Some(binding) = self.scoping.get_binding(root_scope_id, canonical_name.as_str().into())
        && binding != reference_symbol
      {
        let local_ref: SymbolRef = (self.module_idx, binding).into();
        if self.link_output.symbol_db.canonical_ref_for(local_ref).owner == self.module_idx {
          shadowing_locals.insert(local_ref);
        }
      }
    }

    // `require()` of a wrapped-ESM importee — the finalizer rewrites it to
    // `(init_x(), __toCommonJS(xxx_exports))`, so a root-scope local sharing the importee's
    // namespace-object final name shadows that read (issue #9882, require()/namespace channel).
    // We mirror the finalizer's gate (`module_finalizers/mod.rs`): a non-CommonJS importee whose
    // require is actually used. The namespace object lives in the importee module, never in
    // `self.module`, so any same-named root-scope local here is a genuine shadowing local.
    for rec in &self.module.import_records {
      let Some(importee_idx) = rec.resolved_module else {
        continue;
      };
      if rec.meta.contains(ImportRecordMeta::IsRequireUnused) {
        continue;
      }
      let Some(importee) = self.link_output.module_table[importee_idx].as_normal() else {
        continue;
      };
      if matches!(importee.exports_kind, ExportsKind::CommonJs) {
        continue;
      }
      let Some(canonical_name) =
        self.renamer.get_canonical_name(importee.namespace_object_ref).cloned()
      else {
        continue;
      };
      if let Some(binding) = self.scoping.get_binding(root_scope_id, canonical_name.as_str().into())
      {
        let local_ref: SymbolRef = (self.module_idx, binding).into();
        if self.link_output.symbol_db.canonical_ref_for(local_ref).owner == self.module_idx {
          shadowing_locals.insert(local_ref);
        }
      }
    }

    for local_ref in shadowing_locals {
      let original_name = self.scoping.symbol_name(local_ref.symbol);
      self.renamer.override_root_scope_binding(local_ref, original_name, self.scoping);
    }
  }
}
