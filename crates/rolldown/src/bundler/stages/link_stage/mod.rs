use std::{ptr::addr_of, sync::Mutex};

use index_vec::IndexVec;
use rayon::iter::{ParallelBridge, ParallelIterator};
use rolldown_common::{EntryPoint, ExportsKind, ImportKind, ModuleId, StmtInfo, WrapKind};
use rolldown_error::BuildError;
use rolldown_oxc::OxcProgram;

use crate::{
  bundler::{
    module::{Module, ModuleVec, NormalModule},
    runtime::RuntimeModuleBrief,
    types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
    utils::symbols::Symbols,
  },
  InputOptions,
};

use self::wrapping::create_wrapper;

use super::scan_stage::ScanStageOutput;

mod bind_imports_and_exports;
mod tree_shaking;
mod wrapping;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub modules: ModuleVec,
  pub entries: Vec<EntryPoint>,
  pub ast_table: IndexVec<ModuleId, OxcProgram>,
  pub sorted_modules: Vec<ModuleId>,
  pub linking_infos: LinkingMetadataVec,
  pub symbols: Symbols,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildError>,
}

#[derive(Debug)]
pub struct LinkStage<'a> {
  pub modules: ModuleVec,
  pub entries: Vec<EntryPoint>,
  pub symbols: Symbols,
  pub runtime: RuntimeModuleBrief,
  pub sorted_modules: Vec<ModuleId>,
  pub metas: LinkingMetadataVec,
  pub warnings: Vec<BuildError>,
  pub ast_table: IndexVec<ModuleId, OxcProgram>,
  pub input_options: &'a InputOptions,
}

impl<'a> LinkStage<'a> {
  pub fn new(scan_stage_output: ScanStageOutput, input_options: &'a InputOptions) -> Self {
    Self {
      sorted_modules: Vec::new(),
      metas: scan_stage_output
        .modules
        .iter()
        .map(|_| LinkingMetadata::default())
        .collect::<IndexVec<ModuleId, _>>(),
      modules: scan_stage_output.modules,
      entries: scan_stage_output.entry_points,
      symbols: scan_stage_output.symbols,
      runtime: scan_stage_output.runtime,
      warnings: scan_stage_output.warnings,
      ast_table: scan_stage_output.ast_table,
      input_options,
    }
  }

  fn create_exports_for_modules(&mut self) {
    self.modules.iter_mut().for_each(|module| {
      let Module::Normal(module) = module else {
        return;
      };
      let linking_info = &mut self.metas[module.id];

      create_wrapper(module, linking_info, &mut self.symbols, &self.runtime);
      if self.entries.iter().any(|entry| entry.id == module.id) {
        init_entry_point_stmt_info(module, linking_info);
      }

      if matches!(module.exports_kind, ExportsKind::Esm) {
        let linking_info = &self.metas[module.id];
        let mut referenced_symbols = vec![];
        if !linking_info.exclude_ambiguous_sorted_resolved_exports.is_empty() {
          referenced_symbols
            .extend(linking_info.sorted_exports().map(|(_, export)| export.symbol_ref));
          referenced_symbols.push(self.runtime.resolve_symbol("__export"));
        }
        // Create a StmtInfo for the namespace statement
        let namespace_stmt_info = StmtInfo {
          stmt_idx: None,
          declared_symbols: vec![module.namespace_symbol],
          referenced_symbols,
          side_effect: false,
          is_included: false,
          import_records: Vec::new(),
          debug_label: None,
        };

        module.stmt_infos.replace_namespace_stmt_info(namespace_stmt_info);
      }

      // We don't create actual ast nodes for the namespace statement here. It will be deferred
      // to the finalize stage.
    });
  }

  pub fn link(mut self) -> LinkStageOutput {
    self.sort_modules();

    self.determine_module_exports_kind();
    self.wrap_modules();
    self.bind_imports_and_exports();
    tracing::debug!("linking modules {:#?}", self.metas);
    self.create_exports_for_modules();
    self.reference_needed_symbols();
    self.include_statements();

    LinkStageOutput {
      modules: self.modules,
      entries: self.entries,
      sorted_modules: self.sorted_modules,
      linking_infos: self.metas,
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
    let mut entered_ids = index_vec::index_vec![false; self.modules.len()];
    let mut sorted_modules = Vec::with_capacity(self.modules.len());
    let mut next_exec_order = 0;
    while let Some(action) = stack.pop() {
      let module = &mut self.modules[action.module_id()];
      match action {
        Action::Enter(id) => {
          if !entered_ids[id] {
            entered_ids[id] = true;
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
                  self.metas[importee.id].wrap_kind = WrapKind::Cjs;
                  // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
                  unsafe {
                    let importee_mut = addr_of!(*importee).cast_mut();
                    (*importee_mut).exports_kind = ExportsKind::CommonJs;
                  }
                }
              } else {
                self.metas[importee.id].wrap_kind = WrapKind::Esm;
                unsafe {
                  let importee_mut = addr_of!(*importee).cast_mut();
                  (*importee_mut).exports_kind = ExportsKind::Esm;
                }
              }
            }
          }
          ImportKind::Require => match importee.exports_kind {
            ExportsKind::Esm => {
              self.metas[importee.id].wrap_kind = WrapKind::Esm;
            }
            ExportsKind::CommonJs => {
              self.metas[importee.id].wrap_kind = WrapKind::Cjs;
            }
            ExportsKind::None => {
              if compat_mode {
                self.metas[importee.id].wrap_kind = WrapKind::Cjs;
                // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
                // A module with `ExportsKind::None` that `require` self should be turned into `ExportsKind::CommonJs`.
                unsafe {
                  let importee_mut = addr_of!(*importee).cast_mut();
                  (*importee_mut).exports_kind = ExportsKind::CommonJs;
                }
              } else {
                self.metas[importee.id].wrap_kind = WrapKind::Esm;
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
        self.metas[importer.id].wrap_kind = WrapKind::Cjs;
      }
    });
  }

  fn reference_needed_symbols(&mut self) {
    let symbols = Mutex::new(&mut self.symbols);
    self.modules.iter().par_bridge().for_each(|importer| {
      let Module::Normal(importer) = importer else {
        return;
      };

      // safety: No race conditions here:
      // - Mutating on `stmt_infos` is isolated in threads for each module
      // - Mutating on `stmt_infos` does't rely on other mutating operations of other modules
      // - Mutating and parallel reading is in different memory locations
      let stmt_infos = unsafe { &mut *(addr_of!(importer.stmt_infos).cast_mut()) };

      stmt_infos.iter_mut().for_each(|stmt_info| {
        stmt_info.import_records.iter().for_each(|rec_id| {
          let rec = &importer.import_records[*rec_id];
          let importee_id = rec.resolved_module;
          let importee_linking_info = &self.metas[importee_id];
          match rec.kind {
            ImportKind::Import => {
              let is_reexport_all = importer.star_exports.contains(rec_id);
              match importee_linking_info.wrap_kind {
                WrapKind::None => {}
                WrapKind::Cjs => {
                  if is_reexport_all {
                    // something like `__reExport(foo_exports, __toESM(require_bar()))`
                    // Reference to `require_bar`
                    stmt_info.referenced_symbols.push(importee_linking_info.wrapper_ref.unwrap());
                    stmt_info.referenced_symbols.push(self.runtime.resolve_symbol("__toESM"));
                    stmt_info.referenced_symbols.push(self.runtime.resolve_symbol("__reExport"));
                    stmt_info.referenced_symbols.push(importer.namespace_symbol);
                  } else {
                    // something like `var import_foo = __toESM(require_foo())`
                    // Reference to `require_foo`
                    stmt_info.referenced_symbols.push(importee_linking_info.wrapper_ref.unwrap());
                    stmt_info.referenced_symbols.push(self.runtime.resolve_symbol("__toESM"));
                    stmt_info.declared_symbols.push(rec.namespace_ref);
                    let Module::Normal(importee) = &self.modules[importee_id] else {
                      unreachable!("importee should be a normal module")
                    };
                    symbols.lock().unwrap().get_mut(rec.namespace_ref).name =
                      format!("import_{}", &importee.repr_name).into();
                  }
                }
                WrapKind::Esm => {
                  // something like `init_foo()`
                  // Reference to `init_foo`
                  stmt_info.referenced_symbols.push(importee_linking_info.wrapper_ref.unwrap());
                  if is_reexport_all && importee_linking_info.has_dynamic_exports {
                    // something like `__reExport(foo_exports, other_exports)`
                    stmt_info.referenced_symbols.push(self.runtime.resolve_symbol("__reExport"));
                    stmt_info.referenced_symbols.push(importer.namespace_symbol);
                    let Module::Normal(importee) = &self.modules[importee_id] else {
                      unreachable!("importee should be a normal module")
                    };
                    stmt_info.referenced_symbols.push(importee.namespace_symbol);
                  }
                }
              }
            }
            ImportKind::Require => match importee_linking_info.wrap_kind {
              WrapKind::None => {}
              WrapKind::Cjs => {
                // something like `require_foo()`
                // Reference to `require_foo`
                stmt_info.referenced_symbols.push(importee_linking_info.wrapper_ref.unwrap());
              }
              WrapKind::Esm => {
                // something like `(init_foo(), toCommonJS(foo_exports))`
                // Reference to `init_foo`
                stmt_info.referenced_symbols.push(importee_linking_info.wrapper_ref.unwrap());
                stmt_info.referenced_symbols.push(self.runtime.resolve_symbol("__toCommonJS"));
                let Module::Normal(importee) = &self.modules[importee_id] else {
                  unreachable!("importee should be a normal module")
                };
                stmt_info.referenced_symbols.push(importee.namespace_symbol);
              }
            },
            ImportKind::DynamicImport => {}
          }
        });
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

pub fn init_entry_point_stmt_info(module: &mut NormalModule, linking_info: &mut LinkingMetadata) {
  let mut referenced_symbols = vec![];
  if matches!(module.exports_kind, ExportsKind::CommonJs) {
    // If a commonjs module becomes an entry point while targeting esm, we need to at least add a `export default require_foo();`
    // statement as some kind of syntax sugar. So users won't need to manually create a proxy file with `export default require('./foo.cjs')` in it.
    referenced_symbols.push(linking_info.wrapper_ref.unwrap());
  }

  let stmt_info = StmtInfo {
    stmt_idx: None,
    declared_symbols: vec![],
    referenced_symbols,
    // Yeah, it has side effects
    side_effect: true,
    is_included: false,
    import_records: Vec::new(),
    debug_label: None,
  };

  module.stmt_infos.add_stmt_info(stmt_info);
}
