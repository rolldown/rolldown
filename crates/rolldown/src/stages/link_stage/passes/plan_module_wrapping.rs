use std::convert::Infallible;

use rolldown_common::{
  EcmaViewMeta, ExportsKind, ImportKind, Module, ModuleIdx, ModuleTable, WrapKind,
};
use rolldown_utils::{
  IndexBitSet,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
};
use rustc_hash::FxHashSet;

use super::{
  PlanModuleWrappingPass,
  determine_module_formats::{ModuleFormatsDraft, WrapperSeeds, WrapperStateDraftSlot},
};

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct PlanModuleWrappingInput<'a> {
  pub module_table: &'a ModuleTable,
  pub module_formats: &'a ModuleFormatsDraft,
  pub runtime: ModuleIdx,
  pub strict_execution_order: bool,
  pub on_demand_wrapping: bool,
}

pub(in crate::stages::link_stage) struct WrapperPlan {
  slots: oxc_index::IndexVec<ModuleIdx, WrapperStateDraftSlot>,
}

impl WrapperPlan {
  pub(super) fn into_inner(self) -> oxc_index::IndexVec<ModuleIdx, WrapperStateDraftSlot> {
    self.slots
  }
}

struct WrappingContext<'a> {
  visited: &'a mut IndexBitSet<ModuleIdx>,
  module_table: &'a ModuleTable,
  module_formats: &'a ModuleFormatsDraft,
  plan: &'a mut oxc_index::IndexVec<ModuleIdx, WrapperStateDraftSlot>,
  runtime: ModuleIdx,
  on_demand_wrapping: bool,
}

fn wrap_module_recursively(context: &mut WrappingContext<'_>, target: ModuleIdx) {
  if !context.visited.set_bit(target) {
    return;
  }
  let Some(module) = context.module_table[target].as_normal() else { return };
  if target == context.runtime {
    return;
  }

  let Some(exports_kind) = context.module_formats.get(target) else { return };
  if context.on_demand_wrapping
    && std::matches!(exports_kind, ExportsKind::Esm | ExportsKind::None)
    && !module.meta.contains(EcmaViewMeta::ExecutionOrderSensitive)
    && module.import_records.is_empty()
  {
    return;
  }
  if context.plan[target].kind == Some(WrapKind::None) {
    context.plan[target].kind = Some(match exports_kind {
      ExportsKind::Esm | ExportsKind::None => WrapKind::Esm,
      ExportsKind::CommonJs => WrapKind::Cjs,
    });
  }

  for record in &module.import_records {
    let Some(importee_idx) = record.resolved_module else { continue };
    if record.kind == ImportKind::Require {
      context.plan[importee_idx].required_by_other_module = true;
    }
    wrap_module_recursively(context, importee_idx);
  }
}

impl Pass for PlanModuleWrappingPass {
  type InputRead<'a> = PlanModuleWrappingInput<'a>;
  type InputOwned = WrapperSeeds;
  type OutputRead = ();
  type OutputOwned = WrapperPlan;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    seeds: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let PlanModuleWrappingInput {
      module_table,
      module_formats,
      runtime,
      strict_execution_order,
      on_demand_wrapping,
    } = input;
    let mut plan = seeds.into_inner();
    let mut visited = IndexBitSet::new(module_table.modules.len());
    let mut commonjs_modules = FxHashSet::default();

    for module in module_table.modules.iter().filter_map(Module::as_normal) {
      let module_idx = module.idx;
      if strict_execution_order && module_formats.get(module_idx) == Some(ExportsKind::CommonJs) {
        commonjs_modules.insert(module_idx);
      }

      if plan[module_idx].kind == Some(WrapKind::None) {
        for record in &module.import_records {
          let Some(importee_idx) = record.resolved_module else { continue };
          let Some(_) = module_table[importee_idx].as_normal() else { continue };
          if record.kind == ImportKind::Require {
            plan[importee_idx].required_by_other_module = true;
          }
          if module_formats.get(importee_idx) == Some(ExportsKind::CommonJs) {
            wrap_module_recursively(
              &mut WrappingContext {
                visited: &mut visited,
                module_table,
                module_formats,
                plan: &mut plan,
                runtime,
                on_demand_wrapping,
              },
              importee_idx,
            );
          }
        }
      } else {
        wrap_module_recursively(
          &mut WrappingContext {
            visited: &mut visited,
            module_table,
            module_formats,
            plan: &mut plan,
            runtime,
            on_demand_wrapping,
          },
          module_idx,
        );
      }
    }

    if strict_execution_order {
      for (module_idx, module) in module_table.modules.iter_enumerated() {
        if module_idx == runtime {
          continue;
        }
        let Module::Normal(module) = module else { continue };
        if commonjs_modules.contains(&module_idx) {
          plan[module_idx].kind = Some(WrapKind::Cjs);
        } else {
          let avoid_wrapping = on_demand_wrapping
            && !module.meta.contains(EcmaViewMeta::ExecutionOrderSensitive)
            && module.import_records.is_empty()
            && !plan[module_idx].required_by_other_module;
          plan[module_idx].kind = Some(if avoid_wrapping { WrapKind::None } else { WrapKind::Esm });
        }
      }
    }

    Ok(token.finish((), WrapperPlan { slots: plan }))
  }
}

#[cfg(test)]
mod tests {
  use oxc::span::Span;
  use rolldown_common::{
    EntryPointKind, ExportsKind, ImportKind, ModuleTable, OutputFormat, WrapKind,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::{
    CanonicalizeEntriesPass, DetermineModuleFormatsInput, DetermineModuleFormatsPass,
    EntryPlanDraft,
    test_utils::{entry_point, external_module, module_idx, module_table, normal_module},
  };
  use super::{PlanModuleWrappingInput, PlanModuleWrappingPass};

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

  fn plan(
    modules: &ModuleTable,
    runtime: usize,
    strict_execution_order: bool,
    on_demand_wrapping: bool,
  ) -> Vec<(WrapKind, bool)> {
    let entry_plan = entry_plan(modules);
    let mut pipeline = PassPipelineCtx::new();
    let (_, (formats, seeds)) = run_infallible_pass(
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
    let (_, plan) = run_infallible_pass(
      PlanModuleWrappingPass,
      &mut pipeline,
      PlanModuleWrappingInput {
        module_table: modules,
        module_formats: &formats,
        runtime: module_idx(runtime),
        strict_execution_order,
        on_demand_wrapping,
      },
      seeds,
    );
    assert!(pipeline.into_diagnostics().is_empty());
    plan
      .slots
      .iter()
      .map(|slot| (slot.kind.unwrap_or(WrapKind::None), slot.required_by_other_module))
      .collect()
  }

  #[test]
  fn propagates_wrapping_and_require_flags_without_wrapping_the_runtime() {
    let mut modules = module_table(vec![
      normal_module(0, false, Vec::new()),
      normal_module(1, false, vec![(ImportKind::Require, Some(2), Span::new(1, 2))]),
      normal_module(2, false, vec![(ImportKind::Require, Some(3), Span::new(3, 4))]),
      external_module(3, "external"),
    ]);
    modules[module_idx(2)].as_normal_mut().expect("normal module").exports_kind =
      ExportsKind::CommonJs;

    let plan = plan(&modules, 0, false, false);
    assert_eq!(plan[0], (WrapKind::None, false));
    assert_eq!(plan[1], (WrapKind::None, false));
    assert_eq!(plan[2], (WrapKind::Cjs, true));
    assert_eq!(plan[3], (WrapKind::None, true));
  }

  #[test]
  fn strict_order_overrides_normal_modules_but_preserves_runtime_and_externals() {
    let mut modules = module_table(vec![
      normal_module(0, false, Vec::new()),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
      external_module(3, "external"),
    ]);
    modules[module_idx(2)].as_normal_mut().expect("normal module").exports_kind =
      ExportsKind::CommonJs;

    let strict_plan = plan(&modules, 0, true, false);
    assert_eq!(strict_plan[0], (WrapKind::None, false));
    assert_eq!(strict_plan[1], (WrapKind::Esm, false));
    assert_eq!(strict_plan[2], (WrapKind::Cjs, false));
    assert_eq!(strict_plan[3], (WrapKind::None, false));

    let on_demand = plan(&modules, 0, true, true);
    assert_eq!(on_demand[1], (WrapKind::None, false));
    assert_eq!(on_demand[2], (WrapKind::Cjs, false));
  }
}
