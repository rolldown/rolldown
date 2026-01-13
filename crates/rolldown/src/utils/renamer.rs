use oxc::span::CompactStr;
use oxc::syntax::keyword::{GLOBAL_OBJECTS, RESERVED_KEYWORDS};
use rolldown_common::{
  GetLocalDb, ModuleIdx, OutputFormat, SymbolRef, SymbolRefDb, SymbolRefDbForModule, SymbolRefFlags,
};
use rolldown_utils::concat_string;
use rustc_hash::{FxHashMap, FxHashSet};
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

  /// Cache of non-root symbol names per module.
  /// Used to avoid renaming root symbols to names that would conflict with nested scope symbols.
  module_used_names: FxHashMap<ModuleIdx, FxHashSet<&'name str>>,

  /// Names that have been used during the renaming process.
  /// Tracks which names have been assigned to avoid duplicates.
  used_names: FxHashSet<CompactStr>,
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

    // For entry module, pre-cache all symbol names (both root and non-root)
    // to ensure we don't rename other symbols to names that exist in the entry module
    let module_used_names = base_module_index
      .map(|idx| {
        let entry_db = symbol_db.local_db(idx);
        let scoping = entry_db.ast_scopes.scoping();
        let all_names: FxHashSet<&str> =
          scoping.symbol_ids().map(|id| scoping.symbol_name(id)).collect();
        FxHashMap::from_iter([(idx, all_names)])
      })
      .unwrap_or_default();

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
      module_used_names,
      used_names: FxHashSet::default(),
    }
  }

  pub fn reserve(&mut self, name: CompactStr) {
    self.used_canonical_names.insert(name, CanonicalNameInfo::default());
  }

  /// Check if the candidate name is a root binding in the entry module that matches the symbol.
  /// If so, we can use this name directly without further checks.
  fn is_entry_root_binding(&self, candidate_name: &str, symbol_ref: SymbolRef) -> bool {
    match (self.entry_module_idx, self.entry_module) {
      (Some(entry_idx), Some(module)) => {
        let scoping = module.ast_scopes.scoping();
        scoping.get_root_binding(candidate_name).is_some_and(|symbol_id| {
          let entry_symbol = SymbolRef::from((entry_idx, symbol_id));
          // The candidate is valid if it's the same symbol or links to it
          entry_symbol == symbol_ref || entry_symbol.canonical_ref(self.symbol_db) == symbol_ref
        })
      }
      _ => false,
    }
  }

  /// Check if a candidate name is available for use (doesn't conflict with existing names).
  fn is_name_available(&self, candidate_name: &str, symbol_ref: SymbolRef) -> bool {
    // Check 1: Not already used during this renaming pass
    if self.used_names.contains(candidate_name) {
      return false;
    }

    // Check 2: Not used in the entry module's scope tree (if entry module exists)
    if let Some(entry_idx) = self.entry_module_idx {
      if let Some(entry_names) = self.module_used_names.get(&entry_idx) {
        if entry_names.contains(candidate_name) {
          return false;
        }
      }
    }

    // Check 3: Not used in the symbol's own module's non-root scope
    // (only for symbols from modules other than the entry module)
    if let Some(entry_idx) = self.entry_module_idx {
      if symbol_ref.owner != entry_idx {
        if let Some(module_names) = self.module_used_names.get(&symbol_ref.owner) {
          if module_names.contains(candidate_name) {
            return false;
          }
        }
      }
    } else {
      // No entry module, check against the symbol's own module
      if let Some(module_names) = self.module_used_names.get(&symbol_ref.owner) {
        if module_names.contains(candidate_name) {
          return false;
        }
      }
    }

    true
  }

  /// Get or compute the set of non-root symbol names for a module.
  fn get_or_create_module_used_names(&mut self, module_idx: ModuleIdx) -> &FxHashSet<&'name str> {
    self.module_used_names.entry(module_idx).or_insert_with(|| {
      // For runtime module (index 0), return empty set
      const RUNTIME_MODULE_INDEX: ModuleIdx = ModuleIdx::from_usize_unchecked(0);
      if module_idx == RUNTIME_MODULE_INDEX {
        return FxHashSet::default();
      }

      let db = self.symbol_db.local_db(module_idx);
      let scoping = db.ast_scopes.scoping();
      if scoping.symbols_len() == 0 {
        return FxHashSet::default();
      }

      let root_scope_id = scoping.root_scope_id();
      let root_bindings: FxHashSet<_> =
        scoping.get_bindings(root_scope_id).values().copied().collect();

      // Collect all non-root symbol names
      scoping
        .symbol_ids()
        .filter(|id| !root_bindings.contains(id))
        .map(|id| scoping.symbol_name(id))
        .collect()
    })
  }

  /// Assigns a canonical name to a symbol, checking for conflicts with both
  /// top-level names and nested scope names.
  ///
  /// This method is aware of:
  /// 1. Names already used by other top-level symbols (`used_canonical_names`)
  /// 2. Names used in the entry module's scope tree (`module_used_names`)
  /// 3. Names used in the symbol's own module's nested scopes (`module_used_names`)
  ///
  /// By checking all these during root scope renaming, we can avoid the expensive
  /// `rename_non_root_symbol` pass that iterates through all nested scopes.
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

    // Pre-compute module used names for the symbol's owner module (if not entry module)
    // This ensures we don't rename to a name that would conflict with nested scope symbols
    if self.entry_module_idx.is_none_or(|entry_idx| canonical_ref.owner != entry_idx) {
      // Force computation of module_used_names for this module
      let _ = self.get_or_create_module_used_names(canonical_ref.owner);
    }

    // Check if already renamed
    if self.canonical_names.contains_key(&canonical_ref) {
      return;
    }

    let mut candidate_name = original_name.clone();
    let mut was_renamed = false;
    let mut conflict_index = 0u32;

    loop {
      // Check 1: Is this name already used by another top-level symbol or reserved?
      if let Some(info) = self.used_canonical_names.get(&candidate_name) {
        // If this is a reserved name (owner: None) or belongs to another module, we must rename.
        // But if this is a root binding in the entry module for this symbol AND not reserved,
        // we can use this name directly.
        let is_reserved = info.owner.is_none();
        if !is_reserved && self.is_entry_root_binding(&candidate_name, canonical_ref) {
          // The name is owned by this module's entry binding, use it directly
          self.used_names.insert(candidate_name.clone());
          self.canonical_names.insert(canonical_ref, candidate_name);
          return;
        }
        // Name is reserved or conflicts - generate a new name
        conflict_index = info.conflict_index + 1;
        if let Some(existing) = self.used_canonical_names.get_mut(&candidate_name) {
          existing.conflict_index = conflict_index;
        }
        candidate_name =
          concat_string!(original_name, "$", itoa::Buffer::new().format(conflict_index)).into();
        was_renamed = true;
        continue;
      }

      // Check 2: Is this a root binding in the entry module for this symbol?
      // If so, we can use this name directly (it's not in used_canonical_names yet).
      if self.is_entry_root_binding(&candidate_name, canonical_ref) {
        // Mark as used in used_canonical_names
        self.used_canonical_names.insert(
          candidate_name.clone(),
          CanonicalNameInfo { conflict_index: 0, owner: Some(canonical_ref.owner), was_renamed },
        );
        self.used_names.insert(candidate_name.clone());
        self.canonical_names.insert(canonical_ref, candidate_name);
        return;
      }

      // Check 3: Is this name available (not conflicting with nested scope symbols)?
      if !self.is_name_available(&candidate_name, canonical_ref) {
        conflict_index += 1;
        candidate_name =
          concat_string!(original_name, "$", itoa::Buffer::new().format(conflict_index)).into();
        was_renamed = true;
        continue;
      }

      // Name is available - use it
      self.used_canonical_names.insert(
        candidate_name.clone(),
        CanonicalNameInfo { conflict_index: 0, owner: Some(canonical_ref.owner), was_renamed },
      );
      self.used_names.insert(candidate_name.clone());
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

    if shadows_renamed_symbol || shadows_cjs_param {
      // Generate a unique name
      let mut count = 1u32;
      let mut candidate_name: CompactStr =
        concat_string!(original_name, "$", itoa::Buffer::new().format(count)).into();
      while self.used_canonical_names.contains_key(&candidate_name)
        || self.used_names.contains(&candidate_name)
      {
        count += 1;
        candidate_name =
          concat_string!(original_name, "$", itoa::Buffer::new().format(count)).into();
      }
      self.canonical_names.insert(symbol_ref, candidate_name);
    } else {
      // Use original name
      self.canonical_names.insert(symbol_ref, CompactStr::new(original_name));
    }
  }

  #[inline]
  pub fn into_canonical_names(self) -> FxHashMap<SymbolRef, CompactStr> {
    self.canonical_names
  }
}
