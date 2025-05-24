use oxc_index::IndexVec;
#[cfg(debug_assertions)]
use rolldown_common::common_debug_symbol_ref;
use rolldown_common::{
  EntryPoint, EntryPointKind, ImportKind, ImportRecordMeta, ModuleIdx, ModuleTable,
  RuntimeModuleBrief, StmtInfoIdx, SymbolRef, SymbolRefDb,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_error::BuildDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions,
  type_alias::IndexEcmaAst,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

use super::scan_stage::NormalizedScanStageOutput;

mod bind_imports_and_exports;
mod compute_tla;
mod create_exports_for_ecma_modules;
mod determine_module_exports_kind;
mod generate_lazy_export;
mod patch_module_dependencies;
mod reference_needed_symbols;
mod sort_modules;
mod tree_shaking;
mod wrapping;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub module_table: ModuleTable,
  pub entries: Vec<EntryPoint>,
  pub ast_table: IndexEcmaAst,
  // pub sorted_modules: Vec<NormalModuleId>,
  pub metas: LinkingMetadataVec,
  pub symbol_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub safely_merge_cjs_ns_map: FxHashMap<ModuleIdx, Vec<SymbolRef>>,
}

#[derive(Debug)]
pub struct LinkStage<'a> {
  pub module_table: ModuleTable,
  pub entries: Vec<EntryPoint>,
  pub symbols: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metas: LinkingMetadataVec,
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
  pub ast_table: IndexEcmaAst,
  pub options: &'a SharedOptions,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub safely_merge_cjs_ns_map: FxHashMap<ModuleIdx, Vec<SymbolRef>>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

impl<'a> LinkStage<'a> {
  pub fn new(scan_stage_output: NormalizedScanStageOutput, options: &'a SharedOptions) -> Self {
    Self {
      sorted_modules: Vec::new(),
      metas: scan_stage_output
        .module_table
        .modules
        .iter()
        .map(|module| LinkingMetadata {
          dependencies: module
            .import_records()
            .iter()
            .filter_map(|rec| match rec.kind {
              ImportKind::DynamicImport => {
                options.inline_dynamic_imports.then_some(rec.resolved_module)
              }
              ImportKind::Require => None,
              _ => Some(rec.resolved_module),
            })
            .collect(),
          star_exports_from_external_modules: module.as_normal().map_or(vec![], |inner| {
            inner
              .star_exports_from_external_modules(&scan_stage_output.module_table.modules)
              .collect()
          }),
          ..LinkingMetadata::default()
        })
        .collect::<IndexVec<ModuleIdx, _>>(),
      module_table: scan_stage_output.module_table,
      entries: scan_stage_output.entry_points,
      symbols: scan_stage_output.symbol_ref_db,
      runtime: scan_stage_output.runtime,
      warnings: scan_stage_output.warnings,
      errors: vec![],
      ast_table: scan_stage_output.index_ecma_ast,
      dynamic_import_exports_usage_map: scan_stage_output.dynamic_import_exports_usage_map,
      options,
      used_symbol_refs: FxHashSet::default(),
      safely_merge_cjs_ns_map: scan_stage_output.safely_merge_cjs_ns_map,
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub fn link(mut self) -> LinkStageOutput {
    self.sort_modules();
    self.compute_tla();
    self.determine_module_exports_kind();
    self.wrap_modules();
    self.generate_lazy_export();
    self.determine_side_effects();
    self.bind_imports_and_exports();
    self.create_exports_for_ecma_modules();
    self.reference_needed_symbols();
    self.include_statements();
    self.patch_module_dependencies();

    tracing::trace!("meta {:#?}", self.metas.iter_enumerated().collect::<Vec<_>>());

    LinkStageOutput {
      module_table: self.module_table,
      entries: self.entries,
      // sorted_modules: self.sorted_modules,
      metas: self.metas,
      symbol_db: self.symbols,
      runtime: self.runtime,
      warnings: self.warnings,
      errors: self.errors,
      ast_table: self.ast_table,
      used_symbol_refs: self.used_symbol_refs,
      dynamic_import_exports_usage_map: self.dynamic_import_exports_usage_map,
      safely_merge_cjs_ns_map: self.safely_merge_cjs_ns_map,
    }
  }

  #[inline]
  fn get_lived_entry(
    &mut self,
    is_stmt_included_vec: &IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>>,
  ) {
    self.entries.retain(|item| match item.kind {
      EntryPointKind::UserDefined => true,
      EntryPointKind::DynamicImport => {
        let is_dynamic_imported_module_exports_unused = self
          .dynamic_import_exports_usage_map
          .get(&item.id)
          .map(|item| matches!(item, DynamicImportExportsUsage::Partial(set) if set.is_empty()))
          .unwrap_or_default();
        // At least one statement that create this entry is included
        let lived = item.related_stmt_infos.iter().any(|(module_idx, stmt_idx)| {
          let module =
            &self.module_table.modules[*module_idx].as_normal().expect("should be a normal module");
          let stmt_info = &module.stmt_infos[*stmt_idx];
          dbg!(&stmt_info);
          let mut dead_pure_dynamic_import_record_idx = vec![];
          let all_dead_pure_dynamic_import =
            stmt_info.import_records.iter().all(|import_record_idx| {
              let import_record = &module.import_records[*import_record_idx];
              if import_record.resolved_module.is_dummy() {
                return true;
              }
              let importee_side_effects = self.module_table.modules[import_record.resolved_module]
                .side_effects()
                .has_side_effects();
              dbg!(&importee_side_effects);
              let ret = !importee_side_effects;
              if ret {
                dead_pure_dynamic_import_record_idx.push(*import_record_idx);
              }
              ret
            });
          let is_stmt_included = is_stmt_included_vec[*module_idx][*stmt_idx];
          let lived = is_stmt_included
            && (!is_dynamic_imported_module_exports_unused || !all_dead_pure_dynamic_import);
          if !lived {
            // satisfy rustc borrow checker
            let module = self.module_table.modules[*module_idx]
              .as_normal_mut()
              .expect("should be a normal module");
            for ele in dead_pure_dynamic_import_record_idx {
              let rec = &mut module.import_records[ele];
              rec.meta.insert(ImportRecordMeta::DEAD_DYNAMIC_IMPORT);
            }
          }
          lived
        });
        dbg!(&item.id);
        dbg!(&lived);

        lived
      }
    });
  }

  /// A helper function used to debug symbol in link process
  /// given any `SymbolRef` the function will return the string representation of the symbol
  /// format: `${stable_id} -> ${symbol_name}`
  #[cfg(debug_assertions)]
  #[cfg_attr(debug_assertions, allow(unused))]
  pub fn debug_symbol_ref(&self, symbol_ref: SymbolRef) -> String {
    common_debug_symbol_ref(symbol_ref, &self.module_table.modules, &self.symbols)
  }
}
