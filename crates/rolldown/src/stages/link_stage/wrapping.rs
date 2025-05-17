use oxc_index::IndexVec;
use rolldown_common::{
  ExportsKind, IndexModules, Module, ModuleIdx, NormalModule, NormalizedBundlerOptions,
  RuntimeModuleBrief, StmtInfo, StmtInfoMeta, SymbolRefDb, WrapKind,
};

use crate::types::linking_metadata::{LinkingMetadata, LinkingMetadataVec};

use super::LinkStage;

struct Context<'a> {
  pub visited_modules: &'a mut IndexVec<ModuleIdx, bool>,
  pub linking_infos: &'a mut LinkingMetadataVec,
  pub modules: &'a IndexModules,
}

fn wrap_module_recursively(ctx: &mut Context, target: ModuleIdx) {
  let Module::Normal(module) = &ctx.modules[target] else {
    return;
  };

  // Only consider `NormalModule`
  if ctx.visited_modules[target] {
    return;
  }
  ctx.visited_modules[target] = true;

  if matches!(ctx.linking_infos[target].wrap_kind, WrapKind::None) {
    ctx.linking_infos[target].wrap_kind = match module.exports_kind {
      ExportsKind::Esm | ExportsKind::None => WrapKind::Esm,
      ExportsKind::CommonJs => WrapKind::Cjs,
    }
  }

  module.import_records.iter().filter(|item| !item.is_dummy()).for_each(|importee| {
    wrap_module_recursively(ctx, importee.resolved_module);
  });
}

fn has_dynamic_exports_due_to_export_star(
  target: ModuleIdx,
  modules: &IndexModules,
  linking_infos: &mut LinkingMetadataVec,
  visited_modules: &mut IndexVec<ModuleIdx, bool>,
) -> bool {
  if visited_modules[target] {
    return linking_infos[target].has_dynamic_exports;
  }
  visited_modules[target] = true;

  let has_dynamic_exports = match &modules[target] {
    Module::Normal(module) => {
      if matches!(module.exports_kind, ExportsKind::CommonJs) {
        true
      } else {
        module.star_export_module_ids().any(|importee_id| {
          target != importee_id
            && has_dynamic_exports_due_to_export_star(
              importee_id,
              modules,
              linking_infos,
              visited_modules,
            )
        })
      }
    }
    Module::External(_) => true,
  };

  if has_dynamic_exports {
    linking_infos[target].has_dynamic_exports = true;
  }
  linking_infos[target].has_dynamic_exports
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn wrap_modules(&mut self) {
    let mut visited_modules_for_wrapping =
      oxc_index::index_vec![false; self.module_table.modules.len()];
    let mut visited_modules_for_dynamic_exports =
      oxc_index::index_vec![false; self.module_table.modules.len()];

    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      let module_id = module.idx;

      if module.has_star_export() {
        has_dynamic_exports_due_to_export_star(
          module_id,
          &self.module_table.modules,
          &mut self.metas,
          &mut visited_modules_for_dynamic_exports,
        );
      }

      let is_strict_execution_order = self.options.experimental.is_strict_execution_order_enabled();
      let is_wrap_kind_none = matches!(self.metas[module_id].wrap_kind, WrapKind::None);

      if is_strict_execution_order && is_wrap_kind_none {
        self.metas[module_id].wrap_kind = match module.exports_kind {
          ExportsKind::Esm | ExportsKind::None => WrapKind::Esm,
          ExportsKind::CommonJs => WrapKind::Cjs,
        }
      }

      let need_to_wrap = is_strict_execution_order || !is_wrap_kind_none;

      // The `modules` don't seem to be sorted,
      // and if the module is `WrapKind::None`, it might still be wrapped next iter.
      if need_to_wrap {
        visited_modules_for_wrapping[module_id] = true;
      }

      module.import_records.iter().filter(|rec| !rec.is_dummy()).for_each(|rec| {
        let Module::Normal(importee) = &self.module_table.modules[rec.resolved_module] else {
          return;
        };
        if matches!(importee.exports_kind, ExportsKind::CommonJs) || need_to_wrap {
          wrap_module_recursively(
            &mut Context {
              visited_modules: &mut visited_modules_for_wrapping,
              linking_infos: &mut self.metas,
              modules: &self.module_table.modules,
            },
            importee.idx,
          );
        }
      });
    }
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
  match linking_info.wrap_kind {
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
        stmt_idx: None,
        declared_symbols: vec![wrapper_ref],
        referenced_symbols: vec![if options.profiler_names {
          runtime.resolve_symbol("__commonJS").into()
        } else {
          runtime.resolve_symbol("__commonJSMin").into()
        }],
        side_effect: false,
        is_included: false,
        import_records: Vec::new(),
        #[cfg(debug_assertions)]
        debug_label: None,
        meta: StmtInfoMeta::default(),
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
        stmt_idx: None,
        declared_symbols: vec![wrapper_ref],
        referenced_symbols: vec![if options.profiler_names {
          runtime.resolve_symbol("__esm").into()
        } else {
          runtime.resolve_symbol("__esmMin").into()
        }],
        side_effect: false,
        is_included: false,
        import_records: Vec::new(),
        #[cfg(debug_assertions)]
        debug_label: None,
        meta: StmtInfoMeta::default(),
      };

      linking_info.wrapper_stmt_info = Some(module.stmt_infos.add_stmt_info(stmt_info));
      linking_info.wrapper_ref = Some(wrapper_ref);
    }
    WrapKind::None => {}
  }
}
