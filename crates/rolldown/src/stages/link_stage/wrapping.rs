use oxc::index::IndexVec;
use rolldown_common::{
  ExportsKind, ModuleId, NormalModule, NormalModuleId, NormalModuleVec, StmtInfo, WrapKind,
};

use crate::{
  runtime::RuntimeModuleBrief,
  types::{
    linking_metadata::{LinkingMetadata, LinkingMetadataVec},
    symbols::Symbols,
  },
};

use super::LinkStage;

struct Context<'a> {
  pub visited_modules: &'a mut IndexVec<NormalModuleId, bool>,
  pub linking_infos: &'a mut LinkingMetadataVec,
  pub modules: &'a NormalModuleVec,
}

fn wrap_module_recursively(ctx: &mut Context, target: NormalModuleId) {
  let is_visited = &mut ctx.visited_modules[target];
  if *is_visited {
    return;
  }
  *is_visited = true;

  let module = &ctx.modules[target];

  if matches!(ctx.linking_infos[target].wrap_kind, WrapKind::None) {
    ctx.linking_infos[target].wrap_kind = match module.exports_kind {
      ExportsKind::Esm | ExportsKind::None => WrapKind::Esm,
      ExportsKind::CommonJs => WrapKind::Cjs,
    }
  }

  module.import_records.iter().filter_map(|rec| rec.resolved_module.as_normal()).for_each(
    |importee| {
      wrap_module_recursively(ctx, importee);
    },
  );
}

fn has_dynamic_exports_due_to_export_star(
  target: NormalModuleId,
  modules: &NormalModuleVec,
  linking_infos: &mut LinkingMetadataVec,
  visited_modules: &mut IndexVec<NormalModuleId, bool>,
) -> bool {
  if visited_modules[target] {
    return linking_infos[target].has_dynamic_exports;
  }
  visited_modules[target] = true;

  let module = &modules[target];

  if matches!(module.exports_kind, ExportsKind::CommonJs) {
    linking_infos[target].has_dynamic_exports = true;
    return true;
  }

  let has_dynamic_exports = module.star_export_module_ids().any(|importee_id| match importee_id {
    rolldown_common::ModuleId::Normal(importee_id) => {
      target != importee_id
        && has_dynamic_exports_due_to_export_star(
          importee_id,
          modules,
          linking_infos,
          visited_modules,
        )
    }
    rolldown_common::ModuleId::External(_) => true,
  });
  if has_dynamic_exports {
    linking_infos[target].has_dynamic_exports = true;
  }
  linking_infos[target].has_dynamic_exports
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn wrap_modules(&mut self) {
    let mut visited_modules_for_wrapping =
      oxc::index::index_vec![false; self.module_table.normal_modules.len()];

    let mut visited_modules_for_dynamic_exports =
      oxc::index::index_vec![false; self.module_table.normal_modules.len()];

    for module in &self.module_table.normal_modules {
      let module_id = module.id;
      let linking_info = &self.metas[module_id];

      match linking_info.wrap_kind {
        WrapKind::Cjs | WrapKind::Esm => {
          wrap_module_recursively(
            &mut Context {
              visited_modules: &mut visited_modules_for_wrapping,
              linking_infos: &mut self.metas,
              modules: &self.module_table.normal_modules,
            },
            module_id,
          );
        }
        WrapKind::None => {}
      }

      if !module.star_exports.is_empty() {
        has_dynamic_exports_due_to_export_star(
          module_id,
          &self.module_table.normal_modules,
          &mut self.metas,
          &mut visited_modules_for_dynamic_exports,
        );
      }

      module.import_records.iter().for_each(|rec| {
        let ModuleId::Normal(importee_id) = rec.resolved_module else {
          return;
        };
        let importee = &self.module_table.normal_modules[importee_id];
        if matches!(importee.exports_kind, ExportsKind::CommonJs) {
          wrap_module_recursively(
            &mut Context {
              visited_modules: &mut visited_modules_for_wrapping,
              linking_infos: &mut self.metas,
              modules: &self.module_table.normal_modules,
            },
            importee.id,
          );
        }
      });
    }
  }
}

pub fn create_wrapper(
  module: &mut NormalModule,
  linking_info: &mut LinkingMetadata,
  symbols: &mut Symbols,
  runtime: &RuntimeModuleBrief,
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
      let wrapper_ref =
        symbols.create_symbol(module.id, format!("require_{}", &module.repr_name).into());

      let stmt_info = StmtInfo {
        stmt_idx: None,
        declared_symbols: vec![wrapper_ref],
        referenced_symbols: vec![runtime.resolve_symbol("__commonJSMin").into()],
        side_effect: false,
        is_included: false,
        import_records: Vec::new(),
        debug_label: None,
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
        symbols.create_symbol(module.id, format!("init_{}", &module.repr_name).into());

      let stmt_info = StmtInfo {
        stmt_idx: None,
        declared_symbols: vec![wrapper_ref],
        referenced_symbols: vec![runtime.resolve_symbol("__esmMin").into()],
        side_effect: false,
        is_included: false,
        import_records: Vec::new(),
        debug_label: None,
      };

      linking_info.wrapper_stmt_info = Some(module.stmt_infos.add_stmt_info(stmt_info));
      linking_info.wrapper_ref = Some(wrapper_ref);
    }
    WrapKind::None => {}
  }
}
