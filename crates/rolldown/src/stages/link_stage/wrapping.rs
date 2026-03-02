use rolldown_common::{
  NormalModule, NormalizedBundlerOptions, RuntimeModuleBrief, StmtInfo, StmtInfoMeta, SymbolRefDb,
  TaggedSymbolRef, WrapKind,
};

use crate::types::linking_metadata::LinkingMetadata;

use super::LinkStage;
use super::oxc_conversions::from_oxc_wrap_kind;

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn wrap_modules(&mut self) {
    // 1. Compute has_dynamic_exports using oxc_module_graph (already ported).
    let dynamic_export_modules =
      oxc_module_graph::compute_has_dynamic_exports(&self.link_kernel.graph);
    for oxc_idx in &dynamic_export_modules {
      let rd_idx = rolldown_common::ModuleIdx::from_usize(oxc_idx.index());
      self.metas[rd_idx].has_dynamic_exports = true;
      // Also write to graph so downstream algorithms see it.
      if let Some(gm) = self.link_kernel.graph.normal_module_mut(*oxc_idx) {
        gm.has_dynamic_exports = true;
      }
    }

    // 2. Call oxc_module_graph wrap_modules with skip_symbol_creation.
    let config = oxc_module_graph::WrapModulesConfig {
      on_demand_wrapping: self.options.experimental.is_on_demand_wrapping_enabled(),
      strict_execution_order: self.options.is_strict_execution_order_enabled(),
      skip_symbol_creation: true,
    };
    let result = oxc_module_graph::wrap_modules(&mut self.link_kernel.graph, &config);

    // 3. Sync wrap_kind updates to Rolldown metas BEFORE apply() consumes the result.
    //
    // When strict_execution_order is enabled, we need to distinguish between
    // the "original" wrap_kind (set during propagation) and the "final" wrap_kind
    // (possibly overridden by strict execution order logic). Rolldown's
    // sync_wrap_kind sets BOTH wrap_kind AND original_wrap_kind, while
    // update_wrap_kind only sets wrap_kind.
    //
    // The oxc_module_graph algorithm handles strict execution order internally:
    // - original_wrap_kinds: the wrap_kind set during propagation (before strict override)
    // - wrap_kind_updates: the finalized wrap_kind (after strict override)
    if self.options.is_strict_execution_order_enabled() {
      // First, set original_wrap_kind via sync_wrap_kind for all modules that
      // have an original_wrap_kind recorded.
      for (oxc_idx, orig_wk) in &result.original_wrap_kinds {
        let rd_idx = rolldown_common::ModuleIdx::from_usize(oxc_idx.index());
        self.metas[rd_idx].sync_wrap_kind(from_oxc_wrap_kind(*orig_wk));
      }
      // Then, override wrap_kind (but not original_wrap_kind) with the final value.
      for (oxc_idx, final_wk) in &result.wrap_kind_updates {
        let rd_idx = rolldown_common::ModuleIdx::from_usize(oxc_idx.index());
        self.metas[rd_idx].update_wrap_kind(from_oxc_wrap_kind(*final_wk));
      }
    } else {
      // Without strict execution order, original_wrap_kind == wrap_kind,
      // so sync_wrap_kind (which sets both) is correct.
      for (oxc_idx, wrap_kind) in &result.wrap_kind_updates {
        let rd_idx = rolldown_common::ModuleIdx::from_usize(oxc_idx.index());
        self.metas[rd_idx].sync_wrap_kind(from_oxc_wrap_kind(*wrap_kind));
      }
    }

    // 4. Sync required_by_other_module to metas.
    for oxc_idx in &result.required_by_other_module {
      let rd_idx = rolldown_common::ModuleIdx::from_usize(oxc_idx.index());
      self.metas[rd_idx].required_by_other_module = true;
    }

    // 5. Apply to graph (writes wrap_kind, original_wrap_kind, wrapper_refs, required_by_other_module).
    result.apply(&mut self.link_kernel.graph);

    // 6. Create Rolldown-specific wrapper symbols + StmtInfo (unchanged).
    self.module_table.modules.iter_mut().filter_map(|m| m.as_normal_mut()).for_each(
      |ecma_module| {
        let linking_info = &mut self.metas[ecma_module.idx];
        create_wrapper(ecma_module, linking_info, &mut self.symbols, &self.runtime, self.options);
      },
    );
  }
}

pub fn create_wrapper(
  module: &mut NormalModule,
  linking_info: &mut LinkingMetadata,
  symbols: &mut SymbolRefDb,
  runtime: &RuntimeModuleBrief,
  options: &NormalizedBundlerOptions,
) {
  match linking_info.wrap_kind() {
    // If this is a CommonJS file, we're going to need to generate a wrapper
    // for the CommonJS closure. That will end up looking something like this:
    //
    //   var require_foo = __commonJS((exports, module) => {
    //     ...
    //   });
    //
    WrapKind::Cjs => {
      let wrapper_ref = symbols
        .create_facade_root_symbol_ref(module.idx, &format!("require_{}", &module.repr_name));

      let stmt_info = StmtInfo {
        declared_symbols: vec![TaggedSymbolRef::Normal(wrapper_ref)],
        referenced_symbols: vec![if options.profiler_names {
          runtime.resolve_symbol("__commonJS").into()
        } else {
          runtime.resolve_symbol("__commonJSMin").into()
        }],
        side_effect: false.into(),
        import_records: Vec::new(),
        #[cfg(debug_assertions)]
        debug_label: None,
        meta: StmtInfoMeta::default(),

        force_tree_shaking: true,
      };

      linking_info.wrapper_stmt_info = Some(module.stmt_infos.add_stmt_info(stmt_info));
      linking_info.wrapper_ref = Some(wrapper_ref);
    }
    // If this is a lazily-initialized ESM file, we're going to need to
    // generate a wrapper for the ESM closure. That will end up looking
    // something like this:
    //
    //   var init_foo = __esm(() => {
    //     ...
    //   });
    //
    WrapKind::Esm => {
      let wrapper_ref =
        symbols.create_facade_root_symbol_ref(module.idx, &format!("init_{}", &module.repr_name));

      let stmt_info = StmtInfo {
        declared_symbols: vec![TaggedSymbolRef::Normal(wrapper_ref)],
        referenced_symbols: vec![if options.profiler_names {
          runtime.resolve_symbol("__esm").into()
        } else {
          runtime.resolve_symbol("__esmMin").into()
        }],
        side_effect: true.into(),
        import_records: Vec::new(),
        #[cfg(debug_assertions)]
        debug_label: None,
        meta: StmtInfoMeta::default(),
        force_tree_shaking: true,
      };

      linking_info.wrapper_stmt_info = Some(module.stmt_infos.add_stmt_info(stmt_info));
      linking_info.wrapper_ref = Some(wrapper_ref);
    }
    WrapKind::None => {}
  }
}
