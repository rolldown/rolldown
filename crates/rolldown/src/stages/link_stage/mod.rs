use std::{ptr::addr_of, sync::Mutex};

use oxc_index::IndexVec;
use rolldown_common::{
  dynamic_import_usage::DynamicImportExportsUsage, EntryPoint, ExportsKind, ImportKind,
  ImportRecordIdx, ImportRecordMeta, Module, ModuleIdx, ModuleTable, OutputFormat,
  ResolvedImportRecord, RuntimeModuleBrief, StmtInfo, StmtInfoMeta, SymbolRef, SymbolRefDb,
  WrapKind,
};
use rolldown_error::BuildDiagnostic;
use rolldown_utils::{
  concat_string,
  ecmascript::legitimize_identifier_name,
  index_vec_ext::IndexVecExt,
  rayon::{IntoParallelRefIterator, ParallelIterator},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  type_alias::IndexEcmaAst,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
  SharedOptions,
};

use self::wrapping::create_wrapper;

use super::scan_stage::ScanStageOutput;

mod bind_imports_and_exports;
mod generate_lazy_export;
mod sort_modules;
pub(crate) mod tree_shaking;
mod wrapping;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub module_table: ModuleTable,
  pub entries: Vec<EntryPoint>,
  pub ast_table: IndexEcmaAst,
  // pub sorted_modules: Vec<NormalModuleId>,
  pub metas: LinkingMetadataVec,
  pub symbol_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

#[derive(Debug)]
pub struct LinkStage<'a> {
  pub module_table: ModuleTable,
  pub entries: Vec<EntryPoint>,
  pub symbols: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metas: LinkingMetadataVec,
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
  pub ast_table: IndexEcmaAst,
  pub options: &'a SharedOptions,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
}

impl<'a> LinkStage<'a> {
  pub fn new(scan_stage_output: ScanStageOutput, options: &'a SharedOptions) -> Self {
    Self {
      sorted_modules: Vec::new(),
      metas: scan_stage_output
        .module_table
        .modules
        .iter()
        .map(|module| LinkingMetadata {
          dependencies: module
            .import_records()
            .iter()
            .filter_map(|rec| {
              if options.inline_dynamic_imports || !matches!(rec.kind, ImportKind::DynamicImport) {
                Some(rec.resolved_module)
              } else {
                None
              }
            })
            .collect(),
          star_exports_from_external_modules: module.as_normal().map_or(vec![], |inner| {
            inner
              .star_exports_from_external_modules(&scan_stage_output.module_table.modules)
              .collect()
          }),
          ..LinkingMetadata::default()
        })
        .collect::<IndexVec<ModuleIdx, _>>(),
      module_table: scan_stage_output.module_table,
      entries: scan_stage_output.entry_points,
      symbols: scan_stage_output.symbol_ref_db,
      runtime: scan_stage_output.runtime,
      warnings: scan_stage_output.warnings,
      errors: vec![],
      ast_table: scan_stage_output.index_ecma_ast,
      dynamic_import_exports_usage_map: scan_stage_output.dynamic_import_exports_usage_map,
      options,
      used_symbol_refs: FxHashSet::default(),
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub fn link(mut self) -> LinkStageOutput {
    self.sort_modules();

    self.determine_module_exports_kind();
    self.wrap_modules();
    self.generate_lazy_export();
    self.bind_imports_and_exports();
    self.create_exports_for_ecma_modules();
    self.reference_needed_symbols();
    self.include_statements();
    self.patch_module_dependencies();
    tracing::trace!("meta {:#?}", self.metas.iter_enumerated().collect::<Vec<_>>());

    LinkStageOutput {
      module_table: self.module_table,
      entries: self.entries,
      // sorted_modules: self.sorted_modules,
      metas: self.metas,
      symbol_db: self.symbols,
      runtime: self.runtime,
      warnings: self.warnings,
      errors: self.errors,
      ast_table: self.ast_table,
      used_symbol_refs: self.used_symbol_refs,
      dynamic_import_exports_usage_map: self.dynamic_import_exports_usage_map,
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  fn determine_module_exports_kind(&mut self) {
    let entry_ids_set = self.entries.iter().map(|e| e.id).collect::<FxHashSet<_>>();
    self.module_table.modules.iter().filter_map(Module::as_normal).for_each(|importer| {
      // TODO(hyf0): should check if importer is a js module
      importer.import_records.iter().for_each(|rec| {
        let importee_id = rec.resolved_module;
        let Module::Normal(importee) = &self.module_table.modules[importee_id] else {
          return;
        };

        match rec.kind {
          ImportKind::Import => {
            if matches!(importee.exports_kind, ExportsKind::None)
              && !importee.meta.has_lazy_export()
            {
              // `import` a module that has `ExportsKind::None`, which will be turned into `ExportsKind::Esm`
              // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
              unsafe {
                let importee_mut = addr_of!(*importee).cast_mut();
                (*importee_mut).exports_kind = ExportsKind::Esm;
              }
            }
          }
          ImportKind::Require => match importee.exports_kind {
            ExportsKind::Esm => {
              self.metas[importee.idx].wrap_kind = WrapKind::Esm;
            }
            ExportsKind::CommonJs => {
              self.metas[importee.idx].wrap_kind = WrapKind::Cjs;
            }
            ExportsKind::None => {
              self.metas[importee.idx].wrap_kind = WrapKind::Cjs;
              // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
              // A module with `ExportsKind::None` that `require` self should be turned into `ExportsKind::CommonJs`.
              unsafe {
                let importee_mut = addr_of!(*importee).cast_mut();
                (*importee_mut).exports_kind = ExportsKind::CommonJs;
              }
            }
          },
          ImportKind::DynamicImport => {
            if self.options.inline_dynamic_imports {
              // For iife, then import() is just a require() that
              // returns a promise, so the imported file must also be wrapped
              match importee.exports_kind {
                ExportsKind::Esm => {
                  self.metas[importee.idx].wrap_kind = WrapKind::Esm;
                }
                ExportsKind::CommonJs => {
                  self.metas[importee.idx].wrap_kind = WrapKind::Cjs;
                }
                ExportsKind::None => {
                  self.metas[importee.idx].wrap_kind = WrapKind::Cjs;
                  // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
                  // A module with `ExportsKind::None` that `require` self should be turned into `ExportsKind::CommonJs`.
                  unsafe {
                    let importee_mut = addr_of!(*importee).cast_mut();
                    (*importee_mut).exports_kind = ExportsKind::CommonJs;
                  }
                }
              }
            }
          }
          ImportKind::AtImport => {
            unreachable!("A Js module would never import a CSS module via `@import`");
          }
          ImportKind::UrlImport => {
            unreachable!("A Js module would never import a CSS module via `url()`");
          }
          ImportKind::NewUrl => {}
        }
      });

      let is_entry = entry_ids_set.contains(&importer.idx);
      if matches!(importer.exports_kind, ExportsKind::CommonJs)
        && (!is_entry || matches!(self.options.format, OutputFormat::Esm))
      {
        self.metas[importer.idx].wrap_kind = WrapKind::Cjs;
      }
    });
  }

  #[allow(clippy::collapsible_if)]
  #[tracing::instrument(level = "debug", skip_all)]
  fn reference_needed_symbols(&mut self) {
    let symbols = Mutex::new(&mut self.symbols);
    let keep_names = self.options.keep_names;
    let record_meta_update_pending_pairs_list = self
      .module_table
      .modules
      .par_iter()
      .filter_map(Module::as_normal)
      .map(|importer| {
        let mut record_meta_pairs: Vec<(ImportRecordIdx, ImportRecordMeta)> = vec![];
        let importer_idx = importer.idx;
        // safety: No race conditions here:
        // - Mutating on `stmt_infos` is isolated in threads for each module
        // - Mutating on `stmt_infos` doesn't rely on other mutating operations of other modules
        // - Mutating and parallel reading is in different memory locations
        let stmt_infos = unsafe { &mut *(addr_of!(importer.stmt_infos).cast_mut()) };
        // store the symbol reference to the declared statement index
        let mut declared_symbol_for_stmt_pairs = vec![];
        stmt_infos.infos.iter_mut_enumerated().for_each(|(stmt_idx, stmt_info)| {
          stmt_info.import_records.iter().for_each(|rec_id| {
            let rec = &importer.import_records[*rec_id];
            let rec_resolved_module = &self.module_table.modules[rec.resolved_module];
            if !rec_resolved_module.is_normal()
              || is_external_dynamic_import(&self.module_table, rec, importer_idx)
            {
              if matches!(rec.kind, ImportKind::Require)
                || !self.options.format.keep_esm_import_export_syntax()
              {
                if self.options.format.should_call_runtime_require()
                  && self.options.polyfill_require_for_esm_format_with_node_platform()
                {
                  stmt_info
                    .referenced_symbols
                    .push(self.runtime.resolve_symbol("__require").into());
                  record_meta_pairs.push((*rec_id, ImportRecordMeta::CALL_RUNTIME_REQUIRE));
                }
              }
            }
            match rec_resolved_module {
              Module::External(importee) => {
                // Make sure symbols from external modules are included and de_conflicted
                match rec.kind {
                  ImportKind::Import => {
                    let is_reexport_all = rec.meta.contains(ImportRecordMeta::IS_EXPORT_STAR);
                    if is_reexport_all {
                      // export * from 'external' would be just removed. So it references nothing.
                      rec.namespace_ref.set_name(
                        &mut symbols.lock().unwrap(),
                        &concat_string!("import_", legitimize_identifier_name(&importee.name)),
                      );
                    } else {
                      // import ... from 'external' or export ... from 'external'
                      if matches!(
                        self.options.format,
                        OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd
                      ) && !rec.meta.contains(ImportRecordMeta::IS_PLAIN_IMPORT)
                      {
                        stmt_info.side_effect = true;
                        stmt_info
                          .referenced_symbols
                          .push(self.runtime.resolve_symbol("__toESM").into());
                      }
                    }
                  }
                  _ => {}
                }
              }
              Module::Normal(importee) => {
                let importee_linking_info = &self.metas[importee.idx];
                match rec.kind {
                  ImportKind::Import => {
                    let is_reexport_all = rec.meta.contains(ImportRecordMeta::IS_EXPORT_STAR);
                    match importee_linking_info.wrap_kind {
                      WrapKind::None => {
                        // for case:
                        // ```js
                        // // index.js
                        // export * from './foo'; /* importee wrap kind is `none`, but since `foo` has dynamic_export, we need to preserve the `__reExport(index_exports, foo_ns)` */
                        //
                        // // foo.js
                        // export * from './bar' /* importee wrap kind is `cjs`, preserve by
                        // default*/
                        //
                        // // bar.js
                        // module.exports = 1000
                        // ```
                        if is_reexport_all {
                          let meta = &self.metas[importee.idx];
                          if meta.has_dynamic_exports {
                            stmt_info.side_effect = true;
                            stmt_info
                              .referenced_symbols
                              .push(self.runtime.resolve_symbol("__reExport").into());
                            stmt_info.referenced_symbols.push(importer.namespace_object_ref.into());
                            stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
                          }
                        }
                      }
                      WrapKind::Cjs => {
                        if is_reexport_all {
                          stmt_info.side_effect = true;
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
                          stmt_info.side_effect = importee.side_effects.has_side_effects();
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
                          declared_symbol_for_stmt_pairs.push((stmt_idx, rec.namespace_ref));
                          rec.namespace_ref.set_name(
                            &mut symbols.lock().unwrap(),
                            &concat_string!("import_", importee.repr_name),
                          );
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
                      // convert require record into `(init_foo(), __toCommonJS(foo_exports))` if
                      // `require('xxx)` is used, else convert it to `init_foo()`
                      stmt_info
                        .referenced_symbols
                        .push(importee_linking_info.wrapper_ref.unwrap().into());
                      stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());

                      if !rec.meta.contains(ImportRecordMeta::IS_REQUIRE_UNUSED) {
                        stmt_info
                          .referenced_symbols
                          .push(self.runtime.resolve_symbol("__toCommonJS").into());
                      }
                    }
                  },
                  ImportKind::DynamicImport => {
                    if self.options.inline_dynamic_imports {
                      match importee_linking_info.wrap_kind {
                        WrapKind::None => {}
                        WrapKind::Cjs => {
                          //  `__toESM(require_foo())`
                          stmt_info
                            .referenced_symbols
                            .push(importee_linking_info.wrapper_ref.unwrap().into());
                          stmt_info
                            .referenced_symbols
                            .push(self.runtime.resolve_symbol("__toESM").into());
                        }
                        WrapKind::Esm => {
                          // `(init_foo(), foo_exports)`
                          stmt_info
                            .referenced_symbols
                            .push(importee_linking_info.wrapper_ref.unwrap().into());
                          stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
                        }
                      }
                    }
                  }
                  ImportKind::AtImport => {
                    unreachable!("A Js module would never import a CSS module via `@import`");
                  }
                  ImportKind::UrlImport => {
                    unreachable!("A Js module would never import a CSS module via `url()`");
                  }
                  ImportKind::NewUrl => {}
                }
              }
            }
          });
          if keep_names && stmt_info.meta.intersects(StmtInfoMeta::KeepNamesType) {
            stmt_info.referenced_symbols.push(self.runtime.resolve_symbol("__name").into());
          }
        });
        for (stmt_idx, symbol_ref) in declared_symbol_for_stmt_pairs {
          stmt_infos.declare_symbol_for_stmt(stmt_idx, symbol_ref);
        }
        (importer_idx, record_meta_pairs)
      })
      .collect::<Vec<_>>();

    // merge import_record.meta
    for (module_idx, record_meta_pairs) in record_meta_update_pending_pairs_list {
      let Some(module) = self.module_table.modules[module_idx].as_normal_mut() else {
        continue;
      };
      for (rec_id, meta) in record_meta_pairs {
        module.import_records[rec_id].meta |= meta;
      }
    }
  }

  fn create_exports_for_ecma_modules(&mut self) {
    self.module_table.modules.iter_mut().filter_map(|m| m.as_normal_mut()).for_each(
      |ecma_module| {
        let linking_info = &mut self.metas[ecma_module.idx];

        create_wrapper(ecma_module, linking_info, &mut self.symbols, &self.runtime, self.options);
        if let Some(entry) = self.entries.iter().find(|entry| entry.id == ecma_module.idx) {
          init_entry_point_stmt_info(linking_info, entry, &self.dynamic_import_exports_usage_map);
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
            meta: StmtInfoMeta::default(),
          };
          ecma_module.stmt_infos.add_stmt_info(stmt_info);
        });

        // Generate export of Module Namespace Object for Namespace Import
        // - Namespace import: https://tc39.es/ecma262/#prod-NameSpaceImport
        // - Module Namespace Object: https://tc39.es/ecma262/#sec-module-namespace-exotic-objects
        // Though Module Namespace Object is created in runtime, as a bundler, we have stimulus the behavior in compile-time and generate a
        // real statement to construct the Module Namespace Object and assign it to a variable.
        // This is only a concept of esm, so no need to care about this in commonjs.
        if matches!(ecma_module.exports_kind, ExportsKind::Esm) {
          let meta = &self.metas[ecma_module.idx];
          let mut referenced_symbols = vec![];
          let mut declared_symbols = vec![];
          if !meta.is_canonical_exports_empty() {
            referenced_symbols.push(self.runtime.resolve_symbol("__export").into());
            referenced_symbols
              .extend(meta.canonical_exports().map(|(_, export)| export.symbol_ref.into()));
          }
          if !meta.star_exports_from_external_modules.is_empty() {
            referenced_symbols.push(self.runtime.resolve_symbol("__reExport").into());
            match self.options.format {
              OutputFormat::Esm => {
                meta.star_exports_from_external_modules.iter().copied().for_each(|rec_idx| {
                  referenced_symbols.push(ecma_module.import_records[rec_idx].namespace_ref.into());
                  declared_symbols.push(ecma_module.import_records[rec_idx].namespace_ref);
                });
              }
              OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd | OutputFormat::App => {}
            }
          };
          // Create a StmtInfo to represent the statement that declares and constructs the Module Namespace Object.
          // Corresponding AST for this statement will be created by the finalizer.
          declared_symbols.push(ecma_module.namespace_object_ref);
          let namespace_stmt_info = StmtInfo {
            stmt_idx: None,
            declared_symbols,
            referenced_symbols,
            side_effect: false,
            is_included: false,
            import_records: Vec::new(),
            debug_label: None,
            meta: StmtInfoMeta::default(),
          };
          ecma_module.stmt_infos.replace_namespace_stmt_info(namespace_stmt_info);
        }
      },
    );
  }

  fn patch_module_dependencies(&mut self) {
    self.metas.par_iter_mut_enumerated().for_each(|(module_idx, meta)| {
      // Symbols from runtime are referenced by bundler not import statements.
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
        let canonical_ref = self.symbols.canonical_ref_for(*symbol_ref);
        meta.dependencies.insert(canonical_ref.owner);
      });

      let Module::Normal(module) = &self.module_table.modules[module_idx] else {
        return;
      };

      module.stmt_infos.iter().filter(|stmt_info| stmt_info.is_included).for_each(|stmt_info| {
        // We need this step to include the runtime module, if there are symbols of it.
        // TODO: Maybe we should push runtime module to `LinkingMetadata::dependencies` while pushing the runtime symbols.
        stmt_info.referenced_symbols.iter().for_each(|reference_ref| {
          match reference_ref {
            rolldown_common::SymbolOrMemberExprRef::Symbol(sym_ref) => {
              let canonical_ref = self.symbols.canonical_ref_for(*sym_ref);
              meta.dependencies.insert(canonical_ref.owner);
            }
            rolldown_common::SymbolOrMemberExprRef::MemberExpr(member_expr) => {
              if let Some(sym_ref) =
                member_expr.resolved_symbol_ref(&meta.resolved_member_expr_refs)
              {
                let canonical_ref = self.symbols.canonical_ref_for(sym_ref);
                meta.dependencies.insert(canonical_ref.owner);
              } else {
                // `None` means the member expression resolve to a ambiguous export, which means it actually resolve to nothing.
                // It would be rewrite to `undefined` in the final code, so we don't need to include anything to make `undefined` work.
              }
            }
          };
        });
      });
    });
  }
}

fn is_external_dynamic_import(
  table: &ModuleTable,
  record: &ResolvedImportRecord,
  module_idx: ModuleIdx,
) -> bool {
  record.kind == ImportKind::DynamicImport
    && table.modules[module_idx].as_normal().is_some_and(|module| module.is_user_defined_entry)
    && record.resolved_module != module_idx
}

pub fn init_entry_point_stmt_info(
  meta: &mut LinkingMetadata,
  entry: &EntryPoint,
  dynamic_import_exports_usage_map: &FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
) {
  let mut referenced_symbols = vec![];

  // Include the wrapper if present
  if !matches!(meta.wrap_kind, WrapKind::None) {
    // If a commonjs module becomes an entry point while targeting esm, we need to at least add a `export default require_foo();`
    // statement as some kind of syntax sugar. So users won't need to manually create a proxy file with `export default require('./foo.cjs')` in it.
    referenced_symbols.push(meta.wrapper_ref.unwrap());
  }

  referenced_symbols.extend(
    meta
      .referenced_canonical_exports_symbols(entry.id, entry.kind, dynamic_import_exports_usage_map)
      .map(|(_, resolved_export)| resolved_export.symbol_ref),
  );
  // Entry chunk need to generate exports, so we need reference to all exports to make sure they are included in tree-shaking.

  meta.referenced_symbols_by_entry_point_chunk.extend(referenced_symbols);
}
