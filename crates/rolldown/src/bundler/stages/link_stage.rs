use std::ptr::addr_of;

use index_vec::IndexVec;
use rolldown_common::{
  EntryPoint, ExportsKind, ImportKind, ModuleId, StmtInfoId, SymbolRef, WrapKind,
};
use rolldown_error::BuildError;
use rustc_hash::FxHashSet;

use crate::bundler::{
  linker::{
    linker::ImportExportLinker,
    linker_info::{LinkingInfo, LinkingInfoVec},
  },
  module::{Module, ModuleVec, NormalModule},
  runtime::RuntimeModuleBrief,
  utils::symbols::Symbols,
};

use super::scan_stage::ScanStageOutput;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub modules: ModuleVec,
  pub entries: Vec<EntryPoint>,
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
    }
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
    self.reference_needed_symbols();
    // FIXME: should move `linking_info.facade_stmt_infos` into a separate field
    for (id, linking_info) in self.linking_infos.iter_mut_enumerated() {
      std::mem::take(&mut linking_info.facade_stmt_infos).into_iter().for_each(|info| {
        if let Module::Normal(module) = &mut self.modules[id] {
          module.stmt_infos.add_stmt_info(info);
        }
      });
    }
    self.include_statements();
    LinkStageOutput {
      modules: self.modules,
      entries: self.entries,
      sorted_modules: self.sorted_modules,
      linking_infos: self.linking_infos,
      symbols: self.symbols,
      runtime: self.runtime,
      warnings: self.warnings,
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
    self.sorted_modules.iter().copied().for_each(|importer_id| {
      let Module::Normal(importer) = &self.modules[importer_id] else {
        return;
      };

      importer.import_records.iter().for_each(|rec| {
        let Module::Normal(importee) = &self.modules[rec.resolved_module] else {
          return;
        };

        match rec.kind {
          ImportKind::Import | ImportKind::DynamicImport => {
            // We currently don't need to do anything here.
          }
          ImportKind::Require => match importee.exports_kind {
            ExportsKind::Esm => {
              self.linking_infos[importee.id].wrap_kind = WrapKind::Esm;
            }
            ExportsKind::CommonJs | ExportsKind::None => {
              self.linking_infos[importee.id].wrap_kind = WrapKind::Cjs;
              // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
              // A module with `ExportsKind::None` that `require` self should be turned into `ExportsKind::CommonJs`.
              unsafe {
                let importee_mut = addr_of!(*importee).cast_mut();
                (*importee_mut).exports_kind = ExportsKind::CommonJs;
              }
            }
          },
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

  fn include_statements(&mut self) {
    use rayon::prelude::*;
    struct Context<'a> {
      modules: &'a ModuleVec,
      symbols: &'a Symbols,
      is_included_vec: &'a mut IndexVec<ModuleId, IndexVec<StmtInfoId, bool>>,
    }

    fn include_symbol(ctx: &mut Context, symbol_ref: SymbolRef) {
      let mut canonical_ref = ctx.symbols.par_canonical_ref_for(symbol_ref);
      let canonical_ref_module = &ctx.modules[canonical_ref.owner];
      let canonical_ref_symbol = ctx.symbols.get(canonical_ref);
      if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
        canonical_ref = namespace_alias.namespace_ref;
      }
      let Module::Normal(canonical_ref_module) = canonical_ref_module else {
        return;
      };
      canonical_ref_module
        .stmt_infos
        .declared_stmts_by_symbol(&canonical_ref)
        .iter()
        .copied()
        .for_each(|stmt_info_id| {
          include_statement(ctx, canonical_ref_module, stmt_info_id);
        });
    }

    fn include_statement(ctx: &mut Context, module: &NormalModule, stmt_info_id: StmtInfoId) {
      let is_included = &mut ctx.is_included_vec[module.id][stmt_info_id];
      if *is_included {
        return;
      }

      // include the statement itself
      *is_included = true;

      let stmt_info = module.stmt_infos.get(stmt_info_id);

      // include statements that are referenced by this statement
      stmt_info.declared_symbols.iter().chain(stmt_info.referenced_symbols.iter()).for_each(
        |symbol_ref| {
          include_symbol(ctx, *symbol_ref);
        },
      );
    }

    let mut is_included_vec: IndexVec<ModuleId, IndexVec<StmtInfoId, bool>> = self
      .modules
      .iter()
      .map(|m| match m {
        Module::Normal(m) => {
          m.stmt_infos.iter().map(|_| false).collect::<IndexVec<StmtInfoId, _>>()
        }
        Module::External(_) => IndexVec::default(),
      })
      .collect::<IndexVec<ModuleId, _>>();

    let context = &mut Context {
      modules: &self.modules,
      symbols: &self.symbols,
      is_included_vec: &mut is_included_vec,
    };

    for module in &self.modules {
      match module {
        Module::Normal(module) => {
          let mut stmt_infos = module.stmt_infos.iter_enumerated();
          // Skip the first one, because it's the namespace variable declaration.
          // We want to include it on demand.
          stmt_infos.next();
          stmt_infos.for_each(|(stmt_info_id, stmt_info)| {
            if stmt_info.side_effect {
              include_statement(context, module, stmt_info_id);
            }
          });
          if module.is_user_defined_entry {
            let linking_info = &self.linking_infos[module.id];
            linking_info.resolved_exports.values().for_each(|resolved_export| {
              include_symbol(context, resolved_export.symbol_ref);
            });
          }
        }
        Module::External(_) => {}
      }
    }
    self.modules.iter_mut().par_bridge().for_each(|module| {
      let Module::Normal(module) = module else {
        return;
      };
      is_included_vec[module.id].iter_enumerated().for_each(|(stmt_info_id, is_included)| {
        module.stmt_infos.get_mut(stmt_info_id).is_included = *is_included;
      });
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
      linking_info.reference_symbol_in_facade_stmt_infos(ctx.runtime.resolve_symbol("__commonJS"));

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
      linking_info.reference_symbol_in_facade_stmt_infos(ctx.runtime.resolve_symbol("__esm"));
      linking_info.wrapper_ref = Some(module.declare_symbol(
        format!("init_{}", &module.repr_name).into(),
        linking_info,
        ctx.symbols,
      ));
    }
    WrapKind::None => {}
  }
}
