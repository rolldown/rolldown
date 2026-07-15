use std::convert::Infallible;

use rolldown_common::{ExportsKind, Module, ModuleIdx, ModuleTable};
use rolldown_utils::{
  IndexBitSet,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
};

use super::{ComputeDynamicExportsPass, determine_module_formats::ModuleFormatsDraft};

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct ComputeDynamicExportsInput<'a> {
  pub module_table: &'a ModuleTable,
  pub module_formats: &'a ModuleFormatsDraft,
}

pub(in crate::stages::link_stage) struct DynamicExports {
  module_count: usize,
  modules: IndexBitSet<ModuleIdx>,
}

impl DynamicExports {
  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.module_count
  }

  pub(in crate::stages::link_stage) fn modules(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
    self.modules.index_of_one()
  }
}

fn has_dynamic_exports_due_to_export_star(
  target: ModuleIdx,
  module_table: &ModuleTable,
  module_formats: &ModuleFormatsDraft,
  dynamic_exports: &mut IndexBitSet<ModuleIdx>,
  visited: &mut IndexBitSet<ModuleIdx>,
) -> bool {
  if !visited.set_bit(target) {
    return dynamic_exports.has_bit(target);
  }

  let has_dynamic_exports = match &module_table[target] {
    Module::Normal(module) => {
      if module_formats.get(target) == Some(ExportsKind::CommonJs) {
        true
      } else {
        module.star_export_module_ids().any(|importee_idx| {
          target != importee_idx
            && has_dynamic_exports_due_to_export_star(
              importee_idx,
              module_table,
              module_formats,
              dynamic_exports,
              visited,
            )
        })
      }
    }
    Module::External(_) => true,
  };

  if has_dynamic_exports {
    dynamic_exports.set_bit(target);
  }
  dynamic_exports.has_bit(target)
}

impl Pass for ComputeDynamicExportsPass {
  type InputRead<'a> = ComputeDynamicExportsInput<'a>;
  type InputOwned = ();
  type OutputRead = DynamicExports;
  type OutputOwned = ();
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let ComputeDynamicExportsInput { module_table, module_formats } = input;
    let module_count = module_table.modules.len();
    let mut dynamic_exports = IndexBitSet::new(module_count);
    let mut visited = IndexBitSet::new(module_count);

    for module in module_table.modules.iter().filter_map(Module::as_normal) {
      if module.has_star_export() {
        has_dynamic_exports_due_to_export_star(
          module.idx,
          module_table,
          module_formats,
          &mut dynamic_exports,
          &mut visited,
        );
      }
    }

    Ok(token.finish(DynamicExports { module_count, modules: dynamic_exports }, ()))
  }
}

#[cfg(test)]
mod tests {
  use oxc::span::Span;
  use rolldown_common::{
    EcmaViewMeta, EntryPointKind, ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta,
    ModuleTable, OutputFormat,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::{
    CanonicalizeEntriesPass, DetermineModuleFormatsInput, DetermineModuleFormatsPass,
    EntryPlanDraft,
    test_utils::{entry_point, external_module, module_idx, module_table, normal_module},
  };
  use super::{ComputeDynamicExportsInput, ComputeDynamicExportsPass};

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

  fn star(modules: &mut ModuleTable, importer: usize, record: usize) {
    let module = modules[module_idx(importer)].as_normal_mut().expect("normal module");
    module.meta.insert(EcmaViewMeta::HasStarExport);
    module.import_records[ImportRecordIdx::from_usize(record)]
      .meta
      .insert(ImportRecordMeta::IsExportStar);
  }

  #[test]
  fn propagates_commonjs_and_external_dynamic_exports_through_export_stars() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(1, 2))]),
      normal_module(1, false, vec![(ImportKind::Import, Some(2), Span::new(3, 4))]),
      normal_module(2, false, Vec::new()),
      normal_module(3, false, vec![(ImportKind::Import, Some(4), Span::new(5, 6))]),
      external_module(4, "external"),
    ]);
    star(&mut modules, 0, 0);
    star(&mut modules, 1, 0);
    star(&mut modules, 3, 0);
    modules[module_idx(2)].as_normal_mut().expect("normal module").exports_kind =
      ExportsKind::CommonJs;

    let entry_plan = entry_plan(&modules);
    let mut pipeline = PassPipelineCtx::new();
    let (_, (formats, _)) = run_infallible_pass(
      DetermineModuleFormatsPass,
      &mut pipeline,
      DetermineModuleFormatsInput {
        module_table: &modules,
        entry_plan: &entry_plan,
        output_format: OutputFormat::Esm,
        code_splitting_disabled: false,
      },
      (),
    );
    let (dynamic_exports, ()) = run_infallible_pass(
      ComputeDynamicExportsPass,
      &mut pipeline,
      ComputeDynamicExportsInput { module_table: &modules, module_formats: &formats },
      (),
    );

    assert_eq!(dynamic_exports.module_count(), modules.modules.len());
    assert_eq!(dynamic_exports.modules().collect::<Vec<_>>(), [0, 1, 2, 3, 4].map(module_idx));
    assert!(pipeline.into_diagnostics().is_empty());
  }

  #[test]
  fn preserves_cycle_cache_behavior_for_static_export_cycles() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(1, 2))]),
      normal_module(1, false, vec![(ImportKind::Import, Some(0), Span::new(3, 4))]),
    ]);
    star(&mut modules, 0, 0);
    star(&mut modules, 1, 0);
    let entry_plan = entry_plan(&modules);
    let mut pipeline = PassPipelineCtx::new();
    let (_, (formats, _)) = run_infallible_pass(
      DetermineModuleFormatsPass,
      &mut pipeline,
      DetermineModuleFormatsInput {
        module_table: &modules,
        entry_plan: &entry_plan,
        output_format: OutputFormat::Esm,
        code_splitting_disabled: false,
      },
      (),
    );
    let (dynamic_exports, ()) = run_infallible_pass(
      ComputeDynamicExportsPass,
      &mut pipeline,
      ComputeDynamicExportsInput { module_table: &modules, module_formats: &formats },
      (),
    );
    assert!(dynamic_exports.modules().next().is_none());
    assert!(pipeline.into_diagnostics().is_empty());
  }
}
