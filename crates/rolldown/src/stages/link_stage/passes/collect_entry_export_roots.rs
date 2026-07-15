use std::convert::Infallible;

use rolldown_common::{
  EntryPointKind, ModuleIdx, ModuleTable, PreserveEntrySignatures, SymbolRef,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_utils::{
  indexmap::FxIndexMap,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
};
use rustc_hash::FxHashMap;

use super::{
  CollectEntryExportRootsPass, EntryPlanDraft, ModuleWrappers, ResolvedExports, WrapperDeclaration,
};

// See internal-docs/pass-based-pipeline/implementation.md.

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct CollectEntryExportRootsInput<'a> {
  pub module_table: &'a ModuleTable,
  pub entry_plan: &'a EntryPlanDraft,
  pub module_wrappers: &'a ModuleWrappers,
  pub resolved_exports: &'a ResolvedExports,
  pub dynamic_import_usage: &'a FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub preserve_signature_overrides: &'a FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub default_preserve_signature: PreserveEntrySignatures,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::stages::link_stage) struct EntryExportRoot {
  pub symbol_ref: SymbolRef,
  pub came_from_commonjs: bool,
}

pub(in crate::stages::link_stage) struct EntryExportRoots {
  roots: FxIndexMap<ModuleIdx, Vec<EntryExportRoot>>,
}

impl EntryExportRoots {
  pub(in crate::stages::link_stage) fn get(
    &self,
    module_idx: ModuleIdx,
  ) -> Option<&[EntryExportRoot]> {
    self.roots.get(&module_idx).map(Vec::as_slice)
  }

  pub(in crate::stages::link_stage) fn into_entries(
    self,
  ) -> impl Iterator<Item = (ModuleIdx, Vec<EntryExportRoot>)> {
    self.roots.into_iter()
  }
}

impl Pass for CollectEntryExportRootsPass {
  type InputRead<'a> = CollectEntryExportRootsInput<'a>;
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = EntryExportRoots;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let CollectEntryExportRootsInput {
      module_table,
      entry_plan,
      module_wrappers,
      resolved_exports,
      dynamic_import_usage,
      preserve_signature_overrides,
      default_preserve_signature,
    } = input;
    let module_count = module_table.modules.len();
    std::assert_eq!(
      module_wrappers.module_count(),
      module_count,
      "wrapper layout must match modules before entry-root collection"
    );
    std::assert_eq!(
      resolved_exports.module_count(),
      module_count,
      "resolved-export layout must match modules before entry-root collection"
    );
    for (module_idx, module) in module_table.modules.iter_enumerated() {
      let valid = match module {
        rolldown_common::Module::Normal(_) => resolved_exports.has_normal_slot(module_idx),
        rolldown_common::Module::External(_) => {
          !resolved_exports.has_normal_slot(module_idx)
            && std::matches!(module_wrappers.declaration(module_idx), WrapperDeclaration::None)
        }
      };
      std::assert!(valid, "entry-root slot shape must match module {module_idx:?}");
    }

    let mut roots = FxIndexMap::default();
    for (module_idx, entries) in entry_plan.entries() {
      let Some(module) = module_table.modules.get(module_idx).and_then(|module| module.as_normal())
      else {
        std::assert!(
          module_idx.index() < module_count,
          "entry plan must not reference an out-of-range module"
        );
        continue;
      };
      let Some(entry) = entries.first() else { continue };
      let mut module_roots = Vec::new();

      match module_wrappers.declaration(module_idx) {
        WrapperDeclaration::None => {}
        WrapperDeclaration::Cjs { wrapper_ref, .. }
        | WrapperDeclaration::Esm { wrapper_ref, .. } => {
          module_roots.push(EntryExportRoot { symbol_ref: wrapper_ref, came_from_commonjs: false });
        }
      }

      let preserve_signature =
        preserve_signature_overrides.get(&entry.idx).copied().unwrap_or(default_preserve_signature);
      if !std::matches!(preserve_signature, PreserveEntrySignatures::False)
        || !module.dynamic_importers.is_empty()
      {
        let partial_used_exports = match entry.kind {
          EntryPointKind::UserDefined | EntryPointKind::EmittedUserDefined => None,
          EntryPointKind::DynamicImport => {
            dynamic_import_usage.get(&entry.idx).and_then(|usage| match usage {
              DynamicImportExportsUsage::Complete => None,
              DynamicImportExportsUsage::Partial(exports) => Some(exports),
              DynamicImportExportsUsage::Single(_) => {
                std::unreachable!("single dynamic-import usage must be merged before Link")
              }
            })
          }
        };
        module_roots.extend(
          resolved_exports
            .canonical_exports(module_idx, true)
            .filter(|(name, _)| {
              partial_used_exports.is_none_or(|exports| exports.contains(name.as_str()))
            })
            .map(|(_, export)| EntryExportRoot {
              symbol_ref: export.symbol_ref,
              came_from_commonjs: export.came_from_commonjs,
            }),
        );
      }

      if !module_roots.is_empty() {
        roots.insert(module_idx, module_roots);
      }
    }

    Ok(token.finish((), EntryExportRoots { roots }))
  }
}

#[cfg(test)]
mod tests {
  use oxc::{semantic::Scoping, span::Span};
  use rolldown_common::{
    EntryPointKind, LocalExport, ModuleId, PreserveEntrySignatures, SymbolRefDb,
    SymbolRefDbForModule, WrapKind, dynamic_import_usage::DynamicImportExportsUsage,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};
  use rustc_hash::{FxHashMap, FxHashSet};

  use super::super::{
    CanonicalizeEntriesPass, CollectEntryExportRootsInput, CollectEntryExportRootsPass,
    CollectResolvedExportsPass, FinalizeResolvedExportsPass,
    create_wrapper_declarations::test_support::module_wrappers,
    test_utils::{entry_point, external_module, module_idx, module_table, normal_module},
  };

  #[test]
  fn preserves_entry_order_wrapper_priority_usage_filters_and_dynamic_override() {
    let mut modules = module_table(vec![
      normal_module(0, false, Vec::new()),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
      external_module(3, "external"),
      normal_module(4, false, Vec::new()),
      normal_module(5, false, Vec::new()),
      normal_module(6, false, Vec::new()),
      normal_module(7, false, Vec::new()),
    ]);
    modules[module_idx(2)]
      .as_normal_mut()
      .expect("normal emitted entry")
      .dynamic_importers
      .insert(ModuleId::new("dynamic-importer.js"));

    let mut symbols = SymbolRefDb::new();
    let mut exports = [None; 8];
    for module_index in [0, 1, 2, 4, 5, 6, 7] {
      let module_idx = module_idx(module_index);
      let scoping = Scoping::default();
      let root_scope_id = scoping.root_scope_id();
      symbols
        .store_local_db(module_idx, SymbolRefDbForModule::new(scoping, module_idx, root_scope_id));
      let namespace = symbols.create_facade_root_symbol_ref(module_idx, "namespace");
      assert_eq!(
        namespace,
        modules[module_idx].as_normal().expect("normal module").namespace_object_ref
      );
      let alpha = symbols.create_facade_root_symbol_ref(module_idx, "alpha");
      let zeta = symbols.create_facade_root_symbol_ref(module_idx, "zeta");
      modules[module_idx].as_normal_mut().expect("normal module").named_exports.extend([
        (
          "zeta".into(),
          LocalExport { span: Span::new(2, 3), referenced: zeta, came_from_commonjs: true },
        ),
        (
          "alpha".into(),
          LocalExport { span: Span::new(1, 2), referenced: alpha, came_from_commonjs: false },
        ),
      ]);
      exports[module_index] = Some((alpha, zeta));
    }

    let mut pipeline = PassPipelineCtx::new();
    let (_, entry_plan) = run_infallible_pass(
      CanonicalizeEntriesPass,
      &mut pipeline,
      &modules,
      vec![
        entry_point(0, EntryPointKind::UserDefined),
        entry_point(1, EntryPointKind::UserDefined),
        entry_point(4, EntryPointKind::UserDefined),
        entry_point(1, EntryPointKind::DynamicImport),
        entry_point(2, EntryPointKind::EmittedUserDefined),
        entry_point(3, EntryPointKind::DynamicImport),
        entry_point(5, EntryPointKind::DynamicImport),
        entry_point(6, EntryPointKind::DynamicImport),
        entry_point(7, EntryPointKind::DynamicImport),
      ],
    );
    let (_, resolved_draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, &modules, ());
    let (_, resolved_exports) =
      run_infallible_pass(FinalizeResolvedExportsPass, &mut pipeline, &symbols, resolved_draft);
    let wrappers = module_wrappers(&[
      WrapKind::Cjs,
      WrapKind::None,
      WrapKind::None,
      WrapKind::None,
      WrapKind::None,
      WrapKind::None,
      WrapKind::None,
      WrapKind::None,
    ]);
    let dynamic_import_usage = FxHashMap::from_iter([
      (module_idx(1), DynamicImportExportsUsage::Partial(FxHashSet::from_iter(["zeta".into()]))),
      (module_idx(5), DynamicImportExportsUsage::Partial(FxHashSet::from_iter(["zeta".into()]))),
      (module_idx(6), DynamicImportExportsUsage::Complete),
    ]);
    let preserve_signature_overrides = FxHashMap::from_iter([
      (module_idx(2), PreserveEntrySignatures::False),
      (module_idx(4), PreserveEntrySignatures::False),
    ]);

    let (_, roots) = run_infallible_pass(
      CollectEntryExportRootsPass,
      &mut pipeline,
      CollectEntryExportRootsInput {
        module_table: &modules,
        entry_plan: &entry_plan,
        module_wrappers: &wrappers,
        resolved_exports: &resolved_exports,
        dynamic_import_usage: &dynamic_import_usage,
        preserve_signature_overrides: &preserve_signature_overrides,
        default_preserve_signature: PreserveEntrySignatures::Strict,
      },
      (),
    );

    let first = roots.get(module_idx(0)).expect("wrapped entry root");
    assert_eq!(first.len(), 3);
    assert_eq!(
      first[0].symbol_ref,
      modules[module_idx(0)].as_normal().unwrap().namespace_object_ref
    );
    assert!(!first[0].came_from_commonjs);
    assert_eq!(
      first[1..].iter().map(|root| root.symbol_ref).collect::<Vec<_>>(),
      [exports[0].unwrap().0, exports[0].unwrap().1]
    );
    let first_of_multiple = roots.get(module_idx(1)).expect("grouped user entry");
    assert_eq!(first_of_multiple.len(), 2);
    assert_eq!(
      first_of_multiple.iter().map(|root| root.symbol_ref).collect::<Vec<_>>(),
      [exports[1].unwrap().0, exports[1].unwrap().1]
    );
    let imported = roots.get(module_idx(2)).expect("dynamically imported emitted entry");
    assert_eq!(
      imported.iter().map(|root| root.symbol_ref).collect::<Vec<_>>(),
      [exports[2].unwrap().0, exports[2].unwrap().1]
    );
    assert!(roots.get(module_idx(4)).is_none());
    let partial = roots.get(module_idx(5)).expect("partially used dynamic entry");
    assert_eq!(partial.len(), 1);
    assert_eq!(partial[0].symbol_ref, exports[5].unwrap().1);
    assert!(partial[0].came_from_commonjs);
    for module_index in [6, 7] {
      assert_eq!(
        roots
          .get(module_idx(module_index))
          .expect("complete dynamic entry")
          .iter()
          .map(|root| root.symbol_ref)
          .collect::<Vec<_>>(),
        [exports[module_index].unwrap().0, exports[module_index].unwrap().1]
      );
    }
    assert!(roots.get(module_idx(3)).is_none());
    assert_eq!(
      roots.into_entries().map(|(module_idx, _)| module_idx).collect::<Vec<_>>(),
      [module_idx(0), module_idx(1), module_idx(5), module_idx(6), module_idx(7), module_idx(2)]
    );
    assert!(pipeline.into_diagnostics().is_empty());
  }

  #[test]
  fn rejects_unmerged_single_dynamic_import_usage() {
    let mut modules = module_table(vec![normal_module(0, false, Vec::new())]);
    let mut symbols = SymbolRefDb::new();
    let scoping = Scoping::default();
    let root_scope_id = scoping.root_scope_id();
    symbols.store_local_db(
      module_idx(0),
      SymbolRefDbForModule::new(scoping, module_idx(0), root_scope_id),
    );
    assert_eq!(
      symbols.create_facade_root_symbol_ref(module_idx(0), "namespace"),
      modules[module_idx(0)].as_normal().unwrap().namespace_object_ref
    );
    let value = symbols.create_facade_root_symbol_ref(module_idx(0), "value");
    modules[module_idx(0)].as_normal_mut().unwrap().named_exports.insert(
      "value".into(),
      LocalExport { span: Span::new(1, 2), referenced: value, came_from_commonjs: false },
    );
    let mut pipeline = PassPipelineCtx::new();
    let (_, entry_plan) = run_infallible_pass(
      CanonicalizeEntriesPass,
      &mut pipeline,
      &modules,
      vec![entry_point(0, EntryPointKind::DynamicImport)],
    );
    let (_, resolved_draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, &modules, ());
    let (_, resolved_exports) =
      run_infallible_pass(FinalizeResolvedExportsPass, &mut pipeline, &symbols, resolved_draft);
    let wrappers = module_wrappers(&[WrapKind::None]);
    let dynamic_import_usage =
      FxHashMap::from_iter([(module_idx(0), DynamicImportExportsUsage::Single("value".into()))]);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      run_infallible_pass(
        CollectEntryExportRootsPass,
        &mut pipeline,
        CollectEntryExportRootsInput {
          module_table: &modules,
          entry_plan: &entry_plan,
          module_wrappers: &wrappers,
          resolved_exports: &resolved_exports,
          dynamic_import_usage: &dynamic_import_usage,
          preserve_signature_overrides: &FxHashMap::default(),
          default_preserve_signature: PreserveEntrySignatures::Strict,
        },
        (),
      )
    }));
    assert!(result.is_err());
  }

  #[test]
  fn rejects_wrapper_layout_mismatch_before_collecting_roots() {
    let modules = module_table(vec![normal_module(0, false, Vec::new())]);
    let symbols = SymbolRefDb::new();
    let mut pipeline = PassPipelineCtx::new();
    let (_, entry_plan) =
      run_infallible_pass(CanonicalizeEntriesPass, &mut pipeline, &modules, Vec::new());
    let (_, resolved_draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, &modules, ());
    let (_, resolved_exports) =
      run_infallible_pass(FinalizeResolvedExportsPass, &mut pipeline, &symbols, resolved_draft);
    let wrappers = module_wrappers(&[]);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      run_infallible_pass(
        CollectEntryExportRootsPass,
        &mut pipeline,
        CollectEntryExportRootsInput {
          module_table: &modules,
          entry_plan: &entry_plan,
          module_wrappers: &wrappers,
          resolved_exports: &resolved_exports,
          dynamic_import_usage: &FxHashMap::default(),
          preserve_signature_overrides: &FxHashMap::default(),
          default_preserve_signature: PreserveEntrySignatures::Strict,
        },
        (),
      )
    }));
    assert!(result.is_err());
  }
}
