use std::sync::OnceLock;

use oxc::semantic::SymbolId;
use oxc_index::IndexVec;
use petgraph::graph::{DiGraph, NodeIndex};
use rolldown_common::{
  ConstExportMeta, ConstantValue, ExportsKind, GetLocalDb, IndexModules, ModuleIdx, Specifier,
  StmtEvalFlags, StmtInfoIdx, SymbolOrMemberExprRef, SymbolRef, SymbolRefDb,
};
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ast_scanner::stmt_eval_analyzer::{ConstantSymbolLookup, StmtEvalAnalyzer},
  type_alias::IndexStmtInfos,
};

use super::LinkStage;

/// Static-import components, shared by the link-time analyzer runs. Build them only when a
/// lookup needs to check an imported constant, and use iterative DFS to handle deep graphs.
#[derive(Debug, Default)]
pub(super) struct ImportCycles {
  components: OnceLock<IndexVec<ModuleIdx, ModuleIdx>>,
}

impl ImportCycles {
  fn same_cycle(&self, modules: &IndexModules, importer: ModuleIdx, exporter: ModuleIdx) -> bool {
    let components = self.components.get_or_init(|| {
      let mut graph = DiGraph::<(), ()>::with_capacity(modules.len(), 0);
      for _ in modules {
        graph.add_node(());
      }
      for module in modules {
        for record in module.import_records().iter().filter(|record| record.kind.is_static()) {
          if let Some(dependency) = record.resolved_module {
            graph.add_edge(
              NodeIndex::new(module.idx().index()),
              NodeIndex::new(dependency.index()),
              (),
            );
          }
        }
      }
      let mut components = (0..modules.len()).map(ModuleIdx::new).collect::<IndexVec<_, _>>();
      for component in petgraph::algo::kosaraju_scc(&graph) {
        let representative = ModuleIdx::new(component[0].index());
        for node in component {
          components[ModuleIdx::new(node.index())] = representative;
        }
      }
      components
    });
    components[importer] == components[exporter]
  }
}

/// Link-time [`ConstantSymbolLookup`] for one module, for analyzer runs after the link stage binds
/// the imports.
///
/// A binding resolves only when its canonical declaration is a `const` that ran before the analyzed
/// statement: a local binding when every declaring statement precedes `stmt_idx`, an import when
/// the exporter runs before this module and is outside its import cycle. DFS ordering within a
/// cycle depends on the entry path, so it cannot rule out TDZ failures.
/// See `internal-docs/linking/module-execution-order/implementation.md`.
pub(super) struct LinkedConstantLookup<'a> {
  pub module_idx: ModuleIdx,
  pub stmt_idx: StmtInfoIdx,
  pub modules: &'a IndexModules,
  pub import_cycles: &'a ImportCycles,
  pub symbols: &'a SymbolRefDb,
  pub stmt_infos: &'a IndexStmtInfos,
  pub constants: &'a FxHashMap<SymbolRef, ConstExportMeta>,
}

impl LinkedConstantLookup<'_> {
  /// Returns the canonical `const` declaration behind `symbol_ref` and its value.
  fn canonical_constant(&self, symbol_ref: SymbolRef) -> Option<(SymbolRef, &ConstantValue)> {
    let canonical_ref = self.symbols.canonical_ref_for(symbol_ref);
    let meta = self.constants.get(&canonical_ref)?;
    let is_const = self
      .symbols
      .local_db(canonical_ref.owner)
      .ast_scopes
      .scoping()
      .symbol_flags(canonical_ref.symbol)
      .is_const_variable();
    is_const.then_some((canonical_ref, &meta.value))
  }

  fn exporter_runs_first(&self, exporter: ModuleIdx) -> bool {
    self.modules[exporter].exec_order() < self.modules[self.module_idx].exec_order()
      && !self.import_cycles.same_cycle(self.modules, self.module_idx, exporter)
  }

  /// Returns whether `symbol_ref` imports a constant whose exporter runs before this module.
  pub fn is_imported_constant(&self, symbol_ref: SymbolRef) -> bool {
    self.canonical_constant(symbol_ref).is_some_and(|(canonical_ref, _)| {
      canonical_ref.owner != self.module_idx && self.exporter_runs_first(canonical_ref.owner)
    })
  }

  /// Returns whether the binding `canonical_ref` is initialized when statement `stmt_idx` runs,
  /// by the rules in the type documentation.
  pub fn is_initialized(&self, canonical_ref: SymbolRef) -> bool {
    if canonical_ref.owner == self.module_idx {
      self.stmt_infos[self.module_idx]
        .declared_stmts_by_symbol(&canonical_ref)
        .iter()
        .all(|declaring_stmt_idx| *declaring_stmt_idx < self.stmt_idx)
    } else {
      self.exporter_runs_first(canonical_ref.owner)
    }
  }
}

impl ConstantSymbolLookup for LinkedConstantLookup<'_> {
  fn constant_value(&self, symbol_id: SymbolId) -> Option<&ConstantValue> {
    let (canonical_ref, value) = self.canonical_constant((self.module_idx, symbol_id).into())?;
    self.is_initialized(canonical_ref).then_some(value)
  }
}

impl LinkStage<'_> {
  /// Re-analyze the statements that the scanner flagged `UnknownSideEffect` because they coerce an
  /// imported binding. `global_constant_symbol_map` knows those constants now (#10817). This pass
  /// only relaxes flags. It records the relaxed modules, so `recompute_analyzed_side_effects`
  /// rebuilds their verdicts before `reference_needed_symbols` runs.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn refine_stmt_side_effects_with_imported_constants(&mut self) {
    if self.options.treeshake.is_none() || self.global_constant_symbol_map.is_empty() {
      return;
    }

    let refined_flags: Vec<(ModuleIdx, Vec<(StmtInfoIdx, StmtEvalFlags)>)> = self
      .module_table
      .modules
      .par_iter()
      .filter_map(|module| {
        let module = module.as_normal()?;
        let lookup_for_stmt = |stmt_idx| LinkedConstantLookup {
          module_idx: module.idx,
          stmt_idx,
          modules: &self.module_table.modules,
          import_cycles: &self.import_cycles,
          symbols: &self.symbols,
          stmt_infos: &self.stmt_infos,
          constants: &self.global_constant_symbol_map,
        };
        // Most modules import no constant. Skip them without a walk.
        let module_lookup = lookup_for_stmt(StmtInfoIdx::from_raw_unchecked(0));
        if !module.named_imports.keys().any(|local| module_lookup.is_imported_constant(*local)) {
          return None;
        }
        let candidates = self.stmt_infos[module.idx]
          .iter_enumerated_without_namespace_stmt()
          .filter(|(_, stmt_info)| {
            stmt_info.eval_flags.contains(StmtEvalFlags::UnknownSideEffect)
              && stmt_info.referenced_symbols.iter().any(|referenced| match referenced {
                SymbolOrMemberExprRef::Symbol(symbol_ref) => {
                  module_lookup.is_imported_constant(*symbol_ref)
                }
                SymbolOrMemberExprRef::MemberExpr(_) => false,
              })
          })
          .map(|(stmt_idx, _)| stmt_idx)
          .collect::<Vec<_>>();
        if candidates.is_empty() {
          return None;
        }

        let ast = self.ast_table[module.idx].as_ref()?;
        let ast_scopes = &self.symbols.local_db(module.idx).ast_scopes;
        // The scanner does not persist its `add_star_import` set, so rebuild it here.
        let namespace_object_symbol_ids: FxHashSet<SymbolId> = module
          .named_imports
          .iter()
          .filter_map(|(local_ref, named_import)| {
            matches!(named_import.imported, Specifier::Star).then_some(local_ref.symbol)
          })
          .collect();
        let refined = ast.program.with_dependent(|_owner, dep| {
          candidates
            .into_iter()
            .filter_map(|stmt_idx| {
              // `stmt_infos[0]` is the namespace statement. The program body starts at index 1.
              let stmt = &dep.program.body[stmt_idx.index() - 1];
              let constants = lookup_for_stmt(stmt_idx);
              let flags = StmtEvalAnalyzer::new(
                ast_scopes,
                self.flat_options,
                self.options,
                None,
                Some(&namespace_object_symbol_ids),
                Some(&constants),
              )
              .with_top_level_return_ends_module(!matches!(module.exports_kind, ExportsKind::Esm))
              .analyze_stmt(stmt)
              .tree_shaking_flags();
              (!flags.has_side_effect_for_tree_shaking()).then_some((stmt_idx, flags))
            })
            .collect::<Vec<_>>()
        });
        (!refined.is_empty()).then_some((module.idx, refined))
      })
      .collect();

    for (module_idx, refined) in refined_flags {
      for (stmt_idx, flags) in refined {
        self.stmt_infos[module_idx][stmt_idx].eval_flags = flags;
      }
      self.relaxed_side_effect_modules.insert(module_idx);
    }
  }
}
