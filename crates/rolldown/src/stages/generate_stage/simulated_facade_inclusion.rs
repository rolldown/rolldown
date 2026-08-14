use rolldown_common::{
  ModuleIdx, ModuleNamespaceIncludedReason, RuntimeHelper, StmtInfos, UsedSymbolRefsBuilder,
  WrapKind,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  stages::link_stage::{
    IncludeContext, SymbolIncludeReason, compute_body_demand_keys, include_runtime_symbol,
    include_symbol,
  },
  types::linking_metadata::{
    included_info_to_linking_metadata_vec, linking_metadata_vec_to_included_info,
  },
};

use super::GenerateStage;

impl GenerateStage<'_> {
  /// Namespace symbols by default reference all exported symbols from the module.
  /// To preserve dynamic import tree shaking, we should only include symbols that were actually used during the linking stage.
  /// This ensures that including a namespace symbol doesn't inadvertently add unused exported symbols
  pub(super) fn narrow_namespace_stmt_to_used_symbols(
    &mut self,
    entry_module_idx: ModuleIdx,
    used_symbol_refs: &UsedSymbolRefsBuilder,
  ) {
    let runtime_module_idx = self.link_output.runtime.id();
    let wrap_kind = self.link_output.metas[entry_module_idx].wrap_kind();
    if self.link_output.module_table[entry_module_idx].as_normal().is_none() {
      return;
    }
    // For CJS modules, we don't need to include `__exportAll` and the namespace symbols.
    // Instead, we should include the wrapper_ref (`require_xxx`), which will be handled
    // in the `include_simulated_facade_namespace` call.
    if !matches!(wrap_kind, WrapKind::Cjs) {
      // Filter in place to avoid cloning
      self.link_output.stmt_infos[entry_module_idx][StmtInfos::NAMESPACE_STMT_IDX]
        .referenced_symbols
        .retain(|item| match item {
          rolldown_common::SymbolOrMemberExprRef::Symbol(symbol_ref) => {
            // module namespace symbol requires `__exportAll` runtime helper
            used_symbol_refs.contains(symbol_ref) || symbol_ref.owner == runtime_module_idx
          }
          rolldown_common::SymbolOrMemberExprRef::MemberExpr(_member_expr_ref) => true,
        });
    }
  }

  /// Replay the link-stage inclusion semantics: side-effectful statements of
  /// user-declared side-effect-free modules join only through body demand.
  /// Already-included statements make the replayed edges no-ops.
  /// When `f` returns `true`, the `__exportAll` runtime helper is included as well.
  pub(super) fn replay_link_stage_inclusion(
    &mut self,
    used_symbol_refs: &mut UsedSymbolRefsBuilder,
    f: impl FnOnce(&mut IncludeContext<'_>) -> bool,
  ) {
    let (mut stmt_info_included_vec, mut module_included_vec, mut module_namespace_reason_vec) =
      linking_metadata_vec_to_included_info(&mut self.link_output.metas);

    let body_demand_keys = compute_body_demand_keys(
      &self.link_output.module_table.modules,
      &self.link_output.stmt_infos,
      &self.link_output.symbol_db,
      self.options.treeshake.is_some(),
      &self.link_output.user_defined_entry_modules,
    );

    let runtime = &self.link_output.runtime;
    let context = &mut IncludeContext {
      modules: &self.link_output.module_table.modules,
      stmt_infos: &self.link_output.stmt_infos,
      symbols: &self.link_output.symbol_db,
      is_included_vec: &mut stmt_info_included_vec,
      is_module_included_vec: &mut module_included_vec,
      tree_shaking: self.options.treeshake.is_some(),
      runtime_idx: self.link_output.runtime.id(),
      metas: &self.link_output.metas,
      used_symbol_refs,
      used_external_symbols: &mut self.link_output.used_external_symbols,
      constant_symbol_map: &self.link_output.global_constant_symbol_map,
      options: self.options,
      normal_symbol_exports_chain_map: &self.link_output.normal_symbol_exports_chain_map,
      bailout_cjs_tree_shaking_modules: FxHashSet::default(),
      external_importer_modes: FxHashMap::default(),
      module_inclusion_changed: false,
      module_namespace_included_reason: &mut module_namespace_reason_vec,
      inline_const_smart: self.options.optimization.is_inline_const_smart_mode(),
      json_module_none_self_reference_included_symbol: FxHashMap::default(),
      entry_module_idxs: &self.link_output.user_defined_entry_modules,
      body_demand_keys: &body_demand_keys,
      body_demand_swept: FxHashSet::default(),
      pending: Vec::new(),
    };

    if f(context) {
      include_runtime_symbol(context, runtime, RuntimeHelper::ExportAll);
    }

    // Restore the included info before materializing the runtime chunk, because
    // this replay may be the first pass that includes a runtime helper.
    included_info_to_linking_metadata_vec(
      &mut self.link_output.metas,
      stmt_info_included_vec,
      &module_included_vec,
      &module_namespace_reason_vec,
    );
  }
}

/// Include the symbols an absorbed dynamic entry needs so its chunk can simulate the removed
/// facade. Returns whether the namespace object was included — the caller then owes the
/// chunk-level `__exportAll` bookkeeping (`depended_runtime_helper`) and must make
/// [`GenerateStage::replay_link_stage_inclusion`]'s closure return `true`.
pub(super) fn include_simulated_facade_namespace(
  context: &mut IncludeContext<'_>,
  entry_module_idx: ModuleIdx,
) -> bool {
  let Some(module) = context.modules[entry_module_idx].as_normal() else {
    return false;
  };
  let namespace_object_ref = module.namespace_object_ref;
  let wrap_kind = context.metas[entry_module_idx].wrap_kind();

  // For CJS modules, include the wrapper_ref (require_xxx) instead of namespace
  // and use ToEsm runtime helper instead of ExportAll
  if matches!(wrap_kind, WrapKind::Cjs | WrapKind::Esm) {
    if let Some(wrapper_ref) = context.metas[entry_module_idx].wrapper_ref {
      include_symbol(context, wrapper_ref, SymbolIncludeReason::SimulatedFacadeChunk);
    }
  }
  if matches!(wrap_kind, WrapKind::Esm | WrapKind::None) {
    include_symbol(context, namespace_object_ref, SymbolIncludeReason::SimulatedFacadeChunk);
    context.module_namespace_included_reason[entry_module_idx]
      .insert(ModuleNamespaceIncludedReason::SimulateFacadeChunk);
    true
  } else {
    false
  }
}
