use oxc_index::IndexVec;
#[cfg(debug_assertions)]
use rolldown_common::common_debug_symbol_ref;
use rolldown_common::{
  EntryPoint, EntryPointKind, ImportKind, ModuleIdx, ModuleTable, RuntimeModuleBrief, SymbolRef,
  SymbolRefDb, dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_error::BuildDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions,
  type_alias::IndexEcmaAst,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

use super::scan_stage::ScanStageOutput;

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
  pub lived_entry_points: FxHashSet<ModuleIdx>,
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
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

impl<'a> LinkStage<'a> {
  pub fn new(scan_stage_output: ScanStageOutput, options: &'a SharedOptions) -> Self {
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
      lived_entry_points: self.get_lived_entry(),
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
    }
  }

  #[inline]
  fn get_lived_entry(&self) -> FxHashSet<ModuleIdx> {
    self
      .entries
      .iter()
      .filter_map(|item| match item.kind {
        EntryPointKind::UserDefined => Some(item.id),
        EntryPointKind::DynamicImport => {
          // At least one statement that create this entry is included
          let lived = item.related_stmt_infos.iter().any(|(module_idx, stmt_idx)| {
            let module = &self.module_table.modules[*module_idx]
              .as_normal()
              .expect("should be a normal module");
            let stmt_info = &module.stmt_infos[*stmt_idx];
            stmt_info.is_included
          });
          lived.then_some(item.id)
        }
      })
      .collect::<FxHashSet<ModuleIdx>>()
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
