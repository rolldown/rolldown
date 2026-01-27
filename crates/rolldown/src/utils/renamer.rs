use rustc_hash::{FxHashMap, FxHashSet};

use std::collections::hash_map::Entry;

use oxc::semantic::Scoping;
use oxc::span::CompactStr;
use oxc::syntax::keyword::{GLOBAL_OBJECTS, RESERVED_KEYWORDS};

use rolldown_common::{
  ModuleIdx, NormalModule, OutputFormat, SymbolRef, SymbolRefDb, SymbolRefDbForModule,
  SymbolRefFlags, WrapKind,
};
use rolldown_utils::concat_string;

use crate::stages::link_stage::LinkStageOutput;

#[derive(Debug)]
pub struct Renamer<'name> {
  /// Tracks all canonical names used in the top-level scope.
  ///
  /// Key is the canonical name, value is the conflict index for generating unique names.
  /// When we need a unique name like `foo$1`, we increment the conflict index stored here.
  ///
  /// Example:
  /// ```js
  /// // index.js
  /// import {a as b} from './a.js'
  /// const a = 1;      // used_canonical_names: {a: 0}
  /// const a$1 = 1000; // used_canonical_names: {a: 0, a$1: 0}
  ///
  /// // a.js
  /// export const a = 100; // Try `a` (used), `a$1` (used), `a$2` (available) → rename to `a$2`
  /// ```
  used_canonical_names: FxHashMap<CompactStr, u32>,
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
      used_canonical_names: manual_reserved
        .iter()
        .chain(RESERVED_KEYWORDS.iter())
        .chain(GLOBAL_OBJECTS.iter())
        .map(|s| (CompactStr::new(s), 0))
        .collect(),
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
    self.used_canonical_names.entry(name).or_insert(0);
  }

  /// Returns true if `name` exists in any nested (non-root) scope of the module.
  /// Returns false for modules without AST (external modules).
  fn has_nested_scope_binding(&self, module_idx: ModuleIdx, name: &str) -> bool {
    let Some(db) = &self.symbol_db[module_idx] else {
      return false;
    };
    // Skip root scope (index 0), check nested scopes only
    db.ast_scopes.scoping().iter_bindings().skip(1).any(|(_, bindings)| bindings.contains_key(name))
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
  fn is_name_available(
    &self,
    candidate_name: &str,
    symbol_ref: SymbolRef,
    is_original_name: bool,
  ) -> bool {
    if let Some(entry_idx) = self.entry_module_idx {
      if symbol_ref.owner == entry_idx {
        // Entry module symbols can use their original names freely - shadowing is
        // handled by reference-based renaming of nested bindings later
        return true;
      }
    }

    // Renamed candidates must not conflict with own module's nested bindings
    // (original names are allowed to shadow - that's intentional)
    if !is_original_name && self.has_nested_scope_binding(symbol_ref.owner, candidate_name) {
      return false;
    }

    true
  }

  /// Assign a canonical name to a top-level symbol, avoiding conflicts with
  /// other top-level names and nested scope names that could cause capture.
  pub fn add_symbol_in_root_scope(&mut self, symbol_ref: SymbolRef, needs_deconflict: bool) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);
    let canonical_name = canonical_ref.name(self.symbol_db);

    let original_name = if self.symbol_db.is_jsx_preserve
      && canonical_ref
        .flags(self.symbol_db)
        .is_some_and(|flags| flags.contains(SymbolRefFlags::MustStartWithCapitalLetterForJSX))
      && canonical_name.as_bytes()[0].is_ascii_lowercase()
    {
      let mut s = String::with_capacity(canonical_name.len());
      s.push(canonical_name.as_bytes()[0].to_ascii_uppercase() as char);
      s.push_str(&canonical_name[1..]);
      CompactStr::from(s)
    } else {
      CompactStr::new(canonical_name)
    };

    if !needs_deconflict {
      self.canonical_names.insert(canonical_ref, original_name);
      return;
    }

    if self.canonical_names.contains_key(&canonical_ref) {
      return;
    }

    // Fast path: original name is available
    if !self.used_canonical_names.contains_key(&original_name)
      && self.is_name_available(&original_name, canonical_ref, true)
    {
      self.used_canonical_names.insert(original_name.clone(), 0);
      self.canonical_names.insert(canonical_ref, original_name);
      return;
    }

    // Slow path: find alternative name (original$1, original$2, ...)
    let mut conflict_index = self
      .used_canonical_names
      .get_mut(&original_name)
      .map(|idx| {
        *idx += 1;
        *idx
      })
      .unwrap_or(1);

    loop {
      let candidate_name: CompactStr =
        concat_string!(original_name, "$", itoa::Buffer::new().format(conflict_index)).into();

      if let Some(idx) = self.used_canonical_names.get_mut(&candidate_name) {
        *idx += 1;
        conflict_index = *idx;
        continue;
      }

      if !self.is_name_available(&candidate_name, canonical_ref, false) {
        conflict_index += 1;
        continue;
      }

      self.used_canonical_names.insert(candidate_name.clone(), 0);
      self.canonical_names.insert(canonical_ref, candidate_name);
      break;
    }
  }

  pub fn create_conflictless_name(&mut self, hint: &str) -> String {
    let mut conflictless_name = CompactStr::new(hint);
    loop {
      match self.used_canonical_names.entry(conflictless_name.clone()) {
        Entry::Occupied(mut occ) => {
          let next_conflict_index = occ.get() + 1;
          *occ.get_mut() = next_conflict_index;
          conflictless_name =
            concat_string!(hint, "$", itoa::Buffer::new().format(next_conflict_index)).into();
        }
        Entry::Vacant(vac) => {
          vac.insert(0);
          break;
        }
      }
    }
    conflictless_name.to_string()
  }

  pub fn register_nested_scope_symbols(&mut self, symbol_ref: SymbolRef, original_name: &str) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);
    if self.canonical_names.contains_key(&canonical_ref) {
      return;
    }

    // Find unique name: skip candidates that conflict with top-level symbols
    for count in 1u32.. {
      let name: CompactStr =
        concat_string!(original_name, "$", itoa::Buffer::new().format(count)).into();

      if let Entry::Vacant(entry) = self.used_canonical_names.entry(name) {
        let candidate_name = entry.key().clone();
        entry.insert(0);
        self.canonical_names.insert(symbol_ref, candidate_name);
        return;
      }
    }
  }

  #[inline]
  pub fn into_canonical_names(self) -> FxHashMap<SymbolRef, CompactStr> {
    self.canonical_names
  }
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
        if let Some(binding) = self.scoping.get_binding(scope_id, &canonical_name)
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
          if let Some(binding) = self.scoping.get_binding(scope_id, &canonical_name)
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
        if wrapper_param_names.contains(name) {
          let symbol_ref = (self.module_idx, *symbol_id).into();
          self.renamer.register_nested_scope_symbols(symbol_ref, name);
        }
      }
    }
  }
}
