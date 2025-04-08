use oxc::syntax::keyword::{GLOBAL_OBJECTS, RESERVED_KEYWORDS};
use rolldown_common::{
  GetLocalDb, ModuleIdx, OutputFormat, SymbolNameRefToken, SymbolRef, SymbolRefDb,
  SymbolRefDbForModule,
};
use rolldown_rstr::{Rstr, ToRstr};
use rolldown_utils::concat_string;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::hash_map::Entry;

#[derive(Debug)]
pub struct Renamer<'name> {
  /// key is the original name,
  /// value is the how many same variable name in the top level are used before
  /// It is also used to calculate the candidate_name e.g.
  /// ```js
  /// // index.js
  /// import {a as b} from './a.js'
  /// const a = 1; // {a => 0}
  /// const a$1 = 1000; // {a => 0, a$1 => 0}
  ///
  ///
  /// // a.js
  /// export const a = 100; // {a => 0, a$1 => 0}, first we try looking up `a`, it is used. So we try the
  ///                       // candidate_name `a$1`(conflict_index + 1 = 1). Then we try `a$2`, so
  ///                       // on and so forth. Until we find a name that is not used. In this case, `a$2` is not used
  ///                       // so we rename `a` to `a$2`
  /// ```
  ///
  manual_reserved: FxHashSet<&'static str>,
  used_canonical_names: FxHashMap<Rstr, u32>,
  canonical_names: FxHashMap<SymbolRef, Rstr>,
  canonical_token_to_name: FxHashMap<SymbolNameRefToken, Rstr>,
  symbol_db: &'name SymbolRefDb,
  entry_module: Option<&'name SymbolRefDbForModule>,
  module_used_names: FxHashMap<ModuleIdx, FxHashSet<&'name str>>,
}

impl<'name> Renamer<'name> {
  pub fn new(
    base_module_index: Option<ModuleIdx>,
    symbol_db: &'name SymbolRefDb,
    format: OutputFormat,
  ) -> Self {
    // Port from https://github.com/rollup/rollup/blob/master/src/Chunk.ts#L1377-L1394.
    let mut manual_reserved = match format {
      OutputFormat::Esm | OutputFormat::App => FxHashSet::default(),
      OutputFormat::Cjs => {
        FxHashSet::from_iter(["module", "require", "__filename", "__dirname", "exports"])
      }
      OutputFormat::Iife | OutputFormat::Umd => FxHashSet::from_iter(["exports"]), // Also for  AMD, but we don't support them yet.
    };
    // https://github.com/rollup/rollup/blob/bfbea66569491f5466fbba99de2ba6a0225f851b/src/Chunk.ts#L1359
    manual_reserved.extend(["Object", "Promise"]);

    Self {
      used_canonical_names: manual_reserved
        .iter()
        .chain(RESERVED_KEYWORDS.iter())
        .chain(GLOBAL_OBJECTS.iter())
        .map(|s| (Rstr::new(s), 0))
        .collect(),
      manual_reserved,
      canonical_names: FxHashMap::default(),
      canonical_token_to_name: FxHashMap::default(),
      symbol_db,
      module_used_names: FxHashMap::default(),
      entry_module: base_module_index.map(|index| symbol_db.local_db(index)),
    }
  }

  pub fn reserve(&mut self, name: Rstr) {
    self.used_canonical_names.insert(name, 0);
  }

  #[expect(clippy::map_entry)]
  pub fn add_symbol_in_root_scope(&mut self, symbol_ref: SymbolRef) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);
    let original_name = canonical_ref.name(self.symbol_db).to_rstr();

    if !self.canonical_names.contains_key(&canonical_ref) {
      let name = self.get_unique_name(symbol_ref, original_name);
      self.canonical_names.insert(canonical_ref, name);
    }
  }

  #[expect(clippy::map_entry)]
  pub fn add_symbol_in_root_scope_with_original_name(
    &mut self,
    symbol_ref: SymbolRef,
    original_name: Rstr,
  ) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);

    if !self.canonical_names.contains_key(&canonical_ref) {
      let name = self.get_unique_name(symbol_ref, original_name);
      self.canonical_names.insert(canonical_ref, name);
    }
  }

  /// Get the name without `$` with digits.
  ///
  /// `good` -> `good`
  /// `good$1` -> `good`
  /// `good$1$2` -> `good`
  /// `good$1$2$1` -> `good`
  fn normalize_name(name: Rstr) -> Rstr {
    let bytes = name.as_bytes();
    let exclude_index = bytes.iter().rposition(|&b| b != b'$' && !b.is_ascii_digit());

    if let Some(index) = exclude_index {
      if bytes.get(index + 1).copied().is_some_and(|c| c == b'$') {
        return name.split_at(index + 1).0.to_rstr();
      }
    }

    // If there is no `$` in the name, return the original name.
    name
  }

  fn generate_candidate_name(original_name: &Rstr, count: u32) -> Rstr {
    concat_string!(original_name, "$", itoa::Buffer::new().format(count)).into()
  }

  fn get_module_used_names(
    symbol_db: &'name SymbolRefDb,
    canonical_ref: SymbolRef,
  ) -> FxHashSet<&'name str> {
    const RUNTIME_MODULE_INDEX: ModuleIdx = ModuleIdx::from_usize_unchecked(0);
    if canonical_ref.owner.is_dummy() || canonical_ref.owner == RUNTIME_MODULE_INDEX {
      FxHashSet::default()
    } else {
      let scoping = symbol_db.local_db(canonical_ref.owner).ast_scopes.scoping();
      if scoping.symbols_len() == 0 {
        return FxHashSet::default();
      }
      let root_symbol_ids =
        scoping.get_bindings(scoping.root_scope_id()).values().collect::<FxHashSet<_>>();
      scoping
        .symbol_ids()
        .zip(scoping.symbol_names())
        .filter(|(symbol_id, _)| !root_symbol_ids.contains(symbol_id))
        .map(|(_, name)| name)
        .collect::<FxHashSet<&str>>()
    }
  }

  fn get_unique_name(&mut self, canonical_ref: SymbolRef, original_name: Rstr) -> Rstr {
    let original_name = Self::normalize_name(original_name);
    let (mut candidate_name, count) = match self.used_canonical_names.entry(original_name.clone()) {
      Entry::Occupied(o) => {
        let count = o.into_mut();
        *count += 1;
        (Self::generate_candidate_name(&original_name, *count), count)
      }
      Entry::Vacant(v) => (original_name.clone(), v.insert(0)),
    };

    loop {
      let is_root_binding = self.entry_module.is_some_and(|module| {
        let scoping = module.ast_scopes.scoping();
        scoping.get_root_binding(&candidate_name).is_some_and(|symbol_id| {
          let base_symbol = SymbolRef::from((module.owner_idx, symbol_id));
          base_symbol == canonical_ref || base_symbol.canonical_ref(self.symbol_db) == canonical_ref
        })
      });

      if (is_root_binding && !self.manual_reserved.contains(candidate_name.as_str()))
        || (!self
          .module_used_names
          .entry(canonical_ref.owner)
          .or_insert_with(|| Self::get_module_used_names(self.symbol_db, canonical_ref))
          .contains(candidate_name.as_str()))
      {
        return candidate_name;
      }

      *count += 1;
      candidate_name = Self::generate_candidate_name(&original_name, *count);
    }
  }

  pub fn create_conflictless_name(&mut self, hint: &str) -> String {
    let mut conflictless_name = Rstr::new(hint);
    loop {
      match self.used_canonical_names.entry(conflictless_name.clone()) {
        Entry::Occupied(mut occ) => {
          let next_conflict_index = *occ.get() + 1;
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

  #[allow(dead_code)]
  pub fn add_symbol_name_ref_token(&mut self, token: &SymbolNameRefToken) {
    let hint = token.value();
    let mut conflictless_name = Rstr::new(hint);
    loop {
      match self.used_canonical_names.entry(conflictless_name.clone()) {
        Entry::Occupied(mut occ) => {
          let next_conflict_index = *occ.get() + 1;
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
    self.canonical_token_to_name.insert(token.clone(), conflictless_name);
  }

  pub fn into_canonical_names(
    self,
  ) -> (FxHashMap<SymbolRef, Rstr>, FxHashMap<SymbolNameRefToken, Rstr>) {
    (self.canonical_names, self.canonical_token_to_name)
  }
}
