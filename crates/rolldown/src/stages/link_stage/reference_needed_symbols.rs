use std::ptr::addr_of;

use rolldown_common::{
  ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta, Module, ModuleIdx, ModuleTable,
  OutputFormat, ResolvedImportRecord, RuntimeHelper, StmtInfoMeta, SymbolRefDb, TaggedSymbolRef,
  WrapKind, side_effects::DeterminedSideEffects,
};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  concat_string,
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
};

use super::LinkStage;
use crate::utils::external_import_interop::import_record_needs_interop;

fn is_external_dynamic_import(
  table: &ModuleTable,
  record: &ResolvedImportRecord,
  module_idx: ModuleIdx,
) -> bool {
  record.kind == ImportKind::DynamicImport
    && table.modules[module_idx].as_normal().is_some_and(|module| module.is_user_defined_entry)
    && record.resolved_module != module_idx
}

struct DeferUpdateInfo {
  record_meta_pairs: Vec<(ImportRecordIdx, ImportRecordMeta)>,
  /// Whether to set this module's side_effects to Analyzed(true).
  /// This is deferred to avoid race conditions when reading importee.side_effects
  /// while another thread might be writing to it.
  set_side_effects_true: bool,
}
impl LinkStage<'_> {
  #[expect(clippy::collapsible_if, clippy::too_many_lines)]
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn reference_needed_symbols(&mut self) {
    // Since each module only access its own symbol ref db, we use zip rather than a Mutex to
    // access the symbol db in parallel.
    let old_symbol_db = std::mem::take(&mut self.symbols);
    let mut symbols_inner = old_symbol_db.into_inner();
    let keep_names = self.options.keep_names;
    let commonjs_treeshake = self.options.treeshake.commonjs();

    // Pre-compute which modules need side_effects upgraded to true.
    // This must be done before the parallel section to avoid race conditions where
    // one thread reads importee.side_effects while another writes to it.
    // A module needs upgrade if it has `export * from 'wrapped-module'` where the
    // wrapped module is CJS or ESM wrapped, or has dynamic exports.
    let mut side_effects_upgrades: Vec<bool> = vec![false; self.module_table.modules.len()];
    for module in &self.module_table.modules {
      let Module::Normal(importer) = module else { continue };
      for rec in &importer.import_records {
        if rec.kind != ImportKind::Import {
          continue;
        }
        let is_reexport_all = rec.meta.contains(ImportRecordMeta::IsExportStar);
        if !is_reexport_all {
          continue;
        }
        let Module::Normal(importee) = &self.module_table[rec.resolved_module] else { continue };
        let importee_linking_info = &self.metas[importee.idx];
        let needs_upgrade = match importee_linking_info.wrap_kind() {
          WrapKind::None => self.metas[importee.idx].has_dynamic_exports,
          WrapKind::Cjs | WrapKind::Esm => true,
        };
        if needs_upgrade {
          side_effects_upgrades[importer.idx.index()] = true;
        }
      }
    }

    // Build snapshot of side_effects including the upgrades
    let side_effects_snapshot: Vec<bool> = self
      .module_table
      .modules
      .iter()
      .enumerate()
      .map(|(idx, m)| side_effects_upgrades[idx] || m.side_effects().has_side_effects())
      .collect();

    let defer_update_info_list = self
      .module_table
      .modules
      .par_iter()
      .zip(symbols_inner.par_iter_mut())
      .filter_map(|(module, symbol_db)| module.as_normal().map(|importer| (importer, symbol_db)))
      .map(|(importer, symbol_ref_for_module)| {
        let symbol_db =
          symbol_ref_for_module.as_mut().expect("normal module should have symbol db");
        let mut record_meta_pairs: Vec<(ImportRecordIdx, ImportRecordMeta)> = vec![];
        let mut set_side_effects_true = false;
        let importer_idx = importer.idx;
        // safety: No race conditions here:
        // - Mutating on `stmt_infos` is isolated in threads for each module
        // - Mutating on `stmt_infos` doesn't rely on other mutating operations of other modules
        // - Mutating and parallel reading is in different memory locations
        let stmt_infos = unsafe { &mut *(addr_of!(importer.stmt_infos).cast_mut()) };
        let depended_runtime_helper_map =
          unsafe { &mut *(addr_of!(importer.depended_runtime_helper).cast_mut()) };
        let mut symbols_to_be_declared = vec![];
        stmt_infos.infos.iter_mut_enumerated().for_each(|(stmt_info_idx, stmt_info)| {
          if stmt_info.meta.contains(StmtInfoMeta::HasDummyRecord) {
            depended_runtime_helper_map[RuntimeHelper::Require.bit_index()].push(stmt_info_idx);
          }
          stmt_info.import_records.iter().for_each(|rec_id| {
            let rec = &importer.import_records[*rec_id];
            let rec_resolved_module = &self.module_table[rec.resolved_module];
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
                  record_meta_pairs.push((*rec_id, ImportRecordMeta::CallRuntimeRequire));
                }
              }
            }
            match rec_resolved_module {
              Module::External(importee) => {
                // Make sure symbols from external modules are included and de_conflicted
                match rec.kind {
                  ImportKind::Import => {
                    let is_reexport_all = rec.meta.contains(ImportRecordMeta::IsExportStar);
                    if is_reexport_all {
                      // export * from 'external' would be just removed. So it references nothing.
                      symbol_db.ast_scopes.set_symbol_name(
                        rec.namespace_ref.symbol,
                        &concat_string!("import_", &importee.identifier_name),
                      );
                    } else {
                      // import ... from 'external' or export ... from 'external'
                      if matches!(
                        self.options.format,
                        OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd
                      ) {
                        stmt_info.side_effect = true.into();
                        // Only reference __toESM if this import needs interop (namespace or default import)
                        if import_record_needs_interop(importer, *rec_id) {
                          depended_runtime_helper_map[RuntimeHelper::ToEsm.bit_index()]
                            .push(stmt_info_idx);
                        }
                      }
                    }
                  }
                  ImportKind::DynamicImport => {
                    // When format is CJS and dynamicImportInCjs is false, we need __toESM
                    // to wrap the require call: `Promise.resolve().then(() => __toESM(require("external")))`
                    if matches!(self.options.format, OutputFormat::Cjs)
                      && !self.options.dynamic_import_in_cjs
                    {
                      depended_runtime_helper_map[RuntimeHelper::ToEsm.bit_index()]
                        .push(stmt_info_idx);
                    }
                  }
                  _ => {}
                }
              }
              Module::Normal(importee) => {
                let importee_linking_info = &self.metas[importee.idx];

                match rec.kind {
                  ImportKind::Import => {
                    let is_reexport_all = rec.meta.contains(ImportRecordMeta::IsExportStar);
                    match importee_linking_info.wrap_kind() {
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
                            set_side_effects_true = true;
                            stmt_info.side_effect = true.into();
                            stmt_info.meta.insert(StmtInfoMeta::ReExportDynamicExports);
                            depended_runtime_helper_map[RuntimeHelper::ReExport.bit_index()]
                              .push(stmt_info_idx);
                            stmt_info.referenced_symbols.push(importer.namespace_object_ref.into());
                            stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
                          }
                        }
                      }
                      WrapKind::Cjs => {
                        if is_reexport_all {
                          set_side_effects_true = true;
                          stmt_info.side_effect = true.into();
                          // Turn `export * from 'bar_cjs'` into `__reExport(foo_exports, __toESM(require_bar_cjs()))`
                          // Reference to `require_bar_cjs`
                          stmt_info
                            .referenced_symbols
                            .push(importee_linking_info.wrapper_ref.unwrap().into());
                          depended_runtime_helper_map[RuntimeHelper::ToEsm.bit_index()]
                            .push(stmt_info_idx);
                          depended_runtime_helper_map[RuntimeHelper::ReExport.bit_index()]
                            .push(stmt_info_idx);
                          if !commonjs_treeshake {
                            stmt_info.referenced_symbols.push(importer.namespace_object_ref.into());
                          }
                        } else {
                          // Use snapshot to avoid race condition with concurrent writes
                          stmt_info.side_effect =
                            side_effects_snapshot[importee.idx.index()].into();

                          // Turn `import * as bar from 'bar_cjs'` into `var import_bar_cjs = __toESM(require_bar_cjs())`
                          // Turn `import foo from 'bar_cjs'; foo;` into `var import_bar_cjs = __toESM(require_bar_cjs()); import_bar_cjs.default;`
                          // Turn `import { prop } from 'bar_cjs'; prop;` into `var import_bar_cjs = require_bar_cjs(); import_bar_cjs.prop;`
                          // Reference to `require_bar_cjs`
                          stmt_info
                            .referenced_symbols
                            .push(importee_linking_info.wrapper_ref.unwrap().into());
                          // Only reference __toESM if this import needs interop (namespace or default import)
                          let needs_toesm = if let Some(info) =
                            self.safely_merge_cjs_ns_map.get(&rec.resolved_module)
                          {
                            info.needs_interop
                          } else {
                            import_record_needs_interop(importer, *rec_id)
                          };
                          if needs_toesm {
                            depended_runtime_helper_map[RuntimeHelper::ToEsm.bit_index()]
                              .push(stmt_info_idx);
                          }
                          symbols_to_be_declared.push((rec.namespace_ref, stmt_info_idx));
                          symbol_db.ast_scopes.set_symbol_name(
                            rec.namespace_ref.symbol,
                            &concat_string!("import_", importee.repr_name),
                          );
                        }
                      }
                      WrapKind::Esm => {
                        // Turn `import ... from 'bar_esm'` into `init_bar_esm()`
                        // Use snapshot to avoid race condition with concurrent writes
                        stmt_info.side_effect =
                          (is_reexport_all || side_effects_snapshot[importee.idx.index()]).into();
                        // Reference to `init_foo`
                        stmt_info
                          .referenced_symbols
                          .push(importee_linking_info.wrapper_ref.unwrap().into());

                        if is_reexport_all {
                          // This branch means this module contains code like `export * from './some-wrapped-module.js'`.
                          // We need to mark this module as having side effects, so it could be included forcefully and
                          // responsible for generating `init_xxx_dep` calls to ensure deps got initialized correctly.
                          set_side_effects_true = true;
                        }
                        if is_reexport_all && importee_linking_info.has_dynamic_exports {
                          // Turn `export * from 'bar_esm'` into `init_bar_esm();__reExport(foo_exports, bar_esm_exports);`
                          // something like `__reExport(foo_exports, other_exports)`
                          depended_runtime_helper_map[RuntimeHelper::ReExport.bit_index()]
                            .push(stmt_info_idx);
                          stmt_info.meta.insert(StmtInfoMeta::ReExportDynamicExports);
                          stmt_info.referenced_symbols.push(importer.namespace_object_ref.into());
                          stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
                        }
                      }
                    }
                  }
                  ImportKind::Require => match importee_linking_info.wrap_kind() {
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

                      if !rec.meta.contains(ImportRecordMeta::IsRequireUnused) {
                        depended_runtime_helper_map[RuntimeHelper::ToCommonJs.bit_index()]
                          .push(stmt_info_idx);
                      }
                    }
                  },
                  ImportKind::DynamicImport => {
                    if self.options.inline_dynamic_imports {
                      match importee_linking_info.wrap_kind() {
                        WrapKind::None => {}
                        WrapKind::Cjs => {
                          //  `__toESM(require_foo())`
                          stmt_info
                            .referenced_symbols
                            .push(importee_linking_info.wrapper_ref.unwrap().into());
                          depended_runtime_helper_map[RuntimeHelper::ToEsm.bit_index()]
                            .push(stmt_info_idx);
                        }
                        WrapKind::Esm => {
                          // `(init_foo(), foo_exports)`
                          stmt_info
                            .referenced_symbols
                            .push(importee_linking_info.wrapper_ref.unwrap().into());
                          stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
                        }
                      }
                    } else {
                      match &importee.exports_kind {
                        ExportsKind::CommonJs => {
                          // `import('./some-cjs-module.js')` would be converted to
                          // `import('./some-cjs-module.js').then(__toDynamicImportESM(isNodeMode))`
                          depended_runtime_helper_map
                            [RuntimeHelper::ToDynamicImportEsm.bit_index()]
                          .push(stmt_info_idx);
                        }
                        ExportsKind::Esm | ExportsKind::None => {}
                      }
                    }
                  }
                  ImportKind::AtImport => {
                    unreachable!("A Js module would never import a CSS module via `@import`");
                  }
                  ImportKind::UrlImport => {
                    unreachable!("A Js module would never import a CSS module via `url()`");
                  }
                  ImportKind::NewUrl | ImportKind::HotAccept => {}
                }
              }
            }
          });
          if keep_names && stmt_info.meta.intersects(StmtInfoMeta::KeepNamesType) {
            depended_runtime_helper_map[RuntimeHelper::Name.bit_index()].push(stmt_info_idx);
          }
        });

        symbols_to_be_declared.into_iter().for_each(|(symbol_ref, idx)| {
          stmt_infos.declare_symbol_for_stmt(idx, TaggedSymbolRef::Normal(symbol_ref));
        });
        (importer_idx, DeferUpdateInfo { record_meta_pairs, set_side_effects_true })
      })
      .collect::<Vec<_>>();

    // Apply deferred updates after parallel section to avoid race conditions.
    // Since `par_iter + collect` could ensure the order of items is the same as original,
    // we could just push items in order here.
    for (module_idx, defer_update_info) in defer_update_info_list {
      let Some(module) = self.module_table[module_idx].as_normal_mut() else {
        continue;
      };
      for (rec_id, meta) in defer_update_info.record_meta_pairs {
        module.import_records[rec_id].meta |= meta;
      }
      // Apply side_effects update. This was deferred to avoid race conditions where
      // one thread reads importee.side_effects while another writes to it.
      if defer_update_info.set_side_effects_true {
        module.side_effects = DeterminedSideEffects::Analyzed(true);
      }
    }

    self.symbols =
      SymbolRefDb::new(self.options.transform_options.is_jsx_preserve()).with_inner(symbols_inner);
  }
}
