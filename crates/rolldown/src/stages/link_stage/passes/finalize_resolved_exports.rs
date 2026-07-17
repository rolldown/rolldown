use std::convert::Infallible;

use oxc_index::IndexVec;
use oxc_str::CompactStr;
use rolldown_common::{ModuleIdx, ResolvedExport, SymbolRefDb};
use rolldown_utils::{
  indexmap::FxIndexMap,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
  rayon::{IntoParallelIterator, ParallelIterator},
};
use rustc_hash::FxHashMap;

use super::{FinalizeResolvedExportsPass, ResolvedExportsDraft};

// See internal-docs/linking/resolved-exports/implementation.md.

pub(in crate::stages::link_stage) struct ResolvedExportsForModule {
  resolved: FxHashMap<CompactStr, ResolvedExport>,
  sorted_and_non_ambiguous: FxIndexMap<CompactStr, bool>,
}

impl ResolvedExportsForModule {
  pub(in crate::stages::link_stage) fn into_parts(
    self,
  ) -> (FxHashMap<CompactStr, ResolvedExport>, FxIndexMap<CompactStr, bool>) {
    (self.resolved, self.sorted_and_non_ambiguous)
  }
}

pub(in crate::stages::link_stage) struct ResolvedExports {
  slots: IndexVec<ModuleIdx, Option<ResolvedExportsForModule>>,
}

impl ResolvedExports {
  pub(in crate::stages::link_stage) fn has_normal_slot(&self, module_idx: ModuleIdx) -> bool {
    self.slots.get(module_idx).is_some_and(Option::is_some)
  }

  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.slots.len()
  }

  pub(in crate::stages::link_stage) fn get(
    &self,
    module_idx: ModuleIdx,
    name: &str,
  ) -> Option<&ResolvedExport> {
    self
      .slots
      .get(module_idx)
      .and_then(Option::as_ref)
      .and_then(|exports| exports.resolved.get(name))
  }

  pub(in crate::stages::link_stage) fn iter(
    &self,
    module_idx: ModuleIdx,
  ) -> impl Iterator<Item = (&CompactStr, &ResolvedExport)> {
    self
      .slots
      .get(module_idx)
      .and_then(Option::as_ref)
      .into_iter()
      .flat_map(|exports| exports.resolved.iter())
  }

  pub(in crate::stages::link_stage) fn contains_canonical_name(
    &self,
    module_idx: ModuleIdx,
    name: &str,
  ) -> bool {
    self
      .slots
      .get(module_idx)
      .and_then(Option::as_ref)
      .is_some_and(|exports| exports.sorted_and_non_ambiguous.contains_key(name))
  }

  pub(in crate::stages::link_stage) fn canonical_exports(
    &self,
    module_idx: ModuleIdx,
    needs_commonjs_export: bool,
  ) -> impl Iterator<Item = (&CompactStr, &ResolvedExport)> {
    self.slots.get(module_idx).and_then(Option::as_ref).into_iter().flat_map(move |exports| {
      exports.sorted_and_non_ambiguous.iter().filter_map(move |(name, came_from_commonjs)| {
        (needs_commonjs_export || !came_from_commonjs).then_some((name, &exports.resolved[name]))
      })
    })
  }

  pub(in crate::stages::link_stage) fn canonical_exports_is_empty(
    &self,
    module_idx: ModuleIdx,
  ) -> bool {
    self
      .slots
      .get(module_idx)
      .and_then(Option::as_ref)
      .is_none_or(|exports| exports.sorted_and_non_ambiguous.is_empty())
  }

  pub(in crate::stages::link_stage) fn into_slots(
    self,
  ) -> IndexVec<ModuleIdx, Option<ResolvedExportsForModule>> {
    self.slots
  }
}

fn finalize_module(
  symbols: &SymbolRefDb,
  resolved: FxHashMap<CompactStr, ResolvedExport>,
) -> ResolvedExportsForModule {
  let mut sorted_and_non_ambiguous = Vec::new();
  'next_export: for (exported_name, resolved_export) in &resolved {
    if let Some(potentially_ambiguous_symbol_refs) =
      &resolved_export.potentially_ambiguous_symbol_refs
    {
      let main_ref = symbols.canonical_ref_for(resolved_export.symbol_ref);
      for ambiguous_ref in potentially_ambiguous_symbol_refs.iter() {
        if main_ref != symbols.canonical_ref_for(*ambiguous_ref) {
          continue 'next_export;
        }
      }
    }
    sorted_and_non_ambiguous.push((exported_name.clone(), resolved_export.came_from_commonjs));
  }
  sorted_and_non_ambiguous.sort_unstable();
  ResolvedExportsForModule {
    resolved,
    sorted_and_non_ambiguous: FxIndexMap::from_iter(sorted_and_non_ambiguous),
  }
}

impl Pass for FinalizeResolvedExportsPass {
  type InputRead<'a> = &'a SymbolRefDb;
  type InputOwned = ResolvedExportsDraft;
  type OutputRead = ();
  type OutputOwned = ResolvedExports;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    symbols: Self::InputRead<'_>,
    resolved_exports: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let slots = IndexVec::from_vec(
      resolved_exports
        .into_slots()
        .into_par_iter()
        .map(|slot| slot.map(|resolved| finalize_module(symbols, resolved)))
        .collect(),
    );
    Ok(token.finish((), ResolvedExports { slots }))
  }
}

#[cfg(test)]
mod tests {
  use oxc::{semantic::Scoping, span::Span};
  use rolldown_common::{
    LocalExport, ModuleIdx, ModuleTable, SymbolRef, SymbolRefDb, SymbolRefDbForModule,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::{
    CollectResolvedExportsPass, FinalizeResolvedExportsPass,
    collect_resolved_exports::test_support::set_conflicts,
    test_utils::{external_module, module_idx, module_table, normal_module},
  };
  use super::ResolvedExports;

  fn symbols_for(modules: &ModuleTable) -> SymbolRefDb {
    let mut symbols = SymbolRefDb::new();
    for (module_idx, module) in modules.modules.iter_enumerated() {
      if module.as_normal().is_some() {
        let scoping = Scoping::default();
        let root_scope_id = scoping.root_scope_id();
        symbols.store_local_db(
          module_idx,
          SymbolRefDbForModule::new(scoping, module_idx, root_scope_id),
        );
      }
    }
    symbols
  }

  fn insert_export(
    modules: &mut ModuleTable,
    module_idx: ModuleIdx,
    name: &str,
    symbol_ref: SymbolRef,
    came_from_commonjs: bool,
  ) {
    let span_start = u32::try_from(symbol_ref.symbol.index()).expect("test symbol index fits u32");
    modules[module_idx].as_normal_mut().expect("normal export owner").named_exports.insert(
      name.into(),
      LocalExport {
        span: Span::new(span_start, span_start + 1),
        referenced: symbol_ref,
        came_from_commonjs,
      },
    );
  }

  fn collect_and_finalize(modules: &ModuleTable, symbols: &SymbolRefDb) -> ResolvedExports {
    let mut pipeline = PassPipelineCtx::new();
    let (_, draft) = run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, modules, ());
    let (_, resolved) =
      run_infallible_pass(FinalizeResolvedExportsPass, &mut pipeline, symbols, draft);
    assert!(pipeline.into_diagnostics().is_empty());
    resolved
  }

  #[test]
  fn preserves_normal_empty_external_and_physical_slots() {
    let mut modules = module_table(vec![
      normal_module(0, false, Vec::new()),
      external_module(1, "external"),
      normal_module(2, false, Vec::new()),
    ]);
    let mut symbols = symbols_for(&modules);
    let local = symbols.create_facade_root_symbol_ref(module_idx(0), "local");
    insert_export(&mut modules, module_idx(0), "local", local, false);

    let resolved = collect_and_finalize(&modules, &symbols);
    assert_eq!(resolved.module_count(), 3);
    assert_eq!(resolved.get(module_idx(0), "local").map(|export| export.symbol_ref), Some(local));
    assert_eq!(resolved.iter(module_idx(1)).count(), 0);
    assert_eq!(resolved.iter(module_idx(2)).count(), 0);
    assert!(resolved.contains_canonical_name(module_idx(0), "local"));
    assert!(!resolved.contains_canonical_name(module_idx(1), "local"));
  }

  #[test]
  fn sorts_names_and_preserves_primary_provenance() {
    let mut modules = module_table(vec![normal_module(0, false, Vec::new())]);
    let mut symbols = symbols_for(&modules);
    let zed = symbols.create_facade_root_symbol_ref(module_idx(0), "zed");
    let alpha = symbols.create_facade_root_symbol_ref(module_idx(0), "alpha");
    insert_export(&mut modules, module_idx(0), "zed", zed, false);
    insert_export(&mut modules, module_idx(0), "alpha", alpha, true);

    let resolved = collect_and_finalize(&modules, &symbols);
    let slot = resolved.slots[module_idx(0)].as_ref().expect("normal slot");
    assert_eq!(
      slot
        .sorted_and_non_ambiguous
        .iter()
        .map(|(name, from_cjs)| (name.as_str(), *from_cjs))
        .collect::<Vec<_>>(),
      [("alpha", true), ("zed", false)]
    );
    assert_eq!(resolved.get(module_idx(0), "alpha").map(|export| export.symbol_ref), Some(alpha));
  }

  #[test]
  fn keeps_direct_and_multihop_canonical_equal_ambiguity() {
    let mut modules = module_table(vec![normal_module(0, false, Vec::new())]);
    let mut symbols = symbols_for(&modules);
    let primary = symbols.create_facade_root_symbol_ref(module_idx(0), "primary");
    let direct = symbols.create_facade_root_symbol_ref(module_idx(0), "direct");
    let middle = symbols.create_facade_root_symbol_ref(module_idx(0), "middle");
    let multihop = symbols.create_facade_root_symbol_ref(module_idx(0), "multihop");
    insert_export(&mut modules, module_idx(0), "value", primary, false);
    symbols.link(direct, primary);
    symbols.link(multihop, middle);
    symbols.link(middle, primary);

    let mut pipeline = PassPipelineCtx::new();
    let (_, mut draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, &modules, ());
    set_conflicts(&mut draft, module_idx(0), "value", Some(vec![direct, multihop]), None);
    let (_, resolved) =
      run_infallible_pass(FinalizeResolvedExportsPass, &mut pipeline, &symbols, draft);

    assert!(resolved.contains_canonical_name(module_idx(0), "value"));
    assert_eq!(resolved.get(module_idx(0), "value").map(|export| export.symbol_ref), Some(primary));
  }

  #[test]
  fn excludes_a_name_when_any_esm_ambiguity_stays_distinct() {
    let mut modules = module_table(vec![normal_module(0, false, Vec::new())]);
    let mut symbols = symbols_for(&modules);
    let primary = symbols.create_facade_root_symbol_ref(module_idx(0), "primary");
    let same = symbols.create_facade_root_symbol_ref(module_idx(0), "same");
    let distinct = symbols.create_facade_root_symbol_ref(module_idx(0), "distinct");
    insert_export(&mut modules, module_idx(0), "value", primary, false);
    symbols.link(same, primary);

    let mut pipeline = PassPipelineCtx::new();
    let (_, mut draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, &modules, ());
    set_conflicts(&mut draft, module_idx(0), "value", Some(vec![same, distinct]), None);
    let (_, resolved) =
      run_infallible_pass(FinalizeResolvedExportsPass, &mut pipeline, &symbols, draft);

    assert!(!resolved.contains_canonical_name(module_idx(0), "value"));
    assert_eq!(resolved.get(module_idx(0), "value").map(|export| export.symbol_ref), Some(primary));
  }

  #[test]
  fn ignores_cjs_conflicts_and_does_not_rewrite_raw_fields() {
    let mut modules = module_table(vec![normal_module(0, false, Vec::new())]);
    let mut symbols = symbols_for(&modules);
    let primary = symbols.create_facade_root_symbol_ref(module_idx(0), "primary");
    let cjs_conflict = symbols.create_facade_root_symbol_ref(module_idx(0), "cjs_conflict");
    let ambiguous = symbols.create_facade_root_symbol_ref(module_idx(0), "ambiguous");
    insert_export(&mut modules, module_idx(0), "value", primary, true);

    let mut pipeline = PassPipelineCtx::new();
    let (_, mut draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, &modules, ());
    set_conflicts(
      &mut draft,
      module_idx(0),
      "value",
      Some(vec![ambiguous]),
      Some(vec![cjs_conflict]),
    );
    symbols.link(ambiguous, primary);
    let (_, resolved) =
      run_infallible_pass(FinalizeResolvedExportsPass, &mut pipeline, &symbols, draft);

    let export = resolved.get(module_idx(0), "value").expect("raw export retained");
    assert_eq!(export.symbol_ref, primary);
    assert!(export.came_from_commonjs);
    assert_eq!(
      export.potentially_ambiguous_symbol_refs.as_deref().map(Vec::as_slice),
      Some([ambiguous].as_slice())
    );
    assert_eq!(
      export.cjs_conflicting_symbol_refs.as_deref().map(Vec::as_slice),
      Some([cjs_conflict].as_slice())
    );
    assert!(resolved.contains_canonical_name(module_idx(0), "value"));
  }

  #[test]
  fn sorted_keys_are_always_a_subset_of_the_raw_map() {
    let mut modules = module_table(vec![normal_module(0, false, Vec::new())]);
    let mut symbols = symbols_for(&modules);
    for name in ["gamma", "alpha", "beta"] {
      let symbol = symbols.create_facade_root_symbol_ref(module_idx(0), name);
      insert_export(&mut modules, module_idx(0), name, symbol, false);
    }

    let resolved = collect_and_finalize(&modules, &symbols);
    let slot = resolved.slots[module_idx(0)].as_ref().expect("normal slot");
    assert!(slot.sorted_and_non_ambiguous.keys().all(|name| slot.resolved.contains_key(name)));
  }
}
