use std::convert::Infallible;

use rolldown_common::{
  ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta, Module, ModuleIdx, ModuleTable,
  ResolvedImportRecord,
};
use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};
use rustc_hash::FxHashMap;

use crate::utils::external_import_interop::{import_record_needs_interop, specifier_needs_interop};

use super::{ComputeCjsNamespaceMergesPass, determine_module_formats::ModuleFormatsDraft};
use crate::stages::link_stage::SafelyMergeCjsNsInfo;

// Preserve the old early-exit behavior for small importers, then switch to one named-import sweep
// before repeated per-record scans can multiply the work.
const DIRECT_INTEROP_SCAN_LIMIT: usize = 4;

fn eligible_cjs_importee(
  record: &ResolvedImportRecord,
  module_formats: &ModuleFormatsDraft,
) -> Option<ModuleIdx> {
  if record.kind != ImportKind::Import || record.meta.contains(ImportRecordMeta::IsExportStar) {
    return None;
  }
  let importee_idx = record.resolved_module?;
  (module_formats.get(importee_idx) == Some(ExportsKind::CommonJs)).then_some(importee_idx)
}

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct ComputeCjsNamespaceMergesInput<'a> {
  pub module_table: &'a ModuleTable,
  pub module_formats: &'a ModuleFormatsDraft,
  pub strict_execution_order: bool,
}

pub(in crate::stages::link_stage) struct CjsNamespaceMerges {
  module_count: usize,
  modules: FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo>,
}

impl CjsNamespaceMerges {
  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.module_count
  }

  pub(in crate::stages::link_stage) fn needs_interop(&self, module_idx: ModuleIdx) -> Option<bool> {
    self.modules.get(&module_idx).map(|info| info.needs_interop)
  }

  pub(in crate::stages::link_stage) fn identity_owners(
    &self,
  ) -> impl Iterator<Item = ModuleIdx> + '_ {
    self
      .modules
      .values()
      .flat_map(|info| info.namespace_refs.iter().map(|symbol_ref| symbol_ref.owner))
  }

  pub(in crate::stages::link_stage) fn into_legacy(
    self,
  ) -> FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo> {
    self.modules
  }
}

#[cfg(test)]
pub(super) mod test_support {
  use rolldown_common::{ModuleIdx, SymbolRef};

  use super::{CjsNamespaceMerges, SafelyMergeCjsNsInfo};

  pub(in crate::stages::link_stage::passes) fn cjs_namespace_merges(
    module_count: usize,
    modules: impl IntoIterator<Item = (ModuleIdx, bool)>,
  ) -> CjsNamespaceMerges {
    CjsNamespaceMerges {
      module_count,
      modules: modules
        .into_iter()
        .map(|(module_idx, needs_interop)| {
          (
            module_idx,
            SafelyMergeCjsNsInfo { namespace_refs: Vec::<SymbolRef>::new(), needs_interop },
          )
        })
        .collect(),
    }
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
        let mut direct_interop_records = [None::<ImportRecordIdx>; DIRECT_INTEROP_SCAN_LIMIT];
        let mut eligible_record_count = 0_usize;
        let mut needs_interop_scan = false;
        for (record_idx, record) in importer.import_records.iter_enumerated() {
          let Some(importee_idx) = eligible_cjs_importee(record, module_formats) else { continue };
          if eligible_record_count < DIRECT_INTEROP_SCAN_LIMIT {
            direct_interop_records[eligible_record_count] = Some(record_idx);
          }
          eligible_record_count += 1;

          let info = modules.entry(importee_idx).or_default();
          needs_interop_scan |= !info.needs_interop;
          info.namespace_refs.push(record.namespace_ref);
        }
        if eligible_record_count == 0 || !needs_interop_scan {
          continue;
        }
        if eligible_record_count <= DIRECT_INTEROP_SCAN_LIMIT {
          for record_idx in direct_interop_records.into_iter().flatten() {
            if !import_record_needs_interop(importer, record_idx) {
              continue;
            }
            let record = &importer.import_records[record_idx];
            let Some(importee_idx) = eligible_cjs_importee(record, module_formats) else {
              continue;
            };
            if let Some(info) = modules.get_mut(&importee_idx) {
              info.needs_interop = true;
            }
          }
          continue;
        }

        for import in
          importer.named_imports.values().filter(|import| specifier_needs_interop(&import.imported))
        {
          let Some(record) = importer.import_records.get(import.record_idx) else { continue };
          let Some(importee_idx) = eligible_cjs_importee(record, module_formats) else { continue };
          if let Some(info) = modules.get_mut(&importee_idx) {
            info.needs_interop = true;
          }
        }
      }
    }

    Ok(token.finish((), CjsNamespaceMerges { module_count: module_table.modules.len(), modules }))
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
  fn keeps_interop_record_and_importer_local() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(2), Span::new(1, 2)),
          (ImportKind::Import, Some(2), Span::new(3, 4)),
          (ImportKind::Import, Some(3), Span::new(5, 6)),
          (ImportKind::Import, Some(3), Span::new(7, 8)),
          (ImportKind::Import, Some(3), Span::new(9, 10)),
          (ImportKind::Import, Some(3), Span::new(11, 12)),
        ],
      ),
      normal_module(1, false, vec![(ImportKind::Import, Some(2), Span::new(13, 14))]),
      normal_module(2, false, Vec::new()),
      normal_module(3, false, Vec::new()),
    ]);
    for module_idx in [module_idx(2), module_idx(3)] {
      modules[module_idx].as_normal_mut().expect("normal module").exports_kind =
        ExportsKind::CommonJs;
    }

    let named_ref = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(1) };
    let excluded_default_ref = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(2) };
    let invalid_default_ref = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(3) };
    let default_ref = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(4) };
    let importer = modules[module_idx(0)].as_normal_mut().expect("normal importer");
    importer.import_records[ImportRecordIdx::from_usize(0)].namespace_ref = named_ref;
    importer.import_records[ImportRecordIdx::from_usize(1)].namespace_ref = excluded_default_ref;
    importer.import_records[ImportRecordIdx::from_usize(2)].namespace_ref = default_ref;
    importer.import_records[ImportRecordIdx::from_usize(1)]
      .meta
      .insert(ImportRecordMeta::IsExportStar);
    importer.named_imports.insert(
      named_ref,
      NamedImport {
        imported: Specifier::Literal("named".into()),
        span_imported: Span::new(1, 2),
        imported_as: named_ref,
        record_idx: ImportRecordIdx::from_usize(0),
      },
    );
    importer.named_imports.insert(
      excluded_default_ref,
      NamedImport {
        imported: Specifier::Literal("default".into()),
        span_imported: Span::new(3, 4),
        imported_as: excluded_default_ref,
        record_idx: ImportRecordIdx::from_usize(1),
      },
    );
    importer.named_imports.insert(
      invalid_default_ref,
      NamedImport {
        imported: Specifier::Literal("default".into()),
        span_imported: Span::new(5, 6),
        imported_as: invalid_default_ref,
        record_idx: ImportRecordIdx::from_usize(99),
      },
    );
    importer.named_imports.insert(
      default_ref,
      NamedImport {
        imported: Specifier::Literal("default".into()),
        span_imported: Span::new(5, 6),
        imported_as: default_ref,
        record_idx: ImportRecordIdx::from_usize(2),
      },
    );
    let second_named_ref = SymbolRef { owner: module_idx(1), symbol: SymbolId::new(1) };
    let importer = modules[module_idx(1)].as_normal_mut().expect("normal importer");
    importer.import_records[ImportRecordIdx::from_usize(0)].namespace_ref = second_named_ref;
    importer.named_imports.insert(
      second_named_ref,
      NamedImport {
        imported: Specifier::Literal("named".into()),
        span_imported: Span::new(13, 14),
        imported_as: second_named_ref,
        record_idx: ImportRecordIdx::from_usize(0),
      },
    );

    let merges = compute(&modules, false);
    let named_only = merges.get(&module_idx(2)).expect("named-only CommonJS merge group");
    assert_eq!(named_only.namespace_refs, vec![named_ref, second_named_ref]);
    assert!(!named_only.needs_interop);
    let default = merges.get(&module_idx(3)).expect("default CommonJS merge group");
    assert_eq!(default.namespace_refs.len(), 4);
    assert_eq!(default.namespace_refs.first(), Some(&default_ref));
    assert!(default.needs_interop);
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
