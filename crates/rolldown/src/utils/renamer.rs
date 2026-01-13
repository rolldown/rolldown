use oxc::semantic::ScopeId;
use oxc::span::CompactStr;
use oxc::syntax::keyword::{GLOBAL_OBJECTS, RESERVED_KEYWORDS};
use rolldown_common::{
  AstScopes, ModuleIdx, ModuleScopeSymbolIdMap, NormalModule, OutputFormat, SymbolRef, SymbolRefDb,
  SymbolRefFlags, WrapKind,
};
use rolldown_utils::rustc_hash::FxHashMapExt;
use rolldown_utils::{
  concat_string,
  rayon::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator},
};
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::hash_map::Entry;

use crate::stages::link_stage::LinkStageOutput;

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
}

impl<'name> Renamer<'name> {
  pub fn new(symbols: &'name SymbolRefDb, format: OutputFormat) -> Self {
    // Port from https://github.com/rollup/rollup/blob/master/src/Chunk.ts#L1377-L1394.
    let mut manual_reserved = match format {
      OutputFormat::Esm => vec![],
      OutputFormat::Cjs => vec!["module", "require", "__filename", "__dirname", "exports"],
      OutputFormat::Iife | OutputFormat::Umd => vec!["exports"], // Also for  AMD, but we don't support them yet.
    };
    // https://github.com/rollup/rollup/blob/bfbea66569491f5466fbba99de2ba6a0225f851b/src/Chunk.ts#L1359
    manual_reserved.extend(["Object", "Promise"]);
    Self {
      canonical_names: FxHashMap::default(),
      symbol_db: symbols,
      used_canonical_names: manual_reserved
        .iter()
        .chain(RESERVED_KEYWORDS.iter())
        .chain(GLOBAL_OBJECTS.iter())
        .map(|s| (CompactStr::new(s), CanonicalNameInfo::default()))
        .collect(),
    }
  }

  pub fn reserve(&mut self, name: CompactStr) {
    self.used_canonical_names.insert(name, CanonicalNameInfo::default());
  }

  /// Assigns a canonical name to a symbol without checking for conflicts.
  ///
  /// This is used when a top-level symbol becomes non-top-level after transformations,
  /// such as a Commonjs module may be wrapped with a runtime helper function
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

    match self.canonical_names.entry(canonical_ref) {
      Entry::Vacant(vacant) => {
        let mut candidate_name = original_name.clone();
        let mut was_renamed = false;
        loop {
          match self.used_canonical_names.entry(candidate_name.clone()) {
            Entry::Occupied(mut occ) => {
              let next_conflict_index = occ.get().conflict_index + 1;
              occ.get_mut().conflict_index = next_conflict_index;
              candidate_name =
                concat_string!(original_name, "$", itoa::Buffer::new().format(next_conflict_index))
                  .into();
              was_renamed = true;
            }
            Entry::Vacant(vac) => {
              vac.insert(CanonicalNameInfo {
                conflict_index: 0,
                owner: Some(canonical_ref.owner),
                was_renamed,
              });
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

  // non-top-level symbols won't be linked cross-module. So the canonical `SymbolRef` for them are themselves.
  #[tracing::instrument(level = "trace", skip_all)]
  pub fn rename_non_root_symbol(
    &mut self,
    modules_in_chunk: &[ModuleIdx],
    link_stage_output: &LinkStageOutput,
    map: &ModuleScopeSymbolIdMap<'_>,
  ) {
    #[tracing::instrument(level = "trace", skip_all)]
    fn rename_symbols_of_nested_scopes<'name>(
      module: &'name NormalModule,
      scope_id: ScopeId,
      stack: &mut Vec<Cow<'_, FxHashMap<CompactStr, CanonicalNameInfo>>>,
      canonical_names: &mut FxHashMap<SymbolRef, CompactStr>,
      ast_scope: &'name AstScopes,
      map: &ModuleScopeSymbolIdMap<'_>,
    ) {
      let bindings = map.get(&module.idx).map(|vec| &vec[scope_id]).unwrap();
      let mut used_canonical_names_for_this_scope = FxHashMap::with_capacity(bindings.len());

      bindings.iter().for_each(|&(symbol_id, binding_name)| {
        let binding_ref: SymbolRef = (module.idx, symbol_id).into();

        let mut count = 1;
        let mut candidate_name = Cow::Borrowed(binding_name);
        match canonical_names.entry(binding_ref) {
          Entry::Vacant(slot) => loop {
            let is_shadowed =
              stack.iter().enumerate().any(|(i, used_canonical_names)| match used_canonical_names
                .get(candidate_name.as_ref())
              {
                Some(info) => {
                  // top-level names that this module's nested scopes should avoid
                  // shadowing. The rules are:
                  //
                  // 1. Cross-module symbols: ALWAYS avoid - nested scopes shouldn't accidentally
                  //    capture a symbol from another module.
                  //    See: crates/rolldown/tests/rolldown/topics/deconflict/issue_6586_generated_name_conflict
                  //
                  // 2. Same-module symbols that were RENAMED: avoid - the nested scope variable
                  //    might have the same name as the NEW name, which would be an accidental
                  //    collision (the programmer didn't know the symbol would be renamed).
                  //    See: crates/rolldown/tests/rolldown/topics/deconflict/issue_same_module_shadowing
                  //
                  // 3. Same-module symbols that were NOT renamed: allow shadowing - this preserves
                  //    the original JavaScript scoping semantics where inner scopes can shadow outer.
                  //    See: crates/rolldown/tests/rolldown/topics/deconflict/issue_6586
                  if i == 0 {
                    info.owner.is_some_and(|owner| owner != module.idx || info.was_renamed)
                  } else {
                    true
                  }
                }
                None => false,
              }) || used_canonical_names_for_this_scope.contains_key(candidate_name.as_ref());

            if is_shadowed {
              candidate_name =
                Cow::Owned(concat_string!(&binding_name, "$", itoa::Buffer::new().format(count)));
              count += 1;
            } else {
              let name = CompactStr::new(candidate_name.as_ref());
              used_canonical_names_for_this_scope
                .insert(name.clone(), CanonicalNameInfo::default());
              slot.insert(name);
              break;
            }
          },
          Entry::Occupied(_) => {
            // The symbol is already renamed
          }
        }
      });

      stack.push(Cow::Owned(used_canonical_names_for_this_scope));
      let child_scopes = ast_scope.scoping().get_scope_child_ids(scope_id);
      child_scopes.iter().for_each(|scope_id| {
        rename_symbols_of_nested_scopes(module, *scope_id, stack, canonical_names, ast_scope, map);
      });
      stack.pop();
    }

    // CJS wrapper parameters that nested scopes should avoid shadowing.
    // These are synthetic names injected by the __commonJS wrapper.
    // see crates\rolldown\tests\rolldown\topics\hmr\runtime_correctness as an example
    let cjs_wrapper_names: FxHashMap<CompactStr, CanonicalNameInfo> = ["exports", "module"]
      .into_iter()
      .map(|s| (CompactStr::new(s), CanonicalNameInfo::default()))
      .collect();

    let modules = &link_stage_output.module_table.modules;
    let used_canonical_names = &self.used_canonical_names;
    let cjs_wrapper_names_ref = &cjs_wrapper_names;
    let copied_scope_iter =
      modules_in_chunk.par_iter().copied().filter_map(|id| modules[id].as_normal()).flat_map(
        |module| {
          let ast_scope = &link_stage_output.symbol_db[module.idx].as_ref().unwrap().ast_scopes;
          let child_scopes: &[ScopeId] =
            ast_scope.scoping().get_scope_child_ids(ast_scope.scoping().root_scope_id());

          // Check if this module is CJS wrapped
          let is_cjs_wrapped =
            matches!(link_stage_output.metas[module.idx].wrap_kind(), WrapKind::Cjs);

          child_scopes.into_par_iter().map(move |child_scope_id| {
            // Include names_to_avoid in the initial stack.
            // This ensures nested scope symbols are renamed when they would incorrectly
            // shadow a cross-module symbol or a same-module symbol that was renamed.
            //
            // For CJS wrapped modules, also include `exports` and `module` since these
            // are synthetic parameters injected by the __commonJS wrapper that nested
            // scopes should not shadow.
            let mut stack = vec![Cow::Borrowed(used_canonical_names)];
            if is_cjs_wrapped {
              stack.push(Cow::Borrowed(cjs_wrapper_names_ref));
            }
            let mut canonical_names = FxHashMap::default();
            rename_symbols_of_nested_scopes(
              module,
              *child_scope_id,
              &mut stack,
              &mut canonical_names,
              ast_scope,
              map,
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

  #[inline]
  pub fn into_canonical_names(self) -> FxHashMap<SymbolRef, CompactStr> {
    self.canonical_names
  }
}
