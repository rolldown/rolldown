#[cfg(target_family = "wasm")]
use itertools::Itertools as _;
use oxc_index::IndexVec;
use rolldown_common::{
  ConstExportMeta, ImportRecordMeta, IndexModules, MemberExprRef, MemberExprRefResolution, Module,
  ModuleIdx, ModuleNamespaceIncludedReason, NormalModule, NormalizedBundlerOptions,
  RUNTIME_MODULE_ID, RuntimeHelper, RuntimeModuleBrief, SymbolRef, SymbolRefDb,
  UsedExternalSymbols, UsedSymbolRefsBuilder, WrapKind,
};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::rayon::{
  IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};

use rolldown_utils::pass::Sealed;
use rolldown_utils::{IndexBitSet, indexmap::FxIndexMap};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  stages::link_stage::{
    LinkStage,
    passes::{EntryExportRoots, StatementRuntimeRequirements, UnreachableDynamicImports},
  },
  type_alias::IndexStmtInfos,
  types::linking_metadata::LinkingMetadataVec,
};

use super::{
  inclusion_core::{
    InclusionConfig, InclusionCoreContext, InclusionFacts, InclusionModuleFacts, TreeShakingConfig,
    WorkItem, include_cjs_bailout_exports as include_cjs_bailout_exports_core,
    include_declaring_statements as include_declaring_statements_core,
    include_module as include_module_core, include_runtime_symbol as include_runtime_symbol_core,
    include_symbol as include_symbol_core,
    include_symbol_and_check_cjs_bailout as include_symbol_and_check_cjs_bailout_core,
    preserve_reexported_interfaces as preserve_reexported_interfaces_core,
  },
  on_demand::compute_body_demand_keys,
  passes::{
    collect_depended_runtime_helpers, include_cjs_bailout_exports, include_runtime_symbol,
    preserve_reexported_interfaces,
  },
};

pub use super::inclusion_core::{
  ModuleInclusionVec, ModuleNamespaceReasonVec, StmtInclusionVec, SymbolIncludeReason,
};

pub struct IncludeContext<'a> {
  pub modules: &'a IndexModules,
  /// Per-module statement-info table, detached from `EcmaView` and held on
  /// `LinkStage` for the duration of the link/generate stages.
  pub stmt_infos: &'a IndexStmtInfos,
  pub symbols: &'a SymbolRefDb,
  pub is_included_vec: &'a mut StmtInclusionVec,
  pub is_module_included_vec: &'a mut ModuleInclusionVec,
  pub tree_shaking: bool,
  pub inline_const_smart: bool,
  pub runtime_idx: ModuleIdx,
  pub metas: &'a LinkingMetadataVec,
  pub used_symbol_refs: &'a mut UsedSymbolRefsBuilder,
  pub used_external_symbols: &'a mut UsedExternalSymbols,
  pub constant_symbol_map: &'a FxHashMap<SymbolRef, ConstExportMeta>,
  pub options: &'a NormalizedBundlerOptions,
  pub normal_symbol_exports_chain_map: &'a FxHashMap<SymbolRef, Vec<SymbolRef>>,
  /// It is necessary since we can't mutate `module.meta` during the tree shaking process.
  /// see [rolldown_common::ecmascript::ecma_view::EcmaViewMeta]
  pub bailout_cjs_tree_shaking_modules: FxHashSet<ModuleIdx>,
  /// Tracks whether any new module was included during the current convergence iteration.
  /// Used to detect fixpoint without O(N) scanning of `is_module_included_vec`.
  pub module_inclusion_changed: bool,
  pub module_namespace_included_reason: &'a mut ModuleNamespaceReasonVec,
  pub json_module_none_self_reference_included_symbol: FxHashMap<ModuleIdx, FxHashSet<SymbolRef>>,
  /// User-defined entry modules (static and emitted, NOT dynamic). Exempt from
  /// on-demand side-effect inclusion: they are the requested program. Dynamic
  /// entries join through namespace/own-export body demand instead.
  pub entry_module_idxs: &'a FxHashSet<ModuleIdx>,
  /// Body-demand key (a module's own export or namespace object) -> the module whose gated
  /// side-effect statements using that key demands (see
  /// [`super::on_demand::compute_body_demand_keys`]). Consulted by [`include_symbol`] as symbols become
  /// used.
  pub body_demand_keys: &'a FxHashMap<SymbolRef, ModuleIdx>,
  /// The second module-inclusion bit: modules whose *gated* side-effect statements have joined
  /// because their body was demanded. Distinct from `is_module_included_vec` (structural
  /// inclusion), which deliberately skips those statements for
  /// modules selected by [`super::on_demand::compute_body_demand_keys`].
  pub body_demand_swept: FxHashSet<ModuleIdx>,
  /// Work queue of the shared inclusion engine. Edge producers push typed work items here instead
  /// of recursing; the public `include_*` entry points drain it to
  /// empty before returning. Invariant (debug-asserted at every public entry point): empty
  /// whenever control is outside the engine. Visible outside this module only because
  /// `chunk_optimizer` constructs `IncludeContext` with a struct literal; nothing outside this
  /// module should touch it.
  pub(in crate::stages) pending: Vec<WorkItem>,
}

impl<'a> IncludeContext<'a> {
  #[expect(clippy::too_many_arguments)]
  pub fn new(
    modules: &'a IndexModules,
    stmt_infos: &'a IndexStmtInfos,
    symbols: &'a SymbolRefDb,
    is_included_vec: &'a mut StmtInclusionVec,
    is_module_included_vec: &'a mut ModuleInclusionVec,
    runtime_idx: ModuleIdx,
    metas: &'a LinkingMetadataVec,
    used_symbol_refs: &'a mut UsedSymbolRefsBuilder,
    used_external_symbols: &'a mut UsedExternalSymbols,
    constant_symbol_map: &'a FxHashMap<SymbolRef, ConstExportMeta>,
    options: &'a NormalizedBundlerOptions,
    normal_symbol_exports_chain_map: &'a FxHashMap<SymbolRef, Vec<SymbolRef>>,
    module_namespace_included_reason: &'a mut ModuleNamespaceReasonVec,
    entry_module_idxs: &'a FxHashSet<ModuleIdx>,
    body_demand_keys: &'a FxHashMap<SymbolRef, ModuleIdx>,
  ) -> Self {
    Self {
      modules,
      stmt_infos,
      symbols,
      is_included_vec,
      is_module_included_vec,
      tree_shaking: options.treeshake.is_some(),
      inline_const_smart: options.optimization.is_inline_const_smart_mode(),
      runtime_idx,
      metas,
      used_symbol_refs,
      used_external_symbols,
      constant_symbol_map,
      options,
      normal_symbol_exports_chain_map,
      bailout_cjs_tree_shaking_modules: FxHashSet::default(),
      module_inclusion_changed: false,
      module_namespace_included_reason,
      json_module_none_self_reference_included_symbol: FxHashMap::default(),
      entry_module_idxs,
      body_demand_keys,
      body_demand_swept: FxHashSet::default(),
      pending: Vec::new(),
    }
  }
}

struct LegacyInclusionFacts<'a> {
  modules: &'a IndexModules,
  metas: &'a LinkingMetadataVec,
  constant_symbol_map: &'a FxHashMap<SymbolRef, ConstExportMeta>,
  normal_symbol_exports_chain_map: &'a FxHashMap<SymbolRef, Vec<SymbolRef>>,
}

impl InclusionModuleFacts for LegacyInclusionFacts<'_> {
  fn exports_kind(&self, module_idx: ModuleIdx) -> rolldown_common::ExportsKind {
    self.modules[module_idx]
      .as_normal()
      .expect("inclusion facts require a normal module")
      .exports_kind
  }

  fn side_effects(
    &self,
    module_idx: ModuleIdx,
  ) -> rolldown_common::side_effects::DeterminedSideEffects {
    *self.modules[module_idx].side_effects()
  }
}

impl InclusionFacts for LegacyInclusionFacts<'_> {
  fn cjs_namespace_target(
    &self,
    importer_idx: ModuleIdx,
    namespace_ref: SymbolRef,
  ) -> Option<ModuleIdx> {
    self.metas[importer_idx].import_record_ns_to_cjs_module.get(&namespace_ref).copied()
  }

  fn resolved_export_symbol(&self, module_idx: ModuleIdx, name: &str) -> Option<SymbolRef> {
    self.metas[module_idx].resolved_exports.get(name).map(|export| export.symbol_ref)
  }

  fn commonjs_export_symbols(&self, module_idx: ModuleIdx) -> impl Iterator<Item = SymbolRef> + '_ {
    self.metas[module_idx]
      .resolved_exports
      .values()
      .filter(|export| export.came_from_commonjs)
      .map(|export| export.symbol_ref)
  }

  fn included_commonjs_export_symbols(
    &self,
    module_idx: ModuleIdx,
  ) -> impl Iterator<Item = SymbolRef> + '_ {
    self.metas[module_idx].included_commonjs_export_symbol.iter().copied()
  }

  fn dependencies(&self, module_idx: ModuleIdx) -> impl Iterator<Item = ModuleIdx> + '_ {
    self.metas[module_idx].dependencies.iter().copied()
  }

  fn member_expr_resolution<'a>(
    &'a self,
    module_idx: ModuleIdx,
    member_expr_ref: &MemberExprRef,
  ) -> Option<&'a MemberExprRefResolution> {
    member_expr_ref.resolution(&self.metas[module_idx].resolved_member_expr_refs)
  }

  fn esm_wrapper_ref(&self, module_idx: ModuleIdx) -> Option<SymbolRef> {
    let meta = &self.metas[module_idx];
    matches!(meta.wrap_kind(), WrapKind::Esm).then_some(meta.wrapper_ref).flatten()
  }

  fn constant_export(&self, symbol_ref: &SymbolRef) -> Option<&ConstExportMeta> {
    self.constant_symbol_map.get(symbol_ref)
  }

  fn normal_export_chain(&self, symbol_ref: &SymbolRef) -> &[SymbolRef] {
    self.normal_symbol_exports_chain_map.get(symbol_ref).map(Vec::as_slice).unwrap_or(&[])
  }

  fn normal_export_chains(&self) -> impl Iterator<Item = (SymbolRef, &[SymbolRef])> + '_ {
    self
      .normal_symbol_exports_chain_map
      .iter()
      .map(|(symbol_ref, chain)| (*symbol_ref, chain.as_slice()))
  }
}

impl IncludeContext<'_> {
  fn with_core<R>(
    &mut self,
    operation: impl FnOnce(&mut InclusionCoreContext<'_, LegacyInclusionFacts<'_>>) -> R,
  ) -> R {
    let facts = LegacyInclusionFacts {
      modules: self.modules,
      metas: self.metas,
      constant_symbol_map: self.constant_symbol_map,
      normal_symbol_exports_chain_map: self.normal_symbol_exports_chain_map,
    };
    let mut core = InclusionCoreContext {
      facts: &facts,
      modules: self.modules,
      stmt_infos: self.stmt_infos,
      symbols: self.symbols,
      is_included_vec: &mut *self.is_included_vec,
      is_module_included_vec: &mut *self.is_module_included_vec,
      config: InclusionConfig {
        tree_shaking: TreeShakingConfig {
          enabled: self.tree_shaking,
          commonjs: self.options.treeshake.commonjs(),
          property_write_side_effects: self.options.treeshake.property_write_side_effects(),
        },
        inline_const_smart: self.inline_const_smart,
        preserve_modules: self.options.preserve_modules,
        dev_mode: self.options.is_dev_mode_enabled(),
      },
      runtime_idx: self.runtime_idx,
      used_symbol_refs: &mut *self.used_symbol_refs,
      used_external_symbols: &mut *self.used_external_symbols,
      bailout_cjs_tree_shaking_modules: &mut self.bailout_cjs_tree_shaking_modules,
      module_inclusion_changed: &mut self.module_inclusion_changed,
      module_namespace_included_reason: &mut *self.module_namespace_included_reason,
      json_module_none_self_reference_included_symbol: &mut self
        .json_module_none_self_reference_included_symbol,
      entry_module_idxs: self.entry_module_idxs,
      body_demand_keys: self.body_demand_keys,
      body_demand_swept: &mut self.body_demand_swept,
      pending: &mut self.pending,
    };
    operation(&mut core)
  }
}

pub(super) fn include_symbol_and_check_cjs_bailout(
  ctx: &mut IncludeContext,
  symbol_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) {
  ctx.with_core(|core| {
    include_symbol_and_check_cjs_bailout_core(core, symbol_ref, include_reason);
  });
}

pub(super) fn include_declaring_statements(ctx: &mut IncludeContext, symbol_ref: &SymbolRef) {
  ctx.with_core(|core| include_declaring_statements_core(core, symbol_ref));
}

pub fn include_module(ctx: &mut IncludeContext, module: &NormalModule) {
  ctx.with_core(|core| include_module_core(core, module));
}

pub fn include_symbol(
  ctx: &mut IncludeContext,
  symbol_ref: SymbolRef,
  include_reason: SymbolIncludeReason,
) {
  ctx.with_core(|core| include_symbol_core(core, symbol_ref, include_reason));
}

pub(super) fn include_cjs_bailout_exports_with_core(
  ctx: &mut IncludeContext,
  bailout_modules: impl IntoIterator<Item = ModuleIdx>,
) {
  ctx.with_core(|core| include_cjs_bailout_exports_core(core, bailout_modules));
}

pub(super) fn include_runtime_symbol_with_core(
  ctx: &mut IncludeContext,
  runtime: &RuntimeModuleBrief,
  depended_runtime_helper: RuntimeHelper,
) {
  ctx.with_core(|core| include_runtime_symbol_core(core, runtime, depended_runtime_helper));
}

fn preserve_reexported_interfaces_legacy_core(
  core: &mut InclusionCoreContext<'_, LegacyInclusionFacts<'_>>,
) {
  preserve_reexported_interfaces_core(core);
}

pub(super) fn preserve_reexported_interfaces_with_core(ctx: &mut IncludeContext) {
  ctx.with_core(preserve_reexported_interfaces_legacy_core);
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(in crate::stages::link_stage) fn include_statements(
    &mut self,
    unreachable_import_expression_node_ids: &UnreachableDynamicImports,
    statement_runtime_requirements: &Sealed<StatementRuntimeRequirements>,
    entry_export_roots: &EntryExportRoots,
  ) {
    let mut is_stmt_info_included_vec: StmtInclusionVec = self
      .module_table
      .modules
      .iter()
      .zip(self.stmt_infos.iter())
      .map(|(m, stmt_infos)| {
        m.as_normal().map_or(IndexBitSet::default(), |_| IndexBitSet::new(stmt_infos.len()))
      })
      .collect::<IndexVec<ModuleIdx, _>>();
    let mut used_symbol_refs = UsedSymbolRefsBuilder::default();
    let mut used_external_symbols = UsedExternalSymbols::default();
    let mut is_module_included_vec: ModuleInclusionVec =
      IndexBitSet::new(self.module_table.modules.len());
    let mut module_namespace_included_reason: ModuleNamespaceReasonVec =
      oxc_index::index_vec![ModuleNamespaceIncludedReason::empty(); self.module_table.len()];
    self.has_enum_inlining = self
      .module_table
      .modules
      .iter()
      .any(|m| m.as_normal().is_some_and(|n| !n.ecma_view.enum_member_value_map.is_empty()));
    let entry_module_idxs = self.user_defined_entry_module_idxs();
    let body_demand_keys = compute_body_demand_keys(
      &self.module_table.modules,
      &self.stmt_infos,
      &self.symbols,
      self.options.treeshake.is_some(),
      &entry_module_idxs,
    );
    let context = &mut IncludeContext::new(
      &self.module_table.modules,
      &self.stmt_infos,
      &self.symbols,
      &mut is_stmt_info_included_vec,
      &mut is_module_included_vec,
      self.runtime.id(),
      &self.metas,
      &mut used_symbol_refs,
      &mut used_external_symbols,
      &self.global_constant_symbol_map,
      self.options,
      &self.normal_symbol_exports_chain_map,
      &mut module_namespace_included_reason,
      &entry_module_idxs,
      &body_demand_keys,
    );

    let (user_defined_entries, mut dynamic_entries): (Vec<_>, Vec<_>) =
      std::mem::take(&mut self.entries)
        .into_values()
        .flatten()
        .partition(|item| item.kind.is_user_defined());
    user_defined_entries.iter().for_each(|entry| {
      let module = match &self.module_table[entry.idx] {
        Module::Normal(module) => module,
        Module::External(_module) => {
          // Case: import('external').
          return;
        }
      };
      context.bailout_cjs_tree_shaking_modules.insert(module.idx);
      entry_export_roots.get(entry.idx).unwrap_or_default().iter().for_each(|root| {
        let symbol_ref = root.symbol_ref;
        if let Module::Normal(_) = &context.modules[symbol_ref.owner] {
          include_declaring_statements(context, &symbol_ref);
          include_symbol_and_check_cjs_bailout(
            context,
            symbol_ref,
            SymbolIncludeReason::EntryExport,
          );
        }
      });
      include_module(context, module);
    });

    let mut unused_record_idxs = vec![];
    let cycled_idx = self.sort_dynamic_entries_by_topological_order(&mut dynamic_entries);
    let mut included_dynamic_entry = FxHashSet::default();
    loop {
      context.module_inclusion_changed = false;

      // It could be safely take since it is no more used.
      // We extract bailout_modules first to avoid borrowing conflict:
      // passing `context` requires a mutable borrow, which conflicts with
      // borrowing `context.bailout_cjs_tree_shaking_modules` inside the call.
      let bailout_modules = std::mem::take(&mut context.bailout_cjs_tree_shaking_modules);
      include_cjs_bailout_exports(context, bailout_modules);

      dynamic_entries.iter().for_each(|entry| {
        if included_dynamic_entry.contains(&entry.idx) {
          return;
        }
        let included = self.process_and_retain_dynamic_entry(
          entry,
          &cycled_idx,
          context,
          &mut unused_record_idxs,
          unreachable_import_expression_node_ids,
          entry_export_roots,
        );
        if included {
          included_dynamic_entry.insert(entry.idx);
        }
      });

      if !context.module_inclusion_changed {
        break;
      }
    }

    // Under `preserveModules`, preserve each module's re-exports whose canonical value survived
    // tree-shaking, so every emitted file mirrors its source's export interface (issue #9122).
    preserve_reexported_interfaces(context);

    dynamic_entries.retain(|entry| included_dynamic_entry.contains(&entry.idx));

    // update entries with lived only.
    self.entries = {
      let mut entries = FxIndexMap::default();
      for entry in
        user_defined_entries.into_iter().chain(if self.options.code_splitting.is_disabled() {
          itertools::Either::Left(std::iter::empty())
        } else {
          itertools::Either::Right(dynamic_entries.into_iter())
        })
      {
        entries.entry(entry.idx).or_insert_with(Vec::new).push(entry);
      }
      entries
    };

    // Setting the json module none self reference included symbol map
    for (mi, set) in std::mem::take(&mut context.json_module_none_self_reference_included_symbol) {
      let module = self.module_table[mi].as_normal_mut().expect("should be a normal module");
      _ = module.ecma_view.json_module_none_self_reference_included_symbol.insert(Box::new(set));
    }

    // mark those dynamic import records as dead, in case we could eliminate them later in ast
    // visitor.
    for (mi, record_idx) in unused_record_idxs {
      let module = self.module_table[mi].as_normal_mut().expect("should be a normal module");
      let rec = &mut module.import_records[record_idx];
      rec.meta.insert(ImportRecordMeta::DeadDynamicImport);
    }

    self
      .module_table
      .modules
      .par_iter_mut()
      .zip_eq(self.metas.par_iter_mut())
      .zip_eq(statement_runtime_requirements.slots().par_iter())
      .filter_map(|((m, meta), depended_helper)| {
        m.as_normal_mut().map(|m| (m, meta, depended_helper))
      })
      .for_each(|(module, meta, depended_helper)| {
        let idx = module.idx;
        let mut normalized_runtime_helper = RuntimeHelper::default();
        for (helper, stmt_info_idxs) in depended_helper.iter() {
          if stmt_info_idxs.is_empty() {
            continue;
          }
          let any_included = stmt_info_idxs
            .iter()
            .any(|stmt_info_idx| is_stmt_info_included_vec[module.idx].has_bit(*stmt_info_idx));
          // We also need to process the runtime helper of an eliminated module so that we
          // can propagate it to its importers later.
          normalized_runtime_helper.set(
            helper,
            any_included
              || (module.id != RUNTIME_MODULE_ID && !is_module_included_vec.has_bit(idx)),
          );
        }
        meta.depended_runtime_helper = normalized_runtime_helper;
        meta.module_namespace_included_reason = module_namespace_included_reason[module.idx];
      });

    let depended_runtime_helper = collect_depended_runtime_helpers(
      &self.module_table.modules,
      &self.metas,
      &is_module_included_vec,
    );
    let context = &mut IncludeContext::new(
      &self.module_table.modules,
      &self.stmt_infos,
      &self.symbols,
      &mut is_stmt_info_included_vec,
      &mut is_module_included_vec,
      self.runtime.id(),
      &self.metas,
      &mut used_symbol_refs,
      &mut used_external_symbols,
      &self.global_constant_symbol_map,
      self.options,
      &self.normal_symbol_exports_chain_map,
      &mut module_namespace_included_reason,
      &entry_module_idxs,
      &body_demand_keys,
    );
    include_runtime_symbol(context, &self.runtime, depended_runtime_helper);

    self.used_symbol_refs = used_symbol_refs;
    self.used_external_symbols = used_external_symbols;
    // Store the final statement inclusion results back to metas.
    is_stmt_info_included_vec.into_iter_enumerated().for_each(|(module_idx, stmt_included_vec)| {
      self.metas[module_idx].stmt_info_included = stmt_included_vec;
    });
    // Store the final module inclusion results back to metas.
    for (module_idx, meta) in self.metas.iter_mut_enumerated() {
      meta.is_included = is_module_included_vec.has_bit(module_idx);
    }

    tracing::trace!(
      "included statements {:#?}",
      self
        .module_table
        .modules
        .iter()
        .filter_map(Module::as_normal)
        .map(|m| m.to_debug_normal_module_for_tree_shaking(
          &self.stmt_infos[m.idx],
          self.metas[m.idx].is_included,
          &self.metas[m.idx].stmt_info_included
        ))
        .collect::<Vec<_>>()
    );
  }
}

#[cfg(test)]
mod tests {
  use oxc::{
    semantic::{NodeId, SymbolId},
    span::SPAN,
  };
  use oxc_index::IndexVec;
  use oxc_str::CompactStr;
  use rolldown_common::{
    ConstantValue, MemberExprObjectReferencedType, MemberExprRef, MemberExprRefResolution,
    ResolvedExport,
  };

  use crate::stages::link_stage::passes::test_utils::normal_module;
  use crate::types::linking_metadata::LinkingMetadata;

  use super::*;

  fn symbol(module: usize, symbol: usize) -> SymbolRef {
    SymbolRef { owner: ModuleIdx::new(module), symbol: SymbolId::new(symbol) }
  }

  #[test]
  fn legacy_fact_adapter_is_query_equivalent() {
    let importer_idx = ModuleIdx::new(0);
    let target_idx = ModuleIdx::new(1);
    let dependency_idx = ModuleIdx::new(2);
    let namespace_ref = symbol(0, 0);
    let resolved_ref = symbol(1, 1);
    let included_cjs_ref = symbol(1, 2);
    let wrapper_ref = symbol(1, 3);
    let constant_ref = symbol(2, 0);
    let chained_ref = symbol(2, 1);
    let member_ref = MemberExprRef::new(
      namespace_ref,
      Vec::new(),
      NodeId::new(7),
      SPAN,
      MemberExprObjectReferencedType::Namespace,
      None,
      false,
    );
    let member_resolution = MemberExprRefResolution {
      resolved: Some(resolved_ref),
      prop_and_related_span_list: Vec::new(),
      depended_refs: vec![wrapper_ref],
      target_commonjs_exported_symbol: None,
      reference_id: None,
    };

    let mut metas = IndexVec::from_vec(vec![
      LinkingMetadata::default(),
      LinkingMetadata::default(),
      LinkingMetadata::default(),
    ]);
    metas[importer_idx].import_record_ns_to_cjs_module.insert(namespace_ref, target_idx);
    metas[importer_idx].dependencies.insert(dependency_idx);
    metas[importer_idx].resolved_member_expr_refs.insert(member_ref.node_id, member_resolution);
    metas[target_idx]
      .resolved_exports
      .insert(CompactStr::new("named"), ResolvedExport::new(resolved_ref, true));
    metas[target_idx].included_commonjs_export_symbol.insert(included_cjs_ref);
    metas[target_idx].wrapper_ref = Some(wrapper_ref);
    metas[target_idx].set_wrap_kind(WrapKind::Esm);

    let constant = ConstExportMeta::new(ConstantValue::Boolean(true), false);
    let constant_symbol_map = FxHashMap::from_iter([(constant_ref, constant)]);
    let normal_symbol_exports_chain_map = FxHashMap::from_iter([(resolved_ref, vec![chained_ref])]);
    let modules = IndexVec::from_vec(vec![
      normal_module(0, false, Vec::new()),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
    ]);
    let facts = LegacyInclusionFacts {
      modules: &modules,
      metas: &metas,
      constant_symbol_map: &constant_symbol_map,
      normal_symbol_exports_chain_map: &normal_symbol_exports_chain_map,
    };

    assert!(matches!(facts.exports_kind(importer_idx), rolldown_common::ExportsKind::Esm));
    assert!(matches!(
      facts.side_effects(importer_idx),
      rolldown_common::side_effects::DeterminedSideEffects::Analyzed(false)
    ));
    assert_eq!(facts.cjs_namespace_target(importer_idx, namespace_ref), Some(target_idx));
    assert_eq!(facts.resolved_export_symbol(target_idx, "named"), Some(resolved_ref));
    assert_eq!(facts.commonjs_export_symbols(target_idx).collect::<Vec<_>>(), vec![resolved_ref]);
    assert_eq!(
      facts.included_commonjs_export_symbols(target_idx).collect::<Vec<_>>(),
      vec![included_cjs_ref]
    );
    assert_eq!(facts.dependencies(importer_idx).collect::<Vec<_>>(), vec![dependency_idx]);
    assert_eq!(
      facts.member_expr_resolution(importer_idx, &member_ref).map(|value| value.resolved),
      Some(Some(resolved_ref))
    );
    assert_eq!(facts.esm_wrapper_ref(target_idx), Some(wrapper_ref));
    assert!(facts.constant_export(&constant_ref).is_some());
    assert_eq!(facts.normal_export_chain(&resolved_ref), &[chained_ref]);
    assert_eq!(
      facts.normal_export_chains().collect::<Vec<_>>(),
      vec![(resolved_ref, &[chained_ref][..])]
    );
  }

  #[test]
  fn legacy_generate_entrypoint_signatures_stay_stable() {
    let _: for<'ctx, 'borrow> fn(
      &'borrow mut IncludeContext<'ctx>,
      SymbolRef,
      SymbolIncludeReason,
    ) = include_symbol;
    let _: for<'ctx, 'borrow, 'module> fn(
      &'borrow mut IncludeContext<'ctx>,
      &'module NormalModule,
    ) = include_module;
  }
}
