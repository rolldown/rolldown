use std::ptr::addr_of;

use rolldown_common::{ExportsKind, ImportKind, ImportRecordMeta, Module, OutputFormat, WrapKind};

use crate::utils::external_import_interop::import_record_needs_interop;

use super::LinkStage;

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn determine_module_exports_kind(&mut self) {
    self.module_table.modules.iter().filter_map(Module::as_normal).for_each(|importer| {
      // TODO(hyf0): should check if importer is a js module
      importer
        .import_records
        .iter()
        .filter_map(|rec| rec.resolved_module.map(|module_idx| (rec, module_idx)))
        .for_each(|(rec, module_idx)| {
          let Module::Normal(importee) = &self.module_table[module_idx] else {
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
                  (&mut (*importee_mut)).exports_kind = ExportsKind::Esm;
                }
              }
            }
            ImportKind::Require => match importee.exports_kind {
              ExportsKind::Esm => {
                self.metas[importee.idx].sync_wrap_kind(WrapKind::Esm);
              }
              ExportsKind::CommonJs => {
                self.metas[importee.idx].sync_wrap_kind(WrapKind::Cjs);
              }
              ExportsKind::None => {
                self.metas[importee.idx].sync_wrap_kind(WrapKind::Cjs);
                // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
                // A module with `ExportsKind::None` that `require` self should be turned into `ExportsKind::CommonJs`.
                unsafe {
                  let importee_mut = addr_of!(*importee).cast_mut();
                  (&mut (*importee_mut)).exports_kind = ExportsKind::CommonJs;
                }
              }
            },
            ImportKind::DynamicImport => {
              if self.options.code_splitting.is_disabled() {
                // For iife, then import() is just a require() that
                // returns a promise, so the imported file must also be wrapped
                match importee.exports_kind {
                  ExportsKind::Esm => {
                    self.metas[importee.idx].sync_wrap_kind(WrapKind::Esm);
                  }
                  ExportsKind::CommonJs => {
                    self.metas[importee.idx].sync_wrap_kind(WrapKind::Cjs);
                  }
                  ExportsKind::None => {
                    self.metas[importee.idx].sync_wrap_kind(WrapKind::Cjs);
                    // SAFETY: If `importee` and `importer` are different, so this is safe. If they are the same, then behaviors are still expected.
                    // A module with `ExportsKind::None` that `require` self should be turned into `ExportsKind::CommonJs`.
                    unsafe {
                      let importee_mut = addr_of!(*importee).cast_mut();
                      (&mut (*importee_mut)).exports_kind = ExportsKind::CommonJs;
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
            ImportKind::NewUrl | ImportKind::HotAccept => {}
          }
        });

      let is_entry = self.entries.contains_key(&importer.idx);
      if matches!(importer.exports_kind, ExportsKind::CommonJs)
        && (!is_entry || matches!(self.options.format, OutputFormat::Esm))
      {
        self.metas[importer.idx].sync_wrap_kind(WrapKind::Cjs);
      }
    });
  }

  /// Builds the `safely_merge_cjs_ns_map` which groups ESM imports of the same CommonJS module.
  ///
  /// This optimization allows multiple ESM imports of the same CommonJS module to share
  /// a single namespace binding, reducing code size.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn determine_safely_merge_cjs_ns(&mut self) {
    self.safely_merge_cjs_ns_map.clear();

    for importer in self.module_table.modules.iter().filter_map(Module::as_normal) {
      for (rec_idx, rec) in importer.import_records.iter_enumerated() {
        if !matches!(rec.kind, ImportKind::Import)
          || rec.meta.contains(ImportRecordMeta::IsExportStar)
        {
          continue;
        }

        if let Some(importee) =
          rec.resolved_module.and_then(|importee_idx| self.module_table[importee_idx].as_normal())
          && matches!(importee.exports_kind, ExportsKind::CommonJs)
        {
          let info = self.safely_merge_cjs_ns_map.entry(importee.idx).or_default();
          info.namespace_refs.push(rec.namespace_ref);
          info.needs_interop |= import_record_needs_interop(importer, rec_idx);
        }
      }
    }
  }
}
