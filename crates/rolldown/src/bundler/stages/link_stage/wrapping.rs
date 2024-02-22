use index_vec::IndexVec;
use rolldown_common::{ExportsKind, ModuleId, StmtInfo, WrapKind};

use crate::bundler::{
  module::{Module, ModuleVec, NormalModule},
  runtime::RuntimeModuleBrief,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
  utils::symbols::Symbols,
};

use super::LinkStage;

struct Context<'a> {
  pub visited_modules: &'a mut IndexVec<ModuleId, bool>,
  pub linking_infos: &'a mut LinkingMetadataVec,
  pub modules: &'a ModuleVec,
}

fn wrap_module_recursively(ctx: &mut Context, target: ModuleId) {
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

  module.import_records.iter().for_each(|rec| {
    wrap_module_recursively(ctx, rec.resolved_module);
  });
}

fn has_dynamic_exports_due_to_export_star(
  target: ModuleId,
  modules: &ModuleVec,
  linking_infos: &mut LinkingMetadataVec,
  visited_modules: &mut IndexVec<ModuleId, bool>,
) -> bool {
  if visited_modules[target] {
    return linking_infos[target].has_dynamic_exports;
  }
  visited_modules[target] = true;

  let Module::Normal(module) = &modules[target] else {
    return false;
  };

  if matches!(module.exports_kind, ExportsKind::CommonJs) {
    linking_infos[target].has_dynamic_exports = true;
    return true;
  }

  let has_dynamic_exports =
    module.star_export_modules().any(|importee_id| match &modules[importee_id] {
      Module::Normal(_) => {
        target != importee_id
          && has_dynamic_exports_due_to_export_star(
            importee_id,
            modules,
            linking_infos,
            visited_modules,
          )
      }
      Module::External(_) => true,
    });
  if has_dynamic_exports {
    linking_infos[target].has_dynamic_exports = true;
  }
  linking_infos[target].has_dynamic_exports
}

impl LinkStage<'_> {
  pub fn wrap_modules(&mut self) {
    let mut visited_modules_for_wrapping = index_vec::index_vec![false; self.modules.len()];

    let mut visited_modules_for_dynamic_exports = index_vec::index_vec![false; self.modules.len()];

    for module in &self.modules {
      let Module::Normal(module) = module else {
        return;
      };
      let module_id = module.id;
      let linking_info = &self.linking_infos[module_id];

      match linking_info.wrap_kind {
        WrapKind::Cjs | WrapKind::Esm => {
          wrap_module_recursively(
            &mut Context {
              visited_modules: &mut visited_modules_for_wrapping,
              linking_infos: &mut self.linking_infos,
              modules: &self.modules,
            },
            module_id,
          );
        }
        WrapKind::None => {}
      }

      if !module.star_exports.is_empty() {
        has_dynamic_exports_due_to_export_star(
          module_id,
          &self.modules,
          &mut self.linking_infos,
          &mut visited_modules_for_dynamic_exports,
        );
      }

      module.import_records.iter().for_each(|rec| {
        let importee = &self.modules[rec.resolved_module];
        let Module::Normal(importee) = importee else {
          return;
        };
        if matches!(importee.exports_kind, ExportsKind::CommonJs) {
          wrap_module_recursively(
            &mut Context {
              visited_modules: &mut visited_modules_for_wrapping,
              linking_infos: &mut self.linking_infos,
              modules: &self.modules,
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
        referenced_symbols: vec![runtime.resolve_symbol("__commonJSMin")],
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
        referenced_symbols: vec![runtime.resolve_symbol("__esmMin")],
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
