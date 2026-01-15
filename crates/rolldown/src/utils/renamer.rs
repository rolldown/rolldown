use oxc::span::CompactStr;
use oxc::syntax::keyword::{GLOBAL_OBJECTS, RESERVED_KEYWORDS};
use rolldown_common::{
  GetLocalDb, ModuleIdx, OutputFormat, SymbolRef, SymbolRefDb, SymbolRefFlags,
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
  /// - `conflict_index`: Counter for generating unique names (`a$1`, `a$2`, ...)
  /// - `owner`: Module that owns this symbol (`None` for reserved names)
  /// - `was_renamed`: Whether this symbol was renamed during deconflicting
  ///
  /// Example:
  /// ```js
  /// // index.js
  /// import {a as b} from './a.js'
  /// const a = 1;      // used_canonical_names: {a: {conflict_index: 0, ...}}
  /// const a$1 = 1000; // used_canonical_names: {a: ..., a$1: {conflict_index: 0, ...}}
  ///
  /// // a.js
  /// export const a = 100; // Try `a` (used), `a$1` (used), `a$2` (available) → rename to `a$2`
  /// ```
  used_canonical_names: FxHashMap<CompactStr, CanonicalNameInfo>,
  /// Final symbol → name mappings. Only stores renamed symbols; originals use symbol_db.
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
        .map(|s| (CompactStr::new(s), CanonicalNameInfo::default()))
        .collect(),
      entry_module_idx: base_module_index,
    }
  }

  /// Returns the canonical name for a symbol if it exists in the renamer.
  /// Returns `None` if the symbol was not processed or kept its original name
  /// (original names are looked up via `symbol_db` during code generation).
  pub fn get_canonical_name(&self, symbol_ref: &SymbolRef) -> Option<&CompactStr> {
    let canonical_ref = self.symbol_db.canonical_ref_for(*symbol_ref);
    self.canonical_names.get(&canonical_ref)
  }

  pub fn reserve(&mut self, name: CompactStr) {
    self.used_canonical_names.insert(name, CanonicalNameInfo::default());
  }

  /// Returns true if `name` exists in any nested (non-root) scope of the module.
  fn has_nested_scope_binding(&self, module_idx: ModuleIdx, name: &str) -> bool {
    let scoping = self.symbol_db.local_db(module_idx).ast_scopes.scoping();
    if scoping.symbols_len() == 0 {
      return false;
    }

    // Skip root scope (index 0), check nested scopes only
    scoping.iter_bindings().skip(1).any(|(_, bindings)| bindings.contains_key(name))
  }

  /// Check if a name is available for a symbol (no nested scope conflicts).
  fn is_name_available(
    &self,
    candidate_name: &str,
    symbol_ref: SymbolRef,
    is_original_name: bool,
  ) -> bool {
    // Non-entry symbols must not capture entry module's nested bindings
    if let Some(entry_idx) = self.entry_module_idx {
      if symbol_ref.owner != entry_idx && self.has_nested_scope_binding(entry_idx, candidate_name) {
        return false;
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

    // Slow path: find alternative name (original$1, original$2, ...)
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

      if let Some(info) = self.used_canonical_names.get_mut(&candidate_name) {
        conflict_index = info.conflict_index + 1;
        info.conflict_index = conflict_index;
        continue;
      }

      if !self.is_name_available(&candidate_name, canonical_ref, false) {
        conflict_index += 1;
        continue;
      }

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

  /// Rename nested scope symbols when they would conflict with top-level symbols.
  ///
  /// Most nested symbols keep their original names. Renaming is required when:
  /// 1. Symbol shadows a top-level name from a *different* module (would cause capture)
  /// 2. Symbol shadows a top-level name that was *renamed* (should shadow original)
  /// 3. Symbol is `exports`/`module` in a CJS wrapped module (would shadow wrapper params)
  ///
  /// ## Example: Avoiding duplicate declarations
  ///
  /// ```js
  /// // other.js
  /// export const a = 'from-other';
  ///
  /// // nested.js - has both `a` and `a$1` as parameters
  /// export function test(a, a$1) { return [a, a$1]; }
  ///
  /// // main.js
  /// import { a } from './other.js';
  /// import { test } from './nested.js';
  /// ```
  ///
  /// Bundled output:
  /// ```js
  /// const a = "from-other";
  /// function test(a$2, a$1) { return [a$2, a$1]; }  // `a` → `a$2`, skipping `a$1`
  /// ```
  ///
  /// The nested `a` is renamed to `a$2` (not `a$1`) because `a$1` already exists
  /// in the same scope - using it would create duplicate declarations.
  pub fn register_nested_scope_symbols(
    &mut self,
    symbol_ref: SymbolRef,
    original_name: &str,
    is_cjs_wrapped: bool,
  ) {
    // Check if renaming is needed:
    // - Shadows top-level from different module, OR shadows a renamed symbol
    let shadows_renamed_symbol = self.used_canonical_names.get(original_name).is_some_and(|info| {
      info.owner.is_some_and(|owner| owner != symbol_ref.owner || info.was_renamed)
    });
    // - Would shadow CJS wrapper params (`exports`, `module`)
    let shadows_cjs_param = is_cjs_wrapped && Self::CJS_WRAPPER_NAMES.contains(&original_name);

    if shadows_renamed_symbol || shadows_cjs_param {
      let scoping = self.symbol_db.local_db(symbol_ref.owner).ast_scopes.scoping();

      // Find unique name: skip candidates that conflict with top-level or module symbols
      let mut count = 1u32;
      let candidate_name = loop {
        let name: CompactStr =
          concat_string!(original_name, "$", itoa::Buffer::new().format(count)).into();

        // Check if conflicts with top-level or renamed symbols
        let conflicts_with_top_level = self.used_canonical_names.contains_key(&name);

        if conflicts_with_top_level {
          count += 1;
          continue;
        }

        // Check if conflicts with own module's nested bindings
        let conflicts_with_module_symbol = scoping.symbol_names().any(|n| n == name.as_str());

        if conflicts_with_module_symbol {
          count += 1;
          continue;
        }

        break name;
      };

      self.canonical_names.insert(symbol_ref, candidate_name);
    }
    // else: keeps original name, looked up via symbol_db during code generation
  }

  #[inline]
  pub fn into_canonical_names(self) -> FxHashMap<SymbolRef, CompactStr> {
    self.canonical_names
  }
}
