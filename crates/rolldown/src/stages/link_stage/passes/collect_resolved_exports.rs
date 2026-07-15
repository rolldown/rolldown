use std::convert::Infallible;

use oxc_index::IndexVec;
use oxc_str::CompactStr;
use rolldown_common::{
  EcmaModuleAstUsage, IndexModules, Module, ModuleIdx, ModuleTable, ResolvedExport,
};
use rolldown_utils::{
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
  rayon::{IntoParallelIterator, ParallelIterator},
};
use rustc_hash::FxHashMap;

use super::CollectResolvedExportsPass;

// See internal-docs/linking/resolved-exports/implementation.md.

pub(in crate::stages::link_stage) struct ResolvedExportsDraft {
  slots: IndexVec<ModuleIdx, Option<FxHashMap<CompactStr, ResolvedExport>>>,
}

impl ResolvedExportsDraft {
  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.slots.len()
  }

  pub(in crate::stages::link_stage) fn into_slots(
    self,
  ) -> IndexVec<ModuleIdx, Option<FxHashMap<CompactStr, ResolvedExport>>> {
    self.slots
  }
}

fn add_exports_for_export_star(
  modules: &IndexModules,
  resolved_exports: &mut FxHashMap<CompactStr, ResolvedExport>,
  module_idx: ModuleIdx,
  module_stack: &mut Vec<ModuleIdx>,
) {
  if module_stack.contains(&module_idx) {
    return;
  }

  module_stack.push(module_idx);

  let Module::Normal(module) = &modules[module_idx] else {
    return;
  };

  let cjs_reexport_modules: Vec<ModuleIdx> =
    if module.ast_usage.contains(EcmaModuleAstUsage::IsCjsReexport) {
      module
        .ecma_view
        .cjs_reexport_import_record_ids
        .iter()
        .filter_map(|&record_idx| module.import_records[record_idx].resolved_module)
        .collect()
    } else {
      Vec::new()
    };

  // Ordinary export-star records are always observed before the separate CJS reexport vector.
  for dependency_idx in module.star_export_module_ids().chain(cjs_reexport_modules) {
    let Module::Normal(dependency) = &modules[dependency_idx] else {
      continue;
    };

    for (exported_name, named_export) in &dependency.named_exports {
      // ESM export-star skips `default`, while CJS-derived default properties remain observable.
      if exported_name.as_str() == "default" && !named_export.came_from_commonjs {
        continue;
      }
      // A direct export anywhere on this DFS path shadows a deeper star export. The stack is
      // deliberately path-local, so a shared dependency must be revisited through another path.
      if module_stack
        .iter()
        .filter_map(|module_idx| modules[*module_idx].as_normal())
        .any(|module| module.named_exports.contains_key(exported_name))
      {
        continue;
      }
      if let Some(resolved_export) = resolved_exports.get_mut(exported_name) {
        if named_export.referenced != resolved_export.symbol_ref {
          // Keep raw refs and encounter order here. Binding classifies canonical ESM ambiguity only
          // after all symbol links have been committed.
          if resolved_export.came_from_commonjs || named_export.came_from_commonjs {
            resolved_export
              .cjs_conflicting_symbol_refs
              .get_or_insert(Box::default())
              .push(named_export.referenced);
          } else {
            resolved_export
              .potentially_ambiguous_symbol_refs
              .get_or_insert(Box::default())
              .push(named_export.referenced);
          }
        }
      } else {
        resolved_exports.insert(
          exported_name.clone(),
          ResolvedExport::new(named_export.referenced, named_export.came_from_commonjs),
        );
      }
    }

    add_exports_for_export_star(modules, resolved_exports, dependency_idx, module_stack);
  }

  module_stack.pop();
}

fn collect_for_module(
  modules: &IndexModules,
  module_idx: ModuleIdx,
) -> Option<FxHashMap<CompactStr, ResolvedExport>> {
  let Module::Normal(module) = &modules[module_idx] else {
    return None;
  };
  let mut resolved_exports = module
    .named_exports
    .iter()
    .map(|(name, local)| {
      (name.clone(), ResolvedExport::new(local.referenced, local.came_from_commonjs))
    })
    .collect::<FxHashMap<_, _>>();

  if module.has_star_export() || module.ast_usage.contains(EcmaModuleAstUsage::IsCjsReexport) {
    add_exports_for_export_star(modules, &mut resolved_exports, module_idx, &mut Vec::new());
  }

  Some(resolved_exports)
}

impl Pass for CollectResolvedExportsPass {
  type InputRead<'a> = &'a ModuleTable;
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = ResolvedExportsDraft;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    module_table: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let slots = IndexVec::from_vec(
      (0..module_table.modules.len())
        .into_par_iter()
        .map(|index| collect_for_module(&module_table.modules, ModuleIdx::new(index)))
        .collect::<Vec<_>>(),
    );

    Ok(token.finish((), ResolvedExportsDraft { slots }))
  }
}

#[cfg(test)]
mod tests {
  use oxc::{semantic::SymbolId, span::Span};
  use rolldown_common::{
    EcmaModuleAstUsage, EcmaViewMeta, ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta,
    LocalExport, ModuleTable, SymbolRef, bundler_options::ModuleType,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::{
    CollectResolvedExportsPass,
    test_utils::{external_module, module_idx, module_table, normal_module},
  };
  use super::ResolvedExportsDraft;

  fn symbol(owner: usize, symbol: usize) -> SymbolRef {
    SymbolRef { owner: module_idx(owner), symbol: SymbolId::new(symbol) }
  }

  fn insert_export(
    modules: &mut ModuleTable,
    owner: usize,
    name: &str,
    symbol_id: usize,
    came_from_commonjs: bool,
  ) {
    let start = u32::try_from(symbol_id).expect("fixture symbol fits in a span offset");
    modules[module_idx(owner)].as_normal_mut().expect("normal export owner").named_exports.insert(
      name.into(),
      LocalExport {
        span: Span::new(start, start + 1),
        referenced: symbol(owner, symbol_id),
        came_from_commonjs,
      },
    );
  }

  fn mark_star(modules: &mut ModuleTable, importer: usize, record: usize) {
    let module = modules[module_idx(importer)].as_normal_mut().expect("normal importer");
    module.meta.insert(EcmaViewMeta::HasStarExport);
    module.import_records[ImportRecordIdx::from_usize(record)]
      .meta
      .insert(ImportRecordMeta::IsExportStar);
  }

  fn mark_cjs_reexport(modules: &mut ModuleTable, importer: usize, record: usize) {
    let module = modules[module_idx(importer)].as_normal_mut().expect("normal importer");
    module.ast_usage.insert(EcmaModuleAstUsage::IsCjsReexport);
    module.cjs_reexport_import_record_ids.push(ImportRecordIdx::from_usize(record));
  }

  fn collect(modules: &ModuleTable) -> ResolvedExportsDraft {
    let mut pipeline = PassPipelineCtx::new();
    let (_, exports) = run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, modules, ());
    assert!(pipeline.into_diagnostics().is_empty());
    exports
  }

  fn module_exports(
    exports: &ResolvedExportsDraft,
    module: usize,
  ) -> &rustc_hash::FxHashMap<oxc_str::CompactStr, rolldown_common::ResolvedExport> {
    exports.slots[module_idx(module)].as_ref().expect("normal module exports")
  }

  #[test]
  fn preserves_physical_slots_for_normal_empty_and_external_modules() {
    let mut modules = module_table(vec![
      normal_module(0, false, Vec::new()),
      external_module(1, "external"),
      normal_module(2, false, Vec::new()),
    ]);
    insert_export(&mut modules, 0, "local", 1, false);

    let exports = collect(&modules);
    assert_eq!(exports.module_count(), 3);
    assert_eq!(module_exports(&exports, 0)["local"].symbol_ref, symbol(0, 1));
    assert!(exports.slots[module_idx(1)].is_none());
    assert!(module_exports(&exports, 2).is_empty());
  }

  #[test]
  fn keys_dfs_roots_by_physical_slot_instead_of_the_embedded_module_idx() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(0, 1))]),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
    ]);
    mark_star(&mut modules, 0, 0);
    insert_export(&mut modules, 1, "value", 1, false);
    modules[module_idx(0)].as_normal_mut().expect("physical root").idx = module_idx(2);

    let exports = collect(&modules);
    assert_eq!(module_exports(&exports, 0)["value"].symbol_ref, symbol(1, 1));
  }

  #[test]
  fn preserves_star_import_record_order_for_the_primary_symbol() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(2), Span::new(0, 1)),
          (ImportKind::Import, Some(1), Span::new(1, 2)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
    ]);
    mark_star(&mut modules, 0, 0);
    mark_star(&mut modules, 0, 1);
    insert_export(&mut modules, 1, "value", 1, false);
    insert_export(&mut modules, 2, "value", 2, false);

    let exports = collect(&modules);
    let value = &module_exports(&exports, 0)["value"];
    assert_eq!(value.symbol_ref, symbol(2, 2));
    assert_eq!(
      value.potentially_ambiguous_symbol_refs.as_deref().map(Vec::as_slice),
      Some([symbol(1, 1)].as_slice())
    );
  }

  #[test]
  fn a_real_export_in_the_dfs_stack_shadows_deeper_star_exports() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(0, 1))]),
      normal_module(1, false, vec![(ImportKind::Import, Some(2), Span::new(1, 2))]),
      normal_module(2, false, Vec::new()),
    ]);
    mark_star(&mut modules, 0, 0);
    mark_star(&mut modules, 1, 0);
    insert_export(&mut modules, 1, "value", 1, false);
    insert_export(&mut modules, 2, "value", 2, false);

    let exports = collect(&modules);
    let value = &module_exports(&exports, 0)["value"];
    assert_eq!(value.symbol_ref, symbol(1, 1));
    assert!(value.potentially_ambiguous_symbol_refs.is_none());
  }

  #[test]
  fn terminates_star_cycles_without_dropping_reachable_exports() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(0, 1))]),
      normal_module(1, false, vec![(ImportKind::Import, Some(0), Span::new(1, 2))]),
    ]);
    mark_star(&mut modules, 0, 0);
    mark_star(&mut modules, 1, 0);
    insert_export(&mut modules, 1, "reachable", 1, false);

    let exports = collect(&modules);
    assert_eq!(module_exports(&exports, 0)["reachable"].symbol_ref, symbol(1, 1));
  }

  #[test]
  fn revisits_shared_dependencies_per_path_while_terminating_cycles() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(0, 1)),
          (ImportKind::Import, Some(2), Span::new(1, 2)),
        ],
      ),
      normal_module(1, false, vec![(ImportKind::Import, Some(3), Span::new(2, 3))]),
      normal_module(2, false, vec![(ImportKind::Import, Some(3), Span::new(3, 4))]),
      normal_module(
        3,
        false,
        vec![
          (ImportKind::Import, Some(4), Span::new(4, 5)),
          (ImportKind::Import, Some(0), Span::new(5, 6)),
        ],
      ),
      normal_module(4, false, Vec::new()),
    ]);
    mark_star(&mut modules, 0, 0);
    mark_star(&mut modules, 0, 1);
    mark_star(&mut modules, 1, 0);
    mark_star(&mut modules, 2, 0);
    mark_star(&mut modules, 3, 0);
    mark_star(&mut modules, 3, 1);
    insert_export(&mut modules, 1, "x", 1, false);
    insert_export(&mut modules, 4, "x", 4, false);
    insert_export(&mut modules, 4, "y", 5, false);

    let exports = collect(&modules);
    let root = module_exports(&exports, 0);
    assert_eq!(root["x"].symbol_ref, symbol(1, 1));
    assert_eq!(
      root["x"].potentially_ambiguous_symbol_refs.as_deref().map(Vec::as_slice),
      Some([symbol(4, 4)].as_slice())
    );
    assert_eq!(root["y"].symbol_ref, symbol(4, 5));
    assert!(root["y"].potentially_ambiguous_symbol_refs.is_none());
  }

  #[test]
  fn skips_esm_default_but_keeps_commonjs_default_from_star_exports() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(0, 1)),
          (ImportKind::Import, Some(2), Span::new(1, 2)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
    ]);
    mark_star(&mut modules, 0, 0);
    mark_star(&mut modules, 0, 1);
    insert_export(&mut modules, 1, "default", 1, false);
    insert_export(&mut modules, 2, "default", 2, true);
    modules[module_idx(2)].as_normal_mut().expect("CommonJS source").exports_kind =
      ExportsKind::CommonJs;

    let exports = collect(&modules);
    let default = &module_exports(&exports, 0)["default"];
    assert_eq!(default.symbol_ref, symbol(2, 2));
    assert!(default.came_from_commonjs);
  }

  #[test]
  fn same_symbol_from_esm_and_commonjs_sources_is_not_a_conflict() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(0, 1)),
          (ImportKind::Import, Some(2), Span::new(1, 2)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
    ]);
    mark_star(&mut modules, 0, 0);
    mark_star(&mut modules, 0, 1);
    insert_export(&mut modules, 1, "shared", 1, false);
    insert_export(&mut modules, 2, "shared", 2, true);
    modules[module_idx(2)]
      .as_normal_mut()
      .expect("second source")
      .named_exports
      .get_mut("shared")
      .expect("shared export")
      .referenced = symbol(1, 1);

    let exports = collect(&modules);
    let shared = &module_exports(&exports, 0)["shared"];
    assert_eq!(shared.symbol_ref, symbol(1, 1));
    assert!(!shared.came_from_commonjs);
    assert!(shared.potentially_ambiguous_symbol_refs.is_none());
    assert!(shared.cjs_conflicting_symbol_refs.is_none());
  }

  #[test]
  fn separates_esm_ambiguity_from_both_commonjs_conflict_orders() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(0, 1)),
          (ImportKind::Import, Some(2), Span::new(1, 2)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
    ]);
    mark_star(&mut modules, 0, 0);
    mark_star(&mut modules, 0, 1);
    insert_export(&mut modules, 1, "esm", 1, false);
    insert_export(&mut modules, 2, "esm", 2, false);
    insert_export(&mut modules, 1, "conditional", 3, false);
    insert_export(&mut modules, 2, "conditional", 4, true);
    insert_export(&mut modules, 1, "cjs_primary", 5, true);
    insert_export(&mut modules, 2, "cjs_primary", 6, false);

    let exports = collect(&modules);
    let root = module_exports(&exports, 0);
    assert_eq!(
      root["esm"].potentially_ambiguous_symbol_refs.as_deref().map(Vec::as_slice),
      Some([symbol(2, 2)].as_slice())
    );
    assert!(root["esm"].cjs_conflicting_symbol_refs.is_none());
    assert_eq!(
      root["conditional"].cjs_conflicting_symbol_refs.as_deref().map(Vec::as_slice),
      Some([symbol(2, 4)].as_slice())
    );
    assert!(root["conditional"].potentially_ambiguous_symbol_refs.is_none());
    let cjs_primary = &root["cjs_primary"];
    assert_eq!(cjs_primary.symbol_ref, symbol(1, 5));
    assert!(cjs_primary.came_from_commonjs);
    assert_eq!(
      cjs_primary.cjs_conflicting_symbol_refs.as_deref().map(Vec::as_slice),
      Some([symbol(2, 6)].as_slice())
    );
    assert!(cjs_primary.potentially_ambiguous_symbol_refs.is_none());
  }

  #[test]
  fn preserves_ordinary_and_cjs_source_order_and_primary_provenance() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Require, Some(4), Span::new(0, 1)),
          (ImportKind::Import, Some(1), Span::new(1, 2)),
          (ImportKind::Import, Some(2), Span::new(2, 3)),
          (ImportKind::Require, Some(3), Span::new(3, 4)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
      normal_module(3, false, Vec::new()),
      normal_module(4, false, Vec::new()),
    ]);
    mark_star(&mut modules, 0, 1);
    mark_star(&mut modules, 0, 2);
    mark_cjs_reexport(&mut modules, 0, 3);
    mark_cjs_reexport(&mut modules, 0, 0);
    insert_export(&mut modules, 1, "value", 1, false);
    insert_export(&mut modules, 2, "value", 2, false);
    insert_export(&mut modules, 3, "value", 3, true);
    insert_export(&mut modules, 4, "value", 4, true);

    let exports = collect(&modules);
    let value = &module_exports(&exports, 0)["value"];
    assert_eq!(value.symbol_ref, symbol(1, 1));
    assert!(!value.came_from_commonjs);
    assert_eq!(
      value.potentially_ambiguous_symbol_refs.as_deref().map(Vec::as_slice),
      Some([symbol(2, 2)].as_slice())
    );
    assert_eq!(
      value.cjs_conflicting_symbol_refs.as_deref().map(Vec::as_slice),
      Some([symbol(3, 3), symbol(4, 4)].as_slice())
    );
  }

  #[test]
  fn follows_cjs_reexports_in_recursive_modules_without_an_ordinary_star_there() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(0, 1))]),
      normal_module(1, false, vec![(ImportKind::Require, Some(2), Span::new(1, 2))]),
      normal_module(2, false, Vec::new()),
    ]);
    mark_star(&mut modules, 0, 0);
    mark_cjs_reexport(&mut modules, 1, 0);
    insert_export(&mut modules, 2, "default", 2, true);
    modules[module_idx(2)].as_normal_mut().expect("CommonJS source").exports_kind =
      ExportsKind::CommonJs;

    let exports = collect(&modules);
    let default = &module_exports(&exports, 0)["default"];
    assert_eq!(default.symbol_ref, symbol(2, 2));
    assert!(default.came_from_commonjs);
    assert_eq!(module_exports(&exports, 1)["default"].symbol_ref, symbol(2, 2));
  }

  #[test]
  fn ignores_external_and_unresolved_star_and_cjs_reexport_edges() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(0, 1)),
          (ImportKind::Import, None, Span::new(1, 2)),
          (ImportKind::Require, Some(1), Span::new(2, 3)),
          (ImportKind::Require, None, Span::new(3, 4)),
        ],
      ),
      external_module(1, "external"),
    ]);
    insert_export(&mut modules, 0, "local", 1, false);
    mark_star(&mut modules, 0, 0);
    mark_star(&mut modules, 0, 1);
    mark_cjs_reexport(&mut modules, 0, 2);
    mark_cjs_reexport(&mut modules, 0, 3);

    let exports = collect(&modules);
    let root = module_exports(&exports, 0);
    assert_eq!(root.len(), 1);
    assert_eq!(root["local"].symbol_ref, symbol(0, 1));
    assert!(exports.slots[module_idx(1)].is_none());
  }

  #[test]
  fn reads_named_exports_from_the_final_module_table() {
    let mut modules = module_table(vec![normal_module(0, false, Vec::new())]);
    modules[module_idx(0)].as_normal_mut().expect("normal module").module_type = ModuleType::Json;
    insert_export(&mut modules, 0, "normalized", 1, false);

    let exports = collect(&modules);
    assert_eq!(module_exports(&exports, 0)["normalized"].symbol_ref, symbol(0, 1));
  }
}
