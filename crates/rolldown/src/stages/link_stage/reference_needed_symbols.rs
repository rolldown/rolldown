use std::ptr::addr_of;

use rolldown_common::{
  ConcatenateWrappedModuleKind, ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta, Module,
  ModuleIdx, OutputFormat, RuntimeHelper, Specifier, StmtInfoIdx, StmtInfoMeta, SymbolOrMemberExprRef,
  SymbolRef, SymbolRefDb, TaggedSymbolRef, WrapKind,
};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  concat_string,
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
};

use super::LinkStage;
use crate::utils::external_import_interop::import_record_needs_interop;

struct DeferUpdateInfo {
  record_meta_pairs: Vec<(ImportRecordIdx, ImportRecordMeta)>,
}
impl LinkStage<'_> {
  #[expect(clippy::too_many_lines)]
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn reference_needed_symbols(&mut self) {
    // Since each module only access its own symbol ref db, we use zip rather than a Mutex to
    // access the symbol db in parallel.
    let has_module_preserve_jsx = self.symbols.has_module_preserve_jsx();
    let old_symbol_db = std::mem::take(&mut self.symbols);
    let mut symbols_inner = old_symbol_db.into_inner();
    let keep_names = self.options.keep_names;
    let commonjs_treeshake = self.options.treeshake.commonjs();

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
          // Handle non-static dynamic imports like `import(foo)` or `import('a' + 'b')`
          if stmt_info.meta.intersects(StmtInfoMeta::NonStaticDynamicImport) {
            depended_runtime_helper_map[RuntimeHelper::ToEsm.bit_index()].push(stmt_info_idx);
          }
          stmt_info.import_records.iter().for_each(|rec_id| {
            let rec = &importer.import_records[*rec_id];
            let Some(module_idx) = rec.state.resolved_module else { return };
            match &self.module_table[module_idx] {
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
                  ImportKind::Require => {
                    if self.options.format.should_call_runtime_require()
                      && self.options.polyfill_require_for_esm_format_with_node_platform()
                    {
                      stmt_info
                        .referenced_symbols
                        .push(self.runtime.resolve_symbol("__require").into());
                      record_meta_pairs.push((*rec_id, ImportRecordMeta::CallRuntimeRequire));
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
                          stmt_info.side_effect = importee.side_effects.has_side_effects().into();

                          // Turn `import * as bar from 'bar_cjs'` into `var import_bar_cjs = __toESM(require_bar_cjs())`
                          // Turn `import foo from 'bar_cjs'; foo;` into `var import_bar_cjs = __toESM(require_bar_cjs()); import_bar_cjs.default;`
                          // Turn `import { prop } from 'bar_cjs'; prop;` into `var import_bar_cjs = require_bar_cjs(); import_bar_cjs.prop;`
                          // Reference to `require_bar_cjs`
                          stmt_info
                            .referenced_symbols
                            .push(importee_linking_info.wrapper_ref.unwrap().into());
                          // Only reference __toESM if this import needs interop (namespace or default import)
                          let needs_toesm =
                            if let Some(info) = self.safely_merge_cjs_ns_map.get(&importee.idx) {
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
                        stmt_info.side_effect =
                          (is_reexport_all || importee.side_effects.has_side_effects()).into();
                        // Reference to `init_foo`
                        stmt_info
                          .referenced_symbols
                          .push(importee_linking_info.wrapper_ref.unwrap().into());

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
                    if self.options.code_splitting.is_disabled() {
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
                          // `import('./some-cjs-module.js').then((m) => __toESM(m.default, isNodeMode))`
                          depended_runtime_helper_map[RuntimeHelper::ToEsm.bit_index()]
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
        (importer_idx, DeferUpdateInfo { record_meta_pairs })
      })
      .collect::<Vec<_>>();

    // Since `par_iter + collect` could ensure the order of items is the same as original,
    // we could just push items in order here.
    for (module_idx, defer_update_info) in defer_update_info_list {
      let Some(module) = self.module_table[module_idx].as_normal_mut() else {
        continue;
      };
      for (rec_id, meta) in defer_update_info.record_meta_pairs {
        module.import_records[rec_id].meta |= meta;
      }
    }

    let mut symbols = SymbolRefDb::new().with_inner(symbols_inner);
    if has_module_preserve_jsx {
      symbols.set_has_module_preserve_jsx();
    }
    self.symbols = symbols;

    // Post-processing pass (sequential, after symbols are fully restored):
    //
    // For named imports from a `WrapKind::None` importee (e.g. a re-export barrel module),
    // check whether the imported symbol ultimately resolves — via the canonical-ref chain —
    // to a `WrapKind::Esm` module.  If so, add that module's wrapper ref (`init_xxx`) to the
    // import statement's `referenced_symbols`.
    //
    // Why here and not in the parallel pass above?  In the parallel pass `self.symbols` is
    // temporarily taken apart (`into_inner`) so `canonical_ref_for` is not available.  We
    // need the full `SymbolRefDb` to follow the link chain across modules.
    //
    // Why this is needed:
    //   The module finalizer generates an `init_xxx()` call when it sees that the imported
    //   symbol's canonical owner has `WrapKind::Esm` (see
    //   `generate_init_calls_for_transitive_esm_modules`).  But for that call to be valid
    //   in the output, `init_xxx` must be (a) exported from its chunk and (b) imported by
    //   the importer's chunk.  Both are driven by the symbol appearing in
    //   `referenced_symbols` of an included statement.
    self.reference_transitive_esm_wrappers();
  }

  /// Sequential post-pass: for import statements whose direct importee has `WrapKind::None`,
  /// add the `wrapper_ref` of every transitively-referenced `WrapKind::Esm` module to the
  /// statement's `referenced_symbols`.
  fn reference_transitive_esm_wrappers(&mut self) {
    // Phase 1 – collect: gather (module_idx, stmt_info_idx, wrapper_ref) triples.
    // All borrows here are immutable.
    let mut additions: Vec<(ModuleIdx, StmtInfoIdx, SymbolRef)> = Vec::new();

    for (module_idx, module) in self.module_table.modules.iter_enumerated() {
      let Some(importer) = module.as_normal() else { continue };
      for (stmt_info_idx, stmt_info) in importer.stmt_infos.iter_enumerated() {
        for rec_id in &stmt_info.import_records {
          let rec = &importer.import_records[*rec_id];
          // Only care about static `import … from '…'` statements (not require / dynamic).
          if !matches!(rec.kind, ImportKind::Import) { continue }
          // `export * from '…'` is handled elsewhere; skip.
          if rec.meta.contains(ImportRecordMeta::IsExportStar) { continue }
          let Some(importee_module_idx) = rec.resolved_module else { continue };
          let Some(importee) = self.module_table[importee_module_idx].as_normal() else { continue };
          let importee_linking_info = &self.metas[importee.idx];
          if !matches!(importee_linking_info.wrap_kind(), WrapKind::None) { continue }

          // The direct importee has WrapKind::None.  Walk each named import to find the
          // canonical defining module; if it is WrapKind::Esm, record its wrapper ref.
          for named_import in importer.named_imports.values().filter(|ni| ni.record_idx == *rec_id) {
            if !matches!(named_import.imported, Specifier::Literal(_)) { continue }
            let canonical = self.symbols.canonical_ref_for(named_import.imported_as);
            let owner_idx = canonical.owner;
            let owner_li = &self.metas[owner_idx];
            if matches!(owner_li.wrap_kind(), WrapKind::Esm)
              && !matches!(
                owner_li.concatenated_wrapped_module_kind,
                ConcatenateWrappedModuleKind::Inner
              )
            {
              if let Some(wrapper_ref) = owner_li.wrapper_ref {
                additions.push((module_idx, stmt_info_idx, wrapper_ref));
              }
            }
          }
        }
      }
    }

    // Phase 2 – apply: push wrapper refs to the appropriate stmt_info, deduplicating with a
    // small per-statement set so that multiple named imports from the same barrel don't yield
    // duplicate `init_xxx()` calls.
    for (module_idx, stmt_info_idx, wrapper_ref) in additions {
      let Some(module) = self.module_table[module_idx].as_normal_mut() else { continue };
      let referenced = &mut module.stmt_infos.infos[stmt_info_idx].referenced_symbols;
      let already_present = referenced
        .iter()
        .any(|r| matches!(r, SymbolOrMemberExprRef::Symbol(s) if *s == wrapper_ref));
      if !already_present {
        referenced.push(wrapper_ref.into());
      }
    }
  }
}
