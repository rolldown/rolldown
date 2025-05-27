use rolldown_common::Module;
use rolldown_utils::{index_vec_ext::IndexVecExt, rayon::ParallelIterator};

use super::LinkStage;

impl LinkStage<'_> {
  pub(super) fn patch_module_dependencies(&mut self) {
    self.metas.par_iter_mut_enumerated().for_each(|(module_idx, meta)| {
      // Symbols from runtime are referenced by bundler not import statements.
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
        let canonical_ref = self.symbols.canonical_ref_for(*symbol_ref);
        meta.dependencies.insert(canonical_ref.owner);
      });

      let Module::Normal(module) = &self.module_table.modules[module_idx] else {
        return;
      };

      module.stmt_infos.iter().filter(|stmt_info| stmt_info.is_included).for_each(|stmt_info| {
        // We need this step to include the runtime module, if there are symbols of it.
        // TODO: Maybe we should push runtime module to `LinkingMetadata::dependencies` while pushing the runtime symbols.
        stmt_info.referenced_symbols.iter().for_each(|reference_ref| {
          match reference_ref {
            rolldown_common::SymbolOrMemberExprRef::Symbol(sym_ref) => {
              let canonical_ref = self.symbols.canonical_ref_for(*sym_ref);
              meta.dependencies.insert(canonical_ref.owner);
              let symbol = self.symbols.get(canonical_ref);
              if let Some(ns) = &symbol.namespace_alias {
                meta.dependencies.insert(ns.namespace_ref.owner);
              }
            }
            rolldown_common::SymbolOrMemberExprRef::MemberExpr(member_expr) => {
              match member_expr.represent_symbol_ref(&meta.resolved_member_expr_refs) {
                Some(sym_ref) => {
                  let canonical_ref = self.symbols.canonical_ref_for(sym_ref);
                  meta.dependencies.insert(canonical_ref.owner);
                  let symbol = self.symbols.get(canonical_ref);
                  if let Some(ns) = &symbol.namespace_alias {
                    meta.dependencies.insert(ns.namespace_ref.owner);
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
    });
  }
}
