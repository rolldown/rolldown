use std::convert::Infallible;

use rolldown_common::{ExportsKind, ImportKind, ImportRecordMeta, Module, ModuleIdx, ModuleTable};
use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};
use rustc_hash::FxHashMap;

use crate::utils::external_import_interop::import_record_needs_interop;

use super::{ComputeCjsNamespaceMergesPass, determine_module_formats::ModuleFormatsDraft};
use crate::stages::link_stage::SafelyMergeCjsNsInfo;

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct ComputeCjsNamespaceMergesInput<'a> {
  pub module_table: &'a ModuleTable,
  pub module_formats: &'a ModuleFormatsDraft,
  pub strict_execution_order: bool,
}

pub(in crate::stages::link_stage) struct CjsNamespaceMerges {
  modules: FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo>,
}

impl CjsNamespaceMerges {
  pub(in crate::stages::link_stage) fn into_legacy(
    self,
  ) -> FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo> {
    self.modules
  }
}

impl Pass for ComputeCjsNamespaceMergesPass {
  type InputRead<'a> = ComputeCjsNamespaceMergesInput<'a>;
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = CjsNamespaceMerges;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let ComputeCjsNamespaceMergesInput { module_table, module_formats, strict_execution_order } =
      input;
    let mut modules = FxHashMap::<ModuleIdx, SafelyMergeCjsNsInfo>::default();
    if !strict_execution_order {
      for importer in module_table.modules.iter().filter_map(Module::as_normal) {
        for (record_idx, record) in importer.import_records.iter_enumerated() {
          if record.kind != ImportKind::Import
            || record.meta.contains(ImportRecordMeta::IsExportStar)
          {
            continue;
          }
          let Some(importee_idx) = record.resolved_module else { continue };
          if module_formats.get(importee_idx) != Some(ExportsKind::CommonJs) {
            continue;
          }

          let info = modules.entry(importee_idx).or_default();
          info.namespace_refs.push(record.namespace_ref);
          info.needs_interop |= import_record_needs_interop(importer, record_idx);
        }
      }
    }

    Ok(token.finish((), CjsNamespaceMerges { modules }))
  }
}

#[cfg(test)]
mod tests {
  use oxc::{semantic::SymbolId, span::Span};
  use rolldown_common::{
    EntryPointKind, ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleTable,
    NamedImport, OutputFormat, Specifier, SymbolRef,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::{
    CanonicalizeEntriesPass, DetermineModuleFormatsInput, DetermineModuleFormatsPass,
    EntryPlanDraft,
    test_utils::{entry_point, module_idx, module_table, normal_module},
  };
  use super::{ComputeCjsNamespaceMergesInput, ComputeCjsNamespaceMergesPass};

  fn entry_plan(modules: &ModuleTable) -> EntryPlanDraft {
    let mut pipeline = PassPipelineCtx::new();
    let (_, entry_plan) = run_infallible_pass(
      CanonicalizeEntriesPass,
      &mut pipeline,
      modules,
      vec![entry_point(0, EntryPointKind::UserDefined)],
    );
    assert!(pipeline.into_diagnostics().is_empty());
    entry_plan
  }

  fn compute(
    modules: &ModuleTable,
    strict_execution_order: bool,
  ) -> rustc_hash::FxHashMap<
    rolldown_common::ModuleIdx,
    crate::stages::link_stage::SafelyMergeCjsNsInfo,
  > {
    let entry_plan = entry_plan(modules);
    let mut pipeline = PassPipelineCtx::new();
    let (_, (formats, _)) = run_infallible_pass(
      DetermineModuleFormatsPass,
      &mut pipeline,
      DetermineModuleFormatsInput {
        module_table: modules,
        entry_plan: &entry_plan,
        output_format: OutputFormat::Esm,
        code_splitting_disabled: false,
      },
      (),
    );
    let (_, merges) = run_infallible_pass(
      ComputeCjsNamespaceMergesPass,
      &mut pipeline,
      ComputeCjsNamespaceMergesInput {
        module_table: modules,
        module_formats: &formats,
        strict_execution_order,
      },
      (),
    );
    assert!(pipeline.into_diagnostics().is_empty());
    merges.into_legacy()
  }

  #[test]
  fn groups_eligible_imports_in_module_and_record_order() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(2), Span::new(1, 2)),
          (ImportKind::Import, Some(2), Span::new(3, 4)),
        ],
      ),
      normal_module(1, false, vec![(ImportKind::Import, Some(2), Span::new(5, 6))]),
      normal_module(2, false, Vec::new()),
    ]);
    modules[module_idx(2)].as_normal_mut().expect("normal module").exports_kind =
      ExportsKind::CommonJs;
    let first_ref = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(1) };
    let skipped_ref = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(2) };
    let second_ref = SymbolRef { owner: module_idx(1), symbol: SymbolId::new(1) };
    let importer = modules[module_idx(0)].as_normal_mut().expect("normal importer");
    importer.import_records[ImportRecordIdx::from_usize(0)].namespace_ref = first_ref;
    importer.import_records[ImportRecordIdx::from_usize(1)].namespace_ref = skipped_ref;
    importer.import_records[ImportRecordIdx::from_usize(1)]
      .meta
      .insert(ImportRecordMeta::IsExportStar);
    importer.named_imports.insert(
      first_ref,
      NamedImport {
        imported: Specifier::Star,
        span_imported: Span::new(1, 2),
        imported_as: first_ref,
        record_idx: ImportRecordIdx::from_usize(0),
      },
    );
    modules[module_idx(1)].as_normal_mut().expect("normal importer").import_records
      [ImportRecordIdx::from_usize(0)]
    .namespace_ref = second_ref;

    let merges = compute(&modules, false);
    let info = merges.get(&module_idx(2)).expect("CommonJS merge group");
    assert_eq!(info.namespace_refs, vec![first_ref, second_ref]);
    assert!(info.needs_interop);
  }

  #[test]
  fn disables_merging_under_strict_execution_order() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(1, 2))]),
      normal_module(1, false, Vec::new()),
    ]);
    modules[module_idx(1)].as_normal_mut().expect("normal module").exports_kind =
      ExportsKind::CommonJs;
    assert!(!compute(&modules, false).is_empty());
    assert!(compute(&modules, true).is_empty());
  }
}
