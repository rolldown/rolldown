use std::{ptr::addr_of, sync::Mutex};

use rolldown_common::{
  side_effects::DeterminedSideEffects, ImportKind, ImportRecordIdx, ImportRecordMeta, Module,
  ModuleIdx, ModuleTable, OutputFormat, ResolvedImportRecord, StmtInfoMeta, WrapKind,
};
use rolldown_utils::{
  concat_string,
  ecmascript::legitimize_identifier_name,
  rayon::{IntoParallelRefIterator, ParallelIterator},
};

use super::LinkStage;

fn is_external_dynamic_import(
  table: &ModuleTable,
  record: &ResolvedImportRecord,
  module_idx: ModuleIdx,
) -> bool {
  record.kind == ImportKind::DynamicImport
    && table.modules[module_idx].as_normal().is_some_and(|module| module.is_user_defined_entry)
    && record.resolved_module != module_idx
}

impl LinkStage<'_> {
  #[allow(clippy::collapsible_if)]
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn reference_needed_symbols(&mut self) {
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
        let importer_side_effect = unsafe { &mut *(addr_of!(importer.side_effects).cast_mut()) };

        stmt_infos.infos.iter_mut_enumerated().for_each(|(_stmt_idx, stmt_info)| {
          stmt_info.import_records.iter().for_each(|rec_id| {
            let rec = &importer.import_records[*rec_id];
            if rec.is_dummy() {
              if matches!(rec.kind, ImportKind::Require) {
                if self.options.format.should_call_runtime_require()
                  && self.options.polyfill_require_for_esm_format_with_node_platform()
                {
                  stmt_info
                    .referenced_symbols
                    .push(self.runtime.resolve_symbol("__require").into());
                  record_meta_pairs.push((*rec_id, ImportRecordMeta::CALL_RUNTIME_REQUIRE));
                }
              }
              return;
            }
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
                            *importer_side_effect = DeterminedSideEffects::Analyzed(true);
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
                          *importer_side_effect = DeterminedSideEffects::Analyzed(true);
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
                          // - import * as bar from 'bar_cjs'
                          // - import { prop } from 'bar_cjs'
                          // will be removed in the final bundler. Nothing need to do here.
                          // stmt_info.side_effect = importee.side_effects.has_side_effects();

                          // `require_bar_cjs`
                          // stmt_info
                          //   .referenced_symbols
                          //   .push(importee_linking_info.wrapper_ref.unwrap().into());
                        }
                      }
                      WrapKind::Esm => {
                        *importer_side_effect = DeterminedSideEffects::Analyzed(true);
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
                  ImportKind::NewUrl | ImportKind::HotAccept => {}
                }
              }
            }
          });
          if keep_names && stmt_info.meta.intersects(StmtInfoMeta::KeepNamesType) {
            stmt_info.referenced_symbols.push(self.runtime.resolve_symbol("__name").into());
          }
        });
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
}
