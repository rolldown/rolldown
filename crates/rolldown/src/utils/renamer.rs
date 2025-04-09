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
  used_canonical_names: FxHashMap<Rstr, u32>,
  canonical_names: FxHashMap<SymbolRef, Rstr>,
  canonical_token_to_name: FxHashMap<SymbolNameRefToken, Rstr>,
  symbol_db: &'name SymbolRefDb,
  entry_module: Option<&'name SymbolRefDbForModule>,
  // Store every module's used names
  module_used_names: FxHashMap<ModuleIdx, FxHashSet<&'name str>>,
  /// Store names that are used in renaming process
  used_names: FxHashSet<Rstr>,
}

impl<'name> Renamer<'name> {
  pub fn new(
    base_module_index: Option<ModuleIdx>,
    symbol_db: &'name SymbolRefDb,
    format: OutputFormat,
  ) -> Self {
    // Port from https://github.com/rollup/rollup/blob/master/src/Chunk.ts#L1377-L1394.
    let mut manual_reserved = match format {
      OutputFormat::Esm | OutputFormat::App => vec![],
      OutputFormat::Cjs => vec!["module", "require", "__filename", "__dirname", "exports"],
      OutputFormat::Iife | OutputFormat::Umd => vec!["exports"], // Also for  AMD, but we don't support them yet.
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
      canonical_names: FxHashMap::default(),
      canonical_token_to_name: FxHashMap::default(),
      symbol_db,
      module_used_names: base_module_index
        .map(|index| {
          // Special for entry module, the whole symbol names are stored; other modules only store non-root symbol names.
          FxHashMap::from_iter([(
            index,
            symbol_db.local_db(index).ast_scopes.scoping().symbol_names().collect::<FxHashSet<_>>(),
          )])
        })
        .unwrap_or_default(),
      entry_module: base_module_index.map(|index| symbol_db.local_db(index)),
      used_names: FxHashSet::default(),
    }
  }

  pub fn reserve(&mut self, name: Rstr) {
    self.used_canonical_names.insert(name, 0);
  }

  pub fn add_symbol_in_root_scope(&mut self, symbol_ref: SymbolRef) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);
    let original_name = canonical_ref.name(self.symbol_db);

    self.canonical_names.entry(canonical_ref).or_insert_with(|| {
      let (mut candidate_name, count) =
        match self.used_canonical_names.entry(original_name.to_rstr()) {
          Entry::Occupied(o) => {
            let count = o.into_mut();
            *count += 1;
            (Self::generate_candidate_name(original_name, *count), count)
          }
          Entry::Vacant(v) => (original_name.to_rstr(), v.insert(0)),
        };

      loop {
        let is_root_binding = self.entry_module.is_some_and(|module| {
          let scoping = module.ast_scopes.scoping();
          scoping.get_root_binding(&candidate_name).is_some_and(|symbol_id| {
            let base_symbol = SymbolRef::from((module.owner_idx, symbol_id));
            base_symbol == symbol_ref || base_symbol.canonical_ref(self.symbol_db) == symbol_ref
          })
        });

        if is_root_binding {
          return candidate_name;
        }

        if !self.used_names.contains(&candidate_name)
            // Cannot rename to a name that is already used in the entry module
            && !self.entry_module.is_some_and(|entry|
                  self.module_used_names.get(&entry.owner_idx).is_some_and(|used_names| {
                    used_names.contains(candidate_name.as_str())}))
            // Cannot rename to a name that is already used in symbol itself module
            && !self
              .module_used_names
              .entry(symbol_ref.owner)
              .or_insert_with(|| Self::get_module_used_names(self.symbol_db, symbol_ref))
              .contains(candidate_name.as_str())
        {
          self.used_names.insert(candidate_name.clone());
          return candidate_name;
        }

        *count += 1;
        candidate_name = Self::generate_candidate_name(original_name, *count);
      }
    });
  }

  fn generate_candidate_name(original_name: &str, count: u32) -> Rstr {
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
