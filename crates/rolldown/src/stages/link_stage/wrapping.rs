use rolldown_common::{
  NormalModule, NormalizedBundlerOptions, RuntimeModuleBrief, StmtInfo, StmtInfoMeta, StmtInfos,
  SymbolRefDb, TaggedSymbolRef, WrapKind,
};
use smallvec::smallvec;

use crate::types::linking_metadata::LinkingMetadata;

use super::LinkStage;

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn create_wrapper_declarations(&mut self) {
    for module in &self.module_table.modules {
      let Some(module) = module.as_normal() else { continue };
      let module_idx = module.idx;
      create_wrapper(
        module,
        &mut self.stmt_infos[module_idx],
        &mut self.metas[module_idx],
        &mut self.symbols,
        &self.runtime,
        self.options,
      );
    }
  }
}

pub fn create_wrapper(
  module: &NormalModule,
  stmt_infos: &mut StmtInfos,
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
      let wrapper_ref =
        symbols.create_facade_root_symbol_ref(module.idx, &format!("require_{}", module.repr_name));

      let stmt_info = StmtInfo {
        declared_symbols: smallvec![TaggedSymbolRef::normal(wrapper_ref)],
        referenced_symbols: vec![if options.profiler_names {
          runtime.resolve_symbol("__commonJS").into()
        } else {
          runtime.resolve_symbol("__commonJSMin").into()
        }],
        eval_flags: false.into(),
        import_records: Vec::new(),
        #[cfg(debug_assertions)]
        debug_label: None,
        meta: StmtInfoMeta::default(),

        force_tree_shaking: true,
      };

      linking_info.wrapper_stmt_info = Some(stmt_infos.add_stmt_info(stmt_info));
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
        symbols.create_facade_root_symbol_ref(module.idx, &format!("init_{}", module.repr_name));

      let stmt_info = StmtInfo {
        declared_symbols: smallvec![TaggedSymbolRef::normal(wrapper_ref)],
        referenced_symbols: vec![if options.profiler_names {
          runtime.resolve_symbol("__esm").into()
        } else {
          runtime.resolve_symbol("__esmMin").into()
        }],
        eval_flags: true.into(),
        import_records: Vec::new(),
        #[cfg(debug_assertions)]
        debug_label: None,
        meta: StmtInfoMeta::default(),
        force_tree_shaking: true,
      };

      linking_info.wrapper_stmt_info = Some(stmt_infos.add_stmt_info(stmt_info));
      linking_info.wrapper_ref = Some(wrapper_ref);
    }
    WrapKind::None => {}
  }
}
