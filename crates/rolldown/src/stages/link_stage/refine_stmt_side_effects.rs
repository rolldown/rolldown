use std::sync::OnceLock;

use oxc::semantic::SymbolId;
use oxc_index::{IndexVec, index_vec};
use petgraph::{
  graph::{DiGraph, NodeIndex},
  visit::{Dfs, Reversed},
};
use rolldown_common::{
  ConstExportMeta, ConstantValue, EntryPoint, ExportsKind, GetLocalDb, IndexModules, ModuleIdx,
  Specifier, StmtEvalFlags, StmtInfoIdx, SymbolOrMemberExprRef, SymbolRef, SymbolRefDb,
};
use rolldown_utils::{
  IndexBitSet,
  indexmap::FxIndexMap,
  rayon::{IntoParallelRefIterator, ParallelIterator},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ast_scanner::stmt_eval_analyzer::{ConstantSymbolLookup, StmtEvalAnalyzer},
  type_alias::IndexStmtInfos,
};

use super::LinkStage;

/// Static-import components shared by link-time cycle checks, initialized on first use.
/// Iterative DFS handles deep graphs.
#[derive(Debug, Default)]
pub(super) struct ImportCycles {
  components: OnceLock<Components>,
  regions: OnceLock<Regions>,
}

#[derive(Debug)]
struct Components {
  /// One module of the component of each module.
  representatives: IndexVec<ModuleIdx, ModuleIdx>,
  /// The representatives, with each importing component before the components it imports.
  topological_order: Vec<ModuleIdx>,
  /// Whether each module is in a cycle or can reach an external module.
  imports_may_run_later: IndexBitSet<ModuleIdx>,
}

/// A region is a component and the components that only it imports, directly or through other
/// such components. A component with an entry starts its own region. Code can reach a region only
/// through its first component, so when code can start that component at only one module, every
/// entry runs the region in one order.
/// See `internal-docs/linking/module-execution-order/implementation.md`.
#[derive(Debug)]
struct Regions {
  /// The representative of the first component of the region of each representative.
  roots: IndexVec<ModuleIdx, ModuleIdx>,
  /// The roots where code can start the component at only one module.
  one_start: IndexBitSet<ModuleIdx>,
}

impl ImportCycles {
  fn components(&self, modules: &IndexModules) -> &Components {
    self.components.get_or_init(|| {
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
      let mut representatives = (0..modules.len()).map(ModuleIdx::new).collect::<IndexVec<_, _>>();
      let mut imports_may_run_later = IndexBitSet::new(modules.len());
      let components = petgraph::algo::kosaraju_scc(&graph);
      // `kosaraju_scc` returns the components in reverse topological order.
      let topological_order =
        components.iter().rev().map(|component| ModuleIdx::new(component[0].index())).collect();
      for component in components {
        let representative = ModuleIdx::new(component[0].index());
        let is_cycle = component.len() > 1;
        for node in component {
          let module_idx = ModuleIdx::new(node.index());
          representatives[module_idx] = representative;
          if is_cycle {
            imports_may_run_later.set_bit(module_idx);
          }
        }
      }
      // An external module can import emitted modules and complete a cycle we cannot see.
      let reverse_graph = Reversed(&graph);
      let mut dfs = Dfs::empty(reverse_graph);
      dfs.stack.extend(
        modules.iter().filter_map(|module| {
          module.is_external().then_some(NodeIndex::new(module.idx().index()))
        }),
      );
      while let Some(node) = dfs.next(reverse_graph) {
        imports_may_run_later.set_bit(ModuleIdx::new(node.index()));
      }
      Components { representatives, topological_order, imports_may_run_later }
    })
  }

  fn regions(
    &self,
    modules: &IndexModules,
    entries: &FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
  ) -> &Regions {
    let components = self.components(modules);
    self.regions.get_or_init(|| {
      let representatives = &components.representatives;
      // The first module where code can start each component, and the first importing component.
      let mut starts = index_vec![None; modules.len()];
      let mut many_starts = IndexBitSet::new(modules.len());
      let mut add_start = |module_idx: ModuleIdx| {
        let component = representatives[module_idx];
        match starts[component] {
          None => starts[component] = Some(module_idx),
          Some(start) if start != module_idx => {
            many_starts.set_bit(component);
          }
          Some(_) => {}
        }
      };
      let mut importers = index_vec![None; modules.len()];
      let mut own_region = IndexBitSet::new(modules.len());
      for &entry in entries.keys() {
        add_start(entry);
        own_region.set_bit(representatives[entry]);
      }
      for module in modules {
        let importer = representatives[module.idx()];
        for record in module.import_records().iter().filter(|record| record.kind.is_static()) {
          let Some(dependency) = record.resolved_module else { continue };
          let component = representatives[dependency];
          if component == importer {
            continue;
          }
          add_start(dependency);
          match importers[component] {
            None => importers[component] = Some(importer),
            Some(other) if other != importer => {
              own_region.set_bit(component);
            }
            Some(_) => {}
          }
        }
      }
      let mut roots = representatives.clone();
      for &component in &components.topological_order {
        if !own_region.has_bit(component)
          && let Some(importer) = importers[component]
        {
          roots[component] = roots[importer];
        }
      }
      let mut one_start = IndexBitSet::new(modules.len());
      for (component, start) in starts.iter_enumerated() {
        if start.is_some() && !many_starts.has_bit(component) {
          one_start.set_bit(component);
        }
      }
      Regions { roots, one_start }
    })
  }

  pub(super) fn same_cycle(
    &self,
    modules: &IndexModules,
    importer: ModuleIdx,
    exporter: ModuleIdx,
  ) -> bool {
    let representatives = &self.components(modules).representatives;
    representatives[importer] == representatives[exporter]
  }

  /// Whether the static imports of `module` finish before its own code runs, regardless of
  /// which entry loads first.
  /// See `internal-docs/linking/module-execution-order/implementation.md`.
  pub(super) fn imports_finish_before_evaluation(
    &self,
    modules: &IndexModules,
    module: ModuleIdx,
  ) -> bool {
    !self.components(modules).imports_may_run_later.has_bit(module)
  }

  /// Whether every entry runs `caller` and `owner` in one order: both modules are in one region,
  /// and code can start the region at only one module.
  pub(super) fn runs_in_one_order(
    &self,
    modules: &IndexModules,
    entries: &FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
    caller: ModuleIdx,
    owner: ModuleIdx,
  ) -> bool {
    let representatives = &self.components(modules).representatives;
    let regions = self.regions(modules, entries);
    let root = regions.roots[representatives[caller]];
    root == regions.roots[representatives[owner]] && regions.one_start.has_bit(root)
  }
}

/// Link-time [`ConstantSymbolLookup`] for one module, for analyzer runs after the link stage binds
/// the imports.
///
/// A binding resolves only when its canonical declaration is a `const` or an
/// `export default <expression>` that ran before the analyzed statement: a local binding when every declaring statement precedes `stmt_idx`, an import when
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
  /// Returns the canonical declaration behind `symbol_ref` and its value when it is a `const` or an
  /// `export default <expression>`.
  fn canonical_constant(&self, symbol_ref: SymbolRef) -> Option<(SymbolRef, &ConstantValue)> {
    let canonical_ref = self.symbols.canonical_ref_for(symbol_ref);
    let meta = self.constants.get(&canonical_ref)?;
    // `export default <expression>` binds a name that no code can assign.
    let is_default_export_expression = self.modules[canonical_ref.owner]
      .as_normal()
      .is_some_and(|module| module.ecma_view.default_export_ref == canonical_ref)
      && self.symbols.is_facade_symbol(canonical_ref);
    let is_const = is_default_export_expression
      || self
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
  /// Re-analyze the statements that the scanner flagged `UnknownSideEffect` and that read an imported
  /// constant, such as a coercion of it (#10817) or an `if` whose test it decides.
  /// `global_constant_symbol_map` knows those constants now. This pass
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
