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
  let is_visited = &mut ctx.visited_modules[target];
  if *is_visited {
    return;
  }
  *is_visited = true;

  let Module::Normal(module) = &ctx.modules[target] else {
    return;
  };

  if matches!(ctx.linking_infos[target].wrap_kind, WrapKind::None) {
    ctx.linking_infos[target].wrap_kind = match module.exports_kind {
      ExportsKind::Esm | ExportsKind::None => WrapKind::Esm,
      ExportsKind::CommonJs => WrapKind::Cjs,
    }
  }

  module.import_records.iter().for_each(|importee| {
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
  pub fn wrap_modules(&mut self) {
    let mut visited_modules_for_wrapping =
      oxc_index::index_vec![false; self.module_table.modules.len()];

    let mut visited_modules_for_dynamic_exports =
      oxc_index::index_vec![false; self.module_table.modules.len()];

    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      let module_id = module.idx;
      let linking_info = &self.metas[module_id];

      let need_to_wrap = self.options.experimental.is_strict_execution_order_enabled()
        || matches!(linking_info.wrap_kind, WrapKind::Cjs | WrapKind::Esm);

      if need_to_wrap {
        wrap_module_recursively(
          &mut Context {
            visited_modules: &mut visited_modules_for_wrapping,
            linking_infos: &mut self.metas,
            modules: &self.module_table.modules,
          },
          module_id,
        );
      }

      if module.has_star_export() {
        has_dynamic_exports_due_to_export_star(
          module_id,
          &self.module_table.modules,
          &mut self.metas,
          &mut visited_modules_for_dynamic_exports,
        );
      }

      module.import_records.iter().for_each(|rec| {
        let importee_id = rec.resolved_module;
        let Module::Normal(importee) = &self.module_table.modules[importee_id] else {
          return;
        };
        if matches!(importee.exports_kind, ExportsKind::CommonJs) {
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
        debug_label: None,
        meta: StmtInfoMeta::default(),
      };

      linking_info.wrapper_stmt_info = Some(module.stmt_infos.add_stmt_info(stmt_info));
      linking_info.wrapper_ref = Some(wrapper_ref);
    }
    WrapKind::None => {}
  }
}
