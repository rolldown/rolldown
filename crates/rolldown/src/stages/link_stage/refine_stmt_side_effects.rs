use oxc::semantic::SymbolId;
use rolldown_common::{
  ConstExportMeta, ConstantValue, GetLocalDb, IndexModules, ModuleIdx, Specifier, StmtEvalFlags,
  StmtInfoIdx, SymbolOrMemberExprRef, SymbolRef, SymbolRefDb,
};
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ast_scanner::stmt_eval_analyzer::{ConstantSymbolLookup, StmtEvalAnalyzer},
  type_alias::IndexStmtInfos,
};

use super::LinkStage;

/// Resolves the symbols of one module to the constants known at link time, for
/// [`StmtEvalAnalyzer`] runs after imports are bound (see [`ConstantSymbolLookup`]).
///
/// A binding only resolves when its canonical declaration is a `const` whose initializer has run
/// before the analyzed statement:
/// - A binding of the module itself: every statement declaring it precedes `stmt_idx`.
/// - An import binding: the exporting module executes before this module. That fails inside an
///   import cycle, where the exporter may still be running and the `const` is in its TDZ (see
///   `internal-docs/linking/module-execution-order/implementation.md`).
pub(super) struct LinkedConstantLookup<'a> {
  pub module_idx: ModuleIdx,
  pub stmt_idx: StmtInfoIdx,
  pub modules: &'a IndexModules,
  pub symbols: &'a SymbolRefDb,
  pub stmt_infos: &'a IndexStmtInfos,
  pub constants: &'a FxHashMap<SymbolRef, ConstExportMeta>,
}

impl LinkedConstantLookup<'_> {
  /// The canonical `const` declaration behind `symbol_ref` and its constant value.
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
  }

  /// Whether `symbol_ref`, a binding of this module, is an import of a constant that is
  /// initialized before this module runs.
  pub fn is_imported_constant(&self, symbol_ref: SymbolRef) -> bool {
    self.canonical_constant(symbol_ref).is_some_and(|(canonical_ref, _)| {
      canonical_ref.owner != self.module_idx && self.exporter_runs_first(canonical_ref.owner)
    })
  }
}

impl ConstantSymbolLookup for LinkedConstantLookup<'_> {
  fn constant_value(&self, symbol_id: SymbolId) -> Option<&ConstantValue> {
    let (canonical_ref, value) = self.canonical_constant((self.module_idx, symbol_id).into())?;
    let initialized = if canonical_ref.owner == self.module_idx {
      self.stmt_infos[self.module_idx]
        .declared_stmts_by_symbol(&canonical_ref)
        .iter()
        .all(|declaring_stmt_idx| *declaring_stmt_idx < self.stmt_idx)
    } else {
      self.exporter_runs_first(canonical_ref.owner)
    };
    initialized.then_some(value)
  }
}

impl LinkStage<'_> {
  /// Re-analyze statements the scanner flagged `UnknownSideEffect` while an imported binding they
  /// coerce was still unknown (`60 * SECOND`, `` `${NAME}` `` with `SECOND`/`NAME` imported).
  /// Imports are bound now, so `global_constant_symbol_map` can tell what such a binding holds
  /// (#10817). A statement is only ever relaxed to side-effect free here; the flags of every other
  /// statement stay as the scanner computed them. Module-local constants need no second pass: the
  /// scanner already saw them.
  ///
  /// Relaxed modules are recorded in `relaxed_side_effect_modules`;
  /// `recompute_analyzed_side_effects` re-derives their module verdicts afterwards so
  /// `reference_needed_symbols` and `include_statements` see the relaxed statements.
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
          symbols: &self.symbols,
          stmt_infos: &self.stmt_infos,
          constants: &self.global_constant_symbol_map,
        };
        // Most modules import no constant at all; skip their statements without a walk.
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
        // Mirrors the scanner's `add_star_import` set, which is not persisted.
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
              // `stmt_infos[0]` is the namespace statement; the program body starts at index 1.
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
