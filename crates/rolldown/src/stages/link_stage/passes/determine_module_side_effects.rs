use std::convert::Infallible;

use oxc_index::IndexVec;
use rolldown_common::{
  ImportKind, ImportRecordMeta, Module, ModuleIdx, ModuleTable, WrapKind,
  side_effects::DeterminedSideEffects,
};
use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};

use super::{DetermineModuleSideEffectsPass, DynamicExports, ModuleWrappers};

// See internal-docs/linking/module-side-effects/implementation.md.

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct DetermineModuleSideEffectsInput<'a> {
  pub module_table: &'a ModuleTable,
  pub dynamic_exports: &'a DynamicExports,
  pub module_wrappers: &'a ModuleWrappers,
}

pub(in crate::stages::link_stage) struct ModuleSideEffects {
  values: IndexVec<ModuleIdx, DeterminedSideEffects>,
}

impl ModuleSideEffects {
  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.values.len()
  }

  pub(in crate::stages::link_stage) fn get(&self, module_idx: ModuleIdx) -> DeterminedSideEffects {
    self.values[module_idx]
  }
}

#[derive(Debug, Clone, Copy)]
enum SideEffectCache {
  None,
  Visited,
  Cache(DeterminedSideEffects),
}

fn determine_side_effects_for_module(
  module_idx: ModuleIdx,
  module_table: &ModuleTable,
  dynamic_exports: &DynamicExports,
  module_wrappers: &ModuleWrappers,
  side_effects: &IndexVec<ModuleIdx, DeterminedSideEffects>,
  cache: &mut IndexVec<ModuleIdx, SideEffectCache>,
) -> DeterminedSideEffects {
  match cache[module_idx] {
    SideEffectCache::None => cache[module_idx] = SideEffectCache::Visited,
    SideEffectCache::Visited => return side_effects[module_idx],
    SideEffectCache::Cache(value) => return value,
  }

  let module_side_effects = side_effects[module_idx];
  match module_side_effects {
    DeterminedSideEffects::Analyzed(true)
    | DeterminedSideEffects::UserDefined(_)
    | DeterminedSideEffects::NoTreeshake => module_side_effects,
    DeterminedSideEffects::Analyzed(false) => match &module_table[module_idx] {
      Module::Normal(module) => {
        let has_side_effects = module
          .import_records
          .iter()
          .filter_map(|record| record.resolved_module.map(|importee| (record, importee)))
          .any(|(record, importee)| {
            if determine_side_effects_for_module(
              importee,
              module_table,
              dynamic_exports,
              module_wrappers,
              side_effects,
              cache,
            )
            .has_side_effects()
            {
              return true;
            }

            if record.kind == ImportKind::Import
              && record.meta.contains(ImportRecordMeta::IsExportStar)
              && let Module::Normal(importee_module) = &module_table[importee]
            {
              let importee_idx = importee_module.idx;
              return match module_wrappers.wrap_kind(importee_idx) {
                WrapKind::None => dynamic_exports.contains(importee_idx),
                WrapKind::Cjs | WrapKind::Esm => true,
              };
            }

            false
          });
        let determined = DeterminedSideEffects::Analyzed(has_side_effects);
        cache[module_idx] = SideEffectCache::Cache(determined);
        determined
      }
      Module::External(_) => module_side_effects,
    },
  }
}

impl Pass for DetermineModuleSideEffectsPass {
  type InputRead<'a> = DetermineModuleSideEffectsInput<'a>;
  type InputOwned = ();
  type OutputRead = ModuleSideEffects;
  type OutputOwned = ();
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let DetermineModuleSideEffectsInput { module_table, dynamic_exports, module_wrappers } = input;
    let module_count = module_table.modules.len();
    let mut side_effects = module_table
      .modules
      .iter()
      .map(|module| *module.side_effects())
      .collect::<IndexVec<ModuleIdx, _>>();
    let mut cache = oxc_index::index_vec![SideEffectCache::None; module_count];

    for index in 0..module_count {
      let module_idx = ModuleIdx::new(index);
      let determined = determine_side_effects_for_module(
        module_idx,
        module_table,
        dynamic_exports,
        module_wrappers,
        &side_effects,
        &mut cache,
      );
      if module_table[module_idx].as_normal().is_some() {
        side_effects[module_idx] = determined;
      }
    }

    Ok(token.finish(ModuleSideEffects { values: side_effects }, ()))
  }
}

#[cfg(test)]
mod tests {
  use oxc::span::Span;
  use rolldown_common::{
    ImportKind, ImportRecordIdx, ImportRecordMeta, Module, ModuleTable, WrapKind,
    side_effects::DeterminedSideEffects,
  };
  use rolldown_utils::pass::{PassPipelineCtx, Sealed, run_infallible_pass};

  use super::super::{
    DetermineModuleSideEffectsInput, DetermineModuleSideEffectsPass, ModuleSideEffects,
    compute_dynamic_exports::test_support::dynamic_exports,
    create_wrapper_declarations::test_support::module_wrappers,
    test_utils::{external_module, module_idx, module_table, normal_module},
  };

  fn set_side_effects(
    modules: &mut ModuleTable,
    index: usize,
    side_effects: DeterminedSideEffects,
  ) {
    match &mut modules[module_idx(index)] {
      Module::Normal(module) => module.side_effects = side_effects,
      Module::External(module) => module.side_effects = side_effects,
    }
  }

  fn mark_export_star(modules: &mut ModuleTable, importer: usize, record: usize) {
    modules[module_idx(importer)].as_normal_mut().expect("normal importer").import_records
      [ImportRecordIdx::from_usize(record)]
    .meta
    .insert(ImportRecordMeta::IsExportStar);
  }

  fn determine(
    modules: &ModuleTable,
    dynamic: impl IntoIterator<Item = usize>,
    wrap_kinds: &[WrapKind],
  ) -> Vec<DeterminedSideEffects> {
    assert_eq!(modules.modules.len(), wrap_kinds.len());
    let dynamic = dynamic_exports(modules.modules.len(), dynamic.into_iter().map(module_idx));
    let wrappers = module_wrappers(wrap_kinds);
    let mut pipeline = PassPipelineCtx::new();
    let (side_effects, ()) = run_infallible_pass(
      DetermineModuleSideEffectsPass,
      &mut pipeline,
      DetermineModuleSideEffectsInput {
        module_table: modules,
        dynamic_exports: &dynamic,
        module_wrappers: &wrappers,
      },
      (),
    );
    let _: &Sealed<ModuleSideEffects> = &side_effects;
    assert!(pipeline.into_diagnostics().is_empty());
    (0..side_effects.module_count()).map(|index| side_effects.get(module_idx(index))).collect()
  }

  fn flags(values: &[DeterminedSideEffects]) -> Vec<bool> {
    values.iter().map(DeterminedSideEffects::has_side_effects).collect()
  }

  #[test]
  fn preserves_import_record_order_when_a_cycle_precedes_a_side_effectful_sibling() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(0, 1)),
          (ImportKind::Import, Some(2), Span::new(1, 2)),
        ],
      ),
      normal_module(1, false, vec![(ImportKind::Import, Some(0), Span::new(2, 3))]),
      normal_module(2, false, Vec::new()),
    ]);
    set_side_effects(&mut modules, 2, DeterminedSideEffects::Analyzed(true));

    let values = determine(&modules, [], &[WrapKind::None; 3]);
    assert_eq!(flags(&values), [true, false, true]);
  }

  #[test]
  fn preserves_import_record_order_when_a_side_effectful_sibling_precedes_a_cycle() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(2), Span::new(0, 1)),
          (ImportKind::Import, Some(1), Span::new(1, 2)),
        ],
      ),
      normal_module(1, false, vec![(ImportKind::Import, Some(0), Span::new(2, 3))]),
      normal_module(2, false, Vec::new()),
    ]);
    set_side_effects(&mut modules, 2, DeterminedSideEffects::Analyzed(true));

    let values = determine(&modules, [], &[WrapKind::None; 3]);
    assert_eq!(flags(&values), [true, true, true]);
  }

  #[test]
  fn preserves_physical_module_order_across_the_same_cycle_graph() {
    let mut a_first = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(0, 1))]),
      normal_module(
        1,
        false,
        vec![
          (ImportKind::Import, Some(0), Span::new(1, 2)),
          (ImportKind::Import, Some(2), Span::new(2, 3)),
        ],
      ),
      normal_module(2, false, Vec::new()),
    ]);
    set_side_effects(&mut a_first, 2, DeterminedSideEffects::Analyzed(true));

    let mut b_first = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(0, 1)),
          (ImportKind::Import, Some(2), Span::new(1, 2)),
        ],
      ),
      normal_module(1, false, vec![(ImportKind::Import, Some(0), Span::new(2, 3))]),
      normal_module(2, false, Vec::new()),
    ]);
    set_side_effects(&mut b_first, 2, DeterminedSideEffects::Analyzed(true));

    assert_eq!(flags(&determine(&a_first, [], &[WrapKind::None; 3])), [true, true, true]);
    assert_eq!(flags(&determine(&b_first, [], &[WrapKind::None; 3])), [true, false, true]);
  }

  #[test]
  fn propagates_a_transitive_wrapped_export_star() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(0, 1))]),
      normal_module(1, false, vec![(ImportKind::Import, Some(2), Span::new(1, 2))]),
      normal_module(2, false, Vec::new()),
    ]);
    mark_export_star(&mut modules, 0, 0);
    mark_export_star(&mut modules, 1, 0);

    let values = determine(&modules, [], &[WrapKind::None, WrapKind::None, WrapKind::Cjs]);
    assert_eq!(flags(&values), [true, true, false]);
  }

  #[test]
  fn marks_an_unwrapped_dynamic_export_star_as_side_effectful() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(0, 1))]),
      normal_module(1, false, Vec::new()),
    ]);
    mark_export_star(&mut modules, 0, 0);

    let values = determine(&modules, [1], &[WrapKind::None; 2]);
    assert_eq!(flags(&values), [true, false]);
  }

  #[test]
  fn uses_final_cjs_and_esm_wrapper_facts() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(0, 1))]),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, vec![(ImportKind::Import, Some(3), Span::new(2, 3))]),
      normal_module(3, false, Vec::new()),
      normal_module(4, false, vec![(ImportKind::Import, Some(5), Span::new(4, 5))]),
      normal_module(5, false, Vec::new()),
    ]);
    mark_export_star(&mut modules, 0, 0);
    mark_export_star(&mut modules, 2, 0);
    mark_export_star(&mut modules, 4, 0);

    let values = determine(
      &modules,
      [],
      &[
        WrapKind::None,
        WrapKind::Cjs,
        WrapKind::None,
        WrapKind::Esm,
        WrapKind::None,
        WrapKind::None,
      ],
    );
    assert_eq!(flags(&values), [true, false, true, false, false, false]);
  }

  #[test]
  fn keeps_external_modules_unchanged_and_excludes_them_from_export_star_special_cases() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(0, 1))]),
      external_module(1, "external"),
    ]);
    mark_export_star(&mut modules, 0, 0);
    set_side_effects(&mut modules, 1, DeterminedSideEffects::Analyzed(false));

    let values = determine(&modules, [1], &[WrapKind::None; 2]);
    assert_eq!(flags(&values), [false, false]);
  }

  #[test]
  fn traverses_every_resolved_import_kind_including_hot_accept() {
    for kind in [
      ImportKind::Import,
      ImportKind::DynamicImport,
      ImportKind::Require,
      ImportKind::AtImport,
      ImportKind::UrlImport,
      ImportKind::NewUrl,
      ImportKind::HotAccept,
    ] {
      let mut modules = module_table(vec![
        normal_module(0, false, vec![(kind, Some(1), Span::new(0, 1))]),
        normal_module(1, false, Vec::new()),
      ]);
      set_side_effects(&mut modules, 1, DeterminedSideEffects::Analyzed(true));

      let values = determine(&modules, [], &[WrapKind::None; 2]);
      assert!(values[0].has_side_effects(), "resolved {kind:?} edge was skipped");
    }
  }

  #[test]
  fn skips_unresolved_import_records() {
    let modules = module_table(vec![normal_module(
      0,
      false,
      vec![(ImportKind::Import, None, Span::new(0, 1))],
    )]);

    let values = determine(&modules, [], &[WrapKind::None]);
    assert_eq!(flags(&values), [false]);
  }

  #[test]
  fn limits_dynamic_export_handling_to_normal_import_export_stars() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(2), Span::new(0, 1)),
          (ImportKind::Require, Some(2), Span::new(1, 2)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
    ]);
    mark_export_star(&mut modules, 0, 1);

    let values = determine(&modules, [2], &[WrapKind::None; 3]);
    assert_eq!(flags(&values), [false, false, false]);
  }

  #[test]
  fn preserves_non_analyzed_side_effect_variants_without_traversing_dependencies() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(0, 1))]),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
    ]);
    set_side_effects(&mut modules, 0, DeterminedSideEffects::UserDefined(false));
    set_side_effects(&mut modules, 1, DeterminedSideEffects::Analyzed(true));
    set_side_effects(&mut modules, 2, DeterminedSideEffects::NoTreeshake);

    let values = determine(&modules, [], &[WrapKind::None; 3]);
    assert!(std::matches!(values[0], DeterminedSideEffects::UserDefined(false)));
    assert!(std::matches!(values[1], DeterminedSideEffects::Analyzed(true)));
    assert!(std::matches!(values[2], DeterminedSideEffects::NoTreeshake));
  }
}
