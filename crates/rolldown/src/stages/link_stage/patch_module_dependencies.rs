use petgraph::{Direction, prelude::DiGraphMap};
use rolldown_common::{ImportKind, Module, ModuleIdx, RuntimeHelper, StmtInfoMeta, SymbolRef};
use rolldown_utils::{
  index_vec_ext::IndexVecRefExt, indexmap::FxIndexSet, rayon::ParallelIterator,
};
use rustc_hash::{FxHashMap, FxHashSet};

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

    let processed_module_results = self
      .metas
      .par_iter_enumerated()
      .map(|(module_idx, meta)| {
        let mut extended_dependencies = FxIndexSet::default();
        if !meta.depended_runtime_helper.is_empty() {
          extended_dependencies.insert(self.runtime.id());
        }

        // Set when this module's own included code reads an external module as an ES module. The
        // `__toESM` that renders it is requested by the import statement, which may sit in a
        // module tree-shaking already dropped, so the edge to the runtime has to be (re)derived
        // from the reference itself. See `chunk_recorded_external_interop` and issue #10069.
        let mut reads_external_as_esm = false;
        let mut note_external_interop = |canonical_ref: SymbolRef| {
          // Runs for every referenced symbol of every module; the flag is monotonic, so once it is
          // set there is nothing left to learn.
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
          // Mirror `chunk_recorded_external_interop`, which puts the wrapper only in the chunks the
          // observers land in: a module that merely reads a *name* off the same external renders no
          // `__toESM` call and must not demand the helper, or its chunk gains a cross-chunk import
          // whose binding then dies in DCE, leaving a bare `require` of the runtime chunk behind.
          if unattributable_externals.contains(&namespace_ref)
            || observers.contains_key(&module_idx)
          {
            reads_external_as_esm = true;
          }
        };

        // Symbols from runtime are referenced by bundler not import statements.
        meta.referenced_symbols_by_entry_point_chunk.iter().for_each(
          |(symbol_ref, _came_from_cjs)| {
            let canonical_ref = self.symbols.canonical_ref_for(*symbol_ref);
            extended_dependencies.insert(canonical_ref.owner);
            // An entry export may resolve to a facade binding whose value lives on a CJS
            // module's namespace (e.g. re-exporting a name that a CJS module only provides
            // dynamically). Inclusion follows that alias (`follow_cjs_namespace_alias`), so the
            // aliased module is a real dependency of the entry, same as in the statement walk
            // below.
            let symbol = self.symbols.get(canonical_ref);
            if let Some(ns) = &symbol.namespace_alias {
              extended_dependencies.insert(ns.namespace_ref.owner);
            }
            // An entry export can be the *only* live reference to an external — no included
            // statement mentions it, and no `named_imports` entry of this chunk covers it either.
            // The observer recorded for it still lands here, so the chunk renders
            // `__toESM(require(...))`; without this edge the helper has no cross-chunk binding here
            // and finalization panics looking one up. Pinned by
            // `external_interop_default_reexport_only_reachable_via_entry_export`.
            note_external_interop(canonical_ref);
          },
        );

        let Module::Normal(_) = &self.module_table[module_idx] else {
          // External modules are not rendered, so they never need a runtime-helper edge.
          return (module_idx, extended_dependencies, RuntimeHelper::default());
        };

        self.stmt_infos[module_idx]
          .iter_enumerated()
          .filter(|(idx, _)| meta.stmt_info_included.has_bit(*idx))
          .for_each(|(_, stmt_info)| {
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
                  note_external_interop(canonical_ref);
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
                      note_external_interop(canonical_ref);
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
          let Some(_) = self.module_table[*dep_module_idx].as_normal() else {
            return false;
          };
          if self.metas[*dep_module_idx].is_included {
            return false;
          }

          let dep_meta = &self.metas[*dep_module_idx];
          dep_meta.depended_runtime_helper.contains(RuntimeHelper::ToEsm)
        });
        let inherited_runtime = if needs_inherit_to_esm_runtime || reads_external_as_esm {
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
    let tree_shaking = self.options.treeshake.is_some();
    let strict_execution_order = self.options.is_strict_execution_order_enabled();
    for (module_idx, extended_dependencies, runtime_helper) in processed_module_results {
      // Symbol-derived dependencies always force their owner module to be loaded. Import-record
      // targets (what `meta.dependencies` holds at this point) only do so when evaluating them
      // has side effects — the same edge semantics `include_side_effectful_dependencies` uses
      // during tree-shaking and `compute_cross_chunk_links` uses when emitting bare imports.
      //
      // Record edges into entry modules are kept even when side-effect-free: an entry's chunk
      // exists regardless, and letting static importers participate in its bit pattern keeps
      // shared code co-located with the entry chunk (Rollup collapses such code-less entry
      // facades onto the chunk holding their exports — e.g. a statically imported re-export
      // barrel that is also a dynamic entry must not push its re-export targets into a separate
      // chunk, see rollup's `entry-without-code-dynamic`).
      let execution_dependencies = strict_execution_order.then(|| {
        extended_dependencies
          .iter()
          .copied()
          .chain(self.metas[module_idx].dependencies.iter().copied().filter(|dep_idx| {
            !tree_shaking || self.module_table[*dep_idx].side_effects().has_side_effects()
          }))
          .collect::<FxIndexSet<ModuleIdx>>()
      });
      let load_dependencies: FxIndexSet<ModuleIdx> =
        if let Some(execution_dependencies) = &execution_dependencies {
          execution_dependencies
            .iter()
            .copied()
            .chain(
              self.metas[module_idx]
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
            .chain(self.metas[module_idx].dependencies.iter().copied().filter(|dep_idx| {
              !tree_shaking
                || self.entries.contains_key(dep_idx)
                || self.module_table[*dep_idx].side_effects().has_side_effects()
            }))
            .collect()
        };

      let meta = &mut self.metas[module_idx];
      meta.dependencies.extend(extended_dependencies);
      meta.load_dependencies = load_dependencies;
      if let Some(execution_dependencies) = execution_dependencies {
        meta.execution_dependencies = execution_dependencies;
      }
      meta.depended_runtime_helper |= runtime_helper;

      if !runtime_helper.is_empty() {
        meta.dependencies.insert(self.runtime.id());
        meta.load_dependencies.insert(self.runtime.id());
        if strict_execution_order {
          meta.execution_dependencies.insert(self.runtime.id());
        }
      }
    }

    if strict_execution_order && tree_shaking {
      self.patch_order_sensitive_static_dependencies();
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

  fn patch_order_sensitive_static_dependencies(&mut self) {
    let mut graph = DiGraphMap::<ModuleIdx, ()>::new();
    let mut reaches_sensitive_init = FxHashSet::default();
    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      graph.add_node(module.idx);
      for rec in &module.import_records {
        if rec.kind == ImportKind::Import
          && let Some(importee_idx) = rec.resolved_module
        {
          graph.add_edge(module.idx, importee_idx, ());
        }
      }
      let meta = &self.metas[module.idx];
      if meta.is_included
        && self.stmt_infos[module.idx].iter_enumerated_without_namespace_stmt().any(
          |(stmt_idx, stmt_info)| {
            meta.stmt_info_included.has_bit(stmt_idx)
              && stmt_info.meta.contains(StmtInfoMeta::ExecutionOrderSensitive)
              && (!stmt_info.declared_symbols.is_empty()
                || !stmt_info.referenced_symbols.is_empty())
          },
        )
      {
        reaches_sensitive_init.insert(module.idx);
      }
    }
    if reaches_sensitive_init.is_empty() {
      return;
    }

    let mut pending = reaches_sensitive_init.iter().copied().collect::<Vec<_>>();
    while let Some(module_idx) = pending.pop() {
      for importer_idx in graph.neighbors_directed(module_idx, Direction::Incoming) {
        if reaches_sensitive_init.insert(importer_idx) {
          pending.push(importer_idx);
        }
      }
    }
    let components = petgraph::algo::tarjan_scc(&graph);
    for component in &components {
      if (component.len() > 1 || graph.contains_edge(component[0], component[0]))
        && component.iter().any(|idx| reaches_sensitive_init.contains(idx))
      {
        for &module_idx in component {
          self.metas[module_idx].is_included = true;
        }
      }
    }
    for (module_idx, meta) in self.metas.iter_mut_enumerated() {
      meta.has_order_sensitive_initialization = reaches_sensitive_init.contains(&module_idx);
    }

    // Cycles retain their module guards. Acyclic excluded forwarders preserve each import path's
    // record order; global execution order cannot represent opposite-order entry paths.
    // See internal-docs/code-splitting/design.md#tree-shaking-parity-across-strict-modes.
    let mut first_included = FxHashMap::<ModuleIdx, Vec<ModuleIdx>>::default();
    for component in components {
      for module_idx in component {
        if !reaches_sensitive_init.contains(&module_idx) {
          continue;
        }
        if self.metas[module_idx].is_included {
          first_included.insert(module_idx, vec![module_idx]);
          continue;
        }
        let module = self.module_table[module_idx]
          .as_normal()
          .expect("static forwarders should be normal modules");
        let mut targets = FxIndexSet::default();
        for rec in &module.import_records {
          if rec.kind == ImportKind::Import
            && let Some(importee_idx) = rec.resolved_module
            && reaches_sensitive_init.contains(&importee_idx)
          {
            targets.extend(
              first_included
                .get(&importee_idx)
                .expect("dependency components precede their importers")
                .iter()
                .copied(),
            );
          }
        }
        first_included.insert(module_idx, targets.into_iter().collect());
      }
    }

    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      let meta = &mut self.metas[module.idx];
      if !meta.is_included {
        continue;
      }
      let mut dependencies = FxHashMap::default();
      for (rec_idx, rec) in module.import_records.iter_enumerated() {
        if rec.kind == ImportKind::Import
          && let Some(importee_idx) = rec.resolved_module
          && let Some(targets) = first_included.get(&importee_idx)
          && !targets.is_empty()
        {
          dependencies.insert(rec_idx, targets.clone());
        }
      }
      if !dependencies.is_empty() {
        for targets in dependencies.values() {
          meta.dependencies.extend(targets.iter().copied());
          meta.load_dependencies.extend(targets.iter().copied());
          meta.execution_dependencies.extend(targets.iter().copied());
        }
        meta.static_init_dependencies = Some(Box::new(dependencies));
      }
    }
  }
}
