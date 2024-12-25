use oxc::semantic::ScopeId;
use oxc::syntax::keyword::{GLOBAL_OBJECTS, RESERVED_KEYWORDS};
use rolldown_common::{
  IndexModules, ModuleIdx, NormalModule, OutputFormat, SymbolNameRefToken, SymbolRef, SymbolRefDb,
};
use rolldown_rstr::{Rstr, ToRstr};
use rolldown_utils::{
  concat_string,
  rayon::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator},
};
use rustc_hash::FxHashMap;
use std::borrow::Cow;
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
}

impl<'name> Renamer<'name> {
  pub fn new(symbols: &'name SymbolRefDb, _modules_len: usize, format: OutputFormat) -> Self {
    // Port from https://github.com/rollup/rollup/blob/master/src/Chunk.ts#L1377-L1394.
    let mut manual_reserved = match format {
      OutputFormat::Esm | OutputFormat::App => vec![],
      OutputFormat::Cjs => vec!["module", "require", "__filename", "__dirname", "exports"],
      OutputFormat::Iife | OutputFormat::Umd => vec!["exports"], // Also for  AMD, but we don't support them yet.
    };
    // https://github.com/rollup/rollup/blob/bfbea66569491f5466fbba99de2ba6a0225f851b/src/Chunk.ts#L1359
    manual_reserved.extend(["Object", "Promise"]);
    Self {
      canonical_names: FxHashMap::default(),
      canonical_token_to_name: FxHashMap::default(),
      symbol_db: symbols,
      used_canonical_names: manual_reserved
        .iter()
        .chain(RESERVED_KEYWORDS.iter())
        .chain(GLOBAL_OBJECTS.iter())
        .map(|s| (Rstr::new(s), 0))
        .collect(),
    }
  }

  pub fn reserve(&mut self, name: Rstr) {
    self.used_canonical_names.insert(name, 0);
  }

  pub fn add_symbol_in_root_scope(&mut self, symbol_ref: SymbolRef) {
    let canonical_ref = symbol_ref.canonical_ref(self.symbol_db);
    let original_name = canonical_ref.name(self.symbol_db).to_rstr();
    match self.canonical_names.entry(canonical_ref) {
      Entry::Vacant(vacant) => {
        let mut candidate_name = original_name.clone();
        loop {
          match self.used_canonical_names.entry(candidate_name.clone()) {
            Entry::Occupied(mut occ) => {
              let next_conflict_index = *occ.get() + 1;
              *occ.get_mut() = next_conflict_index;
              candidate_name =
                concat_string!(original_name, "$", itoa::Buffer::new().format(next_conflict_index))
                  .into();
            }
            Entry::Vacant(vac) => {
              vac.insert(0);
              break;
            }
          }
        }
        vacant.insert(candidate_name);
      }
      Entry::Occupied(_) => {
        // The symbol is already renamed
      }
    }
  }

  pub fn create_conflictless_name(&mut self, hint: &str) -> String {
    let hint = Rstr::new(hint);
    let mut conflictless_name = hint.clone();
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
    let hint = Rstr::new(token.value());
    let mut conflictless_name = hint.clone();
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
    self.canonical_token_to_name.insert(token.clone(), conflictless_name.clone());
  }

  // non-top-level symbols won't be linked cross-module. So the canonical `SymbolRef` for them are themselves.
  #[tracing::instrument(level = "trace", skip_all)]
  pub fn rename_non_root_symbol(&mut self, modules_in_chunk: &[ModuleIdx], modules: &IndexModules) {
    #[tracing::instrument(level = "trace", skip_all)]
    fn rename_symbols_of_nested_scopes<'name>(
      module: &'name NormalModule,
      scope_id: ScopeId,
      stack: &mut Vec<Cow<FxHashMap<Rstr, u32>>>,
      canonical_names: &mut FxHashMap<SymbolRef, Rstr>,
    ) {
      let mut bindings = module.scope.get_bindings(scope_id).iter().collect::<Vec<_>>();
      bindings.sort_unstable_by_key(|(_, symbol_id)| *symbol_id);
      let mut used_canonical_names_for_this_scope = FxHashMap::default();
      used_canonical_names_for_this_scope.shrink_to(bindings.len());
      bindings.iter().for_each(|(binding_name, &symbol_id)| {
        let binding_ref: SymbolRef = (module.idx, symbol_id).into();

        let mut count = 1;
        let mut candidate_name = binding_name.to_rstr();
        match canonical_names.entry(binding_ref) {
          Entry::Vacant(slot) => loop {
            let is_shadowed = stack
              .iter()
              .any(|used_canonical_names| used_canonical_names.contains_key(&candidate_name))
              || used_canonical_names_for_this_scope.contains_key(&candidate_name);

            if is_shadowed {
              candidate_name =
                concat_string!(binding_name, "$", itoa::Buffer::new().format(count)).into();
              count += 1;
            } else {
              used_canonical_names_for_this_scope.insert(candidate_name.clone(), 0);
              slot.insert(candidate_name);
              break;
            }
          },
          Entry::Occupied(_) => {
            // The symbol is already renamed
          }
        }
        used_canonical_names_for_this_scope.insert(binding_name.to_rstr(), 0);
      });

      stack.push(Cow::Owned(used_canonical_names_for_this_scope));
      let child_scopes = module.scope.get_child_ids(scope_id);
      child_scopes.iter().for_each(|scope_id| {
        rename_symbols_of_nested_scopes(module, *scope_id, stack, canonical_names);
      });
      stack.pop();
    }

    let copied_scope_iter =
      modules_in_chunk.par_iter().copied().filter_map(|id| modules[id].as_normal()).flat_map(
        |module| {
          let child_scopes: &[ScopeId] = module.scope.get_child_ids(module.scope.root_scope_id());

          child_scopes.into_par_iter().map(|child_scope_id| {
            let mut stack = vec![Cow::Borrowed(&self.used_canonical_names)];
            let mut canonical_names = FxHashMap::default();
            rename_symbols_of_nested_scopes(
              module,
              *child_scope_id,
              &mut stack,
              &mut canonical_names,
            );
            canonical_names
          })
        },
      );

    #[cfg(not(target_family = "wasm"))]
    let canonical_names_of_nested_scopes =
      copied_scope_iter.reduce(FxHashMap::default, |mut acc, canonical_names| {
        acc.extend(canonical_names);
        acc
      });
    #[cfg(target_family = "wasm")]
    let canonical_names_of_nested_scopes = copied_scope_iter
      .reduce(|mut acc, canonical_names| {
        acc.extend(canonical_names);
        acc
      })
      .unwrap_or_default();

    self.canonical_names.extend(canonical_names_of_nested_scopes);
  }

  pub fn into_canonical_names(
    self,
  ) -> (FxHashMap<SymbolRef, Rstr>, FxHashMap<SymbolNameRefToken, Rstr>) {
    (self.canonical_names, self.canonical_token_to_name)
  }
}
