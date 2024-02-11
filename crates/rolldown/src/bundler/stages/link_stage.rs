use std::ptr::addr_of;

use index_vec::IndexVec;
use rolldown_common::{EntryPoint, ExportsKind, ImportKind, ModuleId, StmtInfo, WrapKind};
use rolldown_error::BuildError;
use rolldown_oxc::OxcProgram;
use rustc_hash::FxHashSet;

use crate::bundler::{
  linker::{
    linker::ImportExportLinker,
    linker_info::{LinkingInfo, LinkingInfoVec},
  },
  module::{Module, ModuleVec},
  runtime::RuntimeModuleBrief,
  utils::symbols::Symbols,
};

use super::scan_stage::ScanStageOutput;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub modules: ModuleVec,
  pub entries: Vec<EntryPoint>,
  pub ast_table: IndexVec<ModuleId, OxcProgram>,
  pub sorted_modules: Vec<ModuleId>,
  pub linking_infos: LinkingInfoVec,
  pub symbols: Symbols,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildError>,
}

#[derive(Debug)]
pub struct LinkStage {
  pub modules: ModuleVec,
  pub entries: Vec<EntryPoint>,
  pub symbols: Symbols,
  pub runtime: RuntimeModuleBrief,
  pub sorted_modules: Vec<ModuleId>,
  pub linking_infos: LinkingInfoVec,
  pub warnings: Vec<BuildError>,
  pub ast_table: IndexVec<ModuleId, OxcProgram>,
}

impl LinkStage {
  pub fn new(scan_stage_output: ScanStageOutput) -> Self {
    Self {
      sorted_modules: Vec::new(),
      linking_infos: scan_stage_output
        .modules
        .iter()
        .map(|_| LinkingInfo::default())
        .collect::<IndexVec<ModuleId, _>>(),
      modules: scan_stage_output.modules,
      entries: scan_stage_output.entry_points,
      symbols: scan_stage_output.symbols,
      runtime: scan_stage_output.runtime,
      warnings: scan_stage_output.warnings,
      ast_table: scan_stage_output.ast_table,
    }
  }

  fn create_exports_for_modules(&mut self) {
    self.modules.iter_mut().for_each(|module| {
      let Module::Normal(module) = module else {
        return;
      };

      // Create a StmtInfo for the namespace statement
      let namespace_stmt_info = StmtInfo {
        stmt_idx: None,
        declared_symbols: vec![module.namespace_symbol],
        referenced_symbols: vec![self.runtime.resolve_symbol("__export")],
        side_effect: false,
        is_included: true,
      };

      let _namespace_stmt_id = module.stmt_infos.add_stmt_info(namespace_stmt_info);

      // We don't create actual ast nodes for the namespace statement here. It will be deferred
      // to the finalize stage.
    });
  }

  #[tracing::instrument(skip_all)]
  pub fn link(mut self) -> LinkStageOutput {
    self.sort_modules();

    self.determine_module_exports_kind();
    self.wrap_modules();
    let mut linking_infos = std::mem::take(&mut self.linking_infos);
    ImportExportLinker::new(&mut self).link(&mut linking_infos);
    self.linking_infos = linking_infos;
    tracing::debug!("linking modules {:#?}", self.linking_infos);
    self.create_exports_for_modules();
    self.reference_needed_symbols();
    // FIXME: should move `linking_info.facade_stmt_infos` into a separate field
    for (id, linking_info) in self.linking_infos.iter_mut_enumerated() {
      std::mem::take(&mut linking_info.facade_stmt_infos).into_iter().for_each(|info| {
        if let Module::Normal(module) = &mut self.modules[id] {
          module.stmt_infos.add_stmt_info(info);
        }
      });
    }
    LinkStageOutput {
      modules: self.modules,
      entries: self.entries,
      sorted_modules: self.sorted_modules,
      linking_infos: self.linking_infos,
      symbols: self.symbols,
      runtime: self.runtime,
      warnings: self.warnings,
      ast_table: self.ast_table,
    }
  }

  fn sort_modules(&mut self) {
    let mut stack = self
      .entries
      .iter()
      .map(|entry_point| Action::Enter(entry_point.id))
      .rev()
      .collect::<Vec<_>>();
    // The runtime module should always be the first module to be executed
    stack.push(Action::Enter(self.runtime.id()));
    let mut entered_ids: FxHashSet<ModuleId> = FxHashSet::default();
    entered_ids.shrink_to(self.modules.len());
    let mut sorted_modules = Vec::with_capacity(self.modules.len());
    let mut next_exec_order = 0;
    while let Some(action) = stack.pop() {
      let module = &mut self.modules[action.module_id()];
      match action {
        Action::Enter(id) => {
          if !entered_ids.contains(&id) {
            entered_ids.insert(id);
            stack.push(Action::Exit(id));
            stack.extend(
              module
                .import_records()
                .iter()
                .filter(|rec| rec.kind.is_static())
                .map(|rec| rec.resolved_module)
                .rev()
                .map(Action::Enter),
            );
          }
        }
        Action::Exit(id) => {
          *module.exec_order_mut() = next_exec_order;
          next_exec_order += 1;
          sorted_modules.push(id);
        }
      }
    }
    self.sorted_modules = sorted_modules;
    debug_assert_eq!(
      self.sorted_modules.first().copied(),
      Some(self.runtime.id()),
      "runtime module should always be the first module in the sorted modules"
    );
  }

  fn determine_module_exports_kind(&mut self) {
    // Maximize the compatibility with commonjs
    let compat_mode = true;

    self.sorted_modules.iter().copied().for_each(|importer_id| {
      let Module::Normal(importer) = &self.modules[importer_id] else {
        return;
      };

      importer.import_records.iter().for_each(|rec| {
        let Module::Normal(importee) = &self.modules[rec.resolved_module] else {
          return;
        };

        match rec.kind {
          ImportKind::Import => {
            if matches!(importee.exports_kind, ExportsKind::None) {
              if compat_mode {
                // See https://github.com/evanw/esbuild/issues/447
                if (rec.contains_import_default || rec.contains_import_star)
                  && matches!(importee.exports_kind, ExportsKind::None)
                {
                  self.linking_infos[importee.id].wrap_kind = WrapKind::Cjs;
                  // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
                  unsafe {
                    let importee_mut = addr_of!(*importee).cast_mut();
                    (*importee_mut).exports_kind = ExportsKind::CommonJs;
                  }
                }
              } else {
                self.linking_infos[importee.id].wrap_kind = WrapKind::Esm;
                unsafe {
                  let importee_mut = addr_of!(*importee).cast_mut();
                  (*importee_mut).exports_kind = ExportsKind::Esm;
                }
              }
            }
          }
          ImportKind::Require => match importee.exports_kind {
            ExportsKind::Esm => {
              self.linking_infos[importee.id].wrap_kind = WrapKind::Esm;
            }
            ExportsKind::CommonJs => {
              self.linking_infos[importee.id].wrap_kind = WrapKind::Cjs;
            }
            ExportsKind::None => {
              if compat_mode {
                self.linking_infos[importee.id].wrap_kind = WrapKind::Cjs;
                // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
                // A module with `ExportsKind::None` that `require` self should be turned into `ExportsKind::CommonJs`.
                unsafe {
                  let importee_mut = addr_of!(*importee).cast_mut();
                  (*importee_mut).exports_kind = ExportsKind::CommonJs;
                }
              } else {
                self.linking_infos[importee.id].wrap_kind = WrapKind::Esm;
                unsafe {
                  let importee_mut = addr_of!(*importee).cast_mut();
                  (*importee_mut).exports_kind = ExportsKind::Esm;
                }
              }
            }
          },
          ImportKind::DynamicImport => {}
        }
      });

      // TODO: should care about output format
      if matches!(importer.exports_kind, ExportsKind::CommonJs) {
        self.linking_infos[importer.id].wrap_kind = WrapKind::Cjs;
      }
    });
  }

  fn wrap_modules(&mut self) {
    let mut processed_modules = index_vec::index_vec![false; self.modules.len()];
    self.sorted_modules.iter().copied().for_each(|module_id| {
      let linking_info = &self.linking_infos[module_id];
      let Module::Normal(module) = &self.modules[module_id] else {
        return;
      };

      match linking_info.wrap_kind {
        WrapKind::Cjs | WrapKind::Esm => {
          wrap_module(
            &mut WrappingContext {
              processed_modules: &mut processed_modules,
              linking_infos: &mut self.linking_infos,
              modules: &self.modules,
              runtime: &self.runtime,
              symbols: &mut self.symbols,
            },
            module_id,
          );
        }
        WrapKind::None => {}
      }

      module.import_records.iter().for_each(|rec| {
        let importee = &self.modules[rec.resolved_module];
        let Module::Normal(importee) = importee else {
          return;
        };
        if matches!(importee.exports_kind, ExportsKind::CommonJs) {
          wrap_module(
            &mut WrappingContext {
              processed_modules: &mut processed_modules,
              linking_infos: &mut self.linking_infos,
              modules: &self.modules,
              runtime: &self.runtime,
              symbols: &mut self.symbols,
            },
            importee.id,
          );
        }
      });
    });

    // TODO(hyf0): should merge this loop with the other one
    self.sorted_modules.iter().copied().for_each(|module_id| {
      create_wrapper(
        &mut WrappingContext {
          processed_modules: &mut processed_modules,
          linking_infos: &mut self.linking_infos,
          modules: &self.modules,
          runtime: &self.runtime,
          symbols: &mut self.symbols,
        },
        module_id,
      );
    });
  }

  fn reference_needed_symbols(&mut self) {
    self.modules.iter().for_each(|importer| {
      let Module::Normal(importer) = importer else {
        return;
      };

      importer.static_imports().for_each(|rec| {
        let Module::Normal(importee) = &self.modules[rec.resolved_module] else {
          return;
        };
        // Reference runtime symbols in importers of wrapped modules
        match self.linking_infos[importee.id].wrap_kind {
          WrapKind::Cjs | WrapKind::Esm => {
            let importee_wrapper_ref =
              self.linking_infos[importee.id].wrapper_ref.expect("Should have wrapper ref");
            self.linking_infos[importer.id]
              .reference_symbol_in_facade_stmt_infos(importee_wrapper_ref);

            match (rec.kind, importee.exports_kind) {
              (ImportKind::Import, ExportsKind::CommonJs) => {
                self.linking_infos[importer.id]
                  .reference_symbol_in_facade_stmt_infos(self.runtime.resolve_symbol("__toESM"));
              }
              (ImportKind::Require, ExportsKind::Esm) => {
                self.linking_infos[importer.id]
                  .reference_symbol_in_facade_stmt_infos(importee.namespace_symbol);
                self.linking_infos[importer.id].reference_symbol_in_facade_stmt_infos(
                  self.runtime.resolve_symbol("__toCommonJS"),
                );
              }
              _ => {}
            }
          }
          WrapKind::None => {}
        }
      });

      importer.star_export_modules().for_each(|importee_id| {
        let Module::Normal(importee) = &self.modules[importee_id] else {
          return;
        };
        if importee.exports_kind == ExportsKind::CommonJs {
          self.linking_infos[importer.id]
            .reference_symbol_in_facade_stmt_infos(self.runtime.resolve_symbol("__reExport"));
          self.linking_infos[importer.id]
            .reference_symbol_in_facade_stmt_infos(self.runtime.resolve_symbol("__toESM"));
        }
      });
    });
  }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Action {
  Enter(ModuleId),
  Exit(ModuleId),
}

impl Action {
  #[inline]
  fn module_id(&self) -> ModuleId {
    match self {
      Self::Enter(id) | Self::Exit(id) => *id,
    }
  }
}

struct WrappingContext<'a> {
  pub processed_modules: &'a mut IndexVec<ModuleId, bool>,
  pub linking_infos: &'a mut LinkingInfoVec,
  pub modules: &'a ModuleVec,
  pub runtime: &'a RuntimeModuleBrief,
  pub symbols: &'a mut Symbols,
}
fn wrap_module(ctx: &mut WrappingContext, target: ModuleId) {
  let is_processed = &mut ctx.processed_modules[target];
  if *is_processed {
    return;
  }
  *is_processed = true;

  if target == ctx.runtime.id() {
    return;
  }

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
    wrap_module(ctx, rec.resolved_module);
  });
}

fn create_wrapper(ctx: &mut WrappingContext, target: ModuleId) {
  let linking_info = &mut ctx.linking_infos[target];
  let Module::Normal(module) = &ctx.modules[target] else {
    return;
  };
  match linking_info.wrap_kind {
    // If this is a CommonJS file, we're going to need to generate a wrapper
    // for the CommonJS closure. That will end up looking something like this:
    //
    //   var require_foo = __commonJS((exports, module) => {
    //     ...
    //   });
    //
    WrapKind::Cjs => {
      linking_info
        .reference_symbol_in_facade_stmt_infos(ctx.runtime.resolve_symbol("__commonJSMin"));

      linking_info.wrapper_ref = Some(module.declare_symbol(
        format!("require_{}", &module.repr_name).into(),
        linking_info,
        ctx.symbols,
      ));
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
      linking_info.reference_symbol_in_facade_stmt_infos(ctx.runtime.resolve_symbol("__esmMin"));
      linking_info.wrapper_ref = Some(module.declare_symbol(
        format!("init_{}", &module.repr_name).into(),
        linking_info,
        ctx.symbols,
      ));
    }
    WrapKind::None => {}
  }
}
