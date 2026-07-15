use std::convert::Infallible;

use oxc_index::IndexVec;
use rolldown_common::{ImportRecordIdx, ModuleIdx, ModuleTable};
use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};

use super::CollectExternalStarExportsPass;

pub(in crate::stages::link_stage) struct ExternalStarExports {
  records: IndexVec<ModuleIdx, Vec<ImportRecordIdx>>,
}

impl ExternalStarExports {
  pub(in crate::stages::link_stage) fn into_inner(
    self,
  ) -> IndexVec<ModuleIdx, Vec<ImportRecordIdx>> {
    self.records
  }
}

impl Pass for CollectExternalStarExportsPass {
  type InputRead<'a> = &'a ModuleTable;
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = ExternalStarExports;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    module_table: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let records = module_table
      .modules
      .iter()
      .map(|module| {
        module.as_normal().map_or_else(Vec::new, |module| {
          module.star_exports_from_external_modules(&module_table.modules).collect()
        })
      })
      .collect();
    Ok(token.finish((), ExternalStarExports { records }))
  }
}

#[cfg(test)]
mod tests {
  use oxc::span::Span;
  use rolldown_common::{ImportKind, ImportRecordIdx, ImportRecordMeta};
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::test_utils::{external_module, module_idx, module_table, normal_module};
  use super::CollectExternalStarExportsPass;

  #[test]
  fn collects_only_resolved_external_export_stars_in_record_order() {
    let mut importer = normal_module(
      0,
      false,
      vec![
        (ImportKind::Import, Some(1), Span::new(1, 2)),
        (ImportKind::Import, Some(2), Span::new(2, 3)),
        (ImportKind::Import, None, Span::new(3, 4)),
        (ImportKind::Import, Some(3), Span::new(4, 5)),
        (ImportKind::Import, Some(1), Span::new(5, 6)),
      ],
    );
    let records = &mut importer.as_normal_mut().expect("normal importer").import_records;
    for index in [0, 1, 2, 4] {
      records[ImportRecordIdx::from_usize(index)].meta.insert(ImportRecordMeta::IsExportStar);
    }
    let modules = module_table(vec![
      importer,
      external_module(1, "external-a"),
      normal_module(2, false, Vec::new()),
      external_module(3, "external-b"),
    ]);
    let mut pipeline = PassPipelineCtx::new();
    let (_, records) =
      run_infallible_pass(CollectExternalStarExportsPass, &mut pipeline, &modules, ());
    let records = records.into_inner();

    assert_eq!(
      records[module_idx(0)].iter().map(|idx| idx.index()).collect::<Vec<_>>(),
      vec![0, 4]
    );
    assert!(records.iter().skip(1).all(Vec::is_empty));
    assert!(pipeline.into_diagnostics().is_empty());
  }
}
