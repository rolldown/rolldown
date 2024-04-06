use std::{ptr::addr_of, sync::Mutex};

use index_vec::IndexVec;
use rayon::iter::{ParallelBridge, ParallelIterator};
use rolldown_common::{
  EntryPoint, ExportsKind, ImportKind, ModuleId, NormalModule, NormalModuleId, StmtInfo, WrapKind,
};
use rolldown_error::BuildError;
use rolldown_oxc_utils::OxcProgram;

use crate::{
  runtime::RuntimeModuleBrief,
  types::{
    linking_metadata::{LinkingMetadata, LinkingMetadataVec},
    module_table::ModuleTable,
    symbols::Symbols,
  },
  SharedOptions,
};

use self::wrapping::create_wrapper;

use super::scan_stage::ScanStageOutput;

mod bind_imports_and_exports;
mod sort_modules;
mod tree_shaking;
mod wrapping;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub module_table: ModuleTable,
  pub entries: Vec<EntryPoint>,
  pub ast_table: IndexVec<NormalModuleId, OxcProgram>,
  pub sorted_modules: Vec<NormalModuleId>,
  pub metas: LinkingMetadataVec,
  pub symbols: Symbols,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildError>,
}

#[derive(Debug)]
pub struct LinkStage<'a> {
  pub module_table: ModuleTable,
  pub entries: Vec<EntryPoint>,
  pub symbols: Symbols,
  pub runtime: RuntimeModuleBrief,
  pub sorted_modules: Vec<NormalModuleId>,
  pub metas: LinkingMetadataVec,
  pub warnings: Vec<BuildError>,
  pub ast_table: IndexVec<NormalModuleId, OxcProgram>,
  pub input_options: &'a SharedOptions,
}

impl<'a> LinkStage<'a> {
  pub fn new(scan_stage_output: ScanStageOutput, input_options: &'a SharedOptions) -> Self {
    Self {
      sorted_modules: Vec::new(),
      metas: scan_stage_output
        .module_table
        .normal_modules
        .iter()
        .map(|_| LinkingMetadata::default())
        .collect::<IndexVec<NormalModuleId, _>>(),
      module_table: scan_stage_output.module_table,
      entries: scan_stage_output.entry_points,
      symbols: scan_stage_output.symbols,
      runtime: scan_stage_output.runtime,
      warnings: scan_stage_output.warnings,
      ast_table: scan_stage_output.ast_table,
      input_options,
    }
  }

  fn create_exports_for_modules(&mut self) {
    self.module_table.normal_modules.iter_mut().for_each(|module| {
      let linking_info = &mut self.metas[module.id];

      create_wrapper(module, linking_info, &mut self.symbols, &self.runtime);
      if self.entries.iter().any(|entry| entry.id == module.id) {
        init_entry_point_stmt_info(module, linking_info);
      }

      if matches!(module.exports_kind, ExportsKind::Esm) {
        let linking_info = &self.metas[module.id];
        let mut referenced_symbols = vec![];
        if !linking_info.is_canonical_exports_empty() {
          referenced_symbols
            .extend(linking_info.canonical_exports().map(|(_, export)| export.symbol_ref));
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
    tracing::info!("Start link stage");
    self.sort_modules();

    self.determine_module_exports_kind();
    self.wrap_modules();
    self.bind_imports_and_exports();
    tracing::debug!("linking modules {:#?}", self.metas);
    self.create_exports_for_modules();
    self.reference_needed_symbols();
    self.include_statements();

    LinkStageOutput {
      module_table: self.module_table,
      entries: self.entries,
      sorted_modules: self.sorted_modules,
      metas: self.metas,
      symbols: self.symbols,
      runtime: self.runtime,
      warnings: self.warnings,
      ast_table: self.ast_table,
    }
  }

  fn determine_module_exports_kind(&mut self) {
    // Maximize the compatibility with commonjs
    let compat_mode = true;

    self.module_table.normal_modules.iter().for_each(|importer| {
      importer.import_records.iter().for_each(|rec| {
        let ModuleId::Normal(importee_id) = rec.resolved_module else {
          return;
        };
        let importee = &self.module_table.normal_modules[importee_id];

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
    self.module_table.normal_modules.iter().par_bridge().for_each(|importer| {
      // safety: No race conditions here:
      // - Mutating on `stmt_infos` is isolated in threads for each module
      // - Mutating on `stmt_infos` does't rely on other mutating operations of other modules
      // - Mutating and parallel reading is in different memory locations
      let stmt_infos = unsafe { &mut *(addr_of!(importer.stmt_infos).cast_mut()) };

      stmt_infos.iter_mut().for_each(|stmt_info| {
        stmt_info.import_records.iter().for_each(|rec_id| {
          let rec = &importer.import_records[*rec_id];
          match rec.resolved_module {
            ModuleId::External(_) => {
              // Make sure symbols from external modules are included and de_conflicted
              stmt_info.side_effect = true;
            }
            ModuleId::Normal(importee_id) => {
              let importee_linking_info = &self.metas[importee_id];
              match rec.kind {
                ImportKind::Import => {
                  let is_reexport_all = importer.star_exports.contains(rec_id);
                  match importee_linking_info.wrap_kind {
                    WrapKind::None => {}
                    WrapKind::Cjs => {
                      stmt_info.side_effect = true;
                      if is_reexport_all {
                        // Turn `export * from 'bar_cjs'` into `__reExport(foo_exports, __toESM(require_bar_cjs()))`
                        // Reference to `require_bar_cjs`
                        stmt_info
                          .referenced_symbols
                          .push(importee_linking_info.wrapper_ref.unwrap());
                        stmt_info.referenced_symbols.push(self.runtime.resolve_symbol("__toESM"));
                        stmt_info
                          .referenced_symbols
                          .push(self.runtime.resolve_symbol("__reExport"));
                        stmt_info.referenced_symbols.push(importer.namespace_symbol);
                      } else {
                        // Turn `import * as bar from 'bar_cjs'` into `var import_bar_cjs = __toESM(require_bar_cjs())`
                        // Turn `import { prop } from 'bar_cjs'; prop;` into `var import_bar_cjs = __toESM(require_bar_cjs()); import_bar_cjs.prop;`
                        // Reference to `require_bar_cjs`
                        stmt_info
                          .referenced_symbols
                          .push(importee_linking_info.wrapper_ref.unwrap());
                        stmt_info.referenced_symbols.push(self.runtime.resolve_symbol("__toESM"));
                        stmt_info.declared_symbols.push(rec.namespace_ref);
                        let importee = &self.module_table.normal_modules[importee_id];
                        symbols.lock().unwrap().get_mut(rec.namespace_ref).name =
                          format!("import_{}", &importee.repr_name).into();
                      }
                    }
                    WrapKind::Esm => {
                      stmt_info.side_effect = true;
                      // Turn `import ... from 'bar_esm'` into `init_bar_esm()`
                      // Reference to `init_foo`
                      stmt_info.referenced_symbols.push(importee_linking_info.wrapper_ref.unwrap());
                      if is_reexport_all && importee_linking_info.has_dynamic_exports {
                        // Turn `export * from 'bar_esm'` into `init_bar_esm();__reExport(foo_exports, bar_esm_exports);`
                        // something like `__reExport(foo_exports, other_exports)`
                        stmt_info
                          .referenced_symbols
                          .push(self.runtime.resolve_symbol("__reExport"));
                        stmt_info.referenced_symbols.push(importer.namespace_symbol);
                        let importee = &self.module_table.normal_modules[importee_id];
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
                    let importee = &self.module_table.normal_modules[importee_id];
                    stmt_info.referenced_symbols.push(importee.namespace_symbol);
                  }
                },
                ImportKind::DynamicImport => {}
              }
            }
          }
        });
      });
    });
  }
}

pub fn init_entry_point_stmt_info(module: &mut NormalModule, meta: &mut LinkingMetadata) {
  let mut referenced_symbols = vec![];
  if matches!(module.exports_kind, ExportsKind::CommonJs) {
    // If a commonjs module becomes an entry point while targeting esm, we need to at least add a `export default require_foo();`
    // statement as some kind of syntax sugar. So users won't need to manually create a proxy file with `export default require('./foo.cjs')` in it.
    referenced_symbols.push(meta.wrapper_ref.unwrap());
  }

  // Make sure all exports are included
  referenced_symbols.extend(meta.canonical_exports().map(|(_, export)| export.symbol_ref));

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
