use std::cell::OnceCell;
use std::collections::hash_map::Entry;

use rustc_hash::{FxHashMap, FxHashSet};

use oxc::semantic::Scoping;
use oxc::syntax::keyword::{GLOBAL_OBJECTS, RESERVED_KEYWORDS};
use oxc_str::{CompactStr, Ident, IdentHashSet};

use rolldown_common::{
  ModuleIdx, NormalModule, OutputFormat, SymbolRef, SymbolRefDb, SymbolRefDbForModule, WrapKind,
};
use rolldown_utils::concat_string;

use crate::stages::link_stage::LinkStageOutput;
use crate::utils::chunk::conflict_resolver::ConflictResolver;

/// The kind of a top-level binding. The kind decides how the renamer gives the binding a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootBindingKind {
  /// Source code declares the binding, or the binding gets its name from the source (an external
  /// import binding). The binding keeps its original name when that name is free at the top level.
  Authored,
  /// Rolldown makes the binding: for example a wrapper binding, a namespace object, an interop
  /// binding or a runtime helper. The finalizer prints references to it at any depth. Thus it
  /// never takes a name that an inner binding uses ([`InnerBindingNames`]), and no source binding
  /// can capture these references.
  Synthesized,
}

/// The names that the *inner* scopes of the chunk bind. Inner scopes are the scopes below the top
/// level of the chunk in the output: the nested scopes of each module, and the root scope of each
/// CJS-wrapped module. The finalizer prints that root scope as the body of the
/// `__commonJS((exports, module) => { ... })` closure.
///
/// The renamer builds this set on first use. Thus a chunk with no synthesized binding does not pay
/// for it.
pub struct InnerBindingNames<'a> {
  /// Each module's scoping, and whether its root scope is an inner scope.
  scopings: Vec<(&'a Scoping, bool)>,
  names: OnceCell<IdentHashSet<'a>>,
}

impl<'a> InnerBindingNames<'a> {
  pub fn new(scopings: Vec<(&'a Scoping, bool)>) -> Self {
    Self { scopings, names: OnceCell::new() }
  }

  fn contains(&self, name: &str) -> bool {
    self
      .names
      .get_or_init(|| {
        let mut names = IdentHashSet::default();
        for (scoping, root_is_inner) in &self.scopings {
          // Walk the symbol table rather than each scope's bindings map: it is one contiguous
          // array, and nearly every symbol is a binding of some scope.
          let root_scope_id = scoping.root_scope_id();
          for symbol_id in scoping.symbol_ids() {
            let name = scoping.symbol_ident(symbol_id);
            if scoping.symbol_scope_id(symbol_id) == root_scope_id
              // Facades are created in the root scope but never bound there.
              && (!root_is_inner || scoping.get_binding(root_scope_id, name) != Some(symbol_id))
            {
              continue;
            }
            names.insert(name);
          }
        }
        names
      })
      .contains(&Ident::from(name))
  }
}

impl std::fmt::Debug for InnerBindingNames<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("InnerBindingNames").finish_non_exhaustive()
  }
}

#[derive(Debug)]
pub struct Renamer<'name> {
  /// Shared conflict-suffix engine; owns the set of taken top-level names.
  resolver: ConflictResolver,
  /// Final symbol → name mappings.
  canonical_names: FxHashMap<SymbolRef, CompactStr>,
  symbol_db: &'name SymbolRefDb,
  /// Entry module index for this chunk, if any.
  entry_module_idx: Option<ModuleIdx>,
  inner_binding_names: InnerBindingNames<'name>,
}

impl<'name> Renamer<'name> {
  pub fn new(
    base_module_index: Option<ModuleIdx>,
    symbol_db: &'name SymbolRefDb,
    format: OutputFormat,
    inner_binding_names: InnerBindingNames<'name>,
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
      inner_binding_names,
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

  /// Returns the top-level name that the finalizer prints for a reference to `symbol_ref`. The
  /// finalizer prints a namespace alias through its namespace binding (`import_foo.default`,
  /// `node_path.join`). Thus a local with the name of that binding can capture the reference.
  /// This function does the same as `finalized_expr_for_symbol_ref`.
  pub fn printed_name(&self, symbol_ref: SymbolRef) -> Option<&CompactStr> {
    self.get_canonical_name(self.symbol_db.canonical_ref_resolving_namespace(symbol_ref))
  }

  pub fn reserve(&mut self, name: CompactStr) {
    self.resolver.reserve(name);
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

  /// Assign a canonical name to a symbol that the finalizer prints at the top level of the chunk.
  /// The name is different from all other top-level names. `kind` decides which inner names the
  /// name must also avoid (see [`RootBindingKind`]).
  pub fn add_symbol_in_root_scope(&mut self, symbol_ref: SymbolRef, kind: RootBindingKind) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);

    // Fuse the dedup check and the final insert into a single `canonical_names` probe via the
    // entry API. An Occupied slot means this canonical_ref was already assigned, so re-adding is
    // a no-op — and we still skip building the owned name on that path (dedup-before-alloc).
    let Entry::Vacant(slot) = self.canonical_names.entry(canonical_ref) else {
      return;
    };

    let original_name = self.symbol_db.original_name(canonical_ref);

    // Bind the fields the `accept` closure reads as locals so the borrow of
    // `self.resolver` (mutable, in `resolve`) does not overlap a borrow of `self`.
    let symbol_db = self.symbol_db;
    let entry_module_idx = self.entry_module_idx;
    let inner_binding_names = &self.inner_binding_names;
    let resolved = self.resolver.resolve(original_name, |candidate, is_original| match kind {
      RootBindingKind::Authored => Self::is_name_available_with(
        symbol_db,
        entry_module_idx,
        candidate,
        canonical_ref,
        is_original,
      ),
      RootBindingKind::Synthesized => !inner_binding_names.contains(candidate),
    });
    slot.insert(resolved);
  }

  /// Returns a name for a synthesized top-level binding that has no symbol, for example a
  /// cross-chunk `require_*` binding or a bridge. Like each synthesized binding, the name avoids
  /// the inner names of the chunk.
  pub fn create_conflictless_name(&mut self, hint: &str) -> CompactStr {
    let inner_binding_names = &self.inner_binding_names;
    self
      .resolver
      .resolve(CompactStr::new(hint), |candidate, _| !inner_binding_names.contains(candidate))
  }

  /// Rename a nested-scope binding, or a root-scope binding of a CJS-wrapped module. After the
  /// rename, the binding no longer captures a reference in its scope that must resolve to an outer
  /// binding.
  pub fn register_nested_scope_symbols(&mut self, symbol_ref: SymbolRef, original_name: &str) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);
    // A binding linked to a symbol elsewhere (an import binding) is printed under that symbol's
    // name, not its own: renaming it would rename the other symbol.
    if canonical_ref != symbol_ref || self.canonical_names.contains_key(&canonical_ref) {
      return;
    }

    // Find unique name: skip candidates that conflict with top-level symbols
    // or with existing bindings in any scope of the same module.
    for count in 1u32.. {
      let name: CompactStr =
        concat_string!(original_name, "$", itoa::Buffer::new().format(count)).into();

      if self.resolver.contains(&name) {
        continue;
      }

      // Also skip if the candidate name is bound anywhere in the same module.
      // Without this check, renaming `child` to `child$1` could collide with an
      // existing `child$1` binding in the same scope (e.g. from Gleam's variable
      // shadowing convention), or capture a root-scope `child$1` that the
      // resolver never saw: a CJS-wrapped module's root bindings are printed
      // inside its `__commonJS` closure and keep their original names.
      if has_binding(self.symbol_db, symbol_ref.owner, &name) {
        self.resolver.reserve(name);
        continue;
      }

      self.resolver.reserve(name.clone());
      self.canonical_names.insert(symbol_ref, name);
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

/// Returns true if any scope of the module, also the root scope, binds `name`.
fn has_binding(symbol_db: &SymbolRefDb, module_idx: ModuleIdx, name: &str) -> bool {
  let Some(db) = &symbol_db[module_idx] else {
    return false;
  };
  db.ast_scopes.scoping().iter_bindings().any(|(_, bindings)| bindings.contains_key(name))
}

/// The context of the passes that rename the nested-scope symbols of one module that would shadow
/// top-level symbols.
///
/// The passes go from each reference up to the root scope of the module. The finalizer prints the
/// root bindings of a CJS-wrapped module inside its CJS closure, and these bindings get no
/// top-level name. Thus the passes rename a root binding of a CJS-wrapped module like a nested
/// binding. The root bindings of other modules already have their top-level names, and the passes
/// do not change them.
///
/// Generated references resolve to synthesized top-level bindings. These bindings never have the
/// name of a nested binding or of a binding in a CJS closure. Thus the passes check only the
/// references that the source contains.
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
      let Some(printed_name) = self.renamer.printed_name(resolved_symbol).cloned() else {
        continue;
      };

      for scope_id in self.scoping.scope_ancestors(current_reference.scope_id()) {
        if let Some(binding) = self.scoping.get_binding(scope_id, printed_name.as_str().into())
          && binding != symbol
        {
          let symbol_ref = (self.module_idx, binding).into();
          self.renamer.register_nested_scope_symbols(symbol_ref, self.scoping.symbol_name(binding));
        }
      }
    }
  }

  /// Rename the nested bindings that would capture a reference to a renamed named import.
  ///
  /// A top-level conflict can rename a named import. If a nested binding has the new name of the
  /// import, the reference resolves to that binding. Thus this pass renames the nested binding.
  ///
  /// # Example (`basic_scoped`)
  ///
  /// ```js
  /// // a.js
  /// export const a = 'a.js';
  ///
  /// // main.js
  /// import { a as aJs } from './a';
  /// const a = 'main.js';       // This binding keeps `a`, so the import becomes `a$1`.
  /// function foo(a$1) {        // This parameter would capture the reference to `aJs`.
  ///   return [a$1, a, aJs];
  /// }
  /// ```
  ///
  /// Output:
  /// ```js
  /// const a$1 = "a.js";        // The import gets the name `a$1`.
  /// const a = "main.js";
  /// function foo(a$1$1) {      // The pass renames the parameter.
  ///   return [a$1$1, a, a$1];  // `aJs` resolves to `a$1`.
  /// }
  /// ```
  ///
  /// The pass checks the name that the finalizer prints. The finalizer prints an import that the
  /// code reads through a namespace binding as `node_path.join` (an external under CJS output). A
  /// local with the name of the namespace can capture it, also in a module that does not import
  /// the external.
  pub fn rename_bindings_shadowing_named_imports(&mut self) {
    for (symbol_ref, _named_import) in &self.module.named_imports {
      if self.db.is_facade_symbol(symbol_ref.symbol) {
        continue;
      }

      // Only check for shadowing if the symbol was processed by the renamer
      // (i.e. it has a canonical name entry and is rendered at the chunk's root scope).
      let Some(printed_name) = self.renamer.printed_name(*symbol_ref).cloned() else {
        continue;
      };

      for reference in self.scoping.get_resolved_references(symbol_ref.symbol) {
        for scope_id in self.scoping.scope_ancestors(reference.scope_id()) {
          if let Some(binding) = self.scoping.get_binding(scope_id, printed_name.as_str().into())
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

  /// Rename nested bindings that would shadow the ambient names of CommonJS output.
  ///
  /// Several rewrites emit bare, renamer-invisible identifiers into the module body, at arbitrary
  /// nesting depth:
  /// - `require(...)` — external imports, dynamic-import lowering, and the `import.meta.url`
  ///   polyfill (`require("url").pathToFileURL(__filename).href`)
  /// - `__filename` — the argument of that polyfill, and the `import.meta.filename` rewrite
  /// - `__dirname` — the `import.meta.dirname` rewrite
  ///
  /// Those identifiers mean the CommonJS ambient bindings, so a nested binding of the same name
  /// must not capture them. `module`/`exports` are deliberately not in the set: nothing injects
  /// them into nested scopes, and `rename_bindings_shadowing_wrapper_params` already covers the
  /// CJS-wrapped-module case.
  ///
  /// A `var` binding is hoisted, so it shadows even an injected call inside its own initializer:
  ///
  /// ```js
  /// // input, the shape emscripten emits with `-s EXPORT_ES6=1 -s ENVIRONMENT='node'`
  /// function init() {
  ///   var require = createRequire(import.meta.url);
  ///   return require("node:path").sep;
  /// }
  /// ```
  ///
  /// Without renaming, the polyfill resolves to the still-undefined local and the module throws
  /// `require is not a function` on first call:
  ///
  /// ```js
  /// var require = createRequire(require("url").pathToFileURL(__filename).href);
  /// ```
  ///
  /// The same capture breaks a nested `var __filename`/`var __dirname` the same way
  /// (`pathToFileURL(__filename)` reads the still-undefined local and throws).
  ///
  /// Only CommonJS output injects these names. The pass does nothing for the other formats.
  pub fn rename_bindings_shadowing_cjs_ambient_names(&mut self, output_format: OutputFormat) {
    if !matches!(output_format, OutputFormat::Cjs) {
      return;
    }

    // Skip root scope (index 0), check nested scopes only. Root-scope bindings are already covered
    // by the renamer's `manual_reserved` list for CommonJS output.
    for (_, bindings) in self.scoping.iter_bindings().skip(1) {
      for (&name, symbol_id) in bindings {
        if matches!(name.as_str(), "require" | "__filename" | "__dirname") {
          let symbol_ref = (self.module_idx, *symbol_id).into();
          self.renamer.register_nested_scope_symbols(symbol_ref, name.as_str());
        }
      }
    }
  }
}
