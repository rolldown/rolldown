use std::{ptr::addr_of, sync::Mutex};

use oxc::index::IndexVec;
use rolldown_common::{
  EntryPoint, ExportsKind, ImportKind, ModuleId, ModuleTable, NormalModule, NormalModuleId,
  NormalizedBundlerOptions, OutputFormat, StmtInfo, SymbolRef, WrapKind,
};
use rolldown_error::BuildError;
use rolldown_oxc_utils::OxcAst;
use rolldown_utils::{
  ecma_script::legitimize_identifier_name,
  rayon::{ParallelBridge, ParallelIterator},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  runtime::RuntimeModuleBrief,
  types::{
    linking_metadata::{LinkingMetadata, LinkingMetadataVec},
    symbols::Symbols,
  },
  SharedOptions,
};

use self::{tree_shaking::MemberChainToResolvedSymbolRef, wrapping::create_wrapper};

use super::scan_stage::ScanStageOutput;

mod bind_imports_and_exports;
mod sort_modules;
pub(crate) mod tree_shaking;
mod wrapping;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub module_table: ModuleTable,
  pub entries: Vec<EntryPoint>,
  pub ast_table: IndexVec<NormalModuleId, OxcAst>,
  // pub sorted_modules: Vec<NormalModuleId>,
  pub metas: LinkingMetadataVec,
  pub symbols: Symbols,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildError>,
  pub errors: Vec<BuildError>,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub top_level_member_expr_resolved_cache: FxHashMap<SymbolRef, MemberChainToResolvedSymbolRef>,
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
  pub errors: Vec<BuildError>,
  pub ast_table: IndexVec<NormalModuleId, OxcAst>,
  pub input_options: &'a SharedOptions,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub top_level_member_expr_resolved_cache: FxHashMap<SymbolRef, MemberChainToResolvedSymbolRef>,
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
      errors: scan_stage_output.errors,
      ast_table: scan_stage_output.ast_table,
      input_options,
      used_symbol_refs: FxHashSet::default(),
      top_level_member_expr_resolved_cache: FxHashMap::default(),
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  fn create_exports_for_modules(&mut self) {
    self.module_table.normal_modules.iter_mut().for_each(|module| {
      let linking_info = &mut self.metas[module.id];

      create_wrapper(module, linking_info, &mut self.symbols, &self.runtime);
      if self.entries.iter().any(|entry| entry.id == module.id) {
        init_entry_point_stmt_info(self.input_options, &self.runtime, module, linking_info);
      }

      // Create facade StmtInfo that declares variables based on the missing exports, so they can participate in the symbol de-conflict and
      // tree-shaking process.
      linking_info.shimmed_missing_exports.iter().for_each(|(_name, symbol_ref)| {
        let stmt_info = StmtInfo {
          stmt_idx: None,
          declared_symbols: vec![*symbol_ref],
          referenced_symbols: vec![],
          side_effect: false,
          is_included: false,
          import_records: Vec::new(),
          debug_label: None,
        };
        module.stmt_infos.add_stmt_info(stmt_info);
      });

      // Generate export of Module Namespace Object for Namespace Import
      // - Namespace import: https://tc39.es/ecma262/#prod-NameSpaceImport
      // - Module Namespace Object: https://tc39.es/ecma262/#sec-module-namespace-exotic-objects
      // Though Module Namespace Object is created in runtime, as a bundler, we have stimulus the behavior in compile-time and generate a
      // real statement to construct the Module Namespace Object and assign it to a variable.
      // This is only a concept of esm, so no need to care about this in commonjs.
      if matches!(module.exports_kind, ExportsKind::Esm) {
        let meta = &self.metas[module.id];
        let mut referenced_symbols = vec![];
        if !meta.is_canonical_exports_empty() {
          referenced_symbols.push(self.runtime.resolve_symbol("__export").into());
        }
        // Create a StmtInfo to represent the statement that declares and constructs the Module Namespace Object.
        // Corresponding AST for this statement will be created by the finalizer.
        let namespace_stmt_info = StmtInfo {
          stmt_idx: None,
          declared_symbols: vec![module.namespace_object_ref],
          referenced_symbols,
          side_effect: false,
          is_included: false,
          import_records: Vec::new(),
          debug_label: None,
        };
        module.stmt_infos.replace_namespace_stmt_info(namespace_stmt_info);
      }
    });
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub fn link(mut self) -> LinkStageOutput {
    self.sort_modules();

    self.determine_module_exports_kind();
    self.wrap_modules();
    self.bind_imports_and_exports();
    self.create_exports_for_modules();
    self.reference_needed_symbols();
    self.include_statements();
    tracing::trace!("meta {:#?}", self.metas.iter_enumerated().collect::<Vec<_>>());

    LinkStageOutput {
      module_table: self.module_table,
      entries: self.entries,
      // sorted_modules: self.sorted_modules,
      metas: self.metas,
      symbols: self.symbols,
      runtime: self.runtime,
      warnings: self.warnings,
      errors: self.errors,
      ast_table: self.ast_table,
      used_symbol_refs: self.used_symbol_refs,
      top_level_member_expr_resolved_cache: self.top_level_member_expr_resolved_cache,
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  fn determine_module_exports_kind(&mut self) {
    // Maximize the compatibility with commonjs
    let compat_mode = true;
    let entry_ids_set = self.entries.iter().map(|e| e.id).collect::<FxHashSet<_>>();
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
                if rec.contains_import_default || rec.contains_import_star {
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

      let is_entry = entry_ids_set.contains(&importer.id);
      if matches!(importer.exports_kind, ExportsKind::CommonJs)
        && (!is_entry || matches!(self.input_options.format, OutputFormat::Esm))
      {
        self.metas[importer.id].wrap_kind = WrapKind::Cjs;
      }
    });
  }

  #[tracing::instrument(level = "debug", skip_all)]
  fn reference_needed_symbols(&mut self) {
    let symbols = Mutex::new(&mut self.symbols);
    self.module_table.normal_modules.iter().par_bridge().for_each(|importer| {
      // safety: No race conditions here:
      // - Mutating on `stmt_infos` is isolated in threads for each module
      // - Mutating on `stmt_infos` doesn't rely on other mutating operations of other modules
      // - Mutating and parallel reading is in different memory locations
      let stmt_infos = unsafe { &mut *(addr_of!(importer.stmt_infos).cast_mut()) };

      stmt_infos.iter_mut().for_each(|stmt_info| {
        stmt_info.import_records.iter().for_each(|rec_id| {
          let rec = &importer.import_records[*rec_id];
          match rec.resolved_module {
            ModuleId::External(importee_id) => {
              // Make sure symbols from external modules are included and de_conflicted
              stmt_info.side_effect = true;
              match rec.kind {
                ImportKind::Import => {
                  if matches!(self.input_options.format, OutputFormat::Cjs) && !rec.is_plain_import
                  {
                    stmt_info
                      .referenced_symbols
                      .push(self.runtime.resolve_symbol("__toESM").into());
                  }
                  let is_reexport_all = importer.star_exports.contains(rec_id);
                  if is_reexport_all {
                    let importee = &self.module_table.external_modules[importee_id];
                    symbols.lock().unwrap().get_mut(rec.namespace_ref).name =
                      format!("import_{}", legitimize_identifier_name(&importee.name)).into();
                    stmt_info.declared_symbols.push(rec.namespace_ref);
                    stmt_info.referenced_symbols.push(importer.namespace_object_ref.into());
                    stmt_info
                      .referenced_symbols
                      .push(self.runtime.resolve_symbol("__reExport").into());
                  }
                }
                _ => {}
              }
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
                          .push(importee_linking_info.wrapper_ref.unwrap().into());
                        stmt_info
                          .referenced_symbols
                          .push(self.runtime.resolve_symbol("__toESM").into());
                        stmt_info
                          .referenced_symbols
                          .push(self.runtime.resolve_symbol("__reExport").into());
                        stmt_info.referenced_symbols.push(importer.namespace_object_ref.into());
                      } else {
                        // Turn `import * as bar from 'bar_cjs'` into `var import_bar_cjs = __toESM(require_bar_cjs())`
                        // Turn `import { prop } from 'bar_cjs'; prop;` into `var import_bar_cjs = __toESM(require_bar_cjs()); import_bar_cjs.prop;`
                        // Reference to `require_bar_cjs`
                        stmt_info
                          .referenced_symbols
                          .push(importee_linking_info.wrapper_ref.unwrap().into());
                        // dbg!(&importee_linking_info.wrapper_ref);
                        stmt_info
                          .referenced_symbols
                          .push(self.runtime.resolve_symbol("__toESM").into());
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
                      stmt_info
                        .referenced_symbols
                        .push(importee_linking_info.wrapper_ref.unwrap().into());
                      if is_reexport_all && importee_linking_info.has_dynamic_exports {
                        // Turn `export * from 'bar_esm'` into `init_bar_esm();__reExport(foo_exports, bar_esm_exports);`
                        // something like `__reExport(foo_exports, other_exports)`
                        stmt_info
                          .referenced_symbols
                          .push(self.runtime.resolve_symbol("__reExport").into());
                        stmt_info.referenced_symbols.push(importer.namespace_object_ref.into());
                        let importee = &self.module_table.normal_modules[importee_id];
                        stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
                      }
                    }
                  }
                }
                ImportKind::Require => match importee_linking_info.wrap_kind {
                  WrapKind::None => {}
                  WrapKind::Cjs => {
                    // something like `require_foo()`
                    // Reference to `require_foo`
                    stmt_info
                      .referenced_symbols
                      .push(importee_linking_info.wrapper_ref.unwrap().into());
                  }
                  WrapKind::Esm => {
                    // something like `(init_foo(), toCommonJS(foo_exports))`
                    // Reference to `init_foo`
                    stmt_info
                      .referenced_symbols
                      .push(importee_linking_info.wrapper_ref.unwrap().into());
                    stmt_info
                      .referenced_symbols
                      .push(self.runtime.resolve_symbol("__toCommonJS").into());
                    let importee = &self.module_table.normal_modules[importee_id];
                    stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
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

pub fn init_entry_point_stmt_info(
  options: &NormalizedBundlerOptions,
  runtime: &RuntimeModuleBrief,
  module: &mut NormalModule,
  meta: &mut LinkingMetadata,
) {
  let mut referenced_symbols = vec![];

  // Include the wrapper if present
  if !matches!(meta.wrap_kind, WrapKind::None) {
    // If a commonjs module becomes an entry point while targeting esm, we need to at least add a `export default require_foo();`
    // statement as some kind of syntax sugar. So users won't need to manually create a proxy file with `export default require('./foo.cjs')` in it.
    referenced_symbols.push(meta.wrapper_ref.unwrap());
  }

  // Entry chunk need to generate exports, so we need reference to all exports to make sure they are included in tree-shaking.
  referenced_symbols.extend(meta.canonical_exports().map(|(_, export)| export.symbol_ref));

  if matches!(module.exports_kind, ExportsKind::Esm) && matches!(options.format, OutputFormat::Cjs)
  {
    // We will generate `module.exports = __toCommonJS(exports);` for esm modules that are entry points
    // Include the namespace statement
    referenced_symbols.push(module.namespace_object_ref);
    referenced_symbols.push(runtime.resolve_symbol("__toCommonJS"));
  }

  meta.referenced_symbols_by_entry_point_chunk.extend(referenced_symbols);
}
