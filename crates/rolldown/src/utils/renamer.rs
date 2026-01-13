use oxc::span::CompactStr;
use oxc::syntax::keyword::{GLOBAL_OBJECTS, RESERVED_KEYWORDS};
use rolldown_common::{
  GetLocalDb, ModuleIdx, OutputFormat, SymbolRef, SymbolRefDb, SymbolRefDbForModule, SymbolRefFlags,
};
use rolldown_utils::concat_string;
use rustc_hash::FxHashMap;
use std::collections::hash_map::Entry;

/// Information about a canonical name used in the top-level scope.
#[derive(Debug, Clone, Default)]
struct CanonicalNameInfo {
  /// The conflict index used for generating unique names (e.g., `a$1`, `a$2`).
  conflict_index: u32,
  /// The module that owns this top-level symbol, if any.
  /// `None` for reserved names (keywords, global objects, or manually reserved).
  owner: Option<ModuleIdx>,
  /// Whether this symbol was renamed during deconflicting.
  /// Used to determine if nested scopes should avoid shadowing this name.
  was_renamed: bool,
}

#[derive(Debug)]
pub struct Renamer<'name> {
  /// Tracks all canonical names used in the top-level scope.
  ///
  /// Key is the canonical name, value contains:
  /// - `conflict_index`: How many same-named variables exist, used to generate unique names
  /// - `owner`: The module that owns this symbol (None for reserved names)
  /// - `was_renamed`: Whether the symbol was renamed during deconflicting
  ///
  /// Example:
  /// ```js
  /// // index.js
  /// import {a as b} from './a.js'
  /// const a = 1; // {a => {conflict_index: 0, ...}}
  /// const a$1 = 1000; // {a => {conflict_index: 0, ...}, a$1 => {conflict_index: 0, ...}}
  ///
  /// // a.js
  /// export const a = 100; // First try `a` (used), then `a$1` (used), then `a$2` (available)
  ///                       // Result: rename `a` to `a$2`
  /// ```
  used_canonical_names: FxHashMap<CompactStr, CanonicalNameInfo>,
  canonical_names: FxHashMap<SymbolRef, CompactStr>,
  symbol_db: &'name SymbolRefDb,

  /// The entry module index, if this chunk has an entry point.
  entry_module_idx: Option<ModuleIdx>,

  /// Reference to the entry module's symbol database.
  /// Used to preserve entry module's root binding names and check for name conflicts.
  entry_module: Option<&'name SymbolRefDbForModule>,
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
      OutputFormat::Iife | OutputFormat::Umd => vec!["exports"], // Also for  AMD, but we don't support them yet.
    };
    // https://github.com/rollup/rollup/blob/bfbea66569491f5466fbba99de2ba6a0225f851b/src/Chunk.ts#L1359
    manual_reserved.extend(["Object", "Promise"]);

    // Get entry module reference if provided
    let entry_module = base_module_index.map(|idx| symbol_db.local_db(idx));

    Self {
      canonical_names: FxHashMap::default(),
      symbol_db,
      used_canonical_names: manual_reserved
        .iter()
        .chain(RESERVED_KEYWORDS.iter())
        .chain(GLOBAL_OBJECTS.iter())
        .map(|s| (CompactStr::new(s), CanonicalNameInfo::default()))
        .collect(),
      entry_module_idx: base_module_index,
      entry_module,
    }
  }

  pub fn reserve(&mut self, name: CompactStr) {
    self.used_canonical_names.insert(name, CanonicalNameInfo::default());
  }

  /// Check if the candidate name is a root binding in the entry module that matches the symbol.
  /// If so, we can use this name directly without further checks.
  ///
  /// This only checks for DIRECT matches (the symbol IS the entry module's binding).
  /// It does NOT check canonical matches, so linked symbols from other modules
  /// will use first-come-first-served naming via the normal conflict resolution.
  fn is_entry_root_binding(&self, candidate_name: &str, symbol_ref: SymbolRef) -> bool {
    match (self.entry_module_idx, self.entry_module) {
      (Some(entry_idx), Some(module)) => {
        let scoping = module.ast_scopes.scoping();
        scoping.get_root_binding(candidate_name).is_some_and(|symbol_id| {
          let entry_symbol = SymbolRef::from((entry_idx, symbol_id));
          // Only match if this is exactly the entry module's binding
          entry_symbol == symbol_ref
        })
      }
      _ => false,
    }
  }

  /// Check if a name exists as a non-root binding in a module.
  /// Checks directly against scoping without caching.
  fn has_nested_scope_binding(&self, module_idx: ModuleIdx, name: &str) -> bool {
    // Runtime module (index 0) has no nested scope bindings
    const RUNTIME_MODULE_INDEX: ModuleIdx = ModuleIdx::from_usize_unchecked(0);
    if module_idx == RUNTIME_MODULE_INDEX {
      return false;
    }

    let db = self.symbol_db.local_db(module_idx);
    let scoping = db.ast_scopes.scoping();
    if scoping.symbols_len() == 0 {
      return false;
    }

    // Check if name exists as a non-root binding by iterating symbols
    // starting from the second scope (skip root)
    scoping.iter_bindings().skip(1).any(|bindings| bindings.1.contains_key(name))
  }

  /// Check if a candidate name is available for use (doesn't conflict with existing names).
  /// The `is_original_name` flag indicates if this is the symbol's original name (not a renamed candidate).
  fn is_name_available(
    &self,
    candidate_name: &str,
    symbol_ref: SymbolRef,
    is_original_name: bool,
  ) -> bool {
    // Check 1: For non-entry-module symbols, ensure they don't use names that exist
    // in entry module's nested scopes (which would cause accidental capture).
    // Entry module symbols can use any name - their own nested scope shadowing is intentional.
    if let Some(entry_idx) = self.entry_module_idx {
      if symbol_ref.owner != entry_idx && self.has_nested_scope_binding(entry_idx, candidate_name) {
        return false;
      }
    }

    // Check 2: For RENAMED candidates (not original names), check against own module's
    // nested scopes. This prevents renaming `a` to `a$1` when there's already a parameter
    // named `a$1`. For original names, shadowing is intentional and expected.
    if !is_original_name && self.has_nested_scope_binding(symbol_ref.owner, candidate_name) {
      return false;
    }

    true
  }

  /// Assigns a canonical name to a symbol, checking for conflicts with both
  /// top-level names and nested scope names.
  ///
  /// This method checks:
  /// 1. Names already used by other top-level symbols (`used_canonical_names`)
  /// 2. Names used in the entry module's nested scopes (to prevent capture)
  /// 3. Names used in the symbol's own module's nested scopes (for renamed candidates)
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

    // Check if already renamed
    if self.canonical_names.contains_key(&canonical_ref) {
      return;
    }

    // Fast path: try original name first without cloning
    if !self.used_canonical_names.contains_key(&original_name) {
      // Check if it's an entry root binding or available
      if self.is_entry_root_binding(&original_name, canonical_ref)
        || self.is_name_available(&original_name, canonical_ref, true)
      {
        self.used_canonical_names.insert(
          original_name.clone(),
          CanonicalNameInfo {
            conflict_index: 0,
            owner: Some(canonical_ref.owner),
            was_renamed: false,
          },
        );
        self.canonical_names.insert(canonical_ref, original_name);
        return;
      }
    }

    // Slow path: need to find an alternative name
    let mut conflict_index = self
      .used_canonical_names
      .get_mut(&original_name)
      .map(|info| {
        info.conflict_index += 1;
        info.conflict_index
      })
      .unwrap_or(1);

    loop {
      let candidate_name: CompactStr =
        concat_string!(original_name, "$", itoa::Buffer::new().format(conflict_index)).into();

      // Check if this renamed candidate is available
      if let Some(info) = self.used_canonical_names.get_mut(&candidate_name) {
        conflict_index = info.conflict_index + 1;
        info.conflict_index = conflict_index;
        continue;
      }

      // For renamed candidates, check nested scope conflicts
      if !self.is_name_available(&candidate_name, canonical_ref, false) {
        conflict_index += 1;
        continue;
      }

      // Name is available - use it
      self.used_canonical_names.insert(
        candidate_name.clone(),
        CanonicalNameInfo {
          conflict_index: 0,
          owner: Some(canonical_ref.owner),
          was_renamed: true,
        },
      );
      self.canonical_names.insert(canonical_ref, candidate_name);
      break;
    }
  }

  pub fn create_conflictless_name(&mut self, hint: &str) -> String {
    let mut conflictless_name = CompactStr::new(hint);
    loop {
      match self.used_canonical_names.entry(conflictless_name.clone()) {
        Entry::Occupied(mut occ) => {
          let next_conflict_index = occ.get().conflict_index + 1;
          occ.get_mut().conflict_index = next_conflict_index;
          conflictless_name =
            concat_string!(hint, "$", itoa::Buffer::new().format(next_conflict_index)).into();
        }
        Entry::Vacant(vac) => {
          vac.insert(CanonicalNameInfo::default());
          break;
        }
      }
    }
    conflictless_name.to_string()
  }

  /// CJS wrapper parameter names that nested scopes should avoid shadowing.
  const CJS_WRAPPER_NAMES: [&'static str; 2] = ["exports", "module"];

  /// Register nested scope symbols with their original names.
  ///
  /// Since `add_symbol_in_root_scope` now avoids names that would conflict with nested scope
  /// symbols, most nested scope symbols can keep their original names. However, we still need
  /// to handle the case where nested scope symbols shadow top-level symbols that were renamed,
  /// or CJS wrapper parameters.
  pub fn register_nested_scope_symbols(
    &mut self,
    symbol_ref: SymbolRef,
    original_name: &str,
    is_cjs_wrapped: bool,
  ) {
    // Skip if already registered (e.g., as a top-level symbol)
    if self.canonical_names.contains_key(&symbol_ref) {
      return;
    }

    // Check if this name would shadow a top-level symbol that was renamed
    // (from a different module or renamed in the same module)
    let shadows_renamed_symbol = self.used_canonical_names.get(original_name).is_some_and(|info| {
      info.owner.is_some_and(|owner| owner != symbol_ref.owner || info.was_renamed)
    });

    // Check if this name would shadow CJS wrapper parameters
    let shadows_cjs_param = is_cjs_wrapped && Self::CJS_WRAPPER_NAMES.contains(&original_name);

    // Only store in canonical_names if we need to rename.
    // Symbols keeping their original name can be looked up via symbol_db.name()
    if shadows_renamed_symbol || shadows_cjs_param {
      // Generate a unique name
      let mut count = 1u32;
      let mut candidate_name: CompactStr =
        concat_string!(original_name, "$", itoa::Buffer::new().format(count)).into();
      while self.used_canonical_names.contains_key(&candidate_name) {
        count += 1;
        candidate_name =
          concat_string!(original_name, "$", itoa::Buffer::new().format(count)).into();
      }
      self.canonical_names.insert(symbol_ref, candidate_name);
    }
    // else: symbol keeps original name, no need to store
  }

  #[inline]
  pub fn into_canonical_names(self) -> FxHashMap<SymbolRef, CompactStr> {
    self.canonical_names
  }
}
