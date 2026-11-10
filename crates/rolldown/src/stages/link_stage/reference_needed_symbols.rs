use rolldown_common::{
  ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta, Module, OutputFormat, RuntimeHelper,
  StmtInfoMeta, SymbolOrMemberExprRef, SymbolRefDb, TaggedSymbolRef, WrapKind,
};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  concat_string,
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
};

use super::LinkStage;
use crate::types::linking_metadata::DeferredAccess;
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
    // The build has one chunk: code splitting is off, and at most one module is a user-defined or
    // emitted entry. `self.entries` cannot be counted directly: at this point it also holds each
    // `import()` target, which only `include_statements` removes when code splitting is off.
    // Only the modules that `import defer` wraps use it, so a build without `importDefer` does
    // not count the entries.
    let single_chunk_build = self.options.experimental.is_import_defer_enabled()
      && self.options.code_splitting.is_disabled()
      && self
        .entries
        .values()
        .filter(|entry_points| entry_points.iter().any(|entry| entry.kind.is_user_defined()))
        .count()
        <= 1;

    let defer_update_info_list = self
      .module_table
      .modules
      .par_iter()
      .zip(symbols_inner.par_iter_mut())
      .zip(self.depended_runtime_helper.par_iter_mut())
      .zip(self.stmt_infos.par_iter_mut())
      .filter_map(|(((module, symbol_db), depended_helper), stmt_infos)| {
        module.as_normal().map(|importer| (importer, symbol_db, depended_helper, stmt_infos))
      })
      .map(|(importer, symbol_ref_for_module, depended_runtime_helper_map, stmt_infos)| {
        let symbol_db =
          symbol_ref_for_module.as_mut().expect("normal module should have symbol db");
        let mut record_meta_pairs: Vec<(ImportRecordIdx, ImportRecordMeta)> = vec![];
        let importer_idx = importer.idx;
        let mut symbols_to_be_declared = vec![];
        let deferred_access_targets = &self.metas[importer_idx].deferred_access_targets;
        stmt_infos.infos.iter_mut_enumerated().for_each(|(stmt_info_idx, stmt_info)| {
          // A deferred access (`ns.foo`) calls the wrapper of the deferred module, through
          // `__esmDefer` when the module is tracked.
          if !deferred_access_targets.is_empty() {
            let mut targets = stmt_info
              .referenced_symbols
              .iter()
              .filter_map(|symbol| match symbol {
                SymbolOrMemberExprRef::MemberExpr(member) => {
                  deferred_access_targets.get(&member.node_id)
                }
                SymbolOrMemberExprRef::Symbol(_) => None,
              })
              .filter_map(|access| match *access {
                DeferredAccess::Esm(target) => Some(target),
                DeferredAccess::Cjs(rec_idx) => importer.import_records[rec_idx].resolved_module,
              })
              .collect::<Vec<_>>();
            targets.sort_unstable();
            targets.dedup();
            if targets.iter().any(|&target| self.metas[target].import_defer_tracked_deps.is_some())
            {
              stmt_info.referenced_symbols.push(self.runtime.resolve_symbol("__esmDefer").into());
            }
            stmt_info.referenced_symbols.extend(
              targets
                .into_iter()
                .filter_map(|target| self.metas[target].wrapper_ref)
                .map(Into::into),
            );
          }
          if stmt_info.meta.contains(StmtInfoMeta::HasDummyRecord) {
            depended_runtime_helper_map.push(RuntimeHelper::Require, stmt_info_idx);
          }
          // Handle non-static dynamic imports like `import(foo)` or `import('a' + 'b')`
          if stmt_info.meta.intersects(StmtInfoMeta::NonStaticDynamicImport)
            && matches!(self.options.format, OutputFormat::Cjs)
            && !self.options.dynamic_import_in_cjs
          {
            depended_runtime_helper_map.push(RuntimeHelper::ToEsm, stmt_info_idx);
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
                      // A module of a deferred closure in CommonJS output calls
                      // `__reExport(x_exports, require("ext"))` at this statement, in its init.
                      if matches!(self.options.format, OutputFormat::Cjs)
                        && self.metas[importer_idx].wrapped_by_import_defer
                      {
                        stmt_info.eval_flags = true.into();
                        depended_runtime_helper_map.push(RuntimeHelper::ReExport, stmt_info_idx);
                        stmt_info.referenced_symbols.push(importer.namespace_object_ref.into());
                      }
                    } else if rec.is_defer_phase()
                      && matches!(self.options.format, OutputFormat::Cjs)
                    {
                      // `import defer * as ns from 'ext'` in CommonJS output becomes
                      // `var import_ext;`, and the first deferred access assigns
                      // `__toESMCached(require("ext"))` to it. The statement evaluates nothing.
                      depended_runtime_helper_map.push(RuntimeHelper::ToEsmCached, stmt_info_idx);
                      symbols_to_be_declared.push((rec.namespace_ref, stmt_info_idx));
                      symbol_db.ast_scopes.set_symbol_name(
                        rec.namespace_ref.symbol,
                        &concat_string!("import_", &importee.identifier_name),
                      );
                    } else if matches!(self.options.format, OutputFormat::Cjs)
                      && self.metas[importer_idx].wrapped_by_import_defer
                    {
                      // A module of a deferred closure in CommonJS output loads the external at
                      // the position of the import, in its init: `var import_ext = require("ext")`
                      // (`lazy_external_import_stmt`). Tree shaking keeps the statement when the
                      // external has side effects, or when a read of `import_ext` stays.
                      stmt_info.eval_flags = importee.side_effects.has_side_effects().into();
                      if import_record_needs_interop(importer, *rec_id) {
                        depended_runtime_helper_map.push(RuntimeHelper::ToEsmCached, stmt_info_idx);
                      }
                      symbols_to_be_declared.push((rec.namespace_ref, stmt_info_idx));
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
                        stmt_info.eval_flags = true.into();
                        // Only reference __toESM if this import needs interop (namespace or default import)
                        if import_record_needs_interop(importer, *rec_id) {
                          depended_runtime_helper_map.push(RuntimeHelper::ToEsm, stmt_info_idx);
                        }
                      }
                    }
                  }
                  ImportKind::Require
                    if self.options.format.should_call_runtime_require()
                      && self.options.polyfill_require_for_esm_format_with_node_platform() =>
                  {
                    stmt_info
                      .referenced_symbols
                      .push(self.runtime.resolve_symbol("__require").into());
                    record_meta_pairs.push((*rec_id, ImportRecordMeta::CallRuntimeRequire));
                  }
                  ImportKind::DynamicImport
                    if rec.is_defer_phase() && matches!(self.options.format, OutputFormat::Cjs) =>
                  {
                    // `(async () => __deferNs(() => __toESMCached(require("ext"))))()`
                    depended_runtime_helper_map.push(RuntimeHelper::ToEsmCached, stmt_info_idx);
                    depended_runtime_helper_map.push(RuntimeHelper::DeferNs, stmt_info_idx);
                  }
                  ImportKind::DynamicImport
                    if matches!(self.options.format, OutputFormat::Cjs)
                      && !self.options.dynamic_import_in_cjs =>
                  {
                    // When format is CJS and dynamicImportInCjs is false, we need __toESM
                    // to wrap the require call: `Promise.resolve().then(() => __toESM(require("external")))`
                    depended_runtime_helper_map.push(RuntimeHelper::ToEsm, stmt_info_idx);
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
                            stmt_info.eval_flags = true.into();
                            stmt_info.meta.insert(StmtInfoMeta::ReExportDynamicExports);
                            depended_runtime_helper_map
                              .push(RuntimeHelper::ReExport, stmt_info_idx);
                            stmt_info.referenced_symbols.push(importer.namespace_object_ref.into());
                            stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
                          }
                        }
                      }
                      WrapKind::Cjs => {
                        if is_reexport_all {
                          stmt_info.eval_flags = true.into();
                          // Turn `export * from 'bar_cjs'` into `__reExport(foo_exports, __toESM(require_bar_cjs()))`
                          // Reference to `require_bar_cjs`
                          stmt_info
                            .referenced_symbols
                            .push(importee_linking_info.wrapper_ref.unwrap().into());
                          depended_runtime_helper_map.push(RuntimeHelper::ToEsm, stmt_info_idx);
                          depended_runtime_helper_map.push(RuntimeHelper::ReExport, stmt_info_idx);
                          if !commonjs_treeshake {
                            stmt_info.referenced_symbols.push(importer.namespace_object_ref.into());
                          }
                        } else {
                          stmt_info.eval_flags = importee.side_effects.has_side_effects().into();

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
                            depended_runtime_helper_map.push(RuntimeHelper::ToEsm, stmt_info_idx);
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
                        //
                        // An `export *` of a wrapped importee always runs, so that the init of the
                        // importer calls the init of the importee. In a deferred closure of a build
                        // with one chunk, the eager rule applies instead: the statement stays only
                        // when it has side effects or a used binding. The excluded-statement
                        // metadata still emits the init call of each included target, because each
                        // target is in the one chunk. With more chunks, a target in another chunk
                        // would lose that call.
                        // See internal-docs/linking/reference-needed-symbols/implementation.md.
                        let reexport_all_always_runs = is_reexport_all
                          && !(single_chunk_build
                            && self.metas[importer_idx].wrapped_by_import_defer
                            && !importee_linking_info.has_dynamic_exports);
                        stmt_info.eval_flags = (reexport_all_always_runs
                          || importee.side_effects.has_side_effects())
                        .into();
                        // Reference to `init_foo`
                        stmt_info
                          .referenced_symbols
                          .push(importee_linking_info.wrapper_ref.unwrap().into());
                        if importee_linking_info.import_defer_tracked_deps.is_some()
                          && importee_linking_info.is_tla_or_contains_tla_dependency
                        {
                          if rec.is_defer_phase() {
                            // The preparation, `__esmGather(init_foo, init_bar) || await
                            // __esmGather()`, must stay also when `foo` has no side effects.
                            stmt_info.eval_flags = true.into();
                            stmt_info
                              .referenced_symbols
                              .push(self.runtime.resolve_symbol("__esmGather").into());
                          } else if self.metas[importer_idx].import_defer_tracked_deps.is_some() {
                            // `__esmDepDone(init_foo(init_bar), init_foo, init_bar) || await ...`
                            stmt_info
                              .referenced_symbols
                              .push(self.runtime.resolve_symbol("__esmDepDone").into());
                          }
                        }

                        if is_reexport_all && importee_linking_info.has_dynamic_exports {
                          // Turn `export * from 'bar_esm'` into `init_bar_esm();__reExport(foo_exports, bar_esm_exports);`
                          // something like `__reExport(foo_exports, other_exports)`
                          depended_runtime_helper_map.push(RuntimeHelper::ReExport, stmt_info_idx);
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
                        depended_runtime_helper_map.push(RuntimeHelper::ToCommonJs, stmt_info_idx);
                      }
                    }
                  },
                  ImportKind::DynamicImport => {
                    let is_dynamic_defer =
                      rec.is_defer_phase() && importee_linking_info.dynamic_defer_target;
                    if is_dynamic_defer
                      && importee_linking_info.import_defer_tracked_deps.is_some()
                      && importee_linking_info.is_tla_or_contains_tla_dependency
                    {
                      // `import.defer()` first waits for the top-level await that a deferred
                      // access to `foo` can evaluate: `__esmGather(init_foo) || await __esmGather()`.
                      stmt_info
                        .referenced_symbols
                        .push(self.runtime.resolve_symbol("__esmGather").into());
                    }
                    if self.options.code_splitting.is_disabled() {
                      match importee_linking_info.wrap_kind() {
                        WrapKind::None => {}
                        WrapKind::Cjs => {
                          //  `__toESM(require_foo())`
                          stmt_info
                            .referenced_symbols
                            .push(importee_linking_info.wrapper_ref.unwrap().into());
                          depended_runtime_helper_map.push(RuntimeHelper::ToEsm, stmt_info_idx);
                        }
                        WrapKind::Esm => {
                          // `(init_foo(), foo_exports)`
                          stmt_info
                            .referenced_symbols
                            .push(importee_linking_info.wrapper_ref.unwrap().into());
                          stmt_info.referenced_symbols.push(importee.namespace_object_ref.into());
                          // `import.defer()` gives `foo_deferred`.
                          if is_dynamic_defer
                            && let Some(deferred_ref) = importee_linking_info.deferred_namespace_ref
                          {
                            stmt_info.referenced_symbols.push(deferred_ref.into());
                          }
                        }
                      }
                    } else {
                      match &importee.exports_kind {
                        ExportsKind::CommonJs => {
                          // `import('./some-cjs-module.js')` would be converted to
                          // `import('./some-cjs-module.js').then((m) => __toESM(m.default, isNodeMode))`
                          depended_runtime_helper_map.push(RuntimeHelper::ToEsm, stmt_info_idx);
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
            depended_runtime_helper_map.push(RuntimeHelper::Name, stmt_info_idx);
          }
        });

        symbols_to_be_declared.into_iter().for_each(|(symbol_ref, idx)| {
          stmt_infos.declare_symbol_for_stmt(idx, TaggedSymbolRef::normal(symbol_ref));
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
  }
}
