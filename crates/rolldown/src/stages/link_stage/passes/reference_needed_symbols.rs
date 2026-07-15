use std::convert::Infallible;

use oxc_index::IndexVec;
use rolldown_common::{
  DependedRuntimeHelperMap, ExportsKind, ExternalModule, ImportKind, ImportRecordIdx,
  ImportRecordMeta, Module, ModuleIdx, ModuleTable, NormalModule, OutputFormat,
  ResolvedImportRecord, RuntimeHelper, StmtInfo, StmtInfoIdx, SymbolRef, SymbolRefDb,
  SymbolRefDbForModule, TaggedSymbolRef,
};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
};

use crate::{
  type_alias::IndexStmtInfos, utils::external_import_interop::import_record_needs_interop,
};

use super::{
  CjsNamespaceMerges, DynamicExports, ModuleFormats, ModuleSideEffects, ModuleWrappers,
  ReferenceNeededSymbolsPass, WrapperDeclaration,
};

// See internal-docs/pass-based-pipeline/implementation.md.

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct ReferenceChunkingOptions {
  pub dynamic_import_in_cjs: bool,
  pub code_splitting_disabled: bool,
}

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct ReferenceTreeShakingOptions {
  pub keep_names: bool,
  pub commonjs_treeshake: bool,
}

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct ReferenceNeededSymbolsInput<'a> {
  pub module_table: &'a ModuleTable,
  pub module_formats: &'a ModuleFormats,
  pub module_wrappers: &'a ModuleWrappers,
  pub dynamic_exports: &'a DynamicExports,
  pub module_side_effects: &'a ModuleSideEffects,
  pub cjs_namespace_merges: &'a CjsNamespaceMerges,
  pub runtime_require_ref: Option<SymbolRef>,
  pub output_format: OutputFormat,
  pub chunking: ReferenceChunkingOptions,
  pub tree_shaking: ReferenceTreeShakingOptions,
}

pub(in crate::stages::link_stage) struct ReferenceNeededSymbolsOwned {
  pub symbols: SymbolRefDb,
  pub stmt_infos: IndexStmtInfos,
}

pub(in crate::stages::link_stage) struct StatementRuntimeRequirements {
  slots: IndexVec<ModuleIdx, Box<DependedRuntimeHelperMap>>,
}

impl StatementRuntimeRequirements {
  pub(in crate::stages::link_stage) fn slots(
    &self,
  ) -> &IndexVec<ModuleIdx, Box<DependedRuntimeHelperMap>> {
    &self.slots
  }
}

struct ModuleImportRecordPatches {
  events: Vec<CallRuntimeRequirePatch>,
}

struct CallRuntimeRequirePatch {
  importer: ModuleIdx,
  import_record: ImportRecordIdx,
}

pub(in crate::stages::link_stage) struct ReferenceImportRecordPatches {
  modules: Vec<ModuleImportRecordPatches>,
}

impl ReferenceImportRecordPatches {
  pub(in crate::stages::link_stage) fn apply(self, module_table: &mut ModuleTable) {
    for module_patches in self.modules {
      for event in module_patches.events {
        let Some(module) = module_table[event.importer].as_normal_mut() else {
          std::unreachable!("CallRuntimeRequire patches must target normal modules");
        };
        module.import_records[event.import_record]
          .meta
          .insert(ImportRecordMeta::CallRuntimeRequire);
      }
    }
  }
}

/// One-call ownership envelope. The driver must destructure this immediately; no pass accepts it.
pub(in crate::stages::link_stage) struct ReferenceNeededSymbolsOutput {
  pub symbols: SymbolRefDb,
  pub stmt_infos: IndexStmtInfos,
  pub import_record_patches: ReferenceImportRecordPatches,
}

fn import_symbol_name(identifier: &str) -> String {
  let mut name = String::with_capacity("import_".len() + identifier.len());
  name.push_str("import_");
  name.push_str(identifier);
  name
}

fn assert_reference_layout(
  input: ReferenceNeededSymbolsInput<'_>,
  symbols: &SymbolRefDb,
  stmt_infos: &IndexStmtInfos,
) {
  let module_count = input.module_table.modules.len();
  for (domain, actual) in [
    ("format", input.module_formats.module_count()),
    ("wrapper", input.module_wrappers.module_count()),
    ("dynamic-export", input.dynamic_exports.module_count()),
    ("side-effect", input.module_side_effects.module_count()),
    ("CJS-namespace-merge", input.cjs_namespace_merges.module_count()),
    ("statement", stmt_infos.len()),
    ("symbol", symbols.inner().len()),
  ] {
    std::assert_eq!(
      actual,
      module_count,
      "{domain} layout must match modules before reference analysis"
    );
  }
  for (module_idx, module) in input.module_table.modules.iter_enumerated() {
    let valid = match module {
      Module::Normal(module) => {
        module.idx == module_idx
          && input.module_formats.get(module_idx).is_some()
          && symbols.inner()[module_idx].is_some()
      }
      Module::External(_) => {
        input.module_formats.get(module_idx).is_none()
          && symbols.inner()[module_idx].is_some()
          && std::matches!(input.module_wrappers.declaration(module_idx), WrapperDeclaration::None)
      }
    };
    std::assert!(valid, "reference-analysis slot shape must match module {module_idx:?}");
  }
}

struct ModuleReferenceAnalysis<'a> {
  input: ReferenceNeededSymbolsInput<'a>,
  importer: &'a NormalModule,
  symbol_db: &'a mut SymbolRefDbForModule,
  runtime_requirements: &'a mut DependedRuntimeHelperMap,
  patch_events: Vec<CallRuntimeRequirePatch>,
  symbols_to_be_declared: Vec<(SymbolRef, StmtInfoIdx)>,
}

impl ModuleReferenceAnalysis<'_> {
  fn reference_statement(&mut self, stmt_info_idx: StmtInfoIdx, stmt_info: &mut StmtInfo) {
    if stmt_info.meta.contains(rolldown_common::StmtInfoMeta::HasDummyRecord) {
      self.runtime_requirements.push(RuntimeHelper::Require, stmt_info_idx);
    }
    if stmt_info.meta.intersects(rolldown_common::StmtInfoMeta::NonStaticDynamicImport) {
      self.runtime_requirements.push(RuntimeHelper::ToEsm, stmt_info_idx);
    }
    for record_position in 0..stmt_info.import_records.len() {
      let record_idx = stmt_info.import_records[record_position];
      self.reference_record(record_idx, stmt_info_idx, stmt_info);
    }
    if self.input.tree_shaking.keep_names
      && stmt_info.meta.intersects(rolldown_common::StmtInfoMeta::KeepNamesType)
    {
      self.runtime_requirements.push(RuntimeHelper::Name, stmt_info_idx);
    }
  }

  fn reference_record(
    &mut self,
    record_idx: ImportRecordIdx,
    stmt_info_idx: StmtInfoIdx,
    stmt_info: &mut StmtInfo,
  ) {
    let record = &self.importer.import_records[record_idx];
    let Some(importee_idx) = record.state.resolved_module else { return };
    let input = self.input;
    match &input.module_table[importee_idx] {
      Module::External(importee) => {
        self.reference_external(importee, record_idx, record, stmt_info_idx, stmt_info);
      }
      Module::Normal(importee) => {
        self.reference_normal(importee, record_idx, record, stmt_info_idx, stmt_info);
      }
    }
  }

  fn reference_external(
    &mut self,
    importee: &ExternalModule,
    record_idx: ImportRecordIdx,
    record: &ResolvedImportRecord,
    stmt_info_idx: StmtInfoIdx,
    stmt_info: &mut StmtInfo,
  ) {
    match record.kind {
      ImportKind::Import => {
        if record.meta.contains(ImportRecordMeta::IsExportStar) {
          self.symbol_db.ast_scopes.set_symbol_name(
            record.namespace_ref.symbol,
            &import_symbol_name(&importee.identifier_name),
          );
        } else if std::matches!(
          self.input.output_format,
          OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd
        ) {
          stmt_info.eval_flags = true.into();
          if import_record_needs_interop(self.importer, record_idx) {
            self.runtime_requirements.push(RuntimeHelper::ToEsm, stmt_info_idx);
          }
        }
      }
      ImportKind::Require => {
        if let Some(runtime_require_ref) = self.input.runtime_require_ref {
          stmt_info.referenced_symbols.push(runtime_require_ref.into());
          self.patch_events.push(CallRuntimeRequirePatch {
            importer: self.importer.idx,
            import_record: record_idx,
          });
        }
      }
      ImportKind::DynamicImport
        if std::matches!(self.input.output_format, OutputFormat::Cjs)
          && !self.input.chunking.dynamic_import_in_cjs =>
      {
        self.runtime_requirements.push(RuntimeHelper::ToEsm, stmt_info_idx);
      }
      _ => {}
    }
  }

  fn reference_normal(
    &mut self,
    importee: &NormalModule,
    record_idx: ImportRecordIdx,
    record: &ResolvedImportRecord,
    stmt_info_idx: StmtInfoIdx,
    stmt_info: &mut StmtInfo,
  ) {
    match record.kind {
      ImportKind::Import => {
        self.reference_normal_import(importee, record_idx, record, stmt_info_idx, stmt_info);
      }
      ImportKind::Require => {
        self.reference_normal_require(importee, record, stmt_info_idx, stmt_info);
      }
      ImportKind::DynamicImport => {
        self.reference_normal_dynamic_import(importee, stmt_info_idx, stmt_info);
      }
      ImportKind::AtImport => {
        std::unreachable!("A Js module would never import a CSS module via `@import`");
      }
      ImportKind::UrlImport => {
        std::unreachable!("A Js module would never import a CSS module via `url()`");
      }
      ImportKind::NewUrl | ImportKind::HotAccept => {}
    }
  }

  fn reference_normal_import(
    &mut self,
    importee: &NormalModule,
    record_idx: ImportRecordIdx,
    record: &ResolvedImportRecord,
    stmt_info_idx: StmtInfoIdx,
    stmt_info: &mut StmtInfo,
  ) {
    let is_reexport_all = record.meta.contains(ImportRecordMeta::IsExportStar);
    match self.input.module_wrappers.declaration(importee.idx) {
      WrapperDeclaration::None => {
        if is_reexport_all && self.input.dynamic_exports.contains(importee.idx) {
          stmt_info.eval_flags = true.into();
          stmt_info.meta.insert(rolldown_common::StmtInfoMeta::ReExportDynamicExports);
          self.runtime_requirements.push(RuntimeHelper::ReExport, stmt_info_idx);
          stmt_info.referenced_symbols.push(self.importer.namespace_object_ref.into());
          stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
        }
      }
      WrapperDeclaration::Cjs { wrapper_ref, .. } => {
        stmt_info.eval_flags = if is_reexport_all {
          true.into()
        } else {
          self.input.module_side_effects.get(importee.idx).has_side_effects().into()
        };
        stmt_info.referenced_symbols.push(wrapper_ref.into());
        if is_reexport_all {
          self.runtime_requirements.push(RuntimeHelper::ToEsm, stmt_info_idx);
          self.runtime_requirements.push(RuntimeHelper::ReExport, stmt_info_idx);
          if !self.input.tree_shaking.commonjs_treeshake {
            stmt_info.referenced_symbols.push(self.importer.namespace_object_ref.into());
          }
        } else {
          let needs_to_esm = self
            .input
            .cjs_namespace_merges
            .needs_interop(importee.idx)
            .unwrap_or_else(|| import_record_needs_interop(self.importer, record_idx));
          if needs_to_esm {
            self.runtime_requirements.push(RuntimeHelper::ToEsm, stmt_info_idx);
          }
          self.symbols_to_be_declared.push((record.namespace_ref, stmt_info_idx));
          self
            .symbol_db
            .ast_scopes
            .set_symbol_name(record.namespace_ref.symbol, &import_symbol_name(&importee.repr_name));
        }
      }
      WrapperDeclaration::Esm { wrapper_ref, .. } => {
        stmt_info.eval_flags = (is_reexport_all
          || self.input.module_side_effects.get(importee.idx).has_side_effects())
        .into();
        stmt_info.referenced_symbols.push(wrapper_ref.into());
        if is_reexport_all && self.input.dynamic_exports.contains(importee.idx) {
          self.runtime_requirements.push(RuntimeHelper::ReExport, stmt_info_idx);
          stmt_info.meta.insert(rolldown_common::StmtInfoMeta::ReExportDynamicExports);
          stmt_info.referenced_symbols.push(self.importer.namespace_object_ref.into());
          stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
        }
      }
    }
  }

  fn reference_normal_require(
    &mut self,
    importee: &NormalModule,
    record: &ResolvedImportRecord,
    stmt_info_idx: StmtInfoIdx,
    stmt_info: &mut StmtInfo,
  ) {
    match self.input.module_wrappers.declaration(importee.idx) {
      WrapperDeclaration::None => {}
      WrapperDeclaration::Cjs { wrapper_ref, .. } => {
        stmt_info.referenced_symbols.push(wrapper_ref.into());
      }
      WrapperDeclaration::Esm { wrapper_ref, .. } => {
        stmt_info.referenced_symbols.push(wrapper_ref.into());
        stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
        if !record.meta.contains(ImportRecordMeta::IsRequireUnused) {
          self.runtime_requirements.push(RuntimeHelper::ToCommonJs, stmt_info_idx);
        }
      }
    }
  }

  fn reference_normal_dynamic_import(
    &mut self,
    importee: &NormalModule,
    stmt_info_idx: StmtInfoIdx,
    stmt_info: &mut StmtInfo,
  ) {
    if self.input.chunking.code_splitting_disabled {
      match self.input.module_wrappers.declaration(importee.idx) {
        WrapperDeclaration::None => {}
        WrapperDeclaration::Cjs { wrapper_ref, .. } => {
          stmt_info.referenced_symbols.push(wrapper_ref.into());
          self.runtime_requirements.push(RuntimeHelper::ToEsm, stmt_info_idx);
        }
        WrapperDeclaration::Esm { wrapper_ref, .. } => {
          stmt_info.referenced_symbols.push(wrapper_ref.into());
          stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
        }
      }
    } else if self.input.module_formats.get(importee.idx) == Some(ExportsKind::CommonJs) {
      self.runtime_requirements.push(RuntimeHelper::ToEsm, stmt_info_idx);
    }
  }
}

impl Pass for ReferenceNeededSymbolsPass {
  type InputRead<'a> = ReferenceNeededSymbolsInput<'a>;
  type InputOwned = ReferenceNeededSymbolsOwned;
  type OutputRead = StatementRuntimeRequirements;
  type OutputOwned = ReferenceNeededSymbolsOutput;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let ReferenceNeededSymbolsOwned { symbols, mut stmt_infos } = owned;
    assert_reference_layout(input, &symbols, &stmt_infos);

    let module_count = input.module_table.modules.len();
    let has_module_preserve_jsx = symbols.has_module_preserve_jsx();
    let mut symbols_inner = symbols.into_inner();
    let mut runtime_requirements = input
      .module_table
      .modules
      .iter()
      .map(|_| Box::default())
      .collect::<IndexVec<ModuleIdx, Box<DependedRuntimeHelperMap>>>();

    // Preserve the legacy physical four-way indexed traversal. The dense per-slot batches make
    // physical module order explicit while each statement and import-record vector retains its
    // original encounter order.
    let import_record_patches = input
      .module_table
      .modules
      .par_iter()
      .zip(symbols_inner.par_iter_mut())
      .zip(runtime_requirements.par_iter_mut())
      .zip(stmt_infos.par_iter_mut())
      .map(|(((module, symbol_slot), runtime_requirements), stmt_infos)| {
        let Module::Normal(importer) = module else {
          return ModuleImportRecordPatches { events: Vec::new() };
        };
        let Some(symbol_db) = symbol_slot.as_mut() else {
          std::unreachable!("validated normal modules must have owner-local symbol databases");
        };
        let mut analysis = ModuleReferenceAnalysis {
          input,
          importer,
          symbol_db,
          runtime_requirements,
          patch_events: Vec::new(),
          symbols_to_be_declared: Vec::new(),
        };
        for (stmt_info_idx, stmt_info) in stmt_infos.infos.iter_mut_enumerated() {
          analysis.reference_statement(stmt_info_idx, stmt_info);
        }
        let patch_events = std::mem::take(&mut analysis.patch_events);
        let symbols_to_be_declared = std::mem::take(&mut analysis.symbols_to_be_declared);
        drop(analysis);
        for (symbol_ref, stmt_info_idx) in symbols_to_be_declared {
          stmt_infos.declare_symbol_for_stmt(stmt_info_idx, TaggedSymbolRef::normal(symbol_ref));
        }
        ModuleImportRecordPatches { events: patch_events }
      })
      .collect::<Vec<_>>();

    std::assert_eq!(
      import_record_patches.len(),
      module_count,
      "reference patch batches must preserve physical module layout"
    );
    let mut symbols = SymbolRefDb::new().with_inner(symbols_inner);
    if has_module_preserve_jsx {
      symbols.set_has_module_preserve_jsx();
    }
    Ok(token.finish(
      StatementRuntimeRequirements { slots: runtime_requirements },
      ReferenceNeededSymbolsOutput {
        symbols,
        stmt_infos,
        import_record_patches: ReferenceImportRecordPatches { modules: import_record_patches },
      },
    ))
  }
}

#[cfg(test)]
mod tests {
  use oxc::{semantic::Scoping, span::Span};
  use oxc_index::IndexVec;
  use rolldown_common::{
    ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta, NamedImport, OutputFormat,
    Specifier, StmtInfo, StmtInfoIdx, StmtInfoMeta, StmtInfos, SymbolRefDb, SymbolRefDbForModule,
    WrapKind, side_effects::DeterminedSideEffects,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::{
    ReferenceChunkingOptions, ReferenceNeededSymbolsInput, ReferenceNeededSymbolsOwned,
    ReferenceNeededSymbolsPass, ReferenceTreeShakingOptions,
    compute_cjs_namespace_merges::test_support::cjs_namespace_merges,
    compute_dynamic_exports::test_support::dynamic_exports,
    create_wrapper_declarations::test_support::module_wrappers,
    determine_module_formats::test_support::module_formats,
    determine_module_side_effects::test_support::module_side_effects,
    test_utils::{external_module, module_idx, module_table, normal_module},
  };

  const DEFAULT_CHUNKING: ReferenceChunkingOptions =
    ReferenceChunkingOptions { dynamic_import_in_cjs: true, code_splitting_disabled: false };
  const DEFAULT_TREE_SHAKING: ReferenceTreeShakingOptions =
    ReferenceTreeShakingOptions { keep_names: false, commonjs_treeshake: false };

  fn symbols_for(modules: &rolldown_common::ModuleTable) -> SymbolRefDb {
    let mut symbols = SymbolRefDb::new();
    for (module_idx, module) in modules.modules.iter_enumerated() {
      let scoping = Scoping::default();
      let root_scope_id = scoping.root_scope_id();
      symbols
        .store_local_db(module_idx, SymbolRefDbForModule::new(scoping, module_idx, root_scope_id));
      let namespace_ref = module.as_normal().map_or_else(
        || module.as_external().expect("external module").namespace_ref,
        |module| module.namespace_object_ref,
      );
      assert_eq!(symbols.create_facade_root_symbol_ref(module_idx, "namespace"), namespace_ref);
    }
    symbols
  }

  fn helper_statements(
    requirements: &super::StatementRuntimeRequirements,
    module: usize,
    helper: rolldown_common::RuntimeHelper,
  ) -> Vec<StmtInfoIdx> {
    requirements.slots()[module_idx(module)]
      .iter()
      .find_map(|(candidate, statements)| (candidate == helper).then(|| statements.clone()))
      .expect("defined runtime helper")
  }

  #[test]
  fn rejects_dense_statement_layout_mismatch_before_mutation() {
    let modules = module_table(vec![normal_module(0, false, Vec::new())]);
    let symbols = symbols_for(&modules);
    let formats = module_formats(&[Some(ExportsKind::Esm)]);
    let wrappers = module_wrappers(&[WrapKind::None]);
    let dynamic_exports = dynamic_exports(1, []);
    let side_effects = module_side_effects(&[DeterminedSideEffects::Analyzed(false)]);
    let cjs_merges = cjs_namespace_merges(1, []);
    let mut pipeline = PassPipelineCtx::new();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      run_infallible_pass(
        ReferenceNeededSymbolsPass,
        &mut pipeline,
        ReferenceNeededSymbolsInput {
          module_table: &modules,
          module_formats: &formats,
          module_wrappers: &wrappers,
          dynamic_exports: &dynamic_exports,
          module_side_effects: &side_effects,
          cjs_namespace_merges: &cjs_merges,
          runtime_require_ref: None,
          output_format: OutputFormat::Esm,
          chunking: DEFAULT_CHUNKING,
          tree_shaking: DEFAULT_TREE_SHAKING,
        },
        ReferenceNeededSymbolsOwned { symbols, stmt_infos: IndexVec::new() },
      )
    }));
    assert!(result.is_err());
  }

  #[test]
  fn rejects_missing_owner_local_symbol_slot_for_an_external_module() {
    let modules = module_table(vec![external_module(0, "external")]);
    let symbols = SymbolRefDb::new().with_inner(IndexVec::from_vec(vec![None]));
    let formats = module_formats(&[None]);
    let wrappers = module_wrappers(&[WrapKind::None]);
    let dynamic_exports = dynamic_exports(1, []);
    let side_effects = module_side_effects(&[DeterminedSideEffects::Analyzed(false)]);
    let cjs_merges = cjs_namespace_merges(1, []);
    let statements = IndexVec::from_vec(vec![StmtInfos::new()]);
    let mut pipeline = PassPipelineCtx::new();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      run_infallible_pass(
        ReferenceNeededSymbolsPass,
        &mut pipeline,
        ReferenceNeededSymbolsInput {
          module_table: &modules,
          module_formats: &formats,
          module_wrappers: &wrappers,
          dynamic_exports: &dynamic_exports,
          module_side_effects: &side_effects,
          cjs_namespace_merges: &cjs_merges,
          runtime_require_ref: None,
          output_format: OutputFormat::Esm,
          chunking: DEFAULT_CHUNKING,
          tree_shaking: DEFAULT_TREE_SHAKING,
        },
        ReferenceNeededSymbolsOwned { symbols, stmt_infos: statements },
      )
    }));
    assert!(result.is_err());
  }

  #[test]
  fn rejects_embedded_normal_module_index_mismatch() {
    let mut modules = module_table(vec![normal_module(0, false, Vec::new())]);
    let symbols = symbols_for(&modules);
    modules[module_idx(0)].as_normal_mut().unwrap().idx = module_idx(1);
    let formats = module_formats(&[Some(ExportsKind::Esm)]);
    let wrappers = module_wrappers(&[WrapKind::None]);
    let dynamic_exports = dynamic_exports(1, []);
    let side_effects = module_side_effects(&[DeterminedSideEffects::Analyzed(false)]);
    let cjs_merges = cjs_namespace_merges(1, []);
    let statements = IndexVec::from_vec(vec![StmtInfos::new()]);
    let mut pipeline = PassPipelineCtx::new();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      run_infallible_pass(
        ReferenceNeededSymbolsPass,
        &mut pipeline,
        ReferenceNeededSymbolsInput {
          module_table: &modules,
          module_formats: &formats,
          module_wrappers: &wrappers,
          dynamic_exports: &dynamic_exports,
          module_side_effects: &side_effects,
          cjs_namespace_merges: &cjs_merges,
          runtime_require_ref: None,
          output_format: OutputFormat::Esm,
          chunking: DEFAULT_CHUNKING,
          tree_shaking: DEFAULT_TREE_SHAKING,
        },
        ReferenceNeededSymbolsOwned { symbols, stmt_infos: statements },
      )
    }));
    assert!(result.is_err());
  }

  #[test]
  fn preserves_duplicate_patch_order_and_applies_only_call_runtime_require_at_the_adapter() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Require, Some(2), Span::new(1, 2)),
          (ImportKind::Require, Some(2), Span::new(2, 3)),
        ],
      ),
      normal_module(1, false, vec![(ImportKind::Require, Some(2), Span::new(3, 4))]),
      external_module(2, "external"),
    ]);
    let mut symbols = symbols_for(&modules);
    let runtime_require_ref = symbols.create_facade_root_symbol_ref(module_idx(0), "require");
    let mut statements =
      modules.modules.iter().map(|_| StmtInfos::new()).collect::<IndexVec<_, _>>();
    statements[module_idx(0)].add_stmt_info(StmtInfo {
      import_records: vec![
        ImportRecordIdx::from_usize(1),
        ImportRecordIdx::from_usize(0),
        ImportRecordIdx::from_usize(1),
      ],
      ..Default::default()
    });
    statements[module_idx(1)].add_stmt_info(StmtInfo {
      import_records: vec![ImportRecordIdx::from_usize(0)],
      ..Default::default()
    });
    let formats = module_formats(&[Some(ExportsKind::Esm), Some(ExportsKind::Esm), None]);
    let wrappers = module_wrappers(&[WrapKind::None, WrapKind::None, WrapKind::None]);
    let dynamic_exports = dynamic_exports(3, []);
    let side_effects = module_side_effects(&[DeterminedSideEffects::Analyzed(false); 3]);
    let cjs_merges = cjs_namespace_merges(3, []);
    let mut pipeline = PassPipelineCtx::new();

    let (requirements, output) = run_infallible_pass(
      ReferenceNeededSymbolsPass,
      &mut pipeline,
      ReferenceNeededSymbolsInput {
        module_table: &modules,
        module_formats: &formats,
        module_wrappers: &wrappers,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &side_effects,
        cjs_namespace_merges: &cjs_merges,
        runtime_require_ref: Some(runtime_require_ref),
        output_format: OutputFormat::Esm,
        chunking: DEFAULT_CHUNKING,
        tree_shaking: DEFAULT_TREE_SHAKING,
      },
      ReferenceNeededSymbolsOwned { symbols, stmt_infos: statements },
    );

    let ordered_events = output
      .import_record_patches
      .modules
      .iter()
      .flat_map(|batch| batch.events.iter())
      .map(|event| (event.importer, event.import_record))
      .collect::<Vec<_>>();
    assert_eq!(
      ordered_events,
      [
        (module_idx(0), ImportRecordIdx::from_usize(1)),
        (module_idx(0), ImportRecordIdx::from_usize(0)),
        (module_idx(0), ImportRecordIdx::from_usize(1)),
        (module_idx(1), ImportRecordIdx::from_usize(0)),
      ]
    );
    for module_index in 0..2 {
      let module = modules[module_idx(module_index)].as_normal().unwrap();
      assert!(
        module
          .import_records
          .iter()
          .all(|record| { !record.meta.contains(ImportRecordMeta::CallRuntimeRequire) })
      );
    }
    assert_eq!(
      output.stmt_infos[module_idx(0)].get(StmtInfoIdx::from_usize(1)).referenced_symbols,
      vec![runtime_require_ref.into(), runtime_require_ref.into(), runtime_require_ref.into()]
    );
    assert!(
      helper_statements(&requirements, 0, rolldown_common::RuntimeHelper::Require).is_empty()
    );
    modules[module_idx(0)].as_normal_mut().unwrap().import_records[ImportRecordIdx::from_usize(1)]
      .meta
      .insert(ImportRecordMeta::DeadDynamicImport);
    output.import_record_patches.apply(&mut modules);
    for module_index in 0..2 {
      let module = modules[module_idx(module_index)].as_normal().unwrap();
      assert!(
        module
          .import_records
          .iter()
          .all(|record| { record.meta.contains(ImportRecordMeta::CallRuntimeRequire) })
      );
    }
    assert!(
      modules[module_idx(0)].as_normal().unwrap().import_records[ImportRecordIdx::from_usize(1)]
        .meta
        .contains(ImportRecordMeta::DeadDynamicImport)
    );
    assert!(pipeline.into_diagnostics().is_empty());
  }

  #[test]
  fn merge_false_overrides_record_interop_and_statement_flags_keep_exact_helper_indices() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(1, 2))]),
      normal_module(1, false, Vec::new()),
      external_module(2, "external"),
    ]);
    let mut symbols = symbols_for(&modules);
    symbols.set_has_module_preserve_jsx();
    let namespace_ref = modules[module_idx(0)].as_normal().unwrap().import_records
      [ImportRecordIdx::from_usize(0)]
    .namespace_ref;
    modules[module_idx(0)].as_normal_mut().unwrap().named_imports.insert(
      namespace_ref,
      NamedImport {
        imported: Specifier::Star,
        span_imported: Span::new(1, 2),
        imported_as: namespace_ref,
        record_idx: ImportRecordIdx::from_usize(0),
      },
    );
    let mut statements =
      modules.modules.iter().map(|_| StmtInfos::new()).collect::<IndexVec<_, _>>();
    let import_stmt = statements[module_idx(0)].add_stmt_info(StmtInfo {
      import_records: vec![ImportRecordIdx::from_usize(0)],
      ..Default::default()
    });
    let flag_stmt = statements[module_idx(0)].add_stmt_info(StmtInfo {
      meta: StmtInfoMeta::HasDummyRecord
        | StmtInfoMeta::NonStaticDynamicImport
        | StmtInfoMeta::KeepNamesType,
      ..Default::default()
    });
    let formats = module_formats(&[Some(ExportsKind::Esm), Some(ExportsKind::CommonJs), None]);
    let wrappers = module_wrappers(&[WrapKind::None, WrapKind::Cjs, WrapKind::None]);
    let dynamic_exports = dynamic_exports(3, []);
    let side_effects = module_side_effects(&[DeterminedSideEffects::Analyzed(false); 3]);
    let cjs_merges = cjs_namespace_merges(3, [(module_idx(1), false)]);
    let mut pipeline = PassPipelineCtx::new();

    let (requirements, output) = run_infallible_pass(
      ReferenceNeededSymbolsPass,
      &mut pipeline,
      ReferenceNeededSymbolsInput {
        module_table: &modules,
        module_formats: &formats,
        module_wrappers: &wrappers,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &side_effects,
        cjs_namespace_merges: &cjs_merges,
        runtime_require_ref: None,
        output_format: OutputFormat::Esm,
        chunking: DEFAULT_CHUNKING,
        tree_shaking: ReferenceTreeShakingOptions { keep_names: true, commonjs_treeshake: false },
      },
      ReferenceNeededSymbolsOwned { symbols, stmt_infos: statements },
    );

    assert_eq!(
      helper_statements(&requirements, 0, rolldown_common::RuntimeHelper::ToEsm),
      [flag_stmt]
    );
    assert_eq!(
      helper_statements(&requirements, 0, rolldown_common::RuntimeHelper::Require),
      [flag_stmt]
    );
    assert_eq!(
      helper_statements(&requirements, 0, rolldown_common::RuntimeHelper::Name),
      [flag_stmt]
    );
    let import_statement = output.stmt_infos[module_idx(0)].get(import_stmt);
    assert!(import_statement.eval_flags.is_empty());
    assert_eq!(import_statement.referenced_symbols.len(), 1);
    assert_eq!(
      output.stmt_infos[module_idx(0)].declared_stmts_by_symbol(&namespace_ref),
      [import_stmt]
    );
    assert!(output.symbols.has_module_preserve_jsx());
    assert!(pipeline.into_diagnostics().is_empty());
  }

  #[test]
  fn normal_imports_preserve_wrapper_dynamic_side_effect_and_treeshake_branches() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(1, 2)),
          (ImportKind::Import, Some(2), Span::new(2, 3)),
          (ImportKind::Import, Some(2), Span::new(3, 4)),
          (ImportKind::Import, Some(3), Span::new(4, 5)),
          (ImportKind::Import, Some(3), Span::new(5, 6)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
      normal_module(3, false, Vec::new()),
      external_module(4, "external"),
    ]);
    let mut symbols = symbols_for(&modules);
    let mut record_namespaces = Vec::new();
    for record_index in 0..5 {
      let namespace =
        symbols.create_facade_root_symbol_ref(module_idx(0), &format!("record_{record_index}"));
      modules[module_idx(0)].as_normal_mut().unwrap().import_records
        [ImportRecordIdx::from_usize(record_index)]
      .namespace_ref = namespace;
      record_namespaces.push(namespace);
    }
    for record_index in [0, 1, 3] {
      modules[module_idx(0)].as_normal_mut().unwrap().import_records
        [ImportRecordIdx::from_usize(record_index)]
      .meta
      .insert(ImportRecordMeta::IsExportStar);
    }
    modules[module_idx(0)].as_normal_mut().unwrap().named_imports.insert(
      record_namespaces[2],
      NamedImport {
        imported: Specifier::Star,
        span_imported: Span::new(3, 4),
        imported_as: record_namespaces[2],
        record_idx: ImportRecordIdx::from_usize(2),
      },
    );
    let mut statements =
      modules.modules.iter().map(|_| StmtInfos::new()).collect::<IndexVec<_, _>>();
    let statement_indices = (0..5)
      .map(|record_index| {
        statements[module_idx(0)].add_stmt_info(StmtInfo {
          import_records: vec![ImportRecordIdx::from_usize(record_index)],
          ..Default::default()
        })
      })
      .collect::<Vec<_>>();
    let formats = module_formats(&[
      Some(ExportsKind::Esm),
      Some(ExportsKind::Esm),
      Some(ExportsKind::CommonJs),
      Some(ExportsKind::Esm),
      None,
    ]);
    let wrappers = module_wrappers(&[
      WrapKind::None,
      WrapKind::None,
      WrapKind::Cjs,
      WrapKind::Esm,
      WrapKind::None,
    ]);
    let dynamic_exports = dynamic_exports(5, [module_idx(1), module_idx(3)]);
    let side_effects = module_side_effects(&[
      DeterminedSideEffects::Analyzed(false),
      DeterminedSideEffects::Analyzed(false),
      DeterminedSideEffects::Analyzed(true),
      DeterminedSideEffects::Analyzed(true),
      DeterminedSideEffects::Analyzed(false),
    ]);
    let cjs_merges = cjs_namespace_merges(5, []);
    let mut pipeline = PassPipelineCtx::new();
    let (requirements, output) = run_infallible_pass(
      ReferenceNeededSymbolsPass,
      &mut pipeline,
      ReferenceNeededSymbolsInput {
        module_table: &modules,
        module_formats: &formats,
        module_wrappers: &wrappers,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &side_effects,
        cjs_namespace_merges: &cjs_merges,
        runtime_require_ref: None,
        output_format: OutputFormat::Esm,
        chunking: DEFAULT_CHUNKING,
        tree_shaking: DEFAULT_TREE_SHAKING,
      },
      ReferenceNeededSymbolsOwned { symbols, stmt_infos: statements },
    );

    assert_eq!(
      helper_statements(&requirements, 0, rolldown_common::RuntimeHelper::ReExport),
      [statement_indices[0], statement_indices[1], statement_indices[3]]
    );
    assert_eq!(
      helper_statements(&requirements, 0, rolldown_common::RuntimeHelper::ToEsm),
      [statement_indices[1], statement_indices[2]]
    );
    let importer_namespace = modules[module_idx(0)].as_normal().unwrap().namespace_object_ref;
    let none_star = output.stmt_infos[module_idx(0)].get(statement_indices[0]);
    assert!(none_star.eval_flags.has_side_effect_for_tree_shaking());
    assert!(none_star.meta.contains(StmtInfoMeta::ReExportDynamicExports));
    assert_eq!(none_star.referenced_symbols.len(), 2);
    let cjs_star = output.stmt_infos[module_idx(0)].get(statement_indices[1]);
    assert_eq!(cjs_star.referenced_symbols.len(), 2);
    assert_eq!(
      cjs_star.referenced_symbols[1],
      rolldown_common::SymbolOrMemberExprRef::Symbol(importer_namespace)
    );
    let cjs_import = output.stmt_infos[module_idx(0)].get(statement_indices[2]);
    assert!(cjs_import.eval_flags.has_side_effect_for_tree_shaking());
    assert_eq!(cjs_import.referenced_symbols.len(), 1);
    assert_eq!(
      output.stmt_infos[module_idx(0)].declared_stmts_by_symbol(&record_namespaces[2]),
      [statement_indices[2]]
    );
    let esm_star = output.stmt_infos[module_idx(0)].get(statement_indices[3]);
    assert!(esm_star.meta.contains(StmtInfoMeta::ReExportDynamicExports));
    assert_eq!(esm_star.referenced_symbols.len(), 3);
    let esm_import = output.stmt_infos[module_idx(0)].get(statement_indices[4]);
    assert!(esm_import.eval_flags.has_side_effect_for_tree_shaking());
    assert_eq!(esm_import.referenced_symbols.len(), 1);

    let treeshaken_statements =
      modules.modules.iter().map(|_| StmtInfos::new()).collect::<IndexVec<_, _>>();
    let mut treeshaken_statements = treeshaken_statements;
    for record_index in 0..5 {
      treeshaken_statements[module_idx(0)].add_stmt_info(StmtInfo {
        import_records: vec![ImportRecordIdx::from_usize(record_index)],
        ..Default::default()
      });
    }
    let mut treeshaken_symbols = symbols_for(&modules);
    for (record_index, expected) in record_namespaces.iter().enumerate() {
      assert_eq!(
        treeshaken_symbols
          .create_facade_root_symbol_ref(module_idx(0), &format!("record_{record_index}")),
        *expected
      );
    }
    let (_, treeshaken_output) = run_infallible_pass(
      ReferenceNeededSymbolsPass,
      &mut pipeline,
      ReferenceNeededSymbolsInput {
        module_table: &modules,
        module_formats: &formats,
        module_wrappers: &wrappers,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &side_effects,
        cjs_namespace_merges: &cjs_merges,
        runtime_require_ref: None,
        output_format: OutputFormat::Esm,
        chunking: DEFAULT_CHUNKING,
        tree_shaking: ReferenceTreeShakingOptions { keep_names: false, commonjs_treeshake: true },
      },
      ReferenceNeededSymbolsOwned {
        symbols: treeshaken_symbols,
        stmt_infos: treeshaken_statements,
      },
    );
    assert_eq!(
      treeshaken_output.stmt_infos[module_idx(0)]
        .get(StmtInfoIdx::from_usize(2))
        .referenced_symbols
        .len(),
      1
    );
    assert!(pipeline.into_diagnostics().is_empty());
  }

  #[test]
  fn require_and_dynamic_import_use_wrapper_kinds_and_final_formats_without_cross_talk() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Require, Some(1), Span::new(1, 2)),
          (ImportKind::Require, Some(2), Span::new(2, 3)),
          (ImportKind::Require, Some(3), Span::new(3, 4)),
          (ImportKind::Require, Some(3), Span::new(4, 5)),
          (ImportKind::DynamicImport, Some(1), Span::new(5, 6)),
          (ImportKind::DynamicImport, Some(2), Span::new(6, 7)),
          (ImportKind::DynamicImport, Some(3), Span::new(7, 8)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
      normal_module(3, false, Vec::new()),
      external_module(4, "external"),
    ]);
    modules[module_idx(0)].as_normal_mut().unwrap().import_records[ImportRecordIdx::from_usize(3)]
      .meta
      .insert(ImportRecordMeta::IsRequireUnused);
    let formats = module_formats(&[
      Some(ExportsKind::Esm),
      Some(ExportsKind::Esm),
      Some(ExportsKind::CommonJs),
      Some(ExportsKind::Esm),
      None,
    ]);
    let wrappers = module_wrappers(&[
      WrapKind::None,
      WrapKind::None,
      WrapKind::Cjs,
      WrapKind::Esm,
      WrapKind::None,
    ]);
    let dynamic_exports = dynamic_exports(5, []);
    let side_effects = module_side_effects(&[DeterminedSideEffects::Analyzed(false); 5]);
    let cjs_merges = cjs_namespace_merges(5, []);
    let make_statements = || {
      let mut statements =
        modules.modules.iter().map(|_| StmtInfos::new()).collect::<IndexVec<_, _>>();
      let indices = (0..7)
        .map(|record_index| {
          statements[module_idx(0)].add_stmt_info(StmtInfo {
            import_records: vec![ImportRecordIdx::from_usize(record_index)],
            ..Default::default()
          })
        })
        .collect::<Vec<_>>();
      (statements, indices)
    };
    let (statements, indices) = make_statements();
    let mut pipeline = PassPipelineCtx::new();
    let (requirements, output) = run_infallible_pass(
      ReferenceNeededSymbolsPass,
      &mut pipeline,
      ReferenceNeededSymbolsInput {
        module_table: &modules,
        module_formats: &formats,
        module_wrappers: &wrappers,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &side_effects,
        cjs_namespace_merges: &cjs_merges,
        runtime_require_ref: None,
        output_format: OutputFormat::Esm,
        chunking: ReferenceChunkingOptions {
          dynamic_import_in_cjs: true,
          code_splitting_disabled: true,
        },
        tree_shaking: DEFAULT_TREE_SHAKING,
      },
      ReferenceNeededSymbolsOwned { symbols: symbols_for(&modules), stmt_infos: statements },
    );
    assert_eq!(
      helper_statements(&requirements, 0, rolldown_common::RuntimeHelper::ToCommonJs),
      [indices[2]]
    );
    assert_eq!(
      helper_statements(&requirements, 0, rolldown_common::RuntimeHelper::ToEsm),
      [indices[5]]
    );
    let reference_lengths = indices
      .iter()
      .map(|index| output.stmt_infos[module_idx(0)].get(*index).referenced_symbols.len())
      .collect::<Vec<_>>();
    assert_eq!(reference_lengths, [0, 1, 2, 2, 0, 1, 2]);

    let (statements, indices) = make_statements();
    let (requirements, output) = run_infallible_pass(
      ReferenceNeededSymbolsPass,
      &mut pipeline,
      ReferenceNeededSymbolsInput {
        module_table: &modules,
        module_formats: &formats,
        module_wrappers: &wrappers,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &side_effects,
        cjs_namespace_merges: &cjs_merges,
        runtime_require_ref: None,
        output_format: OutputFormat::Esm,
        chunking: DEFAULT_CHUNKING,
        tree_shaking: DEFAULT_TREE_SHAKING,
      },
      ReferenceNeededSymbolsOwned { symbols: symbols_for(&modules), stmt_infos: statements },
    );
    assert_eq!(
      helper_statements(&requirements, 0, rolldown_common::RuntimeHelper::ToEsm),
      [indices[5]]
    );
    assert_eq!(output.stmt_infos[module_idx(0)].get(indices[5]).referenced_symbols.len(), 0);
    assert_eq!(output.stmt_infos[module_idx(0)].get(indices[6]).referenced_symbols.len(), 0);
    assert!(pipeline.into_diagnostics().is_empty());
  }

  #[test]
  fn external_records_preserve_star_rename_cjs_interop_dynamic_option_and_unresolved_noop() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(1, 2)),
          (ImportKind::Import, Some(1), Span::new(2, 3)),
          (ImportKind::DynamicImport, Some(1), Span::new(3, 4)),
          (ImportKind::Import, None, Span::new(4, 5)),
        ],
      ),
      external_module(1, "external"),
    ]);
    let mut symbols = symbols_for(&modules);
    let mut record_namespaces = Vec::new();
    for record_index in 0..4 {
      let namespace =
        symbols.create_facade_root_symbol_ref(module_idx(0), &format!("external_{record_index}"));
      modules[module_idx(0)].as_normal_mut().unwrap().import_records
        [ImportRecordIdx::from_usize(record_index)]
      .namespace_ref = namespace;
      record_namespaces.push(namespace);
    }
    modules[module_idx(0)].as_normal_mut().unwrap().import_records[ImportRecordIdx::from_usize(0)]
      .meta
      .insert(ImportRecordMeta::IsExportStar);
    modules[module_idx(0)].as_normal_mut().unwrap().named_imports.insert(
      record_namespaces[1],
      NamedImport {
        imported: Specifier::Star,
        span_imported: Span::new(2, 3),
        imported_as: record_namespaces[1],
        record_idx: ImportRecordIdx::from_usize(1),
      },
    );
    let make_statements = || {
      let mut statements =
        modules.modules.iter().map(|_| StmtInfos::new()).collect::<IndexVec<_, _>>();
      let indices = (0..4)
        .map(|record_index| {
          statements[module_idx(0)].add_stmt_info(StmtInfo {
            import_records: vec![ImportRecordIdx::from_usize(record_index)],
            ..Default::default()
          })
        })
        .collect::<Vec<_>>();
      (statements, indices)
    };
    let formats = module_formats(&[Some(ExportsKind::Esm), None]);
    let wrappers = module_wrappers(&[WrapKind::None, WrapKind::None]);
    let dynamic_exports = dynamic_exports(2, []);
    let side_effects = module_side_effects(&[DeterminedSideEffects::Analyzed(false); 2]);
    let cjs_merges = cjs_namespace_merges(2, []);
    let (statements, indices) = make_statements();
    let mut pipeline = PassPipelineCtx::new();
    let (requirements, output) = run_infallible_pass(
      ReferenceNeededSymbolsPass,
      &mut pipeline,
      ReferenceNeededSymbolsInput {
        module_table: &modules,
        module_formats: &formats,
        module_wrappers: &wrappers,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &side_effects,
        cjs_namespace_merges: &cjs_merges,
        runtime_require_ref: None,
        output_format: OutputFormat::Cjs,
        chunking: ReferenceChunkingOptions {
          dynamic_import_in_cjs: false,
          code_splitting_disabled: false,
        },
        tree_shaking: DEFAULT_TREE_SHAKING,
      },
      ReferenceNeededSymbolsOwned { symbols, stmt_infos: statements },
    );
    assert_eq!(output.symbols.original_name(record_namespaces[0]).as_str(), "import_external");
    assert_eq!(
      helper_statements(&requirements, 0, rolldown_common::RuntimeHelper::ToEsm),
      [indices[1], indices[2]]
    );
    assert!(
      !output.stmt_infos[module_idx(0)]
        .get(indices[0])
        .eval_flags
        .has_side_effect_for_tree_shaking()
    );
    assert!(
      output.stmt_infos[module_idx(0)]
        .get(indices[1])
        .eval_flags
        .has_side_effect_for_tree_shaking()
    );
    assert!(output.stmt_infos[module_idx(0)].get(indices[3]).referenced_symbols.is_empty());

    let mut second_symbols = symbols_for(&modules);
    for (record_index, expected) in record_namespaces.iter().enumerate() {
      assert_eq!(
        second_symbols
          .create_facade_root_symbol_ref(module_idx(0), &format!("external_{record_index}")),
        *expected
      );
    }
    let (statements, indices) = make_statements();
    let (requirements, _) = run_infallible_pass(
      ReferenceNeededSymbolsPass,
      &mut pipeline,
      ReferenceNeededSymbolsInput {
        module_table: &modules,
        module_formats: &formats,
        module_wrappers: &wrappers,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &side_effects,
        cjs_namespace_merges: &cjs_merges,
        runtime_require_ref: None,
        output_format: OutputFormat::Cjs,
        chunking: DEFAULT_CHUNKING,
        tree_shaking: DEFAULT_TREE_SHAKING,
      },
      ReferenceNeededSymbolsOwned { symbols: second_symbols, stmt_infos: statements },
    );
    assert_eq!(
      helper_statements(&requirements, 0, rolldown_common::RuntimeHelper::ToEsm),
      [indices[1]]
    );
    assert!(pipeline.into_diagnostics().is_empty());
  }
}
