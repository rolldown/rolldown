use rayon::iter::ParallelIterator;
use rolldown_common::{Module, RuntimeHelper};
use rolldown_utils::{index_vec_ext::IndexVecRefExt, indexmap::FxIndexSet};

use super::LinkStage;

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn patch_module_dependencies(&mut self) {
    let processed_module_results = self
      .metas
      .par_iter_enumerated()
      .map(|(module_idx, meta)| {
        let mut extended_dependencies = FxIndexSet::default();
        if !meta.depended_runtime_helper.is_empty() {
          extended_dependencies.insert(self.runtime.id());
        }
        // Symbols from runtime are referenced by bundler not import statements.
        meta.referenced_symbols_by_entry_point_chunk.iter().for_each(
          |(symbol_ref, _came_from_cjs)| {
            let canonical_ref = self.symbols.canonical_ref_for(*symbol_ref);
            extended_dependencies.insert(canonical_ref.owner);
          },
        );

        let Module::Normal(module) = &self.module_table[module_idx] else {
          return (module_idx, extended_dependencies, RuntimeHelper::default());
        };

        module.stmt_infos.iter().filter(|stmt_info| stmt_info.is_included).for_each(|stmt_info| {
          // We need this step to include the runtime module, if there are symbols of it.
          // TODO: Maybe we should push runtime module to `LinkingMetadata::dependencies` while pushing the runtime symbols.
          stmt_info.referenced_symbols.iter().for_each(|reference_ref| {
            match reference_ref {
              rolldown_common::SymbolOrMemberExprRef::Symbol(sym_ref) => {
                let canonical_ref = self.symbols.canonical_ref_for(*sym_ref);
                extended_dependencies.insert(canonical_ref.owner);
                let symbol = self.symbols.get(canonical_ref);
                if let Some(ns) = &symbol.namespace_alias {
                  extended_dependencies.insert(ns.namespace_ref.owner);
                }
              }
              rolldown_common::SymbolOrMemberExprRef::MemberExpr(member_expr) => {
                match member_expr.represent_symbol_ref(&meta.resolved_member_expr_refs) {
                  Some(sym_ref) => {
                    let canonical_ref = self.symbols.canonical_ref_for(sym_ref);
                    extended_dependencies.insert(canonical_ref.owner);
                    let symbol = self.symbols.get(canonical_ref);
                    if let Some(ns) = &symbol.namespace_alias {
                      extended_dependencies.insert(ns.namespace_ref.owner);
                    }
                  }
                  _ => {
                    // `None` means the member expression resolve to a ambiguous export, which means it actually resolve to nothing.
                    // It would be rewrite to `undefined` in the final code, so we don't need to include anything to make `undefined` work.
                  }
                }
              }
            }
          });
        });
        let needs_inherit_to_esm_runtime = meta.dependencies.iter().any(|dep_module_idx| {
          let Some(dep_module) = self.module_table[*dep_module_idx].as_normal() else {
            return false;
          };
          if dep_module.is_included() {
            return false;
          }

          let dep_meta = &self.metas[*dep_module_idx];
          dep_meta.depended_runtime_helper.contains(RuntimeHelper::ToEsm)
        });
        let inherited_runtime = if needs_inherit_to_esm_runtime {
          RuntimeHelper::ToEsm
        } else {
          RuntimeHelper::default()
        };
        (module_idx, extended_dependencies, inherited_runtime)
      })
      .collect::<Vec<_>>();

    // inherit runtime helpers from dependencies
    // Dependencies may be eliminated by tree-shaking, but their runtime helpers might still need to be transitively included.
    // Example: see crates/rolldown/tests/rolldown/issues/4585 for a real-world case
    // ```js
    // // main.js
    // import { A } from './a.js' // a.js has side effects and requires runtime helper
    // console.log(A);
    // // a.js
    // export { A } from './lib.js'
    // // lib.js
    // export { resolve as A } from 'node:path' // generates
    // // `__toESM(require('node:path'))` which requires runtime helper `__toESM`
    // ```
    //
    // When `format: 'cjs'` and platform is set to `node`, external modules with `node:` prefix
    // are considered side-effect free. Therefore `a.js` and `lib.js` are skipped in the linking phase,
    // and only `main.js` is included.
    //
    // Since we're using `format: 'cjs'`, we need to generate code like `const path = __toESM(require('node:path'))`,
    // but runtime helpers are calculated in isolation (main.js didn't reference any runtime helpers at this point).
    // If we don't inherit runtime helpers from eliminated dependencies, the program will panic because
    // `"__toESM" is not in any chunk, which is unexpected.
    //
    // Currently, only the `toESM` helper needs to be transitively included.
    //
    //
    for (module_idx, extended_dependencies, runtime_helper) in processed_module_results {
      let meta = &mut self.metas[module_idx];
      meta.dependencies.extend(extended_dependencies);
      meta.depended_runtime_helper |= runtime_helper;

      if !runtime_helper.is_empty() {
        meta.dependencies.insert(self.runtime.id());
      }
    }
  }
}
