use rolldown_common::{Module, ModuleIdx, RuntimeHelper, SymbolRef};
use rolldown_utils::{
  index_vec_ext::IndexVecRefExt, indexmap::FxIndexSet, rayon::ParallelIterator,
};
use rustc_hash::FxHashSet;

use super::LinkStage;

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn patch_module_dependencies(&mut self) {
    // Externals with an observer that inclusion could not attribute to a module that will get a
    // chunk. Chunks do not exist yet, so `is_included` stands in for "will have a chunk"; for such
    // an external, rendering falls back to wrapping every chunk emitting it, so every referencing
    // module must demand the helper. Resolved once up front: the answer only depends on the
    // external, and checking it inside the per-reference closure below would rescan a popular
    // external's whole observer set for every module referencing it, making this pass quadratic.
    let unattributable_externals: FxHashSet<SymbolRef> = self
      .used_external_symbols
      .iter_interop_uses()
      .filter(|(_, observers)| observers.keys().any(|observer| !self.metas[*observer].is_included))
      .map(|(namespace_ref, _)| *namespace_ref)
      .collect();

    let tree_shaking = self.options.treeshake.is_some();
    let strict_execution_order = self.options.is_strict_execution_order_enabled();
    let runtime_id = self.runtime.id();
    let processed_module_results = self
      .metas
      .par_iter_enumerated()
      .filter(|(_module_idx, meta)| meta.is_included)
      .map(|(module_idx, meta)| {
        let mut extended_dependencies = FxIndexSet::default();
        if !meta.depended_runtime_helper.is_empty() {
          extended_dependencies.insert(runtime_id);
        }

        // Set when this module's own included code reads an external module as an ES module. The
        // `__toESM` that renders it is requested by the import statement, which may sit in a
        // module tree-shaking already dropped, so the edge to the runtime has to be (re)derived
        // from the reference itself. See `chunk_recorded_external_interop` and issue #10069.
        let mut reads_external_as_esm = false;
        let mut note_external_interop = |canonical_ref: SymbolRef| {
          if reads_external_as_esm {
            return;
          }
          let symbol = self.symbols.get(canonical_ref);
          let namespace_ref = match &symbol.namespace_alias {
            Some(ns) => self.symbols.canonical_ref_for(ns.namespace_ref),
            None => canonical_ref,
          };
          let Some(observers) = self.used_external_symbols.interop_uses_by_observer(&namespace_ref)
          else {
            return;
          };
          if unattributable_externals.contains(&namespace_ref)
            || observers.contains_key(&module_idx)
          {
            reads_external_as_esm = true;
          }
        };

        meta.referenced_symbols_by_entry_point_chunk.iter().for_each(
          |(symbol_ref, _came_from_cjs)| {
            let canonical_ref = self.symbols.canonical_ref_for(*symbol_ref);
            extended_dependencies.insert(canonical_ref.owner);
            let symbol = self.symbols.get(canonical_ref);
            if let Some(ns) = &symbol.namespace_alias {
              extended_dependencies.insert(ns.namespace_ref.owner);
            }
            note_external_interop(canonical_ref);
          },
        );

        let inherited_runtime = if let Module::Normal(module) = &self.module_table[module_idx] {
          if module.meta.has_eval() {
            for reference in module.named_imports.keys() {
              let canonical_ref = self.symbols.canonical_ref_for(*reference);
              extended_dependencies.insert(canonical_ref.owner);
              if let Some(namespace) = &self.symbols.get(canonical_ref).namespace_alias {
                extended_dependencies.insert(namespace.namespace_ref.owner);
              }
              note_external_interop(canonical_ref);
            }
          }

          self.stmt_infos[module_idx]
            .iter_enumerated()
            .filter(|(idx, _)| meta.stmt_info_included.has_bit(*idx))
            .for_each(|(_, stmt_info)| {
              stmt_info.referenced_symbols.iter().for_each(|reference_ref| {
                match reference_ref {
                  rolldown_common::SymbolOrMemberExprRef::Symbol(sym_ref) => {
                    let canonical_ref = self.symbols.canonical_ref_for(*sym_ref);
                    extended_dependencies.insert(canonical_ref.owner);
                    let symbol = self.symbols.get(canonical_ref);
                    if let Some(ns) = &symbol.namespace_alias {
                      extended_dependencies.insert(ns.namespace_ref.owner);
                    }
                    note_external_interop(canonical_ref);
                  }
                  rolldown_common::SymbolOrMemberExprRef::MemberExpr(member_expr) => {
                    if let Some(sym_ref) =
                      member_expr.represent_symbol_ref(&meta.resolved_member_expr_refs)
                    {
                      let canonical_ref = self.symbols.canonical_ref_for(sym_ref);
                      extended_dependencies.insert(canonical_ref.owner);
                      let symbol = self.symbols.get(canonical_ref);
                      if let Some(ns) = &symbol.namespace_alias {
                        extended_dependencies.insert(ns.namespace_ref.owner);
                      }
                      note_external_interop(canonical_ref);
                    }
                  }
                }
              });
            });
          let needs_inherit_to_esm_runtime = meta.dependencies.iter().any(|dep_module_idx| {
            let Some(_) = self.module_table[*dep_module_idx].as_normal() else {
              return false;
            };
            if self.metas[*dep_module_idx].is_included {
              return false;
            }
            self.metas[*dep_module_idx]
              .depended_runtime_helper
              .contains(RuntimeHelper::ToEsm)
          });
          if needs_inherit_to_esm_runtime || reads_external_as_esm {
            RuntimeHelper::ToEsm
          } else {
            RuntimeHelper::default()
          }
        } else {
          RuntimeHelper::default()
        };

        let mut execution_dependencies = strict_execution_order.then(|| {
          extended_dependencies
            .iter()
            .copied()
            .chain(meta.dependencies.iter().copied().filter(|dep_idx| {
              !tree_shaking || self.module_table[*dep_idx].side_effects().has_side_effects()
            }))
            .collect::<FxIndexSet<ModuleIdx>>()
        });
        let mut load_dependencies: FxIndexSet<ModuleIdx> =
          if let Some(execution_dependencies) = &execution_dependencies {
            execution_dependencies
              .iter()
              .copied()
              .chain(
                meta
                  .dependencies
                  .iter()
                  .copied()
                  .filter(|dep_idx| self.entries.contains_key(dep_idx)),
              )
              .collect()
          } else {
            extended_dependencies
              .iter()
              .copied()
              .chain(meta.dependencies.iter().copied().filter(|dep_idx| {
                !tree_shaking
                  || self.entries.contains_key(dep_idx)
                  || self.module_table[*dep_idx].side_effects().has_side_effects()
              }))
              .collect()
          };
        if !inherited_runtime.is_empty() {
          load_dependencies.insert(runtime_id);
          if let Some(exec_deps) = &mut execution_dependencies {
            exec_deps.insert(runtime_id);
          }
        }
        (
          module_idx,
          extended_dependencies,
          inherited_runtime,
          load_dependencies,
          execution_dependencies,
        )
      })
      .collect::<Vec<_>>();

    for (
      module_idx,
      extended_dependencies,
      runtime_helper,
      load_dependencies,
      execution_dependencies,
    ) in processed_module_results
    {
      let meta = &mut self.metas[module_idx];
      meta.dependencies.extend(extended_dependencies);
      meta.load_dependencies = load_dependencies;
      if let Some(execution_dependencies) = execution_dependencies {
        meta.execution_dependencies = execution_dependencies;
      }
      meta.depended_runtime_helper |= runtime_helper;
      if !runtime_helper.is_empty() {
        meta.dependencies.insert(runtime_id);
      }
    }

    // If the runtime module has side effects (e.g. from a plugin transform) and is included,
    // ensure entry modules depend on it so the code splitter can reach it via BFS.
    let runtime_idx = self.runtime.id();
    if self.metas[runtime_idx].is_included {
      if let Some(runtime_module) = self.module_table[runtime_idx].as_normal() {
        if runtime_module.side_effects.has_side_effects() {
          for &entry_module_idx in self.entries.keys() {
            self.metas[entry_module_idx].dependencies.insert(runtime_idx);
            self.metas[entry_module_idx].load_dependencies.insert(runtime_idx);
            if strict_execution_order {
              self.metas[entry_module_idx].execution_dependencies.insert(runtime_idx);
            }
            self.metas[entry_module_idx].has_side_effectful_runtime_dep = true;
          }
        }
      }
    }
  }
}
