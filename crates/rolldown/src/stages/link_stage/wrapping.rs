use rolldown_common::{
  ExportsKind, ImportKind, IndexModules, Module, ModuleIdx, NormalModule, NormalizedBundlerOptions,
  RuntimeModuleBrief, StmtInfo, StmtInfoMeta, StmtInfos, SymbolRefDb, TaggedSymbolRef, WrapKind,
};
use rolldown_utils::IndexBitSet;
use smallvec::smallvec;

use crate::types::linking_metadata::{LinkingMetadata, LinkingMetadataVec};

use super::LinkStage;

struct Context<'a> {
  pub visited_modules: &'a mut IndexBitSet<ModuleIdx>,
  pub linking_infos: &'a mut LinkingMetadataVec,
  pub modules: &'a IndexModules,
  pub runtime_idx: ModuleIdx,
}

fn wrap_module_recursively(ctx: &mut Context, target: ModuleIdx) {
  // Only consider `NormalModule`
  if !ctx.visited_modules.set_bit(target) {
    return;
  }

  let Module::Normal(module) = &ctx.modules[target] else {
    return;
  };

  if target == ctx.runtime_idx {
    // Runtime module should not be wrapped.
    // FIXME(hyf0): Currently, only hmr situation will fall into this branch, we should find a better way to handle this.
    return;
  }

  if matches!(ctx.linking_infos[target].wrap_kind(), WrapKind::None) {
    let new_wrap_kind = match module.exports_kind {
      ExportsKind::Esm | ExportsKind::None => WrapKind::Esm,
      ExportsKind::CommonJs => WrapKind::Cjs,
    };
    ctx.linking_infos[target].set_wrap_kind(new_wrap_kind);
  }

  module.import_records.iter().for_each(|rec| {
    let Some(module_idx) = rec.state.resolved_module else { return };
    if matches!(rec.kind, ImportKind::Require) {
      ctx.linking_infos[module_idx].required_by_other_module = true;
    }
    wrap_module_recursively(ctx, module_idx);
  });
}

fn has_dynamic_exports_due_to_export_star(
  target: ModuleIdx,
  modules: &IndexModules,
  linking_infos: &mut LinkingMetadataVec,
  visited_modules: &mut IndexBitSet<ModuleIdx>,
) -> bool {
  if !visited_modules.set_bit(target) {
    return linking_infos[target].has_dynamic_exports;
  }

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
    let mut visited_modules_for_wrapping = IndexBitSet::new(self.module_table.modules.len());
    let mut visited_modules_for_dynamic_exports = IndexBitSet::new(self.module_table.modules.len());

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

      let need_to_wrap = !self.metas[module_id].wrap_kind().is_none();

      if need_to_wrap {
        wrap_module_recursively(
          &mut Context {
            visited_modules: &mut visited_modules_for_wrapping,
            linking_infos: &mut self.metas,
            modules: &self.module_table.modules,
            runtime_idx: self.runtime.id(),
          },
          module_id,
        );
      } else {
        // Make sure depended cjs modules got wrapped.
        module.import_records.iter().for_each(|rec| {
          let Some(module_idx) = rec.resolved_module else { return };
          let Module::Normal(importee) = &self.module_table[module_idx] else {
            return;
          };
          if matches!(rec.kind, ImportKind::Require) {
            self.metas[importee.idx].required_by_other_module = true;
          }
          // Commonjs as a dependency must be wrapped. The wrapper is like a commonjs runtime to help initialize the commonjs module correctly.
          if matches!(importee.exports_kind, ExportsKind::CommonJs) {
            wrap_module_recursively(
              &mut Context {
                visited_modules: &mut visited_modules_for_wrapping,
                linking_infos: &mut self.metas,
                modules: &self.module_table.modules,
                runtime_idx: self.runtime.id(),
              },
              importee.idx,
            );
          }
        });
      }
    }

    // Under strict execution order every CommonJS module must stay behind its lazy `require_*`
    // wrapper once a co-locating `codeSplitting` group is in play. The generate-stage order
    // lowering only wraps ESM modules, and the interop rules above leave a CommonJS module that
    // nothing imports (a CommonJS entry in cjs output) unwrapped — its body would run eagerly at
    // the top level of whatever chunk hosts it, so a group capturing it would leak one entry's
    // execution into another and let competing top-level `module.exports` assignments clobber
    // each other. Without groups no chunking mechanism moves a never-imported CommonJS module out
    // of its own entry chunk, and rendering it raw there is not only safe but load-bearing: the
    // raw body keeps the entry's real Node module contract (`module.filename`,
    // `require.main === module`, the `exports` object shape), which a wrapper's synthetic
    // `module`/`exports` parameters cannot reproduce. The runtime module is ESM, so the
    // `exports_kind` check below skips it.
    if self.options.is_strict_execution_order_enabled()
      && self.options.has_manual_code_splitting_groups()
    {
      for (idx, linking_info) in self.metas.iter_mut_enumerated() {
        let Some(module) = self.module_table[idx].as_normal() else {
          continue;
        };
        if matches!(module.exports_kind, ExportsKind::CommonJs) {
          linking_info.set_wrap_kind(WrapKind::Cjs);
        }
      }
    }

    for m in &self.module_table.modules {
      let Some(ecma_module) = m.as_normal() else { continue };
      let idx = ecma_module.idx;
      let linking_info = &mut self.metas[idx];
      let stmt_infos = &mut self.stmt_infos[idx];
      create_wrapper(
        ecma_module,
        stmt_infos,
        linking_info,
        &mut self.symbols,
        &self.runtime,
        self.options,
      );
    }
  }
}

fn create_wrapper(
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
