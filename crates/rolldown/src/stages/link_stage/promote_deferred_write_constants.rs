use rolldown_common::{
  ConstExportMeta, ExportsKind, ImportKind, ImportRecordMeta, Module, ModuleIdx, NormalModule,
  Specifier, StmtEvalFlags, StmtInfoIdx, StmtInfos, SymbolOrMemberExprRef, SymbolRef,
  SymbolRefFlags, side_effects::DeterminedSideEffects,
};
use rolldown_utils::IndexBitSet;
#[cfg(target_family = "wasm")]
use rolldown_utils::rayon::IteratorExt as _;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use super::LinkStage;

type PromotedConstant = (SymbolRef, ConstExportMeta, Vec<StmtInfoIdx>);

/// What an importer uses from a module with candidates.
enum ImportUse {
  /// `require()`, `new URL()`, `export * from` and `import * as` expose each export.
  All,
  Symbol(SymbolRef),
}

impl LinkStage<'_> {
  /// Makes an exported `let` a constant when every write to it is inside a function that no code
  /// can call (#9698):
  ///
  /// ```js
  /// export let flag = false;
  /// export function enable() { flag = true; } // nothing imports `enable`
  /// ```
  ///
  /// Then tree shaking drops each writer statement, and the binding keeps its initial value. The
  /// decision happens before the constant passes, so they all see the same constant map. See
  /// internal-docs/constant-branches/implementation.md.
  ///
  /// A writer statement can still run if its module exposes it, if code of its module reaches it
  /// from a statement with side effects, or if another module imports it. The checks below must
  /// cover each way that `include_statements` can include a statement, also through the
  /// references that later passes add. In debug builds, `assert_promoted_writers_excluded` checks
  /// this after tree shaking.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn promote_deferred_write_constants(&mut self) {
    if !self.options.optimization.is_inline_const_enabled()
      || self.options.treeshake.is_none()
      // Dev mode includes the namespace object of each ESM module, so each export can be used.
      || self.options.is_dev_mode_enabled()
    {
      return;
    }
    let module_count = self.module_table.modules.len();
    let mut has_candidates = IndexBitSet::<ModuleIdx>::new(module_count);
    let mut is_importer = IndexBitSet::<ModuleIdx>::new(module_count);
    let mut candidate_modules = Vec::new();
    let mut importers = Vec::new();
    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      if module.ecma_view.deferred_write_constants.is_empty() || !self.can_promote_in(module) {
        continue;
      }
      has_candidates.set_bit(module.idx);
      candidate_modules.push(module.idx);
      importers.extend(
        module.ecma_view.importers_idx.iter().copied().filter(|idx| is_importer.set_bit(*idx)),
      );
    }
    if candidate_modules.is_empty() {
      return;
    }

    // Read each importer one time, also when it imports many modules with candidates.
    let import_uses = importers
      .par_iter()
      .map(|importer_idx| self.import_uses(*importer_idx, &has_candidates))
      .flatten_iter()
      .collect::<Vec<_>>();
    // The symbols that importers use from each module, or `None` when they can use each export.
    let mut used_symbols = FxHashMap::<ModuleIdx, Option<Vec<SymbolRef>>>::default();
    for (module_idx, import_use) in import_uses {
      let used = used_symbols.entry(module_idx).or_insert_with(|| Some(Vec::new()));
      match import_use {
        ImportUse::All => *used = None,
        ImportUse::Symbol(symbol_ref) => {
          if let Some(used) = used {
            used.push(symbol_ref);
          }
        }
      }
    }

    let promoted = candidate_modules
      .par_iter()
      .filter_map(|module_idx| {
        let used = match used_symbols.get(module_idx) {
          Some(None) => return None,
          Some(Some(used)) => used.as_slice(),
          None => &[],
        };
        self.promotable_constants(self.module_table[*module_idx].as_normal()?, used)
      })
      .collect::<Vec<_>>();

    for (symbol_ref, meta, writer_stmts) in promoted.into_iter().flatten() {
      // No included code writes the binding, so it is not reassigned in the output. Other passes
      // check this flag together with the constant map, and an export of the binding can be a
      // value instead of a live getter.
      symbol_ref.flags_mut(&mut self.symbols).insert(SymbolRefFlags::IsNotReassigned);
      self.global_constant_symbol_map.insert(symbol_ref, meta);
      self.promoted_constant_writers.push((symbol_ref.owner, writer_stmts));
    }
  }

  /// Whether this pass can see each use of the exports of a module.
  fn can_promote_in(&self, module: &NormalModule) -> bool {
    let module_idx = module.idx;
    matches!(module.exports_kind, ExportsKind::Esm)
      && !module.meta.has_eval()
      && !matches!(module.side_effects, DeterminedSideEffects::NoTreeshake)
      && self.metas[module_idx].included_commonjs_export_symbol.is_empty()
      // An entry, `import()` or `import.meta.hot.accept()` can use each export.
      && !self.entries.contains_key(&module_idx)
      && module.ecma_view.dynamic_importers.is_empty()
      // `reference_needed_symbols` runs after this pass. For an `export *` of a module with dynamic
      // exports, it adds a reference to the namespace object of this module.
      && !self.metas[module_idx].has_dynamic_exports
  }

  /// What one importer uses from the modules with candidates.
  fn import_uses(
    &self,
    importer_idx: ModuleIdx,
    has_candidates: &IndexBitSet<ModuleIdx>,
  ) -> Vec<(ModuleIdx, ImportUse)> {
    // An external module has no import records.
    let Some(importer) = self.module_table[importer_idx].as_normal() else {
      return Vec::new();
    };
    let mut uses = Vec::new();
    for record in &importer.import_records {
      if let Some(module_idx) = record.resolved_module
        && has_candidates.has_bit(module_idx)
        && (record.kind != ImportKind::Import
          || record.meta.contains(ImportRecordMeta::IsExportStar))
      {
        uses.push((module_idx, ImportUse::All));
      }
    }
    for named_import in importer.named_imports.values() {
      let Some(module_idx) = importer.import_records[named_import.record_idx].resolved_module
      else {
        continue;
      };
      if !has_candidates.has_bit(module_idx) {
        continue;
      }
      let Specifier::Literal(name) = &named_import.imported else {
        uses.push((module_idx, ImportUse::All));
        continue;
      };
      let Some(export) = self.metas[module_idx].resolved_exports.get(name) else {
        continue;
      };
      let ambiguous_refs =
        export.potentially_ambiguous_symbol_refs.iter().flat_map(|refs| refs.iter().copied());
      uses.extend(
        std::iter::once(export.symbol_ref)
          .chain(ambiguous_refs)
          .map(|symbol_ref| (module_idx, ImportUse::Symbol(symbol_ref))),
      );
    }
    uses
  }

  /// The candidates of one module that no code can write. `used` holds the symbols that importers
  /// use from the module.
  fn promotable_constants(
    &self,
    module: &NormalModule,
    used: &[SymbolRef],
  ) -> Option<Vec<PromotedConstant>> {
    let module_idx = module.idx;
    let candidates = &module.ecma_view.deferred_write_constants;
    let stmt_infos = &self.stmt_infos[module_idx];
    // Each symbol that a writer statement declares, and the candidates that it writes.
    let mut writers_by_symbol = FxHashMap::<SymbolRef, Vec<usize>>::default();
    for (candidate_idx, candidate) in candidates.iter().enumerate() {
      for stmt_idx in &candidate.writer_stmts {
        for declared in &stmt_infos[*stmt_idx].declared_symbols {
          writers_by_symbol.entry(declared.inner()).or_default().push(candidate_idx);
        }
      }
    }
    let mut is_live = vec![false; candidates.len()];
    let mut live_count = 0;
    // The symbols of this module that another module imports.
    let mut used_from_outside = FxHashSet::<SymbolRef>::default();
    for symbol_ref in used {
      let canonical_ref = self.symbols.canonical_ref_for(*symbol_ref);
      if canonical_ref.owner != module_idx || !used_from_outside.insert(canonical_ref) {
        continue;
      }
      for candidate_idx in writers_by_symbol.get(&canonical_ref).into_iter().flatten() {
        if !is_live[*candidate_idx] {
          is_live[*candidate_idx] = true;
          live_count += 1;
        }
      }
    }
    // Importers import each writer by name.
    if live_count == candidates.len() {
      return None;
    }

    let reached = self.reach_statements(module_idx, &used_from_outside)?;
    let promoted = candidates
      .iter()
      .zip(is_live)
      .filter(|(candidate, is_live)| {
        !is_live && candidate.writer_stmts.iter().all(|stmt_idx| !reached.has_bit(*stmt_idx))
      })
      .map(|(candidate, _)| {
        let symbol_ref: SymbolRef = (module_idx, candidate.symbol).into();
        (symbol_ref, candidate.meta.clone(), candidate.writer_stmts.clone())
      })
      .collect::<Vec<_>>();
    (!promoted.is_empty()).then_some(promoted)
  }

  /// The statements of a module that its own side effects or the symbols that other modules use
  /// can reach. `None` when a statement reaches the namespace object of the module.
  fn reach_statements(
    &self,
    module_idx: ModuleIdx,
    used_from_outside: &FxHashSet<SymbolRef>,
  ) -> Option<IndexBitSet<StmtInfoIdx>> {
    let stmt_infos = &self.stmt_infos[module_idx];
    // The statements that tree shaking includes with a symbol of this module: the declarations,
    // and with `propertyWriteSideEffects: false` the statements that write a property of the
    // symbol. Tree shaking finds the property writes from the symbol that the code references,
    // so the walk adds them for that symbol and for its canonical symbol.
    let symbol_stmts = |symbol_ref: &SymbolRef| {
      (symbol_ref.owner == module_idx)
        .then(|| {
          let property_writes = stmt_infos.symbol_ref_to_referenced_stmt_idx().get(symbol_ref);
          stmt_infos
            .declared_stmts_by_symbol(symbol_ref)
            .iter()
            .chain(property_writes.into_iter().flatten())
            .copied()
        })
        .into_iter()
        .flatten()
    };
    let mut reached = IndexBitSet::new(stmt_infos.len());
    let mut pending = stmt_infos
      .iter_enumerated_without_namespace_stmt()
      .filter(|(_, stmt_info)| {
        // The same flags that `sweep_side_effect_statements` uses.
        stmt_info.eval_flags.has_side_effect_for_tree_shaking()
          || stmt_info.eval_flags.contains(StmtEvalFlags::UnknownSideEffect)
      })
      .map(|(stmt_idx, _)| stmt_idx)
      .collect::<Vec<_>>();
    for symbol_ref in used_from_outside {
      pending.extend(symbol_stmts(symbol_ref));
    }
    let resolved_member_expr_refs = &self.metas[module_idx].resolved_member_expr_refs;
    while let Some(stmt_idx) = pending.pop() {
      if stmt_idx == StmtInfos::NAMESPACE_STMT_IDX {
        return None;
      }
      if !reached.set_bit(stmt_idx) {
        continue;
      }
      for reference in &stmt_infos[stmt_idx].referenced_symbols {
        let mut push = |symbol_ref: SymbolRef| {
          pending.extend(symbol_stmts(&symbol_ref));
          let canonical_ref = self.symbols.canonical_ref_for(symbol_ref);
          if canonical_ref != symbol_ref {
            pending.extend(symbol_stmts(&canonical_ref));
          }
          if let Some(alias) = &self.symbols.get(canonical_ref).namespace_alias {
            let namespace_ref = self.symbols.canonical_ref_for(alias.namespace_ref);
            pending.extend(symbol_stmts(&namespace_ref));
          }
        };
        match reference {
          SymbolOrMemberExprRef::Symbol(symbol_ref) => push(*symbol_ref),
          SymbolOrMemberExprRef::MemberExpr(member_expr) => {
            push(member_expr.object_ref);
            if let Some(resolution) = member_expr.resolution(resolved_member_expr_refs) {
              resolution.resolved.into_iter().for_each(&mut push);
              resolution.depended_refs.iter().copied().for_each(&mut push);
            }
          }
        }
      }
    }
    Some(reached)
  }

  /// Tree shaking must drop each writer statement of a promoted constant.
  pub(super) fn assert_promoted_writers_excluded(&self) {
    if !cfg!(debug_assertions) {
      return;
    }
    let modules_with_included_writers = self
      .promoted_constant_writers
      .iter()
      .filter(|(module_idx, writer_stmts)| {
        let included = &self.metas[*module_idx].stmt_info_included;
        writer_stmts.iter().any(|stmt_idx| included.has_bit(*stmt_idx))
      })
      .map(|(module_idx, _)| self.module_table[*module_idx].id().as_str())
      .collect::<Vec<_>>();
    debug_assert!(
      modules_with_included_writers.is_empty(),
      "a writer of a promoted constant is included in {modules_with_included_writers:?}",
    );
  }
}
